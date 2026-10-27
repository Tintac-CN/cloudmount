# 115 资质申请受理跟踪（b2-115-qualify）

> 产出者：dev（Construction U1）　日期：2026-10-28　规格来源：`architecture.md`「数据模型」节+「受理跟踪状态机」节（gate1 人类裁决钉定载体=本文件——decisions.md 2026-10-18 Gate#1 审批②）
>
> **追加式日志**：status 每次变更**追加新行**（新行含变更后 status 与日期），不改写旧行——保留完整审计历史。
> **draft=尚不建记录**：六态中 draft（未提交）不建记录，**首条记录自 status=submitted 起**（draft 态无提交日期，与 submit_date 不可空不冲突）。
> **敏感边界（TP-02）**：本文件禁止记录密钥/凭证明文/实名证件信息（`intent.md:19`）；AppID 值（非密钥）可记于 notes，但不与任何 secret 同处记录。

## 状态机（口径）

```text
[draft 未提交·不建记录] → [submitted 已提交] → [accepted 已受理·审核中] ─┬→ [approved 通过·得 AppID]
                             │ 记录：提交日期+凭据       │ 记录：受理号     └→ [rejected 驳回] →（补材料）→ submitted
                             └────────── 超期分支：escalated（escalate_date 到期未果，人类裁决去向）
```

## 键说明（7 键）

| 键 | 类型 | 约束 |
|---|---|---|
| `submit_date` | 日期（ISO 8601） | 提交当日记录；不可空 |
| `receipt_no` | 字符串 | 平台受理号/工单号原样记录；不可空（若无受理号机制则记提交凭据截图存放位置说明） |
| `status` | 枚举 submitted/accepted/approved/rejected/escalated | 六态映射：draft=尚不建记录，首条记录自 submitted 起；状态迁移按上方状态机 |
| `expect_date` | 日期 | 由 submit_date 推导（**+7 工作日**，首次审核），非平台承诺（口径源 `docs/ROADMAP.md:3-4`「审核约 7+7 工作日」） |
| `escalate_date` | 日期 | 由 submit_date 推导（**+14 工作日**=7+7 口径，源自 `docs/ROADMAP.md:3-4`），非平台承诺；到期未果→触发人类裁决（开放问题②，`intent.md:24`） |
| `check_method` | 自由文本 | 受理状态**查看方式**（承载 prd AC3②）：平台页面入口（在哪个页面看）+查看凭据（以什么账号/入口查看；=`receipt_no` 平台受理号/工单号）；不可空 |
| `notes` | 自由文本 | **禁止记录**密钥/凭证明文/实名证件信息；**显式承载** prd AC3③「超期升级路径」——人类裁决前留占位+标「待人类裁决（开放问题②）」，裁决后回填（升级裁决/催办记录落此键） |

## 记录区（人类提交后由 U2 落首条——本阶段为空）

> 行语义：每次 status 变更追加一行（记录日期=该行落笔时点，键值以当次时点为准），不改写旧行。首条（status=submitted）由 U2 在人类完成提交后落；检查时点建议（+3/+7/+10/+14 工作日）由 U2 落 notes。

| 记录日期 | submit_date | receipt_no | status | expect_date | escalate_date | check_method | notes |
|---|---|---|---|---|---|---|---|
