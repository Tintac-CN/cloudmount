# CloudMount 边缘节点（NAS / 树莓派 / 无头设备）

同一份 `cloudmountd`（Rust，musl 静态链接）以服务模式运行：
WebDAV 出口 + 后台同步。目标 **512MB 内存设备可跑**（空闲 RSS 5–15MB）。

## 形态

| 需求 | 方案 |
|---|---|
| 局域网/远程设备挂载 | WebDAV（`dav-server-rs` + rustls），Windows/macOS/手机原生支持 |
| 设备本地直接挂盘 | FUSE（`fuser`，关 libfuse feature，纯 Rust） |
| 跨设备管理 UI | `cloudmountd` 托管 web-ui 静态文件，浏览器访问 `:8384` |
| 公网安全访问 | Tailscale（`tailscale serve`）或自管 WireGuard；不开公网端口 |

## 低内存纪律

- tokio `current_thread` 运行时 + `max_blocking_threads(4)`
- 传输 chunk 4MB、并发受限；VFS 缓存落盘不占 RAM
- systemd `MemoryMax=256M` 兜底

## 部署（计划）

- `Dockerfile`：多阶段构建（rust:alpine → scratch），多架构（amd64/arm64/armv7）
- `systemd/cloudmountd.service`：NAS/裸机部署
- 静态二进制从 GitHub Releases 下载即用

> SMB 出口：Rust 无成熟 SMB server 实现，需要 SMB 时建议设备直接装 smbd，
> 把 WebDAV 挂载点再通过 Samba 共享出去。
