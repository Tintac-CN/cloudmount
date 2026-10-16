# Gate#1 AI 互审（2026-10-17）

> 评审对象：intent `b1-doc-corrections`（route=classic）三件工件——prd @ 3c5dcb0 前版+align / architecture（蓝图 28 项）/ test-strategy（7 TP）。
> 评审方：reviewer-pm（0/3/4）+ reviewer-architect（0/1/6）+ reviewer-qa（0/3/3）+ reviewer-dev（0/1/4）——合并 **0 BLOCKER / 8 MAJOR / 17 MINOR**（去重 6 MAJOR / 10 MINOR）。
> 正面确认（四方交叉实证）：蓝图 28 项行锚抽验 20+ 处**零实质漂移**（dev：全部逐字命中）；23 处清单经独立全仓扫描证实**完备无遗漏**（pm：产品面恰 10 文件；qa：词边界全量复扫无清单外残留）；`datauri` 词边界陷阱处置正确；TP-05 基线计数实跑复证；AC↔TP 双向机械核对通过无虚构；断言面零联动结论复核成立。

## BLOCKER

无。

## MAJOR（去重后 6，全部为工件间口径不一致/计数锚错误）

- **M1｜api.ts 触碰面三方冲突（:2-4 vs :1-6 + D7 骨架句）**[pm-M1+arch-M1+dev-M1]：prd 约束③/AC1 #11/TP-06 钉死「api.ts 仅 :2-4 头注」；蓝图 #15+决策 D7 却要求 :1-6 头注重写并搭车订正 :5「当前为骨架」句（实测 ：5 在 :44-81 八命令全实现背景下确属失实）。照 D7 执行=TP-06 假越界；守 ：2-4=头注自我矛盾。须收敛为「:1-6 头注块」单一口径并补 AC/TP 锚（或明示放弃 D7）。
- **M2｜TP-07 合法残留计数锚硬编码漂移**[qa-M2+pm-m2]：TP-07 钉「docs/ 命中 28 文件=aidlc 26」——实测当日 30/28（b1 本批工件自身即含 tauri 留痕且随 gate 增长）；照字面执行必出假漂移阻塞。应改「docs/aidlc/** 动态清点」口径。
- **M3｜desktop fmt/clippy 门口径三处互相矛盾**[qa-M1]：test-strategy TP-05 显式**豁免** desktop fmt/clippy；architecture 完工门+desktop runbook 硬门却**要求** desktop 单独跑。dev 依蓝图多跑、qa 依策略判豁免——须收敛单一口径（建议=要求 desktop 面单独跑，沿 audit intent 终验修正后的口径）。
- **M4｜TP-01 事实锚判据 vs 蓝图 #9 carve-out 冲突**[qa-M3+pm-m1+dev-m3]：TP-01 要求 23 处新表述均含事实锚（egui/eframe 等），蓝图 #9（ARCHITECTURE.md:48）批准「宿主 App（M5+，技术未定）」零锚措辞（理由成立：M5 未启动不预写 egui）——照 TP-01 必假证伪。须为该项加显式 carve-out。
- **M5｜ROADMAP:164（TLS 收敛）三工件分类不一+AC5(b) 措辞域冲突**[pm-M3+arch-m4]：PRD AC5(b) 钉为必达且「:164 其余字面不动」，蓝图 #25 放「联动候选（gate1 勾选）」且措辞「v1 默认 HTTP」超出「其余字面不动」域。须统一分类（建议=维持 PRD 必达，蓝图 #25 移入 B 段+措辞按 prd 域）。
- **M6｜README:151 替换命令主形态三工件不一**[arch-m5+dev-m4+dev-m2]：蓝图以 `cargo build --release` 为主形态，prd/TP-03 钉 `cargo run`；且 `cargo run` 对 GUI 程序「退出 0」判据实操阻塞（事件循环+无头 wgpu 风险）。须统一主形态（建议=`cargo build --release` 可达为判据，cargo run 仅「可拉起」超时冒烟语义）。

## MINOR（去重后 10）

1. decisions.md 缺对齐拍范围扩裁（16→23）记录条目[pm-m3+dev-m6]
2. prd 非目标未点名 README:143 测试计数 stale（防最小邻接调整下顺手改）[pm-m4]
3. README:135「共享前端 SPA」同根因残留未登记（无 tauri 字面，grep 不兜）[arch-m1]
4. prd AC6b 排除清单缺 `.git/`（TP-02 已补，prd 直跑有假阳性风险）[arch-m2]
5. 蓝图三处证据引用精度（E 节 gzip 行锚 :175→:176 等）[arch-m3]
6. 词边界 `-w` 对 `__TAURI_INTERNALS__`（README:10）漏检——AC6b 须并列子串口径[qa-m1+dev]
7. TP-01 缺回写技术栈取值正确性核验（版本数字写错无拦截）[qa-m2]
8. TP-06 与 TP-05 应构造收尾同批执行（执行序注记）[qa-m3]
9. AC1 #5 示例措辞与蓝图 #9 冲突（egui 预写 macOS）[pm-m1，并入 M4 处置]
10. ARCHITECTURE v0.3 整体刷新是否另批（开放问题⑥关联）[arch 开放问题⑥转记]

## 结论

0 BLOCKER / 8 MAJOR（去重 6）/ 17 MINOR（去重 10）。蓝图行锚纪律实测良好、23 处清单完备、AC↔TP 机械核对通过——**但 6 MAJOR 全部为三工件间口径/计数/锚位不一致**（恰是本批要修的病类在本批工件上的复发），照字面执行必出假越界/假漂移/假证伪。一轮修订（收敛口径+订正计数锚+补 carve-out+decisions 补录）可闭合；修订后复审只核收敛点。
**建议人类审批关注点**：①总裁决（打回修订 vs 带病通过——不推荐：M1/M4/M5 属执行必冲突级）②M3 口径裁决建议=要求 desktop fmt/clippy 单独跑（沿 audit intent 终验修正口径）③M6 命令主形态建议=build --release 为判据。
