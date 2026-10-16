# PRD: audit-roadmap-next-steps

> 产出者：pm　日期：2026-10-16　来源：intent.md

## 用户故事
| # | 角色 | 目标 | 价值 | 溯源（intent.md） |
|---|---|---|---|---|
| U1 | 项目所有者 | 核验 `docs/ROADMAP.md` 中 M0–M5 与「长期」段的每个 `[x]` 完成声明是否属实 | 路线图恢复可信，杜绝「勾了但没验」腐蚀可信度 | 「问题与期望」要解决的问题第 1 点 |
| U2 | 项目所有者 | 找出计划/架构文档字面与仓库实现现状的偏离，并给出回写建议 | 掌握真实能力边界，文档与现实对齐 | 「问题与期望」要解决的问题第 1 点（已见苗头） |
| U3 | 项目所有者 | 把路线图开放项与四个已完结 intent 的 P2 backlog 归集为统一清单 | 散落项统一可见、可排序，避免遗忘 | 「问题与期望」要解决的问题第 2、3 点 |
| U4 | 项目所有者 | 盘清质量与交付面现状（测试、五断言脚本、三 CI 工作流、已知边界） | 掌握交付可信度与残留风险 | 「受影响系统/用户」系统面 + 「问题与期望」已列 277 测试/五断言/三工作流 |
| U5 | 项目所有者 | 得到按依赖/风险/价值排序的下一阶段路线图 | 可据此逐个开新 intent 推进实现 | 「一句话意图」+「期望的结果」 |

## 验收标准（可测）

> 本 intent 为审核+规划类：每条 AC 都是**必须回答且有证据要求**的审核问题（Given 审核对象/范围、When 执行什么核验、Then 产出什么可判定的结论/证据形态）。证据形态统一为可复现材料：`文件路径:行`、命令、输出片段；禁止空泛评价。证据标准取 decisions.md A1 默认=**抽样验证**（每里程碑 1–3 条关键声明）；交付范围取默认=**审核+路线图**。里程碑字面与 `docs/ROADMAP.md` 一致。

- **AC1**（U1，抽证方法与分级基线）：Given `docs/ROADMAP.md` M0–M5 与「长期」段全部 `[x]` 完成声明（含各定时子项）与抽样验证标准（每里程碑 1–3 条关键声明）；When 对每条被抽中的声明执行独立核验（对照代码/测试/CI/命令输出）；Then 每条给出三态判定〔已证实／漂移／不可证〕并附可复现证据（文件路径:行、命令、输出片段），不得以转述或估算替代。
- **AC2**（U1，M0 段）：Given M0 `[x]` 项（`cloudmount-cli login` PKCE 设备码扫码、凭据持久化、token 自动刷新、`ls`/`quota`、`upload` 秒传链路、`download` Range 流式、全局限速+指数退避+频控码识别）；When 抽样核验；Then 每项三态判定 + 证据，并标注 M0 唯一未勾项「待 AppID 实测」的阻塞状态。
- **AC3**（U1，M1 + M1.5 段）：Given M1 段（SQLite 元数据库、`LocalWatcher`/`PollingWatcher`、`change_probe`、`reconcile::plan`、队列调度、块缓存、执行器、`cloudmountd`、**114 个测试**声明、已知边界「同秒漏检」）与 M1.5 段（Windows RDC 原生 watcher、Linux inotify、macOS 暂轮询、内容校验通道关闭盲区、重命名零传输、schema v0.2、**131 个测试**声明）；When 抽样核验（含测试计数声明与实际 `cargo test` 现状对照）；Then 每项三态判定 + 证据，并明确「同秒内等大小修改漏检」盲区是否已被 M1.5 内容校验通道正式关闭。
- **AC4**（U1，M2 + M2.5 段）：Given M2 段（CfAPI 后端基座、`CfapiBackend`、`PlaceholderSync`、daemon `--placeholder-mount` 接线、M2.5 内容变化重投影+dehydrate+新本地目录上传、Population 策略 `ALWAYS_FULL(3)`、双向写路径 `LocalChangeUploader`、两处「Tauri 壳」声明、应用一致性目录防护、资源管理器状态图标与右键动词）与未勾的「MSIX 打包」；When 抽样核验（**特别对照「Tauri 壳」字面与实际 `apps/desktop` 技术栈**）；Then 每项三态判定 + 证据，并对「Tauri 壳」声明给出漂移结论。
- **AC5**（U1，M3 段）：Given M3 段（v1 扫码换 Cookie+只读驱动+`--channel cookie` 不等 AppID 可挂载、v2 上传写操作全套、自动回退 `ProviderRouter`、未勾的「Cookie 独有能力：分享/转存/批量操作」）；When 抽样核验；Then 每项三态判定 + 证据，并标注「真实端到端待扫码验证」的当前状态。
- **AC6**（U1，M4 段）：Given M4 段（WebDAV 出口 + web-ui 托管 curl 实测、musl 静态交叉 + rustls TLS + 打包 + systemd unit + Dockerfile + release workflow + `docs/edge.md`）；When 抽样核验；Then 每项三态判定 + 证据，并标注验收指标「512MB 内存设备稳定运行 48h」的达成状态。
- **AC7**（U1，M5 段）：Given M5 段（缓存可视化/设置页/日志导出、万级投影基准 618ms（16,330 条/秒）、未勾的「macOS File Provider 扩展」、并发水合 8 并发流）；When 抽样核验；Then 每项三态判定 + 证据。
- **AC8**（U2，已见苗头偏离核验）：Given intent 已列四项偏离苗头（ROADMAP M2/M5 写「Tauri 壳」而 `apps/desktop` 实为 egui/eframe 壳、`apps/shell-macos` 为占位 README、`.gitignore`/`scripts/gen_icons.py` 残留 `src-tauri` 死路径、`web-ui` 角色变迁未回写）；When 逐项核验实际仓库状态；Then 每条产出四栏记录「文档/路线图字面｜实际实现｜证据｜建议回写位置」，并给出确认为真/为假的明确结论。
- **AC9**（U2，偏离面扩展扫描）：Given `docs/ARCHITECTURE.md`（v0.3）、`docs/design/2026-10-12-exploration-v0.2.md`、`docs/edge.md`、各 README 的字面现状，与顶层结构（workspace 七 crate、`apps/desktop`、`apps/shell-macos`、`edge/`、`tools/bin`、`drivers/win`、`web-ui`）；When 扫描计划/架构文档字面与实现现状之间的其它偏离；Then 产出偏离清单（新增偏离逐条带证据；若确无则显式声明本档扫描未发现新增偏离）。
- **AC10**（U3，路线图开放项归集）：Given 路线图全部未勾项（M0「待 AppID 实测」、M1「待真实凭据联调」「验收指标：空闲 RSS ≤ 20MB」、M2「MSIX 打包」、M3「Cookie 独有能力」、M5「macOS File Provider 扩展」、长期「自研 Windows minifilter」）；When 逐项归集；Then 每项标注依赖源、阻塞性质（外部/内部）、可独立开 intent 的最小粒度与证据位置。
- **AC11**（U3，四 intent backlog 归集）：Given 四个已完结 intent 的 P2 backlog 与观察项（`docs/aidlc/audit-p0p1-fixes/`、`macos-release/`、`installer-packages/`、`desktop-installers/` 各自的 `ops-runbook.md` 与 `retro.md`）；When 归集、去重与分类；Then 产出统一 backlog 清单（每条含来源 intent + 去重标记 + 分类〔代码/文档/CI/发布〕+ 与 AC10 开放项的关联）。
- **AC12**（U4，测试与 CI 现状盘点）：Given 测试现状（runbook/retro 记载 `cargo test --workspace` 277/0、五断言脚本计数 112/72/23/13/9）与三 CI 工作流（`.github/workflows/ci.yml`、`check-macos.yml`、`release.yml`）与 `apps/desktop` 处于根 `Cargo.toml` `exclude` 的事实；When 抽样核验测试计数与 CI 覆盖现状；Then 给出覆盖现状结论 + 已知缺口清单（如根级 fmt/clippy/test 对 workspace-exclude 的 `apps/desktop` 零效力、CI 无 actionlint 步骤、desktop 4 测试 CI 不可见），每条带证据。
- **AC13**（U4，已知边界与交付面盘点）：Given 仓库已知边界（M1.5 声称已关闭的「同秒漏检」、mac 产物未 Developer ID 签名/arm64 ad-hoc、dmg aarch64-only、WiX 硬锁 3.14.1.20250415、`macos-15` runner 依赖、Cookie 通道条款风险、WebDAV v1 无内置 TLS/无速率限制配置）；When 逐项盘点；Then 每条给出当前状态 + 残留风险 + 是否已被后续里程碑消解，带证据。
- **AC14**（U5，下一阶段路线图）：Given AC1–AC13 的审核结论与四个候选头号方向（M0 AppID 实测、macOS File Provider、Cookie 独有能力、backlog 清理）；When 按依赖/风险/价值三维排序；Then 产出下一阶段推进计划（含排序后的批次、每批可开 intent 的边界、批次间依赖关系），使项目所有者可据此逐个开新 intent。

## 非目标（明确排除）
- **不改产品代码**：本期为审核+规划类，实现推进随后续 intent 逐个做（intent.md 约束）。
- **不发明路线图未列的需求**：任何能力不得擅自纳入下一阶段（intent.md 约束）。
- **不做性能/安全专项审计**：仅做现状盘点；深层性能/安全专项留给后续独立 intent。
- **不做全量逐项验证**：证据标准取抽样（每里程碑 1–3 条关键声明）；全量验证属另一档，非本期默认。
- **不改已完结 intent 的原始工件**：`audit-p0p1-fixes`/`macos-release`/`installer-packages`/`desktop-installers` 的 intent/state/工件只读引用，回流只写本新 intent。

## 开放问题（需人类裁决）
- [ ] **本期交付范围复核（待 gate1 人审）**：审核+路线图（decisions.md A1 编排者默认）vs 审核+头部项实现 vs 审核+清全 backlog。
- [ ] **证据标准复核（待 gate1 人审）**：抽样验证（decisions.md A1 编排者默认，每里程碑 1–3 条关键声明）vs 全量逐项 vs 仅文档一致性。
- [ ] **下一阶段头号方向**：M0 AppID 实测（外部阻塞项，等审核现状后定）／macOS File Provider（M5 未勾）／Cookie 独有能力（M3 未勾）／backlog 批量清理——依赖审核结论与人类偏好。
- [ ] **route 缺省 classic 是否认可（待 gate1 前复核）**：classic→express 降级被协议禁止；本期缺省取 classic 即锁定全量路径，若偏好 express 须在 gate1 前以人类裁决留痕切换。
