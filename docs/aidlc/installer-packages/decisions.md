# Decisions: installer-packages

> 只追加（Edit 被 guard-gates deny，一律 Write 全文+追加）；人类裁决、门控回执、降级事件逐字记录于此。

---

2026-10-15 A1 意图接入确认（AskUserQuestion 首次呈批未获选择；用户对话逐字裁决，逐字转写）：
- 意图原话：`windows需要提供msi安装包，mac也需要。请继续推进自动化发布和打包工作，tag即发布`（逐字）。
- 识别的意图一句话：Windows 提供 MSI 安装包、mac 提供安装包（pkg/dmg），扩展 release.yml 自动化发布——tag 即发布。
- 拆分预览（5 项）：①Windows MSI——windows runner 上 cargo-wix（WiX 封装）生成 `.wxs` 并打包 cloudmountd.exe+cloudmount.exe → `cloudmount-windows-x64.msi`（Program Files+PATH；SCM 服务注册留 `cloudmountd service install` 用户命令）②mac 安装包——macos runner 上 pkgbuild/productbuild（原生零新依赖）打包 daemon+cli → `cloudmount-darwin-<arch>.pkg`（/usr/local/bin；.dmg 备选 gate1 裁决）③release.yml 扩展——两腿各增安装包 job、产物上传 + Release 产物表补 MSI/pkg 行（无证书签名状态说明）④结构断言测试（TDD）——新断言脚本守护 MSI/pkg job 结构与产物命名⑤发布语义——「tag 即发布」= push tag 直接正式发布（是否覆盖上 intent「首 tag prerelease 预演」运维裁决——呈请裁决）。
- 仓库事实（编排者核查，已呈用户）：全仓零 wix/msi/pkg/dmg 痕迹（安装包打包全新领域）；daemon 已有 `service install/start/stop/uninstall` 子命令（SCM 注册应用自管，安装器只需铺文件+PATH）；desktop 为独立 cargo 项目（egui，上 intent A1 已排除出 mac 产物）；上 intent（macos-release）已交付 release.yml mac 腿/check-macos.yml/package-macos.sh 与三断言脚本（TDD 锁定）。
- 呈批四问（AskUserQuestion 呈出，用户对话一句话裁决）：
  问：「意图「MSI/pkg 安装包 + tag 即发布自动化」——是否进入 AIDLC 生命周期？」
  问：「规模分级路线：」
  问：「安装包产物范围（与上 intent 同口径为 daemon+cli）：」
  问：「「tag 即发布」语义（是否覆盖上 intent的首 tag prerelease 预演裁决）：」
  答（对全部四问）：「进入，express，daemon+cli，直接正式发布」
- 裁决落点（编排者执行）：建 intent `installer-packages`（route=express）；产物范围=daemon+cli（MSI/pkg 只装 cloudmountd+cloudmount；SCM 服务注册留用户命令；desktop 不入本期）；「tag 即发布」= push `v*` tag 直接触发全流水线并发布正式 Release——**覆盖 macos-release intent ops-runbook 的「首 tag prerelease 预演」裁决**（该 runbook 为已完结 intent 工件不改，以本条为准）；Inception 不派 architect、不产 architecture.md，语义对齐并入 prd.md 内联小节；门控回执/decisions 逐字留痕/TDD 锁/hook 保护与 classic 完全一致。

---

2026-10-15 Inception 对齐拍记录（编排者）：
- express 两工件齐：prd.md（6 AC + 语义对齐内联小节 + 开放问题 4 项附建议默认值：Q1 mac 形态 .pkg（推荐）/Q2 Windows 工具链 cargo-wix（推荐，供应链面仅 CI 侧，workspace bin 支持形态 Construction 首日验证）/Q3 MSI 写 PATH（推荐）/Q4 安装位置 Program Files\CloudMount + /usr/local/bin（推荐））+ test-strategy.md（22 测试点，P0=14/P1=7/P2=1；负路径：构建失败→needs 门控不发布（含无 continue-on-error 旁路防护）/产物缺文件不产出/既有腿零删改护栏（needs 护栏+三锁定脚本回归+既有资产核验）/desktop 与服务注册排除/发布语义负断言（无 prerelease:true/draft:true+on: 守护）/SmartScreen/Gatekeeper 预期拦截；MSI 静默装/卸=线上产物人工核验但执行环境=本机 Windows（如实标注）；mac pkg 只能真机）。
- **对齐拍检出双向错位**：qa 映射列按 A1 拆分预览 5 项锚定（prd 实际 6 条 AC），pm 建议锚为预估编号——两角色各派回只做引用回填（测试点本体零改动）：T11→AC3/AC6、T12→AC4、T14→AC6；prd AC1..AC6 括注锚位改为实际 T 编号（AC1=T1/T2/T3/T4/T16/T17/T18/T22、AC2=T5-T8/T19/T20、AC3=T9/T10/T11/T15、AC4=T12、AC5=T13/T17/T19/T21、AC6=T11/T14）。
- 对齐后核对：AC 引用集合 = prd 实际集合 = {AC1..AC6}，无虚构编号；全部 AC 有 T 点落点，无缺锚（T15 为整文件校验的 AC1/AC2/AC3 多锚、T17/T19 双锚——集合层一致）。
- 相位转移：两工件提交（@ d9dbb41）→ phase 置 gate1（express 评审集合 = prd + test-strategy 两件），提示 `/aidlc:gate`。

---

2026-10-15 Gate#1 人类审批（AskUserQuestion，逐字转写）：
- 互审输入：`reviews/gate1-ai.md` @ c8fb644（reviewer-pm 0/4/5 + architect 0/4/8 + qa 1/3/8 + dev 0/3/7；合并 1 BLOCKER / 8 MAJOR / 15 MINOR）。
- 呈批四问与用户裁决：
  问：「Gate#1 互审：1 BLOCKER（B1 artifact 名零锚——错名将静默产出缺安装包的正式 Release）+8 MAJOR（守护口径冲突/前置硬校验无锚/升级披露不实/双 SOP/路径悬空/WiX v3 EOL 等）+15 MINOR。如何裁决？」
  答：「打回修订（推荐）」
  问：「开放问题 Q1-Q4（互审建议 gate1 显式拍板而非默认通过；Q2 已计入 WiX v3 EOL 权衡）：」
  答：「pkg+直接WiX+PATH（推荐）」
  问：「互审新钉死项（M3/M4/M7 落点，修订轮回填字面）：」
  答：「全部采推荐（推荐）」
  问：「T16 定级与 SKIP 判据（qa M2）：」
  答：「升 P0+SKIP留痕（推荐）」
- 裁决落点（编排者执行）：
  1. gate1 打回（gate1=rejected，phase 回 inception）：只重跑 pm+qa 修订被点名项，对齐拍后 gate1 复位 pending 复审（重点核对 B1/M1/M2/M7 消解）。
  2. **Q1-Q4 裁决**：Q1 = mac 形态 `.pkg`；Q2 = **手写 `.wxs` + CI 直接 WiX**（计入 WiX v3 EOL 与双 bin 必手改 .wxs 的权衡——cargo-wix 不采用，仅 CI 侧装 WiX 工具链，安装方式与版本锁定 Construction 落）；Q3 = MSI 写 Machine 级 PATH；Q4 = 安装位置 `Program Files\CloudMount`（Windows）/ `/usr/local/bin`（mac）。
  3. **钉死项裁决（M3/M4/M7 落点）**：安装器定义 = `packaging/wix/main.wxs`（仓库集中惯例）；mac 打包脚本 = `scripts/package-pkg-macos.sh`（沿 package-macos.sh 先例）；新断言脚本名统一 = `scripts/installers-yaml.test.sh`（复数）；安装器版本取值 = `github.ref_name`（不依赖 Cargo.toml bump）；SOP 载体 = operations 阶段产出本 intent runbook 显式声明取代 macos-release runbook 的发布流程节。
  4. **T16 裁决**：升 P0；SKIP 需显式人工裁决记 decisions.md（含安装尝试与失败输出留痕）。
  5. 修订范围点名——pm：AC3 可测性拆解改口径（新脚本承接既有产物名锚点，与锁定脚本重复无害）/非目标升级措辞改「不设计不保证，Release notes 提示先卸载再装」+版本取值 github.ref_name 入语义对齐/SOP 载体声明/AC6 脚本名统一复数/AC4 行数口径（MSI 1 行+pkg 每 arch 一行共 3 行）+PATH 刷新提示+UAC 预期+rc tag 语义+pkg 无 install 脚本声明+AC3③ 追加口径（签名段追加新行不改既有段）/开放问题 Q1-Q4 转已裁决；qa：B1（T2/T6 补 artifact name 断言）/M1（T11 措辞消歧）/M2（T6 upload path 钉双字面精确路径+前置硬校验断言）/M6（T20 降级判据 xar! magic+体积带+真机补验触发）/M7（断言对象路径钉死+脚本名统一）/T16 升 P0+SKIP 留痕/T8 补 launchd 负断言/T12 双 arch 字面+AC4 负路径/T21 映射补 AC4/T9 补无 if:/status/T10-T11 矛盾消解/T18 判据操作化（OS 新开终端+管理员 shell+command -v）/头部与缺口节同步。
  6. GATE-REJECTED 标记在后；修订轮完成后 gate1 复位 pending 复审。

GATE-REJECTED gate1 2026-10-15

---

2026-10-15 Gate#1 修订轮记录（编排者）：
- 修订范围（只重跑被点名角色 pm+qa；其余工件零改动；修订提交 @ 68a58d2）：
- **prd.md**：M1（AC3 拆解改「新脚本承接既有产物名锚点——双保险」）/M3（非目标升级措辞改「不设计不保证，Release notes 提示先卸载再装」+语义对齐补版本取值 github.ref_name）/M4（AC5+语义对齐补 SOP 载体声明——operations 新 runbook 取代 macos-release 发布流程节）/M7（AC6 脚本名钉死 installers-yaml.test.sh 复数+语义对齐钉死 packaging/wix/main.wxs 与 scripts/package-pkg-macos.sh）/M8-Q2（开放问题四项转已裁决+Windows 工具链段删 cargo-wix 表述、保留 WiX 安装锁定 Construction 落）/MINOR（AC4 行数 3 行+PATH 新开终端+UAC+先卸载再装提示、AC5 rc tag 语义、AC2 pkg 无 install 脚本、AC3③ 追加口径、AC1 仅产出负断言口径）；连带一致性三处（AC1 拆解/非目标 .dmg 行/首日验证条目随 Q2 裁决同步）。
- **test-strategy.md**：B1（T2/T6 补 upload artifact name 以 cloudmount- 开头断言——镜像上 intent 先例）/M2（T6 upload path 钉双字面精确路径+T2/T6 补前置硬校验断言）/M1（T11 改双保险口径+允许被修改面包含式护栏——消 T10/T11/T13 自相矛盾）/M3（T3 补版本取 github.ref_name 断言）/M5（T16 升 P0+SKIP=显式人工裁决留痕；随 Q2 裁决改 WiX 冒烟）/M6（T20 降级判据 xar! magic+体积带+真机补验触发入 SOP）/M7（T3/T4/T8 断言对象钉死=release.yml job 块+packaging/wix/main.wxs / scripts/package-pkg-macos.sh）/MINOR（T8 launchd 负断言、T12 行数口径+双平台签名覆盖负路径+误导措辞负断言、T21 映射补 AC4、T9 无 if:/status+continue-on-error 白名单收窄、T18 判据操作化）；头部/缺口节同步（Q1-Q4 已裁决、钉死项写死、SOP 载体）；计数 **P0=15/P1=6/P2=1**（T16 升 P0）。
- 修订轮对齐拍：AC 引用集合 = {AC1..AC6} 无幻影；prd 锚位与映射列双向核对通过（T21 映射列补 AC4 为 qa 侧超集——集合层一致）；钉死字面（installers-yaml.test.sh、packaging/wix/main.wxs、scripts/package-pkg-macos.sh、github.ref_name）双文件在案。
- 相位转移：修订轮两工件提交（@ 68a58d2）→ gate1 复位 pending（重审以新一轮为准；GATE-REJECTED gate1 2026-10-15 之后须新回执），phase → gate1。复审重点核对 B1/M1/M2/M7 消解。
