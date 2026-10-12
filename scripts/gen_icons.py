"""生成 CloudMount 应用图标（纯 stdlib：PNG + PNG-in-ICO）。

品牌色：云盘蓝渐变（简化为双色方块图案——上白下蓝，象征"云在上、盘在下"）。
"""
import struct
import zlib
import os

OUT = os.path.join(os.path.dirname(__file__), '..', 'apps', 'desktop', 'src-tauri', 'icons')


def make_png(size):
    """上 1/3 白（云）、下 2/3 蓝（盘），中间一条浅色分隔。"""
    white = (255, 255, 255)
    blue = (30, 100, 220)
    light = (150, 200, 255)
    rows = []
    for y in range(size):
        if y < size // 3:
            color = white
        elif y < size // 3 + max(1, size // 16):
            color = light
        else:
            color = blue
        rows.append(b'\x00' + bytes(color) * size)
    raw = b''.join(rows)

    def chunk(tag, data):
        c = tag + data
        return struct.pack('>I', len(data)) + c + struct.pack('>I', zlib.crc32(c))

    ihdr = struct.pack('>IIBBBBB', size, size, 8, 2, 0, 0, 0)
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', ihdr)
            + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))


def make_ico(png_bytes):
    """PNG-in-ICO（Vista+ 原生支持）。"""
    header = struct.pack('<HHH', 0, 1, 1)  # reserved, type=icon, count=1
    entry = struct.pack('<BBBBHHII', 32, 32, 0, 0, 1, 32, len(png_bytes), 22)
    return header + entry + png_bytes


os.makedirs(OUT, exist_ok=True)
for size in (32, 128):
    png = make_png(size)
    with open(os.path.join(OUT, f'{size}x{size}.png'), 'wb') as f:
        f.write(png)
    print(f'{size}x{size}.png: {len(png)} bytes')

ico = make_ico(make_png(32))
with open(os.path.join(OUT, 'icon.ico'), 'wb') as f:
    f.write(ico)
print(f'icon.ico: {len(ico)} bytes')
