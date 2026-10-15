#!/usr/bin/env bash
#
# scripts/installers-yaml.test.sh — installer-packages U1：安装包交付面静态结构断言
#   （TDD，静态 grep/awk 式，不要求 yq；Git Bash 直跑；repo 根或任意工作目录均可）
#
# 被测对象（三）：
#   ① .github/workflows/release.yml —— 现状六 job、无安装包 job → 安装包相关断言红
#   ② packaging/wix/main.wxs        —— 红阶段不存在 → 相关断言带「文件存在」前置，缺失即 FAIL（防空真，不空真）
#   ③ scripts/package-pkg-macos.sh  —— 同上
#
# 契约来源：docs/aidlc/installer-packages/units/1.md（断言不得弱化）
#   + prd.md AC1-AC6 与语义对齐内联小节 + test-strategy.md T1-T15
#
# ── 机制复制声明（上 intent retro 学习；先例 = 已 TDD 锁定的 scripts/release-yaml.test.sh，只读） ──
#   job_block/toplevel_block 提取（先例 L70-88）、release_needs（L90-98）、
#   grep_ok/grep_absent_ok 防空真（L100-112）、CRLF 剥除规范化副本（L64-68）、
#   actionlint 降级（SKIP + python yaml.safe_load → 键配平粗检 + WARN，L232-283）
#   ——均复制自上述先例；同步义务：先例机制若变更，本文件须跟进同步。
#   set -u 沿先例（units/1.md 行文作 set -euo pipefail，先例实为 set -u；
#   -e / -o pipefail 会使「[ -s ] && 短路」与「grep -q 管道」变中途退出 / SIGPIPE 假阴性——按先例 set -u，留痕备查）。
#
# ── Construction 落字面（gate1 未钉死安装包 job 命名与 Windows runner pin；本脚本红阶段钉死，实现据此红绿闭环） ──
#   MSI job 名 = installer-msi-windows，runner = windows-latest
#   pkg job 名 = installer-pkg-macos，runner = macos-15（prd AC2 已钉）
#   upload 步识别 = actions/upload-artifact；build/package/upload 步识别 ERE 见 T9 段 BPU_ERE
#
# 行尾注意：release.yml 为 CRLF 检出；断言前统一剥 \r 至临时规范化副本，再行 grep/awk。
# 输出：逐断言 `PASS/FAIL <名称>`（诊断走 stderr），末行 `<N>/<M> passed`；任一 FAIL → 退出非零。
# TDD 红阶段预期：T1-T9 安装包相关断言、T12 产物表/签名覆盖/安装提示断言全 FAIL；
#   护栏类（T10、T11 既有产物锚点+既有 job 名+收编链三字面、T12 负断言「无已签名/已公证」、
#   T13、T14、T15 降级分支）现状绿属正常（回归护栏）。

set -u
export LC_ALL=C

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WF="$SCRIPT_DIR/../.github/workflows/release.yml"
WXS="$SCRIPT_DIR/../packaging/wix/main.wxs"
PKGS="$SCRIPT_DIR/../scripts/package-pkg-macos.sh"

TMP_ROOT="$(mktemp -d)"
trap 'rm -rf "$TMP_ROOT"' EXIT

PASS=0
FAIL=0
TOTAL=0

[ -f "$WF" ] || printf '[red] 被测文件不存在：%s —— 相关断言按 FAIL 计\n' "$WF" >&2
[ -f "$WXS" ] || printf '[red] 被测文件不存在（红态预期）：%s —— T3/T4 相关断言 FAIL（缺失不空真）\n' "$WXS" >&2
[ -f "$PKGS" ] || printf '[red] 被测文件不存在（红态预期）：%s —— T6/T7/T8 相关断言 FAIL（缺失不空真）\n' "$PKGS" >&2

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

PKGS_NORM="$TMP_ROOT/package-pkg-macos.normalized.sh"
if [ -f "$PKGS" ]; then tr -d '\r' <"$PKGS" >"$PKGS_NORM"; fi

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

# ---- 本 intent 新增提取助手（沿用先例的 awk 段提取风格） ----

# upload_step_block <jobblk 文件> —— 自 actions/upload-artifact 行起，
# 至下一个 step 项（6 空格缩进 `- `）或块尾止（不依赖 upload 是否为 job 末步）
upload_step_block() {
  awk '
    grab && /^      - / && $0 !~ /actions\/upload-artifact/ { exit }
    /actions\/upload-artifact/ { grab = 1 }
    grab { print }
  ' "$1" 2>/dev/null
}

# steps_matching <jobblk 文件> <ERE> —— 输出「段首两行（- name:/- uses: 识别行）命中 ERE」
# 的完整 step 段（run 体不参与识别，防误选）
steps_matching() {
  awk -v ere="$2" '
    /^      - / {
      if (inseg && hdr ~ ere) printf "%s", seg
      inseg = 1; seg = ""; hdr = ""; n = 0
    }
    inseg {
      seg = seg $0 "\n"
      if (n < 2) { hdr = hdr $0 "\n"; n++ }
    }
    END { if (inseg && hdr ~ ere) printf "%s", seg }
  ' "$1" 2>/dev/null
}

# ---- 物化断言对象块 ----
MSI_BLK="$TMP_ROOT/block.installer-msi-windows.yml"
PKG_BLK="$TMP_ROOT/block.installer-pkg-macos.yml"
REL_BLK="$TMP_ROOT/block.release.yml"
ON_BLK="$TMP_ROOT/block.on.yml"
job_block installer-msi-windows >"$MSI_BLK"
job_block installer-pkg-macos >"$PKG_BLK"
job_block release >"$REL_BLK"
toplevel_block on >"$ON_BLK"

MSI_UPLOAD="$TMP_ROOT/block.msi-upload.yml"
PKG_UPLOAD="$TMP_ROOT/block.pkg-upload.yml"
upload_step_block "$MSI_BLK" >"$MSI_UPLOAD"
upload_step_block "$PKG_BLK" >"$PKG_UPLOAD"

# build/package/upload 步识别 ERE（T9 continue-on-error 负断言的限定范围；
# 覆盖中英命名：编译/构建/打包/上传/校验/build/package/upload/WiX/msi/pkg）
BPU_ERE='(编译|构建|打包|上传|校验|[Bb]uild|[Pp]ackage|[Uu]pload|WiX|wix|MSI|msi|pkg)'
MSI_BPU="$TMP_ROOT/msi-bpu-steps.txt"
PKG_BPU="$TMP_ROOT/pkg-bpu-steps.txt"
steps_matching "$MSI_BLK" "$BPU_ERE" >"$MSI_BPU"
steps_matching "$PKG_BLK" "$BPU_ERE" >"$PKG_BPU"

# ============ T1：MSI job 定义（AC1；Construction 落字面：installer-msi-windows + windows-latest） ============

ok=1
if [ "$wf_ready" -eq 1 ] && grep -qE '^  installer-msi-windows:[[:space:]]*(#.*)?$' "$NORM"; then ok=0; fi
report "T1 installer-msi-windows: job 定义存在" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ]; then
  runs_total="$(grep -cE '^[[:space:]]*runs-on:' "$MSI_BLK" || true)"
  runs_win="$(grep -cE '^[[:space:]]*runs-on:[[:space:]]*windows-latest[[:space:]]*$' "$MSI_BLK" || true)"
  if [ "$runs_total" -eq 1 ] && [ "$runs_win" -eq 1 ]; then ok=0; else
    note "T1: msi job 内 runs-on 行数=$runs_total，恰为 windows-latest 的行数=$runs_win"
  fi
fi
report "T1 msi job runs-on 恰为 windows-latest（Construction 落字面，单 runner）" "$ok"

# ============ T2：MSI 产出命名 + upload 兜底 + 前置硬校验（AC1） ============

ok=1
grep_ok "$MSI_BLK" -qF 'cloudmount-windows-x64.msi' && ok=0
report "T2 msi 产出名 cloudmount-windows-x64.msi 字面（打包输出/upload path）" "$ok"

ok=1
grep_ok "$MSI_UPLOAD" -qF 'actions/upload-artifact' && ok=0
report "T2 msi upload 步存在（actions/upload-artifact）" "$ok"

ok=1
grep_ok "$MSI_UPLOAD" -qE '^[[:space:]]*if-no-files-found:[[:space:]]*error[[:space:]]*$' && ok=0
report "T2 msi upload 含 if-no-files-found: error（产物缺失不静默发布）" "$ok"

ok=1
grep_ok "$MSI_UPLOAD" -qE '^[[:space:]]*name:[[:space:]]*cloudmount-([^[:space:]]|$)' && ok=0
report "T2 msi upload artifact name 以 cloudmount- 开头（release pattern 收编前提）" "$ok"

ok=1
grep_ok "$MSI_BLK" -qE '(test -f|\[ -f|Test-Path).*cloudmountd\.exe' \
  && grep_ok "$MSI_BLK" -qE '(test -f|\[ -f|Test-Path).*cloudmount\.exe' && ok=0
report "T2 msi 打包前置构建输入 .exe 存在性校验字面（cloudmountd.exe+cloudmount.exe；WiX 天然报错为注记非断言）" "$ok"

# ============ T3：MSI 安装面 token + 版本同源（AC1；Q2/Q3/M3/M7） ============

ok=1
[ -s "$WXS_NORM" ] && ok=0
report "T3 packaging/wix/main.wxs 存在（Q2 钉死路径：手写 .wxs + CI 直接 WiX）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'cloudmountd.exe' && ok=0
report "T3 main.wxs 含 cloudmountd.exe 字面（daemon 入包，A1）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'cloudmount.exe' && ok=0
report "T3 main.wxs 含 cloudmount.exe 字面（cli 入包，A1）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'ProgramFilesFolder' && ok=0
report "T3 main.wxs 含 ProgramFilesFolder token（Q4：Program Files 安装）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'Name="CloudMount"' && ok=0
report "T3 main.wxs 含 Name=\"CloudMount\"（Q4：安装目录 CloudMount）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'Environment' && ok=0
report "T3 main.wxs 含 Environment 元素（Q3：Machine PATH 载体）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'PATH' && ok=0
report "T3 main.wxs 含 PATH 字面（Q3：写 Machine PATH）" "$ok"

ok=1
grep_ok "$WXS_NORM" -qF 'System="yes"' && ok=0
report "T3 main.wxs 含 System=\"yes\"（Q3：Machine 级而非 User）" "$ok"

ok=1
grep_ok "$MSI_BLK" -qF 'github.ref_name' && ok=0
report "T3 msi job 安装器版本取 github.ref_name 字面（M3：与 pkg --version 同源）" "$ok"

# ============ T4：负断言——范围排除（AC1；A1：只铺文件+PATH，SCM 注册留用户命令） ============

ok=1
grep_absent_ok "$MSI_BLK" -iE 'desktop' && ok=0
report "T4 负 msi job 块不含 desktop/cloudmount-desktop 字面" "$ok"

ok=1
grep_absent_ok "$WXS_NORM" -iE 'desktop' && ok=0
report "T4 负 main.wxs 不含 desktop 字面" "$ok"

ok=1
grep_absent_ok "$MSI_BLK" -iE 'ServiceInstall|service install' && ok=0
report "T4 负 msi job 块不含服务注册字面（ServiceInstall/service install）" "$ok"

ok=1
grep_absent_ok "$WXS_NORM" -iE 'ServiceInstall|ServiceControl' && ok=0
report "T4 负 main.wxs 不含 SCM 注册字面（ServiceInstall/ServiceControl）" "$ok"

# ============ T5：pkg job 定义（AC2；runner=macos-15 已钉，job 名 Construction 落字面） ============

ok=1
if [ "$wf_ready" -eq 1 ] && grep -qE '^  installer-pkg-macos:[[:space:]]*(#.*)?$' "$NORM"; then ok=0; fi
report "T5 installer-pkg-macos: job 定义存在" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ]; then
  runs_total="$(grep -cE '^[[:space:]]*runs-on:' "$PKG_BLK" || true)"
  runs_mac15="$(grep -cE '^[[:space:]]*runs-on:[[:space:]]*macos-15[[:space:]]*$' "$PKG_BLK" || true)"
  if [ "$runs_total" -eq 1 ] && [ "$runs_mac15" -eq 1 ]; then ok=0; else
    note "T5: pkg job 内 runs-on 行数=$runs_total，恰为 macos-15 的行数=$runs_mac15"
  fi
fi
report "T5 pkg job runs-on 恰为 macos-15（prd AC2 pin，单 runner 双 target）" "$ok"

# ============ T6：pkg 产出命名（双 arch 精确路径）+ upload 兜底 + 脚本调用（AC2；MINOR⑥/M2） ============

ok=1
grep_ok "$PKG_BLK" -qF 'cloudmount-darwin-aarch64.pkg' && ok=0
report "T6 pkg 产出名 cloudmount-darwin-aarch64.pkg 字面（job 块内）" "$ok"

ok=1
grep_ok "$PKG_BLK" -qF 'cloudmount-darwin-x86_64.pkg' && ok=0
report "T6 pkg 产出名 cloudmount-darwin-x86_64.pkg 字面（job 块内）" "$ok"

ok=1
grep_ok "$PKG_UPLOAD" -qF 'actions/upload-artifact' && ok=0
report "T6 pkg upload 步存在（actions/upload-artifact）" "$ok"

ok=1
grep_ok "$PKG_UPLOAD" -qF 'cloudmount-darwin-aarch64.pkg' && ok=0
report "T6 pkg upload path 含 cloudmount-darwin-aarch64.pkg 精确字面（不用 glob）" "$ok"

ok=1
grep_ok "$PKG_UPLOAD" -qF 'cloudmount-darwin-x86_64.pkg' && ok=0
report "T6 pkg upload path 含 cloudmount-darwin-x86_64.pkg 精确字面（不用 glob）" "$ok"

ok=1
grep_absent_ok "$PKG_UPLOAD" -qF '*.pkg' && ok=0
report "T6 负 pkg upload 不用 *.pkg glob（MINOR⑥：精确路径落空仅 warning，闭环靠前置硬校验）" "$ok"

ok=1
grep_ok "$PKG_UPLOAD" -qE '^[[:space:]]*if-no-files-found:[[:space:]]*error[[:space:]]*$' && ok=0
report "T6 pkg upload 含 if-no-files-found: error（全部落空即 job 失败）" "$ok"

ok=1
grep_ok "$PKG_UPLOAD" -qE '^[[:space:]]*name:[[:space:]]*cloudmount-([^[:space:]]|$)' && ok=0
report "T6 pkg upload artifact name 以 cloudmount- 开头（release pattern 收编前提）" "$ok"

ok=1
grep_ok "$PKG_BLK" -qF 'scripts/package-pkg-macos.sh' && grep_ok "$PKG_BLK" -qF -- '--bin-dir' && ok=0
report "T6 pkg job 调用 scripts/package-pkg-macos.sh（staging 硬校验调用字面，镜像 package-macos.sh 先例）" "$ok"

# ============ T7：pkg 原生工具链 + 安装目标 + 版本同源（AC2；A1/Q4/M3/M7） ============

ok=1
[ -s "$PKGS_NORM" ] && ok=0
report "T7 scripts/package-pkg-macos.sh 存在（钉死路径）" "$ok"

ok=1
grep_ok "$PKGS_NORM" -qE 'pkgbuild|productbuild' && ok=0
report "T7 pkg 脚本含 pkgbuild/productbuild 字面（原生工具零新依赖，A1）" "$ok"

ok=1
grep_ok "$PKGS_NORM" -qF '/usr/local/bin' && ok=0
report "T7 pkg 脚本含 /usr/local/bin 安装目标字面（Q4）" "$ok"

ok=1
grep_ok "$PKG_BLK" -qF 'github.ref_name' && ok=0
report "T7 pkg job 安装器版本取 github.ref_name 字面（M3：与 MSI ProductVersion 同源）" "$ok"

# ============ T8：负断言——范围排除（AC2；A1/MINOR⑧：纯文件铺放，无 launchd 无安装脚本） ============

ok=1
grep_absent_ok "$PKG_BLK" -iE 'desktop|\.app' && ok=0
report "T8 负 pkg job 块不含 desktop/.app/CloudMount.app 字面" "$ok"

ok=1
grep_absent_ok "$PKGS_NORM" -iE 'desktop|\.app' && ok=0
report "T8 负 pkg 脚本不含 desktop/.app 字面" "$ok"

ok=1
grep_absent_ok "$PKG_BLK" -iE 'LaunchDaemons|launchd|plist' && ok=0
report "T8 负 pkg job 块不含 launchd 字面（LaunchDaemons/plist）" "$ok"

ok=1
grep_absent_ok "$PKGS_NORM" -iE 'LaunchDaemons|launchd|plist' && ok=0
report "T8 负 pkg 脚本不含 launchd 字面（LaunchDaemons/plist）" "$ok"

ok=1
grep_absent_ok "$PKG_BLK" -iE 'preinstall|postinstall|--scripts' && ok=0
report "T8 负 pkg job 块不含 preinstall/postinstall/--scripts 字面（供应链向量排除）" "$ok"

ok=1
grep_absent_ok "$PKGS_NORM" -iE 'preinstall|postinstall|--scripts' && ok=0
report "T8 负 pkg 脚本不含 preinstall/postinstall/--scripts 字面（纯文件铺放）" "$ok"

# ============ T9：release needs 门控（仅追加）+ 安装包 job 零覆盖/零吞错（AC3；MINOR④） ============

ok=1
if [ "$wf_ready" -eq 1 ] && release_needs | grep -q 'installer-msi-windows'; then ok=0; fi
report "T9 release needs 含 installer-msi-windows（MSI 腿失败则不发布）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ] && release_needs | grep -q 'installer-pkg-macos'; then ok=0; fi
report "T9 release needs 含 installer-pkg-macos（pkg 腿失败则不发布）" "$ok"

ok=1
grep_absent_ok "$MSI_BLK" -E '^[[:space:]]*if:' && ok=0
report "T9 负 msi job 块内无 if: 行（防门控覆盖）" "$ok"

ok=1
grep_absent_ok "$MSI_BLK" -E '\b(always|success|failure|cancelled)\(\)' && ok=0
report "T9 负 msi job 块内无 status 函数调用（防门控覆盖）" "$ok"

ok=1
grep_absent_ok "$PKG_BLK" -E '^[[:space:]]*if:' && ok=0
report "T9 负 pkg job 块内无 if: 行（防门控覆盖）" "$ok"

ok=1
grep_absent_ok "$PKG_BLK" -E '\b(always|success|failure|cancelled)\(\)' && ok=0
report "T9 负 pkg job 块内无 status 函数调用（防门控覆盖）" "$ok"

ok=1
grep_absent_ok "$MSI_BPU" -F 'continue-on-error' && ok=0
report "T9 负 msi job build/package/upload 步无 continue-on-error（MINOR④：防构建失败被吞）" "$ok"

ok=1
grep_absent_ok "$PKG_BPU" -F 'continue-on-error' && ok=0
report "T9 负 pkg job build/package/upload 步无 continue-on-error（MINOR④：防门控旁路）" "$ok"

# ============ T10：护栏——release needs 既有腿零删改 + 零覆盖（AC3，沿用上 intent T11 模式） ============

ok=1
if [ "$wf_ready" -eq 1 ] && release_needs | grep -q 'binaries-linux'; then ok=0; fi
report "T10 护栏 release needs 仍含 binaries-linux（AC3 零删改）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ] && release_needs | grep -q 'binaries-windows'; then ok=0; fi
report "T10 护栏 release needs 仍含 binaries-windows（AC3 零删改）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ] && release_needs | grep -q 'binaries-macos'; then ok=0; fi
report "T10 护栏 release needs 仍含 binaries-macos（AC3 零删改）" "$ok"

ok=1
grep_absent_ok "$REL_BLK" -E '^[[:space:]]*if:' && ok=0
report "T10 护栏 release job 块内无 if: 行（防门控覆盖）" "$ok"

ok=1
grep_absent_ok "$REL_BLK" -E '\b(always|success|failure|cancelled)\(\)' && ok=0
report "T10 护栏 release job 块内无 status 函数调用（防 fail-fast 削弱）" "$ok"

# ============ T11：回归护栏——既有产物名锚点 + 既有 job 名 + 收编链三字面（AC3/AC6；MINOR③） ============

ok=1
grep_ok "$NORM" -qE 'cloudmount-linux-[^[:space:]]*\.tar\.gz' && ok=0
report "T11 锚点 cloudmount-linux-*.tar.gz 产物名（AC3 零删改）" "$ok"

ok=1
grep_ok "$NORM" -qF 'cloudmount-windows-x64.zip' && ok=0
report "T11 锚点 cloudmount-windows-x64.zip 产物名（AC3 零删改）" "$ok"

ok=1
grep_ok "$NORM" -qF 'cloudmount-darwin-*.tar.gz' && ok=0
report "T11 锚点 cloudmount-darwin-*.tar.gz 产物名（AC3 零删改）" "$ok"

ok=1
grep_ok "$NORM" -qF 'digests-' && ok=0
report "T11 锚点 digests-* 产物名（AC3 零删改）" "$ok"

ok=1
if [ "$wf_ready" -eq 1 ] \
   && grep -qE '^  binaries-linux:[[:space:]]*(#.*)?$' "$NORM" \
   && grep -qE '^  binaries-windows:[[:space:]]*(#.*)?$' "$NORM" \
   && grep -qE '^  binaries-macos:[[:space:]]*(#.*)?$' "$NORM"; then ok=0; fi
report "T11 既有 job 定义在位（binaries-linux/windows/macos，AC3）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'pattern: cloudmount-*' && ok=0
report "T11 收编链 pattern: cloudmount-*（release 下载步零改，MINOR③）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'merge-multiple: true' && ok=0
report "T11 收编链 merge-multiple: true（release 下载步零改，MINOR③）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'files: dist/*' && ok=0
report "T11 收编链 files: dist/*（release 上传步零改，MINOR③）" "$ok"

# ============ T12：Release body 产物表 + 双平台签名说明 + 安装侧提示（AC4；MINOR①） ============

ok=1
grep_ok "$REL_BLK" -qF 'cloudmount-windows-x64.msi' && ok=0
report "T12 产物表含 cloudmount-windows-x64.msi 行" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'cloudmount-darwin-aarch64.pkg' && ok=0
report "T12 产物表含 cloudmount-darwin-aarch64.pkg 行" "$ok"

ok=1
grep_ok "$REL_BLK" -qF 'cloudmount-darwin-x86_64.pkg' && ok=0
report "T12 产物表含 cloudmount-darwin-x86_64.pkg 行" "$ok"

ok=1
grep_ok "$REL_BLK" -qE 'SmartScreen|Authenticode' && grep_ok "$REL_BLK" -qF 'Gatekeeper' && ok=0
report "T12 签名状态说明双平台覆盖（MSI SmartScreen/Authenticode + pkg Gatekeeper；漏 Windows 侧即缺陷）" "$ok"

ok=1
grep_absent_ok "$REL_BLK" -E '已签名|已公证' && ok=0
report "T12 负 body 不含「已签名/已公证」误导措辞（无证书为已知状态）" "$ok"

ok=1
grep_ok "$REL_BLK" -qF '新开终端' && grep_ok "$REL_BLK" -qF 'UAC' && grep_ok "$REL_BLK" -qF '先卸载再装' && ok=0
report "T12 安装侧提示三关键词（新开终端 PATH 生效 / UAC 提权为预期 / 先卸载再装，MINOR①）" "$ok"

# ============ T13：正式发布语义 + 触发器守护（AC5；A1） ============

ok=1
grep_absent_ok "$REL_BLK" -E 'prerelease:[[:space:]]*true|draft:[[:space:]]*true' && ok=0
report "T13 负 release 块无 prerelease: true/draft: true（tag 即直接正式发布，显式 false 允许）" "$ok"

ok=1
if [ -s "$ON_BLK" ] \
   && grep_ok "$ON_BLK" -q 'tags:' \
   && grep_ok "$ON_BLK" -qF 'v*' \
   && grep_absent_ok "$ON_BLK" -q 'workflow_dispatch'; then ok=0; fi
report "T13 守护 on: 仅 push tags v*（无 workflow_dispatch，触发语义零改动）" "$ok"

# ============ T14：脚本自身交付自检（AC6；红阶段自检项） ============

ok=1
[ -f "$SCRIPT_DIR/installers-yaml.test.sh" ] && ok=0
report "T14 本断言脚本独立成文件 scripts/installers-yaml.test.sh（AC6，零触碰三锁定脚本）" "$ok"

# ============ T15：actionlint 整文件校验 + 拦截手段自校验（不可用时降级并记缺口；复制自 release-yaml.test.sh L232-283） ============

if command -v actionlint >/dev/null 2>&1; then
  # 正：release.yml 整文件合法（零错）
  ok=1
  if [ "$wf_ready" -eq 1 ] && actionlint "$WF" >/dev/null 2>&1; then ok=0; fi
  report "T15 actionlint release.yml 零错（整文件合法性）" "$ok"

  # 负（拦截手段自校验）：注入语法错误的临时副本必须被拦截（非零）
  ok=1
  BROKEN="$TMP_ROOT/release.broken.yml"
  if { cat "$NORM"; printf '\n  broken-injection: [unclosed-flow-seq\n'; } >"$BROKEN" && [ -s "$BROKEN" ]; then
    if actionlint "$BROKEN" >/dev/null 2>&1; then
      note "T15: 注入语法错误的副本 actionlint 返回 0（拦截失效）"
    else
      ok=0
    fi
  else
    note "T15: 注入副本构造失败"
  fi
  report "T15 注入语法错误副本被 actionlint 拦截（非零，自校验）" "$ok"
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
          note "T15 降级: python yaml.safe_load 解析 release.yml 失败（YAML 非法）"
        fi
      else
        # 终极降级：grep 键配平粗检（非完整解析，记缺口）
        if grep -q '^on:' "$NORM" && grep -q '^jobs:' "$NORM" \
           && [ "$(grep -cE '^[A-Za-z_][A-Za-z0-9_-]*:' "$NORM")" -ge 4 ]; then
          ok=0
          note "T15 终极降级: pyyaml 不可用，仅 grep 键配平粗检（非完整解析，记缺口）"
        else
          note "T15 终极降级: grep 键配平粗检未过（on:/jobs: 键缺失）"
        fi
      fi
    else
      note "T15 降级: python 不可用且无其他 YAML 解析手段"
    fi
  fi
  report "T15 降级 YAML 结构断言（release.yml 可解析/键配平，降级分支计入总数）" "$ok"
  echo "WARN 降级告警：actionlint 不可用——T15 仅覆盖 YAML 合法性（python yaml.safe_load 或键配平）+ T1-T14 结构断言；语法拦截自校验（注入负路径）未执行，记缺口（test-strategy：缺口·拦截手段依赖）"
fi

# ============ 汇总 ============
echo "$PASS/$TOTAL passed"
if [ "$FAIL" -ne 0 ]; then
  exit 1
fi
exit 0
