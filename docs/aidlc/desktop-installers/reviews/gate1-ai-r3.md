# Gate#1 AI 互审 r3（2026-10-16，只核收敛点）

> 评审对象：第二轮修订两工件（prd.md + test-strategy.md @ d1294e3；intent.md 补齐 @ 8eb5181）对照上轮发现（reviews/gate1-ai-r2.md）与第二次打回裁决（decisions.md GATE-REJECTED gate1 第二次）。
> 评审者：reviewer-pm（0/0/1）、reviewer-architect（0/0/5）、reviewer-qa（0/0/3）、reviewer-dev（0/0/1）——全部只读。
> 编排者去重合并：**0 BLOCKER / 0 MAJOR / 6 MINOR**。

## 上轮 4 MAJOR 消解核对（收敛点——四方全部确认消解）

| 上轮 | 判定 | 证据 |
|---|---|---|
| M-R2-1（B2 口径扫荡残留+AC6 锚失实） | ✅ 消解 | L13 改四 job 精确枚举（与 release.yml 实际 job ID 逐一吻合）；TP-01/TP-02/L56 拓扑措辞改「追加步+新增 mac job」；**AC6 锚双向真一致**（prd TP-10/TP-03 ↔ TP-03 映射列 AC3/AC6 ↔ 覆盖核对同步）——上轮「完成声称失实」本次为真（decisions r2 记录与工件相符）；grep「五 job」「4 行」零残留 |
| M-R2-2（AC5⑤「4 行口径」不可判定） | ✅ 消解 | 四处「4 行」全改 **8 行口径**（7 既有+MSI 行更新+.dmg 行）——brownfield 实测 7 数据行算术自洽；TP-01 补 body 断言锚（可判定落绿） |
| M-R2-3（body 编辑授权相抵） | ✅ 消解 | AC5③ 扩「body 行追加及 ⑤ 所指 MSI 行描述更新为授权改动（就地改写仅限 MSI 行描述）」+TP-03「body 按 AC5⑤ 为断言对象而非护栏对象」——授权/守护/断言三面无相抵 |
| M-R2-4（RGBA 延留+改动面） | ✅ 消解 | prd AC1①/语义对齐/TP-11/缺口节/intent.md 五处落字 **icons.rs**（gen_icons 增发 Rust RGBA 常量源，零 crate 依赖——dev 消费端语义自洽复核：stdlib 像素行现成 RGBA 可导出，`Icon::from_rgba`/`IconData` 均标准 API，include 先例在 main.rs L746）；intent.md L13 补齐 Cargo.toml+build.rs+icons/+T4 修订授权（三方一致） |

上轮 12 MINOR 全部落位（四锁定脚本精确文件名/TP-04 边界化/TP-09 区段+轮询/TP-21 连锁锚/TP-03 精确清单/TP-11 豁免+尺寸口径/AC5④ 删噪声/AC5① 新增 job/次序约束/降级连锁锚/main.wxs 头注注记/环形依赖豁免）；映射与计数（AC1..AC6 双向、P0=16/P1=6/P2=2、TP-01..24 连续）全部一致；范围纪律 PASS（第二轮修订逐项对照点名清单无超范围）。

## MINOR（6，去重）

1. **AC5⑤/语义对齐括注「linux×2」枚举失实+「其余既有 7 行」off-by-one**［pm/arch/qa/dev 四方同证］：release.yml L213 linux 实为**单行 glob**（覆盖双 target）——括注按行数求和=8≠7（行数与 target 数混用）；AC5③「其余既有 7 行」应为 6（MSI 在 7 行内）。总量「7/8 行」与 TP-01 锚正确可判定，不阻断——一词修复（linux 标注「1 行 glob 覆盖双 target」+「其余 6 行」）。
2. **TP-01「Gatekeeper 引导文案存在」子锚未绑定 .dmg 行**［qa］：现行 body 尾注已含 3 处 Gatekeeper 字面（mac tar/pkg 引导）——朴素 grep 即使 .dmg 行不含引导也通过（假绿）。修复：子锚绑定 .dmg 行（对齐 prd AC5⑤ 原绑定）。
3. **TP-02 追加步 `if:` 断言面窄于策略层**［qa］：追加步的 step 级绕过 `if:` 不在 TP-02 字面断言面（L56 分层策略已覆盖）——一行措辞闭合。
4. **intent.md L22 约束与 L13 改动面 T4 例外注记缺失**［arch］：L22「锁定脚本零触碰」与 L13「installers-yaml.test.sh T4 修订授权」同文件并置无例外注记——字面守卫按 L22 会误判 L13 授权项越界。一行注记闭合（例外已在 decisions/prd/test-strategy 三处精确落字）。
5. **test-strategy L13「release.yml desktop job」单数措辞残留**［arch/dev］：前提节摘要行未同步「desktop 面（追加步+新增 job）」——一词级。
6. **icon.ico CI 侧来源未显式落字**［arch——Construction 可闭合］：Windows job 的 winres/candle 需 icons/icon.ico 在 checkout 中——intent L13 列 icons/ 暗示入仓；TP-12 fail-fast+WiX 缺失 File 天然报错兜底。

## 遗留观察（不计数）

winres 供应链披露句缺（授权已闭环）；元数据 owner 不一致/MSI job 构建来源/UTF-8 description 风险——沿 r2 遗留建议随 Construction 首日一并落。

## 结论

**0 BLOCKER / 0 MAJOR / 6 MINOR。** 上轮 4 MAJOR 全部消解且证据闭环（含 AC6 锚失实修复、icons.rs 消费端语义自洽、8 行口径算术自洽、授权三面闭合）；三裁决（RGBA/改动面/body）逐字回填；语义主干（托盘恰两项/单一图标管线含 icons.rs/dmg 拖拽/三件套铺放/门语义）三方自洽；可实现性终验通过（dev：按修订后方案可直接开工，无未决依赖）。6 MINOR 均为一词/一句/子锚绑定级，无一阻塞 Construction 进入。

**四方一致建议：通过 gate1**——6 MINOR 授权随 Construction 首日断言落笔顺带闭合（沿 gate1 r3 先例）；如人类要求零残留，词级 touch-up 亦可（不必再走完整修订轮）。
