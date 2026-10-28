# Architecture: ci-desktop-fmt-clippy

> 产出者：architect　日期：2026-10-28　来源：intent.md + prd.md
> （prd.md 在制未读——本批语义对齐基准 = intent.md 动词；gate1 对齐拍时以 pm 工件校验补齐）
> route=classic（decisions.md A1：表单未选 → 协议缺省锁定）。本 artifact = CI 变更蓝图。

## 逻辑架构

### 现状与缺口

- **触发面/环境**：ci.yml `on:` = push(main) + pull_request；workflow 级 env `RUSTFLAGS: "-D warnings"`（全 job 继承）。
- **既有四 job**（本批零删改对象）：`rust`（fmt/check/clippy/test ×3 OS matrix）、`rust-mock-features`（mock 腿 ×3 OS）、`web-ui`（npm build）、`arm-cross`（musl 交叉冒烟）。job 间无 needs，全并行。
- **缺口机理**：`apps/desktop` 为根 Cargo.toml `exclude`（:13）的独立 cargo 项目——`rust` job 的 `cargo fmt --all`/`cargo clippy --workspace` 只覆盖 workspace members（7 crates），对 desktop **零效力**（desktop-installers gate2 互审 M3 实证：desktop fmt dirty 394 行未被根级命令拦截）。desktop 的 fmt/clippy 面**在 CI 从不执行**——ops-runbook 硬门（desktop fmt/clippy 单独跑必达）只能靠人工清单兜底，无流水线上游拦截。
- **改动面**：`.github/workflows/ci.yml` 新增 `desktop` job（唯一产品面改动）+ 新断言脚本 `scripts/ci-desktop-yaml.test.sh`（守护面）。两文件之外零触碰；desktop 源码零触碰（intent 约束）。
- **覆盖闭环**：rust job（workspace 7 crates，3 OS）+ desktop job（exclude 项目，1 OS）= 全仓 fmt/clippy CI 覆盖。
- **依赖面**：desktop job 依赖 = `apps/desktop/Cargo.toml` + `apps/desktop/Cargo.lock`（独立锁文件，**已确认在仓**——CI 依赖钉死与根锁同纪律）+ crates.io 官方源（ci.yml env 注释口径：rsproxy 仅本地）。三个 action（checkout@v4 / dtolnay/rust-toolchain@stable / Swatinem/rust-cache@v2）均为 ci.yml 既有 in-use 面——**零新增第三方 action**。
- **复核注记（Construction）**：`.github/workflows/check-macos.yml` 不在本批授权阅读清单内未读；intent/audit 口径断言 desktop fmt/clippy「在 CI 从不执行」覆盖全部 workflow——Construction 落地前顺手 grep 该文件确认无既有 desktop lint 面（若有则蓝图降级为合并评估，回开问题）。

### desktop CI job 设计（核心）

#### 形态对比：新增独立 job vs 既有 `rust` job 内追加 step

| 维度 | A. 新增独立 `desktop` job（**推荐**） | B. `rust` job 内追加 step |
|---|---|---|
| CI 时长 | 与 rust job **并行**——desktop 依赖树（eframe/wgpu，数百 crate）的编译时长不叠加进 rust job 关键路径 | 追加的 step 在**三条 OS matrix 腿各串行执行一遍**——每腿都冷编 desktop 依赖树，rust job 三腿全体系性变慢 |
| 缓存策略 | 自带 `workspaces` 参数为 `apps/desktop/target` 立独立 cache entry（见下），**不动既有 rust-cache step** | rust job 的 rust-cache key 只含根 workspace——desktop 部分要么每跑冷编（不可接受），要么**改既有 rust-cache step 加参数**——与「既有 job 零删改」约束直接相抵 |
| 失败归因 | job 名即失败面（desktop fmt/clippy 红，PR 页一眼可辨） | desktop 红连坐 rust job 三腿显示，需展开看 step |
| 约束合规 | 四既有 job 字面零触碰（最保守解读下合规） | 追加 step 字面允许（「或在既有 job 内追加 step」），但缓存代价使方案劣化 |
| 仓内先例 | `rust-mock-features` 即「并列 job 承载独立检查腿」先例 | release.yml 的 desktop 步追加进 installer-msi-windows 是因 WiX 归集**次序约束**（desktop exe 须先入 BinDir）——CI lint 无此约束，无可比性 |

**推荐 A**。核心理由：desktop 是依赖最重的编译单元��独立 job 化把它的成本从「串行 ×3 腿」变为「并行 ×1 腿」，且缓存与约束合规都只有 A 成立。

#### runs-on 与 matrix 必要性

- **fmt**：rustfmt 输出平台无关（仓内无 rustfmt.toml/.rustfmt.toml——已确认，本地/CI/任意 OS 格式行为零分歧）——**单次执行即足**。rust job 三腿各跑一遍 fmt 属既有形态（冗余但不评不改）；新 job 不复制该冗余。
- **clippy**：desktop own-code 平台门控面实证（全 src grep）——`#[cfg(windows)]` ×3（main.rs:28/682/710，伪隐藏/HWND 面）+ `#[cfg(not(windows))]` ×1（main.rs:745）+ `#[cfg(test)]` ×2；**无 `cfg(target_os="macos")`/`cfg(unix)` 门控**（mac 特定行为在 tray-icon 依赖内，非本地代码）。cfg 摘除的代码 clippy **不 lint**——OS 选择决定门控面覆盖。
- **推荐 `runs-on: windows-latest` 单腿**：
  1. 覆盖最大平台特定块（cfg(windows) 三处——desktop 平台特定逻辑的主体）；
  2. **对齐终验口径执行环境**——ops-runbook 硬门/release-checklist 在 Windows 本机（Git Bash）执行，M3 实证亦为 Windows 平台：CI 绿红与终验绿红同 OS 族，正是「上游补死」的语义（若选 ubuntu，cfg(windows) 块内 clippy 违规将出现「终验红而 CI 绿」——缺口原样复发）；
  3. 免 3 倍成本：3-OS matrix 相对单 windows 腿仅多覆盖 cfg(not(windows)) 一处小分支。
- 备选（开问题 4 交人类）：ubuntu-latest（最省，但漏 windows 门控面——否决理由如上）；3-OS matrix（与 rust job 全对称，3 倍成本）。
- Windows runner 较慢的代价由 rust-cache 缓解（warm 跑为增量）；首跑冷编一次属一次性成本。

#### 推荐形态完整 YAML 蓝图

插入位置：`rust-mock-features` job 块之后、`web-ui` 之前（rust 族 job 相邻分组）。命令形态 = `--manifest-path`（**仓内已锁定先例**：desktop-installers-yaml.test.sh L26「desktop 构建形态 = --manifest-path apps/desktop/Cargo.toml（cd 形态不采）」+ release.yml 两处 desktop 步同形态；产物语义一致落 `apps/desktop/target`）。

```yaml
  # apps/desktop 为 workspace exclude 独立 cargo 项目（根 Cargo.toml:13）——
  # rust job 的 fmt --all / clippy --workspace 只覆盖 workspace members，对
  # desktop 零效力（desktop-installers gate2 互审实证：desktop fmt dirty 394 行
  # 未被根级命令拦截）。本 job 补死该缺口：命令与 ops-runbook 终验硬门同口径，
  # 构建形态沿 release.yml desktop 步先例（--manifest-path，产物落 apps/desktop/target）。
  desktop:
    name: Desktop（fmt + clippy）
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      # desktop 独立 target 目录（apps/desktop/target）不在 rust job 的 cache
      # key 覆盖内——workspaces 参数为其立独立 entry（key 含 desktop 自身的
      # Cargo.lock hash，依赖变更自动失效）
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: apps/desktop -> desktop
      - name: Format
        run: cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check
      - name: Clippy
        run: cargo clippy --manifest-path apps/desktop/Cargo.toml --all-targets -- -D warnings
```

蓝图要点：
- **step 名** `Format`/`Clippy` 镜像 rust job 命名；fmt 先行（廉价快失败），序同 rust job。
- **clippy 含 `--all-targets`**（intent 开问题 2 的推荐答案）：终验口径字面即含；连带编译 desktop 全部 4 个 `#[test]`（main.rs t28_policy_tests ×2 + t30_structural ×2）——lint 测试代码正是 M3 类盲区高发面。时长顾虑由 cache 缓解。
- **RUSTFLAGS env 继承**（workflow 级 `-D warnings`）：与 rust job 行为对等（rustc 级 warning 对本地 path 依赖同样 -D；registry 依赖经 cargo cap-lints 不受累）——无需 job 内覆写。
- **cache key 语义**（Swatinem/rust-cache@v2）：key = job id + OS + rustc 版本 + 各 workspace 的 Cargo.lock hash + `workspaces` 行的 cache-key 后缀。`apps/desktop -> desktop` 使 `apps/desktop/target` 获得独立 entry，desktop Cargo.lock 变更自动失效。已知小开销：`workspaces` 语义为「additional」——根 workspace 的 cache 仍会被连带恢复/保存一份（本 job 不用根 target，属无害浪费；v2 无排除手段，接受）。
- **无 needs 接线**：ci.yml 无 job 间依赖，新 job 自然并行。

### 现状确认

- **历史实证**（intent 转述 desktop-installers gate2 互审 M3，2026-10-16）：desktop fmt dirty **394 行**。此后 desktop-installers 批内 main.rs 有托盘接线改动——**当前 clean/dirty 未知**。
- **clippy 现状**：无任何 dirty/clean 实测留痕——**完全待实测**。
- **本机构建痕迹**（apps/desktop/target/debug/ 指纹）：bin 与 test 目标均在 Windows 本机编译过（`bin-cloudmount-desktop`/`test-bin-cloudmount-desktop` fingerprints + .pdb）——desktop bin 可编译、4 测试目标可编译的旁证。
- **当前 fmt/clippy 状态 = 待 Construction 实测**（本角色无 Bash）。实测命令原文（repo 根，Git Bash）：

```bash
# fmt（不经 rustc，RUSTFLAGS 无关）：
cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check

# clippy——CI 复现口径：ci.yml workflow env 含 RUSTFLAGS="-D warnings"，
# 本机不带此 env 实测会漏报 rustc 级 warning，须显式带上才与 CI 首跑行为一致：
RUSTFLAGS="-D warnings" cargo clippy --manifest-path apps/desktop/Cargo.toml --all-targets -- -D warnings

# 等价终验口径原样形态（apps/desktop 内执行，与 ops-runbook 硬门字面一致）：
cd apps/desktop && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings
```

- **结果分支**：clean → CI 补死后首跑绿；dirty → 首跑红（intent 开问题 3 预案见决策表 #3 与开问题 3）。
- **旁注**：desktop 有 4 个单测，main.rs:961-963 注释自认「desktop 被 workspace exclude，CI 不可见——接线归人工清单⑦兜底」——`cargo test` 面缺口是 fmt/clippy 之外的相邻缺口，本批范围外（开问题 6）。

### 断言守护设计

**落点决策：新独立脚本，否决扩展 `desktop-installers-yaml.test.sh`。**

| 选项 | 裁决 | 为什么 |
|---|---|---|
| 扩展 desktop-installers-yaml.test.sh | 否决 | 该脚本为 gate2 锁定**已完结工件**，112/113 断言计数写进已完结 ops-runbook 硬门（「五脚本」口径）——任何扩展都改计数 = 碰已完结工件口径，违回流机制「已完结工件不再修改」；且其被测对象域 = release.yml 交付面（wxs/dmg/icons），ci.yml 不在其契约内 |
| 新独立脚本 | **推荐** | 沿五脚本先例与命名模式；契约来源 = 本批 prd/test-strategy（在制）；计数由本批 runbook 侧以「六脚本」口径收录 |

**新脚本蓝图**：`scripts/ci-desktop-yaml.test.sh`（命名沿 `<域>-yaml.test.sh` 模式：release-yaml / installers-yaml / desktop-installers-yaml 先例；不取 `ci-yaml.test.sh`——全域名过宽，本批契约只覆盖 desktop job + 护栏，未来 ci 批可扩展）。

机制复制清单（复制自 desktop-installers-yaml.test.sh，其自身复制自 release-yaml.test.sh——同步义务沿先例注记）：
- `set -u` + `LC_ALL=C` + SCRIPT_DIR 定位（任意工作目录可跑，Git Bash 直跑）；
- CRLF 剥除规范化副本（ci.yml 为 CRLF 检出，断言前 `tr -d '\r'`）；
- `job_block`/`toplevel_block` awk 段提取（ci.yml job 同为 2 空格缩进，函数可直接复用）；
- `grep_ok`/`grep_absent_ok` 防空真（空块不计命中）；
- actionlint 降级（先例 L232-283）：可用 → 正断言（ci.yml 整文件零错）+ 负断言（注入语���错误副本必须被拦截——拦截手段自校验）；不可用 → `SKIP actionlint` + python yaml.safe_load 键配平粗检 + `WARN 降级告警`，降级分支计入总数 + 脚本自检断言（沿 TP-10 形态）；
- 输出：逐断言 `PASS/FAIL <名称>` + 末行 `<N>/<M> passed`；任一 FAIL 非零退出。

断言清单蓝图（TP 编号以 QA test-strategy 在制工件为准，gate1 对齐拍后 Construction 落定；计数 Construction 钉死）：

| # | 断言 | 类型 |
|---|---|---|
| 1 | `desktop` job 存在（job_block desktop 非空） | 正 |
| 2 | `name: Desktop（fmt + clippy）` 字面 | 正 |
| 3 | `runs-on: windows-latest` 字面（若 gate1 改选则同步） | 正 |
| 4 | `dtolnay/rust-toolchain@stable` + `components: clippy, rustfmt`（两组件齐——缺一即步骤不可执行） | 正 |
| 5 | `Swatinem/rust-cache@v2` 在位 | 正 |
| 6 | rust-cache `workspaces:` 参数含 `apps/desktop`（intent 约束「独立 target 目录需 cache key 覆盖」字面落断言） | 正 |
| 7 | fmt 命令字面 `cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check` | 正 |
| 8 | clippy 命令字面 `cargo clippy --manifest-path apps/desktop/Cargo.toml --all-targets -- -D warnings` | 正 |
| 9 | desktop job 块内无 `continue-on-error`（防软绿逃生门——「补死」语义不许带后门） | 负 |
| 10 | desktop job 块内无 `cd apps/desktop`（形态锁定 --manifest-path，沿 L26 先例） | 负 |
| 11 | 零删改护栏：四既有 job id 在位（rust / rust-mock-features / web-ui / arm-cross，job_block 非空 ×4） | 护栏 |
| 12 | 零删改护栏：rust job 三命令字面不动（fmt --all --check / clippy --workspace --all-targets / test --workspace） | 护栏 |
| 13 | 零删改护栏：`on:` 触发面不动（push main + pull_request） | 护栏 |
| 14 | 零删改护栏：workflow env `RUSTFLAGS: "-D warnings"` 不动（clippy 行为的一部分） | 护栏 |
| 15 | actionlint 正/负/降级/自校验（复制先例四条形态） | 机制 |

TDD 红阶段预期：desktop job 断言（1-10）全 FAIL（job 不存在）；护栏类（11-14）与 actionlint 现状绿属正常（回归护栏）——沿 desktop-installers-yaml 头注 L41-43 口径。

运行载体注记：新脚本进本批 gate 自验；desktop-installers `release-checklist.md` ② 的「五脚本循环」为已完结工件**不改**——今后 runbook 硬门以「六脚本」口径收录（本批 ops/retro 侧沉淀，非本 artifact 动已完结清单）。

## 数据模型

CI 配置批——无数据库/持久存储；键语义如下：

| 实体 | 键 | 约束/索引 |
|---|---|---|
| ci.yml job | job id（2 空格缩进顶级键） | 新增 `desktop`；四既有 id 由护栏断言锁定不可删改 |
| cache entry（desktop） | job id + OS + rustc 版本 + `apps/desktop/Cargo.lock` hash + 后缀 `desktop` | 经 rust-cache `workspaces` 参数覆盖 `apps/desktop/target`；锁文件变更自动失效 |
| desktop 构建图 | `apps/desktop/Cargo.lock`（独立锁，与根锁零共享） | CI 依赖钉死（供应链约束） |
| 断言脚本 | `scripts/ci-desktop-yaml.test.sh` | 末行 `<N>/<M> passed`；任一 FAIL 非零退出；计数 Construction 落定 |
| desktop 产物目录 | `apps/desktop/target` | `--manifest-path` 语义下产物落此，不入根 target（release.yml 先例同语义） |

## 与 PRD 的语义对齐表

（prd.md 在制——动词取自 intent.md；gate1 对齐拍以 pm 工件校验补齐）

| PRD 动词 | 物理语义 | 说明 |
|---|---|---|
| 补步骤（CI 补 desktop fmt/clippy） | ci.yml 新增 `desktop` job：Format/Clippy 两 step，`--manifest-path apps/desktop/Cargo.toml` 字面 | 唯一产品面改动 = ci.yml；四既有 job 零删改 |
| 违规即 CI 红 | fmt `--check` 非零退出 / clippy `-- -D warnings` 非零退出 → step 失败 → job 红 → PR/push 门控 | 无 `continue-on-error` 逃生门（断言 #9 负断言守护） |
| 上游补死（终验口径缺口） | CI 在 PR/push 即执行与 ops-runbook 硬门同字面的命令——先于人工终验拦截 | 命令口径 = runbook 硬门原文（--all-targets/-D warnings）；OS 族对齐终验本机（windows） |
| 确认现状 | Construction 本机实测（「现状确认」节命令原文），clean/dirty 如实报告 | 本角色无 Bash——标「待 Construction 实测」 |
| 守护（断言） | 新脚本 `scripts/ci-desktop-yaml.test.sh`：字面断言 + 零删改护栏 + actionlint | 沿五脚本先例；扩展旧脚本被否决（runbook 计数锁定已完结工件） |
| 既有 job 零删改 | 护栏断言：四 job id + rust 三命令字面 + `on:` 触发面 + RUSTFLAGS env | 约束文字落为可执行断言 |
| 不改 desktop 源码 | 本批 diff 面 = ci.yml + 新脚本两文件 | 若实测 dirty：修复不混入本批（开问题 3 处置） |
| 缓存覆盖（desktop 独立 target 需 cache key） | rust-cache `workspaces: apps/desktop -> desktop`——独立 entry，key 含 desktop Cargo.lock hash | intent 约束字面的物理落点 |

## 关键决策与理由

| # | 决策 | 为什么 |
|---|---|---|
| 1 | 新增独立 `desktop` job，不在 rust job 内追加 step | 时长：desktop 依赖树最重（eframe/wgpu 数百 crate），独立 job 与 rust job 并行、不进其关键路径；追加则三 OS 腿各串行冷编。缓存：独立 job 自带 workspaces 参数即可覆盖 desktop target，追加方案须改既有 rust-cache step（违「零删改」）。归因：job 名即失败面。先例：rust-mock-features 并列 job |
| 2 | `runs-on: windows-latest` 单腿 | fmt 平台无关单次即足；clippy 门控面实证 = cfg(windows)×3 + cfg(not(windows))×1、无 mac 门控——windows 腿覆盖最大平台特定块且与终验口径执行环境（Windows 本机）同 OS 族：CI 绿红对齐终验绿红即「补死」语义；ubuntu 会造「终验红 CI 绿」缺口复发；3-OS matrix 3 倍成本仅多覆盖一处小分支 |
| 3 | 若实测 dirty：CI 步骤照落 + 修复归紧随的 desktop 代码批 | 补死是本批存在理由——带病也落（否则缺口永续）；「仅 CI 配置变更」约束禁止顺手改 desktop 源码；main 红窗由紧随批收敛（备选：人类特批纯机械 `cargo fmt` 修复并入——开问题 3） |
| 4 | clippy 含 `--all-targets` | 终验口径字面（runbook 硬门）即含；连带 lint 4 个测试目标——测试代码是 M3 类盲区高发面；时长由 cache 缓解 |
| 5 | 命令形态 `--manifest-path`（否决 cd/working-directory） | 仓内已锁定先例：desktop-installers-yaml.test.sh L26 明文「cd 形态不采」+ release.yml 两处 desktop 步同形态；产物语义一致（落 apps/desktop/target） |
| 6 | rust-cache `workspaces: apps/desktop -> desktop` | intent 约束明文要求独立 target 的 cache key 覆盖；Swatinem key 语义 = job+OS+rustc+各 workspace Cargo.lock hash——desktop 独立锁（在仓）入 key 自动失效。已知小开销：根 workspace 缓存连带恢复一份（input 语义 = additional，v2 无排除手段）——无害接受 |
| 7 | 不新增 permissions 块 | ci.yml 现状零 permissions 块（audit 已知发现，沿袭）；新 job 零 secrets 零写域，默认 token 权限即最小可用；显式收紧 = 权限面变更（波及四既有 job 统一风格），归 audit 承接批统一处理——本批不扩权也不动权限面 |
| 8 | 新独立断言脚本（否决扩展 desktop-installers-yaml.test.sh） | 目标脚本为 gate2 锁定已完结工件，112/113 计数写进已完结 runbook 硬门——扩展即改计数即碰已完结口径；其被测对象域亦不含 ci.yml |
| 9 | 脚本名 `scripts/ci-desktop-yaml.test.sh` | 沿 `<域>-yaml.test.sh` 命名先例；域 = ci.yml 的 desktop 面；不取 ci-yaml.test.sh（全域名过宽——本批契约只覆盖 desktop job+护栏） |
| 10 | fmt 单次执行，不复制 rust job 三腿冗余 | rustfmt 输出平台无关（仓内无 rustfmt 配置——已确认零分歧源）；rust job 既有冗余不评不改，新 job 不复制 |
| 11 | desktop `cargo test` 不顺带进本 job | intent 范围 = fmt/clippy；clippy --all-targets 已编译测试目标、追加 test step 边际成本≈0 的诱惑真实，但范围纪律优先——列开问题 6 供人类裁「顺带」，默认 backlog 候选 |
| 12 | 断言含零删改护栏（四 job id/rust 三命令/on:/RUSTFLAGS） | 「既有 job 零删改」从约束文字落为可执行断言——沿 desktop-installers-yaml 护栏类断言先例（头注 L42-43「现状绿属正常」） |

## 安全与权限

- **权限面**：ci.yml 无 workflow/job 级 `permissions` 块——GITHUB_TOKEN 取仓库默认权限。新 desktop job 使用面 = checkout（contents 读）+ actions cache（缓存域）——**零 secrets 引用、零写域需求**。本批不扩权、不新增 permissions 块（决策 7）：显式收紧属权限面变更，audit「无 permissions 块」发现的承接归其批统一处理。
- **供应链**：零新增第三方 action（三 action 均既有 in-use 面）；desktop 独立 `Cargo.lock` 在仓 = CI 构建依赖钉死；CI 走 crates.io 官方源（rsproxy 仅本地——ci.yml env 注释口径）。
- **密钥管理**：无新增密钥；无新增网络端点。
- **cache 面**：cache entry 仅含编译产物/注册表缓存（apps/desktop/target + ~/.cargo），无敏感物；key 随 Cargo.lock 自动失效，无手动 key 维护面。

## 开放问题（需人类裁决）

- [ ] [intent 1] desktop fmt/clippy CI 形态：新增独立 job vs rust job 追加 step——推荐**新增独立 job**（决策 1：时长/缓存/归因/约束合规四维均优）；gate1 确认或否决。
- [ ] [intent 2] desktop clippy 是否含 `--all-targets`——推荐**含**（决策 4：终验口径字面 + lint 测试目标盲区；时长由 cache 缓解）；gate1 确认。
- [ ] [intent 3] 若实测 dirty（CI 补死后首跑即红）：推荐 CI 步骤照落 + desktop fmt/clippy 修复归**紧随的 desktop 代码批**（接受 main 红窗）；备选 = 人类特批纯机械 `cargo fmt` 修复（零语义风险）并入本批作为「仅 CI 配置变更」约束的显式豁免。以 Construction 实测结果触发裁决。
- [ ] [新增] runs-on 选择：推荐 `windows-latest`（决策 2：覆盖 cfg(windows) 门控面 + 对齐终验本机环境）；备选 ubuntu-latest（最省但漏 windows 门控面）/ 3-OS matrix（全覆盖、3 倍成本）。gate1 裁决。
- [ ] [新增] cache key 形态：推荐 `workspaces: apps/desktop -> desktop`（决策 6）；注记根 workspace 缓存连带恢复的无害小开销。gate1 确认。
- [ ] [新增] desktop `cargo test`（4 测试，main.rs:961 注释自认「CI 不可见」）是否顺带进新 job：推荐本批**不扩**（范围纪律）→ backlog 候选；边际成本≈0，人类可裁「顺带」。
- [ ] [新增] 断言脚本名与断言计数落字面：推荐 `scripts/ci-desktop-yaml.test.sh`（决策 9），断言数 Construction 落定（蓝图 15 组，护栏拆分后约 16-19 条）；TP 编号以 QA 在制工件为准。gate1 轻裁决。
