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

2026-10-18 Gate#1 复审 r2 人类审批（AskUserQuestion 表单未获选择；对话逐字裁决）：
- 互审输入：reviews/gate1-ai-r2.md @ 49bd976（reviewer-pm 0/0/0 + reviewer-architect 0/0/1 + reviewer-qa 0/0/2 + reviewer-dev 0/0/4；合并 0 BLOCKER/0 MAJOR/~7 MINOR。上轮 2 主题 MAJOR+14 MINOR 全部消解实证：受理跟踪簇闭环[tracking 载体三工件联动/AC3② check_method 键承载/TP-03 五字段+证伪]/AC4⑤ 分面表述三方实测一致/行锚 15+ 处抽验零漂移）。
- 问：「B2 资质申请 Gate#1 复审（r2，只核收敛点）：上轮 2 主题 MAJOR+14 MINOR 全部消解实证（tracking 载体三工件联动/分面表述三方实测一致/行锚零漂移）；新发现 0 BLOCKER/0 MAJOR/7 MINOR（四步五步计数口径调和/子锚一位差/白名单标签等注记级）。如何裁决？」
  答：「**打回+反馈**」
- 反馈内容（=r2 复审 MINOR 清单，全部注记级；逐字锚定 gate1-ai-r2.md）：
  ①「四步/五步」设备码流程计数口径调和（prd AC1⑤ 与 TP-01 各自与实文一致但计数不同——加调和注记「登录四步+刷新为第 5 步」）②TP-04 子锚 :159-161→:158（空值守卫实在 :158）③AC2③ 白名单补「以 TP-05 判据为准」注记（现窄于 TP-05 面）④蓝图端点表 :55 usertime→user_utime（r1 仅派 pm 修 AC1⑧，architect 侧同字面未同步）⑤thread::scope 计数 5 处→4 处（r1 修正表述时转抄旧计数）⑥建文件规格补 AC3③「超期升级路径」承载键注记（notes 强制子句）。
- 裁决落点（编排者执行）：
  1. gate1 第二次打回（gate1=rejected 维持，phase 保持 inception；**第 2 轮/上限 3 轮**）：修订点名 pm（①③）/architect（④⑤⑥）/qa（①②）——①为 pm+qa 双侧调和注记。
  2. GATE-REJECTED 标记在后；修订完成后 gate1 复位 pending 复审 r3（只核 7 MINOR 消解），须新回执。**轮次提示：本批已用 2/3 轮，r3 若再打回将达上限**——修订务必一次闭合。

GATE-REJECTED gate1 2026-10-18
---

2026-10-18 Gate#1 r2 反馈修订核收（编排者内联代跑，SKILL §8）：
- **降级代跑记录**：三角色 agent 会话均过期（跨会话边界 SendMessage 无活跃任务）→ 编排者内联代跑 6 处一行修。授权链完整：反馈内容=人类裁决「打回+反馈」所锚定的 r2 复审 MINOR 清单（reviews/gate1-ai-r2.md @ 49bd976，每处含复审给出的精确订正内容）——零自由裁量。诚实声明：内联代行，缺独立复核——r3 复审（三方）将独立核验全部 6 处。
- **6 处全落字**（@ a0ae666）：反馈① prd AC1⑤+TS TP-01 双侧口径调和注记（登录四步+刷新第 5 步）；反馈② TS TP-04 子锚守卫 :158；反馈③ prd AC2③ 白名单补「以 TP-05 排除清单为准」；反馈④ 蓝图端点表 :55 usertime→user_utime；反馈⑤ 规格 #7 scope 计数主文+证据列 5→4 处；反馈⑥ 建文件规格 notes 键显式承载 prd AC3③「超期升级路径」栏位（gate2 r2 反馈⑥）。
- gate1 复位 pending 待复审 r3（只核 6 处消解——注意：本批已用 2/3 轮，r3 若再打回将达上限，修订前置条件=复审仅余注记级时按「通过+准出回填」呈出建议）。

---

2026-10-18 Gate#1 复审 r3 人类审批（AskUserQuestion，逐字转写）：
- 互审输入：reviews/gate1-ai-r3.md @ 941c457（reviewer-architect 0/0/1 + reviewer-qa 0/0/1；合并 0 BLOCKER/0 MAJOR/1 MINOR[两方同源一处]。r2 反馈 6 处全部实测消解：①四步五步双侧调和注记[auth/open.rs:6-18 流程图 5 步编号吻合]②TP-04 子锚 :158 实测 ③AC2③ 白名单与 TP-05 证伪面同面 ④:55 user_utime[open.rs:225/:430 实测]⑤scope 4 处实测 ⑥notes 承载 AC3③ 语义落位；§8 内联代跑质量经 r3 独立复核证实）。
- 问：「B2 资质申请 Gate#1 复审（r3，只核 r2 反馈 6 处消解；编排者内联代跑[SKILL §8 留痕]，r3 独立复核即代跑质量验证）：全部实测闭合；0 BLOCKER/0 MAJOR/1 MINOR（notes 溯源标签笔误[gate2→gate1]一行修）。如何裁决？（预置口径：仅余注记级时建议通过+准出回填）」
  答：「**通过+准出回填（推荐）**」
- 裁决落点（编排者执行）：
  1. gate1 = approved（本回执为 decisions.md 最新 GATE-APPROVED gate1，满足两次 GATE-REJECTED gate1 2026-10-18 之后的新鲜度要求），phase → construction。
  2. **准出回填清单**（批准条件式一行订正，编排者执行并核收留痕）：M-R3-1 溯源标签笔误两处——architecture.md 数据模型 notes 行「gate2 r2 反馈⑥」→「gate1 r2 反馈⑥」+decisions.md 核收条目同源笔误同步订正（decisions 只追加，订正以本条目声明为准）。
  3. 后续：phase=construction——Construction 执行=工程侧产出申请材料技术清单文档+tracking.md 建文件+人类执行 115 平台提交（AC2 核心动作）+受理跟踪口径在案（AC3）+AppID 接入预备清单（AC4）；`/aidlc:construct` 进入 Sprint Planning（A1 接入 intent——A3 拆分确认在派发前呈出）。

GATE-APPROVED gate1 2026-10-18
---

2026-10-18 Gate#1 准出回填核收（编排者内联执行——人类裁决「通过+准出回填」条件式一行订正）：
- **M-R3-1 落位**：architecture.md 数据模型 notes 行溯源标签「gate2 r2 反馈⑥」→「gate1 r2 反馈⑥」（实测 grep 唯一处改净）；decisions.md 核收条目内的同源笔误按只追加原则**不改写**——订正以本条目声明为准（该条目内「gate2 r2 反馈⑥」应读作「gate1 r2 反馈⑥」，出处=reviews/gate1-ai-r2.md @ 49bd976 MINOR-6）。
- gate1 裁决的全部准出条件闭合；phase=construction；下一步 `/aidlc:construct` 进入 Sprint Planning（A1 接入 intent——A3 拆分确认在派发前呈出；Construction 核心=工程侧材料清单+tracking.md 建文件+人类执行 115 提交）。

---
