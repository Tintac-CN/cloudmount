#!/usr/bin/env bash
#
# scripts/package-dmg-macos.sh — mac desktop .app + .dmg 打包单一事实来源（CI 与本地 dry-run 同脚本）
#
# 用法：
#   package-dmg-macos.sh --bin-dir <产物目录> --out <输出目录> [--icns <icon.icns>] --version <数值版本> [--dry-run]
#     --bin-dir  编译产物目录：根级须含 cloudmount-desktop（可执行，-x 硬校验——aarch64-only 单架构）
#     --out      输出目录：cloudmount-desktop.dmg 写其根（命名 Construction 落，镜像 pkg 命名先例）
#     --icns     可选，默认 scripts 同仓 apps/desktop/icons/icon.icns（gen_icons 预生成入仓链——
#                首日验证③定稿：stdlib 手写 PNG-in-ICNS，免 CI iconutil 依赖）
#     --version  数值版本：非 dry-run 必填（注入 Info.plist CFBundleShortVersionString/BundleVersion），
#                由 CI 以 tag 去 v 前缀注入（与 MSI/pkg 版本同源，prd M3）；rc 形态 tag 不在发布语义内
#     --dry-run  本地预检：前置硬校验 + cloudmount.app bundle 布局就绪即退出 0
#
# dry-run 边界声明（沿 package-pkg-macos.sh R2-MAJOR-1 边界模式）：
#   hdiutil 为 macOS 独占工具，本地（Windows/Git Bash）无法执行——dry-run 仅覆盖
#   「前置硬校验 + cloudmount.app staging 布局归集（Contents/{MacOS,Resources,Info.plist}）」段；
#   /Applications 拖拽链接与 hdiutil create（dmg 装配段）只在 CI（macos-15）执行。
#
# 分发面（prd A1 / AC4）：.app 只走 .dmg（daemon/cli 既有 pkg 不含 .app 且零变动——TP-08）；
#   aarch64-only（沿 macos-15 单 runner 先例；Intel mac 无 desktop 交付为已知取舍）；
#   未签名：首启 Gatekeeper 引导沿既有口径（release body 的 .dmg 行文案）。
#
# 失败语义：set -euo pipefail，任一步失败整体非零退出（CI job 失败 → release job 因 needs 门控
# 不发布）；前置硬校验（binary/icns 存在性）先于一切产出动作，缺任一立即失败且不产出 dmg。

set -euo pipefail

usage() {
  echo "usage: package-dmg-macos.sh --bin-dir <产物目录> --out <输出目录> [--icns <icon.icns>] --version <数值版本> [--dry-run]" >&2
  echo "  --bin-dir  产物目录：根级须含 cloudmount-desktop（可执行）" >&2
  echo "  --out      输出目录：cloudmount-desktop.dmg 写其根" >&2
  echo "  --icns     icns 图标路径（默认 apps/desktop/icons/icon.icns）" >&2
  echo "  --version  数值版本（非 dry-run 必填，--dry-run 可省）" >&2
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR=""
OUT_DIR=""
VERSION=""
DRY_RUN=0
ICNS="$SCRIPT_DIR/../apps/desktop/icons/icon.icns"

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
    --icns)
      [ $# -ge 2 ] || { echo "error: --icns 需要一个文件参数" >&2; usage; exit 2; }
      ICNS="$2"
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
if [ "$DRY_RUN" -ne 1 ] && [ -z "$VERSION" ]; then
  echo "error: 非 dry-run 必填 --version（Info.plist 版本字段需要；CI 以 tag 去 v 前缀注入）" >&2
  usage
  exit 2
fi
# 版本格式校验（与 MSI/pkg 侧 case 校验对称，沿 gate2 准出回填先例）：数值格式外一律拒绝
case "$VERSION" in
  '') ;;
  *[!0-9.]*) echo "error: --version 需数值格式，收到: $VERSION（rc 形态 tag 不在发布语义内）" >&2
    usage
    exit 2 ;;
esac

# 前置硬校验（先于一切产出动作；缺任一立即失败，不产出任何 dmg——TP-12 fail-fast 链）
[ -d "$BIN_DIR" ] || { echo "error: 产物目录不存在: $BIN_DIR" >&2; exit 1; }
[ -x "$BIN_DIR/cloudmount-desktop" ] || { echo "error: 缺少可执行文件（-x 判定失败）: $BIN_DIR/cloudmount-desktop（aarch64-only 单架构）" >&2; exit 1; }
[ -f "$ICNS" ] || { echo "error: 缺少 icns 图标（gen_icons 预生成链产物）: $ICNS" >&2; exit 1; }

# staging：cloudmount.app bundle 布局归集（MacOS 二进制 + Resources icns + Info.plist）
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
APP="$STAGE/cloudmount.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp -p "$BIN_DIR/cloudmount-desktop" "$APP/Contents/MacOS/cloudmount-desktop"
cp -p "$ICNS" "$APP/Contents/Resources/cloudmount.icns"
cat >"$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>CloudMount</string>
  <key>CFBundleDisplayName</key>
  <string>CloudMount</string>
  <key>CFBundleIdentifier</key>
  <string>io.github.tintac-cn.cloudmount.desktop</string>
  <key>CFBundleExecutable</key>
  <string>cloudmount-desktop</string>
  <key>CFBundleIconFile</key>
  <string>cloudmount</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION:-0.0.0}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION:-0.0.0}</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST
# bundle 布局已就绪自检（三件在位——防布局漂移；dry-run 与 CI 同此判据）
[ -x "$APP/Contents/MacOS/cloudmount-desktop" ] || { echo "error: bundle 二进制布局异常: $APP/Contents/MacOS/cloudmount-desktop" >&2; exit 1; }
[ -f "$APP/Contents/Resources/cloudmount.icns" ] || { echo "error: bundle icns 布局异常: $APP/Contents/Resources/cloudmount.icns" >&2; exit 1; }
[ -f "$APP/Contents/Info.plist" ] || { echo "error: bundle Info.plist 布局异常: $APP/Contents/Info.plist" >&2; exit 1; }

if [ "$DRY_RUN" -eq 1 ]; then
  echo "dry-run ok: 前置硬校验与 cloudmount.app bundle 布局就绪（Contents/MacOS/cloudmount-desktop + Contents/Resources/cloudmount.icns + Contents/Info.plist）；/Applications 拖拽链接与 hdiutil 装配段 macOS 独占，此处不执行（R2-MAJOR-1 边界）"
  exit 0
fi

mkdir -p "$OUT_DIR"
# —— 以下 macOS 独占（CI macos-15 执行） ——
# dmg 内 /Applications 拖拽安装语义（Finder 拖入 /Applications 即装）
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "CloudMount" -srcfolder "$STAGE" -ov -format UDZO "$OUT_DIR/cloudmount-desktop.dmg"

echo "packaged: $OUT_DIR/cloudmount-desktop.dmg"
