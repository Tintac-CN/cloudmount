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
