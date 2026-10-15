#!/usr/bin/env bash
#
# scripts/package-macos.test.sh — U1（macos-release）打包脚本契约测试（TDD）
#
# 被测对象：scripts/package-macos.sh（与本测试文件同目录）
# 契约来源：docs/aidlc/macos-release/units/1.md（断言不得弱化）+ prd.md AC2/错误处理边界
#   用法：package-macos.sh --bin-dir <产物目录> --out <输出目录> [--dry-run]
#   T1 正：dry-run 退出码 0；tar.gz 只写 <out>/dry-run/ 专属子目录（与真实上传路径隔离）；
#         命名 cloudmount-darwin-<arch>.tar.gz（<arch> ∈ aarch64|x86_64，gate1 Q2 裁决）
#   T2 负：占位目录缺 cloudmountd / cloudmount / README.md / edge.md 任一
#         → 非零退出且不产出任何 .tar.gz（硬校验 fail-fast，杜绝残缺 tar）
#   T3 负：缺 --bin-dir 或 --out → 非零退出 + usage 提示（stderr 含 usage 字样即可）
#   T4 正：tar -tzf 清单归一化（剥 ./ 前缀、忽略目录条目）后恰好为根级四项
#         cloudmountd + cloudmount + README.md + edge.md（不多不少，对齐 linux 腿拍平形态）
#   T5 负：清单不含任何 desktop/.app 条目（A1 裁决：desktop 排除范围守护）
#
# 运行：bash scripts/package-macos.test.sh（Git Bash 直跑；repo 根或任意工作目录均可）
# 输出：逐测试 `PASS/FAIL <名称>`（诊断走 stderr），末行 `<N>/<M> passed`；
#       任一 FAIL → 脚本退出非零。
# TDD 红阶段说明：package-macos.sh 尚不存在时，全部测试按 FAIL 计（退出非零为红态预期结果）。

set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PKG="$SCRIPT_DIR/package-macos.sh"

TMP_ROOT="$(mktemp -d)"
trap 'rm -rf "$TMP_ROOT"' EXIT

PASS=0
FAIL=0
TOTAL=0
TARGET_MISSING=1
[ -f "$PKG" ] && TARGET_MISSING=0

if [ "$TARGET_MISSING" -eq 1 ]; then
  printf '[red] 被测脚本不存在：%s —— 以下全部测试按 FAIL 计（TDD 红态预期）\n' "$PKG" >&2
fi

# 被测脚本的调用工作目录（与测试自身 cwd 隔离，兼查 tar 泄漏到 cwd 的失败形态）
CWD_SCRATCH="$TMP_ROOT/cwd"

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

# target_ready —— 被测脚本存在性前置；缺席时该测试直接 FAIL（杜绝缺席导致的空真/恒真断言）
target_ready() { [ "$TARGET_MISSING" -eq 0 ]; }

# make_fixture <目录> —— 构造齐全的占位产物目录（魔数内容 + 可执行位 + 文本占位）
make_fixture() {
  mkdir -p "$1"
  printf '#!/bin/sh\nprintf "cloudmountd-placeholder\\n"\n' >"$1/cloudmountd"
  printf '#!/bin/sh\nprintf "cloudmount-placeholder\\n"\n' >"$1/cloudmount"
  chmod +x "$1/cloudmountd" "$1/cloudmount"
  printf 'README placeholder (macos-release U1 dry-run)\n' >"$1/README.md"
  printf 'edge.md placeholder (macos-release U1 dry-run)\n' >"$1/edge.md"
}

invoke() {  # invoke <被测脚本参数...> —— 在隔离 cwd 中调用被测脚本；RC/OUT/ERR 全局带回
  rm -rf "$CWD_SCRATCH"
  mkdir -p "$CWD_SCRATCH"
  RC=0
  OUT="$(cd "$CWD_SCRATCH" && "$PKG" "$@" 2>"$TMP_ROOT/stderr.cap")" || RC=$?
  ERR="$(cat "$TMP_ROOT/stderr.cap")"
}

run_pkg() {  # run_pkg <bin-dir> <out> [额外参数...]
  local bin="$1" out="$2"
  shift 2
  invoke --bin-dir "$bin" --out "$out" "$@"
}

find_tars() { find "$1" -type f -name '*.tar.gz' 2>/dev/null; }

# normalize_listing —— stdin: tar -tzf 清单 → stdout: 归一化条目
# （剥 ./ 前缀；忽略目录条目如 ./ 与以 / 结尾者；去空行）
normalize_listing() {
  sed -E 's#^(\./)+##' | grep -v -e '/$' -e '^$' || true
}

# ============ T1（正）：dry-run exit 0 + <out>/dry-run/ 隔离产出 + Q2 命名 ============
t1_bin="$TMP_ROOT/t1/bin"
t1_out="$TMP_ROOT/t1/out"
make_fixture "$t1_bin"
run_pkg "$t1_bin" "$t1_out" --dry-run
ok=0
target_ready || ok=1
if [ "$RC" -ne 0 ]; then
  ok=1
  note "T1: 退出码 $RC（期望 0）；stderr: ${ERR:0:200}"
fi
t1_dry="$t1_out/dry-run"
t1_tars=()
if [ -d "$t1_dry" ]; then
  while IFS= read -r f; do t1_tars+=("$f"); done < <(find_tars "$t1_dry")
fi
if [ "${#t1_tars[@]}" -lt 1 ]; then
  ok=1
  note "T1: $t1_dry 下未生成任何 .tar.gz"
fi
for f in ${t1_tars[@]+"${t1_tars[@]}"}; do
  base="$(basename "$f")"
  case "$base" in
    cloudmount-darwin-aarch64.tar.gz | cloudmount-darwin-x86_64.tar.gz) ;;
    *)
      ok=1
      note "T1: 产物命名不符 Q2 裁决（期望 cloudmount-darwin-{aarch64|x86_64}.tar.gz）: $base"
      ;;
  esac
done
# 隔离断言：占位产物与输出只出现在 dry-run 专属路径，不在 <out> 根/任何上传路径
if [ -d "$t1_out" ]; then
  stray="$(find_tars "$t1_out" | grep -Fv "$t1_dry/" | wc -l)"
  if [ "$stray" -ne 0 ]; then
    ok=1
    note "T1: <out> 内 dry-run/ 之外出现 .tar.gz（隔离破坏）: $(find_tars "$t1_out" | grep -Fv "$t1_dry/" | tr '\n' ' ')"
  fi
  rootbins="$(find "$t1_out" -maxdepth 1 -type f \( -name 'cloudmountd' -o -name 'cloudmount' \) | wc -l)"
  if [ "$rootbins" -ne 0 ]; then
    ok=1
    note "T1: <out> 根出现占位产物（隔离破坏）"
  fi
else
  ok=1
  note "T1: <out> 目录未被创建"
fi
cwdtars="$(find_tars "$CWD_SCRATCH" | wc -l)"
if [ "$cwdtars" -ne 0 ]; then
  ok=1
  note "T1: 调用方 cwd 泄漏 .tar.gz"
fi
report "T1 dry-run: exit0 + <out>/dry-run/ 专属产出 + Q2 命名 + 上传路径隔离" "$ok"

# ============ T2（负，×4 参数化）：缺任一必含文件 → 非零退出且零 tar 产出 ============
for victim in cloudmountd cloudmount README.md edge.md; do
  t2_bin="$TMP_ROOT/t2/$victim/bin"
  t2_out="$TMP_ROOT/t2/$victim/out"
  make_fixture "$t2_bin"
  rm -f "$t2_bin/$victim"
  run_pkg "$t2_bin" "$t2_out" --dry-run
  ok=0
  target_ready || ok=1
  if [ "$RC" -eq 0 ]; then
    ok=1
    note "T2[$victim]: 退出码 0，缺必含文件未硬失败"
  fi
  n="$(find_tars "$t2_out" | wc -l)"
  if [ "$n" -ne 0 ]; then
    ok=1
    note "T2[$victim]: 缺文件仍产出 $n 个 .tar.gz（残缺 tar 未杜绝）"
  fi
  ncwd="$(find_tars "$CWD_SCRATCH" | wc -l)"
  if [ "$ncwd" -ne 0 ]; then
    ok=1
    note "T2[$victim]: 调用方 cwd 残留 $ncwd 个 .tar.gz"
  fi
  report "T2 缺 $victim → 非零退出且零 tar 产出" "$ok"
done

# ============ T3（负，×2 参数化）：必填参数缺失 → 非零退出 + usage ============
t3_out="$TMP_ROOT/t3a/out"
invoke --out "$t3_out"
ok=0
target_ready || ok=1
if [ "$RC" -eq 0 ]; then
  ok=1
  note "T3[缺--bin-dir]: 退出码 0"
fi
printf '%s' "$ERR" | grep -qi 'usage' || {
  ok=1
  note "T3[缺--bin-dir]: stderr 未含 usage（实际: ${ERR:0:200}）"
}
report "T3 缺 --bin-dir → 非零退出 + usage 提示" "$ok"

t3_bin="$TMP_ROOT/t3b/bin"
make_fixture "$t3_bin"
invoke --bin-dir "$t3_bin"
ok=0
target_ready || ok=1
if [ "$RC" -eq 0 ]; then
  ok=1
  note "T3[缺--out]: 退出码 0"
fi
printf '%s' "$ERR" | grep -qi 'usage' || {
  ok=1
  note "T3[缺--out]: stderr 未含 usage（实际: ${ERR:0:200}）"
}
report "T3 缺 --out → 非零退出 + usage 提示" "$ok"

# ============ T4（正）：tar 清单归一化后恰为根级四项，不多不少 ============
t4_bin="$TMP_ROOT/t4/bin"
t4_out="$TMP_ROOT/t4/out"
make_fixture "$t4_bin"
run_pkg "$t4_bin" "$t4_out" --dry-run
ok=0
target_ready || ok=1
t4_tars=()
while IFS= read -r f; do t4_tars+=("$f"); done < <(find_tars "$t4_out")
if [ "${#t4_tars[@]}" -lt 1 ]; then
  ok=1
  note "T4: 无 .tar.gz 可断言（dry-run 退出码 $RC）"
fi
expected="$(printf '%s\n' cloudmountd cloudmount README.md edge.md | LC_ALL=C sort)"
t4_listing_all=""
for f in ${t4_tars[@]+"${t4_tars[@]}"}; do
  actual="$(tar -tzf "$f" 2>/dev/null | normalize_listing | LC_ALL=C sort)"
  t4_listing_all="$t4_listing_all$actual"$'\n'
  if [ "$actual" != "$expected" ]; then
    ok=1
    note "T4: $(basename "$f") 归一化清单 ≠ 恰好根级四项"
    note "T4:   期望: $(printf '%s' "$expected" | tr '\n' ' ')"
    note "T4:   实际: $(printf '%s' "$actual" | tr '\n' ' ')"
  fi
done
report "T4 tar 清单归一化后恰为 cloudmountd+cloudmount+README.md+edge.md（不多不少）" "$ok"

# ============ T5（负/范围排除）：清单不含任何 desktop/.app 条目 ============
ok=0
target_ready || ok=1
if [ "${#t4_tars[@]}" -lt 1 ]; then
  ok=1
  note "T5: 无 tar 清单可断言（前置 dry-run 未产出）"
else
  while IFS= read -r entry; do
    [ -n "$entry" ] || continue
    case "$entry" in
      *desktop* | *.app | *.app/*)
        ok=1
        note "T5: 发现 desktop/.app 条目: $entry"
        ;;
    esac
  done <<<"$t4_listing_all"
fi
report "T5 清单不含任何 desktop/.app 条目（A1 范围排除）" "$ok"

# ============ 汇总 ============
echo "$PASS/$TOTAL passed"
if [ "$FAIL" -ne 0 ]; then
  exit 1
fi
exit 0
