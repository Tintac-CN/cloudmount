# CloudMount 云挂

> 把 115 网盘变成你所有设备上的"同步盘"—— OneDrive 式体验、Rust 极致轻量、全开源。
>
> **状态：v0.1.0 已发布**（M0–M5 完成，真实 115 端到端验证通过）。

## 安装

| 平台 | 方式 |
|---|---|
| Linux（x64/arm64） | [Release](https://github.com/Tintac-CN/cloudmount/releases) 下载 tarball（静态 musl 二进制 + systemd unit） |
| Windows | Release 下载 zip（`cloudmountd.exe` + `cloudmount.exe`） |
| Docker（多架构） | `docker pull ghcr.io/tintac-cn/cloudmount:v0.1.0` |

## 快速开始

### 1. 登录（三选一）

```bash
# 官方 OpenAPI（推荐；AppID 申请 https://open.115.com/ ）
cloudmount login

# Cookie 补充通道（无需 AppID；扫码 + 知情确认）
cloudmount login --with-cookie-channel

# 浏览器 Cookie 直接注入（F12 → 应用 → Cookie 复制）
cloudmount login --cookie "UID=..;CID=..;SEID=..;KID=.."
```

### 2. Windows：OneDrive 式占位符挂载

```powershell
cloudmountd --placeholder-mount --sync-root D:\Cloud
# 资源管理器出现"CloudMount 云挂"（与 OneDrive 并排）：
#   - 文件显示为占位符（不占磁盘），双击按需从云端拉取
#   - 状态列："在此设备上可用" / "联机时可用"
#   - 右键："释放空间"、"始终保留在此设备上"
```

### 3. Linux / NAS / 树莓派：WebDAV 边缘网关

```bash
cloudmountd --serve --listen 0.0.0.0:8384 --auth user:pass
# 局域网设备挂载 http://<ip>:8384/dav/（资源管理器/Finder/davfs/rclone）
# 浏览器打开 http://<ip>:8384/ 进入管理页（状态/缓存/设置/日志）
```

Docker 一条命令：

```bash
docker run -d -p 8384:8384 -v cm-data:/var/lib/cloudmount \
  ghcr.io/tintac-cn/cloudmount:v0.1.0 \
  --serve --listen 0.0.0.0:8384 --auth user:pass
```

### 4. 配置文件（可选——所有参数都有合理默认）

```
%APPDATA%\cloudmount\config.toml（Windows）
~/.local/share/cloudmount/config.toml（Linux）
```

```toml
app_id = "你的AppID"          # 官方通道
sync_root = "D:\\Cloud"        # 同步根
channel = "auto"               # auto（官方+Cookie兜底）/ open / cookie
listen = "0.0.0.0:8384"        # serve 监听

[serve]
auth = "user:pass"             # WebDAV Basic 认证
# tls_cert = "/path/cert.pem"  # HTTPS（与 tls_key 成对）
# tls_key = "/path/key.pem"

[sync]
propagate_remote_deletes = false   # 安全默认：删除不传播
propagate_local_deletes = false
```

优先级：CLI flag > 环境变量 > 配置文件 > 内置默认。

## 是什么

- **Windows**：CfAPI 占位符挂载——按需下载、资源管理器状态图标、
  "始终保留/释放空间"右键动词（真机验证：与 OneDrive 行为一致）
- **Linux / 边缘设备**：WebDAV 出口 + web-ui 管理页，512MB 内存设备可跑
  （空闲 RSS 10–20MB；musl 静态二进制 4MB）
- **双通道**：官方 OpenAPI 为主；Cookie 通道为默认关闭的兜底（自动降级，
  见[风险声明](#cookie-通道风险声明)）
- **web-ui 管理页**：边缘网关浏览器管理页（零框架 2.5KB gzip）
- **桌面端**：egui 原生 UI（apps/desktop，不使用 web-ui）
- **全开源** AGPL-3.0

## 架构一图

```
┌────────────────────────────────────────────────────────────┐
│  UI：egui 桌面壳 / web-ui 管理页（边缘浏览器，--serve 托管）  │
├────────────────────────────────────────────────────────────┤
│  挂载后端（cloudmount-vfs::MountBackend，可插拔）             │
│   Windows: CfAPI ✅ → 自研 minifilter(长期)                 │
│   macOS: FileProvider *计划*   Linux/边缘: WebDAV ✅        │
├────────────────────────────────────────────────────────────┤
│  同步引擎（Rust）：RDC/FSEvents/inotify 本地监控             │
│   + usertime 轮询远端 + 队列/限速/冲突处理 + .git 防护       │
├────────────────────────────────────────────────────────────┤
│  驱动（cloudmount-driver115）：官方 OpenAPI 主通道           │
│   + Cookie 兜底通道（默认关）+ ProviderRouter 自动降级        │
├────────────────────────────────────────────────────────────┤
│  基座：SQLite 元数据（bundled）· ureq+rustls 传输            │
│   · tokio current_thread · musl 静态 ARM 交叉编译            │
└────────────────────────────────────────────────────────────┘
```

## 性能（真机实测）

| 指标 | 值 |
|---|---|
| 万级目录投影（10,100 条目） | 618ms（16,330 条/秒） |
| 并发水合（4 文件并行下载） | CDN 8 并发流（原 1 r/s 串行） |
| 空闲内存（边缘网关） | 10–20MB RSS |
| 静态二进制 | ~4MB（musl） |

## 仓库结构

```
crates/
  cloudmount-core        Provider trait / 类型 / 错误 / 配置(TOML)
  cloudmount-vfs         挂载层：CfAPI 后端 + WebDAV 出口
  cloudmount-auth        PKCE 官方登录 + Cookie 通道 + 凭据存储
  cloudmount-driver115   官方 OpenAPI / Cookie / 路由降级
  cloudmount-sync        同步引擎（快照/三向对比/队列/缓存/.git 防护）
  cloudmount-cli         CLI：login/ls/upload/download/quota/dehydrate
  cloudmount-daemon      cloudmountd：同步/挂载/serve 三模式
apps/desktop                桌面壳（egui/eframe，托盘 + 原生 UI）
drivers/win             自研 minifilter（原生 Rust KM-0/1 + KM-2 起步，长期）
web-ui/                  边缘网关管理页（零框架）
packaging/               systemd / Dockerfile / 发布配置
docs/                    架构、路线图、边缘部署手册
```

## 开发者

```bash
cargo test --workspace               # 214 个测试（Windows）/ 185（Linux）
cargo clippy --workspace --all-targets -- -D warnings
cd web-ui && npm run build           # 前端（typecheck + vite）

# 真机 CfAPI 测试（需要本地 NTFS + cldflt；CI 自动跳过）
cargo test -p cloudmount-vfs

# 桌面壳
cd apps/desktop && cargo build --release  # 产物 target/release/cloudmount-desktop.exe
```

- [架构文档](docs/ARCHITECTURE.md) ｜ [路线图](docs/ROADMAP.md) ｜
  [边缘部署手册](docs/edge.md) ｜ [设计探索记录](docs/design/)
- 数据目录：`CLOUDMOUNT_DATA_DIR` 可覆盖（凭据/配置/缓存/元数据）

## 平台能力矩阵（诚实版）

| 平台 | 机制 | 占位符/按需 | 状态图标 |
|---|---|---|---|
| Windows | CfAPI ✅ → 自研 minifilter（长期） | ✅ | ✅ |
| macOS | File Provider 扩展 | 计划 | 计划 |
| Linux/边缘 | WebDAV 出口 ✅ | ⚠️ 挂载盘 | ❌ |

## 许可

AGPL-3.0-or-later（见 [LICENSE](LICENSE)）。
自研 Windows 驱动的许可在 KM 阶段单独评估。

## Cookie 通道风险声明

115 官方 OpenAPI 是本项目的主通道与推荐方式。仓库内包含的 Cookie 通道代码
（`cloudmount-auth::cookie` / `cloudmount-driver115::cookie`）**默认关闭**，
仅作官方 API 不可用时的兜底。该通道使用 115 网页/客户端协议，**可能违反
《115 生活用户服务协议》**（3.2.3(4) / 4.2(11) / 5.4 条），存在账号冻结、
限速、多端互踢风险。启用前的知情同意流程会逐条展示这些风险。

若你不同意上述风险，请不要启用该通道——官方 OpenAPI 已覆盖同步盘全部核心
能力（申请入口 https://open.115.com/ ，个人开发者可申请）。
