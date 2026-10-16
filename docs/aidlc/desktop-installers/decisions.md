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