# Plan: desktop-installers

> 产出者：编排者（Sprint Planning）　日期：2026-10-16　前置：gate1 approved（@ c598cb4，含两轮打回修订）
>
> 依据：prd v3（AC1–AC6，两轮修订+首日 6 MINOR 回填后）、test-strategy v3（24 点 P0=16/P1=6/P2=2，进出门语义：红态准入/全绿准出）、gate1 全部裁决（B2 job 拓扑/B1 授权锁修订/Q1 隐藏托盘/Q2 两处快捷方式/RGBA=icons.rs/body 8 行口径/aarch64-only）。
> TDD 铁律：每 unit 先提交失败测试（红）→ `/aidlc:lock-tests` 置锁 → 实现转绿。上 intent 四锁定脚本零触碰（唯一例外=T4 两条负断言授权锁修订，Construction 期执行）。

## Units

| # | Unit | 目标 | 涉及文件 | 影响面 | 测试 | 验收 |
|---|---|---|---|---|---|---|
| U1 | 安装包结构断言脚本（关键路径，红阶段） | `scripts/desktop-installers-yaml.test.sh`：TP 断言全集（release.yml desktop 面[追加步+新增 job]/main.wxs desktop 组件+两处快捷方式+Icon/dmg 脚本字面+Info.plist/托盘源码字面[TP-09 可判定式]/gen_icons 产物+icons.rs/既有 job 零删改护栏+快照基线/body 8 行锚[Gatekeeper 绑定 .dmg 行]/pkg 无 .app 负断言）——沿上 intent 四脚本先例（防空真/CRLF 剥除/actionlint 降级计数诚实/job_block 复制注） | `scripts/desktop-installers-yaml.test.sh`（新增，TDD 红提交） | low（新文件零既有引用） | TP 断言规格 | 红态：被测对象（desktop 面/wxs 组件/dmg 脚本/托盘接线/icons 产物）不存在全红+护栏现状绿；红提交只含测试文件 |
| U2 | 应用层：图标管线+托盘接线（绿阶段） | ①`scripts/gen_icons.py` 重写（品牌双色方块沿既有像素设计；输出 `apps/desktop/icons/{icon.ico,png 组,icons.rs}`——Rust RGBA 常量源；幂等；stale src-tauri 路径清零）②`apps/desktop/Cargo.toml` 增 [build-dependencies] winres+`build.rs`（嵌 icon.ico 进 cloudmount-desktop.exe）③`apps/desktop/src/main.rs` 托盘接线（tray-icon 初始化[macOS 主线程约束]/菜单恰两项「显示主窗口/退出」/MenuEvent 全局 receiver 轮询/关闭按钮=CancelClose 隐藏到托盘/托盘失败降级不 panic） | `scripts/gen_icons.py`（重写）、`apps/desktop/Cargo.toml`+`build.rs`（新增）、`apps/desktop/src/main.rs`、`apps/desktop/icons/`（产物入仓） | med（apps/desktop 独立 cargo 项目——Rust 编译面+新增 build-dep 供应链[winres 选型 Construction 首日验证②]） | TP-09（托盘字面）/TP-11（gen_icons 产物+icons.rs）/TP-12（fail-fast） | U1 断言脚本 TP-09/11/12 相关断言转绿；`cd apps/desktop && cargo build --release` 过+exe 含图标资源（TP-16 RT_GROUP_ICON 枚举）；本地人工冒烟（托盘/关闭语义） |
| U3 | Windows MSI 扩展（绿阶段） | `packaging/wix/main.wxs` 增 desktop 组件（铺同目录+开始菜单/桌面两处 Shortcut+Icon 元素+头注更新）+ `release.yml` installer-msi-windows **追加步**（desktop 构建 `--manifest-path apps/desktop/Cargo.toml`+归集至 BinDir，次序在校验/candle 之前）+ `installers-yaml.test.sh` T4 两条负断言授权锁修订（解锁→字面收窄→重锁，decisions 留痕）+ 本地 WiX 冒烟（T16 遗产链路：candle+light 构建→msiexec 静默装→三件套+快捷方式+PATH→卸载干净） | `packaging/wix/main.wxs`、`.github/workflows/release.yml`（追加步）、`scripts/installers-yaml.test.sh`（T4 授权修订） | med（既有文件——追加语义+T4 授权锁修订） | TP-03/04/05/13/14/15/17/18/23 | U1 断言脚本 MSI 侧转绿（pkg/dmg 侧仍红为预期）；本地 WiX 冒烟全过（三件套/两处快捷方式/图标/PATH/卸载） |
| U4 | mac .app+.dmg+release body（绿阶段） | `scripts/package-dmg-macos.sh`（staging .app bundle[Info.plist+icns+binary]+hdiutil 打 dmg+aarch64-only）+ `release.yml` 新增 macos-15 job（desktop 构建+.app 打包+dmg upload 精确路径+`cloudmount-` 前缀 artifact）+ release needs 追加+body 8 行口径更新（MSI 行三件套描述+新增 .dmg 行含 Gatekeeper 引导） | `scripts/package-dmg-macos.sh`（新增）、`.github/workflows/release.yml`（新增 job+body） | med（release.yml 新增 job+body 更新[授权改动]） | TP-01/02/07/08/19/24 | U1 断言脚本**全绿**；四锁定脚本回归仍绿（T4 修订后基线）；本地 dry-run 冒烟（dmg 脚本 staging 段） |
| U5 | 终验收尾 | 终验组合全绿+基线比对零回退+人工冒烟清单固化（TP-22 载体=docs/aidlc/desktop-installers/ 附录文件）+首日验证清单结论留痕 | 无新文件（核验+留痕+附录清单） | low | 终验组合 | `cargo test --workspace` 与基线逐数一致+`cd apps/desktop && cargo build --release` 过+fmt/clippy 净+五脚本测试全绿（9/23/13/72/N）→ phase 置 gate2 |

## 验证命令（F2 规范来源）

- **基线/回归命令**：`cargo test --workspace`（第 0 步记录 `state.testBaseline`；本 intent workspace 面零 Rust 改动——apps/desktop 为独立项目[workspace exclude]，预期零回退）
- **终验组合**：上命令 + `cd apps/desktop && cargo build --release`（desktop 独立项目构建面）+ `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`（四件套惯例）+ 五脚本测试（package-macos/release-yaml/check-macos-yaml/installers-yaml/desktop-installers-yaml）+ `actionlint`（可用时）

## 执行顺序与并行泳道

- 泳道 1（断言关键路径）：U1（红）→ 置锁
- 泳道 2（实现串行）：U2（应用层）→ U3（MSI 扩展）→ U4（mac dmg+body）——共用 release.yml 与断言绿态判定（逐断言 PASS/FAIL 可观测部分绿），串行避免冲突
- 泳道 3（收尾）：U5
- 波次：①U1 红+锁 → ②U2 → ③U3 → ④U4 → ⑤U5 终验 → gate2
- 影响面说明：U2/U3/U4 各含一个 med 面（apps/desktop 编译面/release.yml 追加语义/新增 job+body）——均无跨层引用方，无 high/unknown 项。串行派发（沿限流教训）。**U3 含 T4 授权锁修订**：解锁→修订→重锁时序由编排者执行并留痕。

## TDD 锁记录

- 待置位：U1 红提交后锁 `scripts/desktop-installers-yaml.test.sh`（与上 intent 四锁定文件并集——本 intent 契约集五文件）。契约错误走 `/aidc:unlock-tests <理由>` 并留痕 decisions.md。

## gate1 授权落位清单（Construction 首日已执行/随 unit 落）

- **已执行（@ 8e21992）**：6 MINOR 回填（①AC5⑤/语义对齐括注 linux 1 行 glob+「其余 6 行」②TP-01 Gatekeeper 子锚绑定 .dmg 行③TP-02 追加步 step 级 if: 断言面④intent.md L22 T4 例外注记⑤test-strategy L13 desktop 面措辞⑥图标产物入仓声明）。
- **随 U3 落**：installers-yaml.test.sh T4 两条负断言授权锁修订（B1 裁决，解锁→字面收窄→重锁）。
- **首日验证清单（4 项，Construction 期逐项验证）**：①tray-icon+eframe 集成形态（macOS 主线程约束）②winres 构建影响（含选型理由）③icns 生成链（stdlib 手写 vs CI mac 生成二选一）④关闭按钮 CancelClose API 形态（不可行→回退=关闭即真退出+托盘两项）。
