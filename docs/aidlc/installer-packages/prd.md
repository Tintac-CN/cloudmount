# PRD: installer-packages

> 产出者：pm　日期：2026-10-15　来源：intent.md　修订：2026-10-15（gate1 打回，按附带裁决 M1/M3/M4/M7/M8/MINOR 修订）

## 用户故事

| # | 角色 | 目标 | 价值 | 溯源（intent.md） |
|---|---|---|---|---|
| US1 | Windows 安装用户 | 双击 `cloudmount-windows-x64.msi` 完成安装：cloudmountd.exe + cloudmount.exe 铺放至 `Program Files\CloudMount`，PATH 自动写入 | 免手动解压 zip、免自配 PATH，装完命令行直接可用 | 「问题与期望」L9（「用户需手动解压、自配 PATH」）、「受影响系统/用户」L14 |
| US2 | macOS 安装用户 | 双击/命令行安装 `cloudmount-darwin-<arch>.pkg`：cloudmountd + cloudmount 铺放至 `/usr/local/bin` | 免解压 tar、免手动挪文件进 PATH 目录 | 「问题与期望」L9、「受影响系统/用户」L14 |
| US3 | 维护者（发布者） | push `v*` tag 后 release.yml 自动产出 MSI 与 pkg 并附于**正式** GitHub Release，零手工步骤 | 发布语义收敛为「tag 即发布」，安装用户与二进制用户同批拿到产物 | 「一句话意图」L6、「期望的结果」L10、「约束」L21 |
| US4 | 安装用户（首次运行） | Release notes 明示无证书签名状态及 SmartScreen/Gatekeeper 绕过引导 | 首启不被安全拦截劝退，知道「仍要运行 / 右键打开」是预期行为而非缺陷 | 「约束」L17 |
| US5 | 维护者 | 既有 tar/zip/docker 产物与 job 零删改；新增结构断言落独立文件 | 发布面只增不减，上 intent 交付物回归风险受控 | 「约束」L22、L23 |

## 验收标准（可测）

- **AC1（Windows MSI 产出并附于正式 Release）**：Given 本期改动合入后收到 push tag `v*`，When windows runner 上的安装包 job 执行，Then 产出且仅产出 `cloudmount-windows-x64.msi`（artifact 名以 `cloudmount-` 开头、上传 `if-no-files-found: error`），其安装面为：cloudmountd.exe + cloudmount.exe 两文件铺放至 `Program Files\CloudMount` + Machine 级 PATH 写入 + 标准 MSI 卸载条目（WiX 缺省行为），**不含** SCM 服务注册（留用户命令 `cloudmountd service install`）与 desktop；该 job 进入 release job 的 `needs` 门控，MSI 经既有 `pattern: cloudmount-*` 收编后附于正式 Release。
  - 可测性拆解：job 结构 / runner / 产物名 / `needs` 门控 / `if-no-files-found` 由新断言脚本静态断言；安装面（目录、PATH、两 exe 文件清单）由 WiX 定义（`packaging/wix/main.wxs`，钉死）静态断言；「产出且仅产出」含负断言口径——不产出其他 MSI 文件、该 job 不上传 `cloudmount-windows-x64.msi` 之外的 artifact。
  - 测试锚：T1/T2/T3/T4/T16/T17/T18/T22。

- **AC2（mac pkg 产出并附于正式 Release）**：Given 同 AC1 触发，When macos runner（macos-15，顺序步双 target，同 binaries-macos 先例）上的安装包 job 执行，Then 每架构各产出 `cloudmount-darwin-aarch64.pkg` 与 `cloudmount-darwin-x86_64.pkg`（artifact 名以 `cloudmount-` 开头、上传 `if-no-files-found: error`），安装面为：cloudmountd + cloudmount 两文件铺放至 `/usr/local/bin`（目录缺省不存在时由包创建；无 preinstall/postinstall 脚本，纯文件铺放），**不含** launchd 服务注册与 desktop；该 job 进入 release `needs` 门控并附于正式 Release。
  - 测试锚：T5/T6/T7/T8/T19/T20。

- **AC3（既有产物与 job 零删改）**：Given 上 intent 交付基线（release.yml 六 job：binaries-linux / binaries-windows / binaries-macos / release / docker / merge，三断言脚本全绿），When 本期改动合入，Then ① `scripts/release-yaml.test.sh`、`scripts/check-macos-yaml.test.sh`、`scripts/package-macos.test.sh` 三文件零 diff 且运行仍全绿；② 既有 job 的 step 与产物名（`cloudmount-linux-*.tar.gz`、`cloudmount-windows-x64.zip`、`cloudmount-darwin-*.tar.gz`、`digests-*`）零删改；③ release job 仅允许两类追加：`needs` 数组追加新安装包 job、body 产物表追加 MSI/pkg 行与签名说明（签名说明为追加新行/新段，不改既有段——含既有 mac tar Gatekeeper 行）。
  - 可测性拆解：① 由脚本运行结果 + git diff 审查（gate）可测；② 的完整字节级保证由 gate 审查 diff 仅追加承载，新断言脚本承接既有产物名锚点（`cloudmount-linux-*.tar.gz`、`cloudmount-windows-x64.zip`、`cloudmount-darwin-*.tar.gz`、`digests-*` 与既有 job 名）——与锁定脚本重复无害（双保险）；「零删改」量词的实测口径即此二者。
  - 测试锚：T9/T10/T11/T15。

- **AC4（Release notes 产物表与无证书引导）**：Given 正式 Release 创建，When 阅读 release body，Then 产物表新增 3 行（MSI 1 行 + pkg 每 arch 一行共 2 行）：`cloudmount-windows-x64.msi`（Windows x64，安装包：daemon + CLI → `Program Files\CloudMount` + PATH）、`cloudmount-darwin-aarch64.pkg`（macOS arm64，安装包 → `/usr/local/bin`）、`cloudmount-darwin-x86_64.pkg`（macOS x64，安装包 → `/usr/local/bin`）；签名状态说明覆盖双平台：MSI 无 Authenticode（SmartScreen「更多信息→仍要运行」引导）、pkg 无 Developer ID 签名/公证（Gatekeeper 右键→打开 或 `xattr` 引导，与既有 mac tar 行说明同口径）；并含安装侧提示：MSI PATH 写入新开终端生效、per-machine 安装触发 UAC 提权为预期、升级安装先卸载再装。
  - 测试锚：T12（静态断言 body 含新产物行与关键词）。

- **AC5（「tag 即发布」= 直接正式发布）**：Given push tag `v*`，When release.yml 全流水线执行至 release job，Then 直接创建**正式** Release（softprops/action-gh-release 配置无 `prerelease: true`，流程中无预演 tag / 二次发布步骤）；本语义**覆盖**上 intent（macos-release）ops-runbook 的「首 tag prerelease 预演」裁决——该 runbook 为已完结 intent 工件不改，以本 PRD 与 decisions.md A1/gate1 落点为准；SOP 载体 = operations 阶段产出本 intent runbook，固化人工核验清单并显式声明取代 macos-release ops-runbook 的发布流程节；rc 形态 tag（如 `v1.0.0-rc1`）不在本期发布语义内。
  - 测试锚：T13/T17/T19/T21（静态断言无 prerelease 键、触发器仍为 `tags: ["v*"]`）。

- **AC6（新断言独立文件守护）**：Given 上 intent 三断言脚本已 TDD 锁定，When 本期新增安装包结构断言，Then 全部落独立新文件 `scripts/installers-yaml.test.sh`（钉死，复数命名），既有三脚本零修改；新脚本风格同先例：Git Bash 直跑、静态 grep/awk 式（不要求 yq）、actionlint 不可用时降级告警记缺口。
  - 测试锚：T11/T14（新脚本自身为交付物，运行全绿）。

## 非目标（明确排除）

- desktop（egui）不入任何安装包（A1 裁决，同上 intent 口径）。
- 服务注册不入安装器：Windows SCM 注册留用户命令 `cloudmountd service install`（daemon 已自管 service install/start/stop/uninstall 子命令）；mac 不做 launchd 服务安装。
- 代码签名与公证：不做 Authenticode / Developer ID / notarytool（无证书）；SmartScreen/Gatekeeper 引导由 Release notes 承载。
- Linux 安装包（deb/rpm/AppImage）：intent 仅要求 Windows MSI 与 mac 安装包。
- `.dmg` 形态：不实现（gate1 裁决 mac 形态为 `.pkg`）。
- 既有产物重构/删减：linux tar、windows zip、mac tar、docker/merge 全保留原样（AC3 口径）。
- check-macos.yml 扩展：编译面预检不在本期改动面（intent.md 改动面未列）。
- 安装包升级/迁移：升级语义不设计不保证，Release notes 提示先卸载再装。
- web-ui / Docker 面任何改动。

## 语义对齐（express 内联小节）

### 关键动词 → 物理语义映射

- 「安装（MSI）」= 手写 WiX 定义 `packaging/wix/main.wxs`（钉死；CI 直接 WiX，cargo-wix 不采用）——组件将 cloudmountd.exe、cloudmount.exe 铺放至 `Program Files\CloudMount`；写 Machine 级 PATH（新进程生效，已开终端不自动刷新——Release notes 提示新开终端）；per-machine 安装触发 UAC 提权为预期；WiX 缺省注册 Add/Remove Programs 卸载条目。**不**执行 `cloudmountd service install`（SCM 注册用户自管）。
- 「安装（pkg）」= `scripts/package-pkg-macos.sh`（钉死，沿 `package-macos.sh` 先例：staging 归集 + 前置硬校验 + dry-run 可本地跑——**dry-run 仅覆盖 staging+硬校验段，pkgbuild 调用段 mac 独占、本地不覆盖**，与先例的全路径 dry-run 不对等，gate1 复审 R2-MAJOR-1 边界声明）以 pkgbuild 将 cloudmountd、cloudmount 铺放至 `/usr/local/bin`（该目录在 macOS 默认 PATH 内；Apple Silicon 上缺省不存在时由包创建，属 root）；无 preinstall/postinstall 脚本（纯文件铺放）；receipt 记录 pkgutil 可查。**不**装 launchd。
- 「版本」= 安装器版本取值 `github.ref_name`（MSI ProductVersion 与 pkg `--version` 同源，不依赖 Cargo.toml bump；tag 的 `v` 前缀与 WiX ProductVersion 数值格式的适配为 Construction 落点）。
- 「tag 即发布」= push `v*` tag → release.yml 触发 → 全 job（含两个新安装包 job）→ release job `needs` 门控 → softprops/action-gh-release 直接创建**正式** Release（无 prerelease、无预演 tag）。**覆盖**上 intent「首 tag prerelease 预演」运维裁决（runbook 文件不改，以 decisions.md A1/gate1 为准；SOP 载体 = operations 阶段产出本 intent runbook，固化人工核验清单并显式声明取代 macos-release ops-runbook 的发布流程节）。
- 「附于正式 Release」= 新安装包 job 的 artifact 名以 `cloudmount-` 前缀（如 `cloudmount-windows-msi` / `cloudmount-darwin-pkgs`，具体名 Construction 定）→ 被 release job 既有 `pattern: cloudmount-*` + `merge-multiple: true` 自动收编 → `files: dist/*` 上传。release job 下载逻辑零改（兼容点）。

### 错误处理与边界

- 安装包 job 失败 → release `needs` 不满足 → 不发布（与既有 binaries job 同构的门控语义）。
- 产物缺文件不产出：打包步骤前置硬校验（文件不存在即非零退出，同 `scripts/package-macos.sh` 先例）+ 上传 `if-no-files-found: error` 兜底。
- 无证书签名为已知状态非缺陷：MSI 触发 SmartScreen、pkg 触发 Gatekeeper，Release notes 引导（AC4）。
- mac 双 target 产物同名异目录（既有先例）：pkg job 每 target 归集独立 staging 后各打一个 pkg，命名以 `<arch>` 区分（`cloudmount-darwin-aarch64.pkg` / `cloudmount-darwin-x86_64.pkg`，与 tar 命名同构）。

### 与既有 release.yml 机制的兼容（Construction 落点）

- download `pattern: cloudmount-*` 收编：新 artifact 命名必须落在该 glob 内（release job 下载步零改）。
- `if-no-files-found: error` 先例沿用。
- release `needs` 数组追加（不删既有项）；body 产物表仅追加行。
- 既有三断言脚本零修改；新断言独立文件（AC6）。

### Construction 首日验证（技术可行性，非本 PRD 断言）

- CI 侧 WiX Toolset 的安装方式与版本锁定（Construction 落点；cargo-wix 已裁决不采用——WiX v3 EOL 与双 bin 必手改权衡）。
- **WiX 大版本选择（v3 vs v4/v5，MINOR⑨ 补）**：v3 与 v4+ 语法不兼容（.wxs 写法随之不同）；v3 可 choco 直装，v4/v5 需 dotnet SDK（本机实测仅 runtime 8.0.32 无 SDK）——首日随安装方式一并定版本并锁定，`packaging/wix/main.wxs` 语法随之。
- **UTF-8 中文元数据风险（MINOR⑫ 补）**：daemon/cli 的 Cargo.toml description 全中文，将流入 .wxs ARP 描述——WiX v3 对无 BOM UTF-8 源按 ANSI 处理可能乱码/报错，首日验证（缓解：.wxs 存为 UTF-8 with BOM 或 ARP 描述改用英文）。
- pkgbuild/productbuild 在 macos-15 runner 的最小可用链路（零新依赖验证）。

## 开放问题（已裁决：2026-10-15 gate1）

> 四项全部裁决（decisions.md 留痕），无待裁决项。

- [x] **mac 安装形态**：已裁决（2026-10-15 gate1，decisions.md 留痕）——`.pkg`。
- [x] **Windows 工具链**：已裁决（2026-10-15 gate1，decisions.md 留痕）——手写 `.wxs` + CI 直接 WiX（cargo-wix 不采用：WiX v3 EOL 与双 bin 必手改权衡）；CI 装 WiX 的安装方式与版本锁定留 Construction 落点。
- [x] **MSI 是否写 PATH 环境变量**：已裁决（2026-10-15 gate1，decisions.md 留痕）——写，Machine 级。
- [x] **安装位置**：已裁决（2026-10-15 gate1，decisions.md 留痕）——`Program Files\CloudMount`（Windows）/ `/usr/local/bin`（mac）。
