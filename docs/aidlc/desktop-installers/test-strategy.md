# Test Strategy: desktop-installers

> 产出者：qa　日期：2026-10-16　来源：intent.md + prd.md

## 前提与 AC 对齐说明

- **已对齐 prd.md 实际 AC1..AC6（对齐拍全量重映射）**；测试点本体/场景/层级/优先级零改动，仅映射列回填。
- prd 实际 AC：
  - **AC1**（图标管线）：gen_icons 重写 + winres 嵌 exe + stale 路径清零。
  - **AC2**（托盘接线）：恰两项菜单 + 关闭语义按裁决。
  - **AC3**（Windows MSI 扩展）：desktop 组件 + 快捷方式 + Icon + 零回归。
  - **AC4**（mac .app+.dmg）：含「pkg 不含 .app」。
  - **AC5**（发布覆盖）：release.yml desktop job（needs 追加 + 精确路径 upload + 既有五 job 零触碰）。
  - **AC6**（断言独立文件守护）：四锁定脚本零触碰。
- 复用上 intent（installer-packages，done）测试模式：结构断言独立文件 + TDD 红→锁→绿 + 防空真 + CRLF 剥除 + actionlint 降级计数诚实 + 授权锁修订闭环。**四脚本已锁定零触碰**（package-macos / release-yaml / check-macos-yaml / installers-yaml.test.sh），新断言一律落独立文件（建议 `scripts/desktop-installers-yaml.test.sh`，落在 intent 声明的 `scripts/desktop-*.test.sh` glob 内）。
- 环境事实：Windows/Git Bash 本机；**WiX v3.14.1 已装（T16 遗产）→ desktop MSI 可本地构建+msiexec 静默装卸验证**；无 mac 机/无 codesign/pkgbuild → dmg 只能字面断言；egui/tray 无法 headless 自动测 → 托盘/UI 落人工冒烟。

## 测试点清单

| # | 映射 AC | 场景（正/负路径） | 层级 | 优先级 |
|---|---|---|---|---|
| TP-01 | AC5 | 正：release.yml 新增 desktop 构建/打包 job，tag 触发（`on: push: tags` 匹配），上传产物含 desktop MSI 与 .dmg | 单元 | P0 |
| TP-02 | AC5 | 负：desktop job 无 continue-on-error、无绕过性 `if:`；needs 链完整——desktop 构建失败→打包/发布 job 不执行，**不产出不发布**（沿上 intent 无防护即不通过口径） | 单元 | P0 |
| TP-03 | AC3 | 负：既有产物与 job 零删改护栏——release.yml 中 daemon/cli/macos-release 相关 job 与步骤字面对照锁定基线不变，desktop 改动不触碰既有 job | 单元 | P0 |
| TP-04 | AC3 | 正：main.wxs 含 desktop 组件（cloudmount-desktop.exe 铺 Program Files\CloudMount），daemon/cli 组件保持原样 | 单元 | P0 |
| TP-05 | AC3 | 正：main.wxs 含开始菜单+桌面两处 Shortcut 且指向 desktop exe（若 gate 裁决「仅开始菜单」则断言相应修正） | 单元 | P0 |
| TP-06 | AC1 | 负：main.wxs/winres 引用的 .ico 图标文件缺失→断言红（构建前拦截，图标缺失不产出） | 单元 | P0 |
| TP-07 | AC4 | 正：package-dmg-macos.sh 字面断言——.app bundle 结构（Contents/Info.plist、Contents/MacOS/cloudmount-desktop、Contents/Resources/*.icns）+ hdiutil 打 .dmg + dmg 内 /Applications 拖拽语义 | 单元 | P0 |
| TP-08 | AC4 | 负：daemon/cli 既有 pkg 铺放负断言——pkg 打包脚本/内容清单**不含 .app、不含 desktop 组件**（A1：.app 只走 .dmg，desktop 不许混入 daemon/cli pkg） | 单元 | P0 |
| TP-09 | AC2 | 正：apps/desktop 源码托盘接线字面断言——tray-icon 初始化存在，菜单恰「显示主窗口/退出」两项（不多不少） | 单元 | P0 |
| TP-10 | AC6 | 负：断言脚本自身质量（沿上 intent 三模式）——防空真（每条 grep 命中≥1 行否则判红）、CRLF 剥除不误报（Git Bash/Windows）、actionlint 缺失时降级计数如实输出而非静默绿 | 单元 | P1 |
| TP-11 | AC1 | 正：gen_icons.py 本地可跑，产物存在+尺寸集断言（.ico 多尺寸、.png 集合、.icns 生成，输出路径不再指向不存在的 src-tauri） | 单元 | P0 |
| TP-12 | AC1 | 负：图标管线 fail-fast——gen_icons 失败/产物缺失时下游不产出（wxs light 缺 .ico 失败 / dmg 步骤缺 .icns 失败的字面或本地复现） | 单元 | P1 |
| TP-13 | AC3 | 正：本地 WiX v3.14.1 构建 MSI 成功（T16 遗产链路），desktop 组件编入 | 集成 | P0 |
| TP-14 | AC3 | 正：msiexec 静默安装成功，Program Files\CloudMount 三件套齐全（daemon+cli+desktop exe 在位，daemon/cli 铺放不回归，沿 T18 模式） | 集成 | P0 |
| TP-15 | AC3 | 正：安装后开始菜单+桌面快捷方式存在且目标指向 desktop exe | 集成 | P0 |
| TP-16 | AC1 | 正：安装后 exe 带图标——PowerShell ExtractAssociatedIcon 非空（多尺寸视觉效果辅人工目验） | 集成 | P0 |
| TP-17 | AC3 | 正+负：msiexec 静默卸载→desktop exe 与两处快捷方式全部清除，无孤儿残留 | 集成 | P1 |
| TP-18 | AC3/AC5 | 负：desktop 构建失败注入（本地等价线上门控）→Candle/Light 失败→MSI 不产出（=AC3 本地等价）；对应线上 needs 门控不发布语义（=AC5） | 集成 | P1 |
| TP-19 | AC4 | 正：.dmg 真机人工——挂载、拖拽装 /Applications、双击启动 egui 界面、icns 图标显示、Gatekeeper 未签名提示沿既有引导 | E2E（人工） | P1 |
| TP-20 | AC2 | 正：托盘人工冒烟——托盘图标可见、菜单恰两项、「显示主窗口」唤起主窗口、「退出」真退（进程消失+托盘消失） | E2E（人工） | P0 |
| TP-21 | AC2 | 正/负：窗口关闭按钮语义与 gate1 裁决一致（建议默认：隐藏到托盘+托盘常驻；若裁决=退出则核验退出真退）——实际行为与裁决不符即 FAIL | E2E（人工） | P1 |
| TP-22 | AC5 | 负：**首个含 desktop 的 tag 即真实发布**——打 tag 前人工核验清单固化且逐项通过（本地 MSI 冒烟绿 / 断言脚本全绿含零删改护栏 / dmg 结构断言绿 / 托盘+双击 UI 人工冒烟通过 / 产物清单与预期一致），未过清单不打 tag | E2E（人工） | P0 |
| TP-23 | AC3/AC4 | 负：既有 daemon/cli 安装面回归抽查（本机既有 MSI 重装或上版产物下载核对）——铺放不变且无 .app 混入（行为层兜底 TP-08，双护 AC3 零回归 + AC4 pkg 不含 .app） | 集成 | P2 |
| TP-24 | AC4 | 负：无签名证书——SmartScreen/mac Gatekeeper 提示引导文案核对（发布说明/README 沿既有口径），人工 | E2E（人工） | P2 |

**覆盖核对**：AC1←TP-06/11/12/16；AC2←TP-09/20/21；AC3←TP-03/04/05/13/14/15/17/18/23；AC4←TP-07/08/19/23/24；AC5←TP-01/02/18/22；AC6←TP-10。每 AC ≥1 测试点，全部 AC 有 T 点落点，无缺锚。每条主流程均配负路径（门控/缺失/护栏/卸载/无签名）。

**风险排序门**：P0 共 16 项，全绿方可进 Construction；P0 缺项即阻塞。

## 分层策略

- **单元**（新独立断言脚本 + 图标管线，本地全可跑）：`scripts/desktop-installers-yaml.test.sh`（独立文件，四锁定脚本零触碰）——TDD 红→锁→绿；断言面：release.yml desktop job 结构（tag 触发 / needs 门控链 / 无 continue-on-error / 无绕过 if / 既有 job 零删改对照基线）、main.wxs desktop 组件+两处快捷方式+图标引用存在、.app bundle 结构字面（Info.plist / MacOS 二进制 / Resources icns）、daemon/cli pkg 铺放负断言（无 .app 不混入）、tray 接线字面（菜单恰两项）；脚本自身质量测试：防空真、CRLF 剥除、actionlint 降级计数诚实。图标管线本地可跑断言：gen_icons.py 产物存在+尺寸集、失败 fail-fast。**不测**：运行期 UI 行为、真实 mac 工具链行为。
- **集成**（本地 WiX v3.14.1 + msiexec，沿 T16/T18 模式，本机可验）：desktop MSI 静默装→三件套铺放→开始菜单/桌面快捷方式在位→exe 带图标（ExtractAssociatedIcon）→静默卸载干净；负路径=desktop 构建失败注入→MSI 不产出（线上 needs 门控的本地等价验证）。**不测**：升级语义（本期非目标，MajorUpgrade 向前覆盖+提示先卸载即够）、代码签名。
- **E2E**（线上/真机人工冒烟，不可自动化如实落清单）：.dmg 真机挂载/拖拽装/双击启动/图标显示；托盘行为（图标可见/菜单两项/显示主窗口/退出真退）；关闭按钮语义按 gate1 裁决核验；双击 UI 冒烟；**首个含 desktop 的 tag 前人工核验清单**（打 tag 前逐项过，tag 即正式发布、无预发环境）。**不测**：无法自动化的一切 UI 视觉细节，只验存在性与行为符合裁决。

## 缺口与开放问题

**缺口（本地不可自动化，如实记录）**

- [ ] dmg 工具链本地不可自动：无 mac 机，hdiutil/package-dmg-macos.sh 仅能字面断言（TP-07）；挂载/拖拽/真机图标行为只能人工（TP-19）。
- [ ] 托盘/关闭语义行为无法 headless 自动测：egui/tray 属人工冒烟清单（TP-20/21）；TP-21 在 gate1 裁决关闭语义前挂起。
- [ ] exe 图标自动化验证受限：ExtractAssociatedIcon 可程序化探测存在性，多尺寸视觉清晰度仍需人工目验（TP-16 辅助项）。
- [ ] 首个含 desktop 的 tag 即真实发布（无预发）：人工核验清单须在打 tag 前固化（TP-22）；建议 Construction 时落为可勾选清单工件（release 说明或本目录附录），本工件只锚定清单必含项。
- [x] 映射列已按 prd 实际 AC1..AC6 全量重映射；全部 AC 有 T 点落点，无缺锚。

**开放问题（依赖 gate 裁决，影响断言内容）**

- [ ] 快捷方式范围：开始菜单+桌面两处 vs 仅开始菜单（intent 开放问题，默认建议两处）——若反向裁决，TP-05/TP-15 断言修正。
- [ ] 托盘关闭语义：隐藏到托盘（托盘常驻）vs 退出应用（intent 开放问题，建议默认隐藏到托盘）——裁决后 TP-09 字面断言与 TP-21 人工核验按裁决定稿。
