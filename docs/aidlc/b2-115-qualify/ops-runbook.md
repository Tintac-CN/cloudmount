# Ops Runbook: b2-115-qualify

> 产出者：编排者　日期：2026-10-28　前置：gate2 approved（c11b961，含准出回填 8b15527）
>
> 人类裁决（AskUserQuestion 2026-10-28，逐字留痕 decisions.md）：**tracking 维护=留仓+手动更新**；**下一步=直接开下一批（批号待收口时指定）**。
> 本 intent 为**资质申请批**（人类操作为主+工程侧零代码——无运行时服务面、无部署面）——监控/告警/部署三节按运维实质适配为「受理跟踪维护/下游触发/回流」。

## 系统画像

资质申请批：交付物 = application-checklist.md（申请材料技术清单，11 项+15 端点+操作指引）+ tracking.md（受理跟踪，追加式日志——首条占位记录已落）+ appid-readiness.md（AppID 接入预备清单——下游 intent 输入）。**核心外部依赖**：115 开放平台审核（已提交、卡住不审批——人类回报 2026-10-28）。

## 受理跟踪维护（裁决①：留仓+手动更新）

- **tracking.md**（`docs/aidlc/b2-115-qualify/tracking.md`）留仓——人类随审核进展**手动更新**（追加式日志：status 变更追加新行不改旧行；首条占位记录的待补字段[submit_date/receipt_no/check_method]由人类补充）。
- **维护触发点**：人类查审核状态时（建议 +3/+7/+10/+14 工作日各查一次——tracking notes 已载）；审核状态变化时（approved/rejected/escalated）；AppID 下发时（notes 记 AppID 值——非密钥）。
- **敏感边界**：tracking 记录禁止密钥/凭据/实名证件明文（骨架头注已载）。

## 下游触发（AppID 到位后）

- **触发条件**：115 审核通过+AppID 下发（tracking.md status=approved+notes 含 AppID 值）。
- **触发动作**：按 `docs/aidlc/backlog.md` A 节首行（B2 批次）→ 下游「AppID 实测」intent（ROADMAP.md:16 闭环项——两处 TODO(verify)+copy 响应字段+真实凭据全链路实测）；输入=appid-readiness.md（预备清单）+tracking.md（AppID 值）。
- **审核驳回分支**：tracking status=rejected → 按 prd 状态机 rejected→补材料→submitted 循环（人类操作）；持续卡住 → escalate（超期升级路径——开放问题②人类裁决后回填）。

## 回流机制（Operations 协议 §2）

- 生产事件/学习只写**新 intent 的 intent.md**；当前 intent 的学习沉淀于 `retro.md`（classic 硬性），已完结工件不再修改。
- 115 平台条款变更（表单字段/审核标准）：新 intent 处理（本批清单为 2026-10-28 时点快照——平台要求以官方页面实测时点为准）。

## P2 backlog

见 `docs/aidlc/backlog.md`（唯一权威台账）。本批转出项：无新增（M1/8 MINOR 均已随准出回填闭合）；下游「AppID 实测」intent 素材已备（appid-readiness.md）。

## 下一步衔接（裁决②）

**收口后直接开下一批**（批号由人类收口时指定——台账待命：B3 Cookie 独有能力/条目 23 CI fmt-clippy/条目 31 真机核验批[条件已达]/条目 24 hash.rs flaky 根治等）。
