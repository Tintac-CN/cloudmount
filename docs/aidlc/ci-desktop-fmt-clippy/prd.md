# PRD: ci-desktop-fmt-clippy

> 产出者：pm　日期：2026-10-28　来源：intent.md
> 修订：2026-10-29 Gate#1 打回第 1 轮（人类裁决「打回修订，两项推荐采纳」：B1 字面源=`--manifest-path apps/desktop/Cargo.toml` 完整字面；OQ① 形态=独立 `desktop` job + windows-latest 单腿——留痕 decisions.md 2026-10-29 条）。
> 修订 r2（2026-10-29 准出回填）：OQ④ 按人类裁决「脚本入批」关闭（AC3/AC4 条件化措辞收敛）；断言编号引用同步架构 r1 重排（#7/#8→#8/#9、#10→#11、#6→#7）；OQ② 两侧状态注记统一。

## 用户故事

| # | 角色 | 目标 | 价值 | 溯源（intent.md） |
|---|---|---|---|---|
| U1 | CI 消费者（PR/push 门控的评审者与维护者） | ci.yml 新增独立 `desktop` job（gate1 裁决形态），使 `apps/desktop` 的 fmt+clippy 在 CI 真实执行——desktop 侧格式/静态检查违规即 CI 红 | PR/push 门控反映完整终验口径，消除「根级全绿=全部全绿」的覆盖假象（desktop-installers gate2 M3 实证：desktop 内 fmt dirty 394 行未被根级命令拦截） | 「一句话意图」+「问题与期望」（终验口径缺口——audit-roadmap §4.1 缺口 1/2） |
| U2 | desktop 开发者 | desktop 格式/静态检查违规在 PR 阶段即被 CI 拦截，而非终验/发版前才暴露 | 违规早知早修，fmt/clippy 债不再累积到终验（394 行债教训） | 「受影响系统/用户」：「desktop 开发者（违规即早知）」 |
| U3 | 仓库守卫者（后续 workflow 演进者） | 新 desktop 执行面被断言脚本字面守护，既有四 job 零删改 | 后续 workflow 演进不悄悄回退 desktop CI 覆盖、不破坏既有门控（沿五脚本先例；新脚本以「六脚本」硬门口径收录，防死守护） | 「约束」：「沿既有 ci.yml 模式」「既有 job 零删改」+ decisions.md A1 拆分预览③（断言脚本扩展沿五脚本先例） |

## 验收标准（可测）

- **AC1**（U1）——desktop fmt+clippy 执行面字面在位（字面可断言）：
  - Given：ci.yml 现状四 job（`rust`/`rust-mock-features`/`web-ui`/`arm-cross`）；`rust` job 的 Format/Clippy step 为根级命令（`cargo fmt --all -- --check`／`cargo clippy --workspace --all-targets -- -D warnings`），对根 Cargo.toml:13 `exclude = ["apps/desktop"]` 的独立 cargo 项目零效力——desktop fmt/clippy 面在 CI 从不执行。
  - When：本批在 ci.yml 落 desktop 执行面。形态与 runs-on 已按 gate1 裁决钉死（开放问题①）：新增独立 `desktop` job（job id=`desktop`），`runs-on: windows-latest` 单腿（无 matrix）。命令字面唯 `--manifest-path apps/desktop/Cargo.toml` 一源——**三工件同一字面源**（本 PRD AC1 ①②、test-strategy TP-01、architecture 断言 #8/#9 引用同一命令原文；working-directory/cd 形态锚已删除，不得在任何工件复活）。
  - Then（字面可断言子项）：
    - **形态与 runs-on（gate1 裁决落字面）**：ci.yml 存在 job id=`desktop` 的 job；`runs-on: windows-latest` 单腿（无 `strategy.matrix`）。
    1. 存在 desktop fmt 步：命令原文**精确字面** `cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check`——唯 `--manifest-path` 形态，不得以 `working-directory: apps/desktop`／run 内 `cd apps/desktop` 等上下文锚替代或弱化（负断言：desktop job 块内无 `cd apps/desktop`，架构断言 #11）；
    2. 存在 desktop clippy 步：命令原文**精确字面** `cargo clippy --manifest-path apps/desktop/Cargo.toml --all-targets -- -D warnings`（`--all-targets` 含=gate1 B1 裁决字面；开放问题②若收窄须回本 PRD 修订并同步三工件）；
    3. toolchain step 含 `components: clippy, rustfmt`（沿 `rust` job 先例 dtolnay/rust-toolchain@stable）；
    4. 缓存覆盖（intent 缓存约束——desktop 构建依赖重 eframe/wgpu，独立 target 目录）：新增 `desktop` job 含 `Swatinem/rust-cache@v2` step，`workspaces` 参数精确字面 `apps/desktop -> target` 使 `apps/desktop/target` 进入缓存 key（B2 修正口径：target 相对 workspace，不得写成 `apps/desktop -> desktop`；精确断言由 architecture 断言 #7/TP-01 承接）；
    5. 负断言（desktop 违规必须真能红——防「补了步骤但静默绿」空真形态，沿 desktop-installers TP-02 先例口径）：
       - 无 `continue-on-error`（job/step 级均无）；
       - 无绕过性 `if:`：无 `if: false`、无条件收窄（收窄=仅在部分场景执行而可绕过违规）；注：`if: always()` 属强制运行（非跳过），按 gate1 pm-m4 口径不列为 fail-open 形态——fail-open 枚举以 `if: false`／条件收窄为准；
       - 无 status 函数调用作为执行条件：无 `always()`／`success()`／`failure()`／`cancelled()`；
       - 无 shell 级静默绿：无 `|| true`、`; exit 0`、`set +e`。

- **AC2**（U2）——desktop 现状本机实测如实记录（决定 CI 首跑预期）：
  - Given：desktop fmt/clippy 最近留痕验证为 clean——2026-10-16 desktop-installers 准出回填核收复跑「desktop build/clippy/fmt clean」（M3 全量 fmt 394 行@2a02050 清偿后）+ audit-roadmap TP-14 双面复跑 exit 0；其后 b1-doc-corrections（纯文档批）与 b2-115-qualify（文档零触 apps/desktop）均未触 desktop 源码。**现状预期统一为「证据链预期 clean，Construction 当日实测确认」**——不以历史 clean 替代当日实测（retro「覆盖率假象」教训）；pm 无执行工具，实测归 Construction 当日（原开放问题⑤已关闭，事项本身由 AC2 承载）。
  - When：Construction 拆②开工时本机（Git Bash，仓库根）执行 `cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check` 与 `cargo clippy --manifest-path apps/desktop/Cargo.toml --all-targets -- -D warnings`（clippy 侧按 CI 复现口径加 `RUSTFLAGS="-D warnings"`——与 ci.yml workflow env 一致，TP-03 承接；另跑不含 `--all-targets` 口径对比记录，供开放问题②裁决）；命令原文+退出码+（若 dirty）违规明细（文件/行数）留痕于 decisions/units。
  - Then：现状如实记录且 CI 首跑预期随之钉死——clean → CI 补死后首跑绿为预期基线（开放问题③自动消解）；dirty → CI 首跑红为预期（红=门控生效，非本批缺陷）。**dirty 分支准出条件**：dirty 时本批判出（含 ops 回填）须附——①开放问题③裁决记录（decisions.md 留痕）②修复批号或 backlog 条目登记；缺任一项不得判准出。

- **AC3**（U3）——断言守护（沿五脚本先例；新脚本=第六脚本）：
  - Given：现有五断言脚本（scripts/{desktop-installers-yaml,installers-yaml,release-yaml,check-macos-yaml,package-macos}.test.sh）被测对象为 release.yml/check-macos.yml/main.wxs/打包脚本——ci.yml 当前零断言覆盖。
  - When：本批新增独立断言脚本 **`scripts/ci-desktop-yaml.test.sh`**（gate1 同步定名；载体与文件面入批已由 gate1 2026-10-29 人类裁决落定——见开放问题④）守护 AC1 新执行面字面与 AC4 护栏字面，沿五脚本机制先例（job_block 段提取／grep_ok·grep_absent_ok 防空真／CRLF 剥除规范化／actionlint 不可用降级且降级计数诚实）。
  - Then：红阶段（desktop 执行面未实现）desktop 相关断言 FAIL（缺失不空真）；实现后脚本全绿 exit 0（末行 N/N passed）；护栏类断言（AC4 既有四 job 字面锚）现状绿保持绿；脚本自检断言（防空真机制/CRLF 剥除在位）沿 desktop-installers TP-10 先例。「断言不得弱化」操作定义（可核）：①负断言必须用 `grep_absent_ok`（不得以裸 grep/取反或跳过替代）；②正断言不得由精确字面放宽为子串（如 `--manifest-path apps/desktop/Cargo.toml` 完整命令不得退化为 `cargo fmt` 子串匹配）；③脚本自检、CRLF 剥除、降级计数断言必须存在（缺一即视为弱化）。
  - 交付/ops 声明：新脚本纳入本批 ops 沉淀——runbook/checklist 以「**六脚本**」硬门口径收录（写入本批 ops 产物/回填，不修改已完结工件；不收录则新断言无执行载体=死守护）。

- **AC4**（U3）——既有四 job 零删改（diff 边界）：
  - Given：ci.yml 现状四 job 及 workflow 级 env（`CARGO_TERM_COLOR`/`RUSTFLAGS: "-D warnings"`）、`on:` 触发（push main + pull_request）——字面锚集：`rust` job 四步命令原文（`cargo fmt --all -- --check`/`cargo check --workspace`/`cargo clippy --workspace --all-targets -- -D warnings`/`cargo test --workspace`）+ 三 OS matrix + `components: clippy, rustfmt`；`rust-mock-features` 的 `cargo test --workspace --features cloudmount-daemon/mock`；`web-ui` 的 npm 步；`arm-cross` 的 cross 构建步。
  - When：本批改动落盘，diff 审查+断言脚本护栏双通道核验。
  - Then：上述既有字面锚全部在位原样——零删改（本批新增独立 `desktop` job，OQ① 的「rust job 内追加」形态已裁决排除，无追加例外）；本批 diff 仅含 ci.yml 新增面+断言脚本 `scripts/ci-desktop-yaml.test.sh`（gate1 2026-10-29 裁决入批，见 OQ④）——**diff 文件集断言由 TP-02 承载**（`git diff --name-only` 守卫：仅 `.github/workflows/ci.yml` 与 `scripts/ci-desktop-yaml.test.sh`；desktop 源码/release.yml/check-macos.yml/根与 desktop Cargo.toml 零触碰）。

## 非目标（明确排除）

- 不做 desktop 源码修复：desktop fmt/clippy 违规（若开工实测发现）修复归 desktop 代码批——除非开放问题③裁决纳入本批（intent 约束「仅 CI 配置变更」）；dirty 时的批次处置按 AC2「dirty 分支准出条件」。
- 不做 desktop `cargo test` CI 步骤：intent 范围仅 fmt+clippy（desktop 测试 CI 可见性为 audit §4.1 缺口面的另一子项）。**默认排除，选入需先修订本 PRD**（并同步 test-strategy/architecture），不得在 Construction 顺带加入。
- 不做 release.yml 变更：desktop 构建面已在 release.yml 覆盖（desktop-installers 批已落）。
- 不做 check-macos.yml 变更（permissions 显式化=backlog 条目 3 独立批）。
- 不做 actionlint CI 步骤补齐（backlog 条目 1 独立批次；断言脚本内 actionlint 降级机制沿五脚本先例，与 CI 步骤补齐无关）。
- 不改根/desktop Cargo.toml：workspace exclude 为既定架构决策（GUI 依赖图隔离），本批不动。
- 不采用 `working-directory`/`cd` 命令形态：gate1 B1 裁决唯 `--manifest-path apps/desktop/Cargo.toml` 完整字面（三工件同一字面源）。
- 不优化既有四 job 的时长/缓存：既有 job 零删改约束下，缓存/时长优化仅限新增 desktop 面（AC1④）。

## 开放问题（需人类裁决）

- [x] ①（intent 原文）desktop fmt/clippy 的 CI 形态 —— **已裁决（gate1 2026-10-29，decisions.md 留痕）**：新增独立 `desktop` job（job id=`desktop`）+ `runs-on: windows-latest` 单腿；「rust job 内追加」及 ubuntu/3-OS 备选均排除。AC1「形态与 runs-on」子项已落字面，测试侧由 TP-01 同步承接。
- [ ] ②（intent 原文）desktop clippy 是否含 `--all-targets`（终验口径=含；CI 时长考量）。pm 注：gate1 B1 裁决字面已含 `--all-targets`（见 AC1②/AC2 When）；架构侧按含口径设计并在 architecture OQ② 节留「实质收敛、待本项逐字确认」——两侧状态一致。若本项裁决收窄，须回本 PRD 修订并同步 test-strategy/architecture（三工件同一字面源）。
- [ ] ③（intent 原文）若 desktop 当前 fmt/clippy 有违规：CI 补死后首跑即红——修复归本 intent 还是分批？pm 注：证据链指向 clean（2026-10-16 准出回填核收复跑 clean + 其后 b1/b2 两批零触 desktop 源码），预期 clean、Construction 当日实测（AC2）确认后大概率自动消解为「无违规、首跑绿」；若实测 dirty，按 AC2「dirty 分支准出条件」处置（裁决记录+修复批号/backlog 登记）。
- [x] ④（pm 新增）断言脚本载体与文件面张力 —— **已裁决（gate1 2026-10-29 人类逐字「脚本入批」，decisions.md 留痕）**：**断言脚本入批**——新增 `scripts/ci-desktop-yaml.test.sh` 属本批文件面（diff 白名单 = ci.yml + 本脚本）；扩展 desktop-installers-yaml.test.sh（已完结锁定工件）与「排除」（AC3 作废、新执行面无守护）均否决。intent 原文「ci.yml 唯一改动面」由本裁决显式扩面（与 A1 拆分预览③一致）。
- [x] ⑤（pm 新增）当日现状实测执行归属 —— **已关闭（互审 MINOR 9：冗余项）**：归属已由 AC2 When 明确「Construction 拆②开工时执行并留痕」，无需单独裁决。
