import io
import re
import qrcode
from qrcode.image.svg import SvgPathImage

# 从登录日志取当前二维码 URL
log = io.open("/tmp/cookie-login3.log", encoding="utf-8", errors="replace").read()
m = re.search(r"https://115\.com/scan/dg-[a-f0-9]+", log)
assert m, "日志中未找到二维码 URL"
url = m.group(0)

qr = qrcode.QRCode(error_correction=qrcode.constants.ERROR_CORRECT_M)
qr.add_data(url)
qr.make()
img = qr.make_image(image_factory=SvgPathImage)
svg = img.to_string().decode()

# 提取 path 的 d 属性
pm = re.search(r'<path[^>]*\bd="([^"]+)"', svg)
assert pm, "SVG path 提取失败"
d = pm.group(1)
vb = qr.modules_count

frag = (
    '<div id="cm-qr-login" style="display:flex;flex-direction:column;align-items:center;gap:12px;padding:20px;">\n'
    '  <p style="margin:0;font-size:15px;font-weight:500;">请用 <b>115 App</b> 扫描二维码，并在手机上点「确认登录」</p>\n'
    '  <div style="background:#fff;padding:18px;border-radius:12px;">\n'
    f'    <svg width="300" height="300" viewBox="0 0 {vb} {vb}" shape-rendering="crispEdges" role="img" aria-label="115 扫码登录二维码">\n'
    f'      <rect width="{vb}" height="{vb}" fill="#ffffff"/>\n'
    f'      <path d="{d}" fill="#000000"/>\n'
    '    </svg>\n'
    '  </div>\n'
    f'  <p style="margin:0;font-size:11px;opacity:.55;font-family:monospace;">{url}</p>\n'
    '</div>\n'
)

out = r"C:\Users\Admin\.zcode\v2\visualizations\60b385f4284ea086feb6305e22a590a742b3cbdbe038c89a94cf924b0dbe0ba1\cm-qr-login.html"
io.open(out, "w", encoding="utf-8").write(frag)
# 验证：d 属性非空且是真实 path 数据
assert len(d) > 100, f"path 数据过短: {len(d)}"
print(f"fragment ok: modules={vb}, path_len={len(d)}, url={url}")
