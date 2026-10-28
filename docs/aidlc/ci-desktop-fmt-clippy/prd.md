# PRD: ci-desktop-fmt-clippy

> 产出者：pm　日期：2026-10-28　来源：intent.md

## 用户故事

| # | 角色 | 目标 | 价值 | 溯源（intent.md） |
|---|---|---|---|---|
| U1 | CI 消费者（PR/push 门控的评审者与维护者） | ci.yml 新增（或扩展）desktop 执行面，使 `apps/desktop` 的 fmt+clippy 在 CI 真实执行——desktop 侧格式/静态检查违规即 CI 红 | PR/push 门控反映完整终验口径，消除「根级全绿=全部全绿」的覆盖假象（desktop-installers gate2 M3 实证：desktop 内 fmt dirty 394 行未被根级命令拦截） | 「一句话意图」+「问题与期望」（终验口径缺口——audit-roadmap §4.1 缺口 1/2） |
| U2 | desktop 开发者 | desktop 格式/静态检查违规在 PR 阶段即被 CI 拦截，而非终验/发版前才暴露 | 违规早知早修，fmt/clippy 债不再累积到终验（394 行债教训） | 「受影响系统/用户」：「desktop 开发者（违规即早知）」 |
| U3 | 仓库守卫者（后续 workflow 演进者） | 新 desktop 执行面被断言脚本字面守护，既有四 job 零删改 | 后续 workflow 演进不悄悄回退 desktop CI 覆盖、不破坏既有门控（沿五脚本先例的回归护栏） | 「约束」：「沿既有 ci.yml 模式」「既有 job 零删改」+ decisions.md A1 拆分预览③（断言脚本扩展沿五脚本先例） |

## 验收标准（可测）

- **AC1**（U1）——desktop fmt+clippy 执行面字面在位（字面可断言）：
  - Given：ci.yml 现状四 job（`rust`/`rust-mock-features`/`web-ui`/`arm-cross`）；`rust` job 的 Format/Clippy step 为根级命令（`cargo fmt --all -- --check`／`cargo clippy --workspace --all-targets -- -D warnings`），对根 Cargo.toml:13 `exclude = ["apps/desktop"]` 的独立 cargo 项目零效力——desktop fmt/clippy 面在 CI 从不执行。
  - When：本批在 ci.yml 落 desktop 执行面（形态按开放问题①裁决：新增独立 job 或既有 `rust` job 内追加 step；OS matrix 覆盖随①一并裁决）。
  - Then（字面可断言子项）：
    1. 存在 desktop fmt 步：命令原文 `cargo fmt --all -- --check`，且处于 desktop 上下文锚内——`working-directory: apps/desktop`（job defaults 或 step 级）／run 脚本内 `cd apps/desktop`／`--manifest-path apps/desktop/Cargo.toml` 三形态之一（具体形态 Construction 按裁决落字面，沿 desktop-installers-yaml.test.sh「Construction 落字面」先例）；
    2. 存在 desktop clippy 步：命令含 `cargo clippy` 与 `-D warnings`，`--all-targets` 含否按开放问题②裁决落字面（终验口径=含）；desktop 上下文锚同上；
    3. toolchain step 含 `components: clippy, rustfmt`（沿 `rust` job 先例 dtolnay/rust-toolchain@stable）；
    4. 缓存覆盖（intent 缓存约束——desktop 构建依赖重 eframe/wgpu，独立 target 目录）：新增面须使 apps/desktop 的 target 目录进入 Swatinem/rust-cache@v2 缓存 key（新 job 则含该 step；job 内追加则既有 rust-cache step 参数适配——参数形态 Construction 钉死）；
    5. 负断言：新 job/step 无 `continue-on-error`、无绕过性 `if:`（如 `if: always()`）、无 status 函数调用——desktop 违规必须真能红（沿 desktop-installers TP-02 先例口径，防「补了步骤但静默绿」空真形态）。

- **AC2**（U2）——desktop 现状本机实测如实记录（决定 CI 首跑预期）：
  - Given：desktop fmt/clippy 最近留痕验证为 clean——2026-10-16 desktop-installers 准出回填核收复跑「desktop build/clippy/fmt clean」（M3 全量 fmt 394 行@2a02050 清偿后）+ audit-roadmap TP-14 双面复跑 exit 0；其后 b1-doc-corrections（纯文档批）与 b2-115-qualify（文档零触 apps/desktop）均未触 desktop 源码。pm 角色无执行工具，2026-10-28 当日状态无法由 pm 复跑确认（开放问题⑤）。
  - When：Construction 拆②开工时本机（Git Bash）执行 `cd apps/desktop && cargo fmt --all -- --check` 与 `cd apps/desktop && cargo clippy --all-targets -- -D warnings`（另跑不含 `--all-targets` 口径对比记录，供开放问题②裁决），命令原文+退出码+（若 dirty）违规明细（文件/行数）留痕于 decisions/units。
  - Then：现状如实记录且 CI 首跑预期随之钉死——clean → CI 补死后首跑绿为预期基线（开放问题③自动消解）；dirty → CI 首跑红为预期（红=门控生效，非本批缺陷），修复范围归开放问题③裁决。基于证据链的预期为 clean，但以开工当日实测为准——不许以历史 clean 替代当日实测（retro「覆盖率假象」教训）。

- **AC3**（U3）——断言守护（沿五脚本先例）：
  - Given：现有五断言脚本（scripts/{desktop-installers-yaml,installers-yaml,release-yaml,check-macos-yaml,package-macos}.test.sh）被测对象为 release.yml/check-macos.yml/main.wxs/打包脚本——ci.yml 当前零断言覆盖。
  - When：本批新增（或扩展）断言脚本守护 AC1 新执行面字面与 AC4 护栏字面（载体按开放问题④裁决：新脚本 scripts/ci-yaml.test.sh 或既有脚本扩展），沿五脚本机制先例（job_block 段提取／grep_ok·grep_absent_ok 防空真／CRLF 剥除规范化／actionlint 不可用降级且降级计数诚实）。
  - Then：红阶段（desktop 执行面未实现）desktop 相关断言 FAIL（缺失不空真）；实现后脚本全绿 exit 0（末行 N/N passed）；护栏类断言（AC4 既有四 job 字面锚）现状绿保持绿；脚本自检断言（防空真机制/CRLF 剥除在位）沿 desktop-installers TP-10 先例；断言不得弱化（契约口径沿先例）。

- **AC4**（U3）——既有四 job 零删改（diff 边界）：
  - Given：ci.yml 现状四 job 及 workflow 级 env（`CARGO_TERM_COLOR`/`RUSTFLAGS: "-D warnings"`）、`on:` 触发（push main + pull_request）——字面锚集：`rust` job 四步命令原文（`cargo fmt --all -- --check`/`cargo check --workspace`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo test --workspace`）+ 三 OS matrix + `components: clippy, rustfmt`；`rust-mock-features` 的 `cargo test --workspace --features cloudmount-daemon/mock`；`web-ui` 的 npm 步；`arm-cross` 的 cross 构建步。
  - When：本批改动落盘，diff 审查+断言脚本护栏双通道核验。
  - Then：上述既有字面锚全部在位原样——零删改（若裁决①选 `rust` job 内追加，允许纯尾部追加 desktop step，既有 step 序与字面不动；追加不算删改）；本批 diff 仅含 ci.yml 新增面+断言脚本（若④裁决纳入）——desktop 源码/release.yml/check-macos.yml/根与 desktop Cargo.toml 零触碰。

## 非目标（明确排除）

- 不做 desktop 源码修复：desktop fmt/clippy 违规（若开工实测发现）修复归 desktop 代码批——除非开放问题③裁决纳入本批（intent 约束「仅 CI 配置变更」）。
- 不做 desktop `cargo test` CI 步骤：intent 范围仅 fmt+clippy（desktop 测试 CI 可见性为 audit §4.1 缺口面的另一子项，如需另开批次）。
- 不做 release.yml 变更：desktop 构建面已在 release.yml 覆盖（desktop-installers 批已落）。
- 不做 check-macos.yml 变更（permissions 显式化=backlog 条目 3 独立批）。
- 不做 actionlint CI 步骤补齐（backlog 条目 1 独立批次；断言脚本内 actionlint 降级机制沿五脚本先例，与 CI 步骤补齐无关）。
- 不改根/desktop Cargo.toml：workspace exclude 为既定架构决策（GUI 依赖图隔离），本批不动。
- 不优化既有四 job 的时长/缓存：既有 job 零删改约束下，缓存/时长优化仅限新增 desktop 面（AC1④）。

## 开放问题（需人类裁决）

- [ ] ①（intent 原文）desktop fmt/clippy 的 CI 形态：新增独立 job（如 `desktop` job）vs 在既有 `rust` job 内追加 step（matrix 适配）——影响 CI 时长与缓存策略。pm 注：裁决时请一并钉死 OS matrix 覆盖——desktop 含平台 cfg 门控代码（Windows 伪隐藏 WS_EX_TOOLWINDOW／mac 托盘），单 OS clippy 不覆盖他平台 cfg 路径；desktop-installers 终验口径为本机 Windows 单面，CI 侧单 OS vs 三 OS 需同裁决定案。
- [ ] ②（intent 原文）desktop clippy 是否含 `--all-targets`（终验口径=含；CI 时长考量）。pm 注：desktop-installers ops-runbook 硬门即 `cargo clippy --all-targets -- -D warnings` 形态；AC2 要求双口径对比实测记录供本裁决。
- [ ] ③（intent 原文）若 desktop 当前 fmt/clippy 有违规：CI 补死后首跑即红——修复归本 intent 还是分批？pm 注：既有证据链指向 clean（2026-10-16 准出回填核收复跑 clean + 其后 b1/b2 两批零触 desktop 源码），开工当日实测（AC2）确认后本问题大概率自动消解为「无违规、首跑绿」。
- [ ] ④（pm 新增）断言脚本载体与文件面张力：intent.md「受影响系统」称 ci.yml 为「唯一改动面」，而 decisions.md A1 拆分预览③含断言脚本扩展（沿五脚本先例）——本 PRD AC3 按后者纳入。请裁决：确认新增 scripts/ci-yaml.test.sh（或既有脚本扩展）属本批文件面（推荐——A1 预览③+「断言不得弱化」先例），或排除（AC3 作废，desktop 执行面无断言守护）。
- [ ] ⑤（pm 新增）当日现状实测执行归属：pm 无执行工具，2026-10-28 当日 desktop fmt/clippy clean 与否无法由 pm 本机复跑（AC2 输入缺口）。请编排者/人类代跑 `cd apps/desktop && cargo fmt --all -- --check` 与 `cd apps/desktop && cargo clippy --all-targets -- -D warnings` 并留痕 decisions.md，或确认按 AC2 口径归 Construction 拆②执行。
