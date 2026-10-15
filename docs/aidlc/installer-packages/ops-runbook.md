# Ops Runbook: installer-packages

> 产出者：编排者　日期：2026-10-15　前置：gate2 approved（含准出回填）
>
> 人类裁决（AskUserQuestion 2026-10-15，逐字留痕 decisions.md）：**发布前置 = 三重前置固定**；**监控 = 轻量 + 三项观察**；**retro = 保留**。
> **取代声明（gate1 钉死载体）**：本 runbook 取代 `docs/aidlc/macos-release/ops-runbook.md` 的发布流程节（其「首 tag prerelease 预演」裁决已被 A1「直接正式发布」覆盖；该文件其余内容仍有效）。

## 系统画像

CI/发布基建 intent：交付物 = `release.yml` 两个安装包 job（`installer-msi-windows` / `installer-pkg-macos`）+ `packaging/wix/main.wxs`（WiX v3 手写定义，版本硬锁 3.14.1.20250415）+ `scripts/package-pkg-macos.sh`（mac pkg 打包，dry-run 段本地可跑）+ `scripts/installers-yaml.test.sh`（72 断言守护）。监控对象 = CI/发布链路 + 安装包面；无运行时面。

## 发布流程（裁决：三重前置固定 + tag 即发布直接正式）

**固定前置（三者齐备才打 tag）**：
1. **dispatch check**：手动触发 `check-macos.yml`（workflow_dispatch）确认编译面绿；
2. **本地预检**：四脚本全绿（`package-macos.test.sh` 9/9、`release-yaml.test.sh` 23/23、`check-macos-yaml.test.sh` 13/13、`installers-yaml.test.sh` 72/72）+ `bash scripts/package-pkg-macos.sh --bin-dir <占位> --out <tmp> --dry-run` exit 0；
3. **CI 双腿绿**：push 后 ci.yml 三平台双腿绿（Rust 面）。

**发布动作**：push `v*` tag → release.yml 全 job 自动执行 → 直接创建**正式** Release（无 prerelease/预演步骤，A1 裁决）→ 按 test-strategy 人工核验清单（含第 0 步语义已由前置 1 承载）核验 T17-T21：MSI/pkg 资产可下载、T18 MSI 静默装/卸（本机 Windows，管理员 shell）、T20 pkg 真机安装（需真 mac 机；无则降级判据 `xar!` magic+体积带，记缺口）、产物表 4 行（tar/zip/MSI/pkg）、既有资产零删改。

**回退**：Release 页删错误资产/删 Release；workflow 层 `git revert`（既有 job 零删改断言守护 revert 安全性）。

## 监控与观察项（裁决：轻量 + 三项观察，零新增基建）

- **硬门**：CI 三平台双腿绿 + `check-macos.yml` dispatch 绿 + 四脚本断言绿（9/23/13/72）。
- **观察项 1（WiX 版本）**：版本已硬锁 3.14.1.20250415——观察 chocolatey 源对该版本的废弃公告；choco 强制 `--version` 安装失败时 job 显式红（非静默），届时随新 intent 升 WiX 大版本（v4/v5 需 dotnet SDK，.wxs 语法随之重写——prd 首日验证已留痕）。
- **观察项 2（macos-15 runner 可用性）**：GitHub runner-images 退役/迁移公告时，`runs-on: macos-15` 字面与 T9 断言需同步（走新 intent）。
- **观察项 3（安装包真机核验触发）**：首个含安装包的 tag 发布后执行 T17-T21 人工核验清单（MSI 本机静默装/卸、pkg 真机、产物表、零删改）；pkg 无真机时降级判据 `xar!` magic+体积带并记缺口，借到真机时补验 T20 全项。

## P2 backlog（下个 intent 候选）

- ci.yml 补 actionlint 步骤（消 T15 降级缺口，actionlint 本机不可用）；
- state.json U2 条目计数口径勘误注记（「MSI 侧 27 条」为 U2 时点值，定稿分解 45+27）；
- decisions「移除 ICE80」措辞勘误注记（ICE80 从未入提交，仅存在于当时工作树实验）；
- T15 注入自校验正分支在 actionlint 可用环境补跑留痕；
- .wxs 无 UIRef 知会（双击安装走 msiexec 缺省 basic UI，T22 无向导可验）；
- release 下载步注释「只取 binaries」语义漂移修正；
- 元数据 owner 不一致（Cargo.toml repository=zhonghaoxi vs 实际 owner tintac-cn）——已通过 MINOR⑫ 隔离（安装器统一 tintac-cn），Cargo.toml 修正随下个 intent。

## 回流机制（Operations 协议 §2）

- 发布事件/CI 异常/新需求 → 经 `/aidlc:inception` 写**新 intent**（不改本 intent 已完结工件）；过程学习沉淀 `retro.md`。
