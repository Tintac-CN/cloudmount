# Gate#2 AI 互审（2026-10-29）

> 评审对象：intent `ci-desktop-fmt-clippy` 交付面（phase=gate2）——产品面 diff `2d5d695..HEAD`（`.github/workflows/ci.yml` +33/−0；`scripts/ci-desktop-yaml.test.sh` +420/−0）+ plan/units/state 证据链。
> 评审方：reviewer-pm（0/0/5）+ reviewer-architect（0/0/4）+ reviewer-qa（0/0/7）+ reviewer-dev（0/0/8）+ verifier 新鲜上下文终检（0/0/3）。合并去重 **0 BLOCKER / 0 MAJOR / 15 MINOR**。

## BLOCKER

无（五方一致）。

## MAJOR

无（五方一致）。

## MINOR（合并去重，均为文档口径或后续硬化项）

1. **M 计数口径措辞失准**（pm-1 / arch-m1 / qa-1 / dev-3 / verifier-m2，五方中四方独立命中）：architecture.md:160 与脚本头注均写「M=静态 `report` 调用点数（47）」；实测静态调用点 49（#1-#41 共 41 + #42a/#42b + #42 降级分支 + #43-#47 共 5）。运行期 M=47（actionlint 不可用）/48（可用），N==M 自洽——仅「静态调用点=47」标签错误。建议改述「47 个断言编号（#1-#47）；静态 report 调用点 49；运行期 M 以末行为准」。
2. **fmt 作用域的事实性错误**（verifier-m1，实测证伪）：architecture.md:91 称「`--manifest-path` 隔断根 workspace——不会连带根 crate」；实测 `cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check --verbose` 实际检查 **70 文件 = desktop 4 + 根 workspace 66**（desktop 的 path 依赖 core/auth/driver115/sync；`--all` 语义含 local path-based dependencies）。不影响红绿与 AC1 字面（`--all`=B1 裁决字面不得擅改），但「覆盖增量/失败归因」叙述失真，须订正。
3. **负断言语法变体旁路（未来维护风险，无现行违规）**：qa-2/3/4 + dev-2 + verifier-INFO-4——#12 对 `- if:` 列表项形态、#46 对 flow-map `env: { RUSTFLAGS: ... }`、#45 对 `working-directory: ./apps/desktop` 存在漏检；#10-#14 同类字面枚举。当前 ci.yml desktop 块零违规；硬化项候选。
4. **`job_block` 2 空格注释截断风险**（dev-1）：`^  [^ ]` 终止条件使 job 内 2 空格注释会截断块，若违规行位于注释后将被漏检（当前 ci.yml desktop 块无 2 空格注释，不触发）。候选改为 `^  [A-Za-z_][A-Za-z0-9_-]*:` 或头注禁令。
5. **#43/#44 自检深度**（qa-5 / dev-5）：自检为纯文本存在性检查，未端到端验证生产规范化行被实际使用（沿五脚本先例同型）；CRLF 探针锚 `name: CI` 不在 AC4 锁定集内。
6. **正断言未绑定 step 块**（qa-6 / arch-m3）：#5/#7/#15 在 job 块任意位置命中即可；若 `workspaces` 被错挂到 toolchain 的 `with:` 下仍会绿（B2 类静默降级残余面）。当前逐行核对正确。
7. **TP-02 `<base>` 记录载体与措辞不符**（pm-2 / arch-m4 / dev-8 / verifier-7）：test-strategy 写「Construction 记录于 decisions」，实际记录于 plan.md:4 与 units/1.md:5（`2d5d695`），decisions.md 无该字样。建议随批准在 decisions 补记基线锚。
8. **AC3「六脚本」硬门载体未落盘**（pm-3 / qa-7 / dev-6 / verifier-m3）：prd.md:41 要求 runbook/checklist 以六脚本口径收录；本 intent 尚无 ops-runbook.md，`docs/aidlc/desktop-installers/release-checklist.md:56` 仍为五脚本循环。须由 Operations 显式闭合（否则 PRD 自述「死守护」）。
9. **OQ② 小节标题张力**（pm-4）：architecture「已关闭」小节内含「PRD 侧 OQ② 仍留 `[ ]`」；三工件语义已互引一致，仅标题归属可订正。
10. **python/pyyaml 依赖未在脚本头注注明**（dev-4）：#42 降级分支依赖 python+yaml；无 python 环境时红/绿计数与文档不同（仍非零退出，方向正确）。
11. **文档精度小项**（dev-8）：architecture.md:20「一致性由断言 #2/#19 锁定」无顺序断言支撑；:19「CI 依赖钉死」未加 `--locked`（与既有 rust job 同纪律）；`@stable` 无 `rustc -V` 打印步；timeout 60 冷编超时则不保存 cache（架构已注 2x 余量调整）。
12. **TP-06 / M6 未验证披露完整**（pm-5 / dev-7）：CI 首跑（desktop job 实跑、runner rustc 版本、cache 体积/命中率）与 branch protection 未加 required checks 均已如实披露，非缺陷；须在 ops/retro 留痕闭合。

## 独立复跑与正面实证（五方交叉）

- **断言脚本**：五方独立复跑 `bash scripts/ci-desktop-yaml.test.sh` → 末行 `47/47 passed` EXIT=0（本机 actionlint 不可用走降级：`SKIP actionlint` + `yaml.safe_load` + `WARN 降级告警`，计数诚实）；异工作目录运行同样 47/47。
- **既有五脚本零回归**：112/112、72/72、23/23、13/13、9/9 全 EXIT=0（多角色独立复跑一致）。
- **F2 回归**：`cargo test --workspace` 26 目标 / 277 passed / 0 failed，与 `state.testBaseline`（277/0、suites 26 项）逐数一致——零回退。
- **AC2 实测**：desktop fmt EXIT=0；clippy `--all-targets` 带 `RUSTFLAGS` EXIT=0；不含 `--all-targets` 对比 EXIT=0（clean，OQ③ 未触发）。
- **蓝图 vs 实现**：architect 用脚本抽取蓝图代码块与 ci.yml:59-90 逐行比对 `LINE_DIFFS 0`；YAML `safe_load` 通过，jobs 顺序 `rust, rust-mock-features, desktop, web-ui, arm-cross`。
- **diff 与零删改**：产品面仅两文件（`+33/−0`、`+420/−0`）；单 hunk 纯插入于 rust-mock-features 与 web-ui 之间；四既有 job、workflow env/on: 零触碰；黑名单零命中；工作区干净。
- **TDD 过程**：红 `810ad27`（29/47，FAIL 恰 #1-#16/#45/#46）→ 锁 `f5d0295` → 绿 `d5818ed`；`git diff 810ad27..HEAD -- scripts/ci-desktop-yaml.test.sh` 为空（脚本 blob 未变）；负断言判别力经探针抽测（`continue-on-error`/`if: always()`/`|| true`/`working-directory`/`RUSTFLAGS` 覆写/`apps/desktop -> desktop` 均命中对应断言）。
- **过程完整性**（verifier）：reflog 线性无 reset/amend/stash；state.history 无丢失；decisions 末条 `GATE-APPROVED gate1` 与 state 门控一致；无计划外文件、无未跟踪污染。

## 结论

**0 BLOCKER / 0 MAJOR / 15 MINOR**——交付与证据链自洽，断言真实执行且能区分主要违规形态；verifier 终检结论「实现符合计划、验证真实通过，可进入 Gate#2 人类审批」。15 MINOR 均不阻塞：其中 1/2/7/9/11 为文本级订正（可随批准准出回填闭合），3/4/5/6/10 为断言硬化候选（TDD 锁内脚本，建议登记 backlog 后续批），8/12 为 Operations 阶段必须显式闭合的出口条件。

**建议人类审批关注点**：
1. 总裁决（推荐通过+准出回填）。
2. **脚本头注 M 口径订正方式**：保持 TDD 锁不动（仅在 architecture/decisions 订正 + backlog 登记头注措辞）vs 授权 `/aidlc:unlock-tests` 订正头注后重锁（注释级、断言零变更、需重跑 47/47 并留痕）。
3. Operations 出口条件须明确：六脚本硬门口径收录 + TP-06 首跑观察（desktop job 首跑绿/runner rustc 版本/cache 体积命中率）+ branch protection 现状记录。
