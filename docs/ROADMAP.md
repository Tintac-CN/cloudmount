# CloudMount 路线图

> 2026-10 基线。原则：**每阶段都出可用物**；M0 尽早申请 115 开发者资质
> （审核约 7+7 工作日，是唯一的外部阻塞项）。

## M0 · 打通官方 API（1–2 周）

- [ ] 申请 115 开放平台个人开发者 + 创建 PKCE 应用（外部审核，立即启动）
- [x] `cloudmount-cli login`：PKCE(S256) 设备码扫码 → 终端二维码渲染 → 轮询换取 token
- [x] 凭据持久化：Windows 凭据管理器 / macOS Keychain / Linux 0600 文件
- [x] token 自动刷新（单飞 + 5 分钟提前量 + 40140123–26 失效重试一次）
- [x] `ls`（分页 + usertime 排序）、`quota`（rt_space_info 解析）
- [x] `upload`：SHA1/前128K 预计算 → 秒传命中 → 二次认证（sign_check 闭区间）→ OSS 直传
- [x] `download`：downurl 直链 + Range 流式下载（带进度）
- [x] 全局限速（1 r/s 令牌桶）+ 指数退避 + 频控码识别（406 / 40140117）
- [ ] **待 AppID 实测**：真实账号跑通全链路；确认 copy/新建目录的响应字段
- **验收**：CLI 全流程可用；请求速率 ≤1 r/s 无频控

## M1 · Rust 同步引擎（3–5 周）

- [x] SQLite（bundled）元数据库：上次同步快照（三向对比基准）、状态机、pin 策略
      （`水合范围位图` 属 VFS/占位符层，随 M2 落地）
- [x] 本地监控抽象 `LocalWatcher` + 可移植轮询基线（`PollingWatcher`）
      ；USN/FSEvents/inotify 原生实现待接入（M1.5）
- [x] 远端变更发现：`change_probe` 廉价探测（1 请求）+ 全量扫描兜底 + 水位短路
      （短路时**用快照合成远端视图，零远端请求**）
- [x] 三向对比 `reconcile::plan`：纯函数、确定性输出、可穷举测试；
      冲突默认"保留两者"，删除传播默认关闭（保守，防误删）
- [x] 队列调度：并发/重试退避/暂停；只重试可重试错误（网络/频控/5xx）
- [x] 块缓存：LRU + 字节限额 + 命中统计 + 索引持久化（下载去重生效）
- [x] 执行器：原子写入（临时文件 + rename）、保持远端 mtime、删除进**回收站**
- [x] `cloudmountd` 无头服务：同步循环 + `--once`（可作 cron/边缘常驻）
- [x] **114 个测试**（54 单元 + 20 端到端 + 40 其它）；端到端用内存 Mock 远端覆盖
      双向同步/冲突/删除策略/幂等/短路/暂停恢复语义
- [ ] 待真实凭据联调：两台设备经 115 完成一次干净的双向同步（依赖 M0 的 AppID）
- [ ] 验收指标：空闲 RSS ≤ 20MB（需真机测量，`cloudmountd --once` 下观测）

**已知边界（诚实记录）**：变更检测基于 (size, mtime) 秒级粒度，**同秒内等大小修改会漏检**
（已有专门测试固定该行为）；缓解方案是 M1.5 的可选内容校验通道或平台原生 watcher。

## M1.5 · 原生 watcher + 内容校验 + 重命名（已完成）

- [x] **Windows 原生 watcher**（`ReadDirectoryChangesW`）：无需管理员权限（USN 需要提权，
      这是选 RDC 而非 USN 的原因）、一个句柄监听整棵子树、后台线程 + 脏路径归约 +
      溢出/周期性全量对账自愈；本机真实 API 测试 5/5 通过
- [x] **Linux 原生 watcher**（inotify）：递归补挂 watch、`Q_OVERFLOW` 自愈、
      唤醒文件优雅退出（`.cm-wake.cmtmp`，扫描自动过滤）
- [x] **macOS**：暂用轮询（FSEvents 待 M2+），`build_watcher` 工厂自动回退
- [x] **内容校验通道**：每 N 轮按 `content_check` 时间戳轮转哈希校验一批文件，
      比对本地内容与远端 SHA1 —— **正式关闭"同秒同大小修改"盲区**
      （有专门的盲区回归测试与"关闭校验"行为测试）
- [x] **重命名检测（零传输）**：本地改名 → 远端同步 rename/move（不重传内容）；
      远端改名 → 本地同步改名（不重下载）。配对条件保守（需有同步记录 + 大小相等），
      执行前**哈希二次确认**，不匹配自动回退为安全路径（上传新路径+删旧远端 /
      回收站旧文件+下载新文件）
      —— 修复默认保守策略下"一次重命名变成文件重复"的正确性问题
- [x] 快照 schema v0.2：`content_check` 列（含旧库自动迁移）
- [x] 测试：131 个全过（65 sync 单元 + 27 端到端 + 39 其它），
      含原生 watcher 与引擎协同的真实 API 测试

## M2 · Windows 挂载（4–6 周）

- [x] **CfAPI 后端基座**（`cloudmount-vfs::backends::cfapi`，2026-10-12 完成）：
  - SyncRoot 注册/注销（幂等）—— 真机 cldflt 验证通过
  - 占位符创建：自动补全中间目录、目录加 `DISABLE_ON_DEMAND_POPULATION`、
    `RelativeFileName` 不含分隔符（按父目录分组逐目录创建）
  - `CfConnectSyncRoot` + FETCH_DATA/CANCEL_FETCH_DATA 回调
    （callbackcontext 携带上下文，FileIdentity 存相对路径）
  - 水合完成：**`CF_OPERATION_TYPE_TRANSFER_DATA`**（对齐官方 CloudMirror 样例；
    `RETRIEVE_DATA` 不用于完成 FETCH_DATA，会被以 INVALID_REQUEST 拒绝）；
    失败时以 `STATUS_UNSUCCESSFUL` 明确结束读取（不再等 60s 超时）
  - **金标准测试**：注册→占位符→连接→读文件触发 FETCH_DATA→水合→内容校验，
    本机真实 cldflt 上 5/5 通过（含嵌套子目录）
- [x] **CfapiBackend（MountBackend 实现，2026-10-12）**：投影（远端视图→占位符）
      + 水合（回调 → `Provider.open_range`，即 downurl+Range 模式）+ 增量投影
      （`project()`：新文件出现→创建占位符→可水合；幂等）。
      端到端测试用 MockRemote 当"云端"走完整链路——拿到 AppID 后换
      `Open115Provider` 即为真实 115。~~`project` 的已知边界~~
      **已解决（M2.5，2026-10-13）**：内容变化重投影 + dehydrate 均已实现（见下）
- [x] **PlaceholderSync 协调器（2026-10-12）**：远端视图变化 → 本地视图同步
      （新增投影 / 删除传播含嵌套子树 / 内容变化计数）。
      端到端测试：云端增删 → apply → 本地占位符同步 + 新文件可水合
- [x] **daemon 接线完成（2026-10-13）**：`cloudmountd --placeholder-mount`
      （RemoteViewTracker：probe 短路 + 周期全量安全网 → PlaceholderSync 应用）。
      **跨进程全链路已在本机真机验证**：daemon 挂载后，其它进程（cmd/Explorer 级）
      能看到占位符、读文件触发按需水合。`--mock`（feature）提供免凭据自检通道
- [x] **M2.5：内容变化重投影 + dehydrate + 新本地目录上传（2026-10-13）**：
  - 内容变化：`CfUpdatePlaceholder` + `DEHYDRATE`（新元数据生效、陈旧本地数据
    丢弃、下次读取水合新内容）；**冲突保护**：本地 mtime ≠ 旧远端 mtime
    （用户修改未上传）→ 跳过计冲突，绝不覆盖用户数据
  - dehydrate 释放空间：`CfDehydratePlaceholder` 整文件脱水（共享冲突明确报错）
  - 新本地目录上传：`ensure_remote_dir` 逐级创建远端目录链（视图/投影集合同步）
  - e2e（真机 cldflt）：重投影后读到新内容；本地已修改→用户数据保留；
    嵌套新目录+文件全部上传
- [x] **踩坑：Population 策略必须 ALWAYS_FULL(3)**——FULL(2) 下目录条目需
      提供程序信号"填充完成"才对其它进程可见（表现为 dir 看不到文件 +
      跨进程读取报"提供程序未运行"；同进程读却正常，极具迷惑性）
- [x] **双向写路径完成（2026-10-13）**：`LocalChangeUploader`（RDC 事件 →
      三路判定 → 上传 → CfSetInSyncState 绿勾）。
      判定表：Created+已投影=我们的投影跳过；Created+未投影=用户新建上传；
      Modified+水合记录/远端 mtime 匹配=过滤器写入跳过（**实测 Win11 内核态
      水合写入根本不产生 RDC 事件**，双 mtime 判定作为跨版本防御）；
      Modified+不匹配=用户修改上传；Removed=保守不上传。
      e2e（真机 cldflt）：水合零上传 / 用户修改上传 / 新建上传 全过。
      daemon 循环已接入（先本地→远端，再远端→视图）
- [x] **内容变化重投影（2026-10-13）**：`CfUpdatePlaceholder` + `DEHYDRATE`
      （新元数据生效 + 陈旧本地数据丢弃，下次读取水合新内容）。
      **冲突保护**：本地 mtime ≠ 旧远端 mtime（用户修改未上传）→ 跳过重投影
      计入冲突（绝不覆盖用户数据）。e2e：远端变→重投影→读到新内容；
      本地已修改→用户数据保留。
- [x] **Tauri 壳（2026-10-13）**：托盘（状态菜单：打开/暂停恢复/打开目录/退出，
      Linux 菜单文本更新兼容）+ 关窗隐藏到托盘 + 引擎线程（SyncEngine 循环，
      暂停 AtomicBool + 状态快照共享）+ 命令面（get_status/set_sync_root/
      toggle_pause/open_sync_folder，与 web-ui api.ts 契约对齐）+ 应用图标
      （scripts/gen_icons.py 纯 stdlib 生成）。cargo check 通过。
- [x] **应用一致性目录防护（2026-10-13）**：默认排除 `.git`/`.hg`/`.svn`/
      `@eaDir`/`#recycle` 目录与 `.DS_Store`/`Thumbs.db`/`~$*`/`.tmp` 等垃圾
      文件（`exclude.rs`，四处过滤：本地扫描/RDC 事件/inotify 事件/远端拉取）。
      `.git` 纯本地（git 自带同步协议，文件层同步它=仓库损坏——OneDrive 的
      结构性翻车）；远端已有 `.git` 也不拉取（本地权威）。测试覆盖：
      扫描不可见 / 引擎不传不拉 / 占位符不投影 / .git 写入零上传。
      强制同步排除项的规则（静默期门控+.git 内禁冲突副本）设计存档于
      exclude.rs 模块文档（M2.5）
- [x] **Tauri 壳（2026-10-13 真机 GUI 手测通过）**：托盘（菜单/关窗隐藏/
      暂停恢复文本切换）+ web-ui 集成（内嵌 frontendDist，内存 23.8MB）。
      手测修复两个 bug：① custom-protocol feature 缺失（裸 cargo build
      的 exe 加载 devUrl 白窗）；② StatusSnapshot 序列化 camelCase 契约
      （队列 ↑undefined / lastMessage 丢失）。
- [x] **资源管理器状态图标与右键动词（2026-10-13 真机验证）**：导航栏
      'CloudMount 云挂'（与 OneDrive 并排）；状态列'在此设备上可用'/
      '联机时可用'；右键原生动词'释放空间'+'始终保留在此设备上'，
      释放空间实测脱水成功（OFFLINE 置位、再读内容正确回）。
- [ ] MSIX 打包（可选：GitHub 分发 MSI+NSIS 已覆盖，MSIX 仅 Store 渠道需要）
- **验收**： explorer 里打开 115 目录即用，行为与 OneDrive 一致

## M3 · Cookie 补充通道（2–3 周）

- [x] **M3 v1（2026-10-13）：扫码换 Cookie + 只读驱动**——扫码登录
      （qrcodeapi token → 短轮询 → passportapi 换 Cookie，真实 API 实测）；
      独立容器存储（与官方 token 物理隔离）；知情同意流程
      （login --with-cookie-channel，四条风险逐项列出）；只读失效探测
      （my.115.com status 不踢线）；m115 协议加密（下载直链必需，
      移植 115driver）；list/stat/open_range/quota 只读能力；
      `cloudmountd --channel cookie` 挂载读取**不等 AppID 即可用**。
- [x] **M3 v2（2026-10-13）：上传与写操作**——ec115 ECDH 加密
      （P-224 + AES-CBC + LZ4 + token，移植 115driver）；initupload
      秒传/sign_check/OSS 直传（STS + V1 签名 + 回调头）；
      create_dir/rename/move/copy/delete 全套写操作。Cookie 通道
      双向同步完整（真实端到端待扫码验证）。
- [x] **自动回退（2026-10-13）**：ProviderRouter 只对认证类错误切换
      （官方失效→Cookie 兜底 / Cookie 失效→回退官方；频控与网络错误
      不切换），切换 tracing::warn + last_switch 可见（永不静默）；
      daemon `--channel auto`（默认）双通道路由。
- [ ] Cookie 独有能力：分享/转存/批量操作（二级入口 + 常驻风险提示）
- **验收**：拔掉官方 token 能无感切到 Cookie 并回退；无静默行为

## M4 · 边缘网关（2–3 周）

- [x] **WebDAV 出口 + web-ui 托管（2026-10-13，curl 实测）**：
      `cloudmountd --serve`——/dav/ 走 ProviderFs（路径↔ID 解析+TTL
      缓存、PUT spool 上传、MKCOL/DELETE/MOVE/COPY）；Basic 认证；
      web-ui 静态托管（穿越防护）；--mock 自检。4 项集成测试 +
      curl 实测（OPTIONS/GET/PUT/回读/PROPFIND）双平台通过。
      v1 为 HTTP（TLS 见后续）。
- [x] **musl 静态交叉 + TLS + 打包（2026-10-13）**：CI 双目标
      （aarch64/x86_64 musl，含 daemon+CLI+冒烟）；**rustls TLS**
      （--tls-cert/--tls-key，自签证书 curl 实测 HTTPS）；systemd
      unit（内存上限+安全加固）；Dockerfile 多架构 + release workflow
      （GHCR）；docs/edge.md 部署手册（低内存预设/Tailscale/安全）。
- **验收**：512MB 内存设备稳定运行 48h；局域网设备经 WebDAV 挂载读写

## M5 · 打磨（持续）

- [x] **缓存可视化/设置页/日志导出（2026-10-13，42ab2c9）**：web-ui
      完整化（标签页 + 占用/命中率进度条 + 一键清理 + 日志 Blob 导出，
      零框架 2.48KB gzip）；Tauri 壳 8 命令（引擎 Arc 共享 + 日志环形
      缓冲）；daemon serve /api/* REST 同名同参（同一 UI 双宿主）。
- [x] **万级投影基准（2026-10-13 真机）**：10,100 条目 **618ms
      （16,330 条/秒）**——CfCreatePlaceholders 每父目录一次内核批量
      调用已足够，无需优化。
- [ ] macOS File Provider 扩展（Swift 薄桥 + core staticlib）
- [x] **并发水合（2026-10-13，0fb1c4e）**：CDN 限速与 API 拆分
      （8 并发流）——原共用 1 r/s 限速器把多文件读取串行化；
      实测 4 并行下载 0.31s（修复前 ≥ 3s）。

## 长期 · 自研 Windows minifilter（drivers/win）

**实现方案已定稿**：[RUST-MINIFILTER-DESIGN.md](../drivers/win/RUST-MINIFILTER-DESIGN.md)
—— 用 Rust 从零实现（cdylib 直出 `.sys`，手写 FltMgr 绑定层，无 C/C++ 桩），
含 14 项稳定机制与 S1–S6 门禁、KM-0…KM-7 里程碑（18–30 人月，验证占半）。

**2026-10-27 进展**：KM-0 已通过 PR #1 合入 `origin/main`（空 filter、Native、
入口 `DriverEntry`、仅导入 `FLTMGR.SYS`）。KM-1 在特性分支 `feat/cmflt-km1-port`
实现通信端口 + `Hello`/`HelloAck`/`Health` + 用户态 `cmflt-client`：driver 6,144 字节，
导入 `FLTMGR.SYS` + `NTOSKRNL.EXE`，无 FP/SSE；宿主 52 项测试通过。
加载/卸载与真机握手待 testsigning 或 VM 验证。

关键结论：不再走"C 内核 + Rust 用户态"；官方两条红线（HSM 组内不得对 recall 文件
做 targeted 缓存读写、必须处理 section 同步钩子）已写入设计；零售分发需过
HLK/WHCP（2026 签名政策收紧）。**不阻塞任何产品里程碑。**

## 里程碑依赖

```
M0 ──→ M1 ──→ M2（Windows 首发）──→ M3 ──→ M5
              └──→ M4（边缘与 M2 可并行）
```
