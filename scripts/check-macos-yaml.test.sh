#!/usr/bin/env bash
#
# scripts/check-macos-yaml.test.sh — U3（macos-release）check-macos.yml 结构断言（TDD，静态 grep/awk 式，不要求 yq）
#
# 被测对象：.github/workflows/check-macos.yml（只读断言；本测试零修改被测文件）
# 契约来源：docs/aidlc/macos-release/units/3.md（断言不得弱化）
#   + prd.md AC3①（语义对齐内联小节）+ test-strategy.md T6
#   + R2-MAJOR 裁决：独立 workflow 载体，release.yml 零副作用
#
#   T6  文件存在 .github/workflows/check-macos.yml（独立载体，不落 release.yml）；
#       on: 仅 workflow_dispatch（块式唯一键，唯一手动触发入口）；
#       on: 无任何自动触发面（push/pull_request/schedule 等零命中，R2-MAJOR：零副作用）；
#       runs-on 恰为 macos-15（Q1 pin，单 runner，全文件恰一行）；
#       targets 安装面（cargo check 步之外，rustup target add / toolchain targets:）含
#       aarch64-apple-darwin 与 x86_64-apple-darwin；
#       含 cargo check --target aarch64-apple-darwin 与 cargo check --target x86_64-apple-darwin
#       两步字面（顺序步双 target check，AC3①判据），且覆盖 -p cloudmount-daemon -p cloudmount-cli；
#       无 strategy:/matrix:（固定顺序步，与 U2 同口径）；
#       job 骨架含 actions/checkout 与 Swatinem/rust-cache（对齐 release.yml binaries-macos 先例）
#   actionlint 可用时：对 check-macos.yml 零错（整文件合法性）；对注入语法错误的临时副本
#       （mktemp 生成）确认非零（拦截手段自校验）；actionlint 不可用 → 输出
#       `SKIP actionlint（不可用）` + 降级 YAML 解析断言 + 降级告警行（记缺口）
#
# 行尾注意：workflow 文件可能 CRLF 检出；断言前统一剥 \r 至临时规范化副本，再行 grep/awk。
# 运行：bash scripts/check-macos-yaml.test.sh（Git Bash 直跑；repo 根或任意工作目录均可）
# 输出：逐断言 `PASS/FAIL <名称>`（诊断走 stderr），末行 `<N>/<M> passed`；任一 FAIL → 退出非零。
# TDD 红阶段说明：check-macos.yml 未实施时，T6 断言全 FAIL（红态预期）；
#   绿阶段新建该文件后本脚本应全绿（actionlint 不可用时走降级分支，记缺口）。

set -u
export LC_ALL=C

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WF="$SCRIPT_DIR/../.github/workflows/check-macos.yml"

TMP_ROOT="$(mktemp -d)"
trap 'rm -rf "$TMP_ROOT"' EXIT

PASS=0
FAIL=0
TOTAL=0

[ -f "$WF" ] || printf '[red] 被测文件不存在：%s —— T6 断言全红（绿阶段新建该文件）\n' "$WF" >&2

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

# ---- 规范化副本：剥 CR（CRLF 检出），后续 grep 全部基于此 ----
NORM="$TMP_ROOT/check-macos.normalized.yml"
if [ -f "$WF" ]; then tr -d '\r' <"$WF" >"$NORM"; fi
wf_ready=0
[ -s "$NORM" ] && wf_ready=1

# toplevel_block <键名> —— 输出 0 缩进键块（如 on:），至下一个 0 缩进键止（同 U2 口径）
toplevel_block() {
  awk -v pat="^$1:" '
    inblk && /^[^ ]/ { exit }
    inblk { print; next }
    $0 ~ pat { inblk = 1; print }
  ' "$NORM" 2>/dev/null
}

# grep_ok <文件> <grep 参数...> —— 正断言：文件非空且 grep 命中（空文件不计命中，防空真）
grep_ok() {
  local f="$1"; shift
  [ -s "$f" ] || return 1
  grep "$@" "$f" >/dev/null 2>&1
}

# grep_absent_ok <文件> <grep 参数...> —— 负断言：文件非空且 grep 不命中（空文件直接 FAIL，防空真）
grep_absent_ok() {
  local f="$1"; shift
  [ -s "$f" ] || return 1
  ! grep "$@" "$f" >/dev/null 2>&1
}

# ---- 物化断言对象块 ----
ON_BLK="$TMP_ROOT/block.on.yml"
toplevel_block on >"$ON_BLK"
# on 块剥注释副本：防注释里提及触发器字面造成正/负断言失真
ON_NC="$TMP_ROOT/block.on.nocomment.yml"
sed 's/#.*$//' "$ON_BLK" >"$ON_NC" 2>/dev/null
# targets 安装面副本：剥含 cargo check 的行——双 target 须在 check 步之外独立声明
# （rustup target add 或 dtolnay/rust-toolchain with: targets:），防仅靠 check 步字面空过
NOCHK="$TMP_ROOT/check-stripped.yml"
if [ -s "$NORM" ]; then grep -vF 'cargo check --target' "$NORM" >"$NOCHK"; fi

# 自动触发面清单（GitHub Actions 全部非手动触发事件；workflow_dispatch 不在列）
AUTO_TRIGGERS='push|pull_request|pull_request_review|pull_request_target|schedule|release|issues|issue_comment|create|delete|fork|gollum|public|watch|workflow_call|workflow_run|repository_dispatch|deployment|deployment_status|status|check_run|check_suite|label|milestone|page_build|project|project_card|project_column|discussion|registry_package'
# 命中形态：键（  push: / 内层 status:）与 flow 列表项（on: [workflow_dispatch, push]）皆拦
AUTO_RE="(^|[^A-Za-z_-])($AUTO_TRIGGERS)[[:space:]]*(:|,|\])"

# ============ T6：check-macos.yml 结构（AC3①；R2-MAJOR 独立零副作用载体） ============

ok=1
if [ -f "$WF" ]; then ok=0; fi
report "T6 文件存在 .github/workflows/check-macos.yml（独立载体）" "$ok"

ok=1
if [ -s "$ON_NC" ]; then
  on_keys="$(grep -cE '^  [A-Za-z_][A-Za-z0-9_-]*:' "$ON_NC" || true)"
  wd_keys="$(grep -cE '^  workflow_dispatch:' "$ON_NC" || true)"
  if [ "$on_keys" -ge 1 ] && [ "$on_keys" -eq "$wd_keys" ]; then ok=0; else
    note "T6: on 块 2 缩进键数=$on_keys，其中 workflow_dispatch 键数=$wd_keys（要求唯一且恰为 workflow_dispatch）"
  fi
fi
report "T6 on: 仅 workflow_dispatch 键（块式，唯一手动触发入口）" "$ok"

ok=1
if grep_absent_ok "$ON_NC" -E "$AUTO_RE"; then ok=0; fi
report "T6 on: 无任何自动触发面（push/pull_request/schedule 等，R2-MAJOR：零副作用）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ]; then
  runs_total="$(grep -cE '^[[:space:]]*runs-on:' "$NORM" || true)"
  runs_mac15="$(grep -cE '^[[:space:]]*runs-on:[[:space:]]*macos-15[[:space:]]*$' "$NORM" || true)"
  if [ "$runs_total" -eq 1 ] && [ "$runs_mac15" -eq 1 ]; then ok=0; else
    note "T6: 全文件 runs-on 行数=$runs_total，恰为 macos-15 的行数=$runs_mac15（单 job 单 runner）"
  fi
fi
report "T6 runs-on 恰为 macos-15（Q1 pin，单 runner 双 target）" "$ok"

ok=1
grep_ok "$NOCHK" -qF 'aarch64-apple-darwin' && ok=0
report "T6 targets 安装面含 aarch64-apple-darwin（cargo check 步之外独立声明）" "$ok"

ok=1
grep_ok "$NOCHK" -qF 'x86_64-apple-darwin' && ok=0
report "T6 targets 安装面含 x86_64-apple-darwin（cargo check 步之外独立声明）" "$ok"

ok=1
grep_ok "$NORM" -qF -- 'cargo check --target aarch64-apple-darwin' && ok=0
report "T6 含 cargo check --target aarch64-apple-darwin 步（AC3①判据）" "$ok"

ok=1
grep_ok "$NORM" -qF -- 'cargo check --target x86_64-apple-darwin' && ok=0
report "T6 含 cargo check --target x86_64-apple-darwin 步（AC3①判据）" "$ok"

ok=1
grep_ok "$NORM" -qF -- '-p cloudmount-daemon' && grep_ok "$NORM" -qF -- '-p cloudmount-cli' && ok=0
report "T6 check 覆盖 -p cloudmount-daemon -p cloudmount-cli（workspace 双包）" "$ok"

ok=1
if grep_absent_ok "$NORM" -E '^[[:space:]]*(strategy|matrix):'; then ok=0; fi
report "T6 无 strategy:/matrix:（固定顺序步，与 U2 同口径）" "$ok"

ok=1
grep_ok "$NORM" -qF 'actions/checkout' && ok=0
report "T6 job 含 actions/checkout（骨架对齐 binaries-macos）" "$ok"

ok=1
grep_ok "$NORM" -qF 'Swatinem/rust-cache' && ok=0
report "T6 job 含 Swatinem/rust-cache（随 windows 腿先例）" "$ok"

# ============ T6 附：actionlint 整文件校验 + 拦截手段自校验（不可用时降级并记缺口） ============

if command -v actionlint >/dev/null 2>&1; then
  # 正：check-macos.yml 整文件合法（零错）
  ok=1
  if [ "$wf_ready" -eq 1 ] && actionlint "$WF" >/dev/null 2>&1; then ok=0; fi
  report "T6 actionlint check-macos.yml 零错（整文件合法性）" "$ok"

  # 负（拦截手段自校验）：注入语法错误的临时副本必须被拦截（非零）
  ok=1
  BROKEN="$TMP_ROOT/check-macos.broken.yml"
  if { cat "$NORM"; printf '\n  broken-injection: [unclosed-flow-seq\n'; } >"$BROKEN" && [ -s "$BROKEN" ]; then
    if actionlint "$BROKEN" >/dev/null 2>&1; then
      note "T6: 注入语法错误的副本 actionlint 返回 0（拦截失效）"
    else
      ok=0
    fi
  else
    note "T6: 注入副本构造失败"
  fi
  report "T6 注入语法错误副本被 actionlint 拦截（非零，自校验）" "$ok"
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
          note "T6 降级: python yaml.safe_load 解析 check-macos.yml 失败（YAML 非法）"
        fi
      else
        # 终极降级：grep 键配平粗检（非完整解析，记缺口）
        if grep -q '^on:' "$NORM" && grep -q '^jobs:' "$NORM" \
           && [ "$(grep -cE '^[A-Za-z_][A-Za-z0-9_-]*:' "$NORM")" -ge 3 ]; then
          ok=0
          note "T6 终极降级: pyyaml 不可用，仅 grep 键配平粗检（非完整解析，记缺口）"
        else
          note "T6 终极降级: grep 键配平粗检未过（on:/jobs: 键缺失）"
        fi
      fi
    else
      note "T6 降级: python 不可用且无其他 YAML 解析手段"
    fi
  fi
  report "T6 降级 YAML 结构断言（check-macos.yml 可解析/键配平）" "$ok"
  echo "WARN 降级告警：actionlint 不可用——T6 仅覆盖 YAML 合法性（python yaml.safe_load 或键配平）+ 结构断言；语法拦截自校验（注入负路径）未执行，记缺口（test-strategy：缺口·拦截手段依赖）"
fi

# ============ 汇总 ============
echo "$PASS/$TOTAL passed"
if [ "$FAIL" -ne 0 ]; then
  exit 1
fi
exit 0
