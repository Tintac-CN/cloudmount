# Intent: desktop-installers

> 创建：2026-10-16　状态：inception

## 一句话意图
这个安装包不带前端界面不是搞笑吗，这是桌面端吗？无图标，无托盘，无界面，简直了

## 问题与期望
- **要解决的问题**：`installer-packages`（已 done）交付的 MSI/pkg 按 A1 裁决只装 daemon+cli——装完无桌面应用、无图标、无托盘、无界面；且 `apps/desktop` 桌面壳自身存在两层缺口：托盘依赖（tray-icon）已声明但源码未接线、应用图标从未生成嵌入（gen_icons.py 输出路径指向不存在的 src-tauri 目录）。
- **期望的结果**：Windows MSI 一个包装三件套（daemon+cli+desktop），有开始菜单/桌面快捷方式、exe 有图标；mac 出 `cloudmount.app` 的 `.dmg`（拖拽装 /Applications，含 icns 图标）；desktop 应用有托盘（显示主窗口/退出）；自动化发布覆盖新产物面。

## 受影响系统/用户
- 改动面：`apps/desktop/src`（托盘接线）+ `scripts/gen_icons.py`（重写图标管线）+ `packaging/wix/main.wxs`（desktop 组件+快捷方式）+ `scripts/package-dmg-macos.sh`（新增）+ `.github/workflows/release.yml`（desktop 构建/打包 job）+ `scripts/desktop-*.test.sh`（新断言，独立文件——installers-yaml.test.sh 已锁）
- 用户：Windows/mac 桌面用户（双击图标得 egui 界面，托盘可控）；发布面新增 MSI/desktop 与 .dmg 两类产物

## 约束
- 应用层最小集（避免发明需求）：托盘菜单仅「显示主窗口/退出」两项；图标=品牌双色方块（沿 gen_icons.py 设计）；不做单实例/自启动/自动更新
- 无代码签名证书不变：MSI SmartScreen/mac Gatekeeper 提示沿既有引导
- daemon/cli 既有产物与 job 零删改（沿 installer-packages AC3 口径）；其 pkg 不含 .app（A1 裁决：.app 走 .dmg）
- `apps/desktop` 为独立 cargo 项目（workspace exclude）——CI 需独立构建面；构建（编译）不需 GPU，运行期才涉及渲染
- 升级语义沿 installer-packages 非目标：不设计，MajorUpgrade 向前覆盖可用 + 提示先卸载再装
- 上 intent 锁定脚本（installers-yaml.test.sh 等）零触碰——新断言落独立文件
- 配额注意：串行派发；AskUserQuestion 未获选择时按对话裁决兜底（上两 intent 惯例）

## 开放问题（需人类裁决）
- [ ] Windows 快捷方式范围：开始菜单+桌面两处都放，还是仅开始菜单？（建议默认：两处）
- [ ] 托盘关闭语义：点窗口关闭按钮 = 隐藏到托盘（托盘常驻）还是退出应用？（建议默认：隐藏到托盘，托盘菜单「退出」真退出——桌面应用惯例）
