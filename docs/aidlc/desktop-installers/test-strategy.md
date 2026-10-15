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
- **Gate#1 打回裁决落点**（decisions.md 2026-10-16 GATE-REJECTED gate1）：Q1=关闭按钮隐藏到托盘（托盘「退出」真退出；eframe CancelClose 落码形态入 prd 首日验证④）；Q2=快捷方式开始菜单+桌面两处；B2=installer-msi-windows 从零触碰集合移出（允许追加 desktop 构建与归集步）；B1=installers-yaml.test.sh T4 两条负断言授权锁修订（字面收窄，Construction 期执行，decisions.md 留痕）；dmg=aarch64-only；版本显示不一致（ARP 0.2 vs exe 0.1.0）已记录知悉。
- **零触碰集合（gate1 修订后）**：四锁定脚本（package-macos / release-yaml / check-macos-yaml / installers-yaml.test.sh）保持零触碰，唯 installers-yaml.test.sh T4 两条负断言按 B1 执行授权锁修订（修订后纳入零删改基线）；release.yml 侧 installer-msi-windows job 按 B2 移出零触碰集合（仅允许追加，daemon/cli 字面不删改）。新断言一律落独立文件（建议 `scripts/desktop-installers-yaml.test.sh`，落在 intent 声明的 `scripts/desktop-*.test.sh` glob 内）。
- 复用上 intent（installer-packages，done）测试模式：结构断言独立文件 + TDD 红→锁→绿 + 防空真 + CRLF 剥除 + actionlint 降级计数诚实 + 授权锁修订闭环。
- 环境事实：Windows/Git Bash 本机；**WiX v3.14.1 已装（T16 遗产）→ desktop MSI 可本地构建+msiexec 静默装卸验证**；无 mac 机/无 codesign/pkgbuild → dmg 只能字面断言；egui/tray 无法 headless 自动测 → 托盘/UI 落人工冒烟。

## 测试点清单

| # | 映射 AC | 场景（正/负路径） | 层级 | 优先级 |
|---|---|---|---|---|
| TP-01 | AC5 | 正：release.yml 新增 desktop 构建/打包 job，tag 触发（`on: push: tags` 匹配），上传产物含 desktop MSI 与 .dmg | 单元 | P0 |
| TP-02 | AC5 | 负：desktop job 无 continue-on-error、无绕过性 `if:`；needs 链完整——desktop 构建失败→打包/发布 job 不执行，**不产出不发布**（沿上 intent 无防护即不通过口径） | 单元 | P0 |
| TP-03 | AC3 | 负：既有产物与 job 零删改护栏（基线口径修订）——daemon/cli 相关字面锚定（release.yml 实际 job 名 binaries-macos/installer-pkg-macos 等）+改动后快照对照；installer-msi-windows job 按 B2 允许追加 desktop 构建与归集步（daemon/cli 字面不删改）；B1 注：Construction 期对 installers-yaml.test.sh T4 两条负断言执行授权锁修订（字面收窄「daemon/cli 铺放组件除外」，decisions.md 留痕），修订后纳入零删改基线 | 单元 | P0 |
| TP-04 | AC3 | 正：main.wxs 含 desktop 组件（cloudmount-desktop.exe 铺 Program Files\CloudMount），daemon/cli 组件保持原样；负：无 32 位目录字面（ProgramFilesFolder，须 64 位 ProgramFiles64Folder——T16 实证回归防护） | 单元 | P0 |
| TP-05 | AC3 | 正：main.wxs 含开始菜单+桌面两处 Shortcut 且指向 desktop exe（Q2 已裁决两处）；负：快捷方式目标不指 daemon/cli exe（指错即缺陷） | 单元 | P0 |
| TP-06 | AC1 | 负：main.wxs/winres 引用的 .ico 图标文件缺失→断言红（构建前拦截，图标缺失不产出） | 单元 | P0 |
| TP-07 | AC4 | 正：package-dmg-macos.sh 字面断言——.app bundle 结构（Contents/Info.plist、Contents/MacOS/cloudmount-desktop、Contents/Resources/*.icns）+ hdiutil 打 .dmg + dmg 内 /Applications 拖拽语义 | 单元 | P0 |
| TP-08 | AC4 | 负：daemon/cli 既有 pkg 铺放负断言——pkg 打包脚本/内容清单**不含 .app、不含 desktop 组件**（A1：.app 只走 .dmg，desktop 不许混入 daemon/cli pkg） | 单元 | P0 |
| TP-09 | AC2 | 正：apps/desktop 源码托盘接线字面断言（可判定式）——托盘 init 函数区段内 MenuItem 构造计数=2 + 两 id 在 MenuEvent handler 匹配 + 全文件无其他 tray Menu 构造调用（即恰「显示主窗口/退出」两项） | 单元 | P0 |
| TP-10 | AC6 | 负：断言脚本自身质量（沿上 intent 三模式）——防空真（每条 grep 命中≥1 行否则判红）、CRLF 剥除不误报（Git Bash/Windows）、actionlint 缺失时降级计数如实输出而非静默绿 | 单元 | P1 |
| TP-11 | AC1 | 正：gen_icons.py 本地可跑，产物存在+尺寸集断言（.ico 多尺寸、.png 集合；icns 项按 prd 首日验证③结论落位后的对应断言——stdlib 手写 icns 或 CI mac 生成二选一，Construction 定）；输出路径不再指向不存在的 src-tauri | 单元 | P0 |
| TP-12 | AC1 | 负：图标管线 fail-fast——gen_icons 失败/产物缺失时下游不产出（wxs light 缺 .ico 失败 / dmg 步骤缺 .icns 失败的字面或本地复现） | 单元 | P1 |
| TP-13 | AC3 | 正：本地 WiX v3.14.1 构建 MSI 成功（T16 遗产链路），desktop 组件编入 | 集成 | P0 |
| TP-14 | AC3 | 正：msiexec 静默安装成功，Program Files\CloudMount 三件套齐全（daemon+cli+desktop exe 在位，daemon/cli 铺放不回归，沿 T18 模式） | 集成 | P0 |
| TP-15 | AC3 | 正：安装后开始菜单+桌面快捷方式存在且目标指向 desktop exe（Q2 两处）；负：快捷方式目标不指 daemon/cli exe | 集成 | P0 |
| TP-16 | AC1 | 正：安装后 exe 带图标（可判定式）——PowerShell 枚举 exe RT_GROUP_ICON 资源存在（winres 嵌入即有）或提取位图采样品牌色（上白下蓝）；ExtractAssociatedIcon 非空断言作废（对无图标 exe 返回系统默认图标，空真）；多尺寸视觉效果仍辅人工目验 | 集成 | P0 |
| TP-17 | AC3 | 正+负：msiexec 静默卸载→desktop exe 与两处快捷方式全部清除，无孤儿残留 | 集成 | P1 |
| TP-18 | AC3/AC5 | 负：desktop 构建失败注入（本地等价线上门控）→Candle/Light 失败→MSI 不产出（=AC3 本地等价）；对应线上 needs 门控不发布语义（=AC5） | 集成 | P1 |
| TP-19 | AC4 | 正：.dmg 真机人工（aarch64 机型）——挂载、拖拽装 /Applications、双击启动 egui 界面、icns 图标显示、Gatekeeper 未签名提示沿既有引导 | E2E（人工） | P1 |
| TP-20 | AC2 | 正：托盘人工冒烟——托盘图标可见、菜单恰两项、「显示主窗口」唤起主窗口、「退出」真退（进程消失+托盘消失） | E2E（人工） | P0 |
| TP-21 | AC2 | 正/负：窗口关闭按钮=隐藏到托盘（托盘常驻，Q1 已裁决）+托盘「退出」真退出——实际行为与裁决不符即 FAIL；eframe CancelClose 落码形态入 prd 首日验证④ | E2E（人工） | P1 |
| TP-22 | AC5 | 负：**首个含 desktop 的 tag 即真实发布**——打 tag 前人工核验清单固化于仓库内工件（docs/aidlc/desktop-installers/ 附录文件）且逐项通过（本地 MSI 冒烟绿 / 断言脚本全绿含零删改护栏 / dmg 结构断言绿 / 托盘+双击 UI 人工冒烟通过 / 产物清单与预期一致），未过清单不打 tag | E2E（人工） | P0 |
| TP-23 | AC3/AC4 | 负：既有 daemon/cli 安装面回归抽查（本机既有 MSI 重装或上版产物下载核对）——铺放不变且无 .app 混入（行为层兜底 TP-08，双护 AC3 零回归 + AC4 pkg 不含 .app） | 集成 | P2 |
| TP-24 | AC4 | 负：无签名证书——Windows SmartScreen 侧（下载/运行提示与引导文案）+ mac Gatekeeper 侧提示核对（发布说明/README 沿既有口径），人工 | E2E（人工） | P2 |

**覆盖核对**：AC1←TP-06/11/12/16；AC2←TP-09/20/21；AC3←TP-03/04/05/13/14/15/17/18/23；AC4←TP-07/08/19/23/24；AC5←TP-01/02/18/22；AC6←TP-10。每 AC ≥1 测试点，全部 AC 有 T 点落点，无缺锚。每条主流程均配负路径（门控/缺失/护栏/卸载/无签名）。

**进出门（TDD 红→锁→绿对齐，原「风险排序门」校准）**：P0 共 16 项——**P0 全部落位且可运行（红态）方可进 Construction，全绿为准出门**；P0 缺项即阻塞。

## 分层策略

- **单元**（新独立断言脚本 + 图标管线，本地全可跑）：`scripts/desktop-installers-yaml.test.sh`（独立文件，四锁定脚本零触碰——唯 B1 授权锁修订按裁决在 Construction 期收窄 T4 两条负断言）——TDD 红→锁→绿；断言面：release.yml desktop job 结构（tag 触发 / needs 门控链 / 无 continue-on-error / 无绕过 if / daemon/cli 相关字面锚定+改动后快照）、main.wxs desktop 组件+两处快捷方式+图标引用存在+无 32 位目录字面、.app bundle 结构字面（Info.plist / MacOS 二进制 / Resources icns）、daemon/cli pkg 铺放负断言（无 .app 不混入）、tray 接线可判定式（MenuItem 计数=2+MenuEvent id 匹配+无其他 tray Menu 构造）；脚本自身质量测试：防空真、CRLF 剥除、actionlint 降级计数诚实。图标管线本地可跑断言：gen_icons.py 产物存在+尺寸集（icns 断言按首日验证③二选一落位，Construction 定）、失败 fail-fast。**不测**：运行期 UI 行为、真实 mac 工具链行为。
- **集成**（本地 WiX v3.14.1 + msiexec，沿 T16/T18 模式，本机可验）：desktop MSI 静默装→三件套铺放→开始菜单/桌面快捷方式在位（目标指向 desktop exe）→exe 带图标（RT_GROUP_ICON 资源枚举/位图采样品牌色）→静默卸载干净；负路径=desktop 构建失败注入→MSI 不产出（线上 needs 门控的本地等价验证）。**不测**：升级语义（本期非目标，MajorUpgrade 向前覆盖+提示先卸载即够）、代码签名。
- **E2E**（线上/真机人工冒烟，不可自动化如实落清单）：.dmg 真机挂载/拖拽装/双击启动/图标显示（aarch64-only）；托盘行为（图标可见/菜单两项/显示主窗口/退出真退）；关闭按钮语义按 Q1 裁决核验（隐藏到托盘+托盘「退出」真退出）；双击 UI 冒烟；**首个含 desktop 的 tag 前人工核验清单**（固化于 docs/aidlc/desktop-installers/ 附录文件，打 tag 前逐项过，tag 即正式发布、无预发环境）。**不测**：无法自动化的一切 UI 视觉细节，只验存在性与行为符合裁决。

## 缺口与开放问题

**缺口（本地不可自动化，如实记录）**

- [ ] dmg 工具链本地不可自动：无 mac 机，hdiutil/package-dmg-macos.sh 仅能字面断言（TP-07）；挂载/拖拽/真机图标行为只能人工（TP-19）；dmg=aarch64-only（gate1 裁决）。
- [ ] icns 生成路径未定：stdlib 手写 icns 或 CI mac 生成二选一（prd 首日验证③，Construction 定）——TP-11 icns 断言随结论落位，本地必产假设已删。
- [ ] 托盘/关闭语义行为无法 headless 自动测：egui/tray 属人工冒烟清单（TP-20/21）；关闭语义已由 Q1 裁决定稿（隐藏到托盘+托盘「退出」真退出，eframe CancelClose 落码形态入首日验证④），TP-21 不再挂起。
- [ ] exe 图标自动化验证口径修订：ExtractAssociatedIcon 非空已实测空真（无图标 exe 返回系统默认图标），弃用；TP-16 改 RT_GROUP_ICON 资源枚举/位图采样品牌色可判定式；多尺寸视觉清晰度仍人工目验。
- [ ] B1 授权锁修订（Construction 期动作）：installers-yaml.test.sh T4 两条负断言字面收窄（「daemon/cli 铺放组件除外」或等价精确形式，decisions.md 留痕）；修订前该断言以现行字面运行，修订后纳入零删改基线。
- [ ] 首个含 desktop 的 tag 即真实发布（无预发）：人工核验清单固化于仓库内工件 docs/aidlc/desktop-installers/ 附录文件（TP-22），打 tag 前逐项过；本工件锚定清单必含项。
- [x] 映射列已按 prd 实际 AC1..AC6 全量重映射；全部 AC 有 T 点落点，无缺锚。
- [x] 版本显示不一致（ARP 0.2 vs exe 0.1.0）已记录知悉（gate1 裁决附带），不构成本期测试点。

**裁决已定（原开放问题关闭）**

- [x] 快捷方式范围（Q2 已裁决）：开始菜单+桌面两处——TP-05/TP-15 断言按两处钉死，并补「目标不指 daemon/cli exe」负断言。
- [x] 托盘关闭语义（Q1 已裁决）：关闭按钮隐藏到托盘（托盘常驻），托盘「退出」真退出；TP-09 可判定式与 TP-21 人工核验按裁决定稿。
