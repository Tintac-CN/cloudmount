#!/usr/bin/env bash
#
# scripts/package-pkg-macos.sh — mac 双架构 .pkg 安装包打包单一事实来源（CI 与本地 dry-run 同脚本）
#
# 用法：
#   package-pkg-macos.sh --bin-dir <产物目录> --out <输出目录> [--arch aarch64|x86_64]
#                        --version <数值版本> [--dry-run]
#     --bin-dir  编译产物目录：根级须含 cloudmountd + cloudmount（可执行，-x 硬校验）
#     --out      输出目录：.pkg 写其根（命名 cloudmount-darwin-<arch>.pkg，与 mac tar 命名同构）
#     --arch     可选，aarch64（默认）| x86_64（CI 按 target 传参）
#     --version  数值版本：非 dry-run 必填（pkgbuild 需要），--dry-run 可省。
#                由 CI 以 tag 去 v 前缀注入（与 MSI ProductVersion 同源，prd M3）；
#                本脚本不自行推导版本；rc 形态 tag 不在发布语义内（prd AC5）
#     --dry-run  本地预检：前置硬校验 + staging/usr/local/bin 布局就绪即退出 0
#
# dry-run 边界声明（prd 语义对齐「安装（pkg）」节 / gate1 复审 R2-MAJOR-1）：
#   pkgbuild 为 macOS 独占工具，本地（Windows/Git Bash）无法执行——dry-run 仅覆盖
#   「前置硬校验 + staging/usr/local/bin 布局归集」段；pkgbuild 调用段只在 CI（macos-15）
#   执行。与 package-macos.sh 的全路径 dry-run 不对等，属已声明边界而非缺陷。
#
# 安装面（prd A1 / gate1 Q4）：纯文件铺放——staging/usr/local/bin/ 下仅 cloudmountd +
# cloudmount 两个文件；无安装前/后脚本钩子（--root 直铺，不构造额外组件目录）、
# 无系统服务注册、无桌面快捷方式。identifier=io.github.tintac-cn.cloudmount
# （反转域名约定），receipt 经 pkgutil 可查。
#
# 失败语义：set -euo pipefail，任一步失败整体非零退出（CI job 失败 → release job 因
# needs 门控不发布）；前置硬校验先于一切产出动作，缺任一立即失败且不产出 pkg。

set -euo pipefail

usage() {
  echo "usage: package-pkg-macos.sh --bin-dir <产物目录> --out <输出目录> [--arch aarch64|x86_64] --version <数值版本> [--dry-run]" >&2
  echo "  --bin-dir  产物目录：根级须含 cloudmountd + cloudmount（可执行）" >&2
  echo "  --out      输出目录：.pkg 写其根（cloudmount-darwin-<arch>.pkg）" >&2
  echo "  --arch     aarch64（默认）| x86_64" >&2
  echo "  --version  数值版本（非 dry-run 必填，--dry-run 可省）" >&2
}

BIN_DIR=""
OUT_DIR=""
ARCH="aarch64"
VERSION=""
DRY_RUN=0

while [ $# -gt 0 ]; do
  case "$1" in
    --bin-dir)
      [ $# -ge 2 ] || { echo "error: --bin-dir 需要一个目录参数" >&2; usage; exit 2; }
      BIN_DIR="$2"
      shift 2
      ;;
    --out)
      [ $# -ge 2 ] || { echo "error: --out 需要一个目录参数" >&2; usage; exit 2; }
      OUT_DIR="$2"
      shift 2
      ;;
    --arch)
      [ $# -ge 2 ] || { echo "error: --arch 需要一个参数" >&2; usage; exit 2; }
      ARCH="$2"
      shift 2
      ;;
    --version)
      [ $# -ge 2 ] || { echo "error: --version 需要一个参数" >&2; usage; exit 2; }
      VERSION="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    *)
      echo "error: 未知参数: $1" >&2
      usage
      exit 2
      ;;
  esac
done

[ -n "$BIN_DIR" ] || { echo "error: 缺少必填参数 --bin-dir" >&2; usage; exit 2; }
[ -n "$OUT_DIR" ] || { echo "error: 缺少必填参数 --out" >&2; usage; exit 2; }
case "$ARCH" in
  aarch64 | x86_64) ;;
  *) echo "error: --arch 仅支持 aarch64|x86_64，收到: $ARCH" >&2; usage; exit 2 ;;
esac
if [ "$DRY_RUN" -ne 1 ] && [ -z "$VERSION" ]; then
  echo "error: 非 dry-run 必填 --version（pkgbuild 需要；CI 以 tag 去 v 前缀注入）" >&2
  usage
  exit 2
fi

# 前置硬校验（先于一切产出动作；缺任一立即失败，不产出任何 pkg）
[ -d "$BIN_DIR" ] || { echo "error: 产物目录不存在: $BIN_DIR" >&2; exit 1; }
for f in cloudmountd cloudmount; do
  [ -x "$BIN_DIR/$f" ] || { echo "error: 缺少可执行文件（-x 判定失败）: $BIN_DIR/$f" >&2; exit 1; }
done

# staging 归集：铺 /usr/local/bin/ 布局（安装目标 = /usr/local/bin，gate1 Q4；
# Apple Silicon 上该目录缺省不存在时由包创建，属 root）
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$STAGE/usr/local/bin"
cp -p "$BIN_DIR/cloudmountd" "$BIN_DIR/cloudmount" "$STAGE/usr/local/bin/"

if [ "$DRY_RUN" -eq 1 ]; then
  echo "dry-run ok: 前置硬校验与 staging/usr/local/bin 布局就绪（2 文件）；pkgbuild 调用段 macOS 独占，此处不执行（R2-MAJOR-1 边界）"
  exit 0
fi

mkdir -p "$OUT_DIR"
OUT_PKG="$OUT_DIR/cloudmount-darwin-$ARCH.pkg"
pkgbuild --root "$STAGE" --identifier io.github.tintac-cn.cloudmount --version "$VERSION" --out "$OUT_PKG"

echo "packaged: $OUT_PKG"
