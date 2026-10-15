# Gate#1 AI 互审（2026-10-16）

> 评审对象：express 两件（prd.md + test-strategy.md @ 70ebbf5）对照 intent/decisions（A1 裁决）与 brownfield（release.yml 八 job/四锁定脚本/apps/desktop 现状）。
> 评审者：reviewer-pm（1/2/8）、reviewer-architect（2/4/6）、reviewer-qa（0/4/4）、reviewer-dev（2/4/6）——全部只读；qa 对三锁定脚本本机实跑全绿（23/23、13/13、9/9）。
> 编排者去重合并：**2 BLOCKER / 8 MAJOR / 15 MINOR**。

## BLOCKER（2，同根：「同一 MSI 三件套」与「既有 job/锁定脚本零触碰」约束未消歧）

- **B1｜prd AC3 与锁定断言 installers-yaml.test.sh T4 硬冲突**［dev B1+arch B1-part+qa 实证］：T4 两条负断言（L259-260 MSI_BLK 无 desktop 字面 / L263-264 WXS 无 desktop 字面）在三件套落地后**确定性转红**（desktop 组件/构建步必然引入该字面）；「锁定脚本零触碰（AC6②）」+「断言全绿（TP-22）」+「AC3 三件套」三者不可同真。仓库有正确先例（T16 授权锁修订），但本 intent decisions.md 无该授权记录。修复：gate1 授权锁修订（T4 两条负断言字面收窄/豁免+decisions 留痕+重锁）。
- **B2｜prd AC5④ 与 AC3 物理矛盾：installer-msi-windows 字面零触碰 ⇒ MSI 装不进 desktop exe**［dev B2+arch B1-part+pm B1-part 同证］：根 Cargo.toml `exclude = ["apps/desktop"]`，根级构建不可能产出 cloudmount-desktop.exe 进 target/release（candle -dBinDir 指向处）；三件套必然要求 installer-msi-windows 增构建/归集步——任一路径都改该 job 字面，直接违反 AC5④/TP-03。叠加 job 拓扑欠定（MSI job 内追加 vs 新 job 产 artifact+下载）未择一——gate1 级决策。修复：AC5④ 口径重述（零触碰限定 daemon/cli 构建命令/铺放/上传语义，允许追加 desktop 步）或改拓扑（新 desktop job 供 artifact）。

## MAJOR（8）

1. **TP-11 icns 本地生成断言 vs prd「icns mac 独占/生成链未决」P0 矛盾**［dev M1+qa］：TP-11（P0 本地单元）断言「.icns 生成」，prd L20/L46 却说 icns 本地 Windows 不可产、生成链二选一未定——按字面必红或迫使实施者私扩链路。修复：二选一对齐（stdlib 手写 icns 需首日实测接受度 / CI mac 生成则 TP-11 改 CI 产物断言）。
2. **PNG→RGBA 解码链缺环，隐藏依赖未声明**［dev M2］：tray-icon 0.21.3 的 png 依赖仅 linux/macos target——Windows 托盘 `Icon::from_rgba` 需原始 RGBA；egui `ViewportBuilder::with_icon(IconData)` 同。落地需 apps/desktop 显式声明 image/png crate 或 gen_icons.py 增发 Rust RGBA 常量源文件——两者都超出 prd「{icon.ico,*.png,*.icns}」产物面，须写明。
3. **intent 改动面漏 apps/desktop/Cargo.toml 与 build.rs**［dev M3］：AC1 winres 方案必然新增 [build-dependencies]+build.rs——跨文档不一致会让改动面守卫误判越界。
4. **.dmg artifact 命名须匹配 `cloudmount-*` 收编 glob 的约束未声明未断言**［dev M4］：命名脱靶=CI 全绿但 dmg 静默不上 Release（上 intent B1 同型失效模式）；AC5②/TP-01 均未写该约束与 name 前缀断言。
5. **Release body 产物表/签名引导更新无 AC/TP 落点且与 TP-03 锁定读法相抵**［pm M2+arch M1+qa MINOR-7 同根］：MSI 行描述三件套后失实、无 .dmg 行、.app Gatekeeper 引导缺失；TP-03 字面读法又禁止此必要编辑——两头须在 prd 显式落字。
6. **「P0 全绿方可进 Construction」与 TDD 红绿流程自相矛盾**［qa MAJOR-2］：16 项 P0 中 14 项断言 Construction 交付物（进门前必然全红）；须改「P0 全部落位且可运行（红态）方可进 Construction，全绿为准出门」。
7. **TP-16 图标断言空真（本机实证）+TP-09「恰两项」无可判定断言形态**［qa MAJOR-3+4］：ExtractAssociatedIcon 对无图标 exe 返回默认图标非空（当前 exe 实测 32×32 非空=今天就能绿）；「菜单恰两项」需钉可判定式（托盘 init 区段 MenuItem 计数=2+MenuEvent 匹配字面+全文件无其他 tray Menu 构造）。
8. **首日验证无降级路径声明**［arch M4］：验证④（关闭语义 API 不可行时「隐藏到托盘」无法落码的回退语义=真退出+托盘保持「显示/退出」）与验证①（tray 集成不可行的升级决策路径）均无一句话交代——AC2④/TP-21 将失锚。

## MINOR（15）

1. T3 规格/units/1.md token 未随授权修订同步 `ProgramFiles64Folder`［qa MINOR-1+dev MINOR-3］。
2. CI WiX 版本软锁（choco 未 pin，3.14.1 仅注释；v3 EOL 冻结漂移风险趋零，补 flag 即硬锁）［qa+dev］。
3. state.json U2 条目「MSI 侧 27 条」为时点口径，与 U3 定稿分解（45+27）不一致［qa］。
4. T15 注入自校验正分支从未实跑（本机 actionlint 不可用；建议可用环境补跑全绿留痕）［qa］。
5. 人工核验 SOP 缺「第 0 步 dispatch check」项（operations runbook 固化时补）［qa］。
6. 「72/72」隐含 actionlint 不可用降级前提未记录（可用时总数 73，重放会漂移）［dev］。
7. 多实例已知行为未声明（托盘常驻+双快捷方式放大误双开概率——双 SyncEngine 同 sync_root 并发）［arch M3+dev m4］。
8. 托盘初始化失败路径未设计（panic 退出 vs 降级无托盘运行）［arch m2］。
9. 图标产物 CI 侧来源未钉（入仓 vs CI job 内跑 gen_icons）［arch m3］。
10. winres 维护状态/选型理由未披露（继任者 winresource；Construction 复核）［dev m3］。
11. TP-03 措辞「daemon/cli/macos-release 相关 job」与实际 job 名（binaries-macos/installer-pkg-macos）映射需对齐［arch m5］。
12. TP-11「.ico 多尺寸」与 AC1① 基线（32×32 单帧）口径不一致［pm m1+arch m6］。
13. mac desktop 单架构（arm64-only）覆盖决策未落字（Intel mac 无 desktop 为用户可见决策）［arch m1］。
14. .dmg 架构未钉（aarch64-only/双 arch/universal）［dev m6］。
15. release job body 锁粒度未定（AC5③ 只授权 needs 追加；body 产物表/说明追加是否允许未明）［dev m5］。

## 正向确认

「托盘恰两项」封顶三方自洽；图标单一来源管线与 gen_icons 像素设计/基线完全对齐；dmg 语义与 pkg 零触碰兼容；main.wxs 扩展与既有面共存性逐项对应（AC3④ 保护清单实证）；fail-fast 闭环自洽；故事→AC→TP 映射完备无缺锚；非目标 8 项与约束一致；计数 P0=16/P1=6/P2=2 实数一致。

## 结论

**2 BLOCKER / 8 MAJOR / 15 MINOR。** 正向面扎实（映射完备/缺口诚实/护栏有效/范围纪律），但两 BLOCKER 同根且硬：「同一 MSI 三件套」（A1 裁决）与「既有 job/锁定脚本零触碰」（约束继承自 intent 行文）未消歧——B1 需授权锁修订（沿 T16 先例），B2 需 job 拓扑裁决。均为 gate1 可一次裁决+修订一轮闭合的口径/结构问题，无技术不可行项（dev 实证 tray 集成/关闭拦截/winres/.wxs/dmg 全部可实现，eframe 关闭拦截 API 形态已核）。

**建议：打回修订（phase 回 inception）**，gate1 一并裁决四项：①job 拓扑（推荐：installer-msi-windows 从零触碰集合移出，精确口径=「既有构建/铺放/上传行不删改+允许追加 desktop 构建与归集步」，同步修 AC5④/TP-03/AC3④/AC6②）②锁修订授权（T4 两条负断言字面收窄，沿 T16 授权锁修订先例 decisions 留痕+重锁）③Q1 关闭语义（推荐隐藏到托盘；eframe CancelClose 落码形态入首日验证④）④Q2 快捷方式（推荐开始菜单+桌面两处）。MINOR 清单随修订轮一并处理（多实例声明/失败路径/CI 来源/版本口径等）。
