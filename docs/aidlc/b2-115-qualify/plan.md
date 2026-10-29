# Plan: b2-115-qualify

> 产出者：编排者（Sprint Planning）　日期：2026-10-18　前置：gate1 approved（c5e3eee，含准出回填 73a40ec）

## Units

| # | Unit | 目标 | 涉及文件 | 影响面 | 测试 | 验收 |
|---|---|---|---|---|---|---|
| U1 | 申请材料技术清单定稿+tracking.md 建文件 | 工程侧产出：①申请材料技术清单文档（按蓝图规格 11 项逐项附代码锚+「提交时按平台页面核对」未知项标注——载体=`docs/aidlc/b2-115-qualify/application-checklist.md`）②tracking.md 建文件（按蓝图数据模型规格：路径/键集含 check_method/draft=尚不建记录/追加式更新语义——首条记录自 submitted 起，本阶段建骨架+字段说明）③人类执行 115 平台提交的**操作指引**（按清单逐项+开放问题③现场裁决项+敏感边界提醒） | `docs/aidlc/b2-115-qualify/application-checklist.md`（新建）、`docs/aidlc/b2-115-qualify/tracking.md`（新建） | low（纯文档产出——F3 扫描：新增 2 文件均在 intent 目录内，零代码引用方） | TP-01（材料清单逐项核对[代码锚抽验+未知项标注]）/TP-02（敏感边界：两文件零密钥凭证明文） | 清单 11 项全落字+代码锚实测+未知项显式标注；tracking.md 骨架与蓝图规格一致（键集/check_method/draft 注记）；TP-01/02 全过 |
| U2 | 人类提交执行+受理跟踪口径在案 | ①人类按操作指引在 115 开放平台完成提交（AC2 核心动作——**人类执行**）②提交凭据留痕落 tracking.md 首条记录（submit_date/receipt_no/status=submitted/check_method/notes）③受理跟踪口径补全（expect_date 由 submit_date+7 推导/escalate_date +14/超期升级路径占位+待人类裁决标注） | `docs/aidlc/b2-115-qualify/tracking.md`（人类操作+工程侧协助留痕） | low（同上） | TP-03（五字段在案+证伪） | AC2①②③+AC3①②③ 全落字；tracking.md 首条记录完整 |
| U3 | AppID 接入预备清单定稿 | 工程侧产出：AppID 到位预备清单文档（两处 TODO(verify) 逐条+config 注入点分面表述[CLI flag/env vs config.toml 仅 daemon/desktop]+下游「AppID 实测」intent 的输入清单——载体=`docs/aidlc/b2-115-qualify/appid-readiness.md`） | `docs/aidlc/b2-115-qualify/appid-readiness.md`（新建） | low | TP-04（预备清单完备+动态清点）/TP-05（零代码 diff 边界） | 清单与代码实况逐条一致；TP-04/05 全过 |

## 验证命令（F2 规范来源）

- 命令：`cargo test --workspace`
- 辅助现状锚：五断言脚本 `bash scripts/{desktop-installers-yaml,installers-yaml,release-yaml,check-macos-yaml,package-macos}.test.sh`（计数 112/72/23/13/9）
- **基线已录**（第 0 步，2026-10-18T23:27:30+08:00——注意系统日期已跨 2026-10-27，以命令执行时点为准）：workspace **277 passed/0 failed**（26 目标）；五脚本 **112/72/23/13/9** 全 exit 0
- 回归语义（本 intent 特化）：**人类操作批+工程侧零代码**——终验重跑同一命令 **277/0 保持+五脚本计数不变=零影响证明**；TP-05 diff 边界（基点 14a7eb4..HEAD 产品目录零改动）为「零代码变更」唯一机械判据

## 执行顺序与并行泳道

- **串行 U1→U2→U3**（U2 依赖 U1 的操作指引与 tracking 骨架；U3 独立但收尾统一验证）；无并行泳道（纯文档批，无 IaC/E2E 面）
- U2 为**人类执行**段：工程侧产出操作指引后呈人类执行提交，提交完成后工程侧协助落 tracking.md 留痕

## TDD 锁记录

- **N/A（如实记录）**：本 intent 为人类操作批+工程侧零代码，**零测试文件产出**，TDD 锁无锁定对象（`tddLock=false`）；质量守护=TP-01..05 验证+gate2 复审+verifier 独立终检

## 缺口与开放问题

- [ ] U2 人类提交的时点不受 intent 生命周期约束（提交后 7+7 工作日为外部审核期——本 intent 在提交留痕完成后即可进 gate2，**不必等待审核结果**[AC3 跟踪口径在案即满足]；审核结果追踪归 operations/后续 intent）
- [ ] 开放问题①（申请主体实名信息）②（超期升级口径）③（平台强制字段）④（应用展示信息）——①④人类执行时现场定，②留占位待裁决，③提交时按平台页面核对
