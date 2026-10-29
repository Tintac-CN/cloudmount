# Plan: ci-desktop-fmt-clippy

> 产出者：编排者（Sprint Planning）　日期：2026-10-29　前置：gate1 approved（GATE-APPROVED gate1 2026-10-29 @ 2d5d695）
> 开工基线 commit：`2d5d695`（TP-02 diff 白名单的 `<base>`，Construction 各提交均以此为基准）

## Units

| # | Unit | 目标 | 涉及文件 | 影响面 | 测试 | 验收 |
|---|---|---|---|---|---|---|
| U1 | desktop CI 执行面 + 断言脚本（TDD 红→绿） | ①新增 `scripts/ci-desktop-yaml.test.sh`（47 条断言，红阶段 desktop 断言 FAIL）②ci.yml 新增独立 `desktop` job（`runs-on: windows-latest` 单腿；fmt/clippy 唯 `--manifest-path apps/desktop/Cargo.toml` 字面；`workspaces: apps/desktop -> target`；`permissions: contents: read`；`timeout-minutes: 60`；四既有 job 零删改） | `.github/workflows/ci.yml`（仅新增）、`scripts/ci-desktop-yaml.test.sh`（新文件） | low（ci.yml 新增 job 无代码引用方；消费方=GitHub Actions 调度面；新脚本无既有消费方——runbook「六脚本」口径归 ops 沉淀） | TP-01/02/03/04 脚本自含；TP-05 actionlint 降级分支（本机不可用）；TP-06 归 CI 首跑（E2E 不可本地验证） | ①脚本末行 `47/47 passed` exit 0（红阶段预期 desktop 断言 FAIL）②既有五脚本计数 112/72/23/13/9 不变且 exit 0 ③`cargo test --workspace` 回归对照基线 277/0 零回退 ④desktop fmt/clippy 本机实测如实记录（AC2 留痕）⑤diff 白名单=`ci.yml`+新脚本，黑名单零命中 ⑥`cargo fmt --all -- --check`/`cargo clippy --workspace --all-targets -- -D warnings` 仓库面零变化 |

> 影响面依据：`grep -rln "desktop-installers-yaml.test.sh"` 命中全部为 docs/README（非代码 import）；`ci.yml` 无源码引用方。U1 无高影响面拆分需求。

## 验证命令（F2 规范来源）

- **基线/回归共用命令**：`cargo test --workspace`（仓库根，Git Bash；既有基线 277/0，26 目标）。回归比对范围限 `testBaseline.suites`，本批零 Rust 测试新增 → 任何基线 pass 用例新失败 = 回归 = BLOCKER。
- **本批新增验收命令**：`bash scripts/ci-desktop-yaml.test.sh`（末行 `<N>/<M> passed`；M 基线 47；红阶段 desktop 断言 FAIL 为预期，绿阶段 47/47 exit 0）。
- **既有五脚本回归**：`bash scripts/desktop-installers-yaml.test.sh`（112，actionlint 可用环境 113）/`installers-yaml.test.sh`（72）/`release-yaml.test.sh`（23）/`check-macos-yaml.test.sh`（13）/`package-macos.test.sh`（9）——计数不得变化、exit 0。
- **AC2 desktop 现状实测**：`cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check`；`RUSTFLAGS="-D warnings" cargo clippy --manifest-path apps/desktop/Cargo.toml --all-targets -- -D warnings`；另跑不含 `--all-targets` 对比（`RUSTFLAGS="-D warnings" cargo clippy --manifest-path apps/desktop/Cargo.toml -- -D warnings`）供 OQ② 留痕。
- **仓库面零变化**：`cargo fmt --all -- --check` 与 `cargo clippy --workspace --all-targets -- -D warnings`（本批不触 Rust 源码，预期 clean/零 diff）。

## 执行顺序与并行泳道

- **泳道 A（dev）**：U1 单 unit，严格 TDD——①写 `scripts/ci-desktop-yaml.test.sh` 并跑出红（desktop 断言 FAIL）→ 提交 ②编排者置 TDD 锁 ③ci.yml 加 `desktop` job（蓝图照 architecture.md 落字面）→ 脚本转绿 47/47 → 提交 ④AC2 实测 + 五脚本回归 + cargo test 回归并留痕。
- **泳道 B（architect）**：N/A——本批无 IaC/无新架构面；蓝图已由 gate1 批准，Construction 只落字面（architecture.md 断言清单 #1-#47 为唯一施工契约）。
- **泳道 C（tea）**：N/A——TP-06 为 CI 首跑 E2E（本地不可执行，归 PR/push 首跑观察）；无性能目标面。
- **顺序理由**：单 unit 无并行冲突；红→锁→绿满足 TDD 锁语义（测试先于实现提交）。

## TDD 锁记录

- 失败测试提交：`810ad27`（红态 `29/47 passed` EXIT=1——FAIL 恰为 desktop 断言 #1-#16/#45/#46）
- 置位：`f5d0295`（2026-10-29T21:50:16+08:00；`tddLockPaths=[scripts/ci-desktop-yaml.test.sh]`）

## 缺口与开放问题

- **TP-06（CI 首跑）本机不可验证**：归 PR（`pull_request`）首跑优先/push main 兜底；首跑红须先区分真实违规 vs `@stable` 工具链漂移（architecture 现状确认节口径），并对比本机 rustc 版本。
- **OQ②（clippy `--all-targets` 收窄）**：本批按含口径落地（B1 裁决字面）；本机对比记录留痕以支撑人类后续逐字确认。
- **OQ③（desktop dirty 分支）**：按 AC2 当日实测触发——若 dirty，产品面（CI 步骤）照落、首跑红为预期，判出须附裁决记录+修复批号/backlog 登记。
- **M6 缓存体积/10GB 配额**：首跑后观察（entry 体积/命中率/既有 job 时长），不设门；挤占时按 architecture 安全节顺序处置。
- **branch protection**：新 job 未加入 required checks 前不阻塞合并（仓库设置面，ops 记录后续项）。
