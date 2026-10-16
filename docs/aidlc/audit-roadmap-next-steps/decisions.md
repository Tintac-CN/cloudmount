# Decisions: audit-roadmap-next-steps

> 只追加（Edit 被 guard-gates deny，一律 Write 全文+追加）；人类裁决、门控回执、降级事件逐字记录于此。

---

2026-10-16 A1 意图接入确认（AskUserQuestion 呈出未获选择；按协议缺省取值，编排者留痕并标注来源）：
- 意图原话（逐字，命令实参）：`请对现在的开发计划和功能实现做一次审核，开始计划下一步的实现和推进`
- 识别的意图一句话：对 `docs/ROADMAP.md`（开发计划）+ 现有功能实现做一次证据化审核，并据此规划下一阶段实现与推进。
- 粗粒度拆分预览（5 拆，逐字）：
  1. **路线图断言核验**——M0–M5 各 [x] 声明抽证（代码/测试/CI 证据对照），揪出「勾了但没验」项
  2. **计划-实现偏离扫描**——已见苗头：路线图写「Tauri 壳」而 desktop 实为 egui/eframe；`apps/shell-macos` 为占位 README；`.gitignore`/gen_icons 残留 src-tauri 死路径；web-ui 角色变迁
  3. **开放项+backlog 归集**——路线图未勾项（AppID 实测／真实凭据联调／RSS≤20MB／MSIX／macOS File Provider／Cookie 独有能力）+ 四个已完结 intent 的 P2 backlog ~20 项
  4. **质量与交付面体检**——277 测试+五断言脚本（112/72/23/13/9）+ 三 CI 工作流覆盖 + 已知边界（同秒漏检已关闭／未签名／aarch64-only）现状盘点
  5. **下一阶段路线图**——按依赖/风险/价值排序的推进计划（含是否启动 macOS File Provider、Cookie 独有能力、backlog 清理批次）
- 呈批三问（AskUserQuestion，逐字）：Q1「意图【审核开发计划+功能实现并规划下一步】——是否进入 AIDLC 生命周期？路线选哪个？」（进入 express（推荐）／进入 classic／调整拆分后再进／不进流程直接答复）；Q2「本期交付范围（影响 units 拆分规模）：」（审核+路线图（推荐）／审核+头部项实现／审核+清全 backlog）；Q3「审核的证据标准（决定发现可信度）：」（抽样验证（推荐）／全量验证／仅文档审）。**表单未获选择（user did not provide answers）。**
- 裁决落点（编排者，按协议缺省+意图字面取默认，来源逐项标注；**未推断任何人类偏好**）：
  1. **接入=是**：用户以显式 `/aidlc:inception` 命令 + 明确审核指令进入（命令即入口信号；非 auto-route 自然语言入口）。
  2. **route=classic（协议缺省）**：表单未选 express → 按 skill §3.0「用户未选 express 或未确认 → **缺省 classic**」取缺省（fail-safe 全量路径：三工件含 architecture.md、三方互审）。**后果注记**：协议禁止 classic→express 降级，缺省取值即锁定全量路径——人类若偏好 express 须在 gate1 前以裁决留痕复核（已列 intent.md 开放问题）。
  3. **交付范围=审核+路线图（编排者按意图字面取默认）**：意图原话「做一次审核，开始计划下一步的实现和推进」= 审核报告+下阶段计划；实现推进随后续 intent 逐个做。**待 gate1 人审过目**（intent.md 开放问题第 1 项）。
  4. **证据标准=抽样验证（编排者取默认）**：每里程碑 1-3 条关键声明跑证据 + 抽跑测试/CI 现状；**待 gate1 人审过目**（intent.md 开放问题第 2 项）。
- 后续：建 intent 骨架（intent.md/state.json/decisions.md）→ 按 classic 扇出 pm/architect/qa 三工件 → 对齐拍 → phase=gate1。

---

2026-10-16 Inception 对齐拍记录（编排者；含 §8 降级代行）：
- 三工件：prd @ be118d4（AC1..AC14 连续无空号）/ architecture @ 3e3feb7+align 88bf9b1 / test-strategy @ c3276a1+align（本条）。
- **architect 回填**（88bf9b1）：语义对齐表锚 AC1..AC14+补 AC12 缺行（原无对应动词行——由 §逻辑架构 CI 段+§安全与权限已知边界节承载）+prd 落盘事实订正；表与 prd 动词双向一致核过。
- **qa 回填（§8 内联代行）**：qa 代理停滞（对齐派发后 >20 分钟零产出，两次 600s 阻塞等待无果）→ 停单（TaskStop）→ 编排者内联代行映射回填（机械替换：26 TP 映射列+头注说明+缺口项勾销）；映射关系经 prd.md 正文逐条核对；**AC 覆盖机械自检**：26/26 行匹配，AC1..AC14 全覆盖无空锚（AC1←TP-12/17、AC2←TP-01/02、AC3←TP-03..06、AC4←TP-07/08、AC5←TP-09、AC6←TP-10、AC7←TP-11、AC8←TP-15/16/18/19/20、AC9←TP-19/20/24、AC10←TP-23、AC11←TP-21/22、AC12←TP-03/13/14/16、AC13←TP-06/24、AC14←TP-25/26）。
- 对齐拍结论：**工件齐且对齐**（classic 三件全落盘并提交；qa 引用 AC 编号 ↔ prd 实际编号一致[14/14]；architect 语义对齐表 ↔ prd 动词一致[含 AC12 补行]）。
- 相位转移：phase → gate1（gate1=pending），提示 `/aidlc:gate`（party-review classic 三件互审）。

---

2026-10-16 Gate#1 人类审批（AskUserQuestion，逐字转写）：
- 互审输入：reviews/gate1-ai.md @ be5b021（reviewer-pm 0/4/5 + reviewer-architect 0/3/5 + reviewer-qa 0/1/7 + reviewer-dev 0/4/3；合并 0 BLOCKER/12 MAJOR/20 MINOR，去重 10 MAJOR/16 MINOR；正面确认：AC↔TP 映射 14/14 无虚构编号、TP 引用抽验大面积命中、里程碑字面与 ROADMAP 逐字一致）。
- 呈批三问（AskUserQuestion，逐字）：
  问：「Gate#1 互审（审核+规划类 intent，classic 三件）：0 BLOCKER / 12 MAJOR / 20 MINOR。10 主题 MAJOR：M1 TP-01 证伪判据必触（S256 命名不匹配）/M2 AC2「M0 唯一未勾项」事实错误/M3 AC10 缺 L8/M4 非目标未钉零写入/M5 审核报告载体未钉/M6 AS-IS 漏登 merge job/M7 安全节缺 CI 权限面/M8 AC13 六项无断言载体/M9 testutil 备注误导/M10 cldflt 集成测试与只读约束矛盾。如何裁决？（route=classic 已随工件锁定，无法降级 express）」
  答：「**打回修订（推荐）**」
  问：「待你复核的默认值①交付范围（A1 编排者默认=审核+路线图）：」
  答：「**审核+路线图（推荐）**」（维持 A1 缺省）
  问：「待你复核的默认值②证据标准（A1 编排者默认=抽样验证）：」
  答：「**全量验证**」（升级，覆盖 A1 缺省）
- 裁决落点（编排者执行）：
  1. gate1 打回（gate1=rejected，phase 回 inception；第 1 轮/上限 3 轮）：修订点名 pm/architect/qa 三角色（dev 无 inception 工件——其发现 M4/M5/M9/M10 分别归 pm/qa 修订面）。
  2. **证据标准升级=全量验证（人类裁决，覆盖 A1 编排者缺省）**：沿 test-strategy 缺口节预铺的方法学差异——TP 从「每里程碑 1-3 条」扩为「逐 [x] 条目一条」，每条里程碑句内子声称逐一落证据；方法与判据不变，仅覆盖密度提升；审核报告方法节声明此口径。
  3. **交付范围确认=审核+路线图**：M4 按「零写入」钉死——审核期不回写任何仓库文件（含文档与 .gitignore），回写建议只进审核报告。
  4. 修订点名清单——**pm**：M2（AC2 事实订正：M0 两个未勾项 L8+L16）/M3（AC10 补 L8）/M4（非目标补「零写入」条款）/M5（交付载体钉死：审核报告建议=docs/aidlc/audit-roadmap-next-steps/audit-findings.md[沿 audit-p0p1-fixes 先例]，归集表与路线图载体/章节骨架由 pm 定；AC 层补完整性判据）/证据标准措辞（AC1 抽样改全量）/MINOR m1-m5（长期段措辞/分级口径统一/M2.5 引用形态/AC11 retro 面/U4 溯源）。**architect**：M6（AS-IS 补登 merge job=9 job）/M7（安全节补 CI/发布链权限面——workflow 级 permissions 缺省/第三方 action 可变引用/tag 触发面，三元组表述）/MINOR m6-m10（gen_icons 前提失效对账进 D 表/D2 清单订正/D4 补 api.ts 第三处/entries 列补全/edge-packaging 记「已分叉」）。**qa**：M1（TP-01 证伪重标定为实际符号 code_challenge/code_challenge_method/qrcode+Commands::Login；ROADMAP「PKCE(S256)」措辞差异列独立小发现）/M8（AC13 补批量逐项 TP）/M9（testutil 备注改写：workspace 裸跑，仅 -p cloudmount-sync 单包腿需 feature）/M10（显式声明：cargo test 复跑沿用基线口径含本机 cldflt 集成腿——该机即开发机、基线录制时已含，审核不新增写入面，报告如实标注）/全量升级落实（TP 扩为逐 [x] 条目+句内子声称；执行序注记/TP-04 行号订正/TP-16 锚改 AC12 或 AC12+AC9/自检行与表格对齐/AC9 新增偏离复核锚/AC14 模板随 pm 载体）。
  5. GATE-REJECTED 标记在后；修订完成后 gate1 复位 pending 复审（复审只核收敛点：10 MAJOR 消解+全量升级落实），须新回执。

GATE-REJECTED gate1 2026-10-16
---

2026-10-16 Gate#1 复审 r2 人类审批（AskUserQuestion，逐字转写）：
- 互审输入：reviews/gate1-ai-r2.md @ 0aa0048（reviewer-pm 0/1/2 + reviewer-architect 0/0/0 + reviewer-qa 0/0/2 + reviewer-dev 0/1/1；合并 0 BLOCKER/2 MAJOR/5 MINOR。上轮 10 MAJOR+16 MINOR 全部消解实证：43 条 [x] 清点与 ROADMAP 实读精确一致[grep -c=43]/AC1..AC15 覆盖机械终检通过/architect 侧全清/D1 区间锚处置判定正确[上轮建议 :48→:47 本身有误——:47 为代码围栏]）。
- 问：「Gate#1 复审（r2，只核收敛点）：上轮 10 MAJOR+16 MINOR 全部消解实证（含 43 条 [x] 清点与 ROADMAP 实读精确一致/AC1..AC15 覆盖机械终检通过/architect 侧 0/0/0 全清/D1 区间锚处置判定正确）；新发现 0 BLOCKER/2 MAJOR/5 MINOR（均字面级）。如何裁决？」
  答：「**通过+准出回填（推荐）**」
- 裁决落点（编排者执行）：
  1. gate1 = approved（本回执为 decisions.md 最新 GATE-APPROVED gate1，满足 GATE-REJECTED gate1 2026-10-16 之后的新鲜度要求），phase → construction。
  2. **准出回填清单**（批准条件式一行级订正，编排者执行并核收留痕，不重走复审）：M-R2-1 architecture.md 对齐表「本期默认抽样」残留→全量验证口径；M-R2-2 test-strategy TP-28 路径清单 {db,watcher,probe,queue,cache,executor}→实存模块 {store,state,engine,exec,queue,cache,watcher}；m1 architecture 只读面措辞补文档与 .gitignore；m2 「AC1..AC14」陈旧陈述→AC1..AC15（architecture+test-strategy 头注/分层节）；m3 test-strategy 头注 M2 分区枚举补 TP-18（2/12）；m4 TP-25/26 映射列补 AC15（联合承载回写）；m5 TP-27⑥ 行号区间订正（main.rs:48-51→:50-53）。
  3. 追认项打包（SKILL §8 降级代行记录，随本轮回执一并追认）：对齐拍 qa 回填（内联代行——代理停滞停单）+修订轮三工件与各轮编排者代提交（inception 角色工具集无 Bash）——全部产出已经 r2 四方独立复审实证，追认其效力。
  4. 后续：phase=construction，`/aidlc:construct` 进入 Sprint Planning（本 intent 经命令体 A1 式分级确认接入——A3 拆分确认在派发 dev 泳道前呈出）。

GATE-APPROVED gate1 2026-10-16
---

2026-10-16 Gate#1 准出回填核收（编排者内联执行——人类裁决「通过+准出回填」的条件式一行级订正）：
- **7 处全部落位**（机械 patch+复验零残留）：M-R2-1 architecture 对齐表「本期默认抽样」→全量验证口径（引 prd 头注「不得缩减」）；M-R2-2 TP-28 路径清单改实存模块 {store,state,engine,exec,queue,cache,watcher}（附落点对照：SQLite=store/state、change_probe=engine/remote、执行器=exec）；m1 只读面措辞补「亦不回写任何文档与 .gitignore（gate1 裁决② M4 条款）」；m2 「AC1..AC14」→「AC1..AC15」（architecture 对齐表头+test-strategy 头注/分层节三处）；m3 头注 M2 分区枚举补 TP-18（2/12）；m4 TP-25/26 映射列改 AC14/AC15；m5 TP-27⑥ 行号 :48-51→:50-53。
- gate1 裁决的全部准出条件闭合；phase=construction；下一步 `/aidlc:construct` 进入 Sprint Planning（A3 拆分确认将在派发 dev 泳道前呈出——本 intent 经命令体 A1 式分级确认接入，凭据=decisions.md A1 记录）。

---
