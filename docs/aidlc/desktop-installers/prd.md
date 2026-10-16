# PRD: desktop-installers

> 产出者：pm　日期：2026-10-16　来源：intent.md（2026-10-16 gate1 裁决回填修订）

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

- **AC1（图标管线，S5）**：Given scripts/gen_icons.py 已重写（品牌沿既有像素设计：上 1/3 白（云）/浅色分隔/下 2/3 蓝（盘）双色方块，输出目录改为 `apps/desktop/icons/`），When 本地运行 `python scripts/gen_icons.py`，Then ① `apps/desktop/icons/` 产出 icon.ico（≥1 帧有效 ICO，现有基线 32×32 PNG-in-ICO）、PNG 组（现有基线 32×32 与 128×128；如需扩尺寸由 Construction 落，不写死未实测值）与 icons.rs（Rust RGBA 常量源文件：尺寸+RGBA 字节组——apps/desktop include 后喂 tray-icon `Icon::from_rgba` / egui `IconData`，零新增 crate 依赖，gate1 r2 裁决）② 全仓库对 `apps/desktop/src-tauri` 的图标路径引用清零（grep 验证）③ 重复运行幂等。**且**：Given apps/desktop 构建配置含 .ico 资源嵌入（winres 方案——其构建影响见「Construction 首日验证②」），When 本地 `cargo build --release`，Then target/release/cloudmount-desktop.exe 含图标资源。验证层：静态断言=产物文件存在+stale 路径清零+构建配置字面；人工冒烟=资源管理器/文件属性中 exe 显示品牌图标。测试锚：TP-06/TP-11/TP-12/TP-16。
- **AC2（托盘接线，S3）**：Given apps/desktop/src/main.rs 接入已声明的 tray-icon 0.21，When 对 apps/desktop/src 静态检查，Then ① grep tray 有命中（托盘初始化存在）② 托盘菜单字面含且**仅含**两项：「显示主窗口」「退出」③ 「显示主窗口」绑定唤起主窗口、「退出」绑定终止进程（菜单项回调字面可断言）④ 窗口关闭按钮语义=**隐藏到托盘**（Q1 已裁决，2026-10-16 gate1，decisions.md 留痕）：点关闭 → 拦截关闭请求、窗口隐藏、托盘常驻，托盘「退出」真退出；eframe 0.31 落码形态=ctx.input(viewport().close_requested) + ViewportCommand::CancelClose（dev 评审 m1 API 实证，API 形态验证见「Construction 首日验证④」；若该验证触发降级，AC2④ 与 TP-21 断言字面随语义收窄同步修订——连锁锚，gate1 r2）。人工冒烟清单（Win/mac 各一遍）：托盘图标可见（用 AC1 产物）→ 点「显示主窗口」窗口重现 → 点「退出」进程结束 → 点窗口关闭按钮=隐藏到托盘（应用仍常驻）。测试锚：TP-09/TP-20/TP-21。
- **AC3（Windows MSI 扩展，S1）**：Given packaging/wix/main.wxs 增 desktop 组件，When 既有 installer-msi-windows job 构建 MSI（同 job 追加 desktop 构建与归集步——gate1 B2 精确口径，desktop exe 不另立 Windows job，三件套由该 job 追加步产出，与 AC5④ 同口径；**追加步次序约束**：desktop 构建+归集步须插在该 job 既有「校验构建输入/candle」步之前——纯尾部追加物理产不出三件套 MSI），Then ① cloudmount-desktop.exe 组件铺 Program Files\CloudMount（与 daemon/cli 同目录）② 快捷方式指向 desktop exe，**开始菜单+桌面两处**（Q2 已裁决，2026-10-16 gate1，decisions.md 留痕）③ MSI 内嵌品牌图标（Icon 元素 + Shortcut Icon / ARPPRODUCTICON，源自 AC1 的 icon.ico）；desktop 组件升级沿「先卸载再装」提示口径（MajorUpgrade 向前覆盖，desktop 面无专门迁移设计——沿 installer-packages 非目标）④ 回归零破坏：daemon/cli 组件铺放、Machine PATH Environment、MajorUpgrade、Name="CloudMount"、UpgradeCode、Manufacturer、perMachine、UTF-8 BOM 等既有字面不变（diff 断言；main.wxs L8-11 既有头注「不含桌面快捷方式」随本期 desktop 组件纳入而失实——Construction 顺带更新该注记，注记行不计入守护）。注：installers-yaml.test.sh 的 T4 两条负断言字面收窄修订已获 gate1 授权（B1 裁决，decisions.md 留痕），Construction 期执行解锁→修订→重锁（沿 T16 先例）——不影响本条对 main.wxs 的守护口径。验证层：静态断言=新断言脚本对 .wxs 字面；人工冒烟=本地 msiexec 安装后三件套目录齐全、两处快捷方式存在且图标正确、PATH 仍指向安装目录。测试锚：TP-03/TP-04/TP-05/TP-13/TP-14/TP-15/TP-17/TP-18/TP-23。
- **AC4（mac .app + .dmg，S2）**：Given 新增 scripts/package-dmg-macos.sh（Info.plist + icns + hdiutil），When macos runner 执行打包，Then ① 产出 cloudmount.app（含二进制 cloudmount-desktop、icns 图标、Info.plist 声明）——**aarch64-only 单架构**（沿 macos-15 单 runner 先例；Intel mac 无 desktop 交付为已知取舍，gate1 落字）② hdiutil 产出 .dmg（aarch64-only，文件名/卷名 Construction 落定，建议 cloudmount-desktop.dmg），可挂载拖拽安装至 /Applications（dmg 内 Applications 链接为该语义落地惯例，Construction 落）；.app 未签名 Gatekeeper 引导（右键打开）沿既有口径，落 dmg 产物页/release body 文案（见 AC5⑤）③ daemon/cli 既有 pkg 不含 .app 且内容零变动（A1 裁决口径）。**本地不可验项如实标注**：hdiutil/iconutil 为 mac 独占，本地 Windows 无法验证 dmg 产物与 icns 生成——自动化层仅 CI 产物存在性（artifact 精确路径 upload + if-no-files-found: error 口径沿上 intent）；icns 生成链（CI mac runner 生成 vs 预生成入仓）为「Construction 首日验证③」，不断言哪条可行。人工冒烟=下载 dmg → 挂载 → 拖入 /Applications → 图标正确 → 启动有界面（Gatekeeper 首启引导可用）。测试锚：TP-07/TP-08/TP-19/TP-23/TP-24。
- **AC5（release.yml desktop 面，S4）**：Given .github/workflows/release.yml 扩展 desktop 面（Windows：既有 installer-msi-windows job **追加** desktop 构建与归集步产出三件套 MSI——gate1 B2 精确口径；mac：**新增** macos-15 job 构建打包产 .app + .dmg），When push tag（tag 即发布=直接正式，沿 A1），Then ① desktop 构建在 apps/desktop 独立 cargo 项目上执行（cd apps/desktop 或 --manifest-path——workspace exclude 口径；windows 侧在 installer-msi-windows 追加步内、mac 侧在**新增** macos-15 job——区别于既有 installer-pkg-macos 同 runner job）② 新 artifact 以精确路径 upload（if-no-files-found: error 口径沿上 intent）且新 artifact 名以 `cloudmount-` 前缀（收编 glob 前提，镜像上 intent 断言先例；精确名 Construction 落）③ release job needs 链追加（新 job 入链；既有 needs 元素零删改）；body 行追加及 ⑤ 所指 MSI 行描述更新为授权改动（gate1 r2——就地改写仅限 MSI 行描述，其余既有 7 行零删改）④ 零删改守护（B2 精确口径，installer-msi-windows 自零触碰集合移出）：binaries-linux/binaries-windows/binaries-macos/installer-pkg-macos 四 job 字面零触碰；installer-msi-windows 既有构建命令/铺放语义/upload if-no-files-found 行不删改，允许追加 desktop 构建与归集步（AC3 三件套即由该 job 追加步产出——与 AC3① 同口径）⑤ release body 产物表为 **8 行口径**：既有 7 行零删改（linux×2/windows zip/darwin tar×2/MSI/darwin pkg×2）+MSI 行描述更新（三件套：daemon+cli+desktop）+追加 .dmg 行（含 .app Gatekeeper 引导文案，与 AC4② 同源）。验证层：静态断言（新断言脚本对 yml/body 字面：job 名、needs 元素、upload 路径与前缀、error 口径、body 8 行表——7 既有行零删改+MSI 行描述更新+.dmg 追加行）；无人工冒烟项。测试锚：TP-01/TP-02/TP-18/TP-22。
- **AC6（断言守护独立文件，S4/S5）**：Given 新断言落 scripts/desktop-*.test.sh 独立文件（文件拆分与命名 Construction 落），When 运行全部新断言与既有测试面，Then ① 新断言全绿且覆盖 AC1–AC5 全部静态可断言面 ② 既有锁定脚本字面零触碰——其中 installers-yaml.test.sh 的 T4 两条负断言字面收窄修订**已获 gate1 授权**（B1 裁决，decisions.md 留痕）：Construction 期执行解锁→修订→重锁，沿 T16 先例；除该两断言外其余断言与脚本字面零触碰。验证层：纯静态。测试锚：TP-10/TP-03。

## 非目标（明确排除）
- 单实例、开机自启动、自动更新（intent「约束」明确不做）。
- 代码签名（无证书不变；SmartScreen/Gatekeeper 引导沿既有口径，本期不新增签名基础设施）。
- 升级语义设计（沿 installer-packages 非目标：不设计，MajorUpgrade 向前覆盖可用 + 提示先卸载再装）。
- daemon/cli 既有产物、pkg、CI job 的任何删改；mac pkg 打入 .app（A1：.app 只走 .dmg）。
- 托盘菜单超出「显示主窗口/退出」的任何项（设置、暂停同步、关于等不发明需求）。
- desktop exe 版本号元数据注入 / 关于页版本展示（intent 未要求；CI 版本注入仅沿既有 daemon/cli 面）。
- Linux 桌面打包、dmg 增量更新/Sparkle 机制（intent 未要求）。
- 修改既有锁定测试脚本（installers-yaml.test.sh 等四个——新断言独立文件；唯一例外=gate1 B1 授权的 T4 两条负断言字面收窄，Construction 期解锁修订重锁，沿 T16 先例）。

## 语义对齐（express 内联小节）
**关键动词 → 物理语义**
- 「安装（Windows）」= 同一 MSI 三件套：cloudmountd（daemon）+ cloudmount（cli）+ cloudmount-desktop.exe 同铺 Program Files\CloudMount；Machine PATH Environment 指向该目录（既有不动）；快捷方式指向 desktop exe，开始菜单+桌面两处（Q2 已裁决，gate1）；升级沿「先卸载再装」提示。
- 「拖拽装（mac）」= .dmg（aarch64-only）挂载 → 用户拖 cloudmount.app 入 /Applications；daemon/cli 仍走既有 pkg（不含 .app，A1 口径）；Intel mac 无 desktop 交付为已知取舍。
- 「托盘」= tray-icon 常驻系统托盘 + 菜单恰两项：「显示主窗口」→ 唤起/聚焦主窗口；「退出」→ 终止进程；窗口关闭按钮=隐藏到托盘（Q1 已裁决，gate1；落码形态见「Construction 首日验证④」）。
- 「有图标」= 单一来源 scripts/gen_icons.py → apps/desktop/icons/{icon.ico, *.png, *.icns, icons.rs}；.ico → winres 嵌 exe + MSI Icon 元素；.png → 源素材（ico/icns 生成输入）；.icns → .app；icons.rs（Rust RGBA 常量源：尺寸+RGBA 字节组）→ apps/desktop include 后喂 tray-icon `Icon::from_rgba` / egui `IconData`（运行期图标解码链，零新增 crate 依赖——gate1 r2 裁决）。
- 「发布覆盖新产物面」= tag push → release.yml desktop 面（installer-msi-windows 追加步 + 新增 macos-15 job）→ 精确路径 artifact（新 artifact 名一律 `cloudmount-` 前缀，收编既有 glob 口径）→ release 挂载（body 产物表 8 行口径：既有 7 行零删改——linux×2/windows zip/darwin tar×2/MSI/darwin pkg×2——+MSI 行描述更新为三件套+追加 .dmg 行含 Gatekeeper 引导）；无签名，tag 即正式发布（A1）。

**错误处理与边界**
- 构建失败：新 job/追加步任一步失败 → release 链整体失败（upload if-no-files-found: error 口径），不出半成品发布。
- 未签名边界：SmartScreen「更多信息→仍要运行」/ Gatekeeper「右键打开」引导沿既有，不新增基础设施。
- 升级边界：已装旧版时新 MSI 走 MajorUpgrade 向前覆盖 + 提示先卸载再装（desktop 同口径）；desktop 快捷方式随 MajorUpgrade 的重建/清理行为由 Construction 实测确认（不设计专门迁移逻辑）。
- 多实例边界：托盘常驻 + 快捷方式双开 = 双实例并存为**已知行为，非缺陷**（无单实例机制，沿非目标）。
- 托盘初始化失败边界：托盘初始化失败时降级为**无托盘运行**（应用主体可用，不 panic）。
- mac 边界：icns 本地（Windows）不可产 → 生成链必须落 CI mac runner 或预生成入仓（「Construction 首日验证③」二选一）。
- 应用层最小集边界：托盘菜单恰两项即封顶，多一项即越界（「约束」原文）。

**Construction 首日验证清单（如实标注，不断言可行——B1 教训）**
1. tray-icon 0.21 + eframe 0.31 集成形态（含 macOS 托盘/窗口操作主线程约束）——若集成不可行，属**升级人类决策路径**：触发 AC2 整体重审，不自行降级落码。
2. winres 嵌 .ico 对独立 cargo 项目 cloudmount-desktop.exe 的构建影响。
3. icns 生成链（iconutil mac 独占：CI mac runner 生成 vs 预生成入仓）。
4. egui 关闭按钮拦截：dev 评审 m1 API 实证——eframe 0.31 **无 on_close_event**，官方路径=ctx.input(viewport().close_requested) + ViewportCommand::CancelClose；降级路径：该 API 不可行时回退为**关闭即真退出**，托盘仍保持「显示主窗口/退出」两项（Q1 语义收窄，不整体重审；触发时 AC2④/TP-21 字面随语义收窄同步修订——连锁锚）。

**两层可测性总拆分**
- CI/静态可验：gen_icons 产物存在（含 icons.rs）+ stale 路径清零；src 托盘菜单字面；.wxs 新增行/守护行 diff；release.yml job/needs/upload 前缀/body 8 行表字面（7 既有行零删改）；新断言全绿 + 既有锁定脚本零触碰（T4 两断言按 gate1 B1 授权修订除外）。
- 本地人工冒烟（不可 CI 自动化）：exe 图标显示；托盘交互（显示/退出/关闭按钮隐藏语义）；MSI 安装后目录/两处快捷方式/PATH；dmg 挂载拖拽与 .app 启动（hdiutil/iconutil mac 独占，本地 Windows 无法验，仅 CI 产物存在性）。

## 开放问题（均已裁决——2026-10-16 gate1，decisions.md 留痕）
- [x] Q1 窗口关闭按钮语义：已裁决=**隐藏到托盘**（托盘常驻，托盘「退出」真退出）；已落字面至 AC2④、语义对齐「托盘」。
- [x] Q2 Windows 快捷方式范围：已裁决=**开始菜单+桌面两处**；已落字面至 AC3②、语义对齐「安装（Windows）」。
