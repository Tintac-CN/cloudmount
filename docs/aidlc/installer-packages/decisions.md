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
