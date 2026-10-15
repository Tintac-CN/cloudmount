#!/usr/bin/env bash
#
# scripts/package-macos.sh — mac 双架构 tar.gz 打包单一事实来源（CI 与本地 dry-run 同脚本同代码路径）
#
# 用法：
#   package-macos.sh --bin-dir <产物目录> --out <输出目录> [--dry-run] [--arch aarch64|x86_64]
#     --bin-dir  编译产物目录：根级须含 cloudmountd + cloudmount（可执行）+ README.md + edge.md
#     --out      输出目录：tar.gz 写其根；--dry-run 时写 <out>/dry-run/ 专属子目录（与 CI 上传路径隔离）
#     --dry-run  本地预检：bin-dir 内占位文件直接打包（无需真实 Mach-O）
#     --arch     可选，aarch64（默认）| x86_64（CI 按 target 传参）
#
# 本地预检说明（AC3/T8；详见 docs/aidlc/macos-release/prd.md）：
#   1. 本地（Windows/Git Bash）预检只做一道 dry-run——构造含四件占位产物的目录后：
#        bash scripts/package-macos.sh --bin-dir <占位目录> --out <输出目录> --dry-run
#      判据 = 退出码 0，且 <out>/dry-run/cloudmount-darwin-<arch>.tar.gz 生成、
#      tar -tzf 清单恰为根级 cloudmountd/cloudmount/README.md/edge.md 四项
#      （直接跑 scripts/package-macos.test.sh 可自动断言以上全部，9 用例须全绿）。
#   2. 编译面预检已上移 CI（本地无 darwin C 工具链，bundled C build scripts 使
#      cargo check 双 target 不可行）：tag 前手动触发 .github/workflows/check-macos.yml
#      （workflow_dispatch），判据 = aarch64-apple-darwin 与 x86_64-apple-darwin
#      两个 cargo check --target 均绿（job exit 0）。确认绿后再打 v* tag。
#
# 失败语义：set -euo pipefail，任一步失败整体非零退出（CI job 失败 → release job 因
# needs 门控不发布）；必含文件硬校验先于一切产出动作，缺任一立即失败且不产出 tar——
# 与 linux 腿对文档静默容错不同，mac 腿硬校验（AC2 必有内容，杜绝残缺 tar 进上传步）。

set -euo pipefail

usage() {
  echo "usage: package-macos.sh --bin-dir <产物目录> --out <输出目录> [--dry-run] [--arch aarch64|x86_64]" >&2
  echo "  --bin-dir  产物目录：根级须含 cloudmountd + cloudmount（可执行）+ README.md + edge.md" >&2
  echo "  --out      输出目录（--dry-run 时产物写 <out>/dry-run/ 专属子目录）" >&2
  echo "  --arch     aarch64（默认）| x86_64" >&2
}

BIN_DIR=""
OUT_DIR=""
DRY_RUN=0
ARCH="aarch64"

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
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --arch)
      [ $# -ge 2 ] || { echo "error: --arch 需要一个参数" >&2; usage; exit 2; }
      ARCH="$2"
      shift 2
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

# 必含文件硬校验（先于一切产出动作；缺任一立即失败，不产出任何 tar）
[ -d "$BIN_DIR" ] || { echo "error: 产物目录不存在: $BIN_DIR" >&2; exit 1; }
for f in cloudmountd cloudmount; do
  [ -x "$BIN_DIR/$f" ] || { echo "error: 缺少可执行文件（-x 判定失败）: $BIN_DIR/$f" >&2; exit 1; }
done
for f in README.md edge.md; do
  [ -f "$BIN_DIR/$f" ] || { echo "error: 缺少必含文档: $BIN_DIR/$f" >&2; exit 1; }
done

# dry-run 产物只写 <out>/dry-run/ 专属子目录，与 CI 真实上传路径隔离；
# 非 dry-run（CI 用）同布局逻辑，从 bin-dir 打真实二进制
if [ "$DRY_RUN" -eq 1 ]; then
  OUT_DIR="$OUT_DIR/dry-run"
fi
TARBALL="$OUT_DIR/cloudmount-darwin-$ARCH.tar.gz"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

# 根级拍平四件（对齐 linux 腿 `cd dist && tar czf … .` 形态；-p 保留可执行位）
cp -p "$BIN_DIR/cloudmountd" "$BIN_DIR/cloudmount" "$BIN_DIR/README.md" "$BIN_DIR/edge.md" "$STAGE/"

mkdir -p "$OUT_DIR"
tar czf "$TARBALL" -C "$STAGE" .

echo "packaged: $TARBALL"
