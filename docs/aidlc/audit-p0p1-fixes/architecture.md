# Architecture: audit-p0p1-fixes

> 产出者：architect　日期：2026-10-14（**修订轮 v2**，Gate#1 打回后重写）　来源：intent.md + audit-findings.md + 源码核实（逐 F-xx 对照 D:\OneDrive\Devops\cloudmount 当前 HEAD）
> 修订依据：`reviews/gate1-ai.md`（互审点名）+ `decisions.md`（人类 Gate#1 裁决与编排者代行裁决，逐字落实）。修订面：MA1/MA2/MA3/MA4/MA6/MA8/MA9、m1/m2/m3/m9/m10/m11/m12/m13/m18 及语义对齐表/关键决策表相应更新；F-01…F-14 逐项结构与编号保持可追溯。

### Gate#1 裁决落实摘要（修订对照）

| 裁决/互审项 | 落点（本文章节） |
|---|---|
| desktop 壳纳入本 intent（人类裁决 #2） | 组件边界图、apps/desktop 设计节、语义对齐表 desktop 行、决策 #14 |
| F-14 ECDH 回环纳入本期（人类裁决 #3） | F-14 节重写、决策 #1/#12 |
| F-04 扩围：cli main.rs:233 回显 + get_info 日志（代行裁决 #1/#2） | F-04 节、决策 #15 |
| F-14 注入面统一：tests/ 集成 + with_base_urls 纯加法（完整基址含 scheme） | F-14 节 |
| F-08 spool 语义统一（warn+接受残留；flushed 上传成功后置位） | F-08 节 |
| 数值口径（刷新 10 线程、watcher 50-100 轮、F-01 reader 级断言） | F-10/F-06/F-01 节 |
| MA1（refresh() 单飞补全）、MA4（join 有界）、MA6（两处 seam）、MA9（--interval 验收）、m2/m3/m9/m10/m11/m12 | 对应 F-xx 节 |
| MA8（CI features job） | "CI 与门控设计"节、决策 #16 |

## 逻辑架构

### 组件与边界（不变量：本 intent 不新增 crate、不改变既有依赖方向）

```text
cloudmount-core        Error/Provider trait/config.toml schema
   ▲            ▲            ▲              ▲
   │            │            │              │
cloudmount-vfs  cloudmount-sync  cloudmount-auth  cloudmount-driver115
 (cfapi/cfapi_mount) (rdc/exec)   (open/cookie/store) (router/cookie 驱动)
   ▲            ▲
   └── cloudmount-daemon（main / webdav / service）──► apps/desktop
                                                    （Gate#1 裁决纳入：F-05 desktop 落点；
                                                      独立 Cargo.toml、workspace exclude，
                                                      以 path 依赖链接全部既有 crates）
```

各缺陷的归属边界（修复全部在既有 crate 内完成）：

| crate / app | 触及的缺陷 | 触及模块 |
|---|---|---|
| cloudmount-vfs | F-01 | `backends/cfapi.rs`（HydrateFn 契约 + ChunkSink + `pump_chunks` + FETCH_DATA 回调）、`backends/cfapi_mount.rs`（水合闭包） |
| cloudmount-sync | F-06、F-07 | `rdc.rs`（Drop/有界收割/活跃计数 hook）、`exec.rs` |
| cloudmount-auth | F-10（+F-14 测试注入面） | `open.rs`（双入口持锁刷新助手 + `with_passport_base` 测试注入，沿用 cookie.rs 既有模式） |
| cloudmount-driver115 | F-02、F-04、F-11、F-14 | `router.rs`、`cookie.rs`、`ec115.rs`（`#[doc(hidden)]` test_peer 模块） |
| cloudmount-daemon | F-03、F-05、F-08、F-09、F-12、F-13 | `webdav.rs`、`main.rs`、`service.rs` |
| cloudmount-core | F-05（配置 schema） | `config.rs` 新增 `[cookie]` 子配置 |
| cloudmount-cli | F-05（配套：登录时持久化同意标志）、F-04（扩围：cookie 回显脱敏） | `main.rs::cmd_login_cookie`、`main.rs::cmd_inject_cookie` |
| **apps/desktop** | **F-05 desktop 落点（Gate#1 裁决新增）** | `src/main.rs`（登录页同意交互 + `login_auth` + 登录成功写回 `[cookie]`） |

### 逐项修复设计（F-01…F-14，按 crate 分组）

#### cloudmount-vfs

**F-01（P0）CfAPI 水合全量读入内存**
- 现状核实：`cfapi_mount.rs:113-139` 闭包 `Vec::with_capacity(end-start+1)` + `read_to_end`；`cfapi.rs:586` `HydrateFn = Fn(&str,u64,u64)->Result<Vec<u8>>`；`cfapi.rs:713-773` FETCH_DATA 一次性 `transfer_data` 全量缓冲；`:315` 策略为 `CF_HYDRATION_POLICY_FULL`。
- 修复策略：把 `HydrateFn` 从"返回整块 Vec"改为"分块推送"契约：
  ```rust
  pub trait ChunkSink { fn transfer(&mut self, offset: u64, data: &[u8]) -> Result<()>; }
  pub type HydrateFn = Box<dyn Fn(&str, u64, u64, &mut dyn ChunkSink) -> Result<()> + Send + Sync>;
  ```
  - `cfapi.rs::fetch_data_callback` 构造 sink：每个块先查 `ctx.cancelled`（取消 → `Err`，停止读取并丢弃 reader），再 `transfer_data(info, offset, Some(chunk), 0)`；水合成功路径的水合日志（mtime 记录）移到**最后一个块成功之后**整体记录（语义不变：仍是"水合完成后"记一次）。
  - `cfapi_mount.rs` 水合闭包：寻址远端条目与范围钳制逻辑不变；`provider.open_range(RangeSpec{start,end})` **仅调用一次**，循环读入**单个复用的 1 MiB 缓冲**，逐块 `sink.transfer(start+consumed, &buf[..n])`。中间任一读错误 → 返回 `Err` → 回调以 `transfer_data(None, STATUS_UNSUCCESSFUL)` 结束挂起读（沿用现有错误收尾路径）。
  - **分块循环提为 crate 内可测函数（MA6 seam）**：
    ```rust
    /// cfapi.rs 内；T-01/T-02 单元腿的注入点（短读/错误 Reader 直接压循环体）
    pub(crate) fn pump_chunks(
        reader: &mut dyn std::io::Read,
        sink: &mut dyn ChunkSink,
        offset: u64,
        chunk_size: usize,
    ) -> Result<u64>   // 返回泵送总字节数
    ```
    水合闭包调用它；单元测试用记录型桩 Reader（记录每次 `read()` 的请求长度）与桩 ChunkSink（记录 (offset,len) 序列）断言：读取请求恒 ≤ 块上限、transfer 序列连续覆盖范围、短读续读、错误传播无部分残留。
  - 块大小 `const HYDRATION_CHUNK: usize = 1 MiB`，取值理由见关键决策 #2。
  - `CF_HYDRATION_POLICY_FULL` **不变**（改策略改变产品可见行为，且分块后 OOM 根因已消除）。
- **断言口径（m1，编排者裁决）**：F-01 的可区分断言锚定在 **reader 级读取序列**——桩 Reader（`open_range` 返回者）记录每次 `read()` 的请求长度：修复前 `read_to_end` 呈几何增长直至总范围等大；修复后恒 ≤ 1 MiB 定长。**勿以 `open_range` 的 RangeSpec 断言**（修复前后都只调用一次 open_range，无法区分红绿）。
- 背压/取消语义：`CfExecute(TRANSFER_DATA)` 同步返回 → 天然背压（永远最多一个块在途）；取消经 `CANCEL_FETCH_DATA` 已有的 `cancelled: Mutex<HashSet<TransferKey>>`，sink 在每块发送前检查，取消后立即 drop reader（关闭下游 HTTP 连接）。
- 调用方影响：`HydrateFn` 是 crate 内契约，全部调用点共 3 处——`cfapi_mount.rs:113`（本修复主体）、`tests/cfapi_hydration.rs:188/243`（机械适配新签名）。`MountBackend::hydrate`（`cfapi_mount.rs:176-181`，范围由调用方给定、非 FETCH_DATA 路径）保持不变。
- 回归风险：中。TRANSFER_DATA 分块是文档化模式（CloudMirror 样例同款），风险点在"部分传输后取消/失败"的过滤器行为——由 cfapi_hydration 集成测试补"分块读取大范围""中途取消""中途失败"三个用例压住。

#### cloudmount-sync

**F-06 RdcWatcher Drop 空操作**
- 现状核实：`rdc.rs:141-147` `drop(self.handle.clone())` 克隆后立刻丢副本，Arc 引用计数净变化为零；线程持有自己的 `Arc<RawHandle>` 克隆，故 watcher 侧永远关不掉句柄；`JoinHandle` 在 `new()` 中被 `let _thread =` 丢弃（`:76`）。`shutdown` 标志已存在但只在读失败路径被检查。
- **前提澄清（MA4）**：`CloseHandle` 唤醒阻塞中的 `ReadDirectoryChangesW` 是**尽力而为**而非文档化保证——Win32 官方文档未把关闭句柄列为在途同步 IO 的受支持取消手段（对在途同步读关句柄属未定义行为；本项目监听**本地**目录、实践上有效、`event_loop` 的 Err 分支已按此退出，但网络重定向器等场景不保证）。因此 Drop 的线程收割**必须**有界，不能无限期 join。
- 修复策略（所有权重构 + 有界收割）：
  - `RawHandle` 增加显式 `close(&self)`：内部 `closed: AtomicBool` 守卫，`swap(true)` 成功才 `CloseHandle`（幂等，防 OS 句柄值复用被误关）；`RawHandle::drop` 复用同一守卫。
  - `RdcWatcher` 字段改为 `handle: Option<Arc<RawHandle>>`、`thread: Option<JoinHandle<()>>`；新增 `exited: Arc<AtomicBool>`（`event_loop` 全部退出路径统一置位——线程真正退出的信号）。
  - `Drop`：`shutdown.store(true)` → `handle.take()` 并 `close()`（尽力唤醒阻塞读）→ **有界收割**：以 50ms 间隔轮询 `exited`，上限 **5s**；观测到退出 → `join()` 收割；**超时 → `tracing::warn!`（含 root 路径）→ 放弃 join（detach）**——线程与句柄泄漏回到**可观测**形态（warn 日志 + 活跃计数不回落），Drop 绝不挂起。`tx.send` 在无界 mpsc 上永不阻塞，不构成额外挂起源。
  - 选择"join 超时"而非 `CancelSynchronousIo` 的理由见关键决策 #13。
  - **生产 seam（MA6）**：rdc.rs 模块级 `static ACTIVE_WATCHERS: AtomicUsize`（`new()` 成功后 +1，`event_loop` 退出时 -1；**cfg 无关**——不设 `cfg(test)`，生产构建常驻，代价为每次创建/退出各一次原子加减，可忽略）+ `#[doc(hidden)] pub fn active_watcher_count() -> usize`。T-09 以该计数轮询回落基线（上限 5s）。
- 调用方影响：无外部 API 变化（`RdcWatcher::new/poll/snapshot/root` 不变）。
- 回归风险：低-中。测试：**50-100 轮**（编排者数值口径，断言下限 50 轮）"创建+drop"循环后 `active_watcher_count()` 回落基线（泄漏信号放大为 +N，单轮时序抖动不误报）；Windows 上 drop 后可 rename/remove 被监视目录（句柄确已释放）；事件洪峰后 drop 有界完成、不 panic、不悬挂（T-10）。

**F-07 ensure_remote_dir 持锁做网络 IO**
- 现状核实：`exec.rs:454-499`，`dir_ids: Mutex<HashMap<String, RemoteId>>`（`exec.rs:46`）在 `provider.list`（逐组件查重）与 `provider.create_dir` 整个网络往返期间被持有；所有并发上传/建目录动作都在这把锁后串行。
- 修复策略：两阶段 + per-path 创建锁：
  1. **锁内只做计划**：拿 `dir_ids` 锁 → 从根沿 `rel` 组件走 map，得出"已解析到的当前 ID"与缺失前缀链 `Vec<(prefix, comp)>` → 立即放锁。
  2. **锁外做网络**：对每个缺失前缀——取 `dir_creating: Mutex<HashMap<String, Arc<Mutex<()>>>>` 中的 per-prefix 锁（取锁本身不持 map 锁过夜，只克隆 Arc）→ **双检** `dir_ids`（其它线程可能已完成）→ 仍缺失才做 `provider.list` 查重 / `provider.create_dir` → 结果写回 `dir_ids`。
  - 锁序固定为"父前缀先于子前缀"（与遍历顺序一致），无环，无死锁。`dir_creating` 中已完成的条目可在插入 `dir_ids` 时顺手移除，防 map 无界增长。
- 调用方影响：`ensure_remote_dir` 私有方法签名不变；"并发上传不重复建远端目录"的确定性由 per-prefix 锁 + 双检保持（`exec.rs:9` 声明的安全性设计不回退）。
- 回归风险：低-中。测试：并发 N 线程对同一深层目录 `ensure_remote_dir`，断言远端 `create_dir` 恰好一次、全部返回同一 ID；弱网（慢 mock provider，list 与 create_dir 均注入延迟）期间其它动作不被阻塞的时序断言（锁时序记录为主判据）。

#### cloudmount-auth

**F-10 OpenApiAuth 非单例刷新（双入口补全，MA1）**
- 现状核实：`open.rs:163-193` `access_token()` 快路径拿锁检查 → 释放 → `do_refresh`（网络）→ `persist`（重新拿锁）——注释自称"持锁执行天然单飞"但 `drop(guard)` 在 `do_refresh` 之前，实际**未**持锁跨网络。**第二条入口**：`Authenticator::refresh()`（`open.rs:366-374`）同样是"拿 rt → 丢锁 → 网络 → persist"，且被 `client.rs:244`（`with_token_retry`，token 失效并发重试路径）调用——并发双刷互踩正是 F-10 要消灭的 40140117 形态。互审 MA1 指出只改 `access_token()` 病灶残留。
- 修复策略：**两条入口共用同一持锁刷新助手**（锁即 single-flight），伪码：
  ```rust
  /// 持锁刷新助手——single-flight 的唯一实现（access_token 与 refresh 共用）。
  /// skip_if：持锁后的双检谓词（true = 无需刷新，直接共享他人结果）。
  fn refresh_locked(&self, skip_if: impl FnOnce(&TokenSet) -> bool) -> Result<()> {
      let mut guard = self.tokens.lock().expect("lock");
      let current = guard.as_ref()
          .ok_or_else(|| Error::Auth("未登录：请先执行 `cloudmount login`".into()))?;
      if skip_if(current) { return Ok(()); }        // 双检：他人已刷新，共享结果
      let rt = current.refresh_token.clone();
      let ts = self.do_refresh(&rt)?;               // 持锁跨网络——single-flight 核心
      let blob = serde_json::to_vec(&ts)?;
      *guard = Some(ts);                            // 经守卫写回（绝不调 persist()——重入死锁）
      drop(guard);
      self.store.put(SecretKind::OpenApiToken, &blob)?;   // 持久化移到锁外
      Ok(())
  }

  // 入口一：access_token()（时间性过期）——快路径同现状；慢路径：
  self.refresh_locked(|ts| !ts.needs_refresh(Self::now()))?;

  // 入口二：Authenticator::refresh()（服务端判失效强刷）——token 身份双检：
  fn refresh(&self) -> Result<()> {
      let observed = self.tokens.lock().expect("lock")
          .as_ref().map(|t| t.access_token.clone());
      match observed {
          Some(old) => self.refresh_locked(|ts| ts.access_token != old),
          None => Err(Error::Auth("未登录".into())),
      }
  }
  ```
  - **client.rs:244 调用点同步微调**：`self.auth.refresh()?` → `self.auth.refresh_stale(&token)?`（新增 inherent 方法，`stale` = 本次失败的 access_token，`client.rs:224` 已在作用域；`ApiClient.auth` 是具体类型 `Arc<OpenApiAuth>`，inherent 方法可直接调用）。显式传失效 token 关闭一个窗口：若 B 线程在"attempt 失败 → 进入 refresh()"之间 A 已完成刷新，B 观察到的新 token 会触发一次**多余的串行刷新**（背靠背刷新正是 40140117 触发形态）；显式 stale 使 B 的双检命中"A 已换新"而跳过。
  - 等待者持锁阻塞至刷新完成 → 恢复后走快路径命中新 token。失败路径：`?` 提前释放锁，等待者依次（串行，由锁保证）重试——40140117 只由**并发**成功刷新触发，串行重试是合法形态。
- 配套（测试使能）：`OpenApiAuth` 沿用 `cloudmount-auth::cookie::CookieAuth::with_base_url` 既有模式，增加 `#[doc(hidden)] pub fn with_passport_base(...)` 覆盖（`do_refresh` 的 `PASSPORT_BASE` 改经 `self.passport_base()` 取值，默认 = 生产常量；纯加法，集成测试位于 driver115/tests/ 需跨 crate 可见）。`logout/is_logged_in` 维持现状（`logout` 短锁自洽）。
- **并发测试要求（MA1，数值口径 = 10 线程）**：
  1. `access_token()` 入口：10 线程在过期点会合后并发调用 → mock 刷新端点**恰收到 1 次**、全体拿到同一新 token（T-17）。
  2. `refresh()`/`refresh_stale()` 入口（新增）：10 线程并发调用（或经 `with_token_retry` 注入 token 失效信封触发）→ 刷新请求**恰 1 次**（身份双检去重：第一线程换新后其余观察到 token 已变 → 跳过）。
  3. 失败注入（T-18 扩展至双入口）：刷新端点 401/500/坏 JSON → 全体等待者收到一致 Err、锁已释放、无死锁；随后单线程重试成功。
- 调用方影响：`access_token()` 签名不变；`Authenticator` trait 契约不变（`refresh()` 语义 = "替换当前 token"，并发重复请求被身份双检合并）；副作用：刷新期间（≤30s 全局超时）`logout/is_logged_in` 会阻塞等待——语义正确（登出发生在刷新之后，最终状态一致）。
- 回归风险：低。既有 token 恢复/登出测试压住；新增上述并发单飞测试。

#### cloudmount-driver115

**F-02 ProviderRouter 未转发 change_probe**
- 现状核实：`router.rs:162-212` `impl Provider for ProviderRouter` 确无 `change_probe`，走 trait 默认 `Ok(None)`（`cloudmount-core/provider.rs:93`）；`cloudmount-sync/remote.rs:105` 按 `None` 回退全量扫描。
- 修复策略：在 impl 中加一行转发 `fn change_probe(&self, dir) -> Result<Option<ChangeProbe>> { self.call(|p| p.change_probe(dir)) }`——免费获得既有 `call()` 的 Auth 失效自动切换语义。Cookie 通道未实现 `change_probe`（默认 `None`）时行为正确：引擎对该通道回退全量扫描。
- **测试落点（m10，决策 #1 冲突消解）**：在 `router.rs` 既有 `#[cfg(test)] mod tests` 内**扩展现成 Stub**（Stub 增加 `change_probe` 返回值配置 + probe/list 调用计数），断言：probe 值透传、Auth 失效时随通道切换、快路径 `list` 计数不增长。**零 dev-dependency**——不引入 `cloudmount-sync::testutil::MockRemote` 作 driver115 dev-dep（那会连带 rusqlite 构建且违反决策 #1）；`ProviderRouter::new` 本就收 `Box<dyn Provider>`（`router.rs:67`），但 Stub 扩展已足够覆盖。
- 调用方影响：无。回归风险：极低。

**F-04 凭据泄漏（三处主修复 + 两处扩围，共五处）**
- 现状核实（五处施工面）：
  1. `cookie.rs:401`（uploadinfo 原始 JSON，含 userkey）；
  2. `cookie.rs:492`（initupload 表单 `?form`，含 userkey/sig/token）；
  3. `cookie.rs:560`（gettoken 原始响应，含 STS AccessKeyId/Secret/SecurityToken）；
  4. **扩围（m13，编排者裁决升格）**：`cloudmount-cli/src/main.rs:233` `anyhow::bail!("Cookie 缺少必需字段（UID/CID/SEID）；收到: {cookie_str}")` 把原始 cookie（UID/CID/SEID/KID 明文）回显进错误输出——安全不变量 #2 的已知反例；
  5. **扩围（m18）**：`cookie.rs:796`（get_info 原始 JSON debug 日志）——互审记为 "webdav.rs:796" 属文件归属笔误，落点经源码核实为 driver115/cookie.rs:796；非凭据（文件元数据）但同类原样打印，一并收口。
- 修复策略：
  - cookie.rs 四处（1/2/3/5）：**字段级脱敏**——提取纯函数 `redact_fields(&[(k,v)]) -> String`（输出 `k:len` 列表）复用；对 uploadinfo/gettoken/get_info 的顶层 JSON 只记 `state/errno/status_code/size_limit` 等非敏感标量字段名与长度，一律不记值。
  - cli main.rs:233（4）：错误信息只记**缺失字段名 + 实际收到的字段名集合**（不含值）：如 `Cookie 缺少必需字段（UID/CID/SEID）；实际收到字段: UID,KID`。
  - **捕获机制（m2 修正）**：回归测试内**手写 `tracing::Subscriber` 实现**（约 40 行，仅用 `tracing` 本体——driver115 正式依赖；**不用 MakeWriter/tracing-subscriber**，后者属 tracing-subscriber crate 而 driver115 零 dev-deps）：`struct CapturingSubscriber { events: Arc<Mutex<Vec<String>>> }`，实现 `enabled()`（全级别放行）与 `on_event`（经 `event.metadata()` 取 level/target + `field::Visit` 收集字段值），以 `tracing::subscriber::with_default(...)` 捕获；断言金丝雀敏感值（userkey/STS secret/security_token/cookie 值）缺席、字段名/长度允许出现。捕获级别统一 **trace 全开**（AC4/T-07 口径，编排者裁决 #4）。
- 调用方影响：无（日志/错误信息面）。回归风险：极低。
- 附注：`cookie.rs:525` 解密后的 initupload 响应日志不含凭据（bucket/object/callback），保留；施工时若顺手可降为 status/pickcode 字段。

**F-11 oss_put 吞 HTTP 状态**
- 现状核实：`cookie.rs:619-635`，agent 以 `http_status_as_error(false)` 构建，PUT 后 `read_to_string` 即 `Ok(())`——非 2xx 被当成功。
- 修复策略：`send` 后取 `resp.status()`；非 2xx → 读体（`io::Read::take(2048)` 限长；体是 OSS XML 错误，不含凭据）→ `Err(Error::Http { status, body: "OSS 直传失败: {状态码} {响应体摘要}" })`。
- **重试语义（m9，三选一取一）**：非 2xx 一律用 `Error::Http{status}`（**修订**：原稿 `Error::Remote` 废弃）。理由：
  1. `is_retryable`（`error.rs:69-75`）对 `Http` 的既有分类（`status >= 500` 可重试、4xx 不可）恰好给出正确分叉——5xx 瞬时错误进轮内重试，**403 类永久错误快速失败、不无限重试**（互审 m9 硬要求）；
  2. 与 crate 内既有先例一致（`client.rs::read_body_text` :259-273 对 HTTP 层非 2xx 已用 `Error::Http{status, body}`）；
  3. 扩展 `is_retryable` 含 `Remote` 被否——`Remote` 覆盖全部业务错误码（大量永久性 auth/参数错误），一刀切可重试会造成永久错误无限重试；
  4. 仅接受"下轮重计划"被否——丢失 5xx 的轮内重试机会，弱于方案一。
  - 分类边界声明：HTTP 传输层状态码 → `Error::Http`；业务信封码（如 uploadinfo `state=false`，`cookie.rs:405` 现状）→ 维持 `Error::Remote`/`Error::Auth` 不动。
- 调用方影响：`upload()` 链路失败行为从"假成功"变"真失败+重试"，这是缺陷修复本身，属 intent 声明的允许行为变化。上传成功判定不变量见安全一节。
- 回归风险：中（影响真实上传链路的成功/失败判定）——由 F-14 的 HTTP 级测试（含 ECDH 回环直达 oss_put）压住。

**F-14 Cookie 通道零 HTTP 级测试（重写：注入面统一 + ECDH 回环纳入）**
- 现状核实：`cookie.rs` 全文件 1127 行无 HTTP 级测试；`webapi_base/proapi_base` 已可注入（`with_base_urls`，`:220`，全仓零调用方）；上传三端点硬编码（`:40` UPLOAD_INIT_URL、`:42` UPLOAD_TOKEN_URL、`:44` OSS_ENDPOINT + `:605` `https://` 字面量）；`oss_put` 为私有方法（`:542`），经公开 `upload()` 到达它需先构造 ECDH 加密的 initupload 成功响应；`ec115.rs` `EcdhCipher` 以 115 固定公钥 `REMOTE_PUB_KEY`（`:25`）做 ECDH——**无人持有该公钥对应的私钥**；`driver115` 无任何 dev-dependencies；`RateLimiter::new(0.5, 1.0)`（`:211`）会拖慢测试。
- 修复策略（按 Gate#1 裁决与编排者 canonical 裁决统一）：
  - **测试位置**：`driver115/tests/` 集成层——扩展 `mock_api.rs` 的 `spawn_mock`/`Recorded` 先例（手写最小 HTTP/1.1 responder：`std::net::TcpListener` 绑 `127.0.0.1:0`，按 method+path 路由返回预置状态码与 body）。`CookieAuth` 沿用既有 `with_base_url`（auth/cookie.rs:158，覆盖 qrcode/passport 域）+ `MemoryStore` 预置 CookieJar；`Cookie115Provider::try_new(policy, auth)` 后链式注入基址。
  - **生产侧纯加法 builder（不改既有调用方；`with_base_urls` 签名不动，新增链式方法）**：
    - `pub fn with_upload_base(base)`：uplb **完整基址含 scheme**（默认 `https://uplb.115.com`）；`:40/:42` 两常量改为基址 + 路径后缀拼接（`{base}/4.0/initupload.php?k_ec=…`、`{base}/3.0/gettoken.php`），默认渲染与现状逐字节一致。
    - `pub fn with_oss_base(template)`：OSS endpoint 存**完整 URL 模板（含 scheme 与 `{bucket}`/`{object}` 占位）**，默认 `https://{bucket}.cn-shenzhen.oss.aliyuncs.com/{object}`——与 `:605` 现拼接结果逐字节一致；构造经纯函数 `render_oss_url(template, bucket, object)`（一次性分段渲染，避免链式 replace 嵌套占位边角；单元测试锁"默认模板渲染 == 现状 URL"）。**`:605` 的 `https://` 字面量随之消除**（scheme 由存储值携带）。本地 mock 无 DNS 子域能力（`{bucket}.127.0.0.1` 不可解析），注入 `http://127.0.0.1:{port}/{bucket}/{object}` 以路径段承载 bucket——这是模板形态（而非"host 前插子域"）的理由：单一渲染规则、零分支、生产字节等价、mock 可达。
    - `pub fn with_rate_limit(rate, capacity)`：镜像 `ApiClient::with_rate_limit` 既有先例（`client.rs:64-67`），测试用快速限速。
    - `#[doc(hidden)] pub fn with_ecdh_remote_pub(remote: [u8;56])`：ECDH 远端公钥替换（见下）。
  - **ECDH 加密对端（Gate#1 裁决纳入本期）**：形态选择 = **ec115.rs 内 `#[doc(hidden)] pub mod test_peer` 测试辅助模块**（二选一理由见关键决策 #12）。回环硬前提：无人持有 115 固定公钥的私钥 → 测试必须以**测试密钥对替换远端公钥**，该替换 seam 无论对端实现放哪都必须落生产 crate——故 test_peer 模块同时提供替换构造与对端镜像：
    ```rust
    #[doc(hidden)]
    pub mod test_peer {
        pub struct TestEcdhPeer { /* 测试服务端密钥对 */ }
        impl TestEcdhPeer {
            /// 生成服务端密钥对；返回 (peer, remote_pub56)——remote_pub56 注入
            /// Cookie115Provider::with_ecdh_remote_pub
            pub fn generate() -> (Self, [u8; 56]);
            /// 从 k_ec token 解出客户端公钥（encode_token 的逆变换）→ ECDH 共享密钥
            pub fn session(&self, k_ec: &str) -> Result<PeerSession>;
        }
        pub struct PeerSession { /* 与客户端一致的 key/iv */ }
        impl PeerSession {
            /// 解密 initupload 请求体（CBC + PKCS7；请求方向无 LZ4 帧）→ 表单明文（断言用）
            pub fn decrypt_request(&self, body: &[u8]) -> Vec<u8>;
            /// 加密响应：LZ4 压缩 + [长度(2 LE)] 帧 + CBC → 客户端 decrypt 可解
            pub fn encrypt_response(&self, plain: &[u8]) -> Result<Vec<u8>>;
        }
    }
    ```
    `cookie.rs::init_upload`（`:443` `EcdhCipher::new()`）改为经 `self.ecdh_cipher()`：`ecdh_remote_pub` 字段有值时用 test_peer 的替换构造，否则 `EcdhCipher::new()`（生产路径不变）。peer 的加解密复用 ec115.rs 既有 CBC/PKCS7/LZ4 代码（同文件可见私有字段），协议怪癖（key/iv 重叠 4 字节、[len][LZ4] 帧、k_ec 编码）单点维护。
  - **覆盖矩阵**：uploadinfo 成功 / `state=false`；gettoken 成功 / `status_code != 200`；initupload 传输错误 / 响应不可解密（错误路径）+ **解密成功回环**（status=1 未命中 → 走 gettoken + OSS 直传；status=2 秒传零传输不请求 gettoken；status=7 sign_check 二次区间）；oss_put 500/403 → `Err`（F-11 回归）；oss_put 200 → `Ok`；list/downurl 失效映射。**归属写明（MA3③）**：`mock_api.rs:225-263` 既有秒传测试属 **Open 通道**（Open115Provider/ApiClient），本套为 **Cookie 通道**独立用例（不同驱动、不同端点族），不重复；AC14 的 initupload 成功用例经 Gate#1 裁决生效，错误路径基线保留为其子集（T-24 保留）。
  - 前提确认（沿 test-strategy）：本地明文 HTTP mock 需 ureq Agent 不强制 https——mock_api.rs 先例证明 Open 通道可行，cookie 通道同一 Agent 家族；OSS 模板注入 `http://` 形态依赖同一前提。
- 调用方影响：无（纯加法 builder，默认渲染与现状逐字节一致；既有调用方零改动——`with_base_urls` 全仓零调用方，签名不动）。回归风险：低（默认路径字节等价；ECDH 注入仅 `#[doc(hidden)]` 测试路径）。

#### cloudmount-daemon

**F-03 serve_static 路径遍历（Windows 盘符逃逸）**
- 现状核实：`webdav.rs:922-954`，仅 `rel.contains("..")`；`root.join("C:/foo")` 在 Windows 上 `Path::join` 遇绝对成分直接替换基路径 → 任意盘文件读。**补充核实（m11）**：UNC 前导斜杠形态（`//server/share`）经 `:923` `trim_start_matches('/')` 剥掉全部前导斜杠后成为相对段序列、join 结果仍在 root 内——**现状并不逃逸**；真正逃逸的是**盘符形态**（`C:/x`、`C:\x`）。
- 修复策略：**成分级校验 + canonicalize 双检**两层：
  1. 成分级（先拒，零 IO）：百分号解码后按 `/` 切分，跳过空段，拒绝任一满足如下条件的段——`..` 或 `.`；含 `:`（拦盘符 `C:` 与 NTFS 备用数据流 `file:stream`）；含 `\`（维持单分隔符不变量）；为 Windows 保留设备名（CON/PRN/AUX/NUL/COM1-9/LPT1-9，防打开设备挂起）。重组后 `root.join(所有合法段)`。
  2. canonicalize 兜底：`full.canonicalize()` 后要求 `starts_with(root_canonical)`（Rust 的 `starts_with` 按 Path 组件比较，无前缀串误判；两侧都过 canonicalize，`\\?\` 前缀一致；根的 canonical 形式用 `OnceLock` 缓存一次）。解析失败或越界 → 403。
  - web-ui 目录内的合法 symlink 指向目录外也会被第 2 层拦下（第 1 层拦不住的场景）。
- **红测向量口径（m11）**：以**盘符为主**（`/C:/Windows/win.ini`、`/C:\foo`、百分号编码与反斜杠变体）；**UNC 段拒绝保留为纵深防御**（防 trim 语义变化、编码变体与未来路径处理改动——非现状可利用路径）。
- 调用方影响：`serve_static` 私有函数，签名不变。合法资产路径（`index.html`、`assets/*`）不受影响。
- 回归风险：低。测试：盘符向量各断言 403/404 且响应体不含目标内容；`/a/../b`、合法 `/assets/app.js` 断言 403/200；正路径（`/`、`/index.html`、`/assets/app.js` → 200）并入 T-05 harness 顺带覆盖。

**F-05 daemon 硬编码绕过 Cookie 通道开关**
- 现状核实：`main.rs:376-379`（`--channel cookie`）与 `:413-416`（auto 兜底 `try_cookie_provider`）均硬编码 `CookiePolicy { enabled: true, consent_acknowledged: true }`。CLI 侧 `cloudmount-cli/src/main.rs:280-283` 的硬编码属登录流程本身（交互式同意已发生），不属缺陷。**desktop 壳 `apps/desktop/src/main.rs:143-150` 同样硬编码双真且 GUI 无同意交互、不写 `[cookie]` 段——Gate#1 裁决纳入本 intent，设计见 apps/desktop 节。**
- 修复策略：
  - `cloudmount-core/config.rs` 新增子配置 `CookieChannelConfig { enabled: bool = false, consent_acknowledged: bool = false }`，挂为 `Config.cookie` 字段（`serde(default)` + 既有 `deny_unknown_fields`，旧配置文件完全兼容）。
  - `build_channel_provider` / `try_cookie_provider` 改为从 `file_cfg.cookie` 构造 `CookiePolicy` 传入（私有函数签名下传 file_cfg，无外部 API 影响）；`check_enabled()`（`cloudmount-auth/cookie.rs:72`）原样把关：未开启/未同意 → 明确 `Err`（`--channel cookie` 时 fail 并提示"先 `cloudmount login --with-cookie-channel`"），auto 时静默跳过 Cookie 兜底（纯官方）并补一条 info 日志（"Cookie 通道未启用（配置 [cookie]），auto 模式纯官方"）——行为变化可观测，为 AC5 的 auto 断言提供锚点。
  - 配套：`cloudmount-cli::cmd_login_cookie` 登录成功后把 `cookie.enabled=true, consent_acknowledged=true` 写回 `config.toml`（`config::load` → 变更 → `save`，原子写），形成"登录即同意即持久化"闭环。
- **升级行为注（m12 补记，双向）**：
  - 正向（原有）：已登录 Cookie 但无 `[cookie]` 配置段的用户升级后通道回到默认关闭——恢复产品边界（缺陷即错误行为），见开放问题 #2。
  - **反向窗口（m12 新记）**：新 CLI/desktop 登录成功写入 `[cookie]` 段后，**旧版** daemon/desktop（`deny_unknown_fields`）读同一配置将解析失败——旧 daemon 叠加 F-13 未修的静默回退（warn + 默认值 → **全配置丢失**回默认），旧 desktop `load_effective().unwrap_or_default()` 同样吞错。缓解：发布说明提示升级顺序（先升级 daemon/desktop，再用新 CLI 登录/重新登录）；备选"仅对 `[cookie]` 段宽容解析"与 `deny_unknown_fields` 防拼写设计冲突，默认不做——开放问题 #2 扩记。
- 回归风险：中。测试：配置缺省 → cookie 通道不可用；`[cookie]` 双真 → 可用；仅 `enabled=true` 未同意 → 拒绝；auto 通道三态（默认关/未同意/双真）。

**F-08 WebDAV spool flush 泄漏（Windows）**
- 现状核实：`webdav.rs:595-621`，flush 用 `guard.as_ref()`（句柄留在 `Mutex<Option<File>>` 内）上传后 `remove_file`；句柄未关 → Windows 上删除/共享冲突、Drop 语义依赖不成文。无 `Drop` 实现、flush 无幂等护栏（dav-server 可能多次 flush → 重复上传）。
- 修复策略（语义按编排者 canonical 裁决统一）：
  - `WriteFileWithProvider` 增加 `flushed: bool` 字段（`DavFile` 方法均为 `&mut self`，plain bool 足够）。**`flushed` 语义 = 上传确认成功后置位**（`provider.upload` 返回 `Ok` 时）——上传尝试本身不置位（防二次 flush 掩盖失败）。
  - flush：幂等护栏（已 flush 直接 `Ok`）→ 锁内 `flush()` → `guard.take()` **拿出所有权并在作用域结束关闭句柄** → 上传 → `Ok` 时置 `flushed = true` → `remove_file`（尽力；**删除失败 = `tracing::warn!` 记路径后接受残留，不做重试机制**——编排者裁决；不掩盖上传结果）→ 返回上传结果（上传 `Err` 原样上抛，`flushed` 保持 false）。
  - 新增 `impl Drop`：`file.lock().take()` 关句柄 + 若 `!flushed` 删除 spool 残留（尽力 + warn；`NotFound` 视为 no-op 不告警）——PUT 中途断连的自愈；flush 正常成功后 Drop 的 remove 为 no-op。
- 调用方影响：无外部 API。回归风险：低。测试：flush 成功后**本次 PUT 的具体 `cm-dav-put-*` 路径**不存在（勿全目录 glob，防并行测试互踩）；该 PUT 生命周期内 `provider.upload` 恰被调用一次（排除 Drop 二次上传）；删除失败注入（Windows 共享冲突）→ warn 且不视为上传失败、接受残留；上传失败 → Drop 兜底清理。

**F-09 WebDAV 写打开错误混淆**
- 现状核实：`webdav.rs:292-309`，`Err(_)` 把一切 resolve 错误当"新文件"。`resolve_path`（`:107-159`）与 `map_err`（`:180-185`）都能精确产出 `FsError::NotFound`。
- 修复策略：匹配收窄——`Err(FsError::NotFound) => 合成新条目`，`Err(e) => return Err(e)`。权限/网络/频控错误如实上抛（dav-server 映射为 5xx），静默新建误写路径关闭。
- 测试注入点注（MA7 交叉参考）：错误注入的 Provider 包装（`Arc<dyn Provider>` 直接可实现）经 `ProviderFs::new(Arc<dyn Provider>)`（`webdav.rs:651`）即可挂入 harness——**生产侧无需任何改动**，harness 形态归 test-strategy。
- 调用方影响：PUT 既有文件的错误行为更严格，属缺陷修复本身。回归风险：低。测试：对不存在文件 PUT 成功；对父目录不可达（mock provider 返回非 NotFound）PUT 得到错误而非新建、`provider.upload` 计数为 0。

**F-12 SCM 服务路径忽略 placeholder_mount；service.rs"死代码"**
- 现状核实（对审计的修正）：`service.rs` **并非整体死代码**——`install/start/stop/uninstall/run_service_scm_executor/StopSignal/should_exit` 均被 `main.rs:171-222/424-459` 调用。真正死的是 `service.rs:146-148` 的 `define_windows_service!(ffi_service_main, service_main)`：全仓无 `service_dispatcher::start`，`ffi_service_main` 无任何调用方（SCM 实际入口是 main.rs 的 `service run` clap 分支 → `run_service_scm_executor` 手动注册控制处理器）。真实缺陷：`main.rs:193-220` Run 分支从不检查 `cli.placeholder_mount`（而 `build_launch_args:294-303` 确实把 `--placeholder-mount` 烘焙进 binPath）→ 服务模式收到 flag 后静默跑同步模式；`service_main` 的手写参数解析也丢弃 `--placeholder-mount`。**潜伏 bug（MA9）**：`build_launch_args` 不烘焙 `--interval`（`service_main:177` 却解析它）→ 服务模式恒默认 30s。
- 修复策略：
  - `ServiceCommands::Run` 分支按 `cli.placeholder_mount` 分派：true → 新增 `run_service_placeholder_blocking`（SCM 版占位符挂载循环）；false → 维持现有 `run_service_engine_blocking`。
  - `run_placeholder_mount` 重构：把挂载主循环抽出，增加参数 `stop_check: &dyn Fn() -> bool`（前台传 `|| false`；SCM 传 `stop.should_stop() || service::should_exit()`），循环体内轮次间与分段 sleep 处检查；停止后走既有 `control.stop()` + `Arc::try_unwrap` + `unmount` 干净卸载路径。
  - service.rs：**保留**全部在用部分，**删除**死 FFI 入口（`define_windows_service!` + `service_main` 手写解析，共约 70 行）——消除与 clap 的双份解析；保留手动 `run_service_scm_executor` 模式（已在用、可测）。
  - `build_launch_args` 补 `--interval`：**无条件烘焙**（值即 clap 解析结果，含配置文件默认注入——`apply_config_defaults` 已把文件值注入 clap default）。
- **验收落点（MA9）**：`build_launch_args` 快照/单元测试——`Cli::try_parse_from(["cloudmountd", "--placeholder-mount", "--interval", "15", "--sync-root", <root>, ...])` 构造 Cli → 断言输出参数序列含 `--placeholder-mount`、`--sync-root <root>`、`--interval 15`（**`--interval` 与 `--placeholder-mount` 均烘焙**；修复前 `--interval` 缺失即红）。`run_placeholder_mount` 的 stop_check 抽象为纯函数可单测（placeholder_mount=true/false 两路径构造一致）。
- 调用方影响：Windows 服务用户行为与前台运行对齐（缺陷修复本身）；`--hold-secs/--once` 不参与服务路径（服务即常驻），无需烘焙。
- 回归风险：中（SCM 路径只能集成验证；真实服务生命周期进人工清单）。

**F-13 daemon 配置加载失败静默回退**
- 现状核实：`main.rs:162-165` `load().unwrap_or_else(warn + default)`。而 `cloudmount-core/config.rs:100-107` 的 `load_from` 语义已正确：文件不存在 → 默认值（首次运行正常路径）；文件存在但解析/读失败 → `Err`（消息 "配置文件解析失败 {path}"，`config.rs:106`）。错误被 daemon 侧吞掉。
- 修复策略：改为 fail-fast——`let file_cfg = cloudmount_core::config::load().context("配置文件加载失败（修正该文件，或删除后重启以恢复默认配置）")?;`。文件缺失不受影响；解析失败 → anyhow 错误链带路径与原始 toml 错误，进程非零退出。
- **断言锚（编排者裁决 #3）**：AC13 断言子串「解析失败」+ 文件路径——锚定 `load_from` 实际消息（`config.rs:106`）；daemon 侧 context 文案非验收字符串。
- **边界写明（m17）**：fail-fast 位于 main 入口、先于一切子命令分派 → 作用于**全部子命令**（含 `service stop/uninstall`——坏配置时管理命令同样拒绝执行；`sc.exe` 直接操作服务是用户侧兜底途径，错误信息文案可提示）。
- 调用方影响：坏配置从"静默降级"变"拒绝启动"，属缺陷修复本身。回归风险：极低。测试：现有 `malformed_file_errors` 压住 core 语义；daemon 侧以 `load_from` 注入路径单测（无需起服务）；进程级断言（退出码/输出）接受单元代理（提取 main Result 边界断言 Err 链），黑盒用例进人工清单。

#### apps/desktop（Gate#1 裁决新增：F-05 的 desktop 落点）

**现状核实（MA2）**：`apps/desktop/src/main.rs:143-150` `login_auth()` 硬编码 `CookiePolicy{true,true}` 且每帧重建 CookieAuth；登录流程 = 登录页"开始扫码登录"（`:460-493`）→ `begin_login` → `update()` 内每帧 `poll_login`（`:237-261`）→ `Ok(Some(_))` 仅更新内存状态（"登录成功（Cookie 已入独立容器）"），**不写 `[cookie]` 段**；登录页已有风险声明文案（`:443-446`）但无显式同意交互。`apps/desktop/Cargo.toml`（独立项目，根 workspace exclude）**已含** `cloudmount-core/auth/driver115/sync` 全部 path 依赖 → **依赖调整 = 零**。desktop 引擎线程（`engine_loop`）仅构造 Open 通道 Provider，Cookie 仅用于登录——无其它 CookiePolicy 构造点。三个二进制共用同一份 config.toml（`config.rs` 模块文档）。

**最小改动设计（决策 #14）**：
1. **知情同意交互**：登录页风险文案升级为完整版（对齐 CLI 四条风险声明），下方增加 egui 勾选框「我已阅读并知悉上述风险，同意启用 Cookie 通道」——**初值 = 配置 `cookie.consent_acknowledged`**（已同意过的用户不重复打扰）；未勾选时"开始扫码登录"按钮禁用。勾选框是 CLI"输入 yes"的 GUI 等价物（不变量 #4：同意仅由登录交互产生）。
2. **CookiePolicy 从配置构造**：点击"开始扫码登录"时构造**会话级** `CookieAuth`（一次构造、轮询复用——顺带消除现状每帧重建的低效）：policy = `{ enabled: true, consent_acknowledged: true }`（点击即显式开启 + 勾选即同意）；`check_enabled()`（auth crate 既有把关）在未同意场景拒绝 `begin_login`，desktop 不重复实现校验。会话 auth 存为 `CloudMountApp` 字段（`Option<CookieAuth>`），`update()` 轮询复用。
3. **登录成功写回 `[cookie]`**（`update()` 的 `Ok(Some(_))` 分支）：
   ```rust
   if let Ok(mut cfg) = cloudmount_core::config::load() {
       cfg.cookie.enabled = true;
       cfg.cookie.consent_acknowledged = true;
       if let Err(e) = cloudmount_core::config::save(&cfg) {   // save 内部走 save_to 原子写（tmp + rename）
           tracing::warn!(error = %e, "Cookie 同意标志持久化失败（下次启动需重新同意）");
       }
   }
   ```
   写回模式与 desktop 设置页 `sync_root` 写回（`main.rs:411-413`）同构——文件内既有先例。
- **测试口径**：desktop 为 GUI 独立 crate（workspace exclude、无测试基建），写回逻辑为对已测 pub API（`config::load/save`，cloudmount-core 往返测试压住）的三行调用——**不新增自动化测试**；人工清单：扫码登录成功 → config.toml 出现 `[cookie]` 双真 → 重启 desktop 勾选框预勾、可直接再登录 → daemon `--channel cookie` 读同一配置可用。
- 回归风险：低（纯 GUI 流程内改动，引擎线程零触及）。

#### CI 与门控设计（MA8，设计先行声明；IaC 于 Construction 落地）

- 现状缺口：`webdav_serve.rs` 整文件 `#![cfg(feature = "mock")]` 而 CI 仅跑 `cargo test --workspace`（无 `--features`）→ F-03/F-08/F-09 的集成测试**在 CI 从不执行**（安全项空绿）；cfapi 集成腿（T-01/T-02）在 GH windows runner 因 SyncRoot 注册失败（0x80070490）自动跳过。
- 设计：CI workflow **新增并列 job**：`cargo test --workspace --features cloudmount-daemon/mock`（三平台矩阵维持）——使 webdav_serve 套件进入门控。
- 设计注（供 qa/pm 对齐，断言落点属 test-strategy/PRD）：windows-only 清单与 skip 语义（cfapi SyncRoot 失败自动跳过、T-14/T-09 windows 门控）应在 CI 产物中可辨识（job/步骤命名标注），避免"三平台绿"误读为全量覆盖。

## 数据模型

本 intent 不引入持久化 schema 变更，除以下一处：

| 实体/结构 | 变更 | 键/字段 | 约束 |
|---|---|---|---|
| `config.toml`（cloudmount-core::Config） | **新增** `cookie` 子配置 | `[cookie] enabled: bool, consent_acknowledged: bool`（默认均 false） | `serde(default)` + `deny_unknown_fields`：旧文件可解析；缺段 = 双 false = 通道关闭；仅 `enabled=true` 无 consent 仍拒绝启用。**写入方两处**：CLI `cmd_login_cookie` 与 desktop 登录成功回调（均经 `config::save` 原子写：tmp + rename） |
| 令牌（cloudmount-auth::TokenSet，内存+SecretStore） | **无 schema 变更** | `access_token/refresh_token/expires_at` | 不变量：刷新全程持 `tokens` 锁（**双入口**：access_token 与 refresh/refresh_stale 共用同一助手）；持久化顺序 = 先经守卫更新内存、锁外写 SecretStore（崩溃窗口内内存与磁盘至多差一次刷新，不产生互踩） |

进程内瞬态结构（不落盘）：

- `Executor.dir_creating: Mutex<HashMap<String /*prefix*/, Arc<Mutex<()>>>>`（F-07）——per-前缀创建锁，键为 `a/b/c` 形式的目录前缀；条目在对应 `dir_ids` 插入后移除。
- `RdcWatcher` 字段改为 `handle: Option<Arc<RawHandle>>`、`thread: Option<JoinHandle<()>>`、`exited: Arc<AtomicBool>`（F-06）；`RawHandle.closed: AtomicBool` 保证 `CloseHandle` 恰一次；模块级 `ACTIVE_WATCHERS: AtomicUsize` 活跃计数（cfg 无关，生产常驻）。
- `WriteFileWithProvider.flushed: bool`（F-08）与 spool 临时文件（`%TEMP%/cm-dav-put-*`）生命周期：flush 确认成功或 Drop 时必被尝试删除；残留仅允许在 remove_file 失败且已 warn 的场景（接受残留、无重试机制）。
- `Cookie115Provider` 注入字段（F-14，默认 = 生产值，渲染逐字节等价）：`upload_base: String`（完整基址含 scheme）、`oss_base: String`（完整 URL 模板含 scheme 与 `{bucket}/{object}` 占位）、`ecdh_remote_pub: Option<[u8;56]>`（doc(hidden)）、limiter 覆盖。
- `OpenApiAuth` 注入字段（F-10 测试使能）：`passport_base` 覆盖（`#[doc(hidden)]`，默认 = PASSPORT_BASE 常量）。
- 索引：无新增（`dir_ids` 键仍为 rel 前缀字符串，`MountState.remote` 键仍为 rel 路径）。

## 与 PRD 的语义对齐表

对齐锚 = F-xx ↔ AC-x（对齐拍已核对 1:1；AC15 为 intent 约束项）；desktop 行为 Gate#1 裁决新增（U5 扩面）：

| 锚 | 修复动词 | 物理语义 | 说明 |
|---|---|---|---|
| F-01 | 分块流送 | FETCH_DATA 范围经 `open_range` 单次建流 + 1 MiB 块循环 `TRANSFER_DATA`（`pump_chunks` crate 内可测函数）；内存峰值 = 1 块（≈1 MiB），与文件大小无关；取消/失败以失败 NTSTATUS 收尾挂起读 | 原"整个范围 read_to_end 进 Vec"语义废除；**断言口径 = reader 级读取序列**（read_to_end 几何增长 vs 1 MiB 定长可区分；open_range 调用次数前后相同，不可作断言） |
| F-02 | 转发 | `ProviderRouter.change_probe == 活跃通道.change_probe`（Auth 失效随 `call()` 自动切换）；不支持探测的通道返回 `None` → 引擎回退全量扫描 | auto 通道恢复快路径，无行为新增；测试落点 = router.rs 内扩展现成 Stub（零 dev-dep，决策 #1 冲突消解） |
| F-03 | 拒绝 + 双检 | 静态文件服务的每个路径成分先验拒（`..`/`.`/`:`/`\`/空段/保留设备名），再 canonicalize 后强制 `starts_with(root)`；违规 → 403，**任何文件系统访问都发生在校验之后** | 原字符串 `contains("..")` 检查废除；UNC 段拒绝保留为纵深防御（现状 trim 后并不逃逸），红测向量以盘符为主 |
| F-04 | 脱敏 | **五处**（cookie.rs:401/492/560 + cli main.rs:233 错误回显 + cookie.rs:796 get_info 日志）：凭据字段只记字段名+长度，值永不落日志/错误输出（任何 trace 级别） | 三处 debug! 改字段级脱敏 + 两处扩围（编排者裁决：不变量 #2 已知反例与同类原样打印）；捕获机制 = 手写 tracing Subscriber（仅 tracing 本体） |
| F-05 | 回接开关 | Cookie 通道可用性 = 配置 `[cookie] enabled && consent_acknowledged` 双真；默认双 false = 关闭；同意的唯一持久化位置是 config.toml，由 CLI 登录与 **desktop 登录**（双写入方）产生 | daemon/desktop 两处组装硬编码 `true/true` 废除；升级行为（正向关闭 + 反向解析窗口）见开放问题 #2 |
| F-06 | 真释放 + 有界收割 | Drop = 置停止标志 → 幂等关闭目录句柄（尽力唤醒，非文档化保证）→ **5s 有界收割**（轮询 exited；超时 detach + warn，泄漏回到可观测）；Drop 正常返回即线程与句柄均已终结 | 原 `drop(self.handle.clone())` 空操作废除；`ACTIVE_WATCHERS` 计数 hook 支撑 50-100 轮循环断言 |
| F-07 | 锁内计划/锁外执行 | `dir_ids` 锁内只做查表与缺失链计划；网络 IO（list/create_dir）在锁外、per-前缀锁 + 双检之下执行；"不重复建目录"语义保持 | 持锁跨网络往返废除 |
| F-08 | 所有权收口 + 自愈 | flush 幂等；句柄经 `take()` 关闭后再上传再删 spool；**flushed 于上传确认成功后置位**；**删除失败 = warn + 接受残留（无重试机制）**；Drop 兜底清理未 flush 的 spool | `as_ref()` 半泄漏与静默残留废除 |
| F-09 | 精确匹配 | 仅 `FsError::NotFound` 走"合成新文件"路径；其它打开错误原样上抛（5xx） | `Err(_)` 盲当新文件废除 |
| F-10 | 单飞（**双入口**） | 同一 `OpenApiAuth` 实例的并发刷新被 `tokens` 锁串行化，覆盖两条入口——`access_token()`（时间性过期）与 `Authenticator::refresh()`（服务端判失效，`client.rs:244` 并发重试路径经 `refresh_stale` 显式传失效 token）：每时刻至多一个在途刷新请求；等待者共享刷新结果（双检命中）；并发 refresh 经 token 身份双检去重为恰 1 次；刷新失败则等待者依次串行重试 | "拿锁后、网络前丢锁"在**两条入口**都废除（互审 MA1） |
| F-11 | 显式失败 | OSS PUT 非 2xx → `Err(Error::Http{status, 摘要})`：5xx 进既有轮内重试（is_retryable）、4xx（含 403）快速失败不无限重试；"上传成功"判定必须以 2xx + 既有 verify 命中为准 | 吞状态码废除；错误分类边界：HTTP 状态 → Http，业务码 → Remote/Auth（维持现状） |
| F-12 | 接线 + 删死码 | SCM Run 分支按 `--placeholder-mount` 分派到停止感知的占位符挂载循环（与前台同一循环体 + stop_check 闭包）；service.rs 保留在用部分、删除无调用方的 FFI 入口 | 服务/前台行为一致；`binPath` 烘焙补 `--interval`（无条件）+ `build_launch_args` 快照测试（--interval 与 --placeholder-mount 均断言） |
| F-13 | fail-fast | config.toml 存在但解析失败 → 进程非零退出并指明文件路径（断言锚 = load_from 消息"解析失败"+路径）；文件不存在 → 默认值（首次运行不受影响）；边界 = 全部子命令（含 service stop/uninstall，sc.exe 为用户侧兜底） | 静默回退默认值废除 |
| F-14 | 补网 + 回环 | cookie.rs 获得 HTTP 级回归测试（driver115/tests/ 集成层 + 本地 mock 服务器，零新依赖）；**initupload 解密成功回环纳入本期**（ec115.rs test_peer ECDH 对端）；注入面 = `with_base_urls` 纯加法扩展（完整基址含 scheme，消 :605 https:// 字面量） | 零生产代码行为变化（默认渲染逐字节等价；ECDH 注入仅 doc(hidden) 测试路径）；作为 F-04/F-11 的安全网先行提交（TDD） |
| **desktop**（Gate#1 裁决新增） | 登录 + 写回 | GUI 登录成功 → `config.toml [cookie]` 双真（`config::save` 原子写）；CookiePolicy 从配置构造（勾选初值取配置、会话级构造一次）；同意交互 = 登录页显式勾选（CLI "输入 yes" 的 GUI 等价物） | U5"所有组装路径一致生效"（daemon/CLI/desktop 三处）完整成立；desktop 独立 Cargo.toml 零依赖调整 |

## 关键决策与理由

| # | 决策 | 为什么 |
|---|---|---|
| 1 | **不引入任何新依赖（含 dev-dependencies）**：F-14 用手写 `TcpListener` HTTP mock（扩展 mock_api.rs 先例）；F-04 用**手写 `tracing::Subscriber`**（约 40 行，仅 tracing 本体——非 MakeWriter/tracing-subscriber，后者属 tracing-subscriber crate 且 driver115 零 dev-deps）；F-14 ECDH 对端走 ec115.rs `#[doc(hidden)]` 模块、**复用 crate 正式依赖**（p224/aes/lz4_flex 本就是 driver115 正式依赖——同版本复用、无 dev-dep 声明、无新供应链）；F-02 测试在 router.rs 内扩展现成 Stub（m10 冲突消解，不引入 cloudmount-sync dev-dep） | 修复型 intent 最小 diff 原则；workspace 无测试服务器/捕获框架依赖先例；手写 responder 对 ureq 的 HTTP/1.1 用法（固定 body、无 chunked/TLS）足够；避免为测试引入需要审计的第三方供应链；决策与测试策略的注入面描述已按 canonical 裁决统一 |
| 2 | **F-01 块大小 = 1 MiB**，单块复用缓冲 | 过滤器单次请求通常 ≤1–8 MiB，1 MiB 与之同量级；4 GiB 文件 = 4096 次 `TRANSFER_DATA`，调用开销可忽略；峰值内存 ≈1 MiB + reader 内部缓冲，完全不威胁空闲 RSS 5–15MB 纪律（对比：256 KiB 会把调用数翻 4 倍、4 MiB 削弱小内存优势）；`TRANSFER_DATA` 无最小/最大块约束，官方样例即 ~1MB |
| 3 | **service.rs 保留在用部分、仅删除死 FFI 入口**（`define_windows_service!` + `service_main`），不改走 `service_dispatcher` 标准模式 | 源码核实：install/start/stop/uninstall/executor 均在用，整体删除会破坏 `service install` 等已发布子命令；死入口删除消除与 clap 的双份参数解析（正是 F-12 的病灶之一）；`service_control_handler::register` 手动模式已在生产路径验证，切换标准模式属无收益重构 |
| 4 | **F-05 同意标志持久化在 config.toml**（而非独立 marker 文件），由 CLI 登录与 desktop 登录流程写入 | 复用既有配置加载/原子写/优先级体系（clap > env > 文件 > 内置），零新机制；`deny_unknown_fields` 保证拼写错误显式失败；登录交互（CLI yes / desktop 勾选）是同意的唯一产生点，daemon 只消费 |
| 5 | **F-13 采取 fail-fast**（而非 告警+继续） | "配置静默降级"正是缺陷本身；continue-with-banner 仍会以错误配置跑同步/删除传播（propagate_* 是破坏性开关），风险不对称；文件缺失回默认值的首次运行路径不受影响 |
| 6 | **F-03 采用 成分级校验 + canonicalize 双检** 两层，而非只取其一 | 成分级拦截零 IO 且拒绝语义明确（403 原因可日志化）；canonicalize 兜底覆盖符号链接/8.3 短名等校验规则遗漏面；两者成本均可忽略（静态资产服务低频路径） |
| 7 | **F-10 双入口共用同一持锁刷新助手**（修订）：`access_token()`（时间性双检 `!needs_refresh`）与 `Authenticator::refresh()`（token 身份双检）共用 `refresh_locked(skip_if)`；`client.rs:244` 改调 `refresh_stale(&token)` 显式传失效 token | 一把锁达成 single-flight 的最小形态（互审 MA1：只改 access_token 病灶残留）；身份双检关闭"并发重试路径的串行多余刷新"（背靠背刷新即 40140117 触发形态）；独立刷新锁会引入锁间窗口与额外双检状态，复杂度更高收益为零；副作用（刷新期间 logout 阻塞 ≤30s）语义正确且可接受（见开放问题 #5） |
| 8 | **F-01 只改 HydrateFn 契约、不改 CF_HYDRATION_POLICY_FULL** | OOM 根因是全量缓冲不是水合策略；改策略（如 PARTIAL）会改变"任意读触发完整水合"的产品可见行为，超出缺陷修复范围 |
| 9 | **F-07 用 per-前缀锁保住"不重复建远端目录"**，而非简单把网络挪出锁后听任重复创建 | 前者是既有安全性设计（exec.rs:9 文档化），不能为性能回退正确性；per-前缀锁只串行"同一目录"的创建，不同目录并行，锁序固定无死锁 |
| 10 | **F-11 错误类型用 `Error::Http{status, body}`**（修订：原 `Error::Remote` 废弃），错误体读取 `take(2048)` 限长 | `is_retryable` 既有分类（error.rs:69-75：Http ≥500 可重试、4xx 不可）恰好分叉瞬时/永久——403 类永久错误不无限重试（m9 硬要求）；与 client.rs `read_body_text` 既有先例一致；`Remote` 一刀切可重试会让永久业务错误无限重试；限长防异常大响应占内存（低内存纪律） |
| 11 | **TDD 顺序**：每个 F-xx 先失败测试后实现；F-14 的测试基建（mock 服务器 + 注入 builder + ECDH test_peer）最先落地 | intent 约束；F-04/F-11 明确依赖 F-14 安全网 |
| 12 | **F-14 ECDH 对端形态 = ec115.rs 内 `#[doc(hidden)] pub mod test_peer`**（而非 tests/ 集成测试 + dev-dependencies p224/aes/lz4_flex） | ①回环硬前提是替换 115 固定远端公钥（无人持有其私钥），替换 seam 无论选哪案都必须落生产 crate——方案二"测试逻辑全在 tests/"的优势因此残缺；②k_ec 逆解码/key-iv 重叠 4 字节派生/[len][LZ4] 帧等协议怪癖与客户端同居一文件**单点维护**，tests/ 独立重实现约百行协议代码会随生产改动静默漂移（假红/假绿）；③维持零 dev-deps（决策 #1 最强形式）——方案二需 dev-dep 声明且集成测试是独立 crate、需人工与正式依赖保持版本同步；④同文件镜像的"同 bug 假绿"风险（ec115.rs:121 注释自述过）以"peer 加解密复用 ec115.rs 既有 CBC/PKCS7/LZ4 代码"与"回环价值定位 = 压 init_upload 编排链（表单/k_ec/HTTP/解密/JSON/status 分支），对真实 115 的互操作仍走人工清单"缓解 |
| 13 | **F-06 Drop 有界收割 = join 超时 5s**（50ms 轮询 exited 标志）→ 超时 detach + `tracing::warn` 降级；**不采用 CancelSynchronousIo** | `CloseHandle` 唤醒在途同步读是尽力而为、非文档化保证（MA4）——超时兜底把失败模式从"Drop 永久挂起（不可观测）"降级为"可观测泄漏"（warn 日志 + ACTIVE_WATCHERS 计数不回落），泄漏可观测严格优于挂起；`CancelSynchronousIo` 需另行管理线程 HANDLE 生命周期（OpenThread + 新句柄 + windows feature 面），且与 CloseHandle 同属尽力而为类手段，收益不抵复杂度；5s 取值与 AC6"有限超时（如 5s）"对齐（本地目录句柄关闭后实践上 promptly 返回，5s 覆盖慢速 FS） |
| 14 | **desktop 最小改动**：登录页显式勾选同意（初值取配置）+ 会话级 CookieAuth（从配置构造 policy、一次构造轮询复用）+ 成功后 `config::save` 写回 `[cookie]`；**零依赖调整**（cloudmount-core 已在 apps/desktop 独立 Cargo.toml 的 path 依赖内） | Gate#1 裁决纳入；复用 config 既有原子写与 auth crate `check_enabled` 把关（desktop 不重复实现校验）；勾选框是 CLI"输入 yes"的 GUI 等价物（同意语义等强）；写回模式与 desktop 设置页 sync_root 写回（main.rs:411-413）文件内同构；会话级构造顺带消除现状每帧重建 CookieAuth 的低效 |
| 15 | **F-04 扩围收口（三处 → 五处）**：并入 cli main.rs:233 cookie 回显脱敏与 cookie.rs:796 get_info 日志脱敏 | 同一安全不变量 #2 的已知反例（回显）与同类原样打印（get_info），一行级脱敏非新需求（编排者代行裁决 #1/#2）；get_info 落点经源码核实修正为 cookie.rs:796（互审记 webdav.rs:796 属文件归属笔误，发现本身属实） |
| 16 | **CI 增加 features job**：`cargo test --workspace --features cloudmount-daemon/mock`（设计先行声明，IaC/workflow 修改于 Construction 落地） | 现状 webdav_serve 整文件 `cfg(feature="mock")` 而 CI 不带 features → F-03/F-08/F-09 安全集成测试从不执行（安全项空绿，互审 MA8）；带 features 的并列 job 使其进入门控，三平台矩阵语义不变 |

## 安全与权限

本 intent 本身是安全修复。修复后必须成立的安全不变量：

1. **路径边界不变量**：任何由外部输入（HTTP 请求路径、控制通道参数）推导出的文件系统访问，必须满足"校验先于访问"，且最终访问路径 canonicalize 后严格位于服务根内。覆盖点：webdav `serve_static`（F-03，成分级 + canonicalize 双检）；既有 `dehydrate_via_mount`（`main.rs:666-681` 的 `strip_prefix(root_canon)`）维持不变。Windows 特化：拒绝盘符段（`X:`）、UNC（`\\`）、备用数据流（`:`）、保留设备名。
2. **凭据不出现在日志与错误输出的不变量**：userkey、OSS STS（AccessKeyId/AccessKeySecret/SecurityToken）、Cookie 值（UID/CID/SEID/KID）、access_token/refresh_token 在任何 trace 级别的日志产物与错误输出中都不得以明文出现；唯一合法格式是字段名 + 长度（`redact_fields` 纯函数，F-04 五处施工面：cookie.rs:401/492/560/796 + cli main.rs:233）；该不变量由捕获型日志回归测试（手写 tracing Subscriber，trace 全开）固化。
3. **上传成功判定不变量**：远端写操作只有在获得显式服务端确认（OSS PUT 2xx，且既有 `verify_upload` 命中）后才允许记为成功；任何非 2xx、解析失败、传输错误都必须产生 `Err` 并进入既有重试分类（F-11：`Error::Http{status}`——5xx 轮内可重试、4xx 快速失败）。禁止任何"静默当成功"路径。
4. **Cookie 通道边界不变量**：Cookie 通道保持"默认关闭 + 显式开启 + 知情同意"双标志把关（F-05）；同意仅由登录交互产生并持久化（CLI yes / **desktop 勾选**，Gate#1 裁决补全）；daemon 与 desktop 引擎是消费者、永不自产同意；**所有组装路径（daemon/CLI/desktop）一致生效**（U5 语义完整成立）。
5. **权限面**：本 intent 不涉及 IAM/网络策略/密钥管理变更；无新增监听端口（控制通道维持本地回环 + 挂载根前缀校验）；SCM 服务仍以 LocalSystem 运行既有能力，占位符挂载接线（F-12）不扩大攻击面（同一挂载 API、同一 stop 语义）。密钥存储维持 cloudmount-auth 独立容器（OpenApiToken / CookieJar 物理隔离）现状。

## 开放问题（需人类裁决）

- [ ] **F-13 失败语义确认**：fail-fast 意味着"配置文件坏 → 服务/前台都拒绝启动"（含 service stop/uninstall 等管理子命令，sc.exe 为用户侧兜底）。备选是"显式横幅告警 + 拒绝破坏性开关下继续"。另请确认"文件存在但**不可读**（权限/IO）"是否同样 fail-fast（默认：是，只有文件不存在走默认值）。默认按 fail-fast 设计。
- [ ] **F-05 升级行为（双向）**：①正向：已登录 Cookie 通道但配置文件无 `[cookie]` 段的用户，升级后通道回到关闭状态（需重新登录或手动补配置）——恢复产品边界的预期结果，但属用户可感知变化，请确认接受；②**反向窗口（m12 补记）**：新 CLI/desktop 写入 `[cookie]` 段后，**旧版** daemon/desktop（deny_unknown_fields）读同一配置解析失败——旧 daemon 叠加 F-13 未修的静默回退、旧 desktop `unwrap_or_default`，均导致全配置丢失回默认。建议发布说明提示升级顺序（先升 daemon/desktop 再用新 CLI 登录）；备选"仅对 [cookie] 宽容解析"与防拼写设计冲突，默认不做。③同意标志落 config.toml（方案 A）还是独立 marker 文件（方案 B）仍请定夺（本稿按 A 设计）。
- [ ] **F-12 service.rs 处置**：推荐"保留在用部分 + 删除死 FFI 入口"（关键决策 #3）；备选是改走 `windows-service` 标准 `service_dispatcher` 模式（更正统但属无收益重构，且与 clap 解析需重新融合）。
- [ ] **F-01 块大小**：1 MiB 为缺省值（关键决策 #2）；若对超低速链路上的 `TRANSFER_DATA` 调用次数有顾虑，可调成 2/4 MiB（仍是常数内存）。
- [ ] **F-10 锁副作用**：刷新期间（≤30s）`logout/is_logged_in` 将阻塞等待同一把锁。语义正确但属可感知变化；若不可接受，需引入独立刷新锁 + 双检（复杂度更高），默认不采用。
- [ ] **desktop 同意交互形态确认**：本稿按"登录页显式勾选框（初值取配置）"设计（决策 #14）；若产品侧要求更强的同意形态（如每次登录强制重读声明），请指出——改动限于 desktop 登录页，不影响其余设计。
