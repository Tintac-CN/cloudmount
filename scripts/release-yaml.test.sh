#!/usr/bin/env bash
#
# scripts/release-yaml.test.sh — U2（macos-release）release.yml 结构断言（TDD，静态 grep/awk 式，不要求 yq）
#
# 被测对象：.github/workflows/release.yml（只读断言；本测试零修改被测文件）
# 契约来源：docs/aidlc/macos-release/units/2.md（断言不得弱化）
#   + prd.md AC1/AC4/AC5 与语义对齐内联小节 + test-strategy.md T9-T13
#
#   T9  存在 binaries-macos: job 定义；runs-on 恰为 macos-15（Q1 pin，单 runner）；
#       job 内含 --target aarch64-apple-darwin 与 --target x86_64-apple-darwin 两处 build 字面
#       （顺序步双 target，Q1 裁决）；job 内无 strategy:/matrix:（gate1 MINOR① 收紧：不采用 matrix）；
#       job 内调用 scripts/package-macos.sh（AC2：CI 与本地同脚本）；
#       调用含 --arch 传参字面（gate2 准出回填 M1：CI 双 tar 命名正确性守护）
#   T10 mac upload 步含 if-no-files-found: error（对齐 linux/windows 腿先例，glob 落空即整体失败）；
#       upload path 含 cloudmount-darwin-*.tar.gz glob 字面（Q2 命名，与脚本输出严格匹配）；
#       upload artifact name 前缀 cloudmount-darwin-（release job 下载 pattern cloudmount-* 可拾取的前提）
#   T11 release job needs 含 binaries-macos（仅追加，mac 腿失败则不发布）；
#       护栏 = 仍含 binaries-linux、binaries-windows（AC5 零删改守护）；
#       release job 块内无 if: 行、无 status 函数（always()/success()/failure()/cancelled()）
#       调用（防 needs 门控语义被覆盖削弱，AC5 fail-fast 不削弱）
#   T12 Release body/产物表区含 mac 条目字面（cloudmount-darwin-aarch64、
#       cloudmount-darwin-x86_64）+ 签名精确措辞（Developer ID / ad-hoc）+
#       Gatekeeper 指引（xattr -cr）（AC4）
#   T13 actionlint 可用时：对 release.yml 零错（整文件合法性）；对注入语法错误的临时副本
#       （mktemp 生成）确认非零（拦截手段自校验）；actionlint 不可用 → 输出
#       `SKIP actionlint（不可用）` + 降级 YAML 解析断言 + 降级告警行（记缺口）
#   R2  守护：on: 保持仅 push tags v*（无 workflow_dispatch；R2-MAJOR 裁决：
#       check job 不落本文件，零 dispatch 副作用）
#
# 行尾注意：release.yml 为 CRLF 检出；断言前统一剥 \r 至临时规范化副本，再行 grep/awk。
# 运行：bash scripts/release-yaml.test.sh（Git Bash 直跑；repo 根或任意工作目录均可）
# 输出：逐断言 `PASS/FAIL <名称>`（诊断走 stderr），末行 `<N>/<M> passed`；任一 FAIL → 退出非零。
# TDD 红阶段说明：release.yml mac 腿未实施时，T9/T10/T11-mac/T12 断言全 FAIL（红态预期）；
#   护栏类断言（T11 既有依赖、if:/status 负断言、R2 on: 守护、T13）现状绿属正常（回归护栏）。

set -u
export LC_ALL=C

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WF="$SCRIPT_DIR/../.github/workflows/release.yml"

TMP_ROOT="$(mktemp -d)"
trap 'rm -rf "$TMP_ROOT"' EXIT

PASS=0
FAIL=0
TOTAL=0

[ -f "$WF" ] || printf '[red] 被测文件不存在：%s —— 相关断言按 FAIL 计\n' "$WF" >&2

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

# ---- 规范化副本：剥 CR（CRLF 检出），后续 grep/awk 全部基于此 ----
NORM="$TMP_ROOT/release.normalized.yml"
if [ -f "$WF" ]; then tr -d '\r' <"$WF" >"$NORM"; fi
wf_ready=0
[ -s "$NORM" ] && wf_ready=1

# job_block <job 名> —— 输出该 job 块：自 `  <名>:`（2 空格缩进）行起，
# 至下一个 2 空格缩进键/分节注释（^  [^ ]）或 0 缩进键（^[^ ]）止（不含）；块不存在输出空
job_block() {
  awk -v pat="^  $1:" '
    inblk && /^  [^ ]/ { exit }
    inblk && /^[^ ]/   { exit }
    inblk { print; next }
    $0 ~ pat { inblk = 1; print }
  ' "$NORM" 2>/dev/null
}

# toplevel_block <键名> —— 输出 0 缩进键块（如 on:），至下一个 0 缩进键止
toplevel_block() {
  awk -v pat="^$1:" '
    inblk && /^[^ ]/ { exit }
    inblk { print; next }
    $0 ~ pat { inblk = 1; print }
  ' "$NORM" 2>/dev/null
}

# release job 块内 needs: 行及其列表续行（兼容 flow [a, b] 与 block 列表两种形态）
release_needs() {
  job_block release | awk '
    /^[[:space:]]*needs:/ { print; inneeds = 1; next }
    inneeds && /^[[:space:]]*$/ { next }
    inneeds && /^[[:space:]]*-[[:space:]]/ { print; next }
    { inneeds = 0 }
  '
}

# grep_ok <文件> <grep 参数...> —— 正断言：文件非空且 grep 命中（空块不计命中，防空真）
grep_ok() {
  local f="$1"; shift
  [ -s "$f" ] || return 1
  grep "$@" "$f" >/dev/null 2>&1
}

# grep_absent_ok <文件> <grep 参数...> —— 负断言：文件非空且 grep 不命中（空块直接 FAIL，防空真）
grep_absent_ok() {
  local f="$1"; shift
  [ -s "$f" ] || return 1
  ! grep "$@" "$f" >/dev/null 2>&1
}

# ---- 物化断言对象块 ----
MAC_BLK="$TMP_ROOT/block.binaries-macos.yml"
REL_BLK="$TMP_ROOT/block.release.yml"
ON_BLK="$TMP_ROOT/block.on.yml"
MAC_UPLOAD="$TMP_ROOT/block.mac-upload.yml"
job_block binaries-macos >"$MAC_BLK"
job_block release >"$REL_BLK"
toplevel_block on >"$ON_BLK"
# mac upload 步：自 actions/upload-artifact 行至块尾（upload 为 mac job 末步，with: 块必随其后）
sed -n '/actions\/upload-artifact/,$p' "$MAC_BLK" >"$MAC_UPLOAD" 2>/dev/null

# ============ T9：binaries-macos job 结构（AC1；Q1 pin + MINOR① 非 matrix 落点） ============

ok=1
if [ "$wf_ready" -eq 1 ] && grep -qE '^  binaries-macos:[[:space:]]*(#.*)?$' "$NORM"; then ok=0; fi
report "T9 binaries-macos: job 定义存在" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ]; then
  runs_total="$(grep -cE '^[[:space:]]*runs-on:' "$MAC_BLK" || true)"
  runs_mac15="$(grep -cE '^[[:space:]]*runs-on:[[:space:]]*macos-15[[:space:]]*$' "$MAC_BLK" || true)"
  if [ "$runs_total" -eq 1 ] && [ "$runs_mac15" -eq 1 ]; then ok=0; else
    note "T9: mac job 内 runs-on 行数=$runs_total，恰为 macos-15 的行数=$runs_mac15"
  fi
fi
report "T9 runs-on 恰为 macos-15（Q1 pin，单 runner 双 target）" "$ok"

ok=1
grep_ok "$MAC_BLK" -qF -- '--target aarch64-apple-darwin' && ok=0
report "T9 含 --target aarch64-apple-darwin build 字面（顺序步双 target）" "$ok"

ok=1
grep_ok "$MAC_BLK" -qF -- '--target x86_64-apple-darwin' && ok=0
report "T9 含 --target x86_64-apple-darwin build 字面（顺序步双 target）" "$ok"

ok=1
if grep_absent_ok "$MAC_BLK" -E '^[[:space:]]*(strategy|matrix):'; then ok=0; fi
report "T9 job 内无 strategy:/matrix:（gate1 MINOR①：不采用 matrix）" "$ok"

ok=1
grep_ok "$MAC_BLK" -qF 'scripts/package-macos.sh' && ok=0
report "T9 job 调用 scripts/package-macos.sh（AC2：CI 与本地同脚本）" "$ok"

ok=1
grep_ok "$MAC_BLK" -qF -- '--arch "${t%%-*}"' && ok=0
report "T9 mac 打包调用含 --arch 传参字面（gate2 准出回填 M1：CI 双 tar 命名守护——丢/错传将产出错名 tar）" "$ok"

# ============ T10：mac upload 步（glob 落空即失败 + Q2 命名严格匹配） ============

ok=1
grep_ok "$MAC_UPLOAD" -qF 'actions/upload-artifact' && ok=0
report "T10 mac upload 步存在（actions/upload-artifact）" "$ok"

ok=1
grep_ok "$MAC_UPLOAD" -qE '^[[:space:]]*if-no-files-found:[[:space:]]*error[[:space:]]*$' && ok=0
report "T10 mac upload 含 if-no-files-found: error（对齐 linux/windows 腿先例）" "$ok"

ok=1
grep_ok "$MAC_UPLOAD" -qF 'cloudmount-darwin-*.tar.gz' && ok=0
report "T10 upload path 含 cloudmount-darwin-*.tar.gz glob 字面（Q2 命名）" "$ok"

ok=1
grep_ok "$MAC_UPLOAD" -qE '^[[:space:]]*name:[[:space:]]*cloudmount-darwin([^[:space:]]|$)' && ok=0
report "T10 upload artifact name 以 cloudmount-darwin 开头（release 下载 pattern 可拾取）" "$ok"

# ============ T11：release job 门控（仅追加 + 零删改/零覆盖护栏，AC5） ============

ok=1
if [ "$wf_ready" -eq 1 ] && release_needs | grep -q 'binaries-macos'; then ok=0; fi
report "T11 release needs 含 binaries-macos（仅追加，mac 腿失败则不发布）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ] && release_needs | grep -q 'binaries-linux'; then ok=0; fi
report "T11 护栏 release needs 仍含 binaries-linux（AC5 零删改）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ] && release_needs | grep -q 'binaries-windows'; then ok=0; fi
report "T11 护栏 release needs 仍含 binaries-windows（AC5 零删改）" "$ok"

ok=1
if grep_absent_ok "$REL_BLK" -E '^[[:space:]]*if:'; then ok=0; fi
report "T11 护栏 release job 块内无 if: 行（防门控覆盖）" "$ok"

ok=1
if grep_absent_ok "$REL_BLK" -E '\b(always|success|failure|cancelled)\(\)'; then ok=0; fi
report "T11 护栏 release job 块内无 status 函数调用（防 fail-fast 削弱）" "$ok"

# ============ R2-MAJOR 守护：on: 保持仅 push tags v*（零 dispatch 副作用） ============

ok=1
if [ -s "$ON_BLK" ] \
   && grep_ok "$ON_BLK" -q 'tags:' \
   && grep_ok "$ON_BLK" -qF 'v*' \
   && grep_absent_ok "$ON_BLK" -q 'workflow_dispatch'; then ok=0; fi
report "R2 守护 on: 保持仅 push tags v*（无 workflow_dispatch，check job 不落本文件）" "$ok"

# ============ T12：Release body 产物表 mac 行 + 签名措辞 + Gatekeeper 指引（AC4） ============

ok=1
grep_ok "$REL_BLK" -qF 'cloudmount-darwin-aarch64' && ok=0
report "T12 产物表含 cloudmount-darwin-aarch64 条目" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'cloudmount-darwin-x86_64' && ok=0
report "T12 产物表含 cloudmount-darwin-x86_64 条目" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'Developer ID' && ok=0
report "T12 含签名精确措辞 Developer ID（无签名/公证声明）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'ad-hoc' && ok=0
report "T12 含 ad-hoc 字样（arm64 链接器 ad-hoc 签名说明）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'xattr -cr' && ok=0
report "T12 含 Gatekeeper 指引 xattr -cr" "$ok"

# ============ T13：actionlint 整文件校验 + 拦截手段自校验（不可用时降级并记缺口） ============

if command -v actionlint >/dev/null 2>&1; then
  # 正：release.yml 整文件合法（零错）
  ok=1
  if [ "$wf_ready" -eq 1 ] && actionlint "$WF" >/dev/null 2>&1; then ok=0; fi
  report "T13 actionlint release.yml 零错（AC5 整文件合法性）" "$ok"

  # 负（拦截手段自校验）：注入语法错误的临时副本必须被拦截（非零）
  ok=1
  BROKEN="$TMP_ROOT/release.broken.yml"
  if { cat "$NORM"; printf '\n  broken-injection: [unclosed-flow-seq\n'; } >"$BROKEN" && [ -s "$BROKEN" ]; then
    if actionlint "$BROKEN" >/dev/null 2>&1; then
      note "T13: 注入语法错误的副本 actionlint 返回 0（拦截失效）"
    else
      ok=0
    fi
  else
    note "T13: 注入副本构造失败"
  fi
  report "T13 注入语法错误副本被 actionlint 拦截（非零，自校验）" "$ok"
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
          note "T13 降级: python yaml.safe_load 解析 release.yml 失败（YAML 非法）"
        fi
      else
        # 终极降级：grep 键配平粗检（非完整解析，记缺口）
        if grep -q '^on:' "$NORM" && grep -q '^jobs:' "$NORM" \
           && [ "$(grep -cE '^[A-Za-z_][A-Za-z0-9_-]*:' "$NORM")" -ge 4 ]; then
          ok=0
          note "T13 终极降级: pyyaml 不可用，仅 grep 键配平粗检（非完整解析，记缺口）"
        else
          note "T13 终极降级: grep 键配平粗检未过（on:/jobs: 键缺失）"
        fi
      fi
    else
      note "T13 降级: python 不可用且无其他 YAML 解析手段"
    fi
  fi
  report "T13 降级 YAML 结构断言（release.yml 可解析/键配平）" "$ok"
  echo "WARN 降级告警：actionlint 不可用——T13 仅覆盖 YAML 合法性（python yaml.safe_load 或键配平）+ T9-T12 结构断言；语法拦截自校验（注入负路径）未执行，记缺口（test-strategy：缺口·拦截手段依赖）"
fi

# ============ 汇总 ============
echo "$PASS/$TOTAL passed"
if [ "$FAIL" -ne 0 ]; then
  exit 1
fi
exit 0
