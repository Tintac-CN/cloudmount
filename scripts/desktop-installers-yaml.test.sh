#!/usr/bin/env bash
#
# scripts/desktop-installers-yaml.test.sh — desktop-installers U1：desktop 交付面静态结构断言
#   （TDD，静态 grep/awk 式，不要求 yq；Git Bash 直跑；repo 根或任意工作目录均可）
#
# 被测对象（五）：
#   ① .github/workflows/release.yml   —— desktop 面未实施（无追加步/无 dmg job/body 7 行）→ desktop 断言红
#   ② packaging/wix/main.wxs          —— 无 desktop 组件/快捷方式/Icon → TP-04/05/12 断言红
#   ③ scripts/package-dmg-macos.sh    —— 红阶段不存在 → 相关断言带「文件存在」前置，缺失即 FAIL（防空真）
#   ④ apps/desktop/src/main.rs        —— 存在但零托盘接线（tray-icon 已声明未使用）→ TP-09 断言红
#   ⑤ apps/desktop/icons/ + build.rs  —— 红阶段不存在 → TP-06/11/12 相关断言 FAIL（缺失不空真）
#
# 契约来源：docs/aidlc/desktop-installers/units/1.md（断言不得弱化）
#   + prd.md AC1-AC6 与语义对齐内联小节 + test-strategy.md TP-01..TP-24（gate1 r2/r3 修订口径）
#
# ── 机制复制声明（先例 = 已 TDD 锁定的 scripts/release-yaml.test.sh，只读） ──
#   job_block/toplevel_block/release_needs（先例 L70-98）、grep_ok/grep_absent_ok 防空真（L100-112）、
#   CRLF 剥除规范化副本（L64-68）、actionlint 降级（SKIP + python yaml.safe_load → 键配平粗检 + WARN，
#   L232-283）——均复制自上述先例；upload_step_block 另参照 scripts/installers-yaml.test.sh L128-136
#   （同为锁定先例，只读参照）。同步义务：先例机制若变更，本文件须跟进同步。
#   set -u 沿先例（-e/-o pipefail 会使「[ -s ] && 短路」与「grep -q 管道」变中途退出/SIGPIPE 假阴性）。
#
# ── Construction 落字面（gate1 未钉死处，本脚本红阶段钉死，实现据此红绿闭环——沿 installers-yaml 先例） ──
#   新 mac job 名 = installer-dmg-macos（镜像 installer-msi-windows/installer-pkg-macos 命名先例）
#   dmg 产物名 = cloudmount-desktop.dmg（prd AC4② 建议名 Construction 落定）
#   desktop 构建形态 = --manifest-path apps/desktop/Cargo.toml（plan U3 口径；prd AC5① 的 cd 形态不采）
#   托盘菜单 id = "show"/"quit"（标签「显示主窗口」「退出」为 prd AC2② 钉死；id 为本脚本落字面）
#   图标产物名 = icon.ico / 32x32.png / 128x128.png / icons.rs（沿 gen_icons 既有基线命名，prd AC1①）
#
# ── TP 归属注记 ──
#   TP-13..TP-17：本地 WiX 构建/msiexec 装卸/快捷方式在位/exe 图标/卸载干净——集成面归 U3；
#     本脚本仅承载 TP-13 的 wxs 字面子集（TP-04 段 desktop 组件断言）与 TP-18 的 needs/upload
#     结构子集（TP-01/TP-02 段），msiexec/本地安装行为断言不落本文件。
#   TP-19..TP-23：真机人工/集成回归——非本脚本静态面。
#   TP-11 icns 子断言：首日验证③定稿（stdlib 手写 PNG-in-ICNS 预生成入仓，decisions.md U2 条目）
#     后启用——原定「定稿后补启用」，gate2 M2 准出回填兑现（UNLOCK-TESTS 2026-10-16 人类授权）。
#   TP-03「改动后快照对照」以字面锚定集承载（本脚本录基线字面；U5 终验另做全量基线比对）。
#
# 行尾注意：release.yml/main.wxs 等为 CRLF 检出；断言前统一剥 \r 至临时规范化副本，再行 grep/awk。
# 输出：逐断言 `PASS/FAIL <名称>`（诊断走 stderr），末行 `<N>/<M> passed`；任一 FAIL → 退出非零。
# TDD 红阶段预期：desktop 相关断言全 FAIL（被测对象不存在或零接线）；
#   护栏类断言（既有四 job/产物字面/body 7 行/on: tags/收编链/wxs daemon/cli 既有字面/pkg 无 .app/
#   SmartScreen+Gatekeeper/脚本自检/actionlint 降级）现状绿属正常（回归护栏）。

set -u
export LC_ALL=C

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WF="$SCRIPT_DIR/../.github/workflows/release.yml"
WXS="$SCRIPT_DIR/../packaging/wix/main.wxs"
DMGS="$SCRIPT_DIR/../scripts/package-dmg-macos.sh"
MAINRS="$SCRIPT_DIR/../apps/desktop/src/main.rs"
PKGS="$SCRIPT_DIR/../scripts/package-pkg-macos.sh"
GENICONS="$SCRIPT_DIR/../scripts/gen_icons.py"
BUILD_RS="$SCRIPT_DIR/../apps/desktop/build.rs"
ICONS_DIR="$SCRIPT_DIR/../apps/desktop/icons"
ICO_FILE="$ICONS_DIR/icon.ico"
PNG32="$ICONS_DIR/32x32.png"
PNG128="$ICONS_DIR/128x128.png"
ICONS_RS="$ICONS_DIR/icons.rs"
ICNS_FILE="$ICONS_DIR/icon.icns"

TMP_ROOT="$(mktemp -d)"
trap 'rm -rf "$TMP_ROOT"' EXIT

PASS=0
FAIL=0
TOTAL=0

[ -f "$WF" ] || printf '[red] 被测文件不存在：%s —— 相关断言按 FAIL 计\n' "$WF" >&2
[ -f "$WXS" ] || printf '[red] 被测文件不存在（红态预期）：%s —— TP-04/05/12 相关断言 FAIL（缺失不空真）\n' "$WXS" >&2
[ -f "$DMGS" ] || printf '[red] 被测文件不存在（红态预期）：%s —— TP-07/12 相关断言 FAIL（缺失不空真）\n' "$DMGS" >&2
[ -f "$MAINRS" ] || printf '[red] 被测文件不存在（红态预期）：%s —— TP-09 相关断言 FAIL（缺失不空真）\n' "$MAINRS" >&2
[ -d "$ICONS_DIR" ] || printf '[red] 目录不存在（红态预期）：%s —— TP-06/11 相关断言 FAIL（缺失不空真）\n' "$ICONS_DIR" >&2
[ -f "$BUILD_RS" ] || printf '[red] 被测文件不存在（红态预期）：%s —— TP-12 相关断言 FAIL（缺失不空真）\n' "$BUILD_RS" >&2

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

# ---- 规范化副本：剥 CR（CRLF 检出），后续 grep/awk 全部基于此（复制自 release-yaml.test.sh L64-68） ----
NORM="$TMP_ROOT/release.normalized.yml"
if [ -f "$WF" ]; then tr -d '\r' <"$WF" >"$NORM"; fi
wf_ready=0
[ -s "$NORM" ] && wf_ready=1

WXS_NORM="$TMP_ROOT/main.wxs.normalized"
if [ -f "$WXS" ]; then tr -d '\r' <"$WXS" >"$WXS_NORM"; fi

DMGS_NORM="$TMP_ROOT/package-dmg-macos.normalized.sh"
if [ -f "$DMGS" ]; then tr -d '\r' <"$DMGS" >"$DMGS_NORM"; fi

MAINRS_NORM="$TMP_ROOT/main.rs.normalized"
if [ -f "$MAINRS" ]; then tr -d '\r' <"$MAINRS" >"$MAINRS_NORM"; fi

PKGS_NORM="$TMP_ROOT/package-pkg-macos.normalized.sh"
if [ -f "$PKGS" ]; then tr -d '\r' <"$PKGS" >"$PKGS_NORM"; fi

GENICONS_NORM="$TMP_ROOT/gen-icons.normalized.py"
if [ -f "$GENICONS" ]; then tr -d '\r' <"$GENICONS" >"$GENICONS_NORM"; fi

# job_block <job 名> —— 输出该 job 块：自 `  <名>:`（2 空格缩进）行起，
# 至下一个 2 空格缩进键/分节注释（^  [^ ]）或 0 缩进键（^[^ ]）止（不含）；块不存在输出空
# （复制自 release-yaml.test.sh L72-79）
job_block() {
  awk -v pat="^  $1:" '
    inblk && /^  [^ ]/ { exit }
    inblk && /^[^ ]/   { exit }
    inblk { print; next }
    $0 ~ pat { inblk = 1; print }
  ' "$NORM" 2>/dev/null
}

# toplevel_block <键名> —— 输出 0 缩进键块（如 on:），至下一个 0 缩进键止
# （复制自 release-yaml.test.sh L82-88）
toplevel_block() {
  awk -v pat="^$1:" '
    inblk && /^[^ ]/ { exit }
    inblk { print; next }
    $0 ~ pat { inblk = 1; print }
  ' "$NORM" 2>/dev/null
}

# release job 块内 needs: 行及其列表续行（兼容 flow [a, b] 与 block 列表两种形态）
# （复制自 release-yaml.test.sh L91-98）
release_needs() {
  job_block release | awk '
    /^[[:space:]]*needs:/ { print; inneeds = 1; next }
    inneeds && /^[[:space:]]*$/ { next }
    inneeds && /^[[:space:]]*-[[:space:]]/ { print; next }
    { inneeds = 0 }
  '
}

# grep_ok <文件> <grep 参数...> —— 正断言：文件非空且 grep 命中（空块不计命中，防空真）
# （复制自 release-yaml.test.sh L100-105）
grep_ok() {
  local f="$1"; shift
  [ -s "$f" ] || return 1
  grep "$@" "$f" >/dev/null 2>&1
}

# grep_absent_ok <文件> <grep 参数...> —— 负断言：文件非空且 grep 不命中（空块直接 FAIL，防空真）
# （复制自 release-yaml.test.sh L107-112）
grep_absent_ok() {
  local f="$1"; shift
  [ -s "$f" ] || return 1
  ! grep "$@" "$f" >/dev/null 2>&1
}

# upload_step_block <jobblk 文件> —— 自 actions/upload-artifact 行起，
# 至下一个 step 项（6 空格缩进 `- `）或块尾止（不依赖 upload 是否为 job 末步）
# （参照 installers-yaml.test.sh L128-136，只读先例）
upload_step_block() {
  awk '
    grab && /^      - / && $0 !~ /actions\/upload-artifact/ { exit }
    /actions\/upload-artifact/ { grab = 1 }
    grab { print }
  ' "$1" 2>/dev/null
}

# steps_containing <jobblk 文件> <ERE> —— 输出「段内命中 ERE」的完整 step 段
# （本 intent 新增助手，沿用先例 awk 段提取风格；用于圈定 desktop 追加步做 TP-02 负断言）
steps_containing() {
  awk -v ere="$2" '
    /^      - / {
      if (inseg && seg ~ ere) printf "%s", seg
      inseg = 1; seg = ""
    }
    inseg { seg = seg $0 "\n" }
    END { if (inseg && seg ~ ere) printf "%s", seg }
  ' "$1" 2>/dev/null
}

# check_png <文件> <期望边长十六进制 8 位（如 00000020）> —— PNG magic + IHDR 宽高核验（0=有效）
check_png() {
  local f="$1" want="$2" hex
  [ -s "$f" ] || return 1
  hex="$(od -An -tx1 -N24 "$f" 2>/dev/null | tr -d ' \n')"
  [ "${#hex}" -eq 48 ] || return 1
  [ "${hex:0:16}" = "89504e470d0a1a0a" ] || return 1
  [ "${hex:32:8}" = "$want" ] || return 1
  [ "${hex:40:8}" = "$want" ] || return 1
  return 0
}

# ---- 物化断言对象块 ----
MSI_BLK="$TMP_ROOT/block.installer-msi-windows.yml"
DMG_BLK="$TMP_ROOT/block.installer-dmg-macos.yml"
PKG_BLK="$TMP_ROOT/block.installer-pkg-macos.yml"
LIN_BLK="$TMP_ROOT/block.binaries-linux.yml"
WIN_BLK="$TMP_ROOT/block.binaries-windows.yml"
MAC_BLK="$TMP_ROOT/block.binaries-macos.yml"
REL_BLK="$TMP_ROOT/block.release.yml"
ON_BLK="$TMP_ROOT/block.on.yml"
job_block installer-msi-windows >"$MSI_BLK"
job_block installer-dmg-macos >"$DMG_BLK"
job_block installer-pkg-macos >"$PKG_BLK"
job_block binaries-linux >"$LIN_BLK"
job_block binaries-windows >"$WIN_BLK"
job_block binaries-macos >"$MAC_BLK"
job_block release >"$REL_BLK"
toplevel_block on >"$ON_BLK"

MSI_UPLOAD="$TMP_ROOT/block.msi-upload.yml"
DMG_UPLOAD="$TMP_ROOT/block.dmg-upload.yml"
upload_step_block "$MSI_BLK" >"$MSI_UPLOAD"
upload_step_block "$DMG_BLK" >"$DMG_UPLOAD"

# desktop 追加步圈定：installer-msi-windows job 内段体含 desktop 字面的完整 step 段
# （B2 裁决允许该 job 追加 desktop 构建与归集步——TP-02 负断言的限定范围）
MSI_DESK_STEPS="$TMP_ROOT/msi-desktop-steps.txt"
steps_containing "$MSI_BLK" 'desktop' >"$MSI_DESK_STEPS"

# ============ TP-01：release.yml desktop 面（AC5；B2 追加步 + 新增 dmg job + body 8 行锚） ============

ok=1
if [ "$wf_ready" -eq 1 ] && grep -qE '^  installer-dmg-macos:[[:space:]]*(#.*)?$' "$NORM"; then ok=0; fi
report "TP-01 installer-dmg-macos: job 定义存在（新增 mac job，prd AC5①）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ]; then
  runs_total="$(grep -cE '^[[:space:]]*runs-on:' "$DMG_BLK" || true)"
  runs_mac15="$(grep -cE '^[[:space:]]*runs-on:[[:space:]]*macos-15[[:space:]]*$' "$DMG_BLK" || true)"
  if [ "$runs_total" -eq 1 ] && [ "$runs_mac15" -eq 1 ]; then ok=0; else
    note "TP-01: dmg job 内 runs-on 行数=$runs_total，恰为 macos-15 的行数=$runs_mac15"
  fi
fi
report "TP-01 dmg job runs-on 恰为 macos-15（单 runner）" "$ok"

ok=1
grep_ok "$DMG_BLK" -qF -- '--manifest-path apps/desktop/Cargo.toml' && ok=0
report "TP-01 dmg job desktop 构建字面（--manifest-path apps/desktop/Cargo.toml，独立 cargo 项目）" "$ok"

ok=1
grep_ok "$DMG_BLK" -qF 'scripts/package-dmg-macos.sh' && ok=0
report "TP-01 dmg job 调用 scripts/package-dmg-macos.sh（CI 与本地同脚本）" "$ok"

ok=1
if grep_absent_ok "$DMG_BLK" -qF -- 'x86_64-apple-darwin'; then ok=0; fi
report "TP-01 负 dmg job 无 x86_64-apple-darwin 字面（dmg=aarch64-only 裁决，Intel 无 desktop 交付为已知取舍）" "$ok"

ok=1
grep_ok "$DMG_UPLOAD" -qF 'actions/upload-artifact' && ok=0
report "TP-01 dmg upload 步存在（actions/upload-artifact）" "$ok"

ok=1
grep_ok "$DMG_UPLOAD" -qE '^[[:space:]]*if-no-files-found:[[:space:]]*error[[:space:]]*$' && ok=0
report "TP-01/TP-18 dmg upload 含 if-no-files-found: error（产物缺失不静默发布——upload 结构子集）" "$ok"

ok=1
grep_ok "$DMG_UPLOAD" -qF 'cloudmount-desktop.dmg' && ok=0
report "TP-01 dmg upload path 含 cloudmount-desktop.dmg 精确字面（不用 glob）" "$ok"

ok=1
if grep_absent_ok "$DMG_UPLOAD" -qF '*.dmg'; then ok=0; fi
report "TP-01 负 dmg upload 不用 *.dmg glob（精确路径口径沿上 intent 先例）" "$ok"

ok=1
grep_ok "$DMG_UPLOAD" -qE '^[[:space:]]*name:[[:space:]]*cloudmount-([^[:space:]]|$)' && ok=0
report "TP-01 dmg upload artifact name 以 cloudmount- 开头（release 下载 pattern 收编前提）" "$ok"

ok=1
grep_ok "$MSI_BLK" -qF -- '--manifest-path apps/desktop/Cargo.toml' && ok=0
report "TP-01 MSI 追加步 desktop 构建字面（installer-msi-windows 内追加，B2 裁决）" "$ok"

ok=1
grep_ok "$MSI_BLK" -qE '(cp|Copy-Item)[^#]*cloudmount-desktop\.exe' && ok=0
report "TP-01 MSI 追加步 desktop exe 归集字面（cp/Copy-Item cloudmount-desktop.exe 入 BinDir）" "$ok"

ok=1
if [ -s "$MSI_BLK" ]; then
  desk_ln="$(grep -n -- '--manifest-path apps/desktop/Cargo.toml' "$MSI_BLK" | head -1 | cut -d: -f1)"
  verify_ln="$(grep -n -- 'test -f target/release/cloudmountd.exe' "$MSI_BLK" | head -1 | cut -d: -f1)"
  candle_ln="$(grep -n 'candle' "$MSI_BLK" | head -1 | cut -d: -f1)"
  if [ -n "$desk_ln" ] && [ -n "$verify_ln" ] && [ -n "$candle_ln" ] \
     && [ "$desk_ln" -lt "$verify_ln" ] && [ "$desk_ln" -lt "$candle_ln" ]; then
    ok=0
  else
    note "TP-01: 追加步次序行号 desk=$desk_ln verify=$verify_ln candle=$candle_ln（desk 须最小）"
  fi
fi
report "TP-01 MSI 追加步次序：desktop 构建先于既有校验/candle 步（prd AC3 次序约束——纯尾部追加产不出三件套）" "$ok"

ok=1
if [ -s "$ON_BLK" ] \
   && grep_ok "$ON_BLK" -q 'tags:' \
   && grep_ok "$ON_BLK" -qF 'v*' \
   && grep_absent_ok "$ON_BLK" -q 'workflow_dispatch'; then ok=0; fi
report "TP-01 tag 触发守护 on: 仅 push tags v*（无 workflow_dispatch，触发语义零改动）" "$ok"

# ---- body 产物表 8 行锚（AC5⑤：既有 7 行零删改 + MSI 行三件套描述 + .dmg 行 + Gatekeeper 绑定） ----

ok=1
grep_ok "$REL_BLK" -qF 'cloudmount-linux-*-musl.tar.gz' && ok=0
report "TP-01 body 既有行1 linux tar（零删改锚）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'cloudmount-windows-x64.zip' && ok=0
report "TP-01 body 既有行2 windows zip（零删改锚）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'cloudmount-darwin-aarch64.tar.gz' && ok=0
report "TP-01 body 既有行3 darwin aarch64 tar（零删改锚）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'cloudmount-darwin-x86_64.tar.gz' && ok=0
report "TP-01 body 既有行4 darwin x86_64 tar（零删改锚）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'cloudmount-windows-x64.msi' && ok=0
report "TP-01 body 既有行5 windows msi（零删改锚——行在位，描述更新为 AC5⑤ 授权项）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'cloudmount-darwin-aarch64.pkg' && ok=0
report "TP-01 body 既有行6 darwin aarch64 pkg（零删改锚）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'cloudmount-darwin-x86_64.pkg' && ok=0
report "TP-01 body 既有行7 darwin x86_64 pkg（零删改锚）" "$ok"

ok=1
if [ -s "$REL_BLK" ]; then
  msi_row="$(grep -F 'cloudmount-windows-x64.msi' "$REL_BLK" | grep -F '|' | head -1)"
  if [ -n "$msi_row" ] && printf '%s\n' "$msi_row" | grep -qi 'desktop'; then ok=0; fi
fi
report "TP-01 body MSI 行三件套描述（MSI 表行含 desktop——AC5⑤ 授权更新项）" "$ok"

ok=1
if [ -s "$REL_BLK" ]; then
  dmg_row="$(grep -F 'cloudmount-desktop.dmg' "$REL_BLK" | head -1)"
  if [ -n "$dmg_row" ] && printf '%s\n' "$dmg_row" | grep -qF '|'; then ok=0; fi
fi
report "TP-01 body .dmg 行存在（产物表 8 行口径追加行）" "$ok"

ok=1
if [ -s "$REL_BLK" ]; then
  dmg_row="$(grep -F 'cloudmount-desktop.dmg' "$REL_BLK" | head -1)"
  if [ -n "$dmg_row" ] && printf '%s\n' "$dmg_row" | grep -q 'Gatekeeper'; then ok=0; fi
fi
report "TP-01 body Gatekeeper 引导绑定 .dmg 行（防朴素 grep 假绿——尾注 3 处 Gatekeeper 字面不计）" "$ok"

# ============ TP-02：追加步/新 job 无吞错无绕过 + needs 链完整（AC5） ============

ok=1
if grep_absent_ok "$MSI_DESK_STEPS" -F 'continue-on-error'; then ok=0; fi
report "TP-02 负 MSI desktop 追加步无 step 级 continue-on-error（防构建失败被吞）" "$ok"

ok=1
if grep_absent_ok "$MSI_DESK_STEPS" -E '^[[:space:]]*if:'; then ok=0; fi
report "TP-02 负 MSI desktop 追加步无 step 级绕过 if:（如 if: always()——防门控旁路）" "$ok"

ok=1
if grep_absent_ok "$DMG_BLK" -F 'continue-on-error'; then ok=0; fi
report "TP-02 负 dmg job 无 continue-on-error（job/步级均无，防吞错）" "$ok"

ok=1
if grep_absent_ok "$DMG_BLK" -E '^[[:space:]]*if:'; then ok=0; fi
report "TP-02 负 dmg job 无绕过 if: 行（防门控覆盖）" "$ok"

ok=1
if grep_absent_ok "$DMG_BLK" -E '\b(always|success|failure|cancelled)\(\)'; then ok=0; fi
report "TP-02 负 dmg job 无 status 函数调用（防 fail-fast 削弱）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ] && release_needs | grep -q 'installer-dmg-macos'; then ok=0; fi
report "TP-02/TP-18 release needs 含 installer-dmg-macos（desktop 腿失败则不发布——needs 链完整；TP-18 线上门控结构子集，本地失败注入归 U3）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ] && release_needs | grep -q 'binaries-linux'; then ok=0; fi
report "TP-02 护栏 release needs 仍含 binaries-linux（既有 needs 元素零删改）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ] && release_needs | grep -q 'binaries-windows'; then ok=0; fi
report "TP-02 护栏 release needs 仍含 binaries-windows（既有 needs 元素零删改）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ] && release_needs | grep -q 'binaries-macos'; then ok=0; fi
report "TP-02 护栏 release needs 仍含 binaries-macos（既有 needs 元素零删改）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ] && release_needs | grep -q 'installer-msi-windows'; then ok=0; fi
report "TP-02 护栏 release needs 仍含 installer-msi-windows（既有 needs 元素零删改）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ] && release_needs | grep -q 'installer-pkg-macos'; then ok=0; fi
report "TP-02 护栏 release needs 仍含 installer-pkg-macos（既有 needs 元素零删改）" "$ok"

# ============ TP-03：既有产物与 job 零删改护栏（AC3/AC6；B2 基线口径 + 字面锚定集） ============

ok=1
if [ "$wf_ready" -eq 1 ] && grep -qE '^  binaries-linux:[[:space:]]*(#.*)?$' "$NORM"; then ok=0; fi
report "TP-03 护栏 binaries-linux: job 定义在位（精确四 job 清单，零删改）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ] && grep -qE '^  binaries-windows:[[:space:]]*(#.*)?$' "$NORM"; then ok=0; fi
report "TP-03 护栏 binaries-windows: job 定义在位（精确四 job 清单，零删改）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ] && grep -qE '^  binaries-macos:[[:space:]]*(#.*)?$' "$NORM"; then ok=0; fi
report "TP-03 护栏 binaries-macos: job 定义在位（精确四 job 清单，零删改）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ] && grep -qE '^  installer-pkg-macos:[[:space:]]*(#.*)?$' "$NORM"; then ok=0; fi
report "TP-03 护栏 installer-pkg-macos: job 定义在位（精确四 job 清单，零删改）" "$ok"

ok=1
grep_ok "$LIN_BLK" -qF -- '-p cloudmount-daemon -p cloudmount-cli' && ok=0
report "TP-03 护栏 binaries-linux daemon/cli 构建字面（daemon/cli 字面锚定）" "$ok"

ok=1
grep_ok "$WIN_BLK" -qF 'cloudmount-windows-x64.zip' && ok=0
report "TP-03 护栏 binaries-windows zip 产物字面（daemon/cli 字面锚定）" "$ok"

ok=1
grep_ok "$MAC_BLK" -qF 'scripts/package-macos.sh' && ok=0
report "TP-03 护栏 binaries-macos package-macos.sh 调用字面（daemon/cli 字面锚定）" "$ok"

ok=1
grep_ok "$PKG_BLK" -qF 'cloudmount-darwin-aarch64.pkg' && grep_ok "$PKG_BLK" -qF 'cloudmount-darwin-x86_64.pkg' && ok=0
report "TP-03 护栏 installer-pkg-macos 双 arch pkg 产物字面（daemon/cli 字面锚定）" "$ok"

ok=1
grep_ok "$PKG_BLK" -qF 'scripts/package-pkg-macos.sh' && ok=0
report "TP-03 护栏 installer-pkg-macos package-pkg-macos.sh 调用字面（daemon/cli 字面锚定）" "$ok"

ok=1
grep_ok "$MSI_BLK" -qF -- '-p cloudmount-daemon -p cloudmount-cli' && ok=0
report "TP-03 护栏 MSI job daemon/cli 构建字面（B2：允许追加步，daemon/cli 字面不删改）" "$ok"

ok=1
grep_ok "$MSI_BLK" -qF -- 'test -f target/release/cloudmountd.exe' && grep_ok "$MSI_BLK" -qF -- 'test -f target/release/cloudmount.exe' && ok=0
report "TP-03 护栏 MSI job 既有构建输入校验字面（双 exe test -f）" "$ok"

ok=1
grep_ok "$MSI_BLK" -qF 'cloudmount-windows-x64.msi' && ok=0
report "TP-03 护栏 MSI job 既有产物名 cloudmount-windows-x64.msi" "$ok"

ok=1
grep_ok "$MSI_BLK" -qF 'candle' && grep_ok "$MSI_BLK" -qE '\blight\b' && ok=0
report "TP-03 护栏 MSI job WiX candle+light 字面（既有构建链零删改）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'pattern: cloudmount-*' && ok=0
report "TP-03 护栏 收编链 pattern: cloudmount-*（release 下载步零改）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'merge-multiple: true' && ok=0
report "TP-03 护栏 收编链 merge-multiple: true（release 下载步零改）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'files: dist/*' && ok=0
report "TP-03 护栏 收编链 files: dist/*（release 上传步零改）" "$ok"

ok=1
grep_ok "$MSI_UPLOAD" -qF 'cloudmount-windows-x64.msi' && ok=0
report "TP-03 护栏 MSI upload path 精确字面（三件套 MSI 仍经既有精确路径上传）" "$ok"

ok=1
grep_ok "$MSI_UPLOAD" -qE '^[[:space:]]*if-no-files-found:[[:space:]]*error[[:space:]]*$' && ok=0
report "TP-03 护栏 MSI upload if-no-files-found: error（产物缺失不静默发布）" "$ok"

ok=1
if grep_absent_ok "$MSI_BLK" -E '^[[:space:]]*if:'; then ok=0; fi
report "TP-03 护栏 MSI job 块内无 if: 行（防门控覆盖）" "$ok"

ok=1
if grep_absent_ok "$MSI_BLK" -E '\b(always|success|failure|cancelled)\(\)'; then ok=0; fi
report "TP-03 护栏 MSI job 块内无 status 函数调用" "$ok"

ok=1
if grep_absent_ok "$REL_BLK" -E '^[[:space:]]*if:'; then ok=0; fi
report "TP-03 护栏 release job 块内无 if: 行（防门控覆盖）" "$ok"

ok=1
if grep_absent_ok "$REL_BLK" -E '\b(always|success|failure|cancelled)\(\)'; then ok=0; fi
report "TP-03 护栏 release job 块内无 status 函数调用（防 fail-fast 削弱）" "$ok"

# ============ TP-04：main.wxs desktop 组件 + daemon/cli 原样 + 32 位负断言（AC3） ============

ok=1
[ -s "$WXS_NORM" ] && ok=0
report "TP-04 packaging/wix/main.wxs 存在（前置）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'cloudmount-desktop.exe' && ok=0
report "TP-04/TP-13 main.wxs 含 cloudmount-desktop.exe desktop 组件字面（铺 Program Files\\CloudMount；TP-13 wxs 字面子集——本地构建面归 U3）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'cloudmountd.exe' && ok=0
report "TP-04 护栏 daemon 组件字面 cloudmountd.exe 原样（AC3④ 零回归）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'cloudmount.exe' && ok=0
report "TP-04 护栏 cli 组件字面 cloudmount.exe 原样（AC3④ 零回归）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'ProgramFiles64Folder' && ok=0
report "TP-04 护栏 ProgramFiles64Folder 字面（x64 包 64 位标准目录）" "$ok"

ok=1
if [ -s "$WXS_NORM" ]; then
  # Id 定界符匹配（契约口径）：ProgramFilesFolder 为 ProgramFiles64Folder 子串，且本文件头注/行内注释
  # 含 T16 实证叙述的裸字面「ProgramFilesFolder」（弃用理由留痕）——裸字面 grep 会假红，
  # 故按「使用形态」断言：Directory 声明（Id="ProgramFilesFolder"）或属性引用（[ProgramFilesFolder]）。
  if grep -qE 'Id="ProgramFilesFolder"|\[ProgramFilesFolder\]' "$WXS_NORM"; then
    note "TP-04: 检出 32 位目录使用字面（Id= 声明或 [] 属性引用形态；T16 实证回归防护）"
  else
    ok=0
  fi
fi
report "TP-04 负 无 32 位目录使用字面（Id 定界+[] 引用两形态——子串与注释叙述字面均不误报）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'Name="CloudMount"' && ok=0
report "TP-04 护栏 Name=\"CloudMount\"（安装目录名，AC3④）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'Environment' && grep_ok "$WXS_NORM" -qF 'System="yes"' && ok=0
report "TP-04 护栏 Machine PATH 载体（Environment 元素+System=\"yes\"，AC3④）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'MajorUpgrade' && grep_ok "$WXS_NORM" -qF 'UpgradeCode' && ok=0
report "TP-04 护栏 MajorUpgrade+UpgradeCode 字面（升级识别链零删改，AC3④）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'Manufacturer=' && grep_ok "$WXS_NORM" -qF 'InstallScope="perMachine"' && ok=0
report "TP-04 护栏 Manufacturer+perMachine 字面（ARP 元数据/perMachine 安装，AC3④）" "$ok"

ok=1
if [ -f "$WXS" ]; then
  bom="$(head -c 3 "$WXS" | od -An -tx1 | tr -d ' \n')"
  [ "$bom" = "efbbbf" ] && ok=0
fi
report "TP-04 护栏 UTF-8 BOM 在位（MINOR⑫ 中文元数据风险规避）" "$ok"

# ============ TP-05：两处 Shortcut 指向 desktop exe + 负不指 daemon/cli（AC3；Q2 裁决） ============

ok=1
if [ -s "$WXS_NORM" ]; then
  n_shortcut="$(grep -o '<Shortcut' "$WXS_NORM" | wc -l | tr -d '[:space:]')"
  if [ "$n_shortcut" = "2" ]; then ok=0; else note "TP-05: <Shortcut 出现次数=$n_shortcut（期望恰 2）"; fi
fi
report "TP-05 Shortcut 元素恰两处（开始菜单+桌面，Q2 裁决）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'ProgramMenuFolder' && ok=0
report "TP-05 开始菜单标准目录字面 ProgramMenuFolder" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'DesktopFolder' && ok=0
report "TP-05 桌面标准目录字面 DesktopFolder" "$ok"

ok=1
if [ -s "$WXS_NORM" ]; then
  n_target="$(grep -- '<Shortcut' "$WXS_NORM" | grep -c 'cloudmount-desktop\.exe' | tr -d '[:space:]')"
  if [ "$n_target" = "2" ]; then ok=0; else note "TP-05: 指向 desktop exe 的 Shortcut 行数=$n_target（期望 2）"; fi
fi
report "TP-05 两处 Shortcut 均指向 cloudmount-desktop.exe（AC3②）" "$ok"

ok=1
if [ -s "$WXS_NORM" ]; then
  n_shortcut="$(grep -o '<Shortcut' "$WXS_NORM" | wc -l | tr -d '[:space:]')"
  n_bad="$(grep -- '<Shortcut' "$WXS_NORM" | grep -cE 'cloudmountd\.exe|cloudmount\.exe' | tr -d '[:space:]')"
  if [ "${n_shortcut:-0}" -ge 1 ] && [ "${n_bad:-0}" = "0" ]; then ok=0; fi
fi
report "TP-05 负 快捷方式目标不指 daemon/cli exe（指错即缺陷；Shortcut 行存在为前提防空真）" "$ok"

# ============ TP-06：.ico 图标文件存在（构建前拦截——缺失即红） ============

ok=1
[ -f "$ICO_FILE" ] && ok=0
report "TP-06 apps/desktop/icons/icon.ico 存在（wxs/winres 引用的图标缺失即红——构建前拦截）" "$ok"

# ============ TP-07：package-dmg-macos.sh 字面（AC4；.app bundle + hdiutil + /Applications） ============

ok=1
[ -s "$DMGS_NORM" ] && ok=0
report "TP-07 scripts/package-dmg-macos.sh 存在（钉死路径）" "$ok"

ok=1
grep_ok "$DMGS_NORM" -qF 'Contents/Info.plist' && ok=0
report "TP-07 .app bundle 结构：Contents/Info.plist 字面" "$ok"

ok=1
grep_ok "$DMGS_NORM" -qF 'Contents/MacOS/cloudmount-desktop' && ok=0
report "TP-07 .app bundle 结构：Contents/MacOS/cloudmount-desktop 字面（桌面二进制入位）" "$ok"

ok=1
grep_ok "$DMGS_NORM" -qF 'Contents/Resources' && grep_ok "$DMGS_NORM" -qF '.icns' && ok=0
report "TP-07 .app bundle 结构：Contents/Resources + .icns 字面（图标入位）" "$ok"

ok=1
grep_ok "$DMGS_NORM" -qF 'hdiutil' && ok=0
report "TP-07 hdiutil 打 .dmg 字面（mac 原生工具，AC4②）" "$ok"

ok=1
grep_ok "$DMGS_NORM" -qF '/Applications' && ok=0
report "TP-07 dmg 内 /Applications 拖拽语义字面（AC4②）" "$ok"

# ============ TP-08：daemon/cli pkg 铺放负断言（AC4；A1：.app 只走 .dmg 不混入 pkg） ============

ok=1
if grep_absent_ok "$PKGS_NORM" -qF '.app'; then ok=0; fi
report "TP-08 负 package-pkg-macos.sh 不含 .app 字面（A1：.app 只走 .dmg）" "$ok"

ok=1
if grep_absent_ok "$PKGS_NORM" -qiE 'desktop'; then ok=0; fi
report "TP-08 负 package-pkg-macos.sh 不含 desktop 字面（desktop 组件不混入 daemon/cli pkg）" "$ok"

ok=1
if grep_absent_ok "$PKG_BLK" -qiE 'desktop|\.app'; then ok=0; fi
report "TP-08 负 pkg job 块不含 desktop/.app 字面（CI 面同护）" "$ok"

# ============ TP-09：apps/desktop 托盘接线字面（AC2；可判定式——全文件计数界定） ============

ok=1
[ -s "$MAINRS_NORM" ] && ok=0
report "TP-09 apps/desktop/src/main.rs 存在（前置——缺失即本段断言全红）" "$ok"

ok=1
grep_ok "$MAINRS_NORM" -qF 'tray_icon::' && ok=0
report "TP-09 tray-icon 初始化字面（tray_icon:: 路径引用，AC2①）" "$ok"

ok=1
if [ -s "$MAINRS_NORM" ]; then
  n="$(grep -o 'MenuItem::' "$MAINRS_NORM" | wc -l | tr -d '[:space:]')"
  if [ "$n" = "2" ]; then ok=0; else note "TP-09: MenuItem:: 出现次数=$n（期望恰 2）"; fi
fi
report "TP-09 托盘菜单项恰两项（MenuItem 构造计数=2，全文件界定）" "$ok"

ok=1
grep_ok "$MAINRS_NORM" -qF '显示主窗口' && ok=0
report "TP-09 菜单项标签「显示主窗口」（AC2② 恰两项之一）" "$ok"

ok=1
grep_ok "$MAINRS_NORM" -qF '退出' && ok=0
report "TP-09 菜单项标签「退出」（AC2② 恰两项之二）" "$ok"

ok=1
grep_ok "$MAINRS_NORM" -qF 'MenuEvent::receiver()' && grep_ok "$MAINRS_NORM" -qF 'try_recv' && ok=0
report "TP-09 MenuEvent 全局 receiver 轮询形态（MenuEvent::receiver + try_recv）" "$ok"

ok=1
if [ -s "$MAINRS_NORM" ]; then
  n_show="$(grep -o '"show"' "$MAINRS_NORM" | wc -l | tr -d '[:space:]')"
  n_quit="$(grep -o '"quit"' "$MAINRS_NORM" | wc -l | tr -d '[:space:]')"
  if [ "${n_show:-0}" -ge 2 ] && [ "${n_quit:-0}" -ge 2 ]; then ok=0; else
    note "TP-09: \"show\"×${n_show:-0} \"quit\"×${n_quit:-0}（各期望≥2：构造+MenuEvent 匹配）"
  fi
fi
report "TP-09 两 id MenuEvent 匹配（\"show\"/\"quit\" 构造+匹配各≥2 处）" "$ok"

ok=1
if [ -s "$MAINRS_NORM" ]; then
  n="$(grep -o 'Menu::new' "$MAINRS_NORM" | wc -l | tr -d '[:space:]')"
  if [ "$n" = "1" ]; then ok=0; else note "TP-09: Menu::new 出现次数=$n（期望恰 1——无其他 tray Menu 构造）"; fi
fi
report "TP-09 全文件无其他 tray Menu 构造（Menu::new 计数=1）" "$ok"

# ============ TP-10：脚本自身质量（AC6；防空真/CRLF/降级计数诚实） ============

SELF="$SCRIPT_DIR/desktop-installers-yaml.test.sh"

ok=1
[ -f "$SELF" ] && ok=0
report "TP-10 本断言脚本独立成文件 scripts/desktop-installers-yaml.test.sh（AC6：desktop-*.test.sh glob，四锁定脚本零触碰）" "$ok"

ok=1
grep -qF 'grep_absent_ok()' "$SELF" && grep -qF '[ -s "$f" ] || return 1' "$SELF" && ok=0
report "TP-10 防空真机制在位（grep_ok/grep_absent_ok 非空前置——空块不计命中/不空真）" "$ok"

ok=1
grep -qF "tr -d '\r'" "$SELF" && ok=0
report "TP-10 CRLF 剥除规范化在位（Git Bash/Windows 不误报）" "$ok"

ok=1
grep -qF 'SKIP actionlint' "$SELF" && grep -qF 'WARN 降级告警' "$SELF" && grep -qF '降级分支计入总数' "$SELF" && ok=0
report "TP-10 actionlint 降级计数诚实（SKIP+WARN 输出且降级断言计入总数）" "$ok"

# ============ TP-11：gen_icons 产物存在+尺寸集（AC1；含 icns 断言——首日验证③定稿后启用，gate2 M2 补落） ============

ok=1
[ -s "$GENICONS_NORM" ] && ok=0
report "TP-11 scripts/gen_icons.py 存在（图标管线单一来源，AC1）" "$ok"

ok=1
if grep_absent_ok "$GENICONS_NORM" -qF 'src-tauri'; then ok=0; fi
report "TP-11 负 gen_icons.py 无 src-tauri 字面（输出路径不再指向不存在目录，AC1②）" "$ok"

ok=1
if [ -f "$ICO_FILE" ]; then
  ico_hex="$(od -An -tx1 -N8 "$ICO_FILE" 2>/dev/null | tr -d ' \n')"
  if [ "${#ico_hex}" -eq 16 ] \
     && [ "${ico_hex:0:8}" = "00000100" ] \
     && [ "${ico_hex:8:4}" != "0000" ] \
     && [ "${ico_hex:12:4}" = "2020" ]; then
    ok=0
  else
    note "TP-11: icon.ico 头部非预期（期望 00000100+count≥1+首帧 32×32）：$ico_hex"
  fi
fi
report "TP-11 icon.ico 有效 ICO（magic+≥1 帧+首帧 32×32 基线）" "$ok"

ok=1
check_png "$PNG32" "00000020" && ok=0
report "TP-11 32x32.png 存在且有效（PNG magic+IHDR 32×32 基线）" "$ok"

ok=1
check_png "$PNG128" "00000080" && ok=0
report "TP-11 128x128.png 存在且有效（PNG magic+IHDR 128×128 基线）" "$ok"

ok=1
if [ -f "$ICONS_RS" ]; then
  rs_norm="$TMP_ROOT/icons-rs.normalized"
  tr -d '\r' <"$ICONS_RS" >"$rs_norm"
  [ -s "$rs_norm" ] && grep -qi 'rgba' "$rs_norm" && ok=0
fi
report "TP-11 icons.rs 存在且含 RGBA 常量源字面（gate1 r2 裁决，零 crate 依赖）" "$ok"

# icns 子断言（gate2 M2 补落，UNLOCK-TESTS 2026-10-16 授权=人类裁决逐字「补断言+解锁授权（推荐）」；
# 首日验证③定稿=stdlib 手写 PNG-in-ICNS 预生成入仓——存在性+结构校验随定稿启用）
ok=1
if [ -f "$ICNS_FILE" ]; then
  icns_hex="$(od -An -tx1 -N8 "$ICNS_FILE" 2>/dev/null | tr -d ' \n')"
  icns_types="$(grep -aoE 'ic11|ic07|ic08' "$ICNS_FILE" 2>/dev/null | wc -l | tr -d ' ')"
  if [ "${#icns_hex}" -eq 16 ] \
     && [ "${icns_hex:0:8}" = "69636e73" ] \
     && [ "${icns_hex:8:8}" != "00000000" ] \
     && [ "${icns_types:-0}" -ge 2 ]; then
    ok=0
  else
    note "TP-11: icon.icns 头部/载荷非预期（期望 icns magic+非零总长+≥2 个 ic11/ic07/ic08 类型码）：$icns_hex types=$icns_types"
  fi
fi
report "TP-11 icon.icns 存在且有效（icns magic+长度+ic07/ic08/ic11 载荷——首日验证③定稿预生成入仓；gate2 M2 补落）" "$ok"

# ============ TP-12：图标管线 fail-fast（AC1；wxs/winres 引用 .ico + dmg icns 硬校验） ============

ok=1
grep_ok "$WXS_NORM" -qF '.ico' && ok=0
report "TP-12 main.wxs 引用 .ico 图标字面（Icon Source——缺 .ico 时 light 即失败，fail-fast 链）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qE '(<Icon|[[:space:]]Icon=)' && ok=0
report "TP-12 main.wxs Icon 元素/属性字面（<Icon 元素或 Icon= 属性，AC3③）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qi 'ARPPRODUCTICON' && ok=0
report "TP-12 main.wxs ARPPRODUCTICON 字面（ARP 图标属性，AC3③）" "$ok"

ok=1
[ -f "$BUILD_RS" ] && ok=0
report "TP-12 apps/desktop/build.rs 存在（winres 嵌入面，AC1）" "$ok"

ok=1
if [ -f "$BUILD_RS" ]; then
  brs_norm="$TMP_ROOT/build-rs.normalized"
  tr -d '\r' <"$BUILD_RS" >"$brs_norm"
  grep_ok "$brs_norm" -qi 'winres' && grep_ok "$brs_norm" -qF 'icon.ico' && ok=0
fi
report "TP-12 build.rs 引用 winres+icon.ico 字面（缺 .ico 构建即失败，fail-fast 链）" "$ok"

ok=1
grep_ok "$DMGS_NORM" -qE '(\[ -f|test -f|\[ -e|test -e)' && grep_ok "$DMGS_NORM" -qi 'icns' && ok=0
report "TP-12 dmg 脚本 icns fail-fast（存在性硬校验字面+icns 引用共现——缺 .icns 非零退出不产出）" "$ok"

# ============ TP-24：无签名引导文案（AC4；Windows SmartScreen + mac Gatekeeper——body 字面） ============

ok=1
grep_ok "$REL_BLK" -qF 'SmartScreen' && ok=0
report "TP-24 body 含 Windows SmartScreen 引导文案（无签名已知状态，沿既有口径）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'Gatekeeper' && ok=0
report "TP-24 body 含 mac Gatekeeper 引导文案（无签名已知状态，沿既有口径）" "$ok"

ok=1
if grep_absent_ok "$REL_BLK" -qE '已签名|已公证'; then ok=0; fi
report "TP-24 负 body 无「已签名/已公证」误导措辞（无证书为已知状态）" "$ok"

# ============ TP-10/actionlint：整文件校验 + 拦截手段自校验（不可用时降级并记缺口；复制自 release-yaml.test.sh L232-283） ============

if command -v actionlint >/dev/null 2>&1; then
  # 正：release.yml 整文件合法（零错）
  ok=1
  if [ "$wf_ready" -eq 1 ] && actionlint "$WF" >/dev/null 2>&1; then ok=0; fi
  report "TP-10 actionlint release.yml 零错（整文件合法性）" "$ok"

  # 负（拦截手段自校验）：注入语法错误的临时副本必须被拦截（非零）
  ok=1
  BROKEN="$TMP_ROOT/release.broken.yml"
  if { cat "$NORM"; printf '\n  broken-injection: [unclosed-flow-seq\n'; } >"$BROKEN" && [ -s "$BROKEN" ]; then
    if actionlint "$BROKEN" >/dev/null 2>&1; then
      note "TP-10: 注入语法错误的副本 actionlint 返回 0（拦截失效）"
    else
      ok=0
    fi
  else
    note "TP-10: 注入副本构造失败"
  fi
  report "TP-10 注入语法错误副本被 actionlint 拦截（非零，自校验）" "$ok"
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
          note "TP-10 降级: python yaml.safe_load 解析 release.yml 失败（YAML 非法）"
        fi
      else
        # 终极降级：grep 键配平粗检（非完整解析，记缺口）
        if grep -q '^on:' "$NORM" && grep -q '^jobs:' "$NORM" \
           && [ "$(grep -cE '^[A-Za-z_][A-Za-z0-9_-]*:' "$NORM")" -ge 4 ]; then
          ok=0
          note "TP-10 终极降级: pyyaml 不可用，仅 grep 键配平粗检（非完整解析，记缺口）"
        else
          note "TP-10 终极降级: grep 键配平粗检未过（on:/jobs: 键缺失）"
        fi
      fi
    else
      note "TP-10 降级: python 不可用且无其他 YAML 解析手段"
    fi
  fi
  report "TP-10 降级 YAML 结构断言（release.yml 可解析/键配平；降级分支计入总数）" "$ok"
  echo "WARN 降级告警：actionlint 不可用——仅覆盖 YAML 合法性（python yaml.safe_load 或键配平）+ 结构断言；语法拦截自校验（注入负路径）未执行，记缺口（test-strategy：缺口·拦截手段依赖）"
fi

# ============ 汇总 ============
echo "$PASS/$TOTAL passed"
if [ "$FAIL" -ne 0 ]; then
  exit 1
fi
exit 0
