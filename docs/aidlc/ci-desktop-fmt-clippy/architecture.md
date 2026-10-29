# Architecture: ci-desktop-fmt-clippy

> 产出者：architect　日期：2026-10-28（gate1 修订 r1：2026-10-29）
> 来源：intent.md + prd.md（AC1..AC4 定稿实文）+ test-strategy.md（TP-01..TP-06 定稿实文）+ reviews/gate1-ai.md @8576a45 + decisions.md 2026-10-29 Gate#1 人类裁决。
> route=classic（decisions.md A1：表单未选 → 协议缺省锁定）。本 artifact = CI 变更蓝图。
> **r1 已落 gate1 裁决字面**：B1 字面源 = `--manifest-path apps/desktop/Cargo.toml` 完整形态（prd/test-strategy 同一字面源）；OQ① 形态 = 独立 `desktop` job + `windows-latest` 单腿。本文件无占位符；语义对齐表与断言清单按 prd AC1..AC4 + TP-01..TP-06 实文逐条回填。

## 逻辑架构

### 现状与缺口

- **触发面/环境**：ci.yml `on:` = push(main) + pull_request；workflow 级 env `CARGO_TERM_COLOR: always` + `RUSTFLAGS: "-D warnings"`（全 job 继承）。
- **既有四 job**（本批零删改对象）：`rust`（fmt/check/clippy/test ×3 OS matrix）、`rust-mock-features`（mock feature 腿 ×3 OS）、`web-ui`（npm typecheck+build，`working-directory: web-ui`）、`arm-cross`（musl 交叉构建 + `--help` smoke）。job 间无 needs，全并行。
- **缺口机理**：`apps/desktop` 为根 Cargo.toml `exclude`（:13）的独立 cargo 项目——`rust` job 的 `cargo fmt --all`／`cargo clippy --workspace` 只覆盖 workspace members（7 crates），对 desktop **零效力**（desktop-installers gate2 互审 M3 实证：desktop fmt dirty 394 行未被根级命令拦截）。desktop 的 fmt/clippy 面**在 CI 从不执行**——ops-runbook 硬门只能靠人工清单兜底，无流水线上游拦截。
- **全 workflow 前提已核实**（gate1 arch-m8 关闭）：`check-macos.yml` L22/L24 仅 `cargo check --target ... -p cloudmount-daemon -p cloudmount-cli`；`release.yml` 无 fmt/clippy 步——「desktop fmt/clippy 在 CI 从不执行」在全部 workflow 成立。
- **改动面**：`.github/workflows/ci.yml` 新增 `desktop` job（唯一产品面改动）+ 新断言脚本 `scripts/ci-desktop-yaml.test.sh`（守护面，gate1 后落定）。两文件之外零触碰；desktop 源码、release.yml、check-macos.yml、根/desktop Cargo.toml 零触碰（AC4 diff 面）。
- **覆盖增量（非「闭环」——显式接受缺口）**：rust job（workspace 7 crates ×3 OS）+ desktop job（exclude 项目 own-code ×1 OS）。明确接受：`main.rs:745` 的 `#[cfg(not(windows))]` 分支在 windows 腿不参与编译与 clippy；mac 特定行为位于 tray-icon 依赖内部（非本地代码，不进 own-code lint 面）。单 OS 腿是「与终验口径同 OS 族」的定向补死，不宣称全平台闭环（gate1 arch-m2 修订）。
- **依赖面**：desktop job 依赖 = `apps/desktop/Cargo.toml` + `apps/desktop/Cargo.lock`（独立锁文件，**已确认在仓**——CI 依赖钉死与根锁同纪律）+ crates.io 官方源（ci.yml env 注释口径：rsproxy 仅本地）。三个 action（checkout@v4 / dtolnay/rust-toolchain@stable / Swatinem/rust-cache@v2）均为 ci.yml 既有 in-use 面——**零新增第三方 action**。
- **brownfield 一致性**：job 命名（`Desktop（fmt + clippy）`）、step 命名（`Format`/`Clippy`）、2 空格缩进、注释密度均镜像既有四 job；新 job 插入 `rust-mock-features` 之后、`web-ui` 之前（rust 族相邻分组），沿 `rust-mock-features` 并列 job 先例。该一致性由断言 #2 / #19 等字面锁定（gate1 pm-m2）。

### desktop CI job 设计（核心）

#### 形态裁决（gate1 OQ① 已落定：A = 新增独立 `desktop` job）

| 维度 | A. 新增独立 `desktop` job（**裁决采纳**） | B. `rust` job 内追加 step（否决） |
|---|---|---|
| CI 时长 | 与 rust job **并行**——desktop 依赖树（eframe/wgpu，数百 crate）编译时长不叠加进 rust job 关键路径 | 追加 step 在**三条 OS matrix 腿各串行执行一遍**——每腿冷编 desktop 依赖树，rust job 三腿全体系性变慢 |
| 缓存策略 | 自带 `workspaces` 参数为 `apps/desktop/target` 立独立 cache entry，**不动既有 rust-cache step** | rust job 的 rust-cache key 只含根 workspace——desktop 部分要么每跑冷编，要么**改既有 rust-cache step**——与「既有 job 零删改」直接相抵 |
| 失败归因 | job 名即失败面（desktop fmt/clippy 红，PR 页一眼可辨） | desktop 红连坐 rust job 三腿显示，需展开看 step |
| 约束合规 | 四既有 job 字面零触碰（最保守解读下合规） | 追加 step 字面允许，但缓存代价与「零删改」张力使方案劣化 |
| 仓内先例 | `rust-mock-features` 即「并列 job 承载独立检查腿」先例 | release.yml 的 desktop 步追加进 installer-msi-windows 是因 WiX 归集**次序约束**——CI lint 无此可比性 |

**裁决 A 的核心理由**：desktop 是依赖最重的编译单元；独立 job 把成本从「串行 ×3 腿」变为「并行 ×1 腿」，且缓存覆盖与零删改约束只有 A 同时成立。

#### runs-on 与 matrix（gate1 已落定：`windows-latest` 单腿）

- **fmt**：rustfmt 输出平台无关（仓内无 rustfmt.toml/.rustfmt.toml——已确认，本地/CI/任意 OS 格式行为零分歧）——单次执行即足。rust job 三腿各跑一遍 fmt 属既有形态（冗余但不评不改）；新 job 不复制该冗余。
- **clippy**：desktop own-code 平台门控面实证——`#[cfg(windows)]` ×3（main.rs:28/682/710，伪隐藏/HWND 面）+ `#[cfg(not(windows))]` ×1（main.rs:745）+ `#[cfg(test)]` ×2；**无 `cfg(target_os="macos")`/`cfg(unix)` 门控**（mac 特定行为在 tray-icon 依赖内，非本地代码）。
- **`runs-on: windows-latest`**（裁决字面）：
  1. 覆盖最大平台特定块（cfg(windows) 三处——desktop 平台特定逻辑的主体）；
  2. **对齐终验口径执行环境**——ops-runbook 硬门/release-checklist 在 Windows 本机（Git Bash）执行，M3 实证亦为 Windows 平台：CI 绿红与终验绿红同 OS 族，正是「上游补死」的语义（若选 ubuntu，cfg(windows) 块内 clippy 违规将出现「终验红而 CI 绿」——缺口原样复发）；
  3. 免 3 倍成本：3-OS matrix 相对单 windows 腿仅多覆盖 cfg(not(windows)) 一处小分支（该分支为已接受的覆盖缺口，见上）。
- Windows runner 较慢的代价由 rust-cache 缓解（warm 跑为增量）；首跑冷编一次属一次性成本，由 `timeout-minutes` 兜底。

#### 完整 YAML 蓝图（可直接落笔）

插入位置：`rust-mock-features` job 块之后、`web-ui` 之前（rust 族 job 相邻分组）。命令形态 = `--manifest-path`（**gate1 B1 裁决字面**；仓内先例：desktop-installers-yaml.test.sh L26「desktop 构建形态 = --manifest-path apps/desktop/Cargo.toml（cd 形态不采）」+ release.yml 两处 desktop 步同形态；产物语义一致落 `apps/desktop/target`）。

```yaml
  # apps/desktop 为 workspace exclude 独立 cargo 项目（根 Cargo.toml:13）——
  # rust job 的 fmt --all / clippy --workspace 只覆盖 workspace members，对
  # desktop 零效力（desktop-installers gate2 互审实证：desktop fmt dirty 394 行
  # 未被根级命令拦截）。本 job 补死该缺口：命令与 ops-runbook 终验硬门同口径，
  # 构建形态沿 release.yml desktop 步先例（--manifest-path，产物落 apps/desktop/target）。
  # gate1 裁决：独立 job + windows-latest 单腿；fmt/clippy 字面 = B1 裁决的
  # --manifest-path 完整形态（与 prd/test-strategy 同一字面源）。
  desktop:
    name: Desktop（fmt + clippy）
    runs-on: windows-latest
    # 冷编上限护栏：防依赖树/缓存异常导致 runner 长时间占用；首跑实测后按
    # 约 2x 余量调整（调整须 decisions 留痕）。
    timeout-minutes: 60
    # 最小权限：checkout 只需 contents 读；cache 走 runtime token，不受 GITHUB_TOKEN 权限面约束。
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      # desktop 独立 target 目录（apps/desktop/target）不在 rust job 的 cache key 覆盖内。
      # Swatinem/rust-cache@v2 语义 = $workspace -> $target，target 相对 workspace 解析：
      # `apps/desktop -> target` 即 apps/desktop/target；workspaces 一旦给出，v2 只缓存
      # 列出的 workspace（不连带根 workspace），key 含 desktop Cargo.lock hash（依赖变更自动失效）。
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: apps/desktop -> target
      - name: Format
        run: cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check
      - name: Clippy
        run: cargo clippy --manifest-path apps/desktop/Cargo.toml --all-targets -- -D warnings
```

蓝图要点：

- **step 名** `Format`/`Clippy` 镜像 rust job 命名；fmt 先行（廉价快失败），序同 rust job。
- **clippy 含 `--all-targets`**：B1 裁决字面已含（= 终验口径/runbook 硬门口径）；连带编译 desktop 全部 4 个 `#[test]`（main.rs `t28_policy_tests` ×2 + `t30_structural` ×2）——lint 测试代码正是 M3 类盲区高发面。时长顾虑由 cache 缓解。
- **RUSTFLAGS 继承**：workflow 级 `-D warnings` 自动生效（与 rust job 对等；registry 依赖经 cargo cap-lints 不受累）——无需 job 内覆写。
- **cache 值语义（B2 修正）**：`workspaces: apps/desktop -> target`。v2 的 `$workspace -> $target` 中 **target 相对 workspace 解析**（`path.join(workspace_root, target)`）——`apps/desktop -> target` = `apps/desktop/target`；写成 `apps/desktop -> desktop` 会解析为不存在的 `apps/desktop/desktop`，desktop target 永不入缓存（每次 PR 冷编数百 crate）。另：**`workspaces` 一旦给出，v2 只缓存列出的 workspace，不连带根 workspace**（r0 的「连带恢复根 workspace」叙述错误，已删）。key 含 job id + OS + rustc 版本 + 各列出 workspace 的 Cargo.lock hash（desktop 独立锁变更自动失效）。
- **fmt `--all` 语义边界**：作用于 manifest 所属 workspace 的全部 package（desktop 自身是单包独立 workspace）；`--manifest-path` 隔断根 workspace——**不会**连带根 crate（修正 r0 归因偏差）。
- **无 needs 接线**：ci.yml 无 job 间依赖，新 job 自然并行。
- **checkout 默认浅克隆即足**：desktop Cargo.lock 在树内，无需 submodules/fetch-depth 调整。
- **permissions**：job 级 `contents: read`（新 job 最小权限；四既有 job 权限面零触碰，其显式 permissions 归 audit 承接批）。
- **timeout-minutes: 60**：冷编上限初值；首跑实测后按约 2x 余量收紧/放宽（值调整不改断言形态，见断言 #16）。

### 现状确认（AC2 / TP-03 的架构侧口径）

**统一口径：证据链预期 clean；以 Construction 当日实测为准（不许以历史 clean 替代当日实测）。**

证据链（时间线按提交顺序修正，gate1 M4）：

1. 2026-10-16 11:21 `533e456`（U2-tray：main.rs 托盘接线）——**早于** fmt 清偿，不是最后一次 desktop 触碰（r0 时间线错误，已修正）。
2. 2026-10-16 15:40 `2a02050`（desktop-installers 准出回填：M1 伪隐藏守卫 + M3 全量 fmt 394 行清偿）——**最后一次 desktop 触碰**；此后 desktop fmt `--check` clean。
3. 2026-10-16 desktop-installers 终验复跑：desktop build/clippy/fmt clean；audit-roadmap TP-14 双面复跑 exit 0。
4. 其后 `b1-doc-corrections`（纯文档批）、`b2-115-qualify`（文档零触 apps/desktop）两批均未触 desktop 源码。
5. 2026-10-28 Gate#1 dev 互审实测：`cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check` → exit 0（fmt clean，可引为前期证据；clippy 未在互审实测）。

**预期**：clean → CI 补死后首跑绿为基线；dirty → 首跑红为预期（红 = 门控生效，非本批缺陷），修复范围归 OQ③ 裁决。基于证据链的预期为 clean，但以开工当日实测为准（retro「覆盖率假象」教训）。

**Construction 当日实测命令原文**（AC2 When；Git Bash，repo 根）：

```bash
# fmt（不经 rustc，RUSTFLAGS 无关）：
cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check

# clippy——CI 复现口径：ci.yml workflow env 含 RUSTFLAGS="-D warnings"，
# 本机不带此 env 会漏报 rustc 级 warning，须显式带上才与 CI 首跑行为一致：
RUSTFLAGS="-D warnings" cargo clippy --manifest-path apps/desktop/Cargo.toml --all-targets -- -D warnings

# 等价终验口径原样形态（apps/desktop 内执行，与 ops-runbook 硬门字面一致；
# 另跑不含 --all-targets 口径对比记录，供 OQ② 留痕）：
cd apps/desktop && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings
```

- **留痕**：命令原文 + 退出码 +（若 dirty）违规明细（文件/行数）写 decisions/units（AC2 Then）。
- **工具链漂移提示**：CI 用 `dtolnay/rust-toolchain@stable` 浮动版本；若首跑红源于新版 rustc/clippy lint 或 rustfmt 规则变化（而非 2a02050 后引入的违规），按「工具链漂移」单独报告，不与现状基线混判；必要时另开工具链钉版批（本批不扩）。
- **旁注**：desktop 有 4 个单测，main.rs:961-963 注释自认「desktop 被 workspace exclude，CI 不可见」——`cargo test` 面缺口属相邻缺口，本批默认排除（决策 11）。

### 断言守护设计

**落点：新独立脚本 `scripts/ci-desktop-yaml.test.sh`（gate1 后落定，不再留载体裁决）。**

| 选项 | 裁决 | 为什么 |
|---|---|---|
| 扩展 `desktop-installers-yaml.test.sh` | 否决 | 该脚本为 gate2 锁定**已完结工件**，112/112 断言计数写进已完结 ops-runbook 硬门（「五脚本」口径）——扩展即改计数即碰已完结口径；其被测对象域 = release.yml 交付面（wxs/dmg/icons），ci.yml 不在其契约内 |
| 新独立脚本 | **采纳** | 沿五脚本先例与命名模式；契约来源 = 本批 prd AC1..AC4 + test-strategy TP-01..06 + 本架构断言清单；计数由本批 runbook 侧以「六脚本」口径收录 |

**脚本头注必须包含**（可直接落笔的契约文字）：

- 被测对象：`.github/workflows/ci.yml`（唯一被测文件；其「无既有 desktop lint 面」已由 check-macos/release 核实）。
- 契约来源：prd AC1..AC4 + test-strategy TP-01..06 + 本架构断言清单（含 B1/OQ① 裁决字面）。
- 机制复制声明（复制自 desktop-installers-yaml.test.sh ← release-yaml.test.sh）；同步义务沿先例注记。
- **「断言不得弱化」操作定义**（gate1 pm-m5）：已锁定断言不得删除；不得改宽判据（子串命中替代整行精确、包含判断替代精确值、删除负断言、缩小 job_block 作用域）；断言计数不得下降；新增断言允许。任何弱化须 decisions.md 留痕并经人类批准。
- TDD 红阶段预期：desktop 断言（#1-#16）全 FAIL（job 不存在；负断言因空块直接 FAIL——`grep_absent_ok` 空块即失败，防空真）；护栏类（#17-#41）现状绿属正常；机制项（#42-#44）按先例降级口径计。
- Construction 落字面范围：job id/runs-on/命令/缓存值/脚本名/权限/超时均已钉死；Construction 只落断言计数与实现，不再自行选形态。

机制复制清单（复制自 desktop-installers-yaml.test.sh，其自身复制自 release-yaml.test.sh）：

- `set -u` + `LC_ALL=C` + SCRIPT_DIR 定位（任意工作目录可跑，Git Bash 直跑）；
- CRLF 剥除规范化副本（ci.yml 为 CRLF 检出，断言前 `tr -d '\r'`）；
- `job_block`/`toplevel_block` awk 段提取（ci.yml job 同为 2 空格缩进，函数可直接复用；2 空格注释行会正确终止上一 job 块）；
- `grep_ok`/`grep_absent_ok` 防空真（空块不计命中；负断言空块直接 FAIL）；
- actionlint 降级（先例 L232-283）：可用 → 正断言（ci.yml 整文件零错）+ 负断言（注入语法错误副本必须被拦截——拦截手段自校验）；不可用 → `SKIP actionlint` + python yaml.safe_load 键配平粗检 + `WARN 降级告警`，降级分支计入总数 + 脚本自检断言；
- 输出：逐断言 `PASS/FAIL <名称>` + 末行 `<N>/<M> passed`；任一 FAIL 非零退出。

#### 断言清单（r1 重排；每条 = 一次 `report` 调用，M 基线 44，只增不减）

r1 编号变更说明：旧 #6（宽松 grep）→ 新 #7（精确值）；旧 #12（三命令、漏 `--`）→ 新 #21-#24（四命令、`--` 齐）；旧 #9/#10 负断言 → 新 #10-#14（枚举补全）；旧 #11/#13/#14 护栏 → 新 #17-#41（按 AC4 锚集展开）。

| # | 断言（可执行判据） | 类型 | 锚定 AC / TP |
|---|---|---|---|
| 1 | `desktop` job 存在：`job_block desktop` 非空（2 空格顶级键） | 正 | AC1 / TP-01 |
| 2 | `name: Desktop（fmt + clippy）` 整行字面（brownfield 命名风格镜像既有 job） | 正 | AC1 / TP-01 |
| 3 | `runs-on: windows-latest` 整行字面（OQ① 裁决） | 正 | AC1 / TP-01 |
| 4 | `dtolnay/rust-toolchain@stable` 在位 | 正 | AC1③ / TP-01 |
| 5 | `components: clippy, rustfmt` 整行字面（两组件齐——缺一即步骤不可执行） | 正 | AC1③ / TP-01 |
| 6 | `Swatinem/rust-cache@v2` 在位 | 正 | AC1④ / TP-01 |
| 7 | desktop job 内 `workspaces:` 行**精确等于** `apps/desktop -> target`（仅允许行首缩进；token 序列必须为 `apps/desktop`、`->`、`target`；禁止 `-> desktop`、禁止仅 grep `apps/desktop` 的宽松判据、禁止多 workspace 列表） | 正 | AC1④ / TP-01 |
| 8 | fmt 命令整行字面：`cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check`（B1 裁决） | 正 | AC1① / TP-01 |
| 9 | clippy 命令整行字面：`cargo clippy --manifest-path apps/desktop/Cargo.toml --all-targets -- -D warnings`（B1 裁决） | 正 | AC1② / TP-01 |
| 10 | desktop job 块内无 `continue-on-error`（任何缩进） | 负 | AC1⑤ / TP-01 |
| 11 | desktop job 块内无 `cd apps/desktop`（形态锁定 --manifest-path，沿 desktop-installers L26 先例） | 负 | AC1① / TP-01 |
| 12 | desktop job 块内无任何 `if:` 行（`^[[:space:]]*if:`，含 `if: false`、`if: always()`） | 负 | AC1⑤ / TP-01 |
| 13 | desktop job 块内无 status 函数调用（ERE `\b(always\|success\|failure\|cancelled)\(\)`——沿 desktop-installers-yaml.test.sh:370-372 先例） | 负 | AC1⑤ / TP-01 |
| 14 | desktop job 块内无 shell 级静默绿：`\|\|[[:space:]]*true`、`;[[:space:]]*exit[[:space:]]+0`、`set[[:space:]]+\+e`、`if:[[:space:]]*false`（只作用于 desktop job 块——web-ui 的 `npm ci \|\| npm install` 是既有合法降级，不得误伤） | 负 | AC1⑤ / TP-01 |
| 15 | desktop job 块含 `permissions:` + `contents: read`（最小权限；architect 增补，非 AC 锚） | 正 | 安全 / 最小权限 |
| 16 | desktop job 块含 `timeout-minutes:` 且为正整数（初值 60；值调整不改断言形态） | 正 | M6 / 风险护栏 |
| 17 | `rust` job 块非空 | 护栏 | AC4 / TP-02 |
| 18 | `rust-mock-features` job 块非空 | 护栏 | AC4 / TP-02 |
| 19 | `web-ui` job 块非空 | 护栏 | AC4 / TP-02 |
| 20 | `arm-cross` job 块非空 | 护栏 | AC4 / TP-02 |
| 21 | rust 命令整行字面：`cargo fmt --all -- --check`（gate1 M1：修正 r0 旧断言 #12 的 `--` 遗漏，两个 `--` 齐） | 护栏 | AC4 / TP-02 |
| 22 | rust 命令整行字面：`cargo check --workspace` | 护栏 | AC4 / TP-02 |
| 23 | rust 命令整行字面：`cargo clippy --workspace --all-targets -- -D warnings` | 护栏 | AC4 / TP-02 |
| 24 | rust 命令整行字面：`cargo test --workspace` | 护栏 | AC4 / TP-02 |
| 25 | rust job `components: clippy, rustfmt` 字面 | 护栏 | AC4 / TP-02 |
| 26 | rust job `dtolnay/rust-toolchain@stable` 字面 | 护栏 | AC4 / TP-02 |
| 27 | rust job `runs-on: ${{ matrix.os }}` 字面 | 护栏 | AC4 / TP-02 |
| 28 | rust job `os: [ubuntu-latest, windows-latest, macos-latest]` 整行字面（三 OS matrix 不得收窄） | 护栏 | AC4 / TP-02 |
| 29 | rust-mock-features job `runs-on: ${{ matrix.os }}` 字面 | 护栏 | AC4 / TP-02 |
| 30 | rust-mock-features job `os: [ubuntu-latest, windows-latest, macos-latest]` 整行字面（第二处） | 护栏 | AC4 / TP-02 |
| 31 | rust-mock-features 命令整行字面：`cargo test --workspace --features cloudmount-daemon/mock` | 护栏 | AC4 / TP-02 |
| 32 | web-ui `working-directory: web-ui` 字面 | 护栏 | AC4 / TP-02 |
| 33 | web-ui `npm ci \|\| npm install` 字面 | 护栏 | AC4 / TP-02 |
| 34 | web-ui `npm run build` 字面 | 护栏 | AC4 / TP-02 |
| 35 | arm-cross `cross build -p cloudmount-daemon -p cloudmount-cli` 字面 | 护栏 | AC4 / TP-02 |
| 36 | arm-cross `cloudmountd --help` smoke 字面 | 护栏 | AC4 / TP-02 |
| 37 | arm-cross `cloudmount --help` smoke 字面 | 护栏 | AC4 / TP-02 |
| 38 | workflow 0 缩进 `env:` 块含 `CARGO_TERM_COLOR: always` 字面 | 护栏 | AC4 / TP-02 |
| 39 | workflow 0 缩进 `env:` 块含 `RUSTFLAGS: "-D warnings"` 字面（引号原样） | 护栏 | AC4 / TP-02 |
| 40 | `on:` 块 `push:` 下 `branches: [main]` 字面 | 护栏 | AC4 / TP-02 |
| 41 | `on:` 块含 `pull_request:` 字面 | 护栏 | AC4 / TP-02 |
| 42 | actionlint 正/负/降级/自校验（复制先例四条形态；降级 = SKIP + safe_load 粗检 + WARN，计数诚实） | 机制 | AC3 / TP-05 |
| 43 | 脚本自检：防空真机制在位（空块/空文件不得计通过） | 机制 | AC3 / TP-04 |
| 44 | 脚本自检：CRLF 剥除规范化副本在位（对 CRLF 副本断言可命中） | 机制 | AC3 / TP-04 |

**diff 面守卫（AC4 Then 第二句；非脚本断言）**：预期 `git diff --name-only <base>..HEAD` 产品面文件集 = `.github/workflows/ci.yml` + `scripts/ci-desktop-yaml.test.sh`（docs/aidlc 工件不属产品面）。`apps/desktop/**`、`.github/workflows/release.yml`、`.github/workflows/check-macos.yml`、根/desktop `Cargo.toml` 必须零触碰；由 Construction/QA 在准出核验执行（静态脚本无 base 概念，不承载此断言；对齐 test-strategy TP-02）。

**运行载体注记**：新脚本进本批 gate 自验；desktop-installers `release-checklist.md` ② 的「五脚本循环」为已完结工件**不改**——今后 runbook 硬门以「六脚本」口径收录（本批 ops/retro 侧沉淀，非本 artifact 动已完结清单）。

## 数据模型

CI 配置批——无数据库/持久存储；键语义如下：

| 实体 | 键 | 约束/索引 |
|---|---|---|
| ci.yml job | job id（2 空格缩进顶级键） | 新增 `desktop`；四既有 id（rust/rust-mock-features/web-ui/arm-cross）由护栏断言锁定不可删改 |
| cache entry（desktop） | key = prefix/shared + job id `desktop` + OS `windows` + rustc 版本 + `apps/desktop/Cargo.lock` hash + env-vars hash；路径 = `apps/desktop/target`（`workspaces` 精确值） | 锁文件变更自动失效；**总量计入仓库 10GB 配额**（见安全节体积评估） |
| desktop 构建图 | `apps/desktop/Cargo.lock`（独立锁，与根锁零共享） | CI 依赖钉死（供应链约束） |
| 断言脚本 | `scripts/ci-desktop-yaml.test.sh` | 末行 `<N>/<M> passed`（M 基线 44，只增不减）；任一 FAIL 非零退出 |
| desktop 产物目录 | `apps/desktop/target` | `--manifest-path` 语义下产物落此，不入根 target（release.yml 先例同语义） |
| 本批 diff 面 | `.github/workflows/ci.yml` + `scripts/ci-desktop-yaml.test.sh` | 其余产品文件零触碰（AC4） |

## 与 PRD 的语义对齐表（按 AC1..AC4 + TP-01..06 定稿实文回填）

| PRD 锚（实文） | PRD 动词 / 条目 | 物理语义 | 本架构落点 | 守护断言 / TP |
|---|---|---|---|---|
| AC1 Then① | 「补」desktop fmt 步 | ci.yml **新增** `desktop` job 的 `Format` step；命令 = B1 裁决字面 `cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check`；不采 cd/working-directory 形态 | YAML 蓝图 Format step | #8 / #11 / TP-01 |
| AC1 Then② | 「补」desktop clippy 步 | 同一 job 的 `Clippy` step；命令 = `cargo clippy --manifest-path apps/desktop/Cargo.toml --all-targets -- -D warnings`（`-D warnings` 必含；`--all-targets` 由 B1 字面锁定） | YAML 蓝图 Clippy step | #9 / TP-01 |
| AC1 Then③ | 「含」toolchain components | `dtolnay/rust-toolchain@stable` + `components: clippy, rustfmt`（两组件齐） | YAML 蓝图 toolchain step | #4 / #5 / TP-01 |
| AC1 Then④ | 「覆盖」缓存（desktop 独立 target 需 cache key） | `Swatinem/rust-cache@v2` 在位 + `workspaces: apps/desktop -> target`（独立 entry；target 相对 workspace；key 含 desktop Cargo.lock hash） | YAML 蓝图 cache step + 数据模型 cache entry | #6 / #7 / TP-01 |
| AC1 Then⑤ | 「必须真能红」（负断言） | 无 `continue-on-error`；无任何 `if:`（含 `if: false`）；无 status 函数调用；无 shell 级静默绿（`\|\| true`/`; exit 0`/`set +e`）——违规 → step 非零退出 → job 红 → PR/push 门控 | 负断言组 | #10-#14 / TP-01 负子项 |
| AC1 When（形态/runs-on 裁决） | 「按开放问题①裁决落字面」 | 独立 `desktop` job；`runs-on: windows-latest` 单腿 | YAML 蓝图 job 头 | #1 / #2 / #3 / TP-01 / TP-06 |
| AC2 Then | 「如实记录」现状 | Construction 当日本机实测（AC2 When 命令原文，clippy 带 RUSTFLAGS）；clean → 首跑绿基线；dirty → 首跑红=门控生效（修复归 OQ③）；不许以历史 clean 替代当日实测 | 现状确认节（证据链预期 clean） | TP-03（实测留痕，非静态断言） |
| AC3 Then | 「守护」断言 | 新脚本：字面断言 + 零删改护栏 + 负断言 + actionlint 降级；断言不得弱化（操作定义见脚本头注）；红阶段 desktop 断言 FAIL、实现后全绿 | 断言守护设计 + #42-#44 | TP-01/02/04/05 |
| AC4 Then（锚集） | 「零删改」既有四 job | 四 job id + rust 四命令 + rust components/toolchain + 两处三 OS matrix + rust-mock 命令 + web-ui working-directory/npm 两命令 + arm-cross cross 命令/双 smoke + env 双键 + on: 触发面全部原样 | 护栏断言组 | #17-#41 / TP-02 |
| AC4 Then（diff 面） | 「仅含」新增面 | 产品面 diff = ci.yml + 断言脚本；desktop 源码/release.yml/check-macos.yml/两个 Cargo.toml 零触碰 | diff 面守卫（准出核验执行） | TP-02 |
| 非目标① | 「不做」desktop 源码修复 | dirty 修复不混入本批；CI 步骤照落，修复归紧随 desktop 代码批 | 决策 3 / OQ③ | TP-03 |
| 非目标② | 「不做」desktop `cargo test` CI 步骤 | **默认排除；选入需 PRD 修订**（不是本批可用裁决开启的开关） | 决策 11 | 无（范围外） |
| 非目标③ | 「不优化」既有四 job | 新增 desktop 缓存不得挤占/劣化既有三 OS 缓存（10GB 配额评估 + 首跑实测） | 安全节缓存体积评估 | M6 follow-up |

## 关键决策与理由

| # | 决策 | 为什么 |
|---|---|---|
| 1 | 新增独立 `desktop` job，不在 rust job 内追加 step | gate1 OQ① 裁决落定。时长：desktop 依赖树最重（eframe/wgpu 数百 crate），独立 job 与 rust job 并行、不进其关键路径；追加则三 OS 腿各串行冷编。缓存：独立 job 自带 workspaces 参数即可覆盖 desktop target，追加方案须改既有 rust-cache step（违「零删改」）。归因：job 名即失败面。先例：rust-mock-features 并列 job |
| 2 | `runs-on: windows-latest` 单腿 | gate1 OQ① 裁决落定。fmt 平台无关单次即足；clippy 门控面实证 = cfg(windows)×3 + cfg(not(windows))×1、无 mac 门控——windows 腿覆盖最大平台特定块且与终验口径执行环境同 OS 族：CI 绿红对齐终验绿红即「补死」语义；ubuntu 会造「终验红 CI 绿」缺口复发。显式接受 cfg(not(windows)) 分支不覆盖（不再宣称闭环） |
| 3 | 若实测 dirty：CI 步骤照落 + 修复归紧随的 desktop 代码批 | 补死是本批存在理由——带病也落（否则缺口永续）；「仅 CI 配置变更」约束禁止顺手改 desktop 源码；main 红窗由紧随批收敛（备选：人类特批纯机械 `cargo fmt` 修复并入——OQ③）。以 Construction 实测触发 |
| 4 | clippy 含 `--all-targets` | B1 裁决字面已含（= 终验口径/runbook 硬门）；连带 lint 4 个测试目标——测试代码是 M3 类盲区高发面；时长由 cache 缓解 |
| 5 | 命令形态 `--manifest-path`（否决 cd/working-directory） | gate1 B1 裁决字面（三工件同一字面源）；仓内先例：desktop-installers-yaml.test.sh L26 明文「cd 形态不采」+ release.yml 两处 desktop 步同形态；产物语义一致（落 apps/desktop/target）。注：`web-ui` 用 `working-directory` 是 npm 步的既有惯例，与 cargo lint 上下文不可比（修正 r0「仓内已锁定先例」理由不全） |
| 6 | rust-cache `workspaces: apps/desktop -> target` | v2 语义 = `$workspace -> $target`，target 相对 workspace（`path.join(root, target)`）——`-> target` 才是 `apps/desktop/target`；`-> desktop` 会指向不存在的 `apps/desktop/desktop`（缓存永久 miss）。`workspaces` 给出后只缓存列出项，**不连带根 workspace**（r0 叙述错误已删）。key 含 desktop 独立锁 hash，依赖变更自动失效 |
| 7 | 新 job 级 `permissions: contents: read` | 最小权限默认：checkout 只需 contents 读，cache 走 runtime token 不受 GITHUB_TOKEN 权限面约束；只约束新 job，四既有 job 权限面零触碰（其无 permissions 块的现状归 audit 承接批统一处理）——不扩权、不改既有风格 |
| 8 | 新独立断言脚本（否决扩展 desktop-installers-yaml.test.sh） | 目标脚本为 gate2 锁定已完结工件，112/112 计数写进已完结 runbook 硬门——扩展即改计数即碰已完结口径；其被测对象域亦不含 ci.yml |
| 9 | 脚本名 `scripts/ci-desktop-yaml.test.sh` | gate1 后落定（不再留裁决占位）。沿 `<域>-yaml.test.sh` 命名先例；域 = ci.yml 的 desktop 面；不取 `ci-yaml.test.sh`（全域名过宽——本批契约只覆盖 desktop job + 护栏，未来 ci 批可扩展） |
| 10 | fmt 单次执行，不复制 rust job 三腿冗余 | rustfmt 输出平台无关（仓内无 rustfmt 配置——已确认零分歧源）；rust job 既有冗余不评不改，新 job 不复制 |
| 11 | desktop `cargo test` 不进本 job（**默认排除**） | intent/PRD 非目标明确：范围仅 fmt+clippy；选入需 PRD 修订（新增 AC），不属本批可自行扩张项。相邻缺口（main.rs:961-963 自认 CI 不可见）留待另批 |
| 12 | 断言含完整零删改护栏 + 负断言枚举 | 「既有 job 零删改」「违规必须真能红」从约束文字落为可执行断言：四 job id/rust 四命令/两处三 OS matrix/components/另三 job 命令/env 双键/on: + continue-on-error/if:/status 函数/shell 静默绿——锚集按 AC4 实文逐条对齐（gate1 M1/M2） |
| 13 | `timeout-minutes: 60` + 首跑缓存体积实测 | 防冷编/缓存异常长时间占用 runner；desktop target 本机 5.0GB vs 仓库 10GB 缓存配额——首跑后量 entry 体积/命中率/既有 job 时长，超限再按安全节备选处置（gate1 M6） |
| 14 | 无 needs 接线 | ci.yml 无 job 间依赖先例，新 job 自然并行；不引入跨 job 门控（与既有结构一致） |

## 安全与权限

- **权限面**：新 `desktop` job 显式 `permissions: contents: read`（最小权限：checkout 读；cache 用 runtime token）。ci.yml 四既有 job 无 permissions 块——本批不触碰（显式收紧属权限面变更，归 audit 承接批统一处理）；本批不扩权。
- **供应链**：零新增第三方 action（checkout/toolchain/rust-cache 均既有 in-use 面）；desktop 独立 `Cargo.lock` 在仓 = CI 构建依赖钉死；CI 走 crates.io 官方源（rsproxy 仅本地——ci.yml env 注释口径）。**工具链浮动**：`@stable` 随 upstream 漂移，首跑红须区分「工具链漂移」与「现状 dirty」。
- **密钥管理**：无新增密钥；无新增网络端点。
- **cache 面内容**：cache entry 仅含编译产物/注册表缓存（`apps/desktop/target` + ~/.cargo），无敏感物；key 随 Cargo.lock 自动失效，无手动 key 维护面。

### 缓存体积与 10GB 配额风险（gate1 M6）

- **体量事实**：本机 `apps/desktop/target` ≈ **5.0GB**（其中 debug ≈ **3.9GB**）——该值是累积全产物（release+debug 二进制等）；CI 只跑 clippy（check/metadata 产物 + proc-macro + build script 输出），预计显著小于本机累积值，但必须实测，不得以本机值直接外推。
- **配额约束**：GitHub Actions 每仓缓存上限 **10GB**，超限按 LRU 淘汰旧 entry。desktop 新增 entry 可能挤掉既有三 OS 的根 workspace 缓存 → 既有 job 变慢，违反 PRD 非目标「不优化既有 job」的**不劣化**意图。
- **控制措施（按序）**：
  1. 新 job 加 `timeout-minutes`（初值 60）——防失控跑占用；
  2. **首跑后实测**：Actions 缓存页的 entry 体积、desktop hit-rate、既有四 job 时长对比（本批前后）；
  3. 若出现挤占/抖动：优先 `shared-key: desktop` 做键命名空间隔离（避免键耦合；**不增加配额**，只防键冲突）；其次 `save-if` 限制仅 main 保存（PR 只恢复，降低 entry churn）；最后备选 `cache-targets: false`（仅注册表缓存，明确记录为降级并保留回滚条件）。v2 各选项可用性 Construction 核实后落定，变更须 decisions 留痕。
- **失败模式**：缓存淘汰/保存失败不影响正确性，仅影响时长；job 红绿由 fmt/clippy 命令退出码决定。

### 其他风险与外部依赖

- **branch protection（仓库外设置）**：新 `desktop` job 未加入 required checks 前不阻塞合并——属仓库设置面，本批不改；ops 侧记录为后续项（gate1 dev-MINOR18）。
- **Windows runner 时长**：首跑冷编较慢属一次性成本；warm 跑依赖缓存命中（TP-06 观察项，不设门）。
- **actionlint 正断言现状基线**：本机不可用，走降级分支；CI 首跑前 actionlint 正断言未本地验证（gate1 dev-MINOR17）——降级路径计入总数且 WARN 诚实，不静默绿。

## 开放问题（需人类裁决 / 待触发）

### 已关闭（gate1 裁决或本文件落定，不再呈裁）

- [x] **PRD OQ①（intent 1）——关闭**：形态 = 独立 `desktop` job + `windows-latest` 单腿（裁决字面已落 YAML 蓝图与断言 #1/#3）。
- [x] **PRD OQ②（intent 2）——实质收敛**：B1 裁决字面已含 `--all-targets`（#9 锁定）；PRD 侧编号关闭由 pm 同步，本架构按含口径设计。
- [x] **PRD OQ④（脚本载体）——关闭**：新增 `scripts/ci-desktop-yaml.test.sh`（沿五脚本先例；扩展已完结脚本否决）。
- [x] **PRD OQ⑤（当日实测归属）——关闭（冗余项）**：AC2 When 已归 Construction 拆②执行；dev 互审 fmt 实测仅作前期证据，不替代当日实测。
- [x] **架构旧 OQ「runs-on 选择」——关闭**：`windows-latest`（裁决）。
- [x] **架构旧 OQ「cache key 形态」——关闭**：`workspaces: apps/desktop -> target`（精确断言 #7）。
- [x] **架构旧 OQ「脚本名/断言计数」——关闭**：脚本名 `scripts/ci-desktop-yaml.test.sh`；断言基线 M=44（只增不减）。
- [x] **desktop `cargo test` 是否顺带——关闭**：**默认排除；选入需 PRD 修订**（决策 11）。

### 待人类裁决 / 待触发

- [ ] **OQ③（intent 3）dirty 分支处置**：若 Construction 当日实测 dirty，推荐 CI 步骤照落 + desktop fmt/clippy 修复归紧随的 desktop 代码批（接受 main 红窗）；备选 = 人类特批纯机械 `cargo fmt` 修复并入本批作为「仅 CI 配置变更」的显式豁免。**仅当实测 dirty 时触发**（证据链预期 clean）。
- [ ] [M6 follow-up，非 gate2 阻塞] **缓存体积/配额**：CI 首跑后实测 entry 体积/命中率/既有 job 时长；若挤占，按安全节顺序处置（shared-key → save-if → cache-targets 降级），变更 decisions 留痕。
