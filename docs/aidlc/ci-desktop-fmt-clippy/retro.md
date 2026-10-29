# Retro: ci-desktop-fmt-clippy

> 复盘日期：2026-10-29　参与：人类（门控裁决）+ 编排者 + pm/architect/qa/dev/reviewer 系列/verifier

## 什么有效

- **Gate#1 两轮抓出实质错误**：互审抓出 B1（fmt/clippy 命令字面三工件互斥）与 B2（rust-cache `workspaces` 值语义错误——`apps/desktop -> desktop` 解析为不存在目录）两个 BLOCKER；人类「打回修订，两项推荐采纳」一次钉死字面源与 job 形态，r2 只核收敛点即收敛。
- **TDD 红→锁→绿执行严格**：红提交 `810ad27`（29/47，FAIL 恰为 desktop 断言 18 条）→ 锁 `f5d0295` → 绿 `d5818ed`（47/47）；`git diff 810ad27..HEAD -- scripts/ci-desktop-yaml.test.sh` 为空，锁定后脚本零改动有机械证据。
- **红阶段双向可证伪验证**（dev 主动作为）：隔离临时树注入蓝图 job→47/47（证明绿可达）；注入违规（continue-on-error/if: false/|| true/working-directory/RUSTFLAGS 覆写/错缓存值/第三处 matrix）→ 对应断言 FAIL（证明非空真）。这是本批测试可信度的关键支撑。
- **五方 Gate#2 互审 + verifier 新鲜上下文**：五方独立复跑同一组命令（断言 47/47、五脚本 112/72/23/13/9、cargo test 277/0）形成可交叉验证的证据链；verifier 用 `cargo fmt --verbose` 实测证伪了架构「`--manifest-path` 不会连带根 crate」的叙述（实际检查 70 文件），这是本轮最有价值的发现。
- **准出回填模式高效**：gate1 3 MAJOR、gate2 15 MINOR 均以文本级订正随批准闭合，未再走复审；`--all` 作为 B1 裁决字面保持不动，避免为文档措辞改动已批准命令。
- **基线/回归纪律**：第 0 步 277/0 基线先于测试文件写入；验证前回归逐数一致；零 Rust 源码改动使回归比对语义干净。

## 什么没用

- **M 计数口径反复**：r1 写「M=44」、r2 写「M=47 静态 report 调用点」、gate2 五方实测静态调用点实为 49（编号 47、运行期 47/48）。同一口径经三轮才收敛，暴露「断言编号/静态调用点/运行期计数」三个概念混用。
- **负断言字面枚举的旁路**：`- if:` 列表项、flow-map `env: {RUSTFLAGS: ...}`、`working-directory: ./apps/desktop` 等 YAML 合法变体未被现有字面枚举覆盖；当前实现无违规，但守护强度有残余缺口。
- **`job_block` 启发式边界**：2 空格注释会终止块提取；若 job 内出现 2 空格注释且违规行在其后，负断言可能漏检（沿五脚本先例同型缺陷）。
- **文档事实未在写作时实测**：`--all` 作用域叙述（r0/r1 两版均错）直到 verifier 跑 `--verbose` 才暴露。架构中的 cargo/action 语义声明缺少「写作时实测」纪律。
- **AskUserQuestion 三问未获选择**：ops 的观察窗口/required checks/缓存阈值以推荐暂定值落 runbook（标注待追认），未拿到人类逐字裁决。

## 学到的东西（沉淀进 AGENTS.md / 角色约束）

- **断言计数必须三值定义**：断言编号数 / 静态 report 调用点 / 运行期 M（环境相关）；三者不得混用，头注与文档一律写清。
- **cargo 语义声明须实测**：`fmt --all` 含 local path-based dependencies（连带根 workspace）；`clippy --workspace` 不含 exclude 成员——凡进架构的 cargo 行为断言，写作时用 `--verbose`/`cargo metadata` 实证并留命令。
- **锁定脚本的注释准确性**：TDD 锁后不为措辞改脚本（人类裁决「保持锁定」）；口径订正走架构/decisions，脚本头注字面登记 backlog 随下次解锁批处理。
- **红阶段验证应包含「绿可达 + 违规可证伪」双注入**：只证明红不够，须证明「实现后可达绿」与「违规会被抓」两方向。
- **门控阈值类问题表单未答时**：记录「未获选择」并用推荐暂定值落 runbook，标注待追认——不得伪装为人类已裁决，也不得因此阻塞 ops 工件产出。

## 下个 bolt 要改的

- **断言硬化批**（backlog 条目 40）：负断言语法变体（`- if:`/flow-map env/`./` 前缀）、`job_block` 终止条件收紧、#43/#44 端到端自检、头注 M 口径与 python 依赖注明；需 `/aidlc:unlock-tests` 授权。
- **首跑观察**（backlog 条目 41）：desktop job 首次 push/PR 运行——绿红、时长、runner `rustc -V`、cache 体积/命中率；required checks 稳定后加入。
- **后续新 job 写法**：先在本地对 cargo 命令作用域做 `--verbose` 实证，再写入架构；断言脚本从「三值计数定义」起笔。
- **已完结工件只读纪律保持**：`desktop-installers/release-checklist.md` 五脚本循环不改，六脚本口径由本 runbook 承接。
