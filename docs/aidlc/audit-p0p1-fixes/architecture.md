# Architecture: audit-p0p1-fixes

> 产出者：architect　日期：2026-10-14　来源：intent.md + audit-findings.md + 源码核实（逐 F-xx 对照 D:\OneDrive\Devops\cloudmount 当前 HEAD）

## 逻辑架构

### 组件与边界（不变量：本 intent 不新增 crate、不改变既有依赖方向）

```text
cloudmount-core        Error/Provider trait/config.toml schema
   ▲            ▲            ▲              ▲
   │            │            │              │
cloudmount-vfs  cloudmount-sync  cloudmount-auth  cloudmount-driver115
 (cfapi/cfapi_mount) (rdc/exec)   (open/cookie/store) (router/cookie 驱动)
   ▲            ▲
   └── cloudmount-daemon（main / webdav / service）──► apps/desktop（不在本 intent 触及面）
```

各缺陷的归属边界（修复全部在既有 crate 内完成）：

| crate | 触及的缺陷 | 触及模块 |
|---|---|---|
| cloudmount-vfs | F-01 | `backends/cfapi.rs`（HydrateFn 契约 + FETCH_DATA 回调）、`backends/cfapi_mount.rs`（水合闭包） |
| cloudmount-sync | F-06、F-07 | `rdc.rs`、`exec.rs` |
| cloudmount-auth | F-10（+F-14 测试注入面） | `open.rs`（`with_base_url` 测试注入，沿用 cookie.rs 既有模式） |
| cloudmount-driver115 | F-02、F-04、F-11、F-14 | `router.rs`、`cookie.rs` |
| cloudmount-daemon | F-03、F-05、F-08、F-09、F-12、F-13 | `webdav.rs`、`main.rs`、`service.rs` |
| cloudmount-core | F-05（配置 schema） | `config.rs` 新增 `[cookie]` 子配置 |
| cloudmount-cli | F-05（配套：登录时持久化同意标志） | `main.rs::cmd_login_cookie` |

### 逐项修复设计（F-01…F-14，按 crate 分组）

#### cloudmount-vfs

**F-01（P0）CfAPI 水合全量读入内存**
- 现状核实：`cfapi_mount.rs:113-139` 闭包 `Vec::with_capacity(end-start+1)` + `read_to_end`；`cfapi.rs:586` `HydrateFn = Fn(&str,u64,u64)->Result<Vec<u8>>`；`cfapi.rs:729-756` FETCH_DATA 一次性 `transfer_data` 全量缓冲；`:315` 策略为 `CF_HYDRATION_POLICY_FULL`。
- 修复策略：把 `HydrateFn` 从"返回整块 Vec"改为"分块推送"契约：
  ```rust
  pub trait ChunkSink { fn transfer(&mut self, offset: u64, data: &[u8]) -> Result<()>; }
  pub type HydrateFn = Box<dyn Fn(&str, u64, u64, &mut dyn ChunkSink) -> Result<()> + Send + Sync>;
  ```
  - `cfapi.rs::fetch_data_callback` 构造 sink：每个块先查 `ctx.cancelled`（取消 → `Err`，停止读取并丢弃 reader），再 `transfer_data(info, offset, Some(chunk), 0)`；水合成功路径的水合日志（mtime 记录）移到**最后一个块成功之后**整体记录（语义不变：仍是"水合完成后"记一次）。
  - `cfapi_mount.rs` 水合闭包：寻址远端条目与范围钳制逻辑不变；`provider.open_range(RangeSpec{start,end})` **仅调用一次**，循环读入**单个复用的 1 MiB 缓冲**（`Vec::with_capacity(CHUNK)`，零重分配），逐块 `sink.transfer(start+consumed, &buf[..n])`。中间任一读错误 → 返回 `Err` → 回调以 `transfer_data(None, STATUS_UNSUCCESSFUL)` 结束挂起读（沿用现有错误收尾路径）。
  - 块大小 `const HYDRATION_CHUNK: usize = 1 MiB`，取值理由见关键决策 #2。
  - `CF_HYDRATION_POLICY_FULL` **不变**（改策略改变产品可见行为，且分块后 OOM 根因已消除）。
- 背压/取消语义：`CfExecute(TRANSFER_DATA)` 同步返回 → 天然背压（永远最多一个块在途）；取消经 `CANCEL_FETCH_DATA` 已有的 `cancelled: Mutex<HashSet<TransferKey>>`，sink 在每块发送前检查，取消后立即 drop reader（关闭下游 HTTP 连接）。
- 调用方影响：`HydrateFn` 是 crate 内契约，全部调用点共 3 处——`cfapi_mount.rs:113`（本修复主体）、`tests/cfapi_hydration.rs:188/243`（机械适配新签名）。`MountBackend::hydrate`（`cfapi_mount.rs:176-181`，范围由调用方给定、非 FETCH_DATA 路径）保持不变。
- 回归风险：中。TRANSFER_DATA 分块是文档化模式（CloudMirror 样例同款），风险点在"部分传输后取消/失败"的过滤器行为——由 cfapi_hydration 集成测试补"分块读取大范围""中途取消""中途失败"三个用例压住。

#### cloudmount-sync

**F-06 RdcWatcher Drop 空操作**
- 现状核实：`rdc.rs:141-147` `drop(self.handle.clone())` 克隆后立刻丢副本，Arc 引用计数净变化为零；线程持有自己的 `Arc<RawHandle>` 克隆，故 watcher 侧永远关不掉句柄；`JoinHandle` 在 `new()` 中被 `let _thread =` 丢弃。`shutdown` 标志已存在但只在读失败路径被检查。
- 修复策略（所有权重构）：
  - `RawHandle` 增加显式 `close(&self)`：内部 `closed: AtomicBool` 守卫，`swap(true)` 成功才 `CloseHandle`（幂等，防 OS 句柄值复用被误关）；`RawHandle::drop` 复用同一守卫。
  - `RdcWatcher` 字段改为 `handle: Option<Arc<RawHandle>>`、`thread: Option<JoinHandle<()>>`。
  - `Drop`：`shutdown.store(true)` → `handle.take()` 并 `close()`（关闭句柄使阻塞中的 `ReadDirectoryChangesW` 以失败返回，这是文档化取消方式，`event_loop` 的 `Err` 分支已按此退出）→ `thread.take().join()`。join 有界：挂起调用被句柄关闭唤醒；`tx.send` 在无界 mpsc 上永不阻塞。
- 调用方影响：无外部 API 变化（`RdcWatcher::new/poll/snapshot/root` 不变）。
- 回归风险：低。测试：循环创建/丢弃 N 个 watcher 后新 watcher 仍能收到事件（防句柄耗尽回归）；poll 功能既有测试压住。

**F-07 ensure_remote_dir 持锁做网络 IO**
- 现状核实：`exec.rs:454-499`，`dir_ids: Mutex<HashMap<String, RemoteId>>`（`exec.rs:46`）在 `provider.list`（逐组件查重）与 `provider.create_dir` 整个网络往返期间被持有；所有并发上传/建目录动作都在这把锁后串行。
- 修复策略：两阶段 + per-path 创建锁：
  1. **锁内只做计划**：拿 `dir_ids` 锁 → 从根沿 `rel` 组件走 map，得出"已解析到的当前 ID"与缺失前缀链 `Vec<(prefix, comp)>` → 立即放锁。
  2. **锁外做网络**：对每个缺失前缀——取 `dir_creating: Mutex<HashMap<String, Arc<Mutex<()>>>>` 中的 per-prefix 锁（取锁本身不持 map 锁过夜，只克隆 Arc）→ **双检** `dir_ids`（其它线程可能已完成）→ 仍缺失才做 `provider.list` 查重 / `provider.create_dir` → 结果写回 `dir_ids`。
  - 锁序固定为"父前缀先于子前缀"（与遍历顺序一致），无环，无死锁。`dir_creating` 中已完成的条目可在插入 `dir_ids` 时顺手移除，防 map 无界增长。
- 调用方影响：`ensure_remote_dir` 私有方法签名不变；"并发上传不重复建远端目录"的确定性由 per-prefix 锁 + 双检保持（`exec.rs:9` 声明的安全性设计不回退）。
- 回归风险：低-中。测试：并发 N 线程对同一深层目录 `ensure_remote_dir`，断言远端 `create_dir` 恰好一次、全部返回同一 ID；弱网（慢 mock provider）期间其它动作不被阻塞的时序断言。

#### cloudmount-auth

**F-10 OpenApiAuth 非单例刷新**
- 现状核实：`open.rs:163-193`，快路径拿锁检查 → 释放 → `do_refresh`（网络）→ `persist`（重新拿锁）。两线程同时走到 `drop(guard)` 后各自刷新 → 115 侧 40140117（旧 refresh_token 发出的新 token 立即失效互踩）。
- 修复策略：**同一把 `tokens` 锁跨网络调用**（锁即 single-flight），伪码：
  ```rust
  let mut guard = self.tokens.lock()...;
  // 快路径 + 未登录检查（同现有）
  let refresh_token = guard.as_ref()...refresh_token.clone();
  let ts = self.do_refresh(&refresh_token)?;   // 持锁跨网络；do_refresh 只用 self.agent，无重入
  let blob = serde_json::to_vec(&ts)?;
  *guard = Some(ts);                            // 经守卫写回，绝不调 persist()（会重入死锁）
  drop(guard);
  self.store.put(SecretKind::OpenApiToken, &blob)?;  // 持久化移到锁外
  ```
  等待者持锁阻塞至刷新完成 → 恢复后走快路径命中新 token。失败路径：`?` 提前释放锁，等待者依次（串行，由锁保证）重试——40140117 只由**并发**成功刷新触发，串行重试是合法形态。
- 配套（测试使能）：`OpenApiAuth` 沿用 `cloudmount-auth::cookie::CookieAuth::with_base_url` 既有模式，增加 `passport_base()` 覆盖（`#[doc(hidden)]`），使 F-10 的失败测试可用本地 mock HTTP 压住（并发 N 调用者断言刷新请求恰好 1 次）。
- 调用方影响：`access_token()` 签名不变；副作用：刷新期间（≤30s 全局超时）`logout/is_logged_in` 会阻塞等待——语义正确（登出发生在刷新之后，最终状态一致）。
- 回归风险：低。既有 token 恢复/登出测试压住；新增并发单飞测试。

#### cloudmount-driver115

**F-02 ProviderRouter 未转发 change_probe**
- 现状核实：`router.rs:162-212` `impl Provider for ProviderRouter` 确无 `change_probe`，走 trait 默认 `Ok(None)`；`cloudmount-core/provider.rs:93`、`cloudmount-sync/remote.rs:105` 均按 `None` 回退全量扫描。
- 修复策略：在 impl 中加一行转发 `fn change_probe(&self, dir) -> Result<Option<ChangeProbe>> { self.call(|p| p.change_probe(dir)) }`——免费获得既有 `call()` 的 Auth 失效自动切换语义。Cookie 通道未实现 `change_probe`（默认 `None`）时行为正确：引擎对该通道回退全量扫描。
- 调用方影响：无。回归风险：极低。测试：router 挂支持 probe 的 stub，断言 probe 值透传 + Auth 失效时随通道切换。

**F-04 cookie.rs 调试日志泄漏凭据**
- 现状核实：`cookie.rs:401`（uploadinfo 原始 JSON，含 userkey）、`:492`（initupload 表单 `?form`，含 userkey/sig/token）、`:560`（gettoken 原始响应，含 STS AccessKeyId/Secret/SecurityToken）。
- 修复策略：三处全部改为**字段级脱敏**——只记字段名 + 值长度（如 `userkey_len = 64`），一律不记值。提取纯函数 `redact_fields(&[(k,v)]) -> String`（输出 `k:len` 列表）供三处复用；对 uploadinfo/gettoken 的顶层 JSON 只记 `state/errno/size_limit` 等非敏感标量字段名。
- 回归测试（无新依赖）：测试内实现 `MakeWriter`（`Arc<Mutex<Vec<u8>>>`）+ `tracing::subscriber::with_default` 捕获 debug 输出，断言不含 userkey/STS 真值。
- 调用方影响：无（日志面）。回归风险：极低。
- 附注：`cookie.rs:525` 解密后的 initupload 响应日志不含凭据（bucket/object/callback），保留；但施工时若顺手可降为 status/pickcode 字段。

**F-11 oss_put 吞 HTTP 状态**
- 现状核实：`cookie.rs:619-635`，agent 以 `http_status_as_error(false)` 构建（`:207`），PUT 后 `read_to_string` 即 `Ok(())`——非 2xx 被当成功。
- 修复策略：`send` 后取 `resp.status()`；非 2xx → `Err(Error::Remote { code: status, message: "OSS 直传失败: <状态行> + 响应体前 2KB 摘要" })`（读体用 `io::Read::take(2048)` 限长；体是 OSS XML 错误，不含凭据）。`Error::Remote` 沿既有执行器重试语义进重试队列（`stats.errors → 下轮重试`），"失败上传当成功"的数据丢失路径关闭。上传成功判定不变量见安全一节。
- 调用方影响：`upload()` 链路失败行为从"假成功"变"真失败+重试"，这是缺陷修复本身，属 intent 声明的允许行为变化。
- 回归风险：中（影响真实上传链路的成功/失败判定）——由 F-14 的 HTTP 级测试压住。

**F-14 Cookie 通道零 HTTP 级测试**
- 现状核实：`cookie.rs` 全文件 1127 行无 HTTP 级测试；`webapi_base/proapi_base` 已可注入（`:220`），但上传三端点硬编码（`:40` UPLOAD_INIT_URL、`:42` UPLOAD_TOKEN_URL、`:44` OSS_ENDPOINT）；`driver115` 无任何 dev-dependencies。
- 修复策略：
  - 注入面：测试位于 `cookie.rs` 的 `#[cfg(test)]` 模块（同 crate 可直接写私有字段），为 `upload_init_url/upload_token_url/oss_endpoint` 增加字段（默认 = 生产常量）+ `#[cfg(test)]` 快速限速构造器（`RateLimiter::new(1000.0, 1.0)`，避免 0.5 r/s 拖慢测试）。**不改公开 API**。
  - 测试服务器：手写最小 HTTP/1.1 responder（`std::net::TcpListener` 绑 `127.0.0.1:0`，按 method+path 路由返回预置状态码与 body）。`CookieAuth` 用 `MemoryStore`（`cloudmount-auth::store`，pub）预置 CookieJar。
  - 覆盖矩阵（审计要求为**错误路径**）：uploadinfo 成功 / `state=false`；gettoken `status_code != 200`；oss_put 500/403 → `Err`（F-11 回归）；oss_put 200 → `Ok`；initupload 传输错误 / 响应不可解密；list/downurl 失效映射。initupload **解密 happy-path 回环**不在本期（需测试侧实现 P-224 ECDH 对端，超出错误路径范围，记入开放问题）。
- 调用方影响：无。回归风险：无（纯增测试）。

#### cloudmount-daemon

**F-03 serve_static 路径遍历（Windows 盘符逃逸）**
- 现状核实：`webdav.rs:922-932`，仅 `rel.contains("..")`；`root.join("C:/foo")` 在 Windows 上 `Path::join` 遇绝对成分直接替换基路径 → 任意盘文件读。UNC（`//server/share`）同理。
- 修复策略：**成分级校验 + canonicalize 双检**两层：
  1. 成分级（先拒，零 IO）：百分号解码后按 `/` 切分，跳过空段，拒绝任一满足如下条件的段——`..` 或 `.`；含 `:`（拦盘符 `C:` 与 NTFS 备用数据流 `file:stream`）；含 `\`（维持单分隔符不变量）；为 Windows 保留设备名（CON/PRN/AUX/NUL/COM1-9/LPT1-9，防打开设备挂起）。重组后 `root.join(所有合法段)`。
  2. canonicalize 兜底：`full.canonicalize()` 后要求 `starts_with(root_canonical)`（Rust 的 `starts_with` 按 Path 组件比较，无前缀串误判；两侧都过 canonicalize，`\\?\` 前缀一致；根的 canonical 形式用 `OnceLock` 缓存一次）。解析失败或越界 → 403。
  - web-ui 目录内的合法 symlink 指向目录外也会被第 2 层拦下（第 1 层拦不住的场景）。
- 调用方影响：`serve_static` 私有函数，签名不变。合法资产路径（`index.html`、`assets/*`）不受影响。
- 回归风险：低。测试：`/C:/Windows/win.ini`、`/..%2F..%2Fwindows/win.ini`（解码后穿越）、`//server/share/x`、`/a/../b`、合法 `/assets/app.js` 各断言 403/200。

**F-05 daemon 硬编码绕过 Cookie 通道开关**
- 现状核实：`main.rs:376-379`（`--channel cookie`）与 `:413-416`（auto 兜底 `try_cookie_provider`）均硬编码 `CookiePolicy { enabled: true, consent_acknowledged: true }`。CLI 侧 `cloudmount-cli/src/main.rs:280-283` 的硬编码属登录流程本身（交互式同意已发生），不属缺陷。
- 修复策略：
  - `cloudmount-core/config.rs` 新增子配置 `CookieChannelConfig { enabled: bool = false, consent_acknowledged: bool = false }`，挂为 `Config.cookie` 字段（`serde(default)` + 既有 `deny_unknown_fields`，旧配置文件完全兼容）。
  - `build_channel_provider` / `try_cookie_provider` 改为从 `file_cfg.cookie` 构造 `CookiePolicy` 传入；`check_enabled()`（`cloudmount-auth/cookie.rs:72`）原样把关：未开启/未同意 → 明确 `Err`（`--channel cookie` 时 fail 并提示"先 `cloudmount login --with-cookie-channel`"），auto 时静默跳过 Cookie 兜底（纯官方）。
  - 配套：`cloudmount-cli::cmd_login_cookie` 登录成功后把 `cookie.enabled=true, consent_acknowledged=true` 写回 `config.toml`（`config::load_from` → 变更 → `save_to`），形成"登录即同意即持久化"闭环。
- 调用方影响：desktop 壳若也构造 CookiePolicy 需同步核对（construction 阶段检查项）。升级行为变化：已登录 Cookie 但无 `[cookie]` 配置段的用户升级后通道回到默认关闭——这是恢复产品边界（缺陷即错误行为），见开放问题 #2。
- 回归风险：中。测试：配置缺省 → cookie 通道不可用；`[cookie]` 双真 → 可用；仅 `enabled=true` 未同意 → 拒绝。

**F-08 WebDAV spool flush 泄漏（Windows）**
- 现状核实：`webdav.rs:595-621`，flush 用 `guard.as_ref()`（句柄留在 `Mutex<Option<File>>` 内）上传后 `remove_file`；句柄未关 → Windows 上删除/共享冲突、Drop 语义依赖不成文。无 `Drop` 实现、flush 无幂等护栏（dav-server 可能多次 flush → 重复上传）。
- 修复策略：
  - `WriteFileWithProvider` 增加 `flushed: bool` 字段（`DavFile` 方法均为 `&mut self`，plain bool 足够）。
  - flush：幂等护栏（已 flush 直接 `Ok`）→ 锁内 `flush()` → `guard.take()` **拿出所有权并在作用域结束关闭句柄** → 上传 spool 路径 → `remove_file`（无论上传成败都尽力清理；删除失败 `tracing::warn!` 记录路径后降级放行，不掩盖上传结果）。
  - 新增 `impl Drop`：`file.lock().take()` 关句柄 + 若 `!flushed` 删除 spool 残留（PUT 中途断连的自愈；flush 正常完成后 Drop 的 remove 为 no-op）。
- 调用方影响：无外部 API。回归风险：低。测试：flush 后 spool 文件不存在；上传失败后句柄已释放（Windows 上可重删）；二次 flush 不重复上传。

**F-09 WebDAV 写打开错误混淆**
- 现状核实：`webdav.rs:292-309`，`Err(_)` 把一切 resolve 错误当"新文件"。`resolve_path`（`:107-159`）与 `map_err`（`:180-185`）都能精确产出 `FsError::NotFound`。
- 修复策略：匹配收窄——`Err(FsError::NotFound) => 合成新条目`，`Err(e) => return Err(e)`。权限/网络/频控错误如实上抛（dav-server 映射为 5xx），静默新建误写路径关闭。
- 调用方影响：PUT 既有文件的错误行为更严格，属缺陷修复本身。回归风险：低。测试：对不存在文件 PUT 成功；对父目录不可达（mock provider 返回非 NotFound）PUT 得到错误而非新建。

**F-12 SCM 服务路径忽略 placeholder_mount；service.rs"死代码"**
- 现状核实（对审计的修正）：`service.rs` **并非整体死代码**——`install/start/stop/uninstall/run_service_scm_executor/StopSignal/should_exit` 均被 `main.rs:171-222/424-459` 调用。真正死的是 `service.rs:146` 的 `define_windows_service!(ffi_service_main, service_main)`：全仓无 `service_dispatcher::start`，`ffi_service_main` 无任何调用方（SCM 实际入口是 main.rs 的 `service run` clap 分支 → `run_service_scm_executor` 手动注册控制处理器）。真实缺陷：`main.rs:193-220` Run 分支从不检查 `cli.placeholder_mount`（而 `build_launch_args:296-298` 确实把 `--placeholder-mount` 烘焙进 binPath）→ 服务模式收到 flag 后静默跑同步模式；`service_main` 的手写参数解析也丢弃 `--placeholder-mount`。
- 修复策略：
  - `ServiceCommands::Run` 分支按 `cli.placeholder_mount` 分派：true → 新增 `run_service_placeholder_blocking`（SCM 版占位符挂载循环）；false → 维持现有 `run_service_engine_blocking`。
  - `run_placeholder_mount` 重构：把挂载主循环抽出，增加参数 `stop_check: &dyn Fn() -> bool`（前台传 `|| false`；SCM 传 `stop.should_stop() || service::should_exit()`），循环体内轮次间与分段 sleep 处检查；停止后走既有 `control.stop()` + `Arc::try_unwrap` + `unmount` 干净卸载路径。
  - service.rs：**保留**全部在用部分，**删除**死 FFI 入口（`define_windows_service!` + `service_main` 手写解析，共约 70 行）——消除与 clap 的双份解析；保留手动 `run_service_scm_executor` 模式（已在用、可测）。顺带修复：`build_launch_args` 补 `--interval`（`service_main` 曾解析它但 install 从不烘焙——潜伏 bug）。
- 调用方影响：Windows 服务用户行为与前台运行对齐（缺陷修复本身）；`--hold-secs/--once` 不参与服务路径（服务即常驻），无需烘焙。
- 回归风险：中（SCM 路径只能集成验证）。测试：`run_placeholder_mount` 的 stop_check 抽象为纯函数可单测；`build_launch_args` 快照测试。

**F-13 daemon 配置加载失败静默回退**
- 现状核实：`main.rs:162-165` `load().unwrap_or_else(warn + default)`。而 `cloudmount-core/config.rs:100-107` 的 `load_from` 语义已正确：文件不存在 → 默认值（首次运行正常路径）；文件存在但解析/读失败 → `Err`。错误被 daemon 侧吞掉。
- 修复策略：改为 fail-fast——`let file_cfg = cloudmount_core::config::load().context("配置文件加载失败（修正该文件，或删除后重启以恢复默认配置）")?;`。文件缺失不受影响；解析失败 → anyhow 错误链带路径与原始 toml 错误，进程非零退出。SCM 路径同样受益（main 在 service 分支前加载）。
- 调用方影响：坏配置从"静默降级"变"拒绝启动"，属缺陷修复本身。回归风险：极低。测试：现有 `malformed_file_errors` 压住 core 语义；daemon 侧以 `load_from` 注入路径单测（无需起服务）。

## 数据模型

本 intent 不引入持久化 schema 变更，除以下两处：

| 实体/结构 | 变更 | 键/字段 | 约束 |
|---|---|---|---|
| `config.toml`（cloudmount-core::Config） | **新增** `cookie` 子配置 | `[cookie] enabled: bool, consent_acknowledged: bool`（默认均 false） | `serde(default)` + `deny_unknown_fields`：旧文件可解析；缺段 = 双 false = 通道关闭；仅 `enabled=true` 无 consent 仍拒绝启用 |
| 令牌（cloudmount-auth::TokenSet，内存+SecretStore） | **无 schema 变更** | `access_token/refresh_token/expires_at` | 不变量：刷新全程持 `tokens` 锁；持久化顺序 = 先经守卫更新内存、锁外写 SecretStore（崩溃窗口内内存与磁盘至多差一次刷新，不产生互踩） |

进程内瞬态结构（不落盘）：

- `Executor.dir_creating: Mutex<HashMap<String /*prefix*/, Arc<Mutex<()>>>>`（F-07）——per-前缀创建锁，键为 `a/b/c` 形式的目录前缀；条目在对应 `dir_ids` 插入后移除。
- `RdcWatcher` 字段改为 `handle: Option<Arc<RawHandle>>`、`thread: Option<JoinHandle<()>>`（F-06）；`RawHandle.closed: AtomicBool` 保证 `CloseHandle` 恰一次。
- `WriteFileWithProvider.flushed: bool`（F-08）与 spool 临时文件（`%TEMP%/cm-dav-put-*`）生命周期：flush 或 Drop 时必被删除，残留仅允许在 remove_file 失败且已 warn 的场景。
- 索引：无新增（`dir_ids` 键仍为 rel 前缀字符串，`MountState.remote` 键仍为 rel 路径）。

## 与 PRD 的语义对齐表

PRD 并行编写中，本表以 **F-xx 编号为对齐锚**（对齐拍请核对 F-xx ↔ prd AC 一一对应）：

| 锚 | 修复动词 | 物理语义 | 说明 |
|---|---|---|---|
| F-01 | 分块流送 | FETCH_DATA 范围经 `open_range` 单次建流 + 1 MiB 块循环 `TRANSFER_DATA`；内存峰值 = 1 块（≈1 MiB），与文件大小无关；取消/失败以失败 NTSTATUS 收尾挂起读 | 原"整个范围 read_to_end 进 Vec"语义废除 |
| F-02 | 转发 | `ProviderRouter.change_probe == 活跃通道.change_probe`（Auth 失效随 `call()` 自动切换）；不支持探测的通道返回 `None` → 引擎回退全量扫描 | auto 通道恢复快路径，无行为新增 |
| F-03 | 拒绝 + 双检 | 静态文件服务的每个路径成分先验拒（`..`/`.`/`:`/`\`/空段/保留设备名），再 canonicalize 后强制 `starts_with(root)`；违规 → 403，**任何文件系统访问都发生在校验之后** | 原字符串 `contains("..")` 检查废除 |
| F-04 | 脱敏 | 日志中凭据字段只记 字段名+长度，值永不落日志（任何 trace 级别） | 三处 debug! 全部改为字段级脱敏 |
| F-05 | 回接开关 | Cookie 通道可用性 = 配置 `[cookie] enabled && consent_acknowledged` 双真；默认双 false = 关闭；同意的唯一持久化位置是 config.toml，由 CLI 登录流程写入 | daemon 组装处硬编码 `true/true` 废除 |
| F-06 | 真释放 | Drop = 置停止标志 → 幂等关闭目录句柄（唤醒阻塞读）→ join 线程；Drop 返回后线程与句柄均已终结 | 原 `drop(self.handle.clone())` 空操作废除 |
| F-07 | 锁内计划/锁外执行 | `dir_ids` 锁内只做查表与缺失链计划；网络 IO（list/create_dir）在锁外、per-前缀锁 + 双检之下执行；"不重复建目录"语义保持 | 持锁跨网络往返废除 |
| F-08 | 所有权收口 + 自愈 | flush 幂等；句柄经 `take()` 关闭后再上传再删 spool；Drop 兜底清理未 flush 的 spool；删除失败降级为 warn 不掩盖上传结果 | `as_ref()` 半泄漏与静默残留废除 |
| F-09 | 精确匹配 | 仅 `FsError::NotFound` 走"合成新文件"路径；其它打开错误原样上抛（5xx） | `Err(_)` 盲当新文件废除 |
| F-10 | 单飞 | 同一 `OpenApiAuth` 实例的并发刷新被 `tokens` 锁串行化：每时刻至多一个在途刷新请求；等待者共享刷新结果（快路径命中）；刷新失败则等待者依次串行重试 | "拿锁后、网络前丢锁"废除 |
| F-11 | 显式失败 | OSS PUT 非 2xx → `Err(Error::Remote{code,status 摘要})` 进既有重试队列；"上传成功"判定必须以 2xx + 既有 verify 命中为准 | 吞状态码废除 |
| F-12 | 接线 + 删死码 | SCM Run 分支按 `--placeholder-mount` 分派到停止感知的占位符挂载循环（与前台同一循环体 + stop_check 闭包）；service.rs 保留在用部分、删除无调用方的 FFI 入口 | 服务/前台行为一致；`binPath` 烘焙补 `--interval` |
| F-13 | fail-fast | config.toml 存在但解析失败 → 进程非零退出并指明文件路径；文件不存在 → 默认值（首次运行不受影响） | 静默回退默认值废除 |
| F-14 | 补网 | cookie.rs 错误路径获得 HTTP 级回归测试（本地 mock 服务器，无新依赖）；作为 F-04/F-11 的安全网先行提交（TDD） | 零生产代码行为变化（注入面仅 cfg(test)） |

## 关键决策与理由

| # | 决策 | 为什么 |
|---|---|---|
| 1 | **不引入任何新依赖**（含 dev-dependencies）：F-14 用手写 `TcpListener` HTTP mock，F-04 用自实现 `MakeWriter` 捕获日志 | 修复型 intent 最小 diff 原则；workspace 无测试服务器依赖先例；手写 responder 对 ureq 的 HTTP/1.1 用法（固定 body、无 chunked/TLS）足够；避免为测试引入需要审计的第三方供应链 |
| 2 | **F-01 块大小 = 1 MiB**，单块复用缓冲 | 过滤器单次请求通常 ≤1–8 MiB，1 MiB 与之同量级；4 GiB 文件 = 4096 次 `TRANSFER_DATA`，调用开销可忽略；峰值内存 ≈1 MiB + reader 内部缓冲，完全不威胁空闲 RSS 5–15MB 纪律（对比：256 KiB 会把调用数翻 4 倍、4 MiB 削弱小内存优势）；`TRANSFER_DATA` 无最小/最大块约束，官方样例即 ~1MB |
| 3 | **service.rs 保留在用部分、仅删除死 FFI 入口**（`define_windows_service!` + `service_main`），不改走 `service_dispatcher` 标准模式 | 源码核实：install/start/stop/uninstall/executor 均在用，整体删除会破坏 `service install` 等已发布子命令；死入口删除消除与 clap 的双份参数解析（正是 F-12 的病灶之一）；`service_control_handler::register` 手动模式已在生产路径验证，切换标准模式属无收益重构 |
| 4 | **F-05 同意标志持久化在 config.toml**（而非独立 marker 文件），由 CLI 登录流程写入 | 复用既有配置加载/原子写/优先级体系（clap > env > 文件 > 内置），零新机制；`deny_unknown_fields` 保证拼写错误显式失败；登录交互（yes 确认）是同意的唯一产生点，daemon 只消费 |
| 5 | **F-13 采取 fail-fast**（而非 告警+继续） | "配置静默降级"正是缺陷本身；continue-with-banner 仍会以错误配置跑同步/删除传播（propagate_* 是破坏性开关），风险不对称；文件缺失回默认值的首次运行路径不受影响 |
| 6 | **F-03 采用 成分级校验 + canonicalize 双检** 两层，而非只取其一 | 成分级拦截零 IO 且拒绝语义明确（403 原因可日志化）；canonicalize 兜底覆盖符号链接/8.3 短名等校验规则遗漏面；两者成本均可忽略（静态资产服务低频路径） |
| 7 | **F-10 用既有的 `tokens` 锁跨刷新**（而非新引入 OnceCell/独立刷新锁） | 最小改动达成 single-flight；独立刷新锁会引入"锁间窗口"需要额外双检状态，复杂度更高且收益为零；副作用（刷新期间 logout 阻塞 ≤30s）语义正确且可接受（见开放问题 #6） |
| 8 | **F-01 只改 HydrateFn 契约、不改 CF_HYDRATION_POLICY_FULL** | OOM 根因是全量缓冲不是水合策略；改策略（如 PARTIAL）会改变"任意读触发完整水合"的产品可见行为，超出缺陷修复范围 |
| 9 | **F-07 用 per-前缀锁保住"不重复建远端目录"**，而非简单把网络挪出锁后听任重复创建 | 前者是既有安全性设计（exec.rs:9 文档化），不能为性能回退正确性；per-前缀锁只串行"同一目录"的创建，不同目录并行，锁序固定无死锁 |
| 10 | **F-11 错误类型用 `Error::Remote`**，错误体读取 `take(2048)` 限长 | 沿既有错误分类进重试队列；限长防异常大响应占内存（低内存纪律） |
| 11 | **TDD 顺序**：每个 F-xx 先失败测试后实现；F-14 的测试基建（mock 服务器 + 测试构造器）最先落地 | intent 约束；F-04/F-11 明确依赖 F-14 安全网 |

## 安全与权限

本 intent 本身是安全修复。修复后必须成立的安全不变量：

1. **路径边界不变量**：任何由外部输入（HTTP 请求路径、控制通道参数）推导出的文件系统访问，必须满足"校验先于访问"，且最终访问路径 canonicalize 后严格位于服务根内。覆盖点：webdav `serve_static`（F-03，成分级 + canonicalize 双检）；既有 `dehydrate_via_mount`（`main.rs:666-681` 的 `strip_prefix(root_canon)`）维持不变。Windows 特化：拒绝盘符段（`X:`）、UNC（`\\`）、备用数据流（`:`）、保留设备名。
2. **凭据不出现在日志的不变量**：userkey、OSS STS（AccessKeyId/AccessKeySecret/SecurityToken）、Cookie 值（UID/CID/SEID/KID）、access_token/refresh_token 在任何 trace 级别都不得以明文出现在日志产物中；唯一合法格式是字段名 + 长度（`redact_fields` 纯函数，F-04）；该不变量由捕获型日志回归测试固化。
3. **上传成功判定不变量**：远端写操作只有在获得显式服务端确认（OSS PUT 2xx，且既有 `verify_upload` 命中）后才允许记为成功；任何非 2xx、解析失败、传输错误都必须产生 `Err` 并进入既有重试队列（F-11）。禁止任何"静默当成功"路径。
4. **Cookie 通道边界不变量**：Cookie 通道保持"默认关闭 + 显式开启 + 知情同意"双标志把关（F-05）；同意仅由 CLI 登录交互产生并持久化；daemon 是消费者、永不自产同意。
5. **权限面**：本 intent 不涉及 IAM/网络策略/密钥管理变更；无新增监听端口（控制通道维持本地回环 + 挂载根前缀校验）；SCM 服务仍以 LocalSystem 运行既有能力，占位符挂载接线（F-12）不扩大攻击面（同一挂载 API、同一 stop 语义）。密钥存储维持 cloudmount-auth 独立容器（OpenApiToken / CookieJar 物理隔离）现状。

## 开放问题（需人类裁决）

- [ ] **F-13 失败语义确认**：fail-fast 意味着"配置文件坏 → 服务/前台都拒绝启动"。备选是"显式横幅告警 + 拒绝破坏性开关下继续"。默认按 fail-fast 设计，Gate#1 可改。
- [ ] **F-05 升级行为**：已登录 Cookie 通道但配置文件无 `[cookie]` 段的用户，升级后通道回到关闭状态（需重新执行一次 `cloudmount login --with-cookie-channel` 或手动补配置）。这是恢复产品边界的预期结果，但属于用户可感知的行为变化，请确认接受；同意标志落在 config.toml（方案 A）还是独立 marker 文件（方案 B）也请定夺。
- [ ] **F-12 service.rs 处置**：推荐"保留在用部分 + 删除死 FFI 入口"（关键决策 #3）；备选是改走 `windows-service` 标准 `service_dispatcher` 模式（更正统但属无收益重构，且与 clap 解析需重新融合）。
- [ ] **F-01 块大小**：1 MiB 为缺省值（关键决策 #2）；若对超低速链路上的 `TRANSFER_DATA` 调用次数有顾虑，可调成 2/4 MiB（仍是常数内存）。
- [ ] **F-14 覆盖边界**：initupload 的**解密 happy-path 回环**（测试侧实现 P-224 ECDH 对端 + LZ4）不在本期错误路径范围内；若要求全链路回环需给 `ec115` 增加测试辅助函数，请确认是否纳入。
- [ ] **F-10 锁副作用**：刷新期间（≤30s）`logout/is_logged_in` 将阻塞等待同一把锁。语义正确但属可感知变化；若不可接受，需引入独立刷新锁 + 双检（复杂度更高），默认不采用。
