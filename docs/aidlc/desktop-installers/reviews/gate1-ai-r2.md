# Gate#1 AI 互审 r2（2026-10-16）

> 评审对象：修订轮两工件（prd.md + test-strategy.md @ aac8eaf；gate1 复位 @ be7e054）对照上轮发现（reviews/gate1-ai.md）与打回裁决（decisions.md 2026-10-16 GATE-REJECTED gate1）。
> 评审者：reviewer-pm（0/4/3）、reviewer-architect（0/4/5）、reviewer-qa（0/2/5）、reviewer-dev（0/3/8）——全部只读。
> 编排者去重合并：**0 BLOCKER / 4 MAJOR / 12 MINOR（去重后；另 3 条遗留观察）**。

## 上轮发现消解核对（B1/B2/M1/M5/M6/M7——全部消解实证）

| 上轮 | 判定 | 证据 |
|---|---|---|
| B1（锁定断言 T4 硬冲突） | ✅ 消解 | 授权链完整且六处口径一致：prd AC3 注/AC6②/非目标唯一例外句 + test-strategy 头部/零触碰集合节/TP-03 B1 注/缺口节 + decisions 落点 3；「Construction 期解锁→修订→重锁（沿 T16 先例）+修订后纳入零删改基线」闭环清晰 |
| B2（AC5④ 物理矛盾+拓扑欠定） | ✅ 消解（主体） | prd AC3/AC5④/语义对齐/TP-03/decisions 五处一致：installer-msi-windows 从零触碰集合移出，精确口径=既有构建/铺放/upload 行不删改+允许追加；物理可行性 dev 本机实证（rust-cache 在位，追加构建无障碍；归集步拷入 BinDir 可行） |
| M5（release body 无落点） | ✅ 消解（主体） | AC5⑤ 入改动面+AC4② Gatekeeper 引导（残留见 MAJOR-3） |
| M6（P0 门语义自相矛盾） | ✅ 消解 | L51 改「P0 全部落位且可运行（红态）方可进 Construction，全绿为准出门」；grep 无残留 |
| M7（TP-16 空真/TP-09 无形态） | ✅ 消解 | TP-16 改 RT_GROUP_ICON 资源枚举/位图采样品牌色（ExtractAssociatedIcon 空真作废注明）；TP-09 钉三件套可判定式 |
| MINOR 多数 | ✅ 落位 | 多实例声明/托盘失败降级/aarch64-only 五处/TP-05/15 负断言/TP-04 32 位负断言/TP-22 仓库内载体/TP-24 Windows 侧/Q1-Q2 逐字一致 |

## MAJOR（4）

1. **M-R2-1｜B2 口径扫荡不彻底：修订前字面残留+锚注双向不一致+完成声称失实**［qa M1+M2、pm F1/F2、arch MAJOR-1 同证］：
   - test-strategy L13「既有**五 job** 零触碰」与同文件 L16（B2 收窄后四 job+msi 移出）及 prd AC5④ 直接矛盾；
   - TP-01（P0）「新增 desktop 构建/打包 job…上传产物含 desktop MSI 与 .dmg」与 prd AC3「desktop exe 不另立 Windows job（追加步内）」矛盾——Windows 侧断言对象不存在；TP-02/L55「desktop job 结构」措辞同源；
   - AC6 锚双向不一致：prd AC6 锚=TP-10/TP-03，TP-03 映射列仅 AC3、L49 覆盖核对「AC6←TP-10」缺 TP-03——**decisions.md 修订轮记录「双向一致（AC6←TP-10/03）」与工件实际不符**（完成声明失实）。
2. **M-R2-2｜AC5⑤「body 产物表重述为 4 行口径」不可判定且与 brownfield 失实**［qa MAJOR-2+arch MAJOR-2+dev MAJOR-3 三方同证］：release.yml 现网产物表 **7 行**；AC5⑤ 实际操作=MSI 行就地更新（7 中 1）+新增 .dmg 行=**8 行**——「4 行」既非增量亦非全表，断言锚三处（prd L21/L58/L40+decisions L58）确定性无法落绿或迫使违规删行（违反 S4 零回退）。修复：改逐行枚举（既有 7 行零删改+MSI 行更新+追加 .dmg 行）并 TP-01 补 body 锚。
3. **M-R2-3｜body 既有 MSI 行描述「更新（就地改写）」与「行追加」授权相抵**［arch MAJOR-3+pm F4 同根］：decisions B2 精确口径与 AC5③ 授权均为「needs/body 行追加」；AC5⑤ 强制改写既有 MSI 行——修改≠追加；TP-03 快照口径会把含「daemon + CLI」字面的该行纳入快照判红。修复：AC5③ 措辞扩为「body 行追加及 AC5⑤ 所指 MSI 行描述更新」，TP-03 注明 body 产物表按 AC5⑤ 为断言对象而非护栏对象。
4. **M-R2-4｜PNG→RGBA 解码链缺环延留无人认领 + intent 改动面延留**［dev MAJOR-1+MAJOR-2、arch MAJOR-4 同证——**上轮修订指派清单（decisions 落点 6）未含 M2/M3/M4 三项，流程漏项**］：prd 语义对齐仍只有「.png→托盘/窗口图标源」，消费端（tray-icon `Icon::from_rgba`/egui `IconData`）需原始 RGBA——Cargo.toml 无解码 crate，gen_icons 产物面缺 RGBA 源；intent.md L13 改动面仍缺 apps/desktop/Cargo.toml+build.rs。修复：二选一声明（image crate 解码 vs gen_icons 增发 Rust RGBA 常量）+ intent 改动面补齐。

## MINOR（12）

1. test-strategy L13 残留前-B2 计数（与 1 同源但单列：一行修复）。
2. AC5④ 授权噪声：「允许追加…及 needs/body 行追加」——installer-msi-windows 无 needs/body，授予无对象；守卫白名单写法无所适从。
3. AC5①「mac 侧在 macos-15 job」歧义：既有 installer-pkg-macos 同 runner 且属零触碰集合——应写「新增 macos-15 job」。
4. 追加步次序未声明：desktop 构建+归集步须插在校验构建输入/candle 之前，「追加」按尾部读法物理产不出三件套——prd 补序约束。
5. TP-11「.ico 多尺寸」与 AC1① 基线（32×32 单帧）口径不一致——延留未随修订处理。
6. TP-04 负断言子串误匹配陷阱：「ProgramFilesFolder」是「ProgramFiles64Folder」子串（main.wxs 必含 64Folder），朴素 grep_absent 假红——需边界化形式（Id 定界或先滤 64Folder）。
7. TP-09「托盘 init 函数区段」机械界定未钉（按 fn 名或按 tray 初始化调用所在函数）——计数范围漂移可假绿/假红。
8. TP-21 未镜像首日验证④降级分支（prd 降级语义收窄后 TP-21 按字面必红）——注「按首日验证④落定语义核验」。
9. TP-03「等」字残留（AC5④ 已有精确四 job 清单，宜直接引用）。
10. T16 版本锁定软注释与 choco --version 硬锁已就绪——表述统一（已完成硬锁，注释的「如需强锁定可」滞留）——小项并入 M1 修复。
11. MenuEvent 接收形态（全局 receiver 轮询）未入 TP-09 断言面——并入可判定式。
12. 进出门与 TP-11 icns 断言环形依赖——补「TP-11 icns 子断言允许首日定稿」豁免（与 M1 同修）。

## 遗留观察（上轮已记录未点名，不计数）

元数据 owner 不一致（Cargo.toml vs release.yml，安装器已隔离）；MSI job 构建来源未明示；UTF-8 中文 description 风险未入首日验证——沿上 intent 核查清单建议随 Construction 首日一并落。

## 结论

**0 BLOCKER / 4 MAJOR / 12 MINOR。** B1/B2 两 BLOCKER 消解干净（授权链/口径重述/物理可行性实证），修订方向正确；残留 4 MAJOR 均为口径级（B2 扫荡残留+「4 行」常数错锚+授权相抵+RGBA 延留认领），无结构返工；12 MINOR 为枚举/锚定补强。语义主干（托盘恰两项/单一图标管线/dmg 拖拽/三件套铺放/门语义改写）三方自洽。

**建议：打回第二轮轻量修订（phase 回 inception，第 2 轮/上限 3 轮）**——gate1 附带裁决：①RGBA 解码链选型②intent 改动面补齐授权③body 逐行口径采推荐。pm+qa 修订被点名项后，复审 r3 只核收敛点。
