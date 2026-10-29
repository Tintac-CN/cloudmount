# Gate#1 复审 r2（2026-10-16）

> 评审对象：修订轮三工件——prd.md @ f13d80d（AC1..AC15）/ architecture.md @ 7eb70ba / test-strategy.md @ ad264f0+align2。
> 评审方：reviewer-pm（0/1/2）+ reviewer-architect（0/0/0）+ reviewer-qa（0/0/2）+ reviewer-dev（0/1/1）——合并 **0 BLOCKER / 2 MAJOR / 5 MINOR**。
> 复审定位：只核收敛点（上轮 10 MAJOR+16 MINOR 消解），新发现仅报实质问题。

## 上轮 10 MAJOR 消解实证（全部闭合）

- M1 TP-01 证伪重标定 ✓（code_challenge/Commands::Login 实测命中；S256 差异列独立小发现）[qa+dev 双证]
- M2 AC2 两未勾项订正 ✓（prd:28 与 TP-02/23 同口径，实读 ROADMAP L8/L16 一致）[pm]
- M3 AC10 补 L8+外部阻塞链源头 ✓（AC14 排序输入同步）[pm]
- M4 零写入条款 ✓（prd:44+test-strategy:53 同口径）[pm]
- M5 载体钉死 ✓（audit-findings.md+骨架 §0–§6+AC15 五项机械判据+TP-17/25/26 落点）[pm+dev 双证]
- M6 AS-IS 9 job ✓（merge :355-384 区间锚精确，三方口径一致）[arch]
- M7 CI/CD 权限小节 ✓（逐项实测命中：默认 permissions 缺失/三显式收权/六继承/12 可变 tag/无 environment）[arch]
- M8 TP-27 AC13 七项批量 ✓（七项锚点逐一实测命中+TLS 收敛句）[qa+dev 双证]
- M9 testutil 裸命令 ✓（ci.yml:34+dev-dependencies 机制核实，三方口径一致）[qa+dev 双证]
- M10 cldflt 执行面声明 ✓（基线同机口径可比+git 面零写入边界）[dev]
- 上轮 16 MINOR 逐条核对无残留（qa 全清核对）；另 arch 侧 0/0/0 全清，D1 区间锚处置判定正确（上轮建议 :48→:47 本身有误，:47 为代码围栏）。
- 43 条 [x] 清点与 ROADMAP 实读精确一致（grep -c=43）；TP-01..30 编号稳定连续；AC1..AC15 覆盖机械终检通过。

## 本轮新发现

### MAJOR（2，均字面级一行修）
- **M-R2-1｜architecture.md:79 对齐表「核验」行残留「本期默认抽样」**[pm]：与人类裁决③（全量验证，prd:19/47/52「不得缩减为抽样」）直接矛盾——该表自认是 qa 断言锚接口，照此行执行即缩减覆盖；r1 修了 M6/M7 漏同步此格。一行订正：「全量验证（gate1 裁决③，见 prd 头注）」。
- **M-R2-2｜TP-28 执行动作引用 3 个不存在的模块路径**[dev]：`{db,watcher,probe,queue,cache,executor}` 中 `db`/`probe`/`executor` 不存在（sync/src 无子目录；实际落点 SQLite=store.rs/state.rs、change_probe=engine.rs/remote.rs、执行器=exec.rs）——P0 动作照字面部分失败（与上轮 M9 同类）。一行修：括号清单改 `store/state/engine/exec/queue/cache/watcher`。

### MINOR（5，注记级）
1. architecture.md:80 只读面措辞窄于零写入裁决（未覆盖文档与 .gitignore 面）[pm]
2. architecture.md:75 与 test-strategy:5/54「AC1..AC14」陈旧陈述未随 AC15 更新 [pm]
3. test-strategy:6 M2 分区枚举不含 TP-18（L111/124 场外承接），N/N 分母 10/12 不闭合——实际覆盖无缺口 [qa]
4. AC15 联合承载未回写 TP-25/26 映射列（仅 TP-17 注记+自检行；机械 grep 得 AC15←TP-17）[qa]
5. TP-27⑥ 行号区间微漂（main.rs:48-51→实际 consent@:53）[dev]

## 结论

上轮 10 MAJOR+16 MINOR 全部消解实证；新发现 2 MAJOR（M-R2-1/M-R2-2）均字面级一行修、5 MINOR 注记级，无结构问题、无新 BLOCKER。**建议**：通过+准出回填（2 MAJOR+5 MINOR 随批准闭合，编排者核收留痕）——避免为两处一行修再走一轮全流程复审。
