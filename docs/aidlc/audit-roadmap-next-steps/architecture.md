# Architecture: audit-roadmap-next-steps

> 产出者：architect　日期：2026-10-16　来源：intent.md + prd.md

> **本 artifact 定位**：AS-IS 架构快照 + 偏离定位基线。本期为审核+规划类 intent，**不设计新架构、不实施**。
> 逻辑架构一律按**实际代码**写，并与 `docs/ROADMAP.md` 声称的架构逐条对照；已知偏离候选已核实落表（见 §逻辑架构「计划-实现偏离表」）。
> 注意：`docs/ARCHITECTURE.md`（v0.3）为**规划态**文档，其 Windows/macOS 段落含已漂移表述（Tauri），不作为 AS-IS 依据。

## 逻辑架构

### 七 crate 职责链（workspace，`Cargo.toml:3-11`）

依赖自下而上（→ 表示 "编译期依赖"）：

```
core  ← auth
core  ← driver115
core  ← sync
core  ← vfs
core,auth,driver115,sync,vfs ← cli
core,auth,driver115,sync,vfs ← daemon
(all) ← apps/desktop (独立 cargo 项目，workspace exclude: Cargo.toml:13)
```

| crate | 职责（源码自证） | 关键符号 |
|---|---|---|
| `cloudmount-core` | 平台无关抽象层：`Provider` trait、通用类型、错误模型、115 响应信封解析、路径/配置 | `Provider` trait（provider.rs:38，list/stat/open_range/create_dir/upload/rename/move_entry/copy/delete/quota/change_probe）、`RemoteId/RemoteEntry/ChangeProbe/Quota/RangeSpec/EntryKind`、`Envelope`/`ApiErrorKind`/`classify_code`、`VERSION`/`DEFAULT_USER_AGENT` |
| `cloudmount-auth` | 认证与会话：官方 PKCE 设备码、Cookie 通道、凭据存储 | `open`(OpenApiAuth)、`cookie`(CookieAuth)、`pkce`、`store`(`SecretKind`/`SecretStore`/`SystemStore`/`FileStore`)、`LoginMethod` |
| `cloudmount-driver115` | 实现 `Provider`：两条通道 + 路由 + 传输细节 | `Open115Provider`、`Cookie115Provider`、`ProviderRouter`、`ApiClient`（Bearer/限速/token 重试）、`ec115`(ECDH)、`m115`(协议加密)、`hash`(SHA1/preid)、`ratelimit`(令牌桶) |
| `cloudmount-sync` | 双向同步引擎：发现→对比→调度→执行→状态 | `scan`、`remote`(`RemoteViewTracker`/change_probe)、`reconcile::plan`(纯函数)、`queue`、`exec`、`store`(SQLite)、`cache`(内容块 LRU)、`watcher`(PollingWatcher)、`rdc`(Windows RDC，`#[cfg(windows)]`)、`inotify_watcher`、`exclude`、`engine::SyncEngine`、`state` |
| `cloudmount-vfs` | 挂载层抽象：`MountBackend` trait + 平台后端 + 本地控制通道 | `placeholder`、`backends`(cfapi/cfapi_mount)、`control`(回环 TCP + port file)、`MountBackend` |
| `cloudmount-cli` | 命令行入口 | `main.rs`（login/ls/quota/upload/download/...） |
| `cloudmount-daemon` | 常驻服务 `cloudmountd`，四模式合体 | 同步模式 / `--placeholder-mount` / `service` 子命令(Windows SCM) / `--serve`(WebDAV + web-ui 托管)；`service.rs`(SCM)、`webdav.rs`(WebDAV 出口) |

### 平台外壳与交付面

- **`apps/desktop`——egui/eframe 纯 Rust 桌面壳（非 Tauri、无 WebView）**：`eframe 0.31 + wgpu`、`rfd`、`tray-icon 0.21`、`qrcode`、`raw-window-handle`；同进程链接 `core/auth/driver115/sync`（Cargo.toml:11-27）。UI 参考 ArtCraft brutalist 暗色系；托盘两项「显示主窗口/退出」，关窗=隐藏到托盘；引擎线程与 daemon 同构（`Arc<Mutex<SyncEngine>>` + `AtomicBool` 暂停 + 日志环形缓冲，main.rs:1-14）。**独立 cargo 项目**（`Cargo.toml:13` exclude；根 Cargo.toml:12-13 注：Tauri 桌面壳是独立 cargo 项目——注释本身亦为漂移）。
- **`apps/shell-macos`——占位 README，无代码**：仅 README.md 声明 `NSFileProviderReplicatedExtension` + core 编译为 staticlib(C ABI)/cbindgen 薄桥方案；目录（xcodeproj/FileProviderExt/RustCoreBridge）**待建**。README「状态：占位，计划于 Windows 版（M2）稳定后启动」。
- **`web-ui`——vanilla TS SPA，现为边缘单宿主**：`src/api.ts` 定义 8 命令契约（get_status/get_cache_stats/clear_cache/get_quota/export_logs/set_sync_root/toggle_pause/open_sync_folder）；`src/platform.ts` **只保留浏览器路径**（`hostKind()` 恒返回 `"web"`，`call()` 只走 `POST /api/{cmd}`），头注明示「桌面端已切换为纯 Rust...不再使用本 web-ui」。宿主 = `cloudmountd --serve`（webdav.rs:7-14 架构注释：`/dav/*`→ProviderFs，其余→web-ui 静态托管）。构建产物 `web-ui/dist` 供 daemon/Docker COPY。
- **`edge/`——边缘部署面**：`edge/Dockerfile`（多阶段 rust:alpine→scratch，COPY cloudmountd + web-ui/dist，ENV CLOUDMOUNT_WEB_UI=/web-ui、CLOUDMOUNT_LISTEN=0.0.0.0:8384，EXPOSE 8384）、`edge/systemd/cloudmountd.service`（User=cloudmount，非 root，MemoryMax=256M/TasksMax=128，加固项见安全节）。注：release.yml 实际引用的是 `packaging/{docker,systemd,wix}/`（Dockerfile/systemd/wix/main.wxs）——edge/ 与 packaging/ 并存，属部署面双份布局。
- **`drivers/win/`——设计态，未实现**：`README.md`（状态：设计定稿/待实现）+ `RUST-MINIFILTER-DESIGN.md`（主文档，cdylib 直出 .sys + 自建 FltMgr 绑定 + 薄 C 壳回退，含 14 项机制 / S1–S6 门禁 / KM-0…KM-7，18–30 人月）。`src/filter.c`（**已废弃，历史参考**）、`include/cloudmount_proto.h`（将被 Rust schema 取代）、`cloudmount.inf`（骨架）。
- **`tools/bin/`**：仅 `gh.exe`（`.gitignore:39` `/tools/` 不入库）——本地工具，非架构面。
- **CI（`.github/workflows/`）**：`ci.yml`（job: `rust` 三 OS matrix = fmt/check/clippy/test；`rust-mock-features` 三 OS `--features cloudmount-daemon/mock` 补 webdav 安全集成腿；`web-ui` typecheck+build；`arm-cross` musl 双目标 + x86_64 冒烟）；`check-macos.yml`（**仅 workflow_dispatch**，macos-15 单 runner 双 target `cargo check`）；`release.yml`（tag `v*` 触发：binaries-linux/-windows/-macos + installer-msi-windows(WiX v3 硬锁 3.14.1.20250415) + installer-pkg-macos + installer-dmg-macos + release + docker GHCR 多架构）。

### 计划-实现偏离表（AS-IS vs `docs/ROADMAP.md` 声称，逐条已核实）

| # | 声称（路线图/文档，含定位） | 代码实际（证据） | 判定 |
|---|---|---|---|
| D1 | ROADMAP M2「Tauri 壳（2026-10-13）」两处（ROADMAP.md:111-115、124-128）、M5「Tauri 壳 8 命令」（:176-178）；ARCHITECTURE.md §Windows「Tauri 2 桌面壳」(:22)、§macOS「宿主 App（Tauri）(:48)、§前端复用「platform.ts 运行时探测 Tauri/invoke()」(:86-88) | `apps/desktop` = egui/eframe+wgpu+rfd+tray-icon，无 WebView 无 Tauri（Cargo.toml:1-3、13-27；main.rs:1-14）；`web-ui/src/platform.ts` 头注「桌面端已切换为纯 Rust…不再使用本 web-ui」，`hostKind()` 恒 `"web"`，无 Tauri 探测分支 | **已漂移（高可信）**：路线图/架构文档的「Tauri 壳」与实际实现不符 |
| D2 | `.gitignore:12-14` 含 `apps/desktop/src-tauri/gen/`、`apps/desktop/src-tauri/target/` | `apps/desktop` 无 `src-tauri/`（顶层仅 Cargo.toml/build.rs/src/icons/target） | **死路径残留**（desktop-installers backlog 已备案 L13-14） |
| D3 | ROADMAP M5「macOS File Provider 扩展」未勾（:181） | `apps/shell-macos` 仅 README，状态「占位」（与「待做」语义一致） | **一致**（无需纠偏；README 与路线图未勾吻合） |
| D4 | ARCHITECTURE.md §前端复用「一套 SPA 两种宿主，桌面壳内→invoke()」；web-ui/README.md 同述 | `platform.ts` 已单宿主化；daemon `--serve` 托管 web-ui（webdav.rs:7-14）；Dockerfile COPY web-ui/dist；release docker job 构建 web-ui | **角色变迁未回写**（web-ui 从「Win桌面+边缘」降为「边缘单宿主」；文档两处未同步） |
| D5 | 元数据 owner：`apps/desktop/Cargo.toml:7` repository=`Tintac-CN/cloudmount` vs 根 `Cargo.toml:20` repository=`zhonghaoxi/cloudmount` | 两处不一致 | **元数据漂移**（installer-packages backlog 已备案，待下个 intent） |
| D6 | 根 Cargo.toml:12-13 注释「Tauri 桌面壳是独立 cargo 项目」 | 同 D1（实际是 egui 壳） | **注释漂移**（D1 的同一根因） |

> 对照方法可复现：`grep -rn "Tauri" apps/ web-ui/src/ docs/ARCHITECTURE.md docs/ROADMAP.md` 与 `ls apps/desktop` 交叉。

## 数据模型

审核对象相关的持久化面（**本期不新增任何数据模型**）：

| 持久化面 | 位置 | 键/结构 | 约束 |
|---|---|---|---|
| SQLite 同步元数据库（bundled rusqlite） | `cloudmount-sync/src/store.rs` | `meta(k TEXT PRIMARY KEY, ...)`；`entries(rel_path TEXT PRIMARY KEY, remote_id, local_mtime, size, state, pin, updated_at, content_check)`；索引 `idx_entries_remote_id`、`idx_entries_state`（store.rs:56-75） | **snapshot schema v0.2**：`content_check` 列，旧库经 `migrate_v02_content_check` 幂等补列（store.rs:79-99）——路线图 M1.5「快照 schema v0.2」对应此 |
| 内容块缓存 | `cloudmount-sync/src/cache.rs` | LRU + 字节限额 + 命中统计 + 索引持久化 | 下载去重；索引落盘 |
| 本地控制通道 | `cloudmount-vfs/src/control.rs` | 回环 TCP `127.0.0.1:0` + port file `<data_dir>/control.port`（内容 `port pid sync_root`，control.rs:26-68） | 无鉴权，本地信任边界；进程退出清理 port file |
| 凭据存储 | `cloudmount-auth/src/store.rs` | `SecretKind`：`cloudmount/openapi-token`、`cloudmount/cookie-jar`（store.rs:14-26） | 两容器物理隔离；后端 SystemStore(keyring) / FileStore(0600) / MemoryStore |

> 偏离扫描引用锚：schema 版本锚 = v0.2（`content_check`）；控制通道锚 = `.port` 文件格式 `port pid root`。

## 与 PRD 的语义对齐表

> 本期 prd 缺席（审核类 intent，pm 侧为审核报告）；下表按 intent 原话动词（审核/核验/盘点/归集/排序）落到**物理语义**（读什么文件、跑什么、证据形态）。此表是 **qa 设计断言锚的接口**。

| PRD 动词 | 物理语义 | 说明 |
|---|---|---|
| 核验（路线图 `[x]` 声明） | 读 `docs/ROADMAP.md` 里程碑条目 → 提取 claim → 在代码/测试/CI 定位对应证据；证据形态 = 文件路径+行号 / 命令输出 / workflow job 名 | 抽样标准：每里程碑 1-3 条关键声明（intent 开放问题），本期默认抽样 |
| 审核（功能实现，只读） | 只读扫描 `crates/*/src`、`apps/desktop/src`、`edge/`、`drivers/win/`、`web-ui/src`、CI；**不写产品代码/配置** | 只读硬约束（intent 约束节） |
| 盘点（计划-实现偏离） | `diff`（路线图/文档声称架构）vs（Cargo.toml/目录树/源码自证）；证据 = 两侧原文+定位，形如 §逻辑架构 D1–D6 表 | 已见苗头四条 + 本次补核 |
| 归集（backlog） | 抄录四份 `ops-runbook.md` 的「P2 backlog」节条目并去重；证据 = runbook 路径+行号 | 来源：audit-p0p1-fixes(:52-55)、macos-release(:27-33)、installer-packages(:30-38)、desktop-installers(:30-33)；合计约 20+ 项（audit-p0p1 数字为约数，需重新逐条核对源码） |
| 排序（下一阶段） | 按依赖（M0 AppID 外部阻塞优先）/风险/价值排出推进序；证据 = ROADMAP「里程碑依赖」图(:196-201) + 开放项清单 | 排序结论由 pm/qa 审核报告汇入；本表只锚「排序依据可复现」 |
| 留痕（不发明需求） | 仅登记 ROADMAP 已列能力；未列能力不入下一阶段 | intent 约束「不发明需求」 |

## 关键决策与理由

| # | 决策 | 为什么 |
|---|---|---|
| A1 | 本 artifact = **AS-IS 快照 + 偏离定位基线**，不设计新架构 | 本期为审核+规划类 intent，只读核验、不改产品代码；设计新架构=超范围（intent 约束） |
| A2 | 逻辑架构按**实际代码**写，不用 `docs/ARCHITECTURE.md`（v0.3 规划态） | 后者 Windows/macOS/前端段落含已漂移的「Tauri」表述（D1/D4）；AS-IS 必须自证于源码 |
| A3 | 偏离候选**逐条核实并落表**（D1–D6），不只列苗头 | 审核要求证据化（intent）；「勾了但没验」的可信度腐蚀需可定位基线 |
| A4 | 数据模型仅登记**审核对象相关持久化面**（snapshot v0.2、控制通道、凭据容器），不新增实体 | 本期无实现；登记是为偏离扫描提供引用锚 |
| A5 | 语义对齐表按**审核语义**（核验/盘定/归集/排序）写，落到物理动作与证据形态 | 该表是 qa 断言锚的接口——断言需可复现到文件/命令 |
| A6 | 安全节**只做现状盘点**（凭据三分制/知情同意/双通道路由边界），不做新设计 | 本期不实施（intent）；新安全设计应为后续 intent 的 architect 产出 |
| A7 | drivers/win 与 shell-macos 记为**设计态/占位态**，不计入已实现架构 | 二者均无实现代码（drivers/win 有废弃 C 骨架+设计文档；shell-macos 仅 README）；避免把计划当现状 |

## 安全与权限

> 现状盘点，**不做新设计**。每个授权以「主体 + 资源 + 动作」三元组表述。

### 凭据存储（三分制，`cloudmount-auth/src/store.rs`）

- **后端选择**：`default_store()` 桌面端走系统凭据库（Windows 凭据管理器/DPAPI、macOS Keychain、Linux Secret Service）；无头/边缘回退 `FileStore`（`<data_dir>/secrets`，权限 0600，仅 Unix 强制收权限，Windows 依赖用户目录 ACL）——store.rs:4-6、93-141、189-192。
- **容器隔离**：`SecretKind` 两容器物理分离——官方 token 与 Cookie JAR（store.rs:14-26）。**授权三元组**：`进程 + 系统凭据库 entry(cloudmount/openapi-token|cloudmount/cookie-jar) + get/put/delete`。

### 知情同意与通道合规（`cloudmount-auth/src/lib.rs`、`driver115`）

- Cookie 通道**默认关闭**；开启走 `login --with-cookie-channel`，逐条列四条风险（违反 115 服务协议 3.2.3(4)/4.2(11)/5.4 + 挤其它设备会话）——auth/lib.rs:7-9。
- **双通道路由权限边界**（`ProviderRouter`，ROADMAP M3「自动回退」节）：只对**认证类错误**切换（官方失效→Cookie；Cookie 失效→回退官方）；**频控/网络错误不切换**；切换 `tracing::warn` + `last_switch` 可见，**永不静默**。
- Cookie 通道独立 UA + 独立限速 **0.5 r/s**（区别于官方 1 r/s）。
- **授权三元组**：`用户(显式同意) + 115 Cookie 通道(passportapi/my.115.com) + 扫码换 Cookie/读写全套`；`用户 + 115 OpenAPI 资源域 + list/stat/open_range/create_dir/upload/rename/move/copy/delete/quota`。

### 本地与边缘面权限边界

- **本地控制通道**（`vfs/control.rs`）：仅 `127.0.0.1` 回环 TCP + port file——**无鉴权**，属本地信任边界。授权三元组：`本地进程 + 127.0.0.1:{port} + ping/命令`。
- **边缘 WebDAV**（`daemon/webdav.rs`）：`--serve --auth user:pass`（Basic 认证）；TLS 可选（`--tls-cert/--tls-key`，rustls）；**v1 默认 HTTP（LAN 场景）**——Dockerfile ENV `CLOUDMOUNT_LISTEN=0.0.0.0:8384`。授权三元组：`WebDAV 客户端 + /dav/* → ProviderFs + GET/PUT/PROPFIND/MKCOL/DELETE/MOVE/COPY`；`浏览器 + web-ui 静态目录(CLOUDMOUNT_WEB_UI) + 只读`（穿越防护）。
- **Windows 服务**（`daemon/service.rs`）：SCM 注册 `OWN_PROCESS + AutoStart`；授权三元组 `cloudmountd 服务 + --sync-root 目录 + 投影/水合/上传(CfAPI)`。
- **systemd 加固**（`edge/systemd/cloudmountd.service`）：`User=cloudmount`（非 root）、`NoNewPrivileges=yes`、`ProtectSystem=strict`、`ProtectHome=yes`、`PrivateTmp=yes`、`ReadWritePaths=/var/lib/cloudmount`、`MemoryMax=256M`、`TasksMax=128`。
- **长期内核面**（drivers/win，**设计态未实现**）：minifilter 挂 NTFS 卷，拦截 CREATE/READ 触发水合；HSM 组两条官方红线（不在 HSM 组内对 recall 文件做 targeted 缓存读写、必须处理 section sync 钩子）已写入设计——RUST-MINIFILTER-DESIGN.md / drivers/win/README.md。

### 密钥管理现有形态

- 主：系统 keyring（DPAPI/Keychain/Secret Service）；边缘：`<data_dir>/secrets` 0600 文件。
- token/Cookie 分容器（不混存、不互相降级泄漏）。
- **日志脱敏不变量**：任何日志级别不得出现凭据明文（audit-p0p1-fixes F-04 七处脱敏，`cookie_logging` 断言守护；ops-runbook.md:28「出现即安全回归」）。
- 无自建服务端 IAM 面（边缘自托管，公网经 Tailscale/WireGuard，不开公网端口——edge/README.md）。

### 已知边界（诚实记录）

- Windows MSI / macOS pkg / macOS .app(.dmg) **均未 Authenticode / Developer ID 签名**，无公证（release.yml release notes 明示）；首启 Gatekeeper/SmartScreen 拦截为预期行为（右键→打开 / 更多信息→仍要运行 / `xattr -cr`）。
- 边缘 WebDAV v1 为 HTTP（TLS 见后续）。

## 开放问题（需人类裁决）

- [ ] **macOS File Provider 桥接是否影响 core ABI**：`shell-macos` 方案为 core 编译 staticlib(C ABI) + cbindgen/Swift 薄桥。是否需在启动 M5 前**冻结 core 导出 ABI**（否则薄桥随 core 演进反复破坏）？本期不实施，需裁决 ABI 稳定性策略。
- [ ] **minifilter 是否现在预冻结 `MountBackend` trait 接口**：`vfs::backends` 矩阵已把 `CloudMountFilterBackend` 列为「长期目标」，但 trait 现形状为 CfAPI 服务。是否现在预留 minifilter 所需接口面，避免后续破坏性变更？（长期旗舰，不阻塞产品里程碑）
- [ ] **web-ui 是否正式降级为「边缘单宿主」**：`platform.ts` 的 Tauri invoke 路径已死（hostKind 恒 "web"）。是回写 `ARCHITECTURE.md` §前端复用 + `web-ui/README.md` 为单宿主并删宿主抽象，还是保留桌面复用可能（并存双宿主抽象）？此裁决决定 §逻辑架构 D4 的纠偏方向。
- [ ] **ARCHITECTURE.md「Tauri」表述纠偏方向**：「文档纠偏（改 ARCHITECTURE.md/ROADMAP 称 egui）」vs「代码回归（若坚持 Tauri）」——本期只审不改，须人类定方向（牵连 D1/D4/D6）。
- [ ] **repository owner 元数据统一**：根 `Cargo.toml` = `zhonghaoxi` vs `apps/desktop/Cargo.toml` = `Tintac-CN`（D5）。统一到哪一方、是否随下个 intent 落（installer-packages backlog 已备案）。
- [ ] **部署面双份布局**（`edge/` 与 `packaging/`）：二者 Dockerfile/systemd 并存，release.yml 引用的是 `packaging/`。是否合并/说明主从，避免后续维护分叉？
