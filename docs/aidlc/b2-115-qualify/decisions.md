# Decisions: b2-115-qualify

> 只追加（Edit 被 guard-gates deny，一律 Write 全文+追加）；人类裁决、门控回执、降级事件逐字记录于此。

---

2026-10-18 A1 意图接入确认（AskUserQuestion 表单未获选择；按协议缺省取值，编排者留痕并标注来源）：
- 意图原话（逐字，接入信号=ops 裁决③「直接开下一批」+批次表顺位裁定）：`M0 外部阻塞链启动：申请 115 开放平台个人开发者资质+创建 PKCE 应用（材料提交+受理跟进；人类操作为主，工程侧配套=申请材料技术清单整理+AppID 到位后的 TODO(verify) 闭环预备）——路线图批次表 B2`
- 识别的意图一句话：启动 115 开发者资质申请（外部阻塞链源头，审核约 7+7 工作日）——工程侧整理申请材料技术清单+受理跟踪口径+AppID 接入预备。
- 粗粒度拆分预览（3 拆，逐字）：①申请材料技术清单（按 115 开放平台要求整理 PKCE 应用创建所需技术信息——源自现有代码事实：PKCE S256/扫码/token 刷新）②提交与受理跟进（人类执行提交；记录提交凭据与受理状态跟踪口径）③AppID 到位预备（open.rs:274,371 两处 TODO(verify) 闭环项+config 注入点确认——为 M0「待 AppID 实测」intent 铺路）。
- 呈批一问（AskUserQuestion，逐字）：Q「意图【B2 资质申请：115 开放平台个人开发者+PKCE 应用创建（外部阻塞链源头，审核约 7+7 工作日）】——是否进入 AIDLC 生命周期？路线选哪个？」（进入 express（推荐）／进入 classic／改开 CI 小批／搁置）。**表单未获选择。**
- 裁决落点（编排者，按协议缺省取值+来源标注）：
  1. **接入=是**：ops 裁决③「直接开下一批」（b1-doc-corrections ops-runbook/decisions.md 2026-10-18）——批次表顺位 B2 为下一批。
  2. **route=classic（协议缺省）**：表单未选 express → 按 skill §3.0 缺省 classic（fail-safe 全量路径）；classic→express 降级被禁止，缺省即锁定——人类若偏好 express 须在 gate1 前裁决（列 intent.md 开放问题——注意本 intent 呈批选项曾提议 express，未获选择即按协议缺省，不推断偏好）。
  3. **范围边界**：人类操作为主+工程侧零代码（审核报告 §3.1 原判沿袭）；「改开 CI 小批」选项未获选择，不切换（CI 卫生批留在台账条目 1/3/23 待命）。
- 后续：建 intent 骨架 → 按 classic 扇出 pm/architect/qa 三工件 → 对齐拍 → phase=gate1。

---

2026-10-18 Inception 对齐拍记录（编排者；qa 映射回填由 qa 角色执行、跨 intent 重派新 agent）：
- 三工件：prd @ 本轮（AC1..AC4：申请材料清单定稿/提交凭据留痕/受理跟踪口径/AppID 预备清单）/ architecture（申请材料技术规格 11 项[PKCE 构造/method/verifier/刷新机制/15 端点面]+AppID 接入预备[config 注入点+两处 TODO 闭环条件]+受理跟踪状态机六态）/ test-strategy（5 TP P0=3/P1=2）。
- **勘误锚定**：open.rs:274,371 两处 TODO(verify) 实际位于 `crates/cloudmount-driver115/src/open.rs`（编排者派发消息误写 auth/open.rs——architect 落笔前实测纠正并锁定，供下游「AppID 实测」intent 使用）。
- **qa 对齐回填**（跨 intent 重派新 qa agent，B1 qa 会话已过期）：映射列 TP-01→AC1/TP-02→AC1+AC2/TP-03→AC2+AC3/TP-04→AC4/TP-05→全 AC 约束；AC1..AC4 每条 ≥1 TP 机械核无缺口；prd 三条新增开放问题（tracking.md 载体/平台强制字段现场裁决/应用展示信息）经核均有 TP 判据联动或归记录类，无需新增 TP；「四拆/三拆」措辞差实为 AC 四条与三拆预映射的计数差，回填后已消解。
- 对齐拍结论：**工件齐且对齐**（classic 三件全落盘并提交；qa 引用 AC↔prd 实际编号一致[AC1..AC4 全覆盖]）。
- 相位转移：phase → gate1（gate1=pending），提示 `/aidlc:gate`。

---
