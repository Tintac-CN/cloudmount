# PRD: b1-doc-corrections

> 产出者：pm　日期：2026-10-16　来源：intent.md

## 用户故事
| # | 角色 | 目标 | 价值 | 溯源（intent.md） |
|---|---|---|---|---|
| U1 | 新读者/贡献者 | 首屏 README/ROADMAP/ARCHITECTURE/web-ui 文档对桌面壳的描述与实现一致（egui/eframe 0.31+wgpu+tray-icon 0.21，无 WebView），并拿到**可执行**的桌面壳开发命令 | 消除「Tauri 壳」误导与 `cargo tauri dev` 不可执行死命令，首屏可信 | 「问题与期望」§1（P1×3 首屏漂移）；「期望的结果」16 处回写+死命令改实际方式 |
| U2 | 发布/镜像维护者 | 根 Cargo.toml `repository` 指向真实 owner Tintac-CN（与 apps/desktop/Cargo.toml:7、git remote、release.yml GHCR 镜像面一致） | 发布/镜像引用一致（ghcr.io/tintac-cn/cloudmount），元数据不错侧 | 「问题与期望」owner 元数据错侧；审核 §2.2 NEW-5 |
| U3 | 仓库卫生维护者 | .gitignore 不含 src-tauri 死路径；webdav.rs 与 ROADMAP 的 TLS 表述与 M4 已落地事实（rustls 可选，--tls-cert/--tls-key）一致 | 配置面/注释面可信，不误导后续开发 | 「问题与期望」死路径清理+TLS 注释订正；审核 §2.1③/§2.2 NEW-2/NEW-3 |
| U4 | 后续意图执行者/审阅者 | 回写可机械验证（grep 清零+守护断言+基线不变），回潮可检出 | 变更零风险证明、防回潮有据 | 「约束」五断言脚本与 277/0 基线不受影响 |

## 验收标准（可测）

> **回写措辞约束（适用于全部 AC，构造与互审共用）**：
> 1. **事实订正非删除**——ROADMAP 三处条目保留 `[x]` 完成语义与既有功能子声称（托盘/关窗隐藏/引擎线程/状态快照/应用图标/真机手测记录等），仅订正命名与技术栈表述（Tauri→egui/eframe；frontendDist→无）；不删条目、不改勾选状态。
> 2. **不发明新能力表述**——回写后措辞不得声称任何当前不存在的能力（如桌面 IPC 命令层、双宿主、WebView 集成）；规划图（ARCHITECTURE macOS 段）订正为不与 egui 事实矛盾的表述。
> 3. **触碰面最小化**——webdav.rs 仅注释行（`//`/`///`/`//!`）；api.ts 仅 :2-4 头注；根 Cargo.toml 仅 :12 注释与 :20 repository 字段（依赖、代码、其余字段不动）；不触碰任何历史 AIDLC 工件（`docs/aidlc/**` 内 tauri 字面=审计留痕）；apps/desktop 代码零触碰。
> 4. 整节自洽的最小邻接措辞调整允许（如 web-ui/README.md:3 表头语境），但不得扩出本 AC 清单的事实性新增声明。

- **AC1**（U1，stale Tauri 16 处全量回写）：Given 审核报告 §2.1①+§2.2 NEW-1+§6/TP-18 钉死的 16 处字面（下表），事实基准=audit AS-IS（egui/eframe 0.31+wgpu+rfd+tray-icon 0.21；web-ui 单宿主=daemon `--serve` 托管；desktop=独立 cargo 项目，workspace exclude @ 根 Cargo.toml:13）；When 按下表逐处执行回写；Then 逐处满足「回写后形态」列，且 16 处所在文件的 `grep -in tauri` 零命中（证据形态：grep 输出+回写后行文本摘录，逐处落 plan/workbook）。

  | # | 定位 | 现状字面（锚点） | 回写后形态要求 |
  |---|---|---|---|
  | 1 | docs/ROADMAP.md:111 | 「**Tauri 壳（2026-10-13）**：托盘…+命令面（get_status/…，与 web-ui api.ts 契约对齐）+应用图标」 | 标题改「桌面壳（egui/eframe）（2026-10-13）」，`[x]` 与功能子声称保留；「命令面…契约对齐」按 §2.1④ 订正为 egui 直连引擎的等价操作面，命令契约承载方=daemon `/api/*`（webdav.rs:925-963，实锚） |
  | 2 | docs/ROADMAP.md:124 | 「**Tauri 壳（…真机 GUI 手测通过）**：托盘…+web-ui 集成（内嵌 frontendDist，内存 23.8MB）」 | 标题改「桌面壳（egui）（…真机 GUI 手测通过）」；「内嵌 frontendDist」订正（egui 无 frontendDist；web-ui 由 daemon `--serve` 托管）；手测声称（23.8MB/两 bug 记录）保留为历史手测留痕 |
  | 3 | docs/ROADMAP.md:176 | 「Tauri 壳 8 命令（引擎 Arc 共享+日志环形缓冲）；daemon serve /api/* REST 同名同参（同一 UI 双宿主）」 | 「Tauri 壳」→「桌面壳（egui）等价能力」（桌面侧无 8 命令 IPC 层，审核 §1.7）；「同一 UI 双宿主」措辞随开放问题 Q2 默认同步为单宿主事实表述（gate1 维持默认则为必达项） |
  | 4 | docs/ARCHITECTURE.md:22 | 「[CloudMount.exe] Tauri 2 桌面壳（web-ui + 托盘 + 平台集成）」 | 改 egui/eframe 桌面壳表述（原生界面+托盘+平台集成；无 WebView、无 web-ui 内嵌）；ASCII 图结构保留 |
  | 5 | docs/ARCHITECTURE.md:48 | 「[宿主 App（Tauri）] ←→ [NSFileProviderReplicatedExtension（Swift 薄桥）]」 | 「Tauri」订正为与 egui 事实不矛盾的宿主表述（如「桌面壳（egui，与 Windows 同栈）」）；M5+ 规划性质与 Swift 薄桥/staticlib 事实（ROADMAP:181 未勾项）不变 |
  | 6 | docs/ARCHITECTURE.md:86-88（§前端复用 :84-90 整节） | 「前端复用（一套 SPA，两种宿主）…platform.ts 运行时探测 Tauri…invoke() 直调…POST /api/{cmd}」 | 整节重写为单宿主事实：web-ui=边缘单宿主（cloudmountd `--serve` 托管，`POST /api/{cmd}`）；桌面=egui 原生 UI（apps/desktop），不经 web-ui；删「运行时探测 Tauri/invoke()」叙述（platform.ts:2-5 头注+hostKind 恒 "web" 为事实锚） |
  | 7 | web-ui/README.md:7 | 表行「Tauri 桌面壳（Win/mac/Linux）｜系统 WebView 内嵌｜invoke() → 同进程 Rust 命令」 | 表订正为单宿主（或仅存边缘网关一行+桌面已切换说明行）；与 :3 表头（「两种宿主」语境，随 Q2 默认）整体自洽 |
  | 8 | web-ui/README.md:10 | 「运行时探测 `__TAURI_INTERNALS__`」 | 改 platform.ts 现事实：保留 fetch 单路径（`POST /api/{cmd}`），hostKind 恒 "web"，无宿主探测分支 |
  | 9 | web-ui/README.md:18 | 「npm run build # 产物 dist/（Tauri 与 cloudmountd 共用）」 | 改「产物 dist/（供 cloudmountd `--serve` 静态托管 / Docker COPY）」 |
  | 10 | web-ui/README.md:23 | 「Tauri 侧实现为 `#[tauri::command]`」 | 改「命令由 cloudmountd 实现为 `POST /api/{cmd}`」（契约锚 webdav.rs:925-963）；「命名与参数必须一致」约束句保留 |
  | 11 | web-ui/src/api.ts:2-4 | 头注「桌面 Tauri 壳与边缘 cloudmountd 都要实现这些命令」 | 头注改：命令由边缘 cloudmountd 实现（`POST /api/{cmd}`）；桌面端已切换为纯 Rust egui 壳（apps/desktop），不经此 API 面——与 platform.ts:2-5 头注事实一致；仅头注块，代码不动 |
  | 12 | README.md:89 | 「一套前端：桌面壳（Tauri 2）与边缘网关共用同一个 web-ui（零框架 2.5KB gzip）」 | 消除 Tauri 与「共用」叙事：桌面壳=egui（apps/desktop）；web-ui=边缘网关管理页（daemon `--serve` 托管）；「零框架 2.5KB gzip」数值字面不动（时点值订正不在本批） |
  | 13 | README.md:96 | 架构图「UI：web-ui（共享 SPA）→ Tauri 桌面壳 / 边缘浏览器管理页」 | 改「egui 桌面壳（apps/desktop）/ 边缘浏览器管理页（web-ui，cloudmountd --serve 托管）」；ASCII 框线对齐保持 |
  | 14 | README.md:133 | 仓库结构「apps/desktop　Tauri 2 桌面壳（托盘 + web-ui）」 | 改「apps/desktop　桌面壳（egui/eframe，托盘；独立 cargo 项目）」（"托盘 + web-ui" 订正——egui 壳无 web-ui 内嵌） |
  | 15 | README.md:151 | `cd apps/desktop && cargo tauri dev` | → **AC2** 单列 |
  | 16 | Cargo.toml:12 | 注释「# Tauri 桌面壳是独立 cargo 项目（依赖重、构建需前端产物），不进 workspace：」 | 注释改「桌面壳（egui/eframe）是独立 cargo 项目，不进 workspace：」；去除「构建需前端产物」（Tauri 专属旧事实，egui 无此前置）；:13 `exclude = ["apps/desktop"]` 行不动 |

- **AC2**（U1，README:151 死命令改实际命令）：Given README.md「开发者」段 :151 现状为 `cd apps/desktop && cargo tauri dev`，而 apps/desktop 无 tauri 依赖与 tauri.conf.json（`ls apps/desktop` 无 src-tauri/，desktop Cargo.toml 零 tauri 命中）、为独立 cargo 项目（根 Cargo.toml:13 exclude）；When 回写该命令；Then 桌面壳命令为当前树可直接执行的实际命令：dev=`cd apps/desktop && cargo run`（审核 NEW-1 建议形态），如需发布构建注记用 `cargo build --release`；证据形态=回写后行文本+`apps/desktop/Cargo.toml` 在位（`[package]` 可解析）；命令行内不含 tauri 字面。

- **AC3**（U2，owner 元数据统一）：Given 根 Cargo.toml:20 `repository = "https://github.com/zhonghaoxi/cloudmount"`，错侧于 apps/desktop/Cargo.toml:7（Tintac-CN/cloudmount）、git remote（Tintac-CN/cloudmount.git）与 release.yml GHCR 镜像面（`${GITHUB_REPOSITORY_OWNER}`=Tintac-CN）；When 回写 :20；Then `repository = "https://github.com/Tintac-CN/cloudmount"`；homepage(:21)/authors(:22) 按 intent 约束「仅注释+repository 字段」不动（边界残留见开放问题 Q4）；证据=修改后行文本；workspace 可正常解析（`cargo metadata`/任意 cargo 命令加载成功）。

- **AC4**（U3，.gitignore 死路径清理）：Given .gitignore:12-14 三行现状（`# Tauri` 段头+`apps/desktop/src-tauri/gen/`+`apps/desktop/src-tauri/target/`），而 apps/desktop 无 src-tauri/ 目录且自带 .gitignore；When 删除该三行（段头与两行死路径同块删除）；Then (a) `.gitignore` 全文无 `src-tauri` 字面；(b) 其余段逐行不变（Rust/`/build/`/Node/Windows driver/本地凭据/IDE/OS/`/tools/`/audit.log 各段保持）；(c) desktop 侧 target/gen 忽略不受影响（apps/desktop/.gitignore 在位）；证据=回写后全文 diff（净删 3 行，其余零改动）。

- **AC5**（U3，TLS 表述收敛订正：webdav.rs:29 + ROADMAP:164，NEW-2+NEW-3）：Given (a) webdav.rs:29 注释现状「HTTP（无 TLS）——LAN 场景；TLS（rustls + 证书管理）见 M4 后续」，而 M4 已落地 rustls 可选 TLS（`--tls-cert/--tls-key` @ daemon main.rs:122-128,611）；(b) ROADMAP:164 同条目「v1 为 HTTP（TLS 见后续）。」与 :167「rustls TLS…已落地」未收敛；When 订正两处；Then (a) webdav.rs:29 改为与 M4 事实一致的表述（如「默认 HTTP（LAN 场景）；TLS 可选（rustls，`--tls-cert/--tls-key`，见 daemon main.rs serve 装配）」），收敛口径=审核 §4.2#7 一句话版本；(b) ROADMAP:164 括号改「（TLS 可选，见下条）」，`[x]` 与条目其余字面不动；(c) 两处均为注释/文档行，零行为变更；证据=回写后行文本+`cargo test -p cloudmount-daemon` mock 腿不受影响（AC6c 覆盖）。

- **AC6**（U4，回写后验证）：
  - **AC6a（钉死面清零）**：Given AC1-AC5 回写完成；When 对 intent 受影响文件清单 8 文件（根 README.md、docs/ROADMAP.md、docs/ARCHITECTURE.md、web-ui/README.md、web-ui/src/api.ts、根 Cargo.toml、.gitignore、webdav.rs）执行 `grep -in tauri`；Then 全部零命中（webdav.rs 含 :694/:850/:874/:893 的清单外注释残留与否由开放问题 Q3 裁决——Q3 未裁前 AC6a 仅要求钉死 16 处+关联面清零，webdav.rs 残留须在 workbook 显式留痕）。
  - **AC6b（全仓清零，前提=Q3 裁决并入）**：When 执行 `grep -rwi tauri`（**词边界必须**——`datauri` 含子串 `tauri`，node_modules 的 DataURI 字面在无词边界模式下海量假阳性；或等价排除 `node_modules/`、`target/`、`web-ui/dist/`、`docs/aidlc/`）扫全仓；Then 命中仅剩历史 AIDLC 工件（docs/aidlc/**）与守护断言自身字面（scripts/desktop-installers-yaml.test.sh:697-698 的 `'src-tauri'` 查找参数）；产品/文档/注释面零残留。
  - **AC6c（守护断言与基线不变）**：When 复跑五断言脚本与工作区基线；Then (a) 五脚本全 exit 0：desktop-installers-yaml 112（actionlint 可用环境 113，环境相关注记照录）/installers 72/release-yaml 23/check-macos 13/package-macos 9——本批相关锚预判：TP-11 负断言（:697-698）守 gen_icons.py（本批不触碰），package-macos.test.sh T4 对 README.md 仅查 tar 清单存在性（内容变更不敏感）；若任一脚本断言引用本批目标文件字面而翻红，即构造范围缺陷须返工；(b) `cargo test --workspace` = 277 passed / 0 failed（基线 277/0 逐数一致）；(c) `cargo fmt --all -- --check` 与 `cargo clippy --workspace --all-targets -- -D warnings` exit 0（webdav.rs/config.rs 若触注释须过 fmt）；(d) 根 Cargo.toml 解析正常（277/0 复跑本身即证）。证据形态=命令输出末行计数留痕于 plan/workbook。

## 非目标（明确排除）
- **零产品逻辑变更**：任何 .rs 触碰仅注释行；api.ts 仅头注；Cargo.toml 仅注释+repository 字段；不动依赖、不动代码、不动 CI/release.yml、不动 apps/desktop。
- **不做历史 AIDLC 工件清理**：`docs/aidlc/**` 内 tauri/src-tauri 字面为审计留痕，一律不触碰。
- **不裁决 web-ui 未来双宿主**：本批仅做「单宿主=daemon serve」的文档事实化；是否恢复双宿主属 architecture.md 开放问题，不在本批。
- **不更新 ROADMAP checkbox 完成语义**：`[x]` 全保留、不新增不删除条目；仅事实表述订正。
- **不做 edge/ vs packaging/ 部署面收敛**（审核 NEW-4）：独立裁决项，未纳入 intent。
- **不做脱水 API 命名注记**（小发现⑤ CfDehydratePlaceholder）：§5.1 B1 行虽提及，intent 未纳入，不入本批。
- **不订正 README 性能时点数值**（2.5KB gzip 实测≈2.29KB、618ms 基准等）：记录性口径，不在本批。
- **不动 homepage/authors 相邻字段**（Cargo.toml:21-22）：intent 约束钉死「仅注释+repository 字段」（边界见 Q4）。

## 开放问题（需人类裁决）
- [ ] **Q1（intent.md 原列；编排者默认待 gate1 过目）**：ROADMAP.md「Tauri 壳」条目回写后其 `[x]` 完成语义保留（egui 壳已完成同等功能）——以「事实订正」形态而非删除条目（编排者默认）。gate1 若否决，则 AC1 第 1-3 项的回写形态需重定（删除/重构条目）。
- [ ] **Q2（intent.md 原列；编排者默认待 gate1 过目）**：web-ui 角色定位表述（单宿主=daemon serve）随回写同步——architecture 开放问题「是否正式降级为边缘单宿主」在本批以文档事实化方式处理（不裁决未来是否恢复双宿主）。gate1 若否决，AC1 第 3/6/7 项与 web-ui/README.md:3 表头的单宿主同步回写需回退为仅订 Tauri 字面。
- [ ] **Q3（pm 新增——范围缺口，实测证据）**：全仓清点发现 **intent 钉死清单之外仍有 6 处 tauri 字面**（均为注释/元数据，同性质零逻辑）：`crates/cloudmount-core/src/config.rs:3`（「三个二进制（cloudmountd / cloudmount / Tauri 壳）共用同一份配置」）、`crates/cloudmount-daemon/src/webdav.rs:694/850/874/893`（「与 Tauri 壳的 StatusSnapshot/AppState 同构」「与 Tauri 命令同名（同参）」）、`web-ui/package.json:6`（description「同一套代码跑在 Tauri 桌面壳与边缘网关浏览器」）。intent「期望的结果」要求「全量 `grep -ri tauri`（排除历史 AIDLC 工件）清零」，但「受影响系统」钉死的 8 文件清单不含上述 6 处——**两节自相矛盾：不并入则全仓清零验收（AC6b）必然失败**。是否将 6 处并入本批（均满足「仅注释行/元数据」触碰约束）？其余面已实测零命中：apps/desktop、apps/shell-macos、docs/edge.md、docs/design/、.github/、packaging/、edge/、scripts/（仅守护断言自身字面）。
- [ ] **Q4（pm 新增——相邻字段边界）**：根 Cargo.toml:21 `homepage` 与 :22 `authors = ["zhonghaoxi"]` 与 :20 repository 同为 D5/NEW-5 根因面（owner 错侧）。intent 约束钉死「Cargo.toml 仅注释+repository 字段」→ 本批不改；是否批准随本批一并统一到 Tintac-CN（3 行同类元数据，改动成本为零），还是留待后续 intent（代价：回写后相邻行仍存 owner 不一致，新读者可见）？
