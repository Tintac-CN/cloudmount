# Gate#1 复审 r2（2026-10-29）

> 评审对象：intent `ci-desktop-fmt-clippy`（route=classic）修订轮三工件——prd/architecture/test-strategy r1。
> 评审方：reviewer-architect（0/3/5）+ reviewer-qa（0/0/4）——合并 **0 BLOCKER / 3 MAJOR / ~6 MINOR**。
> 复审定位：只核收敛点（上轮 2 BLOCKER+8 MAJOR 消解），新发现仅报实质问题。

## 上轮 2 BLOCKER+8 MAJOR 消解实证（全部闭合）

- **B1 命令字面互斥 ✓**（qa 机械 `grep -F` 逐字命中）：三工件同串——`cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check`（prd:21/arch:79-169/TS:13）+ `cargo clippy --manifest-path apps/desktop/Cargo.toml --all-targets -- -D warnings`（prd:22/arch:81-170/TS:13）；TP-01 含 job id=desktop+runs-on 精确断言+cd/working-directory 双负断言；无「三形态」残留。
- **B2 rust-cache ✓**：蓝图值 `apps/desktop -> target`（arch:77）+断言 #7 精确值禁宽松 grep（arch:168）+「不连带根 workspace」叙述修正（arch:73-74/89/251）；三工件同步。
- **M1 AC4 锚集 ✓**：#21-#41 逐条对照 ci.yml 实文全部命中（含 check 命令/matrix 两处/components/另三 job/env 双键/on:）；#21 两个 `--` 齐。
- **M2 负断言 ✓**：#10-#14 覆盖 continue-on-error/任何 if:/status 函数/shell 静默绿且限定 desktop 块（不误伤 web-ui 的 npm ci || npm install）。
- **M4/M5/M6/M7 ✓**：现状统一「证据链预期 clean+当日实测」（时间线修正 533e456 11:21→2a02050 15:40）；语义对齐表按实文回填；缓存体积风险节+timeout 60；OQ① 裁决落实（独立 job+windows 单腿）消解形态冲突。
- **qa 侧 6 项全 PASS**：B1/M3（缓存精确断言+desktop 负断言）/M2/M6（diff 白名单+matrix 计数+on:/env）/M4+TP-04 升 P0/TP-05 注入机制/TP-06 PR 优先。

## 本轮新发现

### MAJOR（3，跨工件一致性/待裁决项）

- **M-R2-1｜OQ④（断言脚本载体/文件面是否入批）三工件状态不一致+架构单方关闭**[arch-MAJOR-1+qa-MINOR-3]：architecture 称「OQ④ 关闭/不再留载体裁决」（:131/:290），但 prd:63 与 TS:37 仍 `[ ]` 待人类裁决；decisions.md 人类裁决仅含 B1+OQ① 两项——架构无权关闭人类裁决项。牵连：AC3 约束力与 diff 白名单未定；TP-02 白名单仍写「若裁决 ci-yaml.test.sh 同步替换」双名分支（prd 已定名 ci-desktop-yaml.test.sh）；若④ 排除则 AC3 作废+TP-04/05 的 AC3 面失被测对象。**需 gate1 人类当场裁决**。
- **M-R2-2｜AC3「断言不得弱化」操作定义两版不一致**[arch-MAJOR-2]：prd:39 定义三条（负断言必须 grep_absent_ok/正断言不得放宽为子串/自检+CRLF+降级计数断言必须存在）；架构脚本头注（arch:143）只写「不得删除/不得改宽/计数不降/新增允许」——缺 prd ①③；对齐表 AC3 行又指向架构版本（arch:235）。gate2 无法判定哪版为准。修复=统一为 prd 三条（架构头注补齐）。
- **M-R2-3｜44 条断言清单与 TP-01/TP-02 实文未完全对齐**[arch-MAJOR-3]：TP-01 要求「无 RUSTFLAGS 覆写」负断言（inherit workflow 级 -D warnings）+working-directory 负断言——架构 #11 仅覆盖 cd、#10-#14 无 RUSTFLAGS；TP-02 要求 matrix 全文件精确计数=2——架构 #28/#30 仅 job 块内整行断言。RUSTFLAGS 覆盖是实质 fail-open 面（job 级 env 覆写会静默放行 rustc 级 warning）。修复=架构清单补 3 条（RUSTFLAGS 负断言/working-directory 负断言/matrix 全文件计数）→M 基线 44→47（或口径钉死）。

### MINOR（~6，去重）

1. prd 引用架构断言编号沿用 r0 旧编号 3 处错位（#7/#8→#8/#9、#10→#11、#6→#7）[arch-MINOR-1+qa-MINOR]
2. 架构 AC2 实测节仍保留 cd 形态（arch:120-122）与 prd「不得在任何工件复活」冲突 [qa-MINOR]
3. AC1「无 strategy.matrix」子项无直接负断言（TP-01 无 desktop 内 strategy/matrix 负断言）[qa-MINOR]
4. OQ② 状态表述不一致（arch 称实质收敛 vs prd 仍 `[ ]`）[arch-MINOR-3]
5. AC1⑤ `if: always()` 口径张力（prd 认为非 fail-open vs 架构 #12 禁任何 if:）——从严可接受但需明示 [arch-MINOR-4]
6. M=44 计数口径与 #42 多分支 report 不一致（静态条数 vs 实际执行条数——先例 112/113 环境相关）[arch-MINOR-5]

## 结论

上轮 2 BLOCKER+8 MAJOR 全部消解实证（qa 机械 grep 逐字命中+arch 逐项实测）；新发现 0 BLOCKER、3 MAJOR（OQ④ 待人类裁决/AC3 定义统一/清单补 3 条）~6 MINOR。**建议**：通过+准出回填（OQ④ 本门裁决后，三项文本级修订随批准由编排者闭合并留痕）→ Construction。
**建议人类审批关注点**：①总裁决②**OQ④ 裁决：断言脚本入批**（推荐——否则 AC3 断言守护作废、新 job 无守护；脚本=ci-desktop-yaml.test.sh 受本批 diff 白名单约束）。
