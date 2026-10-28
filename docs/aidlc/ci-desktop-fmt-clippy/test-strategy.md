# Test Strategy: ci-desktop-fmt-clippy

> 产出者：qa　日期：2026-10-28　来源：intent.md + prd.md

## 测试点清单

映射口径：**prd.md 未落盘**——映射列按 A1 三拆骨架预映射（拆①=ci.yml 新增 desktop job[fmt+clippy，沿既有 job 模式]；拆②=desktop 现状确认；拆③=断言脚本扩展），**待对齐拍核对**（prd 落盘后按实际 AC 编号回填，禁虚构编号）。环境事实：Windows/Git Bash 本机；actionlint 不可用（降级口径见 TP-05）；CI 实跑不可本地验证（TP-06 归 E2E/CI 首跑）。方法学参照：desktop-installers test-strategy（TP-05 desktop fmt/clippy 必达口径来源）+ 五断言脚本先例机制（job_block 提取/grep_ok 防空真/CRLF 剥除/actionlint 降级计数诚实）。

| # | 映射 AC | 场景（正/负路径） | 层级 | 优先级 |
|---|---|---|---|---|
| TP-01 | 拆① | 正：ci.yml 含 desktop fmt+clippy 执行面字面——job_block 提取+grep 断言沿五脚本先例（desktop-installers-yaml.test.sh 头部机制：job_block/grep_ok 防空真/CRLF 剥除）：desktop fmt 命令原文（`cargo fmt --all -- --check`）+ desktop clippy 命令原文（`cargo clippy --all-targets -- -D warnings`——`--all-targets` 含否随开放问题②裁决，终验口径=含；desktop 为 workspace exclude 独立项目，根级 `--workspace` 形态的适配按 desktop Cargo 结构 Construction 实测钉死）出现于 desktop 上下文锚（`working-directory: apps/desktop` 或 `cd apps/desktop` 或 `--manifest-path apps/desktop/Cargo.toml` 三形态之一，Construction 落字面——沿 desktop-installers-yaml.test.sh「Construction 落字面」先例节）；toolchain step 含 `components: clippy, rustfmt`（沿 rust job 先例）+ `Swatinem/rust-cache@v2` step 存在（intent 缓存约束——desktop 独立 target 需 cache key 覆盖，key/workspaces 参数形态 Construction 钉死）；负子项：新 job/step 无 `continue-on-error`、无绕过性 `if:`（如 `if: always()`）——desktop 违规必须真能红，防「补了步骤但静默绿」空真形态（沿 desktop-installers TP-02 先例口径） | 单元 | P0 |
| TP-02 | 拆① | 负：既有四 job 零删改护栏（diff 边界）——rust/rust-mock-features/web-ui/arm-cross 四 job_block 内 step 字面锚定集不变：rust（`cargo fmt --all -- --check`/`cargo check --workspace`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo test --workspace` + `components: clippy, rustfmt`）、rust-mock-features（`cargo test --workspace --features cloudmount-daemon/mock`）、web-ui（`npm ci || npm install`/`npm run build` + `working-directory: web-ui`）、arm-cross（`cross build -p cloudmount-daemon -p cloudmount-cli` + 双 `--help` smoke）；job_block 界定防跨 job 字面误配（desktop 新面若含同字面 fmt 命令不得误计入 rust job 锚定）；注：开放问题①若裁决「rust job 内追加 step」形态，护栏口径修订为「既有 step 字面零删改+仅允许追加」（desktop-installers B2 先例口径），修订须 decisions.md 留痕 | 单元 | P0 |
| TP-03 | 拆② | 正/负：desktop 现状本机实测——`cd apps/desktop && cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings`（Git Bash 本机；clippy 按终验口径含 `--all-targets`，另跑不含口径对比记录供开放问题②）；clean→CI 补死后首跑绿为预期；dirty→违规明细（文件/行数——desktop-installers gate2 M3 实证 desktop 内 fmt dirty 394 行未被根级命令拦截，fmt 侧大概率 dirty）如实记录，CI 首跑红为预期（红=门控生效，非缺陷），修复范围归开放问题③裁决 | 集成 | P0 |
| TP-04 | 拆③ | 负：断言脚本自身质量（沿五脚本先例三模式）——防空真（每条 grep 命中≥1 行否则判红；被测文件/锚缺失不空真）、CRLF 剥除规范化副本（ci.yml 为 CRLF 检出，断言前剥 \r 再 grep/awk）、actionlint 缺失降级计数如实（SKIP+WARN 非静默绿）；新断言不破坏既有计数——载体若为扩展 desktop-installers-yaml.test.sh 则其 `<N>/<M> passed` 计数变化如实（M 增属预期，既有断言不得由绿转红——该脚本为 desktop-installers 批锁定绿态基线）；载体若为新脚本则沿五脚本命名模式（`*.test.sh`）独立落文件 | 单元 | P1 |
| TP-05 | 拆①/③ | 正/负：ci.yml YAML 合法性——actionlint 不可用降级：python `yaml.safe_load` 解析成功+键配平粗检（沿五脚本先例 actionlint 降级模式：SKIP + safe_load 粗检 + WARN）；负子项：safe_load 解析失败→FAIL 非静默跳过；降级输出如实（SKIP+WARN 计数诚实）；YAML schema 级有效性（GitHub Actions 表达式/on 语法）本地不可全验，归 CI 首跑兜底 | 集成 | P1 |
| TP-06 | 拆① | 正/负（E2E，不可本地验证——归 CI 首跑）：push 后 GitHub Actions 首跑——desktop fmt/clippy step 实际执行于 runner；结果与 TP-03 现状一致（clean→绿；dirty→红且日志含 fmt/clippy 违规明细——红即门控生效证明，终验口径缺口[audit-roadmap §4.1 缺口 1/2]补死的最终行为判据）；缓存命中观察（二次跑 `Swatinem/rust-cache` 对 apps/desktop 独立 target 的覆盖——时长优化项，不设门） | E2E | P0 |

**覆盖核对**：拆①←TP-01/02/05/06；拆②←TP-03；拆③←TP-04。每拆 ≥1 测试点，无缺锚。负路径覆盖：TP-02（零删改护栏）/TP-04（脚本质量）为负路径测试点；TP-01 含无绕过负子项；TP-03 含 dirty 负向现状实录；TP-06 含 dirty 时首跑红预期。

**进出门**（沿 desktop-installers 先例口径）：P0 共 4 项（TP-01/02/03/06）——P0 全部落位且可运行（红态：TP-01 desktop 断言红——ci.yml 现无 desktop 面；TP-02 护栏现状绿属正常[回归护栏]；TP-03 可跑出现状实录；TP-06 为 E2E 项以「落位+执行条件明确」计，先例 TP-22 同模式）方可进 Construction；出门=全绿（TP-06 以 CI 首跑结果为准——desktop dirty 时按开放问题③裁决口径判定）。

## 分层策略

- **单元**（字面断言脚本，本地 Git Bash 全可跑；载体=新脚本或扩展 desktop-installers-yaml.test.sh，归拆③/prd 定）：TP-01（desktop fmt+clippy 命令原文/desktop 上下文锚/toolchain components/rust-cache step/无 continue-on-error 绕过）+ TP-02（既有四 job step 字面锚定集零删改护栏，job_block 界定防跨 job 误配）+ TP-04（脚本自检：防空真/CRLF 剥除/降级计数诚实/不破坏既有计数）。**不测**：YAML 语义级有效性、CI runner 实际执行行为、desktop 源码修复（dirty 修复归 desktop 代码批/开放问题③裁决——本批仅 CI 配置变更）。
- **集成**（ci.yml 结构+desktop 现状，本机可跑）：TP-05（safe_load 解析粗检——actionlint 缺失降级沿先例）+ TP-03（本机 cargo fmt/clippy 实测 desktop 现状，clean/dirty 如实记录——决定 CI 首跑红绿预期）。**不测**：GitHub Actions 三平台 runner 差异、真实缓存命中、CI 时长。
- **E2E**（CI 实跑，不可本地验证——标注归 CI 首跑）：TP-06——push 后 GitHub Actions 首跑 desktop fmt/clippy step 实际执行+结果与 TP-03 现状一致+缓存命中观察。**不测**：CI 时长优化本身（只观察不设门）；desktop UI 行为（非本批面）。

## 缺口与开放问题

- [ ] CI 实际执行不可本地验证：push 后 GitHub Actions 首跑为准（TP-06 归 E2E——本地只做字面/结构/现状三层代理验证）。
- [ ] desktop dirty 时修复范围待开放问题③裁决（intent 开放问题③）：TP-03 实测后如实记录违规明细；CI 补死后首跑红的处置（修复归本 intent 还是分批）不属本测试策略可裁决项。
- [ ] CI 形态未决（开放问题①）：新增独立 job vs rust job 内追加 step——TP-01 断言字面随形态钉死；追加形态下 TP-02 护栏口径修订（既有 step 零删改+仅允许追加，B2 先例），修订须 decisions.md 留痕。
- [ ] clippy `--all-targets` 含否（开放问题②）：终验口径=含——TP-01/TP-03 按含口径断言/实测；若裁决收窄须同步修订断言字面（decisions.md 留痕）。
- [ ] actionlint 本机不可用：TP-05 降级 safe_load 粗检；YAML schema 级有效性（GitHub Actions 表达式/on 语法）本地不可全验，归 CI 首跑兜底。
- [ ] 断言脚本载体未决（拆③）：新脚本（沿五脚本 `*.test.sh` 命名模式）vs 扩展 desktop-installers-yaml.test.sh（既有计数基线变化须如实，该脚本为 desktop-installers 批锁定绿态）——归 prd/Construction 定。
- [ ] prd 未落盘：映射列按 A1 三拆骨架预映射，待对齐拍核对——prd 落盘后按实际 AC 编号回填（禁虚构编号）。
- [ ] 缓存 key 对 apps/desktop 独立 target 的实际命中：本地不可验——CI 首跑后二次跑观察（时长优化非正确性，不设门）。
