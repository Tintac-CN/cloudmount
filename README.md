# CloudMount 云挂

> 把 115 网盘变成你所有设备上的"同步盘"—— OneDrive 式体验、Rust 极致轻量、全开源。
>
> **状态：M0 实现完成**（官方 OpenAPI 全链路代码就绪：扫码登录 / 列目录 / 秒传上传 /
> Range 下载 / 配额）。待 115 开放平台 AppID 审核通过后即可端到端实测；
> 里程碑见 [docs/ROADMAP.md](docs/ROADMAP.md)。

## 是什么

- **Windows**：把 115 挂成"类 OneDrive"的同步盘 —— 占位符、按需下载、
  资源管理器状态图标、"始终保留/释放空间"。
- **macOS**：File Provider 扩展（Apple 官方云盘机制，Finder 原生体验）。*计划*
- **Linux / 边缘设备**：FUSE 挂载 + WebDAV 出口，NAS/树莓派（512MB 内存）可跑。
- **一套前端**：桌面壳（Tauri）与边缘网关（浏览器）共用同一个 Web UI。
- **双通道登录**：115 官方 OpenAPI 为主（合规稳定）；Cookie 通道为**默认关闭**的
  兜底补充（见[风险声明](#cookie-通道风险声明)）。

## 架构一图

```
┌────────────────────────────────────────────────────────────┐
│  UI：web-ui（共享 SPA）→ Tauri 桌面壳 / 边缘浏览器管理页      │
├────────────────────────────────────────────────────────────┤
│  挂载后端（cloudmount-vfs::MountBackend，可插拔）             │
│   Windows: CfAPI(M2) → 自研 minifilter(长期)                 │
│   macOS: FileProvider *计划*   Linux/边缘: FUSE + WebDAV     │
├────────────────────────────────────────────────────────────┤
│  同步引擎（Rust）：USN/FSEvents/inotify 本地监控             │
│   + usertime 轮询远端 + 队列/限速/冲突处理                    │
├────────────────────────────────────────────────────────────┤
│  驱动（cloudmount-driver115）：官方 OpenAPI 主通道           │
│   + Cookie 兜底通道（默认关）+ ProviderRouter 自动降级        │
├────────────────────────────────────────────────────────────┤
│  基座：SQLite 元数据（bundled）· ureq+rustls 传输            │
│   · tokio current_thread · musl 静态 ARM 交叉编译            │
└────────────────────────────────────────────────────────────┘
```

详细设计：[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) ｜
完整探索过程：[docs/design/](docs/design/)

## 仓库结构

```
crates/
  cloudmount-core        核心抽象：Provider trait / 类型 / 错误 / 同步状态机
  cloudmount-vfs         挂载层抽象：占位符语义 + MountBackend 可插拔接口
  cloudmount-auth        认证：官方 PKCE + Cookie 通道（带合规边界）+ 凭据存储
  cloudmount-driver115   115 驱动：官方 OpenAPI / Cookie / 路由降级
  cloudmount-cli         CLI（M0 验证工具）：login/ls/upload/download/quota
  cloudmount-daemon      cloudmountd 常驻服务：同步引擎 + WebDAV 边缘出口
apps/
  desktop                Tauri 2 桌面壳（三端共用，托盘 + 平台挂载集成）
  shell-macos            macOS File Provider 扩展（占位，计划）
drivers/win             自研 Windows minifilter（长期目标，设计文档就绪）
web-ui/                  共享前端 SPA（Tauri 与边缘 Web 复用，零框架）
edge/                   边缘部署（Docker / systemd / 低内存预设）
docs/                   架构、路线图、设计探索记录
```

## 平台能力矩阵（诚实版）

| 平台 | 机制 | 占位符/按需 | 状态图标 |
|---|---|---|---|
| Windows | CfAPI（M2）→ 自研 minifilter（长期） | ✅ | ✅（CfAPI 原生；自研需自建 Shell 扩展） |
| macOS | File Provider 扩展 | ✅ | ✅ |
| Linux/边缘 | FUSE + 本地缓存 | ⚠️ 挂载盘 | ❌（平台限制，无占位符机制） |
| 全平台 | WebDAV 出口 | ⚠️ | ❌ |

## 快速开始（开发者）

```bash
# 工具链：Rust stable + Node.js 18+
cargo check --workspace      # 核心 Rust 全部编译通过
cargo test --workspace       # 34 个单元测试（含 RFC 7636 PKCE 向量、真实报文样本）

# 115 官方 API 全链路（需要开放平台 AppID，https://open.115.com/ ）
export CLOUDMOUNT_APP_ID=<你的 AppID>
cargo run -p cloudmount-cli -- login     # 终端二维码，用 115 App 扫码
cargo run -p cloudmount-cli -- status    # 登录状态
cargo run -p cloudmount-cli -- ls 0      # 列出根目录
cargo run -p cloudmount-cli -- quota     # 空间配额
cargo run -p cloudmount-cli -- upload ./report.pdf --to 0
cargo run -p cloudmount-cli -- download <file_id> ./report.pdf

# 前端
cd web-ui && npm install && npm run dev

# 桌面壳（需 tauri-cli）
cd apps/desktop && cargo tauri dev
```

凭据存放：Windows 凭据管理器 / macOS Keychain / Linux 下 `<data_dir>/secrets`（0600）。
数据目录可用 `CLOUDMOUNT_DATA_DIR` 覆盖。

## 路线图

[docs/ROADMAP.md](docs/ROADMAP.md) —— M0 打通 115 官方 API → M1 同步引擎 →
M2 Windows 挂载 → M3 Cookie 通道 → M4 边缘网关 → M5 打磨 → （长期）自研内核。

## 许可

AGPL-3.0-or-later（见 [LICENSE](LICENSE)）。
自研 Windows 驱动的许可在 KM 阶段单独评估（倾向允许被更广泛项目复用）。

## Cookie 通道风险声明

115 官方 OpenAPI 是本项目的主通道与推荐方式。仓库内包含的 Cookie 通道代码
（`cloudmount-auth::cookie` / `cloudmount-driver115::cookie`）**默认关闭**，
仅作官方 API 不可用时的兜底。该通道使用 115 网页/客户端协议，**可能违反
《115 生活用户服务协议》**（3.2.3(4) 非授权接口、4.2(11) 非授权工具、5.4
反向工程），存在账号被冻结/限速/数据访问受限的风险，并可能导致多端登录互踢。
启用需用户显式开启并确认知情同意；本项目不做任何主动风控规避。
