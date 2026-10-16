# Architecture: b1-doc-corrections

> 产出者：architect　日期：2026-10-16　来源：intent.md + prd.md
>
> **本 artifact 定位 = 回写蓝图**：逐处回写定位表（行锚全部本档 `Read`/`Grep` 实测，非抄审核报告——上 intent gate2 M4 教训）+ AS-IS 统一事实表述基准。dev 据此执行；本文零产品逻辑设计（纯文档/元数据/注释批）。

## 逻辑架构

### 回写面组件与边界

- **回写面（intent 钉死 8 文件）**：根 `README.md`、`docs/ROADMAP.md`、`docs/ARCHITECTURE.md`、`web-ui/README.md`、`web-ui/src/api.ts`（头注）、根 `Cargo.toml`（注释+owner）、`.gitignore`、`crates/cloudmount-daemon/src/webdav.rs:29`（注释）。
- **architect 实测扩展面（gate1 过目，见开放问题③）**：`crates/cloudmount-core/src/config.rs:3` + `webdav.rs:694/850/874/893`——5 处 Tauri 注释字面，审核 16 处清单未覆盖（审核 grep 方法只扫 apps/、web-ui/src、两份文档，未扫 crates/）。**不清这 5 处，intent 验收「全量 `grep -ri tauri`（排除历史 AIDLC 工件）清零」必挂**。全部为 `//`/`///`/`//!` 注释行，符合「零产品逻辑变更」约束类。
- **不回写面（硬边界）**：`docs/aidlc/**` 历史工件（intent 约束：审计留痕）；五断言脚本与三 workflow（零改动，见断言面交叉）；`apps/desktop` 代码（无 tauri 字面，实测零命中）；`docs/ARCHITECTURE.md` 其余规划态内容（v0.3 整体刷新=另一批）；`platform.ts` 代码（已单宿主化，无需改）。

### AS-IS 统一事实表述基准（回写后 16+ 处必须相互一致，dev 照此措辞族落笔）

| # | 统一表述 | 事实锚（实测） |
|---|---|---|
| F1 | **desktop 栈**：`apps/desktop` = 桌面壳（egui/eframe 0.31 + wgpu 渲染 + rfd 文件对话框 + tray-icon 0.21 托盘 + qrcode），**纯 Rust、无 WebView、无 Tauri**；同进程链接 core/auth/driver115/sync；引擎线程 `Arc<Mutex<SyncEngine>>` + AtomicBool 暂停 + 日志环形缓冲；**无 IPC 命令层**（get_status 等 8 命令名仅存在于 daemon `/api/*` 与 api.ts 契约，桌面侧 UI 直连引擎） | `apps/desktop/Cargo.toml:2`（name=cloudmount-desktop）、:11-31（依赖全表）；main.rs 事实=审核 §1.4/AS-IS 快照 D1 实测；`grep -i tauri apps/desktop/src` 零命中（本档实测） |
| F2 | **构建方式**：独立 cargo 项目（根 `Cargo.toml:13` exclude）；构建 `cd apps/desktop && cargo build --release` → 产物 `target/release/cloudmount-desktop.exe`；开发运行 `cargo run`。**README:151 死命令 `cargo tauri dev` 以此替换** | `apps/desktop/Cargo.toml:2` package name；编排者指令以 apps/desktop 现状为准 |
| F3 | **web-ui 单宿主**：唯一宿主 = `cloudmountd --serve`（浏览器 → `POST /api/{cmd}`）；`platform.ts` 已单宿主化（头注「桌面端已切换为纯 Rust（egui + wgpu + rfd，apps/desktop），不再使用本 web-ui」，`hostKind()` 恒 `"web"`）；8 命令契约在 api.ts，daemon 侧实现于 `/api/*`；构建产物 `web-ui/dist` 由 daemon（CLOUDMOUNT_WEB_UI）/Docker COPY 托管。**桌面 UI = egui 原生，不经 web-ui** | `web-ui/src/platform.ts:2-5,9-11`（本档实测）；daemon `/api/*` = 审核 §1.7 实测 webdav.rs:925-963（边缘四命令 501=文档化边界） |
| F4 | **CI 结构**：`ci.yml` 四 job（rust / rust-mock-features / web-ui / arm-cross）；`release.yml` **九 job**（binaries-linux/windows/macos、installer-msi-windows、installer-pkg-macos、installer-dmg-macos、release、docker、merge） | 审核 AS-IS 快照 §CI 段（TP-16 实测，gate1 过）——本批不涉 CI 回写，仅表述基准备查 |
| F5 | **TLS 统一句**（全文唯一口径）：「WebDAV v1 **默认 HTTP**（LAN 场景），TLS 为**可选非默认**——`--tls-cert/--tls-key` 成对传入即 rustls HTTPS（daemon main.rs serve 装配）」 | `crates/cloudmount-daemon/src/main.rs:122-128,611`（审核 NEW-2/§4.2 边界 7 实测）；本档用于 webdav.rs:29 与 ROADMAP:164（候选）两处 |
| F6 | **owner = Tintac-CN**，来源依据四重：① git remote = `git@github.com:Tintac-CN/cloudmount.git`（审核 NEW-5/TP-24 实测）；② `release.yml:307,380` 镜像变量 `ghcr.io/${GITHUB_REPOSITORY_OWNER,,}/cloudmount` 实际解析为 tintac-cn（**本档实测**）；③ `apps/desktop/Cargo.toml:7` = `Tintac-CN/cloudmount`；④ `README.md:11,13` 发布链接均为 Tintac-CN/tintac-cn。根 Cargo.toml `zhonghaoxi` 为**错侧** | 上列行锚 |
| F7 | **workspace 基线不动**：七 crate members + `exclude=["apps/desktop"]`；`cargo test --workspace` = **277/0**（2026-10-16 基线）；五断言脚本 **112/72/23/13/9**。本批零依赖/零代码行为变更，两基线完工后必须逐数复现 | 根 `Cargo.toml:3-13`（本档实测）；基线数值=审核 §4.1/§0 复跑 |

### 逐处回写定位表（行锚=本档实测；dev 按行执行，行号以执行时 grep 复核为准）

**A. 审核 16 处 Tauri 字面（苗头①+NEW-1）**

| # | 定位 | 现状字面（摘要） | 回写后表述要点（按 F1/F2/F3 统一措辞族） |
|---|---|---|---|
| 1 | `README.md:89` | 「**一套前端**：桌面壳（Tauri 2）与边缘网关共用同一个 web-ui（零框架 2.5KB gzip）」 | 改两条事实：「web-ui 管理页：边缘网关浏览器管理页（零框架 2.5KB gzip——**字节数不动**，见决策 D5）」＋「桌面端为 egui 原生 UI（apps/desktop，不使用 web-ui）」 |
| 2 | `README.md:96` | 架构图行「UI：web-ui（共享 SPA）→ Tauri 桌面壳 / 边缘浏览器管理页」 | 改「UI：桌面壳（egui 原生 UI）/ web-ui 管理页（边缘浏览器，cloudmountd --serve 托管）」；**保持 ASCII 图框列宽对齐** |
| 3 | `README.md:133` | 仓库结构「apps/desktop  Tauri 2 桌面壳（托盘 + web-ui）」 | 改「apps/desktop  桌面壳（egui/eframe，托盘 + 原生 UI）」；保持代码块缩进对齐 |
| 4 | `README.md:151` | 「cd apps/desktop && cargo tauri dev」（**不可执行死命令**——apps/desktop 无 tauri 依赖、无 src-tauri/） | 按 F2 改「cd apps/desktop && cargo build --release　# 产物 target/release/cloudmount-desktop.exe（开发调试可 cargo run）」 |
| 5 | `docs/ROADMAP.md:111-115`（字面 @:111；牵连 :114） | 「[x] **Tauri 壳（2026-10-13）**：托盘…＋命令面（get_status/set_sync_root/toggle_pause/open_sync_folder，与 web-ui api.ts 契约对齐）＋应用图标…cargo check 通过」 | **事实订正形态，保留 [x]**：标题改「**桌面壳（egui/eframe，2026-10-13）**」；「命令面（…与 web-ui api.ts 契约对齐）」改「UI 直连引擎（状态快照/暂停/打开目录，`Arc<Mutex<SyncEngine>>` 共享）——api.ts 同名命令契约由 daemon `/api/*` 承载」（F1+D4）；应用图标与 cargo check 通过保留（gen_icons.py 在位且 TP-11 断言守护） |
| 6 | `docs/ROADMAP.md:124-128`（字面 @:124） | 「[x] **Tauri 壳（2026-10-13 真机 GUI 手测通过）**：托盘＋web-ui 集成（内嵌 frontendDist，内存 23.8MB）。手测修复两个 bug：① custom-protocol feature 缺失…；② StatusSnapshot 序列化 camelCase 契约…」 | 标题改「**桌面壳（egui/eframe，2026-10-13 真机 GUI 手测通过）**」；「web-ui 集成（内嵌 frontendDist…）」改「桌面 UI 为 egui 原生（无 WebView、无 frontendDist；web-ui 由 daemon `--serve` 单独托管），内存 23.8MB（真机手测）」（F3）；bug② 落点改注「daemon 侧 StatusSnapshot 序列化契约」（审核 §1.4 佐证）；**bug① custom-protocol 为 Tauri 特有机制、egui 载体无此概念**→建议改历史注记「早期 Tauri 技术路线验证期记录，现 egui 载体不适用」——措辞见开放问题① |
| 7 | `docs/ROADMAP.md:176-177`（字面 @:176） | 「Tauri 壳 8 命令（引擎 Arc 共享＋日志环形缓冲）；daemon serve /api/* REST 同名同参（同一 UI 双宿主）」 | 改「桌面壳（egui）UI 直连引擎（引擎 Arc 共享＋日志环形缓冲）；daemon serve /api/* REST 同名同参（web-ui 单宿主=daemon `--serve` 托管）」——「同一 UI 双宿主」必须一并消失（苗头④） |
| 8 | `docs/ARCHITECTURE.md:22` | 图行「[CloudMount.exe]  Tauri 2 桌面壳（web-ui + 托盘 + 平台集成）」 | 改「[CloudMount.exe]  桌面壳（egui/eframe 纯 Rust：原生 UI + 托盘 + 平台集成）」 |
| 9 | `docs/ARCHITECTURE.md:48` | §macOS 图行「[宿主 App（Tauri）] ←→ [NSFileProviderReplicatedExtension（Swift 薄桥）]」 | 去 Tauri 名：「[宿主 App（M5+，技术未定）] ←→ …」——`apps/shell-macos/README.md:13` 仅泛称「宿主 App」，无技术承诺（本档实测）；M5 未启动，不预写 egui 承诺 |
| 10 | `docs/ARCHITECTURE.md:84-90`（字面 @:86） | §「前端复用（一套 SPA，两种宿主）」：「platform.ts 运行时探测 Tauri：桌面壳内 → invoke()；边缘浏览器 → POST /api/{cmd}」 | 段标题改「## web-ui 管理页（边缘单宿主）」；正文按 F3 重写：唯一宿主=cloudmountd `--serve`（浏览器 → `POST /api/{cmd}`）；契约在 `web-ui/src/api.ts`，daemon 实现 `/api/*`；桌面端为 egui 原生 UI 不使用 web-ui（platform.ts 已单宿主化，hostKind 恒 `"web"`） |
| 11 | `web-ui/README.md:3,5-8`（字面 @:7） | 「同一套代码跑在两种宿主里」＋宿主表 Tauri 行「invoke() → 同进程 Rust 命令」 | 单宿主化：宿主表只留「边缘网关：cloudmountd 直接托管静态文件，`POST /api/*`」一行；补一句「桌面端已切换为 egui 纯 Rust 壳（apps/desktop），不使用本 web-ui」 |
| 12 | `web-ui/README.md:10-11`（字面 @:10） | 「切换逻辑在 platform.ts：运行时探测 `__TAURI_INTERNALS__`」 | 改「platform.ts 为服务端/边缘专用 fetch 单路径（已移除宿主探测，`hostKind()` 恒 `"web"`）」 |
| 13 | `web-ui/README.md:18` | 「产物 dist/（Tauri 与 cloudmountd 共用）」 | 改「产物 dist/（由 cloudmountd `--serve` 托管：CLOUDMOUNT_WEB_UI / Docker COPY）」 |
| 14 | `web-ui/README.md:23-24`（字面 @:23） | 「Tauri 侧实现为 `#[tauri::command]`，cloudmountd 侧实现为 POST /api/{cmd}，命名与参数必须一致」 | 改「命令契约由 cloudmountd `POST /api/{cmd}` 单方实现（daemon webdav.rs `/api/*`），命名与参数以 api.ts 为准」 |
| 15 | `web-ui/src/api.ts:1-6`（字面 @:4） | 头注「桌面 Tauri 壳与边缘 cloudmountd 都要实现这些命令，命名与参数保持一致。当前为骨架；随里程碑逐步填充。」 | 头注重写：「唯一实现方为边缘 cloudmountd（`POST /api/{cmd}`，见 daemon webdav.rs `/api/*`）；桌面端为 egui 原生 UI，不经本契约。命名与参数保持一致。」；同块「当前为骨架」已失实（8/8 命令已实现，审核 §1.7 api.ts:44-80）——**同块搭车订正**为「8 命令已全部实现」，见决策 D7 |
| 16 | 根 `Cargo.toml:12` | 注释「# Tauri 桌面壳是独立 cargo 项目（依赖重、构建需前端产物），不进 workspace：」 | 改「# 桌面壳（egui/eframe）是独立 cargo 项目（GUI 依赖重，与 workspace 依赖图隔离），不进 workspace：」——「构建需前端产物」为 Tauri frontendDist 语义，删除（egui 无此需求，F2） |

**B. 3 项关联订正（intent 钉死）**

| # | 定位 | 现状字面 | 回写后表述要点 |
|---|---|---|---|
| 17 | 根 `Cargo.toml:20` | `repository = "https://github.com/zhonghaoxi/cloudmount"` | 改 `https://github.com/Tintac-CN/cloudmount`（F6 四重依据）。**域注意**：:21 homepage、:22 authors 同含 zhonghaoxi——intent 约束钉「仅注释与 repository 字段」，是否扩展见开放问题④ |
| 18 | `.gitignore:12-14` | 「# Tauri」＋`apps/desktop/src-tauri/gen/`＋`apps/desktop/src-tauri/target/` | **三行整删**（`apps/desktop` 无 src-tauri/ 目录=死路径，本档经审核 D2 确认；:12 注释头属该块，随行删） |
| 19 | `webdav.rs:29` | 「HTTP（无 TLS）——LAN 场景；TLS（rustls + 证书管理）见 M4 后续；」 | 按 F5 统一句改「默认 HTTP——LAN 场景；TLS 可选非默认（`--tls-cert/--tls-key`，rustls，见 daemon main.rs serve 装配）；」——仅注释行 |

**C. architect 新增发现面（超出审核 16 处清单；gate1 过目——不清零则验收挂）**

| # | 定位 | 现状字面（Grep 实测） | 回写后表述要点 |
|---|---|---|---|
| 20 | `crates/cloudmount-core/src/config.rs:3` | 「三个二进制（cloudmountd / cloudmount / Tauri 壳）共用同一份配置」 | 「Tauri 壳」→「桌面壳」；`//!` 模块注释行 |
| 21 | `webdav.rs:694` | 「// 网关状态（/api/* 数据源；与 Tauri 壳的 StatusSnapshot 同构）」 | 「与 Tauri 壳的」→「与桌面壳的」；仅注释行 |
| 22 | `webdav.rs:850` | 「// /api/*：web-ui 的 REST 面（与 Tauri 命令同名）」 | 改「（命令名与 web-ui api.ts 契约同名）」 |
| 23 | `webdav.rs:874` | 「/// 与 Tauri 壳的 AppState 同构：…」 | 「与 Tauri 壳的」→「与桌面壳（egui）的」 |
| 24 | `webdav.rs:893` | 「`/api/*` 处理（与 Tauri 命令同名同参；POST JSON）。」 | 「与 Tauri 命令」→「与 web-ui api.ts 契约命令」 |

**D. 联动候选（audit 建议同批、intent 字面未钉；gate1 逐项勾选，见开放问题⑤）**

| # | 定位 | 现状 | 候选回写 |
|---|---|---|---|
| 25 | `docs/ROADMAP.md:164`（NEW-3） | 「v1 为 HTTP（TLS 见后续）」与同里程碑 :167「rustls TLS 已落地」未收敛 | 按 F5 收敛：「v1 默认 HTTP（TLS 可选，见下条）」——与 #19 共用统一句，避免修了 webdav.rs 仍留文档面过时 TLS 叙事 |
| 26 | `docs/ROADMAP.md:91`（小发现⑤；§5.1 B1 批次行点名「脱水 API 命名注记」） | 「dehydrate 释放空间：`CfDehydratePlaceholder` 整文件脱水」 | 加命名注记：实现在 cfapi.rs 刻意绕开未文档化 `CfDehydratePlaceholder` 导出，用文档化两步路径（CfSetPinState(UNPINNED)+CfUpdatePlaceholder+DEHYDRATE）；能力语义等价 |

**E. 明示不回写（防范围蔓延）**：`docs/aidlc/**`（历史工件）；`docs/ARCHITECTURE.md` 其余规划态内容（含 §边缘设备 FUSE 提法等——v0.3 整体刷新另批）；`README.md:143` 测试计数 214/185（现值 277/0，审核未列入偏离清单——是否搭车见开放问题⑤）；`README.md:89` 与 `ROADMAP.md:175` 的 gzip 字节数（审核 §1.7 判构建时点微差、非偏离，两处保留不动）。

### 断言面交叉（实测结论：回写零断言联动风险）

- `scripts/desktop-installers-yaml.test.sh:697-698`（TP-11 负断言：gen_icons.py 无 `src-tauri` 字面）——本批**不改 gen_icons.py**，断言不受影响、继续为绿。
- 五断言脚本 **无任何** 对 README.md/ROADMAP.md/ARCHITECTURE.md 内容、根 Cargo.toml repository 值、.gitignore 行的断言（Grep 实测：脚本面 `tauri|README|ROADMAP|ARCHITECTURE|gitignore|zhonghaoxi|Tintac|gen_icons|src-tauri` 仅命中上条 gen_icons 负断言；`package-macos*.sh` 的 README.md 引用=打包 tar 占位**文件名**，非根 README 内容）。TP-18/24 面无文档字面断言需同步。
- 完工门（intent 约束）：复跑 `cargo test --workspace`=**277/0**＋五脚本 **112/72/23/13/9**＋`cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings` 双面 clean（desktop 面单独跑：`cd apps/desktop && cargo fmt…/clippy…`——根级命令不触 exclude 成员）。webdav.rs/config.rs 仅注释行，理论上零影响，仍须复跑确认。
- 验收命令（回写后）：`grep -rni tauri` 于产品面（README.md、docs/ROOT 两文档、web-ui/、crates/、apps/、packaging/、edge/、.github/、scripts/、根 Cargo.toml、.gitignore——**排除 docs/aidlc/**）应零命中。

## 数据模型

**N/A——零实体/键/索引/约束变更**。本批无任何持久化面触碰（SQLite schema、缓存索引、控制通道、凭据容器均不动）。

唯一「数据」 touched = **发布元数据**（Cargo package metadata 的 repository 字段，#17）：非运行时数据、非密钥、无 schema。owner 取值 `Tintac-CN` 的来源依据见 F6（git remote＋release.yml 镜像变量＋desktop 侧同字段＋README 发布链接，四源一致）。

## 与 PRD 的语义对齐表

| PRD 动词 | 物理语义 | 说明 |
|---|---|---|
| 订正（stale Tauri→egui 事实表述） | 编辑定位表 #1-16+#20-24 的指定行：文档段落/代码注释字面替换，零行为行（Rust 侧仅 `//`/`///`/`//!` 与 js 头注） | 证据形态=回写后 `grep -rni tauri`（排除 docs/aidlc/**）产品面零命中；`git diff` 仅注释/文档/元数据行 |
| 清理（.gitignore 死路径） | 删除 `.gitignore:12-14` 三行，无新增行 | 证据形态=回写后 `grep -n src-tauri .gitignore` 零命中 |
| 统一（owner 元数据） | 根 `Cargo.toml:20` repository 字段值 `zhonghaoxi`→`Tintac-CN` | 证据形态=`grep -n zhonghaoxi Cargo.toml` 仅剩 :21/:22（若 gate1 维持最小域，见开放问题④）；与 F6 四源一致 |
| 订正（TLS 注释过时） | `webdav.rs:29`（+#25 候选 ROADMAP:164）按 F5 统一句改写，仅注释行 | 证据形态=两处不再含「TLS 见后续/M4 后续」字面；全文 TLS 口径与 §4.2 边界 7 收敛句一致 |
| 验证（基线不动） | 复跑双基线：workspace 277/0＋五脚本 112/72/23/13/9＋fmt/clippy 双面（含 desktop 单独面） | 证据形态=命令输出逐数一致；任何漂移即回归 |

## 关键决策与理由

| # | 决策 | 为什么 |
|---|---|---|
| D1 | 本 artifact = **回写蓝图**（定位表+统一表述基准），不设计新架构；行锚全部本档实测（Read/Grep），不转抄审核行号 | 文档批无架构可设计；gate2 M4 教训=转抄行号漂移——实测后落笔才可执行 |
| D2 | 全文统一称谓「桌面壳（egui/eframe）」，禁止「Tauri 兼容/原 Tauri/（Tauri→egui）」过渡措辞 | 审核 AC4 漂移结论＋F1；过渡措辞保留歧义、grep 清零验收也过不了 |
| D3 | ROADMAP 采用「事实订正」形态保留 `[x]` 条目（非删除） | intent 默认＋功能面在 egui 侧等价存在（审核 §1.4）；删除会丢真机手测/基准历史记录 |
| D4 | 桌面侧「8 命令/命令面/api.ts 契约对齐」类措辞一律改写为「UI 直连引擎（Arc<Mutex<SyncEngine>>）＋契约承载方=daemon `/api/*`」 | egui 壳无 IPC 命令层（审核 §1.7 grep 零命中）；直接改「桌面壳 8 命令」=换名不换实的伪订正 |
| D5 | gzip 字节数（README:89/ROADMAP:175）不动 | 审核 §1.7 判「构建时点值微差」非偏离；动它=范围蔓延且两处需联动改，反引入新矛盾 |
| D6 | owner 修正最小域=repository（:20）；homepage（:21）/authors（:22）同值问题列开放问题④不擅扩 | intent 约束钉死「仅注释与 repository 字段」；扩域须人类过目——但需知会：不改则残留同值不一致 |
| D7 | api.ts 头注块内「当前为骨架」句同块搭车订正 | 同一注释块（:1-6）一次改完；审核 §1.7 证 8/8 已实现——留着=回写后头注自我矛盾 |
| D8 | crates/ 5 处注释字面（#20-24）纳入蓝图并标注为「超出审核清单的新增发现」 | intent 验收=「全量 grep tauri 清零」，不含则必挂；全部注释行、零行为，符合约束类；但超出 intent 8 文件钉死面——须 gate1 过目而非 dev 自行扩 |
| D9 | 断言脚本/三 workflow 零改动；完工门=双基线复跑＋产品面 grep 清零 | intent 约束；实测五脚本无文档字面断言（断言面交叉节），无需联动改 |

## 安全与权限

**N/A——零鉴权/网络/密钥/IAM 面变更。**

- 回写内容全部为文档/注释/元数据字面，不含凭据、端点凭据、token 或网络配置。
- `.gitignore` 删除的是**不存在目录**的死路径（apps/desktop/src-tauri/），无敏感面泄漏或暴露语义变化。
- owner 字段为公开仓库元数据（见数据模型节），不构成权限授予面。
- 无最小权限议题：不触碰 workflow `permissions:`、凭据存储、WebDAV 认证/TLS 行为（webdav.rs:29 仅注释，实际 `--tls-cert/--tls-key` 装配路径不变）。

## 开放问题（需人类裁决）

- [ ] ① **ROADMAP 条目回写形态**：事实订正（intent/本蓝图默认，#5/#6/#7 按此设计）vs 删除条目。若 gate1 选删除需重出三处形态。细化子项：#6 的 bug①（custom-protocol，Tauri 特有机制）在订正形态下建议改历史注记「早期 Tauri 技术路线验证期记录，现 egui 载体不适用」——该措辞请 gate1 过目。
- [ ] ② **web-ui 单宿主事实化边界**：本蓝图按「文档事实化」处理（#10-15 改表述；platform.ts 代码/宿主抽象不动）。「是否正式裁决未来双宿主恢复」不在本批（audit AS-IS 开放问题延续）——确认此边界即可。
- [ ] ③ **crates/ 5 处注释字面是否纳入本批**（#20-24：config.rs:3＋webdav.rs:694/850/874/893）——超出 intent 8 文件钉死清单，但为「grep 清零」验收的必要面，全为注释行零行为。architect 建议纳入；gate1 勾选。
- [ ] ④ **owner 修正域**：仅 `Cargo.toml:20` repository（intent 字面最小域），还是连同 :21 homepage/:22 authors 一并统一 `Tintac-CN`（否则残留同值 zhonghaoxi 不一致）。architect 建议三点一并（同为发布元数据、同一处 diff），但 intent 字面只钉 repository——须裁决。
- [ ] ⑤ **搭车候选逐项勾选**（audit 建议同批、intent 字面未钉）：#25 ROADMAP:164 TLS 收敛（NEW-3，建议纳入——否则修 webdav.rs 后文档仍留过时 TLS 叙事）；#26 ROADMAP:91 脱水 API 命名注记（§5.1 B1 批次行点名）；api.ts 头注「当前为骨架」句（D7，建议随块）；README:143 测试计数 214/185→现值 277/0（审核未列偏离，architect 发现的额外 stale，可留后续批）。
- [ ] ⑥ **docs/ARCHITECTURE.md v0.3 规划态整体刷新**是否另开批（本批只回写 #8-10 三处 Tauri 面；其余规划态内容如 §边缘设备 FUSE、§Windows「当前规划：M2」等未动）。
