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
