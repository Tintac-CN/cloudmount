# Ops Runbook: desktop-installers

> 产出者：编排者　日期：2026-10-16　前置：gate2 approved（含准出回填闭环）
>
> 人类裁决（AskUserQuestion 2026-10-16，逐字留痕 decisions.md）：**发布前置 = checklist 单载体**；**监控 = 轻量 + 四观察**；**retro = 保留**；**部署窗口 = 无窗口约束**。
> **取代声明**：本 runbook 的发布流程节取代 `docs/aidlc/installer-packages/ops-runbook.md` 的同名节（产物表 7→8 行、守护脚本四→五条、发布前置改 checklist 单载体）；其 WiX/macos-15 两观察项并入本文件沿用。

## 系统画像

应用层+安装包 intent：交付物 = ①应用层图标管线（`scripts/gen_icons.py` → `apps/desktop/icons/{icon.ico,32x32.png,128x128.png,icon.icns,icons.rs}`，winresource 嵌 exe/MSI Icon/.app icns/tray+egui 四消费端）；②托盘接线+伪隐藏关闭语义（`apps/desktop/src/main.rs`，恰两项菜单/CancelClose+离屏+WS_EX_TOOLWINDOW）；③MSI 三件套（`packaging/wix/main.wxs` desktop 组件+两处快捷方式+Icon/ARPPRODUCTICON）；④mac .app+.dmg（`scripts/package-dmg-macos.sh`+`installer-dmg-macos` job，aarch64-only）；⑤release.yml desktop 面（追加步+新 job+body 8 行）+ `scripts/desktop-installers-yaml.test.sh`（112 断言守护）。监控对象 = CI/发布链路 + 安装包面 + 应用层交付形态；无运行时服务面。

## 发布流程（裁决：checklist 单载体 + 无窗口约束 + tag 即发布直接正式）

**打 tag 前唯一关口**：按 `docs/aidlc/desktop-installers/release-checklist.md`（TP-22 载体）逐项执行并勾选——五项：①本地 MSI 冒烟绿（命令链在清单内，管理员 Git Bash）②断言脚本全绿含零删改护栏（五脚本循环命令在清单内）③dmg 结构断言绿+hdiutil 装配真机/CI 段④托盘+双击 UI 人工冒烟（Windows/mac 各 6 步骤，Q1 判据）⑤产物清单 9 文件与预期一致。**铁律：未逐项通过不打 tag；任一 FAIL 修复后重验，不得带病发布。**

**发布动作**：push `v<纯数值>` tag（rc 形态不在发布语义内，WiX/pkg 脚本拒收）→ release.yml 全 job 自动执行 → 直接创建**正式** Release（无 prerelease，A1 裁决）。

**首个含 desktop 的 tag 特别项**：a) **mac desktop CI 首跑**（desktop 应用首次 mac 编译——观察 aarch64 构建与依赖树新 plat 面；本机交叉 check 曾受阻于 ring 需 darwin C 工具链，实际以 CI 为准）；b) 产物清单 9 文件核对（checklist ⑤ 有精确清单+来源 job 映射）；c) dmg 真机核验（TP-19：挂载/拖装/启动/图标/Gatekeeper 右键打开引导）；d) 托盘真机核验（TP-20/21：恰两项/显示主窗口/关闭隐藏[含双关毒化修复后体验]/退出真退）。

**回退**：Release 页删错误资产/删 Release；workflow 层 `git revert`（112 断言守护下 revert 安全性同既有口径）。

## 监控与观察项（裁决：轻量 + 四观察，零新增基建）

- **硬门**：五脚本全绿（`desktop-installers-yaml.test.sh` 112/112、`installers-yaml.test.sh` 72/72、`release-yaml.test.sh` 23/23、`check-macos-yaml.test.sh` 13/13、`package-macos.test.sh` 9/9——actionlint 可用环境 desktop 脚本为 113）+ `cargo test --workspace` 277/0 + desktop `cargo fmt --all -- --check`/`cargo clippy --all-targets -- -D warnings` clean（gate2 终验口径修正后的完整覆盖）。
- **观察项 1（mac desktop CI 首跑）**：desktop 首次 mac 编译——观察 `installer-dmg-macos` job 首跑的构建时长/内存/choco 或系统工具依赖失败面；失败则错误信息+复现留痕进新 intent。
- **观察项 2（伪隐藏/托盘真机核验）**：Windows 侧关闭语义已自动实证+M1 守卫回填；完整体验（真实托盘点击/mac 侧伪隐藏形态——Dock 图标留存+Mission Control 可见性）随真机核验收敛（release-checklist §4 承接）；mac 形态若体验不佳（Dock 残留），LSUIElement 方案为新 intent 候选。
- **观察项 3（WiX 硬锁版本，沿上 intent）**：版本已硬锁 3.14.1.20250415——观察 chocolatey 源对该版本的废弃公告；choco 强制 `--version` 安装失败时 job 显式红（非静默），届时随新 intent 升 WiX 大版本。
- **观察项 4（macos-15 runner 退役，沿上 intent）**：GitHub runner-images 退役/迁移公告时，`runs-on: macos-15` 字面与断言（两层：installers-yaml TP 断言+desktop-yaml TP-01 dmg job 断言）需同步（走新 intent）。

## P2 backlog（gate2 备案 11 项 + 上 intent 沿用项，下个 intent 候选）

- **本期备案**：wxs Package Description/Comments 仍两件套描述（ARP 元数据欠准，修复需新授权——AC3④ 既有字面守护）；HKCU KeyPath 多用户卸载孤儿（ICE43 经典代价，决策留痕）；dmg 脚本不校验二进制架构（直跑场景无 file/lipo 兜底）；TP-23 无显式承接载体（行为层由 TP-08/等价冒烟兜底，建议下个 intent 补埋点）；T4 修订断言①防空真依赖步骤名字面（结构脆弱性）；desktop 断言计数环境相关口径（112/113——actionlint 可用性）注记进 README 或脚本头注；gen_icons 幂等跨 zlib 版本边界（注记建议）；TP-12 build.rs 'winres' 子串宽松（同时命中 winresource，现态无害）；FFI GetWindowLongPtrW 32 位约束注释留痕；units/2.md 契约字面补伪隐藏偏差指针；.gitignore L13-14 src-tauri 死路径残留清理。
- **沿用（installer-packages backlog 未清项）**：ci.yml 补 actionlint 步骤；Cargo.toml repository 元数据修正（zhonghaoxi→tintac-cn）；decisions 计数口径勘误注记随对账。

## 回流机制（Operations 协议 §2）

生产事件/学习只写**新 intent 的 intent.md**（`/aidlc:inception` 进入时先查本 runbook 的 backlog 与观察项）；当前 intent 的学习沉淀在 `retro.md`，已完结工件不再修改。
