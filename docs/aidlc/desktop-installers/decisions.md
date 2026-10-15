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
