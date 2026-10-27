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

2026-10-18 Gate#1 人类审批（AskUserQuestion，逐字转写）：
- 互审输入：reviews/gate1-ai.md @ 7cf99dc（reviewer-pm 0/1/3 + reviewer-architect 0/1/5 + reviewer-qa 0/1/5 + reviewer-dev 0/2/6；合并 0 BLOCKER/8 MAJOR 报告条目[去重 2 主题]/19 MINOR[去重 ~14]。正面确认：规格 11 项+15 端点面代码锚抽验 40+ 处全部属实[含 RFC 7636 官方向量逐字]/勘误锚定复核正确/「零写入」TP-05 机械判据成立/受理状态机六态+7+7 口径三方自洽）。
- 呈批两问（AskUserQuestion，逐字）：
  问：「B2 资质申请 Gate#1 互审：0 BLOCKER / 8 MAJOR 报告条目（去重 2 主题）/ 19 MINOR。2 主题：M1 受理跟踪簇（载体未钉+数据模型缺「查看方式」键+TP-03 判据未单列——三工件连环缺口，一个裁决链闭环）/M2 AC4⑤ AppID 到位动作对 CLI login 不成立（config.toml 路线仅 daemon/desktop 生效——分面表述一行修）。如何裁决？」
  答：「**打回修订（推荐）**」
  问：「M1 载体裁决（gate1 当场可决）：受理跟踪载体采纳哪种？（采纳即同时补数据模型 check_method 键+TP-03 第⑤字段，三工件联动闭环）」
  答：「**tracking.md（推荐）**」
- 裁决落点（编排者执行）：
  1. gate1 打回（gate1=rejected，phase 回 inception；第 1 轮/上限 3 轮）：修订点名 pm/architect/qa 三角色（dev 无 inception 工件）。
  2. **M1 载体裁决=tracking.md**：受理跟踪载体=独立 tracking.md（可变键值文档，含 check_method 键承载「查看方式」；status 多次更新不改审计历史）——architect 蓝图补 tracking.md 规格（路径/键集/check_method/六态映射）+数据模型补 check_method 键+draft 态注记（draft=尚不建记录）；pm AC2①/AC3② 定指 tracking.md；qa TP-03 补第⑤字段（查看方式在案+证伪）判据并更新载体引用。
  3. **M2**：AC4⑤/蓝图下游输入清单① 改分面表述「CLI login 须 `--app-id` 或 `CLOUDMOUNT_APP_ID`；config.toml 路线仅 daemon/desktop 生效」。
  4. 修订点名清单——**pm**：M1（AC2①/AC3② 定指 tracking.md）/M2（AC4⑤ 分面表述）/MINOR（AC1 #5 示例对齐蓝图 #9[MacOS M5+ 技术未定不预写]/AC1⑧ usertime→user_utime 代码事实/AC1⑧ copy/delete 出处收窄[仅 copy]/preid 128K 补锚 hash.rs:14）。**architect**：M1 关联（数据模型补 check_method 键+draft 态「尚不建记录」注记+tracking.md 规格节落蓝图）/M2（下游输入清单① 分面表述）/MINOR（client.rs/envelope.rs 补 crate 前缀/scope 表述改「零 OAuth scope 请求参数」/multipart 表格锚改 client.rs:137+收录 open.rs:5-17 表格漂移为收录项）。**qa**：M1 关联（TP-03 补第⑤字段「查看方式在案」+判据+载体引用更新）/MINOR（TP-07 计数锚改动态清点——b1 先例/TP-05 基点钉 14a7eb4/TP-02 自命中排除规则/TP-05 白名单措辞收敛/TP-05 P1 定级注记/7+7 锚 :8→:3-4）。**decisions 补录（编排者随本条）**：🚀 emoji 错位（backlog A 节首行无 🚀、实为 B1 行——引用措辞失真勘误）。
  5. GATE-REJECTED 标记在后；修订完成后 gate1 复位 pending 复审（只核收敛点：2 主题 MAJOR 消解+载体裁决落实+MINOR 批量），须新回执。

GATE-REJECTED gate1 2026-10-18
---
