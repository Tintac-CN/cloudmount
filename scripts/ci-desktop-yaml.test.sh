#!/usr/bin/env bash
#
# scripts/ci-desktop-yaml.test.sh — ci-desktop-fmt-clippy U1：ci.yml desktop 执行面 + 零删改护栏静态断言
#   （TDD，静态 grep/awk 式，不要求 yq；Git Bash 直跑；repo 根或任意工作目录均可）
#
# 被测对象（唯一）：.github/workflows/ci.yml
#   —— desktop 执行面尚未实施（无 desktop job）→ desktop 断言（#1-#16、#45、#46）红；
#      其「无既有 desktop lint 面」已由 check-macos.yml / release.yml 核实（architecture 现状缺口）。
#
# 契约来源：docs/aidlc/ci-desktop-fmt-clippy/prd.md AC1..AC4 + test-strategy.md TP-01..TP-06
#   + architecture.md「断言清单 #1-#47」（含 B1 命令字面 / OQ① 独立 job + windows-latest 单腿裁决）。
#   M 基线 = 47 个 report 调用点（只增不减）；#42 按 actionlint 先例在可用/不可用分支展开不同数量
#   report 调用——实际 M 以运行末行为准，N 必须 == M（沿 112/113 环境相关先例，不得为对齐固定值弱化）。
#
# ── 机制复制声明（复制自 scripts/desktop-installers-yaml.test.sh ← 其先例 scripts/release-yaml.test.sh） ──
#   job_block/toplevel_block awk 段提取、grep_ok/grep_absent_ok 防空真、CRLF 剥除规范化副本
#   （tr -d '\r'）、actionlint 降级（SKIP + python yaml.safe_load → 键配平粗检 + WARN 降级告警；
#   可用分支含正断言 + 坏 YAML 注入自校验，TMP_ROOT/trap 隔离，不一致即 FAIL）——均复制自上述先例。
#   同步义务：先例机制若变更，本文件须跟进同步。
#   set -u 沿先例（-e/-o pipefail 会使「[ -s ] && 短路」与「grep -q 管道」变中途退出/SIGPIPE 假阴性）。
#
# ── 「断言不得弱化」操作定义（prd AC3 三条，可核） ──
#   ① 负断言必须经 grep_absent_ok（不得以裸 grep/取反或跳过替代；空块直接 FAIL，防空真）；
#   ② 正断言不得由精确字面放宽为子串（如 `--manifest-path apps/desktop/Cargo.toml` 完整命令
#      不得退化为 `cargo fmt` 子串匹配；不得以「包含判断」替代精确值、删除负断言、缩小 job_block 作用域）；
#   ③ 脚本自检、CRLF 剥除、降级计数断言必须存在（#43 / #44 / #42，缺一即视为弱化）。
#   另：已锁定断言不得删除、断言计数不得下降、新增断言允许；任何弱化须 decisions.md 留痕并经人类批准。
#
# ── TDD 红阶段预期（desktop 执行面未实现时） ──
#   #1-#16 + #45/#46 全 FAIL（desktop job 不存在；负断言因空块直接 FAIL——grep_absent_ok 空块即失败）；
#   护栏类 #17-#41 + #47 现状绿属正常（回归护栏）；机制项 #42-#44 按先例降级口径计。
#   本地 actionlint 不可用（M=47）时红阶段预期：FAIL=18（#1-#16、#45、#46），PASS=29。
#
# ── Construction 落字面范围声明 ──
#   job id（desktop）/ name（Desktop（fmt + clippy））/ runs-on（windows-latest）/
#   Format/Clippy 命令字面 / rust-cache workspaces 值（apps/desktop -> target）/
#   permissions（contents: read）/ timeout-minutes（60）/ 本脚本名均已由 architecture.md 钉死；
#   Construction 只落断言计数与实现，不再自行选形态。负断言 #14 只作用于 desktop job 块——
#   web-ui 的 `npm ci || npm install` 是既有合法降级，不得误伤。
#
# 行尾注意：ci.yml 为 CRLF 检出；断言前统一剥 \r 至临时规范化副本，再行 grep/awk。
# 输出：逐断言 `PASS/FAIL <名称>`（诊断走 stderr），末行 `<N>/<M> passed`；任一 FAIL → 退出非零。

set -u
export LC_ALL=C

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WF="$SCRIPT_DIR/../.github/workflows/ci.yml"
SELF="$SCRIPT_DIR/ci-desktop-yaml.test.sh"

TMP_ROOT="$(mktemp -d)"
trap 'rm -rf "$TMP_ROOT"' EXIT

PASS=0
FAIL=0
TOTAL=0

report() {  # report <名称> <ok:0/1>
  TOTAL=$((TOTAL + 1))
  if [ "$2" -eq 0 ]; then
    PASS=$((PASS + 1))
    echo "PASS $1"
  else
    FAIL=$((FAIL + 1))
    echo "FAIL $1"
  fi
}

note() { printf '  [diag] %s\n' "$*" >&2; }

# ---- 规范化副本：剥 CR（CRLF 检出），后续 grep/awk 全部基于此（复制自 desktop-installers-yaml.test.sh L90-94） ----
NORM="$TMP_ROOT/ci.normalized.yml"
if [ -f "$WF" ]; then tr -d '\r' <"$WF" >"$NORM"; fi
wf_ready=0
[ -s "$NORM" ] && wf_ready=1
[ "$wf_ready" -eq 1 ] || printf '[red] 被测文件不存在或为空：%s —— 全部断言按 FAIL 计（空真防护）\n' "$WF" >&2

# job_block <job 名> —— 输出该 job 块：自 `  <名>:`（2 空格缩进）行起，
# 至下一个 2 空格缩进键/分节注释（^  [^ ]）或 0 缩进键（^[^ ]）止（不含）；块不存在输出空
# （复制自 desktop-installers-yaml.test.sh L114-121 ← release-yaml.test.sh）
job_block() {
  awk -v pat="^  $1:" '
    inblk && /^  [^ ]/ { exit }
    inblk && /^[^ ]/   { exit }
    inblk { print; next }
    $0 ~ pat { inblk = 1; print }
  ' "$NORM" 2>/dev/null
}

# toplevel_block <键名> —— 输出 0 缩进键块（如 on:/env:），至下一个 0 缩进键止
# （复制自 desktop-installers-yaml.test.sh L125-131 ← release-yaml.test.sh）
toplevel_block() {
  awk -v pat="^$1:" '
    inblk && /^[^ ]/ { exit }
    inblk { print; next }
    $0 ~ pat { inblk = 1; print }
  ' "$NORM" 2>/dev/null
}

# grep_ok <文件> <grep 参数...> —— 正断言：文件非空且 grep 命中（空块不计命中，防空真）
# （复制自 desktop-installers-yaml.test.sh L146-150 ← release-yaml.test.sh）
grep_ok() {
  local f="$1"; shift
  [ -s "$f" ] || return 1
  grep "$@" "$f" >/dev/null 2>&1
}

# grep_absent_ok <文件> <grep 参数...> —— 负断言：文件非空且 grep 不命中（空块直接 FAIL，防空真）
# （复制自 desktop-installers-yaml.test.sh L154-158 ← release-yaml.test.sh）
grep_absent_ok() {
  local f="$1"; shift
  [ -s "$f" ] || return 1
  ! grep "$@" "$f" >/dev/null 2>&1
}

# ---- 物化断言对象块（job 均为 2 空格缩进；2 空格注释行会正确终止上一 job 块） ----
DESK_BLK="$TMP_ROOT/block.desktop.yml"
RUST_BLK="$TMP_ROOT/block.rust.yml"
RUSTMOCK_BLK="$TMP_ROOT/block.rust-mock-features.yml"
WEBUI_BLK="$TMP_ROOT/block.web-ui.yml"
ARM_BLK="$TMP_ROOT/block.arm-cross.yml"
ENV_BLK="$TMP_ROOT/block.env.yml"
ON_BLK="$TMP_ROOT/block.on.yml"
job_block desktop >"$DESK_BLK"
job_block rust >"$RUST_BLK"
job_block rust-mock-features >"$RUSTMOCK_BLK"
job_block web-ui >"$WEBUI_BLK"
job_block arm-cross >"$ARM_BLK"
toplevel_block env >"$ENV_BLK"
toplevel_block on >"$ON_BLK"

# ============ #1-#16：ci.yml desktop 执行面（AC1；B1 唯一字面源 + OQ① 形态） ============

ok=1
[ -s "$DESK_BLK" ] && ok=0
report "#1 desktop job 存在（job_block desktop 非空，2 空格顶级键）" "$ok"

ok=1
grep_ok "$DESK_BLK" -qE '^[[:space:]]*name: Desktop（fmt \+ clippy）[[:space:]]*$' && ok=0
report "#2 name: Desktop（fmt + clippy）整行字面（brownfield 命名镜像既有 job）" "$ok"

ok=1
grep_ok "$DESK_BLK" -qE '^[[:space:]]*runs-on: windows-latest[[:space:]]*$' && ok=0
report "#3 runs-on: windows-latest 整行字面（OQ① 单腿裁决）" "$ok"

ok=1
grep_ok "$DESK_BLK" -qE '^[[:space:]]*- uses: dtolnay/rust-toolchain@stable[[:space:]]*$' && ok=0
report "#4 dtolnay/rust-toolchain@stable 在位（AC1③）" "$ok"

ok=1
grep_ok "$DESK_BLK" -qE '^[[:space:]]*components: clippy, rustfmt[[:space:]]*$' && ok=0
report "#5 components: clippy, rustfmt 整行字面（两组件齐——缺一即步骤不可执行）" "$ok"

ok=1
grep_ok "$DESK_BLK" -qE '^[[:space:]]*- uses: Swatinem/rust-cache@v2[[:space:]]*$' && ok=0
report "#6 Swatinem/rust-cache@v2 在位（AC1④）" "$ok"

ok=1
ws_n="$(grep -cE '^[[:space:]]*workspaces:' "$DESK_BLK" 2>/dev/null || true)"
ws_n="${ws_n:-0}"
if [ -s "$DESK_BLK" ] && [ "$ws_n" -eq 1 ] \
   && grep_ok "$DESK_BLK" -qE '^[[:space:]]*workspaces: apps/desktop -> target[[:space:]]*$' \
   && grep_absent_ok "$DESK_BLK" -qF 'apps/desktop -> desktop'; then
  ok=0
else
  note "#7: desktop 块 workspaces 行数=$ws_n（期望恰 1 且值精确 apps/desktop -> target）"
fi
report "#7 workspaces 精确等于 apps/desktop -> target（仅行首缩进；禁 -> desktop/多 workspace 列表）" "$ok"

ok=1
grep_ok "$DESK_BLK" -qE '^[[:space:]]*run: cargo fmt --manifest-path apps/desktop/Cargo[.]toml --all -- --check[[:space:]]*$' && ok=0
report "#8 fmt 命令整行字面（B1：cargo fmt --manifest-path apps/desktop/Cargo.toml --all -- --check）" "$ok"

ok=1
grep_ok "$DESK_BLK" -qE '^[[:space:]]*run: cargo clippy --manifest-path apps/desktop/Cargo[.]toml --all-targets -- -D warnings[[:space:]]*$' && ok=0
report "#9 clippy 命令整行字面（B1：cargo clippy --manifest-path apps/desktop/Cargo.toml --all-targets -- -D warnings）" "$ok"

ok=1
if grep_absent_ok "$DESK_BLK" -F 'continue-on-error'; then ok=0; fi
report "#10 负 desktop 块内无 continue-on-error（任何缩进，防吞错）" "$ok"

ok=1
if grep_absent_ok "$DESK_BLK" -qF 'cd apps/desktop'; then ok=0; fi
report "#11 负 desktop 块内无 cd apps/desktop（形态锁定 --manifest-path）" "$ok"

ok=1
if grep_absent_ok "$DESK_BLK" -qE '^[[:space:]]*if:'; then ok=0; fi
report "#12 负 desktop 块内无任何 if: 行（含 if: false / if: always()——从严口径）" "$ok"

ok=1
if grep_absent_ok "$DESK_BLK" -qE '\b(always|success|failure|cancelled)\(\)'; then ok=0; fi
report "#13 负 desktop 块内无 status 函数调用（always()/success()/failure()/cancelled()）" "$ok"

ok=1
if grep_absent_ok "$DESK_BLK" -qE '\|\|[[:space:]]*true|;[[:space:]]*exit[[:space:]]+0|set[[:space:]]+\+e|if:[[:space:]]*false'; then ok=0; fi
report "#14 负 desktop 块内无 shell 级静默绿（|| true / ; exit 0 / set +e / if: false；不误伤 web-ui）" "$ok"

ok=1
grep_ok "$DESK_BLK" -qE '^[[:space:]]*permissions:[[:space:]]*$' \
  && grep_ok "$DESK_BLK" -qE '^[[:space:]]*contents: read[[:space:]]*$' && ok=0
report "#15 desktop 块含 permissions: + contents: read（最小权限）" "$ok"

ok=1
grep_ok "$DESK_BLK" -qE '^[[:space:]]*timeout-minutes:[[:space:]]*[1-9][0-9]*[[:space:]]*$' && ok=0
report "#16 desktop 块含 timeout-minutes 正整数（初值 60，值调整不改断言形态）" "$ok"

# ============ #17-#41：既有四 job + workflow 级零删改护栏（AC4 / TP-02） ============

ok=1
[ -s "$RUST_BLK" ] && ok=0
report "#17 护栏 rust job 块非空" "$ok"

ok=1
[ -s "$RUSTMOCK_BLK" ] && ok=0
report "#18 护栏 rust-mock-features job 块非空" "$ok"

ok=1
[ -s "$WEBUI_BLK" ] && ok=0
report "#19 护栏 web-ui job 块非空" "$ok"

ok=1
[ -s "$ARM_BLK" ] && ok=0
report "#20 护栏 arm-cross job 块非空" "$ok"

ok=1
grep_ok "$RUST_BLK" -qE '^[[:space:]]*run: cargo fmt --all -- --check[[:space:]]*$' && ok=0
report "#21 护栏 rust 命令整行字面：cargo fmt --all -- --check（两个 -- 齐）" "$ok"

ok=1
grep_ok "$RUST_BLK" -qE '^[[:space:]]*run: cargo check --workspace[[:space:]]*$' && ok=0
report "#22 护栏 rust 命令整行字面：cargo check --workspace" "$ok"

ok=1
grep_ok "$RUST_BLK" -qE '^[[:space:]]*run: cargo clippy --workspace --all-targets -- -D warnings[[:space:]]*$' && ok=0
report "#23 护栏 rust 命令整行字面：cargo clippy --workspace --all-targets -- -D warnings" "$ok"

ok=1
grep_ok "$RUST_BLK" -qE '^[[:space:]]*run: cargo test --workspace[[:space:]]*$' && ok=0
report "#24 护栏 rust 命令整行字面：cargo test --workspace" "$ok"

ok=1
grep_ok "$RUST_BLK" -qE '^[[:space:]]*components: clippy, rustfmt[[:space:]]*$' && ok=0
report "#25 护栏 rust job components: clippy, rustfmt 字面" "$ok"

ok=1
grep_ok "$RUST_BLK" -qE '^[[:space:]]*- uses: dtolnay/rust-toolchain@stable[[:space:]]*$' && ok=0
report "#26 护栏 rust job dtolnay/rust-toolchain@stable 字面" "$ok"

ok=1
grep_ok "$RUST_BLK" -qE '^[[:space:]]*runs-on: [$][{][{] matrix[.]os [}][}][[:space:]]*$' && ok=0
report "#27 护栏 rust job runs-on: \${{ matrix.os }} 字面" "$ok"

ok=1
grep_ok "$RUST_BLK" -qE '^[[:space:]]*os: \[ubuntu-latest, windows-latest, macos-latest\][[:space:]]*$' && ok=0
report "#28 护栏 rust job 三 OS matrix 整行字面（不得收窄）" "$ok"

ok=1
grep_ok "$RUSTMOCK_BLK" -qE '^[[:space:]]*runs-on: [$][{][{] matrix[.]os [}][}][[:space:]]*$' && ok=0
report "#29 护栏 rust-mock-features job runs-on: \${{ matrix.os }} 字面" "$ok"

ok=1
grep_ok "$RUSTMOCK_BLK" -qE '^[[:space:]]*os: \[ubuntu-latest, windows-latest, macos-latest\][[:space:]]*$' && ok=0
report "#30 护栏 rust-mock-features job 三 OS matrix 整行字面（第二处）" "$ok"

ok=1
grep_ok "$RUSTMOCK_BLK" -qE '^[[:space:]]*run: cargo test --workspace --features cloudmount-daemon/mock[[:space:]]*$' && ok=0
report "#31 护栏 rust-mock-features 命令整行字面：cargo test --workspace --features cloudmount-daemon/mock" "$ok"

ok=1
grep_ok "$WEBUI_BLK" -qE '^[[:space:]]*working-directory: web-ui[[:space:]]*$' && ok=0
report "#32 护栏 web-ui working-directory: web-ui 字面" "$ok"

ok=1
grep_ok "$WEBUI_BLK" -qE '^[[:space:]]*- run: npm ci \|\| npm install[[:space:]]*$' && ok=0
report "#33 护栏 web-ui npm ci || npm install 字面（既有合法降级）" "$ok"

ok=1
grep_ok "$WEBUI_BLK" -qE '^[[:space:]]*- run: npm run build[[:space:]]*$' && ok=0
report "#34 护栏 web-ui npm run build 字面" "$ok"

ok=1
grep_ok "$ARM_BLK" -qF -- 'cross build -p cloudmount-daemon -p cloudmount-cli' && ok=0
report "#35 护栏 arm-cross cross build -p cloudmount-daemon -p cloudmount-cli 字面" "$ok"

ok=1
grep_ok "$ARM_BLK" -qF -- 'cloudmountd --help' && ok=0
report "#36 护栏 arm-cross cloudmountd --help smoke 字面" "$ok"

ok=1
grep_ok "$ARM_BLK" -qF -- 'cloudmount --help' && ok=0
report "#37 护栏 arm-cross cloudmount --help smoke 字面" "$ok"

ok=1
grep_ok "$ENV_BLK" -qF 'CARGO_TERM_COLOR: always' && ok=0
report "#38 护栏 workflow env 块含 CARGO_TERM_COLOR: always 字面" "$ok"

ok=1
grep_ok "$ENV_BLK" -qF 'RUSTFLAGS: "-D warnings"' && ok=0
report "#39 护栏 workflow env 块含 RUSTFLAGS: \"-D warnings\" 字面（引号原样）" "$ok"

ok=1
grep_ok "$ON_BLK" -qE '^  push:[[:space:]]*$' \
  && grep_ok "$ON_BLK" -qE '^    branches: \[main\][[:space:]]*$' && ok=0
report "#40 护栏 on: 块 push: 下 branches: [main] 字面" "$ok"

ok=1
grep_ok "$ON_BLK" -qE '^  pull_request:[[:space:]]*$' && ok=0
report "#41 护栏 on: 块含 pull_request: 字面" "$ok"

# ============ #42：actionlint 正/负/降级/自校验（AC3 / TP-05；复制自 desktop-installers-yaml.test.sh L791-842） ============

if command -v actionlint >/dev/null 2>&1; then
  # 正：ci.yml 整文件合法（零错）
  ok=1
  if [ "$wf_ready" -eq 1 ] && actionlint "$WF" >/dev/null 2>&1; then ok=0; fi
  report "#42a actionlint ci.yml 零错（整文件合法性）" "$ok"

  # 负（拦截手段自校验）：注入语法错误的临时副本必须被拦截（非零）
  ok=1
  BROKEN="$TMP_ROOT/ci.broken.yml"
  if { cat "$NORM"; printf '\n  broken-injection: [unclosed-flow-seq\n'; } >"$BROKEN" && [ -s "$BROKEN" ]; then
    if actionlint "$BROKEN" >/dev/null 2>&1; then
      note "#42: 注入语法错误的副本 actionlint 返回 0（拦截失效）"
    else
      ok=0
    fi
  else
    note "#42: 注入副本构造失败"
  fi
  report "#42b 注入语法错误副本被 actionlint 拦截（非零，自校验）" "$ok"
else
  echo "SKIP actionlint（不可用）"
  ok=1
  if [ "$wf_ready" -eq 1 ]; then
    if command -v python >/dev/null 2>&1 || command -v python3 >/dev/null 2>&1; then
      PY_BIN="$(command -v python || command -v python3)"
      pypath="$NORM"
      command -v cygpath >/dev/null 2>&1 && pypath="$(cygpath -w "$NORM")"
      if "$PY_BIN" -c "import yaml" >/dev/null 2>&1; then
        if "$PY_BIN" -c "import yaml,sys; yaml.safe_load(open(sys.argv[1], encoding='utf-8'))" "$pypath" >/dev/null 2>&1; then
          ok=0
        else
          note "#42 降级: python yaml.safe_load 解析 ci.yml 失败（YAML 非法）"
        fi
      else
        # 终极降级：grep 键配平粗检（非完整解析，记缺口）
        if grep -q '^on:' "$NORM" && grep -q '^jobs:' "$NORM" \
           && [ "$(grep -cE '^[A-Za-z_][A-Za-z0-9_-]*:' "$NORM")" -ge 4 ]; then
          ok=0
          note "#42 终极降级: pyyaml 不可用，仅 grep 键配平粗检（非完整解析，记缺口）"
        else
          note "#42 终极降级: grep 键配平粗检未过（on:/jobs: 键缺失）"
        fi
      fi
    else
      note "#42 降级: python 不可用且无其他 YAML 解析手段"
    fi
  fi
  report "#42 降级 YAML 结构断言（ci.yml 可解析/键配平；降级分支计入总数）" "$ok"
  echo "WARN 降级告警：actionlint 不可用——仅覆盖 YAML 合法性（python yaml.safe_load 或键配平）+ 结构断言；语法拦截自校验（注入负路径）未执行，记缺口（test-strategy TP-05：拦截手段依赖）"
fi

# ============ #43-#44：脚本自检（AC3 / TP-04；防空真 + CRLF 剥除必须存在） ============

ok=1
grep -qF 'grep_ok()' "$SELF" \
  && grep -qF 'grep_absent_ok()' "$SELF" \
  && grep -qF '[ -s "$f" ] || return 1' "$SELF" && ok=0
report "#43 脚本自检：防空真机制在位（grep_ok/grep_absent_ok 非空前置，空块不计通过）" "$ok"

ok=1
if [ -f "$SELF" ] && [ -s "$NORM" ] && grep -qF "tr -d '\r'" "$SELF"; then
  CRLF_PROBE="$TMP_ROOT/crlf.probe.yml"
  awk '{ printf "%s\r\n", $0 }' "$NORM" >"$CRLF_PROBE"
  probe_cr="$(tr -cd '\r' <"$CRLF_PROBE" | wc -c | tr -d '[:space:]')"
  probe_ln="$(wc -l <"$CRLF_PROBE" | tr -d '[:space:]')"
  if [ "${probe_cr:-0}" -ge 1 ] && [ "${probe_cr:-0}" -ge "${probe_ln:-0}" ]; then
    if tr -d '\r' <"$CRLF_PROBE" | grep -qF 'name: CI'; then ok=0; else
      note "#44: CRLF 探针经 tr -d '\\r' 后未命中已知字面 name: CI"
    fi
  else
    note "#44: CRLF 探针未实际构造出 CRLF（cr=$probe_cr lines=$probe_ln）"
  fi
fi
report "#44 脚本自检：CRLF 剥除规范化副本在位（CRLF 探针经 tr 后断言可命中）" "$ok"

# ============ #45-#46：r2 增补负断言（形态/环境继承锁定；AC1①/⑤） ============

ok=1
if grep_absent_ok "$DESK_BLK" -qF 'working-directory: apps/desktop'; then ok=0; fi
report "#45 负 desktop 块内无 working-directory: apps/desktop（与 #11 成对，形态锁定 --manifest-path）" "$ok"

ok=1
if grep_absent_ok "$DESK_BLK" -qE '^[[:space:]]*RUSTFLAGS[[:space:]]*:'; then ok=0; fi
report "#46 负 desktop 块内无 RUSTFLAGS 覆写键（继承 workflow 级 -D warnings，防静默放行）" "$ok"

# ============ #47：matrix 全文件精确计数 + desktop 单腿负断言（AC1 形态 / AC4 / TP-01 / TP-02） ============

ok=1
n_matrix="$(grep -cE '^[[:space:]]*os: \[ubuntu-latest, windows-latest, macos-latest\][[:space:]]*$' "$NORM" 2>/dev/null || true)"
n_matrix="${n_matrix:-0}"
if [ "$wf_ready" -eq 1 ] && [ "$n_matrix" -eq 2 ]; then
  if [ -s "$DESK_BLK" ]; then
    grep_absent_ok "$DESK_BLK" -qE '^[[:space:]]*(strategy|matrix):' && ok=0
  else
    # 红阶段：desktop job 缺失由 #1 直接 FAIL 覆盖；本断言的护栏部分（matrix 精确计数）现状绿
    # （architecture 红阶段预期：护栏类 #47 绿）。desktop job 一旦存在则负断言严格生效。
    ok=0
  fi
else
  note "#47: 全文件三 OS matrix 精确行计数=$n_matrix（期望恰 2）"
fi
report "#47 全文件三 OS matrix 精确计数=2 且 desktop 无 strategy/matrix（windows 单腿）" "$ok"

# ============ 汇总 ============
echo "$PASS/$TOTAL passed"
if [ "$FAIL" -ne 0 ]; then
  exit 1
fi
exit 0
