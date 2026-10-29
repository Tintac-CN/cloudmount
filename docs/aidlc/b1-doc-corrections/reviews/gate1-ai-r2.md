# Gate#1 复审 r2（2026-10-17）

> 评审对象：intent `b1-doc-corrections`（route=classic）修订轮三工件——prd.md @ 1d651a6 / architecture.md @ r1 / test-strategy.md @ ee8466d。
> 评审方：reviewer-pm（0/0/2）+ reviewer-architect（0/1/2）+ reviewer-qa（0/1/2）+ reviewer-dev（0/0/2）——合并 **0 BLOCKER / 2 MAJOR / 7 MINOR**。
> 复审定位：只核收敛点（上轮 6 MAJOR+10 MINOR 消解），新发现仅报实质问题。

## 上轮 6 MAJOR 消解实证（全部闭合）

- M1 api.ts 触碰面 ✓（四方确认「:1-6 头注块」单一口径，:5 骨架句搭车采纳，:1-6/:44-81 边界实证精确可判）
- M2 TP-07 计数锚 ✓（动态清点，无绝对值断言，全篇无残留）
- M3 fmt/clippy 门口径 ✓（desktop 单独跑必达三工件一致，豁免删净）
- M4 TP-01 carve-out ✓（ARCHITECTURE.md:48「M5+ 技术未定」豁免+取值正确性核验补齐）
- M5 ROADMAP:164 ✓（#25 移 B 段必达+措辞域收敛+D11 记录——残留见 M-A）
- M6 命令主形态 ✓（build --release 主判据+cargo run 超时冒烟，超时+kill 语义写明）
- 10 MINOR 全消（逐条核对：decisions 补录/README:143 点名/E+ 段登记/排除清单/证据精度/双口径/执行序等）
- 行锚纪律保持：r2 触及面抽验 15+ 处逐字命中零漂移（dev/pr/arch 三方独立实测）。

## 本轮新发现

### MAJOR（2，均一行级收敛）
- **M-R2-1｜#25 括号内替换字面三工件两套**[arch-R2-M1+dev-m1+pm-m1 同源]：r1 重写 #25 时引入——蓝图钉「TLS 见后续」→「TLS 可选已落地——--tls-cert/--tls-key（见下条）」（含嵌套全角括号），prd AC5(b)/TP-06 一致钉「（TLS 可选，见下条）」。域与分类已归一（M5 主诉求闭合）；dev 判定蓝图长形态为唯一合理落笔路径（TP-06 事实锚判据仅长形态可满足）。处置=三处统一单一形态（dev 建议扁平含锚「（TLS 可选——`--tls-cert/--tls-key`，见下条）」），防构造期逐字比对假返工。
- **M-R2-2｜TP-03 辅判据「回写行 dev 形态=cargo run」与蓝图 #4「cargo run 不写入本行」冲突+误引 prd AC2**[qa]：prd AC2 实文未钉回写行文本，cargo run 仅冒烟语义——照 TP-03 字面把 cargo run 落入回写行=违反蓝图 #4。一句话收敛（辅判据改「cargo run 冒烟为独立冒烟指令，非回写行文本；回写行文本按蓝图 #4=build --release 形态」）。

### MINOR（7，注记/精度级）
1. api.ts 证据区间 :44-80 vs :44-81（arch-m2+pm-m2 同源——转写精度，执行锚 :1-6 三方一致）
2. TP-02 AC6a 分项清单 8 文件 < prd 定义 10 文件（两扩展文件在分片清单内被覆盖，仅标签不对齐）[qa-m2]
3. TP-06 执行序双列（①初核+⑤收尾，应单一口径）[qa-m1]
4. architecture「并行中」时态过期叙述三处 [dev-m2]
5. TP-01 引 :11-31 vs 蓝图 F1 :11-32（一位差，核验对象在 ：11-31 内）[arch-m1 部分]
6. AC5(b) 引文形态统一后的连带措辞（随 M-R2-1 一并）[同源]
7. decisions 扩裁补录已落（pm-m3 消解——归并入上轮 MINOR 消解清单，此处并入计数）

## 结论

上轮 6 MAJOR+10 MINOR 全部消解实证；新发现 2 MAJOR 均一行级收敛（M-R2-1 三处统一字面/M-R2-2 TP-03 辅判据一句话改写），7 MINOR 注记级。**建议**：通过+准出回填（2 MAJOR+MINOR 随批准由编排者一行修闭合并留痕）——避免为一行修再走一轮全流程复审。Construction（23 处回写执行）随后即开。
