# Gate#1 AI 互审（2026-10-28）

> 评审对象：intent `ci-desktop-fmt-clippy`（route=classic）三件工件——prd（AC1..AC4）/ architecture（CI 变更蓝图）/ test-strategy（6 TP）。
> 评审方：reviewer-pm（1/4/8）+ reviewer-architect（1/7/8）+ reviewer-qa（0/4/7）+ reviewer-dev（1/3/7）——合并 **2 BLOCKER / 8 MAJOR 主题（去重）/ ~20 MINOR（去重 ~14）**。
> 正面确认（四方交叉实证）：ci.yml 四 job 结构/workflow env/on: 实证；`apps/desktop/Cargo.lock` 在仓；desktop 最后一次触碰=2a02050（2026-10-16，其后零触——AC2 证据链成立）；「desktop fmt/clippy 在 CI 从不执行」前提全 workflow 成立（check-macos/release 均无 lint 步）；断言脚本可写性（job_block 对 ci.yml 提取实测：rust 22/rust-mock 20/web-ui 16/arm-cross 24 行、desktop 0=红阶段符合预期）；desktop fmt 当前 **clean**（dev 实测 `cargo fmt --manifest-path ... --all -- --check` exit 0）；Windows job 无额外系统依赖（winresource rc.exe 自带）。

## BLOCKER（2，均须 gate1 裁决/修订后方可进 Construction）

- **B1｜fmt/clippy 命令字面与断言互斥（三工件直接矛盾）**[pm-B1+arch-M1+qa-M1]：prd AC1①（19 行）/TP-01（11 行）要求「命令原文 `cargo fmt --all -- --check`」且上下文锚允许三形态（working-directory/cd/--manifest-path）；架构蓝图（69/71 行）锁定 `cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check` 完整字面+断言 #7/#8 钉死+断言 #10 负断言「无 `cd apps/desktop`」——**按蓝图实现则 TP-01 原文 grep 必红；按 TP-01 字面实现则架构断言红且违决策 5**。必须 gate1 钉死唯一字面源（建议=arch #7/#8 完整 --manifest-path 字面，同步修订 prd AC1①/TP-01 措辞并删除与锁定形态冲突的锚选项）。
- **B2｜rust-cache `workspaces: apps/desktop -> desktop` 语义错误（缓存完全失效）**[arch-B1+dev-B1]：Swatinem/rust-cache@v2 的 `$workspace -> $target` 语义中 **target 相对 workspace**（src/config.ts：`target = path.join(root, target)`）——`apps/desktop -> desktop` 实际解析为 `apps/desktop/desktop`（不存在）→ **desktop target 永不入缓存**，每次 PR 冷编 eframe/wgpu 数百 crate；且 `workspaces` 一旦给出**不连带根 workspace**（架构「连带恢复」叙述错误）；断言 #6 只 grep `apps/desktop` 对错误值照样 PASS。修复=值改 `apps/desktop -> target`（或仅 `apps/desktop`）+断言锁精确形态+TP-01 承接。

## MAJOR（8 主题，去重）

- **M1｜AC4 零删改护栏锚集不完整**[pm-M3+arch-M2+qa-M4+dev-M2]：架构 #11-14 只锁四 job id+rust 三命令（漏 `cargo check --workspace`）+on:/RUSTFLAGS；漏三 OS matrix/components/另三 job 命令/`CARGO_TERM_COLOR`；#12 的 `fmt --all --check` 少了 `--`。删 Check 步/改 matrix/改 npm 命令脚本仍全绿。修复=补齐至 AC4 实文。
- **M2｜AC1⑤ 负断言不完整**[pm-m4+arch-M3+qa-m3+dev-M3]：只锁 continue-on-error+if:，漏「无 status 函数调用」（always/success/failure/cancelled()）+shell 级静默绿（`|| true`/`set +e`）——先例 regex 现成（desktop-installers-yaml.test.sh:355-372）。且 pm-m4 指出 `if: always()` 是强制运行非跳过——真正 fail-open 形态=`if: false`/条件收窄。
- **M3｜AC1④ 缓存覆盖无门控判据**[pm-m1+arch-M7+qa-M2]：TP-01 只查 rust-cache step 存在/参数「Construction 钉死」→ B2 错误值也全绿；TP-06 缓存命中不设门。修复=TP-01 承接精确字面断言（`apps/desktop -> target`）。
- **M4｜现状基线三方矛盾+架构时间线错误**[pm-M2+arch-M4+qa-m4]：prd AC2 证据链预期 clean vs 架构「未知」+TP-03「大概率 dirty」；dev 实测 fmt **clean**（exit 0）；架构称「托盘接线 533e456 在 fmt 清偿 2a02050 之后」实为之前（11:21 vs 15:40）。修复=三方统一「证据链预期 clean，Construction 当日实测确认」+修正时间线。
- **M5｜架构未对齐 PRD（根因）**[arch-M6+dev-M1]：architecture.md:4 自认「prd 在制未读」、语义对齐表占位——M1/M4 分歧的直接产物。修复=架构按 prd AC1-AC4 实文回填对齐表+同步断言清单。
- **M6｜AC4 diff 文件面零触碰无测试点**[qa-M3]：TP-02 无 `git diff --name-only` 文件集断言——「仅 CI 配置变更」硬约束无守卫（desktop 源码/release.yml/check-macos/Cargo.toml 零触碰）。
- **M7｜OQ① 裁决未成 AC/TP 判据**[pm-M1]：job 形态与 runs-on 交给开放问题但 AC1/TP-01 无 job id/runs-on 判据——裁决落地无法在 gate2 验证。修复=AC1 增「形态与 runs-on 按裁决落字面」子项+TP-01 同步。
- **M8｜缓存体积/10GB 淘汰风险未评估**[arch-M5]：desktop target 本机 5.0GB（debug 3.9GB）——GitHub 每仓缓存上限 10GB，可能挤掉既有三 OS 缓存（反 PRD 非目标「不优化既有 job」意图）。首跑后量体积/命中率；必要时 shared-key 隔离+timeout。

## MINOR（~20 报告条目，去重 ~14）

1. 断言脚本名分歧：prd 写 `scripts/ci-yaml.test.sh` vs 架构钉 `scripts/ci-desktop-yaml.test.sh`（架构理由域过宽成立，gate1 同步 prd）[arch-m1]
2. 「覆盖闭环」过度声明：windows 单腿不 lint `#[cfg(not(windows))]`（main.rs:745）——写明确接受项而非闭环 [arch-m2]
3. AC2/TP-03 本机命令未复刻 CI 的 `RUSTFLAGS="-D warnings"`（本机不带会漏报 rustc 级 warning）[arch-m3]
4. 架构决策 5「仓内已锁定先例」理由不全（ci.yml web-ui job 就用 working-directory）[arch-m4]
5. 架构 OQ⑥「desktop cargo test 顺带可裁」与 prd 非目标「不做 desktop cargo test」冲突——默认排除，选入需 PRD 修订 [arch-m5+pm-m8]
6. 新 job 无 permissions 块——最小权限建议 job 级 `contents: read`（或显式接受现状）[arch-m6]
7. 「断言不得弱化」未落到新脚本蓝图（先例头注）[arch-m7+pm-m5]
8. 架构 check-macos 未读风险已在互审关闭——架构补「已核实」[arch-m8]
9. OQ⑤ 冗余（当日实测归属 AC2 When 已归 Construction）[pm-m8]
10. brownfield 一致性约束无验收锚（命名/风格沿既有）[pm-m2]
11. AC2 dirty 分支缺批次准出条件（修复批号/backlog 登记）[pm-m3]
12. AC3「断言不得弱化」无操作定义 [pm-m5]
13. 新脚本六脚本口径未进 PRD 交付/非目标（可能成死守护——架构:144 承认）[pm-m6+dev-MINOR4]
14. 架构断言 #12 三命令弱于 AC4/TP-02 四命令 [pm-m7，并入 M1]
15. FLoat 工具链漂移风险（`@stable` 浮动——clean 是本地口径，CI 首跑红也可能是工具链漂移）[dev-MINOR3]
16. fmt `--all` 会连带根 crate（归因叙述小偏差）[dev-MINOR5]
17. actionlint 正断言现状基线未验证 [dev-MINOR6]
18. branch protection 依赖仓库外设置（required checks）[dev-MINOR7]
19. TP-05 负向注入机制未钉（坏 YAML 构造法）[qa-m2]
20. P0 进门充分性依赖 P1 元机制（TP-04 防空真验证 P0 grep 可信度）[qa-m6]
21. TP-06 首跑渠道写死 push（应 PR 优先）[qa-m7]

## 结论

2 BLOCKER / 8 MAJOR 主题 / ~20 MINOR。**两个 BLOCKER 均实质**：B1 命令字面三方互斥（实现即假红/自证）、B2 rust-cache 语义错误（intent 核心约束「缓存覆盖」完全落空且断言抓不到）。其余 MAJOR 集中于「架构蓝图与定稿 PRD 未对齐」（M5 根因）与「护栏/负断言锚集不全」（M1/M2/M3/M6）。
**建议人类审批关注点**：①总裁决（打回修订 vs 带病通过——不推荐：B1/B2 属实现即失效级）②**B1 字面源裁决**：采架构 `--manifest-path` 完整字面（推荐——与 release.yml 先例一致+断言可精确）③**OQ① job 形态裁决**：独立 `desktop` job+windows-latest 单腿（推荐——四维对比+平台门控实证）④修订范围：架构按 PRD 回填对齐表+修 B2/M1/M2/M4，prd/TP 同步字面与断言锚集。
