# Decisions: desktop-installers

> 只追加（Edit 被 guard-gates deny，一律 Write 全文+追加）；人类裁决、门控回执、降级事件逐字记录于此。

---

2026-10-16 A1 意图接入确认（AskUserQuestion，逐字转写——本 intent 首次表单即获选择）：
- 意图原话：`这个安装包不带前端界面不是搞笑吗，这是桌面端吗？无图标，无托盘，无界面，简直了`（逐字）。
- 识别的意图一句话：桌面端进安装包——MSI/dmg 装上有图标、有托盘、双击有界面的 cloudmount 桌面应用；自动化发布覆盖新产物面。
- 仓库事实（编排者核查，已呈用户）：`apps/desktop` 为 egui+wgpu+rfd 桌面壳（独立 cargo 项目，workspace exclude，二进制 cloudmount-desktop），**托盘依赖 tray-icon 0.21 已声明但源码零接线（grep 无命中）**、**应用图标从未生成**（scripts/gen_icons.py 输出路径指向不存在的 apps/desktop/src-tauri/icons，脚本已 stale）；上 intent（installer-packages，done）MSI 按 A1 旧裁决只装 daemon+cli、macos-release A1 排除 desktop——「无图标无托盘无界面」为两层缺口叠加（打包层没打+应用层没实现）。
- 呈批四问（AskUserQuestion，逐字）：
  问：「意图「桌面端进安装包：MSI/dmg 有图标有托盘有界面 + 自动化发布」——是否进入 AIDLC 生命周期？路线选哪个？」
  答：「进入 express（推荐）」
  问：「Windows 安装包形态（desktop 进包方式）：」
  答：「同一 MSI 三件套（推荐）」
  问：「mac 安装包形态（desktop 应用分发方式；daemon/cli 既有 pkg 保持不变）：」
  答：「.app 进 .dmg（推荐）」
  问：「应用层缺口（托盘接线/图标嵌入）是否纳入本期范围：」
  答：「托盘+图标纳入（推荐）」
- 裁决落点（编排者执行）：建 intent `desktop-installers`（route=express）；Windows = 同一 MSI 三件套（daemon+cli+desktop 铺 Program Files\CloudMount，开始菜单/桌面快捷方式指向 desktop exe）；mac = `cloudmount.app` 打 `.dmg`（daemon/cli 既有 pkg 不动）；应用层最小集纳入本期（gen_icons 重写 ico/png/icns 管线 + winres 嵌 exe 图标 + egui 托盘最小接线：显示主窗口/退出两项）；升级语义沿 installer-packages 非目标（不设计，MajorUpgrade 向前覆盖+提示先卸载再装）；上 intent 已锁脚本零触碰，新断言落独立文件；Inception 不派 architect，语义对齐并入 prd.md 内联小节。

---

2026-10-16 Inception 对齐拍记录（编排者）：
- express 两工件齐：prd.md（6 AC + 语义对齐内联小节 + Construction 首日验证清单 4 项（tray-icon+eframe 集成/winres 构建/icns 生成链/关闭按钮拦截 API——B1 教训不断言可行）+ 开放问题 Q1 关闭语义/Q2 快捷方式范围）+ test-strategy.md（24 测试点 TP-01..TP-24，P0=16/P1=6/P2=2；负路径：needs 门控+无 continue-on-error+零删改护栏+图标缺失红+pkg 无 .app+卸载干净+发布前清单）。
- **对齐拍检出全量映射错位**：qa 映射按 A1 占位 5 项锚定（prd 实际 6 条 AC 边界不同：占位 AC1=MSI 三件套↔prd AC3、占位 AC3=托盘↔prd AC2、占位 AC4=图标管线↔prd AC1）——两角色派回做全量重映射（TP→prd AC 逐条定锚）+ pm 补六条 AC 测试锚行。
- 对齐后核对：AC 引用集合 = prd 实际集合 = {AC1..AC6}，无虚构编号；全部 AC 有 T 点落点（AC1←TP-06/11/12/16、AC2←TP-09/20/21、AC3←TP-03/04/05/13/14/15/17/18/23、AC4←TP-07/08/19/23/24、AC5←TP-01/02/18/22、AC6←TP-10/03），无缺锚。
- 相位转移：两工件提交 → phase 置 gate1（express 评审集合 = prd + test-strategy 两件），提示 `/aidlc:gate`。

---

2026-10-16 Gate#1 人类审批（AskUserQuestion，逐字转写）：
- 互审输入：`reviews/gate1-ai.md` @ 8a6eec0（reviewer-pm 1/2/8 + architect 2/4/6 + qa 0/4/4 + dev 2/4/6；合并 2 BLOCKER / 8 MAJOR / 15 MINOR）。
- 呈批四问与用户裁决：
  问：「Gate#1 互审：2 BLOCKER（B1 锁定断言 T4 硬冲突需授权锁修订；B2 AC5④ 零触碰与 AC3 三件套物理矛盾）+8 MAJOR（icns 矛盾/RGBA 解码链缺环/改动面漏项/dmg 命名收编约束未声明等）+15 MINOR。如何裁决？」
  答：「打回修订（推荐）」
  问：「B2 裁决：desktop exe 进 MSI 的 job 拓扑（连带 AC5④/TP-03/AC6② 口径重述）：」
  答：「MSI job 扩口径（推荐）」
  问：「B1 裁决：锁定断言 installers-yaml.test.sh T4 两条负断言（无 desktop 字面）的处置：」
  答：「授权锁修订（推荐）」
  问：「开放问题 Q1（关闭按钮语义）/Q2（快捷方式范围）拍板（修订轮回填字面）：」
  答：「隐藏托盘+两处（推荐）」
- 裁决落点（编排者执行）：
  1. gate1 打回（gate1=rejected，phase 回 inception）：只重跑 pm+qa 修订被点名项，对齐拍后 gate1 复位 pending 复审（重点核对 B1/B2/M1/M2/M7 消解）。
  2. **B2 裁决（job 拓扑）**：installer-msi-windows 从零触碰集合移出——精确口径 = 「既有构建命令/铺放语义/upload if-no-files-found 行不删改，允许追加 desktop 构建与归集步+needs/body 行追加」；AC5④/TP-03/AC6② 同步改口径；desktop exe 构建落该 job 内（apps/desktop 独立 cargo 构建）。
  3. **B1 裁决（锁修订授权）**：沿 T16 先例授权修订 installers-yaml.test.sh T4 两条负断言字面（「不含 desktop 字面」→ 精确化「daemon/cli 铺放组件除外」或等价形式）；修订时于 decisions.md 留痕授权依据（本条目），修订后重锁。**注意**：本 intent 尚未进入 Construction，无既有锁；该修订属本期 Construction 期间对该上 intent 工件的授权改动。
  4. **Q1 裁决**：关闭按钮 = **隐藏到托盘**（托盘「退出」真退出）；eframe 0.31 落码形态 = `ctx.input(viewport().close_requested)` + `ViewportCommand::CancelClose`（dev 评审 m1 已核 API 实证）入首日验证④。
  5. **Q2 裁决**：快捷方式 = **开始菜单 + 桌面两处**。
  6. 修订范围点名——pm：B2（AC5④ 口径重述）/B1（AC3④/AC6② 措辞同步）/M5（release body 产物表与引导更新入改动面）/M4（dmg 命名收编约束声明）/Q1/Q2 裁决回填（开放问题转已裁决）/M8（首日验证补降级路径：④关闭语义 API 不可行回退=真退出+托盘两项、①tray 集成不可行升级人类决策）+MINOR（多实例已知行为声明/托盘失败路径/单架构覆盖落字/dmg 架构落字/body 锁粒度/版本显示不一致知悉）；qa：M6（P0 门语义改「红态准入+全绿准出」）/M7（TP-16 可判定式替换 ExtractAssociatedIcon 非空+TP-09 恰两项可判定式钉死）/M1（TP-11 icns 项按首日验证③二选一落位后断言）/MINOR（TP-05/15 负断言/TP-04 32 位目录负断言/TP-03 实际 job 名映射+快照基线/TP-22 清单载体钉仓库内工件/TP-24 补 Windows SmartScreen 侧/头部缺口节同步/计数如实）。
  7. GATE-REJECTED 标记在后；修订轮完成后 gate1 复位 pending 复审。

GATE-REJECTED gate1 2026-10-16

---

2026-10-16 Gate#1 修订轮记录（编排者）：
- 修订范围（只重跑被点名角色 pm+qa；其余工件零改动；修订提交见 git log）：
- **prd.md**：B2（AC5④ 口径重述——installer-msi-windows 自零触碰集合移出，守护收窄为「既有构建命令/铺放语义/upload if-no-files-found 行不删改，允许追加 desktop 构建与归集步 + needs/body 行追加」；AC3 Given 注明同 job 追加步产出三件套）/B1 同步（AC3④/AC6② 注明 T4 断言字面收窄已获授权，非目标补唯一例外句）/M5（AC5⑤ release body 4 行口径：MSI 行改三件套描述+新增 .dmg 行含 Gatekeeper 引导；AC4② 补 .app Gatekeeper 引导）/M4（AC5②+语义对齐补新 artifact 名 `cloudmount-` 前缀收编约束）/Q1/Q2 裁决回填（开放问题转已裁决：隐藏到托盘/两处快捷方式）/M8（首日验证④ 补 dev m1 实证 eframe 0.31 无 on_close_event、官方路径=input close_requested+CancelClose；降级路径=关闭即真退出+托盘两项；① 不可行=升级人类决策）/MINOR（多实例双开已知行为、托盘失败降级不 panic、mac desktop/.dmg aarch64-only 已知取舍、AC3③ 先卸载再装提示）。
- **test-strategy.md**：M6（「风险排序门」改「进出门（TDD 红→锁→绿对齐）」——P0 全部落位且可运行（红态）方可进 Construction，全绿为准出门）/M7（TP-16 改 RT_GROUP_ICON 资源枚举/位图采样品牌色可判定式；TP-09 钉托盘 init 区段 MenuItem 计数=2+两 id MenuEvent 匹配+无其他 tray Menu 构造）/M1（TP-11 icns 项按首日验证③结论落位后断言）/MINOR（TP-05/15 负断言补、TP-04 32 位目录负断言、TP-03 快照基线+实际 job 名映射、TP-22 载体钉仓库内附录文件、TP-24 Windows SmartScreen 侧）；头部/覆盖核对/缺口节同步；计数如实不变 P0=16/P1=6/P2=2。
- 修订轮对齐拍：映射未变无需重对齐（本轮回填仅措辞/枚举/锚注），prd 锚注与 test-strategy 映射列双向一致（AC1←TP-06/11/12/16、AC2←TP-09/20/21、AC3←TP-03/04/05/13/14/15/17/18/23、AC4←TP-07/08/19/23/24、AC5←TP-01/02/18/22、AC6←TP-10/03）。
- 相位转移：修订轮两工件提交 → gate1 复位 pending（重审以新一轮为准；GATE-REJECTED gate1 2026-10-16 之后须新回执），phase → gate1。复审重点核对 B1/B2/M1/M2/M7 消解。

---

2026-10-16 Gate#1 复审人类审批（AskUserQuestion 首次呈批未获选择；用户对话逐字裁决，逐字转写）：
- 互审输入：`reviews/gate1-ai-r2.md` @ 37bfc49（reviewer-pm 0/4/3 + architect 0/4/5 + qa 0/2/5 + dev 0/3/8；合并 0 BLOCKER / 4 MAJOR / 12 MINOR。上轮 2 BLOCKER/8 MAJOR 大部消解实证——B1 授权链/B2 拓扑钉死均确认）。
- 呈批三问（AskUserQuestion 呈出，用户对话逐条裁决）：
  问：「Gate#1 复审（r2）：上轮 2 BLOCKER/8 MAJOR 大部消解实证（B1 授权链/B2 拓扑钉死均确认）；新检出 4 MAJOR（B2 口径扫荡残留/AC5⑤ 4 行口径不可判定/RGBA 解码链延留认领/body 编辑授权相抵）+ 12 MINOR——均为措辞/枚举/锚注级，一轮轻量修订可闭合。如何裁决？」
  答：「打回第二轮轻量修订」
  问：「RGBA 解码链选型（M-R2-4：tray-icon Windows 侧/egui 均需原始 RGBA，png 解码依赖缺位）：」
  答：「gen_icons.py 增发 Rust RGBA 常量源文件」
  问：「intent 改动面补齐授权（M-R2-4 延留认领 + B1 锁修订的文件面）：」
  答：「齐 apps/desktop/Cargo.toml + build.rs + icons/ + installers-yaml.test.sh T4 修订授权——消除 Construction 守卫误判越界风险」
  （body 产物表口径随第一问推荐选项一并裁决：**8 行口径——7 既有行零删改 + MSI 行描述更新 + 追加 .dmg 行，TP-01 补 body 锚**）
- 裁决落点（编排者执行）：
  1. gate1 第二次打回（gate1=rejected，phase 回 inception；第 2 轮/上限 3 轮）：只重跑 pm+qa 修订被点名项，修订后复审 r3 只核收敛点。
  2. **RGBA 解码链裁决**：`gen_icons.py` 增发 Rust RGBA 常量源文件（icons.rs：尺寸+RGBA 字节组，apps/desktop include 后喂 tray-icon `Icon::from_rgba`/egui `IconData`）——零新增 crate 依赖，供应链面最小；产物面从 `{icon.ico,*.png,*.icns}` 扩为含 `icons.rs`。
  3. **intent 改动面补齐授权**：intent.md 改动面补 `apps/desktop/Cargo.toml`+`build.rs`（winres）+`apps/desktop/icons/`（图标产物含 icons.rs）+ `scripts/installers-yaml.test.sh`（T4 断言字面收窄——B1 授权，Construction 期执行）。
  4. **body 产物表口径裁决**：8 行口径——7 既有行零删改 + MSI 行描述更新 + 追加 .dmg 行；TP-01 补 body 锚。
  5. 第二轮修订范围点名——pm：M-R2-2（AC5⑤ 三处「4 行口径」改 8 行逐行枚举）/M-R2-3（AC5③ 措辞扩「body 行追加及 AC5⑤ 所指 MSI 行描述更新」）/M-R2-4-part1（语义对齐「有图标」补 RGBA 解码链声明+产物面扩 icons.rs）/MINOR（AC5④ 删「及 needs/body 行追加」授权噪声、AC5① 改「新增 macos-15 job」、追加步次序声明、AC2④/TP-21 降级连锁锚注记、main.wxs L8-11 头注更新注记）；qa：M-R2-1（L13 四 job 口径+TP-01/TP-02/L55 拓扑措辞改追加步+AC6 锚双向一致 TP-03 映射列改 AC3/AC6+L49 同步）/M-R2-2（TP-01 补 body 8 行锚）/M-R2-3（TP-03 注明 body 按 AC5⑤ 为断言对象）/M-R2-4（TP-11 补 icons.rs 存在性断言）/MINOR（TP-04 边界化、TP-09 区段定位+MenuEvent 轮询、TP-21 降级镜像、TP-03 精确清单、TP-11 icns 首日定稿豁免+多尺寸对齐、四锁定脚本精确文件名）。
  6. GATE-REJECTED 第二次标记在后；修订轮完成后 gate1 复位 pending 复审 r3（只核收敛点）。

GATE-REJECTED gate1 2026-10-16

---

2026-10-16 Gate#1 修订轮 r2 记录（编排者）：
- 修订范围（只重跑被点名角色 pm+qa；intent.md 改动面补齐由编排者执行 @ 8eb5181；修订提交 @ d1294e3）：
- **prd.md**：M-R2-2（AC5⑤ 本体/AC5 验证层/语义对齐三处「4 行口径」全部改 8 行逐行枚举——既有 7 行零删改+MSI 行描述更新+追加 .dmg 行；两层可测性总拆分同步）/M-R2-3（AC5③ 扩「body 行追加及 ⑤ 所指 MSI 行描述更新为授权改动（gate1 r2——就地改写仅限 MSI 行描述，其余既有 7 行零删改）」）/M-R2-4-part1（语义对齐「有图标」补 icons.rs 声明——gen_icons.py 增发 Rust RGBA 常量源（尺寸+RGBA 字节组，apps/desktop include 后喂 tray-icon Icon::from_rgba/egui IconData，零新增 crate 依赖）；产物面扩含 icons.rs；AC1① 同步补 icons.rs 产物项；.png 角色改为源素材）/MINOR（AC5④ 删「及 needs/body 行追加」噪声、AC5① 及 Given 改「新增 macos-15 job」、AC3 Given 补追加步次序约束（desktop 构建+归集步须插在校验/candle 之前）、AC2④ 补降级连锁锚、AC3④ 补 main.wxs L8-11 头注失实更新注记）。
- **test-strategy.md**：M-R2-1（L13 前提节改四 job 零触碰口径+TP-01 改「Windows=installer-msi-windows 追加步+mac=新增 macos-15 job」+TP-02 改「追加步无 step 级 continue-on-error+新 mac job 无 continue-on-error」+分层策略同步+**AC6 锚双向一致**：TP-03 映射列改 AC3/AC6+L49 覆盖核对「AC6←TP-10/TP-03」同步）/M-R2-2（TP-01 补 body 断言锚——产物表 8 行口径：既有 7 行零删改+MSI 行含三件套描述+.dmg 行存在+Gatekeeper 引导文案存在）/M-R2-3（TP-03 注明 body 产物表按 AC5⑤ 为断言对象而非护栏对象）/M-R2-4（TP-11 补 icons.rs 存在性断言）/MINOR（TP-04 负断言边界化（子串问题，Id 定界或先滤 64Folder）、TP-09 init 区段按 fn 名/tray 初始化调用所在函数钉死+MenuEvent 全局 receiver 轮询形态入断言面、TP-21 注首日验证④连锁锚、TP-03 精确四 job 清单（无「等」）、TP-11 icns 子断言注首日验证③定稿豁免+尺寸口径改「≥1 帧（基线 32×32 单帧）+PNG 32/128」、四锁定脚本清单改精确文件名）；头部增 gate1 r2 裁决落点+缺口节新增 icons.rs 产物面条目；计数如实不变 P0=16/P1=6/P2=2。
- 修订轮 r2 对齐拍：映射未变（TP-03 映射列扩 AC3/AC6 为锚注双向一致修复），prd 锚注与映射列双向一致。
- 相位转移：修订轮 r2 两工件提交（@ d1294e3）→ gate1 复位 pending（重审以新一轮为准；GATE-REJECTED gate1 2026-10-16 第二次之后须新回执），phase → gate1。复审 r3 只核收敛点：M-R2-1/2/3/4。

---

2026-10-16 Gate#1 复审 r3 人类审批（AskUserQuestion，逐字转写）：
- 互审输入：`reviews/gate1-ai-r3.md` @ 70b635e（reviewer-pm 0/0/1 + architect 0/0/5 + qa 0/0/3 + dev 0/0/1；合并 0 BLOCKER / 0 MAJOR / 6 MINOR。上轮 4 MAJOR 全部消解实证——B2 口径扫荡+AC6 锚修复/8 行口径算术自洽/授权三面闭合/icons.rs 消费端自洽；四方一致建议通过）。
- 问：「Gate#1 复审（r3，只核收敛点）：上轮 4 MAJOR 全部消解实证（B2 口径扫荡+AC6 锚修复/8 行口径算术自洽/授权三面闭合/icons.rs 消费端自洽）；新检出 0 BLOCKER / 0 MAJOR / 6 MINOR（括注枚举 off-by-one/子锚绑定/断言面措辞——均词级）。如何裁决？」
  答：「通过（推荐）」
- 裁决落点（编排者执行）：
  1. gate1 = approved（本回执为 decisions.md 中最新 GATE-APPROVED gate1，2026-10-16 两次 GATE-REJECTED gate1 的新鲜度要求均由本回执满足），phase → construction。
  2. **6 MINOR 授权随 Construction 首日断言落笔顺带闭合**（gate1 r3 先例）：①AC5⑤/语义对齐括注「linux×2」改「1 行 glob 覆盖双 target」+AC5③「其余既有 7 行」改「其余 6 行」（off-by-one）②TP-01「Gatekeeper 引导文案存在」子锚绑定 .dmg 行（消朴素 grep 假绿）③TP-02 补追加步 step 级 `if:` 断言面（对齐 L56 策略层）④intent.md L22 约束行补 T4 例外注记（与 L13 改动面授权一致）⑤test-strategy L13「desktop job」改「desktop 面（追加步+新增 job）」⑥icon.ico CI 侧来源（入仓）随 Construction 首日落字。
  3. 遗留观察备案：winres 供应链披露句/元数据 owner 不一致/MSI job 构建来源/UTF-8 description 风险——随 Construction 首日一并落。
  4. 后续：phase = construction，`/aidlc:construct` 进入 Sprint Planning（A1 接入 intent——A3 拆分确认表单在派发 dev 泳道前呈出）。

GATE-APPROVED gate1 2026-10-16

---

2026-10-16 A3 拆分确认（AskUserQuestion 首次呈批未获选择；用户对话逐字裁决，逐字转写）：
- 呈批对象：plan.md @ e621ef6（5 units 串行 TDD——U1 安装包结构断言脚本红/U2 应用层[图标管线+winres+托盘接线，含首日验证 4 项]/U3 Windows MSI 扩展[含 T4 授权锁修订+本地 WiX 冒烟]/U4 mac .app+.dmg+release body/U5 终验收尾；验证命令=cargo test --workspace，基线 26 目标 277 passed 0 failed（@ 525c55a 记录）；gate1 r3 授权 6 MINOR 已首日回填 @ 8e21992；串行派发沿限流教训）。
- 问：「A3 拆分确认（intent desktop-installers，5 units 串行 TDD）：是否批准开始施工？」
  答：「批准」
- 裁决落点（编排者执行）：PLAN-CONFIRMED 回执在后；开始串行派发 U1→U2→U3→U4→U5。

PLAN-CONFIRMED 2026-10-16

---

2026-10-16 U1 红阶段完成并置锁（编排者核收）：

- 交付：`scripts/desktop-installers-yaml.test.sh` @ 2a16a3a（831 行，仅此一文件）；**TP-01..TP-24 静态面合计 111 断言，红态 58 passed / 53 failed**（53 红全为 desktop 面断言——被测对象缺失/零接线即 FAIL 防空真；58 绿为既有护栏现状绿）。
- 落位口径（dev 处置留痕）：TP-13..TP-17 集成面归 U3（本阶段仅结构子集）；TP-19..TP-23 E2E/人工面头注注明非本脚本面；TP-18 结构子集由 needs/upload 断言承载；5 项契约歧义处置——①TP-04 32 位负断言改 Id 定界（「先滤 64Folder」实测假红：现行 wxs 注释含裸字面）②红阶段钉死字面（installer-dmg-macos 新 job 名/cloudmount-desktop.dmg/--manifest-path 构建形态/菜单 id "show"/"quit"/图标产物名沿 gen_icons 基线）③TP-03 快照对照以字面锚定集承载（U5 另做全量基线比对）④TP-09 采全文件计数界定（严格强于函数区段界定）⑤追加步次序=行号比较断言。
- TDD 锁置位 @ 8d838a9：tddLock=true，tddLockPaths=["scripts/desktop-installers-yaml.test.sh"]（并集只增）；U1 完成判据（红提交+锁）达成 → unitStatus.U1=completed。
- 回归证据：四既有锁定脚本全绿（零触碰）；`cargo test --workspace` 277 passed/0 failed（与基线逐数一致）。

---

2026-10-16 U2 应用层完成核收（编排者）：
- 提交：c8521cc（U2-icons：gen_icons 重写+四件套入仓）/ 3bbbec3（U2-winres：build.rs+winresource）/ 533e456（U2-tray：main.rs 托盘接线）；改动面 10 文件全在授权范围（scripts/gen_icons.py + apps/desktop/{Cargo.toml,Cargo.lock,build.rs,src/main.rs,icons/×5}）。
- **首日验证 4 项结论**（units/2.md 开工执行要求，留痕于提交信息+此处）：
  ① tray-icon 0.21+eframe 0.31 集成形态=**可行**：初始化落点=首次 update()（主线程+事件循环已运行，macOS 主线程硬约束满足；Windows 同事件循环线程）；菜单事件=update() 内全局 receiver 轮询（`MenuEvent::receiver().try_recv()`，帧节奏驱动，与既有 1s request_repaint_after 兼容）。
  ② winres 构建=**定稿选 winresource**（winres 0.1.12 停更于 2021-09-28；winresource 0.1.34 @ 2026-10-12 活跃维护、WindowsResource API 兼容）；build.rs 目标门控 CARGO_CFG_TARGET_OS=windows（mac 跳过）；嵌入实证=exe RT_GROUP_ICON id=1+RT_ICON×4（多帧 16/32/48/256）。
  ③ icns 生成链=**定稿 stdlib 手写 PNG-in-ICNS 预生成入仓**（icon.icns=ic11 32+ic07 128+ic08 256；免 CI iconutil 依赖，.app bundle 直接携带）。
  ④ CancelClose API=**可行**（egui 0.31.1 `ViewportInfo::close_requested()`+`ViewportCommand::CancelClose`，源码核实+实测），**未触发降级**；但配套实测发现 `Visible(false)` 真隐藏两缺陷（见偏差条目）——落码形态修订为「CancelClose+伪隐藏」。
- **偏差留痕（物理形态；未触发连锁锚修订）**：units/2.md 契约字面「CancelClose+Visible(false)」，实测后改「CancelClose+伪隐藏（离屏 OuterPosition(-32000,-32000)+WS_EX_TOOLWINDOW 去任务栏/Alt-Tab，Win32 FFI 直连 user32，零新 crate）」。两项实测缺陷（3 探针+winit/eframe 源码佐证）：① `Visible(false)` 后窗口不再收 WM_PAINT（winit 源码注释：WM_PAINT 投递要求窗口可见）→ update() 停帧，菜单轮询/恢复失联；② 专用线程轮询绕开①后，重绘到期 eframe 事件循环进入 ControlFlow::Poll 且 egui 重绘请求在无帧期被抑制→实测隐藏态 CPU 5.016s/5s（100% 单核自旋）。伪隐藏保持 WS_VISIBLE→帧流不断（实测隐藏 5s CPU 0.016s）、用户不可见（离屏+去任务栏）、恢复=位置复位+去 TOOLWINDOW+Focus。**语义与 AC2④/TP-21 完全一致**（关闭=隐藏到托盘/托盘常驻/托盘「退出」真退）——未触发降级（「关闭即真退出」），故 AC2④/TP-21 字面连锁锚修订未激活：prd AC2④ 钉死的 CancelClose 落码形态保留成立，test-strategy TP-21「按首日验证④落定语义核验」即按本条落定；物理形态差异备案本条目供 gate2 核验。
- **核收证据（编排者独立复跑）**：desktop-installers-yaml.test.sh **73/111**（红 58→73：应用层 +15 全转绿——TP-06×1/TP-09×7/TP-11×5/TP-12 build.rs×2；剩余 38 红全为 MSI/dmg 面：TP-01×16/TP-02×6/TP-04+13 wxs×1/TP-05×5/TP-07×6/TP-12 wxs+dmg×4，归 U3/U4）；`cargo test --workspace` **277 passed/0 failed**（与 testBaseline 逐数一致）；`cd apps/desktop && cargo build --release` 零警告（8.1MB exe 在位）；四既有锁定脚本回归全绿（installers-yaml 72/72 等）；gen_icons 幂等（二次运行 5 产物全 unchanged）；全仓 `src-tauri` 引用清零。
- **本地冒烟（自动化部分）**：启动→窗口可见→WM_CLOSE→窗口离屏(-32000)+TOOLWINDOW 位置位+进程存活（关闭=隐藏到托盘）；隐藏 5s CPU 0.016s；帧流保持+恢复原语（位置复位/去 TOOLWINDOW）实证有效。**不可自动化如实标注**：真实托盘图标点击（「显示主窗口」/「退出」）——托盘图标非可编程 HWND，待人工冒烟（TP-20/21 真机项）。
- **遗留观察**：a) macOS 无 WS_EX_TOOLWINDOW 等效步→伪隐藏后 Dock 图标留存（已知差异，待真机核验/后续处理）；b) raw-window-handle 0.6 直声明（原为既存传递依赖，零新供应链面）；c) 本机 OneDrive 目录 R 属性触发 autocfg 严格可写误报（清属性后过——环境注记，非代码问题，CI 不受影响）。
- unitStatus.U2=completed；下一步 U3（含 T4 授权锁修订——解锁窗口随 U3 派发前开启，见下条）。

---

2026-10-16 UNLOCK-TESTS（T4 授权锁修订窗口开启；授权依据=gate1 B1 人类裁决，逐字复引）：
- 人类原话（2026-10-16 Gate#1 首次打回，AskUserQuestion 逐字）：问「B1 裁决：锁定断言 installers-yaml.test.sh T4 两条负断言（无 desktop 字面）的处置：」答「**授权锁修订（推荐）**」。
- 裁决落点复引（同日 Gate#1 条目裁决落点 3，逐字）：「沿 T16 先例授权修订 installers-yaml.test.sh T4 两条负断言字面（「不含 desktop 字面」→ 精确化「daemon/cli 铺放组件除外」或等价形式）；修订时于 decisions.md 留痕授权依据（本条目），修订后重锁。」
- 解锁窗口范围：`scripts/installers-yaml.test.sh` **仅 T4 两条负断言**（L259-264 一带，「无 desktop 字面」→ 精确化等价形式）；其余断言零触碰；修订后本文件重归零删改基线（TP-03 口径）。
- 机制注记：该文件之锁属上 intent（installer-packages，phase=done）；本 intent（desktop-installers）为活跃 intent——override 记于本 intent state.json：`tddLockOverride={by:"human", at:"2026-10-16", reason:"gate1 B1 裁决授权的 T4 两条负断言字面收窄（首次打回逐字『授权锁修订（推荐）』；裁决落点 3 沿 T16 先例）」}`。修订完成后清除 override 并留 LOCK-TESTS 条目（义务：U3 核收时执行，不得拖延——长期不重锁等于契约裸奔）。
- 附带说明：guard-gates 的 TDD 锁 deny 经查仅命中 `isTestFile()` 匹配的测试文件（.test.sh 不在其列）；本解锁属**流程纪律遵循**（授权→窗口→修订→重锁留痕），非 hook 强制——纪律不因机制宽松而免除。

---

2026-10-16 U3 Windows MSI 扩展完成核收（编排者）：
- 提交 9384be4（3 文件 +72/-14：packaging/wix/main.wxs / .github/workflows/release.yml / scripts/installers-yaml.test.sh[T4 授权修订]）。
- **T4 授权锁修订执行实录**（解锁窗口 @29c5dd5 开启 → 本修订 → 重锁见下条）：仅 T4 两条负断言（单 hunk）；①MSI job：原「整块不含 desktop」→「daemon/cli 既有编译步+upload 步 desktop-free」（步骤语义行判定、注释叙述不计）；②main.wxs：原「整文件不含 desktop」→「daemon/cli 三既有铺放组件（CloudMountdComponent/CloudmountCliComponent/MachinePathEnv）desktop-free」（Id 定界提取、空块即红防空真）；ServiceInstall 两条零触碰；非恒真注入验证（副本注入 desktop 字面→双双捕获）；修订后 72/72 绿。
- **本地 WiX 冒烟（TP-13..17+TP-18 注入）**：before=0.2 在位 → candle/light 全绿（全量 ICE 零抑制）→ msiexec /i /qn exit 0（MajorUpgrade 覆盖 0.2→0.3.0）→ 装后验证=三 exe 在位/两处快捷方式（开始菜单+公共桌面；Target=[INSTALLDIR]cloudmount-desktop.exe、IconLocation=MSI Installer cache）/desktop exe RT_GROUP_ICON id=1+RT_ICON×4/Machine PATH 含安装目录（尾反斜杠形态）/ARP DisplayVersion=0.3.0/安装副本启动+关闭隐藏到托盘 → msiexec /x /qn exit 0 → 卸载全清（目录/快捷方式 4 候选位置/PATH/HKCU+HKLM 键/ARP/无进程）→ TP-18 注入（BinDir 缺 desktop exe/icon.ico）light LGHT0103×2 exit 103 零 MSI 产出。终态=本机无 CloudMount（0.2 被覆盖后连同 0.3.0 一并卸载——A3 批准冒烟链路的预期终态；正式 tag 发布后重装新版）。
- ICE43/ICE57 处置：首版 File 嵌套 Shortcut 触发 CNDL0062（File 不允许嵌套 @Target）；改组件级后触发 ICE43/ICE57（快捷方式组件混装）→ 拆独立快捷方式组件（CloudMountShortcutsComponent，HKCU 注册表 KeyPath）；candle/light 全绿零抑制（沿 T16 全量 ICE 口径）——U1 钉死字面（Shortcut 恰两处/Target 含 desktop exe/两目录）全部保持。
- 核收证据（编排者独立复跑）：installers-yaml **72/72**、release-yaml **23/23**、check-macos-yaml **13/13**、desktop-installers-yaml **87/111**（余 24 红全为 dmg/body 面=U4）、`cargo test --workspace` **277/0**；机器状态复查=无 CloudMount 残留（目录无/ARP 0 条）。
- 施工注记：candle 对 File@Source 不校验存在性，fail-fast 实际落点=light LGHT0103+CI 校验步（TP-12/18 断言已覆盖）；build/ 为本地冒烟产物目录（本地 MSI/日志/脚本）——加 `/build/` 入 .gitignore（本地 scratch 不入仓；保留本地 MSI 供试装）。
- unitStatus.U3=completed。

---

2026-10-16 LOCK-TESTS（T4 修订后重锁；解锁窗口关闭）：
- 修订已按授权完成（commit 9384be4，上条实录）：仅 T4 两条负断言字面收窄，其余断言零触碰；修订后 installers-yaml.test.sh 72/72 绿。
- state.json：tddLockOverride 清除（置 null）；tddLock 保持 true（tddLockPaths=[scripts/desktop-installers-yaml.test.sh] 不变，并集只增）。
- **修订后字面纳入零删改基线（TP-03 口径）**：installers-yaml.test.sh 自本条目起重归零删改状态；后续任何改动（含 U5 终验）均须新的授权解锁。

---

2026-10-16 U4 mac .app+.dmg+release body 完成核收（编排者）：
- 提交 e7301ed（2 文件 +181/-2：scripts/package-dmg-macos.sh[新增 149 行] / .github/workflows/release.yml）。
- **断言全绿里程碑**：desktop-installers-yaml.test.sh **111/111 exit 0**（U4 前余 24 红全部转绿）；四既有锁定脚本零回退（installers-yaml 72/72 / release-yaml 23/23 / check-macos-yaml 13/13 / package-macos 9/9——编排者独立复跑一致）。
- 交付要点：package-dmg-macos.sh=staging cloudmount.app bundle（Contents/{MacOS/cloudmount-desktop, Resources/cloudmount.icns, Info.plist[CFBundleName/Identifier/Icon/Executable/LSMinimumSystemVersion 等]}）+前置硬校验 fail-fast（binary -x/icns 缺失 exit 1/非 dry-run 缺版本 exit 2）+dry-run 段（本地实测 exit 0：staging 三件在位+plist 键集+版本注入 9.9.9 实证）；hdiutil 装配段（含 ln -s /Applications）mac 独占置 dry-run 门后（R2-MAJOR-1 边界）；release.yml 新 job installer-dmg-macos（macos-15/aarch64 显式 target/调脚本注入 tag 版本/upload 精确路径 cloudmount-desktop.dmg+if-no-files-found: error+artifact cloudmount-desktop-dmg 前缀）；release needs 仅尾部追加（既有五元素零删改）；body 8 行（MSI 行三件套描述更新+.dmg 行含 Gatekeeper 绑定该行+其余 6 行零删改）。
- 遗留观察：a) **mac CI 首次构建**——desktop 应用此前从未 mac 编译，本机交叉 check 受阻于 ring 需 darwin C 工具链（环境限制非代码问题），实际以 CI macos-15 为准（发布前首跑即知）；b) actionlint 本机不可用降级路径（yaml.safe_load 通过+诚实计数）。
- unitStatus.U4=completed；下一步 U5 终验。

---

2026-10-16 U5 终验收尾完成核收；Construction 收口，phase → gate2（编排者）：
- 提交 5c685e5（1 文件 +137：docs/aidlc/desktop-installers/release-checklist.md——TP-22 载体固化）。
- **终验组合五项全绿**（dev 执行+编排者独立复跑关键项一致）：`cargo test --workspace` **277 passed/0 failed**（26 目标，与 testBaseline 逐数一致零回退）+ `cd apps/desktop && cargo build --release` 零警告（8.1MB exe）+ `cargo fmt --all -- --check` 零 diff（编排者复跑 clean）+ `cargo clippy --workspace --all-targets -- -D warnings` 零告警（编排者复跑 clean）+ 五脚本 **9/23/13/72/111 全 exit 0**（编排者复跑一致）。
- **TP-22 载体固化**：release-checklist.md=发布前人工核验清单（铁律「首个含 desktop 的 tag 即正式发布（无预发）；未逐项通过不打 tag」）；必含五项各带当前证据状态+操作性核验方法/命令/通过判据——①本地 MSI 冒烟=已过（U3 全过；发布前按清单命令重跑）②断言脚本全绿含零删改护栏=已过③dmg 结构断言=绿[111/111 内]，真机/CI hdiutil 装配待 CI macos-15 首跑+TP-19 真机步骤④托盘+双击 UI 人工冒烟=待人工（Windows 关闭语义已自动实证；真实托盘点击+mac 侧待真机）⑤产物清单 9 文件与预期一致=待 tag 后核验。复选框固化时点如实未勾。
- **首日验证 4 项留痕核验**（units/5.md 第 3 项）：decisions.md「2026-10-16 U2 应用层完成核收」条目四项结论+偏差留痕全部在案（①可行②winresource 定稿③stdlib PNG-in-ICNS 入仓定稿④CancelClose 可行未降级+伪隐藏偏差）——确认闭合，无需补记。
- gate2 待验注记（供互审/verifier 关注）：a) mac CI 首次 desktop 构建（CI macos-15 实测为准）b) 伪隐藏物理形态偏差（语义一致，AC2④/TP-21 连锁锚未激活）c) 真机人工项（TP-19/20/21/22）固化的 release-checklist.md 承接 d) gate1 r3 授权 6 MINOR+遗留观察（winres 披露/owner/MSI job 构建来源/UTF-8）随首日/施工落字情况。
- unitStatus.U5=completed（5/5）；**phase → gate2**（Construction 收口；提示人类运行 `/aidlc:gate` 进入互审——party-review 四视角+verifier 对照 plan.md 终验，随后人类审批）。

---

2026-10-16 Gate#2 人类审批（AskUserQuestion，逐字转写）：
- 互审输入：`reviews/gate2-ai.md` @ f5459c1（reviewer-pm 0/0/7 + reviewer-architect 0/0/5 + reviewer-qa 0/1/4 + reviewer-dev 0/2/8 + verifier 0/0/2；合并 0 BLOCKER / 3 MAJOR / 26 MINOR。verifier 判定 5/5 units 兑现、验证声称全部独立复现属实[含 U1 红态在 bbe706e 档案树复现 58/53]、无计划外文件、四方对账无实质差异）。
- 呈批三问（AskUserQuestion，逐字转写）：
  问：「Gate#2 互审汇总：0 BLOCKER / 3 MAJOR / 26 MINOR（verifier 判定 5/5 units 兑现、验证声称全部独立复现属实、无计划外文件）。3 MAJOR：M1 伪隐藏状态机缺陷（隐藏态再收 WM_CLOSE 会毒化 restore_pos→「显示主窗口」永久失效，一行守卫可修）/M2 icns 断言未落位（icon.icns 已入仓但无存在性断言，双层运行时兜底在）/M3 终验 fmt 口径缺口（fmt --all 不触 apps/desktop，desktop 内 fmt 含本期新增行 dirty）。如何裁决？」
  答：「通过+准出回填（推荐）」
  问：「M2 icns 断言处置：icon.icns 已入仓（首日验证③定稿）但断言脚本已锁——补存在性断言需新开 UNLOCK-TESTS 授权窗口；或按豁免条款显式裁决留痕。选哪个？」
  答：「补断言+解锁授权（推荐）」
  问：「M3 fmt 口径：apps/desktop 为 workspace exclude，根 fmt/clippy 不触及；desktop 内 fmt dirty 394 行（含本期新增行，其中约 109 行为既有债）。如何处置？」
  答：「全量 fmt apps/desktop」
- 裁决落点（编排者执行）：
  1. gate2 = approved（本回执为 decisions.md 最新 GATE-APPROVED gate2），phase → operations。
  2. **准出回填清单**（批准条件式小修，沿 audit-p0p1-fixes gate2 先例；编排者核收留痕后闭环，无需重开 gate2）：
     - M1：main.rs hide 分支补「已隐藏」守卫（restore_pos 已有值时仅再次拦截、不得覆写 restore_pos）——消除毒化路径；回填后 desktop 构建冒烟复验（关闭→隐藏→再收 WM_CLOSE→托盘「显示主窗口」仍能复位）。
     - M2：UNLOCK-TESTS 窗口（授权=本回执第 2 问逐字「补断言+解锁授权（推荐）」，见下条）→ desktop-installers-yaml.test.sh 补 icon.icns 存在性断言（含 icn 类型载荷校验可选）→ 重锁。窗口范围仅此。
     - M3：`cd apps/desktop && cargo fmt`（全量 394 行含既有 ≈109 行债——用户裁决「全量 fmt apps/desktop」，纯格式无行为变更）；终验口径修正留痕（workspace fmt 与 desktop fmt 分开声明）。
     - 词级 MINOR 随回填：release-checklist.md L21 证据指针笔误（「U3 应用层完成核收」→「U3 Windows MSI 扩展完成核收」）；release.yml:102-103 既有 job 头注失实同步（「只铺两 exe」→三件套+快捷方式叙述）。
  3. **追认项打包（本回执一并追认）**：a) .gitignore /build/ 追加（decisions U3 施工注记留痕）b) apps/desktop/Cargo.lock 依赖锁跟随 c) raw-window-handle 0.6 直声明（原传递依赖转直，decisions U2 遗留观察 b）d) 伪隐藏物理形态偏差（语义与 Q1/AC2④/TP-21 一致，decisions U2 偏差条目；mac Dock 图标留存+Mission Control 可见性记录口径缺口随真机核验收敛）e) AC5④ 校验步两 exe→三 exe 就地改名（A3 批准的 units/3.md L16 契约）f) Info.plist 版本注入（安装器版本同源口径，decisions U4 披露）。
  4. **备案项（不入本期回填，转 backlog/后续 intent）**：wxs Package Description/Comments 两件套描述欠准（AC3④ 既有字面守护，修复需新授权）；HKCU KeyPath 多用户卸载孤儿；dmg 脚本架构校验缺位；TP-23 显式承接；T4 断言①步骤名脆弱性；「111」环境相关口径注记；gen_icons 跨 zlib 幂等边界；TP-12 'winres' 子串宽松；FFI 32 位约束注释；units/2.md 契约偏差指针；.gitignore src-tauri 死路径残留。

GATE-APPROVED gate2 2026-10-16

---

2026-10-16 UNLOCK-TESTS（gate2 准出回填 M2：icns 断言补落窗口；授权依据=gate2 人类裁决逐字复引）：
- 人类原话（2026-10-16 Gate#2 审批，AskUserQuestion 逐字）：问「M2 icns 断言处置：icon.icns 已入仓（首日验证③定稿）但断言脚本已锁——补存在性断言需新开 UNLOCK-TESTS 授权窗口；或按豁免条款显式裁决留痕。选哪个？」答「**补断言+解锁授权（推荐）**」。
- 解锁窗口范围：`scripts/desktop-installers-yaml.test.sh` **仅新增 icon.icns 产物断言**（存在性+结构校验），**零改动既有 111 断言**；补后本文件重锁（override 清除+LOCK-TESTS 条目）。
- 机制注记：本 intent state.json `tddLockOverride={by:"human", at:"2026-10-16", reason:"gate2 准出回填 M2：icon.icns 存在性断言补落（第 2 问逐字『补断言+解锁授权（推荐）』）"}`；流程纪律遵循同前（授权→窗口→修订→重锁）。

---

2026-10-16 准出回填核收（含降级代跑记录，SKILL §8）——回填闭环，UNLOCK 窗口关闭重锁（编排者）：
- **降级代跑记录（§8）**：dev 代理派发失败于 5 小时额度上限（2026-10-16 14:40:02 触发，18:07:47 重置；即时重派将同样失败）→ 编排者内联代跑全部回填项。授权链完整：回填四项均为 gate2 人类裁决逐字授权（①「通过+准出回填（推荐）」②「补断言+解锁授权（推荐）」③「全量 fmt apps/desktop」+词级清单），零自由裁量。诚实声明：本条目产出为「内联代行，缺独立复核」——§8 第 4 条恢复后补强：额度恢复后的独立复核并入 ops 收尾（届时或派 verifier 抽检回填 diff，或由人类在 ops 确认时知悉）。
- **M1 已隐藏守卫**（2a02050）：main.rs hide 分支 `restore_pos.is_none()` 守卫——隐藏态再收 WM_CLOSE 仅重复拦截+CancelClose，不覆写 restore_pos（毒化路径消除）。证据：build --release 过/clippy -D warnings 零告警/desktop fmt clean；冒烟=启动(78,78)→WM_CLOSE→(-32000,-32000)+进程存活（单关路径回归过；双关毒化为外部不可区分状态——由守卫代码路径消除，完整体验待人工冒烟[T20/21 清单面]）。
- **M2 icns 断言补落**（3bc308c，UNLOCK 窗口内）：新增 1 断言「TP-11 icon.icns 存在且有效（magic 69636e73+长度非零+ic07/ic08/ic11 载荷≥2 匹配）」；实文件 3 匹配 PASS、注入假文件即 FAIL（非恒真验证）；既有 111 断言零触碰；豁免注记同步三处（头注/TP-11 段标题/icons.rs 标签——豁免条款按其自带「定稿后补启用」承诺兑现）。**新口径计数：112/112**（actionlint 不可用分支；可用时 113——环境相关口径注记沿 qa MINOR-4）。
- **M3 全量 fmt**（2a02050）：`cd apps/desktop && cargo fmt`——394 行纯格式（main.rs 守卫+格式/theme.rs/widgets.rs 既有 ≈109 行债清）；desktop fmt --check clean。
- **词级 MINOR**（218de5e）：release-checklist.md L21 指针笔误改「U3 Windows MSI 扩展完成核收」；release.yml installer-msi-windows job 头注失实同步（三件套+两处快捷方式+图标叙述）。
- **核收证据（编排者全量复跑）**：五脚本 **112/72/23/13/9 全 exit 0**；`cargo test --workspace` **277 passed/0 failed**（零回退）；desktop build/clippy/fmt 全 clean。
- **重锁（LOCK-TESTS）**：state.json tddLockOverride 清除（置 null）；tddLock 保持 true（tddLockPaths=[scripts/desktop-installers-yaml.test.sh] 不变）——本文件重归零删改基线；后续改动须新授权。
- 回填闭环：gate2 裁决的全部准出条件已闭合；phase=operations；下一步 /aidlc:ops（retro 保留或省略裁决随 ops 呈出）。

---
