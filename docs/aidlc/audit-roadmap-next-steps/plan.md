# Plan: audit-roadmap-next-steps

> 产出者：编排者（Sprint Planning）　日期：2026-10-16　前置：gate1 approved（f1bec70，含准出回填 a130fd3）

## Units

| # | Unit | 目标 | 涉及文件 | 影响面 | 测试 | 验收 |
|---|---|---|---|---|---|---|
| U1 | 里程碑断言核验·上（M0/M1/M1.5） | 执行 TP-01..06+28/29（承接 43 条 [x] 中 M0×7/M1×9/M1.5×7=23 条，全量逐条+句内子声称），创建 `audit-findings.md`（骨架 §0–§6 占位+§1 的 M0–M1.5 内容），每条三态判定〔已证实/漂移/不可证〕+可复现证据 | `docs/aidlc/audit-roadmap-next-steps/audit-findings.md`（新建） | low（新增审核报告单文件，无代码引用方——F3 扫描对象不存在，审核类 unit 如实记 low） | 承接 TP 的四要素执行（核验对象/动作/通过/证伪） | TP 全执行+三态判定齐+证据可复现（命令可重跑） |
| U2 | 里程碑断言核验·下（M2 段/M3/M4/M5/长期） | 执行 TP-07..12+30（M2 段×12/M3×3/M4×2/M5×3/长期声明=21 条），续写 §1 | `audit-findings.md`（续写） | low | 同上 | 同上 |
| U3 | 偏离扫描+质量与交付面体检 | 执行 TP-13..20+24+27（假完成探测器/偏离扫描/审核自证/文档一致性/AC13 七项批量——TP-27 为 gate1 r1 新增，Sprint Planning 漏分配，gate2 M2 准出回填补录；执行载体=报告 §4.2），写 §2（计划-实现偏离）+§4（质量与交付面盘点） | `audit-findings.md`（续写） | low | 同上 | AC8/AC9/AC12/AC13 的核验产出齐备 |
| U4 | 归集+路线图+报告集成 | 执行 TP-21/22/23/25/26（backlog 归集去重+开放项+排序），写 §3（归集表）+§5（下一阶段路线图批次表）+补 §0（摘要与计数矩阵）；**AC15 五项机械完整性自检**（七章齐备/AC1–AC13 逐条落点/证据行/批次依赖/计数矩阵一致） | `audit-findings.md`（收尾） | low | 同上 | AC15 五项全过=报告完成（gate2 机械判定口径） |

## 验证命令（F2 规范来源）

- 命令：`cargo test --workspace`
- 辅助现状锚（非基线回归面）：五断言脚本 `bash scripts/{desktop-installers-yaml,installers-yaml,release-yaml,check-macos-yaml,package-macos}.test.sh`（末行计数 112/72/23/13/9）
- **基线已录**（第 0 步，2026-10-16T21:26:42+08:00）：workspace **277 passed/0 failed**；五脚本 **112/72/23/13/9** 全 exit 0——与 desktop-installers 终验状态一致（本机即开发机，无环境漂移）
- 回归语义（本 intent 特化）：审核为**只读**（prd 非目标「审核期零写入」）——终验时重跑同一命令，**277/0 保持+五脚本计数不变=零影响证明**；任何漂移=审核过程污染工作区=BLOCKER

## 执行顺序与并行泳道

- **串行 U1→U2→U3→U4**（沿限流教训；§1 章节由 U1 建骨架后续写，§0/§5/§6 依赖前序结论后置）——无并行泳道（审核类无 IaC/E2E 面，tea 不派）
- 执行序沿 test-strategy 注记：静态 grep→符号核验→TP-03/14 共享同一次 cargo test 复跑→TP-13→TP-17/25/26 报告后置
- 本机执行环境注记（test-strategy M10 声明）：cargo test 复跑沿用基线口径（含本机 cldflt 集成腿，瞬态夹具副作用显式接受）

## TDD 锁记录

- **N/A（如实记录）**：本 intent 为审核+规划类，非目标「审核期零写入」——**不产出任何测试文件**，TDD 锁无锁定对象（`tddLock=false`）；审核质量的守护=TP 四要素+gate2 复审+verifier 独立终检，非测试文件锁

## 缺口与开放问题

- [ ] 真机项/外部项（AppID 实测、Mac 真机、资质审批进度）——沿 test-strategy 缺口节如实标「声称·未独立复现」，不阻塞审核
- [ ] 计数环境相关：desktop 断言脚本 actionlint 可用环境为 113——报告口径注明
