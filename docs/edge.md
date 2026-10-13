# 边缘网关部署指南

把 115 挂到局域网/边缘设备（NAS、树莓派、软路由、VPS）：`cloudmountd --serve`
提供 WebDAV 出口（`/dav/`）与 web-ui 状态页，局域网任何设备可挂载读写。

## 快速开始

```bash
# 1. 登录（三选一）
cloudmount login                          # 官方 OpenAPI 扫码（推荐，需 AppID）
cloudmount login --with-cookie-channel    # Cookie 补充通道（需知情确认）
cloudmount login --cookie "UID=..;CID=..;SEID=..;KID=.."  # 浏览器复制注入

# 2. 启动网关
cloudmountd --serve --listen 0.0.0.0:8384 \
  --auth myuser:mypass \
  --web-ui-dir /opt/cloudmount/web-ui

# 3. 局域网设备挂载
# Windows：资源管理器 → 映射网络驱动器 → http://<ip>:8384/dav
# macOS：Finder → 前往 → 连接服务器 → http://<ip>:8384/dav
# Linux：mount -t davfs http://<ip>:8384/dav /mnt/115
# rclone：rclone config 配 webdav → rclone mount :webdav: /mnt/115
```

## 低内存预设

边缘设备（512MB RAM）的运行基线：

| 项 | 值 | 说明 |
|---|---|---|
| 运行时 | tokio current_thread + 阻塞池(4) | 单线程 async + 阻塞线程池 |
| 发布构建 | `opt-level=z + lto=fat + strip` | 静态二进制约 5-8MB |
| 空闲 RSS | 目标 10-20MB | 无常驻大缓冲 |
| 大文件上传 | spool 到 `/tmp` | PUT 中转，不驻留内存 |

内存敏感设备建议（systemd 部署时已内置）：

```ini
MemoryMax=64M
MemoryHigh=48M
```

 musl 静态编译产物无 glibc 依赖，Alpine/scratch 容器直接运行：

```bash
cross build -p cloudmount-daemon --target aarch64-unknown-linux-musl --release
# 产物：target/aarch64-unknown-linux-musl/release/cloudmountd
```

## systemd 部署

```bash
sudo useradd -r -s /usr/sbin/nologin cloudmount
sudo install -m 0755 target/release/cloudmountd /usr/local/bin/
sudo cp -r web-ui/dist /opt/cloudmount/web-ui
sudo cp packaging/systemd/cloudmountd.service /etc/systemd/system/
sudo cp packaging/systemd/cloudmountd.default /etc/default/cloudmountd
sudo systemctl daemon-reload && sudo systemctl enable --now cloudmountd
```

服务单元内置安全加固（ProtectSystem=strict、NoNewPrivileges 等）与
内存上限；配置经 `/etc/default/cloudmountd` 注入。

## Tailscale 组网（远程安全访问）

不暴露公网端口的最简方案：设备与手机/电脑都装 Tailscale（免费，
WireGuard 组网），网关监听 Tailscale 接口：

```bash
# 设备上
curl -fsSL https://tailscale.com/install.sh | sh
tailscale up
# 记下 100.x.y.z 地址

# 网关只监听 Tailscale 接口（内网外都够不着）
cloudmountd --serve --listen 100.x.y.z:8384 --auth myuser:mypass
```

手机 Tailscale 开启后，115 挂载客户端连 `http://100.x.y.z:8384/dav`。
HTTPS 由 Tailscale 提供（`tailscale cert` + --tls-cert/--tls-key，
或 tailscale serve 代理）。

## 安全建议

- **务必启用 `--auth`**：WebDAV 写操作直通 115（删除进回收站）
- 优先 Tailscale/WireGuard，其次仅内网监听，避免直接暴露公网
- Cookie 通道有账号风险（见登录时的风险声明），公网暴露请用官方通道
- v1 无内置 TLS：外网暴露请前置反向代理（Caddy/nginx）或 Tailscale

## 已知边界

- PUT 非流式：大文件先落 spool 再上传（`/tmp` 需有足够空间）
- 单进程单监听：多通道/多账号请起多实例（不同端口）
- v1 无速率限制配置：依赖 Provider 内建的保守限速
