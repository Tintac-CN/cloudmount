# PRD: audit-roadmap-next-steps

> 产出者：pm　日期：2026-10-16　来源：intent.md
> 修订 r1（2026-10-16，gate1 打回）：人类裁决①「打回修订」②交付范围=审核+路线图 ③证据标准=**全量验证**（覆盖 decisions.md A1 缺省）；互审发现清单 `reviews/gate1-ai.md` @ be5b021（pm 点名项 M2/M3/M4/M5、MINOR m1–m5 本轮闭合）。

## 用户故事
| # | 角色 | 目标 | 价值 | 溯源 |
|---|---|---|---|---|
| U1 | 项目所有者 | 核验 `docs/ROADMAP.md` 中 M0–M5 与「长期」段的每个 `[x]` 完成声明是否属实 | 路线图恢复可信，杜绝「勾了但没验」腐蚀可信度 | intent.md「问题与期望」要解决的问题第 1 点 |
| U2 | 项目所有者 | 找出计划/架构文档字面与仓库实现现状的偏离，并给出回写建议 | 掌握真实能力边界，文档与现实对齐 | intent.md「问题与期望」要解决的问题第 1 点（已见苗头） |
| U3 | 项目所有者 | 把路线图开放项与四个已完结 intent 的 P2 backlog 归集为统一清单 | 散落项统一可见、可排序，避免遗忘 | intent.md「问题与期望」要解决的问题第 2、3 点 |
| U4 | 项目所有者 | 盘清质量与交付面现状（测试、五断言脚本、三 CI 工作流、已知边界） | 掌握交付可信度与残留风险 | intent.md「受影响系统/用户」（CI 三工作流、五断言脚本）+ decisions.md A1 拆分预览④（277 测试） |
| U5 | 项目所有者 | 得到按依赖/风险/价值排序的下一阶段路线图 | 可据此逐个开新 intent 推进实现 | intent.md「一句话意图」+「期望的结果」 |

## 验收标准（可测）

> 本 intent 为审核+规划类：每条 AC 都是**必须回答且有证据要求**的审核问题（Given 审核对象/范围、When 执行什么核验、Then 产出什么可判定的结论/证据形态）。证据形态统一为可复现材料：`文件路径:行`、命令、输出片段；禁止空泛评价。
>
> **证据标准（gate1 人类裁决③，覆盖 A1 缺省）＝全量验证**：对 M0–M5 段逐 `[x]` 条目、逐条目内子声称落证据；方法与判据不变，只是覆盖面从抽样扩为全量。
>
> **判定口径双轨定义（两者并用、互不替代）**：
> - **真实性三态**（声明是否属实）：已证实（证据支持声明）／漂移（声明与实现现状不符）／不可证（无法取得证据）。
> - **严重度分级**（发现的影响）：P0＝关键声明失实或阻塞下一步推进；P1＝文档-实现漂移需回写；P2＝记录性偏差/低影响。
>
> **交付载体（gate1 M5 钉死）**：审核报告主体为单文件 `docs/aidlc/audit-roadmap-next-steps/audit-findings.md`（沿 audit-p0p1-fixes 先例）；**开放项/backlog 归集表与下一阶段路线图均为报告内章节（§3/§5），不另立独立文件**（归集表需与 §1/§2 发现交叉引用，同文件锚点最稳、避免多载体漂移）。章节骨架 §0–§6 与机械完整性判据见 AC15。

- **AC1**（U1，抽证方法与判定口径基线）：Given `docs/ROADMAP.md` M0–M5 段全部 `[x]` 完成声明（含条目内子声称）与「长期」段声明（该段无 checkbox，核对象为设计定稿结论与「不阻塞任何产品里程碑」等文字声明）；When 按全量验证标准逐条目+逐子声称执行核验（对照代码/测试/CI/命令输出）；Then 每条给出真实性三态判定〔已证实／漂移／不可证〕+ 严重度分级〔P0/P1/P2，定义见头注〕+ 可复现证据（文件路径:行、命令、输出片段），不得以转述或估算替代。
- **AC2**（U1，M0 段）：Given M0 `[x]` 项（`cloudmount-cli login` PKCE 设备码扫码、凭据持久化、token 自动刷新、`ls`/`quota`、`upload` 秒传链路、`download` Range 流式、全局限速+指数退避+频控码识别）；When 全量核验（逐条目+子声称）；Then 每项三态判定 + 严重度 + 证据，并标注 M0 **两个**未勾项的阻塞状态：「申请 115 开放平台个人开发者 + 创建 PKCE 应用（外部审核，立即启动）」与「待 AppID 实测：真实账号跑通全链路」。
- **AC3**（U1，M1 + M1.5 段）：Given M1 段（SQLite 元数据库、`LocalWatcher`/`PollingWatcher`、`change_probe`、`reconcile::plan`、队列调度、块缓存、执行器、`cloudmountd`、**114 个测试**声明、已知边界「同秒漏检」）与 M1.5 段（Windows RDC 原生 watcher、Linux inotify、macOS 暂轮询、内容校验通道关闭盲区、重命名零传输、schema v0.2、**131 个测试**声明）；When 全量核验（含测试计数声明与实际 `cargo test` 现状对照）；Then 每项三态判定 + 严重度 + 证据，并明确「同秒内等大小修改漏检」盲区是否已被 M1.5 内容校验通道正式关闭。
- **AC4**（U1，M2 段，含段内 M2.5 条目）：Given `docs/ROADMAP.md` M2 段（段内条目含 M2.5「内容变化重投影 + dehydrate + 新本地目录上传」——M2.5 为段内条目而非独立里程碑段）逐条声明（CfAPI 后端基座、`CfapiBackend`、`PlaceholderSync`、daemon `--placeholder-mount` 接线、Population 策略 `ALWAYS_FULL(3)`、双向写路径 `LocalChangeUploader`、两处「Tauri 壳」声明、应用一致性目录防护、资源管理器状态图标与右键动词）与未勾的「MSIX 打包」；When 全量核验（**特别对照「Tauri 壳」字面与实际 `apps/desktop` 技术栈**）；Then 每项三态判定 + 严重度 + 证据，并对「Tauri 壳」声明给出漂移结论。
- **AC5**（U1，M3 段）：Given M3 段（v1 扫码换 Cookie+只读驱动+`--channel cookie` 不等 AppID 可挂载、v2 上传写操作全套、自动回退 `ProviderRouter`、未勾的「Cookie 独有能力：分享/转存/批量操作」）；When 全量核验；Then 每项三态判定 + 严重度 + 证据，并标注「真实端到端待扫码验证」的当前状态。
- **AC6**（U1，M4 段）：Given M4 段（WebDAV 出口 + web-ui 托管 curl 实测、musl 静态交叉 + rustls TLS + 打包 + systemd unit + Dockerfile + release workflow + `docs/edge.md`）；When 全量核验；Then 每项三态判定 + 严重度 + 证据，并标注验收指标「512MB 内存设备稳定运行 48h」的达成状态。
- **AC7**（U1，M5 段）：Given M5 段（缓存可视化/设置页/日志导出、万级投影基准 618ms（16,330 条/秒）、未勾的「macOS File Provider 扩展」、并发水合 8 并发流）；When 全量核验；Then 每项三态判定 + 严重度 + 证据。
- **AC8**（U2，已见苗头偏离核验）：Given intent 已列四项偏离苗头（ROADMAP M2/M5 写「Tauri 壳」而 `apps/desktop` 实为 egui/eframe 壳、`apps/shell-macos` 为占位 README、`.gitignore`/`scripts/gen_icons.py` 残留 `src-tauri` 死路径、`web-ui` 角色变迁未回写）；When 逐项核验实际仓库状态；Then 每条产出四栏记录「文档/路线图字面｜实际实现｜证据｜建议回写位置」，并给出确认为真/为假的明确结论。
- **AC9**（U2，偏离面扩展扫描）：Given `docs/ARCHITECTURE.md`（v0.3）、`docs/design/2026-10-12-exploration-v0.2.md`、`docs/edge.md`、各 README 的字面现状，与顶层结构（workspace 七 crate、`apps/desktop`、`apps/shell-macos`、`edge/`、`tools/bin`、`drivers/win`、`web-ui`）；When 扫描计划/架构文档字面与实现现状之间的其它偏离；Then 产出偏离清单（新增偏离逐条带证据；若确无则显式声明本档扫描未发现新增偏离）。
- **AC10**（U3，路线图开放项归集）：Given 路线图全部未勾项与长期声明——M0「申请 115 开放平台个人开发者 + 创建 PKCE 应用（外部审核，立即启动）」（**外部阻塞链源头**）、M0「待 AppID 实测」、M1「待真实凭据联调」「验收指标：空闲 RSS ≤ 20MB」、M2「MSIX 打包」、M3「Cookie 独有能力」、M5「macOS File Provider 扩展」、长期段「自研 Windows minifilter」（无 checkbox，作为长期开放项归集）；When 逐项归集；Then 每项标注依赖源、阻塞性质（外部/内部）、可独立开 intent 的最小粒度与证据位置。
- **AC11**（U3，四 intent backlog 归集）：Given 四个已完结 intent 的候选输入——各 `ops-runbook.md` 的「P2 backlog」与「观察项」节 + 各 `retro.md` 的「下个 bolt 要改的」中可执行候选项（**归集输入面明确为以上两类；retro 的过程学习节——什么有效/什么没用/学到的东西——不入归集**，其属过程经验且已沉淀 AGENTS.md/角色约束，非可执行项）；When 归集、去重与分类；Then 产出统一 backlog 清单（每条含来源 intent + 去重标记 + 分类〔代码/文档/CI/发布〕+ 与 AC10 开放项的关联）。
- **AC12**（U4，测试与 CI 现状盘点）：Given 测试现状（runbook/retro 记载 `cargo test --workspace` 277/0、五断言脚本计数 112/72/23/13/9）与三 CI 工作流（`.github/workflows/ci.yml`、`check-macos.yml`、`release.yml`）与 `apps/desktop` 处于根 `Cargo.toml` `exclude` 的事实；When 全量核验测试计数与 CI 覆盖现状；Then 给出覆盖现状结论 + 已知缺口清单（如根级 fmt/clippy/test 对 workspace-exclude 的 `apps/desktop` 零效力、CI 无 actionlint 步骤、desktop 4 测试 CI 不可见），每条带证据。
- **AC13**（U4，已知边界与交付面盘点）：Given 仓库已知边界（M1.5 声称已关闭的「同秒漏检」、mac 产物未 Developer ID 签名/arm64 ad-hoc、dmg aarch64-only、WiX 硬锁 3.14.1.20250415、`macos-15` runner 依赖、Cookie 通道条款风险、WebDAV v1 无内置 TLS/无速率限制配置）；When 逐项盘点；Then 每条给出当前状态 + 残留风险 + 是否已被后续里程碑消解，带证据。
- **AC14**（U5，下一阶段路线图）：Given AC1–AC13 的审核结论与四个候选头号方向（M0 AppID 实测及其外部阻塞链源头 L8、macOS File Provider、Cookie 独有能力、backlog 清理）；When 按依赖/风险/价值三维排序；Then 产出下一阶段推进计划，以排序批次表呈现（固定列：批次／方向／依赖／风险／价值／建议 intent 边界）+ 批次间依赖关系，使项目所有者可据此逐个开新 intent。
- **AC15**（U5，交付载体与报告完整性判据）：Given 交付载体钉死为单文件 `docs/aidlc/audit-roadmap-next-steps/audit-findings.md`，章节骨架七章——§0 执行摘要（三态×严重度计数矩阵 + 头号方向建议一句话）；§1 路线图断言核验（AC1–AC7 逐条判定表）；§2 计划-实现偏离清单（AC8–AC9 四栏记录表）；§3 开放项与 backlog 归集（AC10–AC11 归集表，含来源/去重/分类列）；§4 质量与交付面体检（AC12–AC13）；§5 下一阶段路线图（AC14 排序批次表）；§6 证据索引（文件路径:行 + 命令 + 输出片段）；When gate2 对照本骨架机械核查；Then **同时满足以下五项才判「报告完成」，任一不满足即「报告未完成」**：①§0–§6 七章齐备且非空；②AC1–AC13 每条 AC 在 §1–§4 有可按 AC 编号定位的落点；③每条非「已证实」判定与每条 P0/P1 发现均有 §6 可解析证据行；④§5 含排序批次表与批次间依赖；⑤§0 计数矩阵与各章明细行数一致。

## 非目标（明确排除）
- **审核期零写入（gate1 裁决②范围含义）**：本期不回写**任何**仓库文件（含产品代码、文档与 `.gitignore`）——所有回写建议只落在审核报告 `audit-findings.md` 内，待后续 intent 执行；不为「顺手修正」破坏审核自身的 git 证据链。
- **不发明路线图未列的需求**：任何能力不得擅自纳入下一阶段（intent.md 约束）。
- **不做性能/安全专项审计**：仅做现状盘点；深层性能/安全专项留给后续独立 intent。
- **不降级证据标准**：证据标准已按 gate1 人类裁决③定为**全量验证**（逐条目+子声称），施工中不得缩减为抽样或仅文档一致性审。
- **不改已完结 intent 的原始工件**：`audit-p0p1-fixes`/`macos-release`/`installer-packages`/`desktop-installers` 的 intent/state/工件只读引用，回流只写本新 intent。

## 开放问题（需人类裁决）
- [x] **本期交付范围**：已裁决（gate1 2026-10-16 人类裁决②）＝**审核+路线图**（确认 decisions.md A1 默认；实现推进随后续 intent）。
- [x] **证据标准**：已裁决（gate1 2026-10-16 人类裁决③）＝**全量验证**（覆盖 decisions.md A1 缺省的抽样验证；逐条目+子声称落证据）。
- [ ] **下一阶段头号方向**：M0 AppID 实测（外部阻塞项，等审核现状后定）／macOS File Provider（M5 未勾）／Cookie 独有能力（M3 未勾）／backlog 批量清理——依赖审核结论与人类偏好。
- [ ] **route 缺省 classic 是否认可（待 gate1 前复核）**：classic→express 降级被协议禁止；本期缺省取 classic 即锁定全量路径，若偏好 express 须以人类裁决留痕切换（gate1 r1 裁决未涉及 route，仍开放）。
