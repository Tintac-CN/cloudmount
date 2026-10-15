# PRD: desktop-installers

> 产出者：pm　日期：2026-10-16　来源：intent.md

## 用户故事
| # | 角色 | 目标 | 价值 | 溯源（intent.md） |
|---|---|---|---|---|
| S1 | Windows 桌面用户 | 运行一个 MSI 装出 daemon+cli+desktop 三件套，开始菜单/桌面有带品牌图标的 CloudMount 快捷方式，双击即见 egui 界面 | 装完即得完整桌面端，终结「无图标无界面」 | 「期望的结果」第 1 条；「用户」节 |
| S2 | mac 桌面用户 | 下载 .dmg，拖拽 cloudmount.app 入 /Applications，应用带 icns 图标、启动有界面 | mac 分发具备正规桌面应用形态 | 「期望的结果」第 2 条；A1「.app 进 .dmg」裁决 |
| S3 | 桌面用户（Win/mac） | 系统托盘常驻 CloudMount 图标，菜单可「显示主窗口」「退出」 | 应用可控可退，托盘语义完整 | 「期望的结果」「desktop 应用有托盘（显示主窗口/退出）」；「约束」应用层最小集 |
| S4 | 发布维护者 | push tag 后 release 自动构建 desktop 面并产出新产物（MSI 升为三件套 + .dmg），旧产物面零回退 | 发布自动化一次覆盖，不破坏既有交付 | 「期望的结果」「自动化发布覆盖新产物面」；「约束」daemon/cli 零删改 |
| S5 | 仓库维护者 | 全部图标由 scripts/gen_icons.py 单一管线再生（ico/png/icns → apps/desktop/icons/），exe/MSI/托盘/.app 图标同源 | 品牌一致，清灭 src-tauri stale 路径 | 「改动面」gen_icons.py 重写；A1 裁决落点「gen_icons 重写 ico/png/icns 管线」 |

## 验收标准（可测）
两层可测性口径（如实拆分）：**静态断言** = 断言脚本可验的源码字面/产物文件存在/配置 diff；**人工冒烟** = 应用层交互与 GUI 语义，无法 CI 自动化，逐条列于 AC 后清单。

- **AC1（图标管线，S5）**：Given scripts/gen_icons.py 已重写（品牌沿既有像素设计：上 1/3 白（云）/浅色分隔/下 2/3 蓝（盘）双色方块，输出目录改为 `apps/desktop/icons/`），When 本地运行 `python scripts/gen_icons.py`，Then ① `apps/desktop/icons/` 产出 icon.ico（≥1 帧有效 ICO，现有基线 32×32 PNG-in-ICO）与 PNG 组（现有基线 32×32 与 128×128；如需扩尺寸由 Construction 落，不写死未实测值）② 全仓库对 `apps/desktop/src-tauri` 的图标路径引用清零（grep 验证）③ 重复运行幂等。**且**：Given apps/desktop 构建配置含 .ico 资源嵌入（winres 方案——其构建影响见「Construction 首日验证②」），When 本地 `cargo build --release`，Then target/release/cloudmount-desktop.exe 含图标资源。验证层：静态断言=产物文件存在+stale 路径清零+构建配置字面；人工冒烟=资源管理器/文件属性中 exe 显示品牌图标。测试锚：TP-06/TP-11/TP-12/TP-16。
- **AC2（托盘接线，S3）**：Given apps/desktop/src/main.rs 接入已声明的 tray-icon 0.21，When 对 apps/desktop/src 静态检查，Then ① grep tray 有命中（托盘初始化存在）② 托盘菜单字面含且**仅含**两项：「显示主窗口」「退出」③ 「显示主窗口」绑定唤起主窗口、「退出」绑定终止进程（菜单项回调字面可断言）④ 窗口关闭按钮语义按开放问题 Q1 裁决落码（建议默认：隐藏到托盘，托盘「退出」真退出；egui 拦截 API 形态见「Construction 首日验证④」）。人工冒烟清单（Win/mac 各一遍）：托盘图标可见（用 AC1 产物）→ 点「显示主窗口」窗口重现 → 点「退出」进程结束 → 关闭按钮行为符合 Q1 裁决。测试锚：TP-09/TP-20/TP-21。
- **AC3（Windows MSI 扩展，S1）**：Given packaging/wix/main.wxs 增 desktop 组件，When 既有 installer-msi-windows job 构建 MSI，Then ① cloudmount-desktop.exe 组件铺 Program Files\CloudMount（与 daemon/cli 同目录）② 快捷方式指向 desktop exe，范围按开放问题 Q2 裁决（建议默认：开始菜单+桌面两处）③ MSI 内嵌品牌图标（Icon 元素 + Shortcut Icon / ARPPRODUCTICON，源自 AC1 的 icon.ico）④ 回归零破坏：daemon/cli 组件铺放、Machine PATH Environment、MajorUpgrade、Name="CloudMount"、UpgradeCode、Manufacturer、perMachine、UTF-8 BOM 等既有字面不变（diff 断言）。验证层：静态断言=新断言脚本对 .wxs 字面；人工冒烟=本地 msiexec 安装后三件套目录齐全、快捷方式存在且图标正确、PATH 仍指向安装目录。测试锚：TP-03/TP-04/TP-05/TP-13/TP-14/TP-15/TP-17/TP-18/TP-23。
- **AC4（mac .app + .dmg，S2）**：Given 新增 scripts/package-dmg-macos.sh（Info.plist + icns + hdiutil），When macos runner 执行打包，Then ① 产出 cloudmount.app（含二进制 cloudmount-desktop、icns 图标、Info.plist 声明）② hdiutil 产出 .dmg（文件名/卷名 Construction 落定，建议 cloudmount-desktop.dmg——不入开放问题），可挂载拖拽安装至 /Applications（dmg 内 Applications 链接为该语义落地惯例，Construction 落）③ daemon/cli 既有 pkg 不含 .app 且内容零变动（A1 裁决口径）。**本地不可验项如实标注**：hdiutil/iconutil 为 mac 独占，本地 Windows 无法验证 dmg 产物与 icns 生成——自动化层仅 CI 产物存在性（artifact 精确路径 upload + if-no-files-found: error 口径沿上 intent）；icns 生成链（CI mac runner 生成 vs 预生成入仓）为「Construction 首日验证③」，不断言哪条可行。人工冒烟=下载 dmg → 挂载 → 拖入 /Applications → 图标正确 → 启动有界面。测试锚：TP-07/TP-08/TP-19/TP-23/TP-24。
- **AC5（release.yml desktop job，S4）**：Given .github/workflows/release.yml 增 desktop 构建与打包 job（windows-latest 构建 cloudmount-desktop.exe；macos-15 构建 .app 并打 .dmg），When push tag（tag 即发布=直接正式，沿 A1），Then ① desktop 构建在 apps/desktop 独立 cargo 项目上执行（cd apps/desktop 或 --manifest-path——workspace exclude 口径）② 新 artifact 以精确路径 upload（if-no-files-found: error 口径沿上 intent）③ release job needs 链追加新 job（既有 needs 元素零删改）④ 既有 binaries-linux/binaries-windows/binaries-macos/installer-msi-windows/installer-pkg-macos 五 job 字面零触碰。验证层：静态断言（新断言脚本对 yml 字面：job 名、needs 元素、upload 路径、error 口径）；无人工冒烟项。测试锚：TP-01/TP-02/TP-18/TP-22。
- **AC6（断言守护独立文件，S4/S5）**：Given 新断言落 scripts/desktop-*.test.sh 独立文件（文件拆分与命名 Construction 落），When 运行全部新断言与既有测试面，Then ① 新断言全绿且覆盖 AC1–AC5 全部静态可断言面 ② 既有锁定脚本（installers-yaml.test.sh 等四个）git diff 为空、字面零触碰。验证层：纯静态。测试锚：TP-10/TP-03。

## 非目标（明确排除）
- 单实例、开机自启动、自动更新（intent「约束」明确不做）。
- 代码签名（无证书不变；SmartScreen/Gatekeeper 引导沿既有口径，本期不新增签名基础设施）。
- 升级语义设计（沿 installer-packages 非目标：不设计，MajorUpgrade 向前覆盖可用 + 提示先卸载再装）。
- daemon/cli 既有产物、pkg、CI job 的任何删改；mac pkg 打入 .app（A1：.app 只走 .dmg）。
- 托盘菜单超出「显示主窗口/退出」的任何项（设置、暂停同步、关于等不发明需求）。
- desktop exe 版本号元数据注入 / 关于页版本展示（intent 未要求；CI 版本注入仅沿既有 daemon/cli 面）。
- Linux 桌面打包、dmg 增量更新/Sparkle 机制（intent 未要求）。
- 修改既有锁定测试脚本（installers-yaml.test.sh 等——零触碰，新断言独立文件）。

## 语义对齐（express 内联小节）
**关键动词 → 物理语义**
- 「安装（Windows）」= 同一 MSI 三件套：cloudmountd（daemon）+ cloudmount（cli）+ cloudmount-desktop.exe 同铺 Program Files\CloudMount；Machine PATH Environment 指向该目录（既有不动）；快捷方式指向 desktop exe（范围待 Q2 裁决）。
- 「拖拽装（mac）」= .dmg 挂载 → 用户拖 cloudmount.app 入 /Applications；daemon/cli 仍走既有 pkg（不含 .app，A1 口径）。
- 「托盘」= tray-icon 常驻系统托盘 + 菜单恰两项：「显示主窗口」→ 唤起/聚焦主窗口；「退出」→ 终止进程；窗口关闭按钮语义待 Q1 裁决（intent 建议默认：隐藏到托盘）。
- 「有图标」= 单一来源 scripts/gen_icons.py → apps/desktop/icons/{icon.ico, *.png, *.icns}；.ico → winres 嵌 exe + MSI Icon 元素；.png → 托盘/窗口图标源；.icns → .app。
- 「发布覆盖新产物面」= tag push → release.yml 新 desktop job → 精确路径 artifact → release 挂载；无签名，tag 即正式发布（A1）。

**错误处理与边界**
- 构建失败：新 job 任一步失败 → release 链整体失败（upload if-no-files-found: error 口径），不出半成品发布。
- 未签名边界：SmartScreen「更多信息→仍要运行」/ Gatekeeper「右键打开」引导沿既有，不新增基础设施。
- 升级边界：已装旧版时新 MSI 走 MajorUpgrade 向前覆盖；desktop 快捷方式随 MajorUpgrade 的重建/清理行为由 Construction 实测确认（不设计专门迁移逻辑）。
- mac 边界：icns 本地（Windows）不可产 → 生成链必须落 CI mac runner 或预生成入仓（「Construction 首日验证③」二选一）。
- 应用层最小集边界：托盘菜单恰两项即封顶，多一项即越界（「约束」原文）。

**Construction 首日验证清单（如实标注，不断言可行——B1 教训）**
1. tray-icon 0.21 + eframe 0.31 集成形态（含 macOS 托盘/窗口操作主线程约束）。
2. winres 嵌 .ico 对独立 cargo 项目 cloudmount-desktop.exe 的构建影响。
3. icns 生成链（iconutil mac 独占：CI mac runner 生成 vs 预生成入仓）。
4. egui 关闭按钮拦截（隐藏到托盘语义）在 eframe 0.31 的 API 形态（落码形态取决于 Q1 裁决）。

**两层可测性总拆分**
- CI/静态可验：gen_icons 产物存在 + stale 路径清零；src 托盘菜单字面；.wxs 新增行/既有行 diff；release.yml job/needs/upload 字面；新断言全绿 + 既有锁定脚本零触碰。
- 本地人工冒烟（不可 CI 自动化）：exe 图标显示；托盘交互（显示/退出/关闭按钮）；MSI 安装后目录/快捷方式/PATH；dmg 挂载拖拽与 .app 启动（hdiutil/iconutil mac 独占，本地 Windows 无法验，仅 CI 产物存在性）。

## 开放问题（需人类裁决）
- [ ] Q1 窗口关闭按钮语义：隐藏到托盘（托盘常驻，托盘「退出」真退出——桌面应用惯例，intent 建议默认）还是真退出应用？影响 AC2④。
- [ ] Q2 Windows 快捷方式范围：开始菜单+桌面两处（intent 建议默认）还是仅开始菜单？影响 AC3②。
