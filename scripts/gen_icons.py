"""生成 CloudMount 应用图标（纯 stdlib：PNG / PNG-in-ICO / PNG-in-ICNS / Rust RGBA 常量源）。

品牌沿既有像素设计：上 1/3 白（云）/浅色分隔/下 2/3 蓝（盘）双色方块。
单一管线（intent S5 / prd AC1）：全部产物输出 apps/desktop/icons/——
  icon.ico      多帧 PNG-in-ICO（首帧 32×32 基线，另含 16/48/256），winres 嵌 exe + MSI Icon 元素
  <n>x<n>.png   PNG 组（32×32 + 128×128 基线），ico/icns 生成输入兼源素材
  icon.icns     PNG-in-ICNS（ic11 32 / ic07 128 / ic08 256）——首日验证③定稿：stdlib 手写、
                预生成入仓（免 CI mac iconutil 依赖，.app bundle 直接携带）
  icons.rs      Rust RGBA 常量源（32×32，零 crate 依赖）——tray-icon `Icon::from_rgba` /
                egui `IconData` 消费（gate1 r2 裁决的运行期图标解码链）

幂等：确定性编码（无时间戳/随机量），重复运行字节一致（write-if-changed 逐文件报告）。
stale 路径清零：输出路径为 apps/desktop/icons/（不再指向已废弃的旧图标目录，prd AC1②）。
"""
import os
import struct
import zlib

OUT = os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "..", "apps", "desktop", "icons"
)

WHITE = (255, 255, 255)
BLUE = (30, 100, 220)
LIGHT = (150, 200, 255)

PNG_MAGIC = b"\x89PNG\r\n\x1a\n"


def row_color(size, y):
    """按既有品牌像素设计取第 y 行颜色：上 1/3 白（云）/浅色分隔/下 2/3 蓝（盘）。"""
    if y < size // 3:
        return WHITE
    if y < size // 3 + max(1, size // 16):
        return LIGHT
    return BLUE


def make_png(size):
    """RGB PNG（filter 0 逐行编码；确定性 zlib 压缩）。"""
    rows = []
    for y in range(size):
        rows.append(b"\x00" + bytes(row_color(size, y)) * size)
    raw = b"".join(rows)

    def chunk(tag, data):
        c = tag + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c))

    ihdr = struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0)
    return (
        PNG_MAGIC
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw))
        + chunk(b"IEND", b"")
    )


def make_ico(entries):
    """多帧 PNG-in-ICO（Vista+ 原生支持）。entries: [(size, png_bytes), ...]（首项为首帧）。"""
    header = struct.pack("<HHH", 0, 1, len(entries))  # reserved, type=icon, count
    offset = 6 + 16 * len(entries)
    directory = b""
    for size, png in entries:
        dim = 0 if size >= 256 else size  # ICO 约定：0 表示 256
        directory += struct.pack(
            "<BBBBHHII", dim, dim, 0, 0, 1, 32, len(png), offset
        )
        offset += len(png)
    return header + directory + b"".join(png for _, png in entries)


def make_icns(entries):
    """PNG-in-ICNS 容器（ICNS 块：4 字节类型 + 大端 u32 块长（含头）+ PNG 数据）。
    entries: [(b'ic07', png_bytes), ...]——现代 macOS（10.7+）原生支持 PNG 负载块。"""
    chunks = b""
    for icns_type, png in entries:
        chunks += icns_type + struct.pack(">I", 8 + len(png)) + png
    return b"icns" + struct.pack(">I", 8 + len(chunks)) + chunks


def make_icons_rs(size):
    """Rust RGBA 常量源（尺寸 + RGBA 字节组；32×32=4096 字节，tray/窗口图标消费）。"""
    rgba = []
    for y in range(size):
        r, g, b = row_color(size, y)
        rgba.extend([r, g, b, 255] * size)
    lines = []
    lines.append("// 自动生成，请勿手改 —— scripts/gen_icons.py（幂等再生产物）。")
    lines.append("// CloudMount 品牌 RGBA 位图（上 1/3 白/浅色分隔/下 2/3 蓝）：")
    lines.append("// 供 tray-icon `Icon::from_rgba` 与 egui `IconData` 消费（零 crate 依赖解码链）。")
    lines.append("")
    lines.append(f"pub const ICON_{size}_WIDTH: u32 = {size};")
    lines.append(f"pub const ICON_{size}_HEIGHT: u32 = {size};")
    lines.append(f"pub const ICON_{size}_RGBA: [u8; {len(rgba)}] = [")
    for i in range(0, len(rgba), 16):
        chunk = ", ".join(f"0x{v:02x}" for v in rgba[i : i + 16])
        lines.append(f"    {chunk},")
    lines.append("];")
    lines.append("")
    return "\n".join(lines)


def write_if_changed(path, data):
    """确定性写入：内容一致时不落盘（幂等报告 `unchanged`）。"""
    if isinstance(data, str):
        data = data.encode("utf-8")
    if os.path.isfile(path):
        with open(path, "rb") as f:
            if f.read() == data:
                print(f"unchanged {os.path.basename(path)} ({len(data)} bytes)")
                return False
    with open(path, "wb") as f:
        f.write(data)
    print(f"wrote     {os.path.basename(path)} ({len(data)} bytes)")
    return True


def main():
    os.makedirs(OUT, exist_ok=True)

    png32 = make_png(32)
    png128 = make_png(128)

    # ICO：首帧 32×32（TP-11 基线断言口径），多帧覆盖 Explorer 常用档位
    ico = make_ico([(32, png32), (16, make_png(16)), (48, make_png(48)), (256, make_png(256))])

    # ICNS：ic11=32×32 / ic07=128×128 / ic08=256×256（256 为 icns 专用中间产物，不落盘）
    icns = make_icns([(b"ic11", png32), (b"ic07", png128), (b"ic08", make_png(256))])

    write_if_changed(os.path.join(OUT, "icon.ico"), ico)
    write_if_changed(os.path.join(OUT, "32x32.png"), png32)
    write_if_changed(os.path.join(OUT, "128x128.png"), png128)
    write_if_changed(os.path.join(OUT, "icon.icns"), icns)
    write_if_changed(os.path.join(OUT, "icons.rs"), make_icons_rs(32))


if __name__ == "__main__":
    main()
