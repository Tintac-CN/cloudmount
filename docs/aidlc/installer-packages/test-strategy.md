# Test Strategy: installer-packages

> 产出者：qa　日期：2026-10-15　来源：intent.md + prd.md
>
> AC 锚位说明：已对齐 prd.md 实际 AC1..AC6（对齐拍回填）：**AC1**=Windows MSI 产出并附正式 Release；**AC2**=mac pkg 产出并附正式 Release；**AC3**=既有产物与 job 零删改；**AC4**=Release notes 产物表与无证书引导；**AC5**=「tag 即发布」直接正式发布；**AC6**=新断言独立文件守护。无 T 点落点的 AC 不得存在——6 条 AC 全部有 T 点落点。
> 已知事实（编排者核查）：release.yml 仅 push tag `v*` 触发（PR 不跑）——安装包产物真实验证只能 tag 后人工核验；本地=Windows/Git Bash（**MSI 可下载后本机 msiexec 静默验证**；无 mac 机/无 codesign/pkgbuild）；上 intent 三断言脚本已 TDD 锁定（`scripts/package-macos.test.sh`、`scripts/release-yaml.test.sh`、`scripts/check-macos-yaml.test.sh`）——新断言必须落**独立文件**（免与已锁文件冲突，retro 学习）；无证书签名：MSI SmartScreen/pkg Gatekeeper 为**预期拦截负路径**；断言字面（job 命名/runner pin/PATH 写否/安装位置/.pkg vs .dmg/cargo-wix vs WiX）待 prd/gate1 裁决后写死。

## 测试点清单

| # | 映射 AC | 场景（正/负路径） | 层级 | 优先级 |
|---|---|---|---|---|
| T1 | AC1 | 正：release.yml 含 Windows MSI 安装包 job 定义（新增 job；命名与 runner 按 prd/gate1 裁决字面写死，如 `installer-msi-windows:` + `runs-on: windows-latest`）；断言基于剥 CRLF 的规范化副本，job 块不存在即 FAIL（防空真） | 单元 | P0 |
| T2 | AC1 | 正+负：MSI job 产出命名 `cloudmount-windows-x64.msi` 字面（打包步输出名/upload path 严格匹配）；upload 步含 `if-no-files-found: error`——**负路径：产物缺文件（glob 落空）即 job 整体失败，不静默产出缺安装包的 Release** | 单元 | P0 |
| T3 | AC1 | 正：MSI 打包对象含 `cloudmountd.exe`+`cloudmount.exe` 字面（daemon+cli 范围，A1 裁决）；安装位置/PATH 配置字面（`Program Files\CloudMount`+PATH 环境变量——Q3/Q4 裁决后写死；若裁决不写 PATH 则改为负断言其不写）；断言对象=release.yml job 块+安装器定义文件（.wxs/cargo-wix 配置，路径 Construction 定） | 单元 | P1 |
| T4 | AC1 | 负（范围排除）：MSI job 块与安装器定义不含 desktop/cloudmount-desktop 字面、不含服务注册字面（ServiceInstall/service install/SCM 注册）——A1 裁决：安装器只铺文件+PATH，SCM 注册留 `cloudmountd service install` 用户命令 | 单元 | P1 |
| T5 | AC2 | 正：release.yml 含 mac pkg 安装包 job 定义（新增 job；命名/runner pin 裁决后写死，如 `installer-pkg-macos:` + `runs-on: macos-15`）；同 T1 防空真口径 | 单元 | P0 |
| T6 | AC2 | 正+负：pkg job 产出命名双 arch 字面（`cloudmount-darwin-aarch64.pkg`+`cloudmount-darwin-x86_64.pkg`，即 `cloudmount-darwin-<arch>.pkg` 形态）；upload 步含 `if-no-files-found: error`——**负路径同 T2：缺产物即 job 失败** | 单元 | P0 |
| T7 | AC2 | 正：pkg job 调用 pkgbuild/productbuild 字面（原生工具零新依赖，A1 拆分预览②）；安装目标 `/usr/local/bin` 字面（staging 布局或打包脚本内体现，字面 Construction 落） | 单元 | P1 |
| T8 | AC2 | 负（范围排除）：pkg job 块与安装器定义不含 desktop/.app/CloudMount.app 字面（A1 范围守护，沿用上 intent T5 模式） | 单元 | P1 |
| T9 | AC3 | 正+负：release job `needs` 含两个新安装包 job（仅追加）——**负路径语义：安装包构建失败→needs 门控→release 不执行、不发布**；安装包 job 块内无 `continue-on-error` 字面（防构建失败被吞、门控旁路） | 单元 | P0 |
| T10 | AC3 | 负（护栏，沿用上 intent T11 模式）：release needs 仍含 binaries-linux/binaries-windows/binaries-macos（**既有腿零删改**）；release job 块内无 `if:` 行、无 status 函数（always/success/failure/cancelled）——防 needs 门控被覆盖削弱 | 单元 | P0 |
| T11 | AC3/AC6 | 正（回归护栏）：上 intent 三锁定断言脚本零修改且继续全绿——既有 job 内部结构由锁定脚本守护（release-yaml.test.sh 等已断言 binaries-macos/needs/on: 等），新脚本不重复断言既有 job 内部；新断言落独立文件 `scripts/installers-yaml.test.sh` | 单元（回归） | P0 |
| T12 | AC4 | 正：release job 产物表含 MSI/pkg 行字面（`cloudmount-windows-x64.msi`、`cloudmount-darwin-aarch64.pkg`、`cloudmount-darwin-x86_64.pkg`）+ 无证书签名状态说明措辞（MSI SmartScreen/pkg Gatekeeper 引导） | 单元 | P1 |
| T13 | AC5 | 正+负：release job 块内无 `prerelease: true` 与 `draft: true` 字面（A1 裁决：tag 即直接正式发布，覆盖上 intent 首 tag prerelease 预演；显式 `prerelease: false` 允许）；on: 保持仅 push tags `v*`（无 workflow_dispatch——触发语义零改动，沿用上 intent R2 守护） | 单元 | P0 |
| T14 | AC6 | 正：新断言脚本独立成文件 `scripts/installers-yaml.test.sh`（零触碰三锁定脚本）；TDD 红→锁→绿：实施前新断言全 FAIL（红态预期）、实施后全绿、锁定后不得弱化；脚本设计约束（上 intent 学习沿用）：防空真（被测文件缺失/空块→FAIL 不静默通过；负断言限定块内白名单防误伤）、CRLF 剥除（`tr -d '\r'` 规范化副本）、计数诚实（降级分支计入总数，末行 `<N>/<M> passed`） | 单元 | P0 |
| T15 | AC1/AC2/AC3 | 正+负：actionlint（可用时）对 release.yml 零错；对注入语法错误的临时副本（mktemp 生成）确认非零（拦截手段自校验）；不可用→`SKIP`+降级 YAML 解析（python yaml.safe_load，再降级键配平粗检）+ 降级告警记缺口（降级分支计数诚实）——沿用上 intent T13 模式，内嵌于 installers-yaml.test.sh | 集成 | P1 |
| T16 | AC1 | 条件性正（Construction 首日、工具链选型可用时）：本地 Windows cargo-wix 冒烟——本地构建 MSI→`msiexec /i /qn` 静默安装→PATH 生效→卸载干净（判据同 T18），把 MSI 安装行为验证**提前到 tag 前**；工具链本地不可装/选型为 CI 侧 WiX 则记缺口，退化为 tag 后人工 | 集成（本地工具链） | P1 |
| T17 | AC1/AC5 | 正（线上人工）：tag 后 Release 资产含 `cloudmount-windows-x64.msi`，可下载；体积带复核（对照 windows zip 量级+打包开销，异常大/小人工复核） | E2E | P0 |
| T18 | AC1 | 正+负（线上产物人工核验，**执行环境=本机 Windows**——如实标注：MSI 静默装/卸可本地执行）：`msiexec /i cloudmount-windows-x64.msi /qn` 退出码 0；`Program Files\CloudMount` 下 cloudmountd.exe/cloudmount.exe 在位；PATH 生效（新 shell `where cloudmount`/`cloudmount --version` 冒烟）；**负路径=无 Authenticode 签名被 SmartScreen 提示为预期**（Release notes 引导核验）；卸载 `msiexec /x /qn`（或控制面板）→ 文件+PATH 清理干净 | E2E | P0 |
| T19 | AC2/AC5 | 正（线上人工）：tag 后 Release 资产含 `cloudmount-darwin-aarch64.pkg`+`cloudmount-darwin-x86_64.pkg`，可下载；体积带复核（对照 mac tar 量级） | E2E | P0 |
| T20 | AC2 | 正+负（线上人工，**需真 mac 机**——本地无 mac 无法执行，如实标注）：`sudo installer -pkg <pkg> -target /`（或双击）→ `/usr/local/bin` 下 cloudmountd/cloudmount 在位可执行、`cloudmount --version` 冒烟；**负路径=无 Developer ID 签名/公证被 Gatekeeper 拦截为预期**（notes 引导右键打开/`xattr -cr` 可解除）；卸载干净（删 /usr/local/bin 两文件）；可选：x86_64 包经 Rosetta 冒烟 | E2E | P0 |
| T21 | AC5 | 正（线上人工）：push tag → Release 为**正式**（非 prerelease/draft）；产物表含 MSI/pkg 行+签名状态说明；既有资产（linux tar/windows zip/mac tar）仍在（零删改，intent 约束口径） | E2E | P0 |
| T22 | AC1 | 正（可选，边缘）：MSI 双击交互安装 UI 路径冒烟（静默路径已覆盖核心；UI 安装/卸载为补充） | E2E | P2 |

负路径覆盖核对：安装包构建失败→job 失败→needs 门控不发布（T9 needs+无 continue-on-error、T10 无 if:/status 覆盖）；产物缺文件不产出（T2/T6 `if-no-files-found: error`）；既有腿零删改（T10 needs 护栏+T11 锁定脚本回归+T21 既有资产核验）；范围排除（T4 服务注册/desktop、T8 desktop/.app）；发布语义负断言（T13 无 `prerelease: true`/`draft: true`+on: 守护）；签名拦截预期负路径（T18 SmartScreen、T20 Gatekeeper）；拦截手段自校验（T15 注入语法错误副本）。每条主流程 ≥1 负路径，满足。

## 分层策略

- **单元**：新结构断言脚本 `scripts/installers-yaml.test.sh`（本地 Git Bash 直跑，静态 grep/awk 断言 release.yml+安装器定义文件；T1-T14）。测：MSI/pkg job 存在与结构（runner/命名）、产物命名严格匹配、upload `if-no-files-found: error`、needs 门控+零删改护栏+无 `if:`/status/`continue-on-error` 覆盖、范围排除（desktop/服务注册）、`prerelease: true`/`draft: true` 负断言、on: 守护、脚本自身交付（独立文件+TDD 红→锁→绿+防空真/CRLF 剥除/计数诚实）。不测：真实 MSI/pkg 构建、真实安装行为、runner 真实执行、既有 job 内部结构（由上 intent 锁定脚本守护，新脚本不重复断言）。
- **集成**：actionlint 整文件校验+注入语法错误自校验（不可用降级 python YAML 解析/键配平，T15）；条件性本地 cargo-wix 冒烟（T16，Construction 首日工具链验证——MSI 安装行为唯一 tag 前验证路径）。不测：GitHub Actions 真实调度语义（needs 门控行为依赖平台语义，静态断言其结构前提：needs 在位+无覆盖字面）。
- **E2E**：线上人工（release.yml 仅 tag `v*` 触发、PR 不跑——安装包产物只能 tag 后核验）：T17/T19 Release 资产+体积带；T18 MSI 静默装/PATH/卸载（执行环境=本机 Windows，属线上产物人工核验）；T20 pkg 真机安装（需真 mac 机）；T21 正式 Release 语义+既有资产零删改；T22 UI 安装冒烟（P2）。人工核验清单固化见缺口节（首个含安装包的 tag 即真实发布）。

## 缺口与开放问题

- [ ] **【缺口·E2E 不可自动化】MSI/pkg 真实安装行为无法 CI 自动验证**（release.yml 仅 tag 触发，PR 不跑）。MSI 核验执行环境=本机 Windows（下载后 msiexec 静默装/卸可本地执行，T18）；mac pkg 安装只能真 mac 机（T20；无 mac 机则降级为资产完整性+体积核验并记缺口）。**人工核验清单（发布 SOP 固化）**：① Release 资产含 `cloudmount-windows-x64.msi`+双 arch pkg，可下载，体积带复核；② MSI：`msiexec /i /qn`→Program Files\CloudMount 两 exe 在位→新 shell PATH 生效→SmartScreen 提示为预期→`msiexec /x /qn` 卸载干净（文件+PATH）；③ pkg：真机 `installer -pkg`→/usr/local/bin 两文件在位可执行→Gatekeeper 拦截为预期（右键打开/`xattr -cr` 解除）→卸载干净；④ 产物表含 MSI/pkg 行+签名状态说明；⑤ 既有资产仍在（零删改）。
- [ ] **【缺口·工具链】cargo-wix/WiX 工具链在 CI windows runner 的可用性属 Construction 首日验证**（选型本身为开放问题 Q2/gate1 裁决）；本地 Windows 可先行冒烟（T16 条件性）——不可用则 MSI 安装行为验证完全退化为 tag 后人工。
- [ ] **【缺口·首次即生产】「tag 即发布」=直接正式**（A1 裁决，覆盖上 intent 首 tag prerelease 预演）——无 prerelease 预演路径：**首个含安装包的 tag 即真实发布**，上述人工核验清单必须固化进发布 SOP 且打 tag 前就绪。
- [ ] **【缺口·拦截手段依赖】actionlint 本地 Windows/Git Bash 未必可用**；降级 YAML 解析+结构断言（T15），降级分支计数诚实；工具可用性 Construction 首日确认并回写。
- [ ] **【开放·字面待裁决】**MSI/pkg job 命名与 runner pin、MSI 是否写 PATH（Q3）、安装位置（Q4）、.pkg vs .dmg（Q1）、cargo-wix vs WiX（Q2）——断言字面待 prd/gate1 裁决后写死；当前测试点以占位描述，Construction 落字面（对齐拍/门控回执校正映射列）。
- [ ] 映射列已经对齐拍回填为 prd 实际 AC1..AC6；全部 AC 有 T 点落点，无缺锚。
