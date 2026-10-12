# CloudMount 桌面壳（Tauri 2）

三端（Windows / macOS / Linux）共用的桌面应用。系统 WebView 渲染共享 [web-ui](../../web-ui)，
Rust 侧提供托盘与平台挂载集成。

## 为什么是 Tauri

- **系统托盘**是同步盘刚需：`TrayIconBuilder` 成熟，支持动态图标/角标/菜单。
- 与 Rust 核心同进程：`cloudmount-core` 直接作为依赖链接，无 IPC 开销。
- 体积/内存远优于 Electron（安装包 ~10MB vs 100MB+，无打包 Chromium）。
- 前端零框架（vanilla TS），与边缘 Web 完全复用。

## 已知平台坑（来自调研，开发时对照）

| 平台 | 坑 | 对策 |
|---|---|---|
| Linux | WebKitGTK：托盘**点击事件不触发**（菜单仍可用）；托盘菜单设置后无法移除 | 菜单结构固定，仅更新项文本/勾选态；不依赖点击事件 |
| Windows | 用户缺 WebView2 时无法运行 | 安装器选 `downloadBootstrapper` 模式（Win10 1803+/Win11 自带） |
| macOS | 托盘必须在主线程创建且要有事件循环 | Tauri 已处理，勿在后台线程动托盘 |

## 构建前置

- Rust stable + 平台 WebView：Windows(WebView2) / macOS(WKWebView) / Linux(`libwebkit2gtk-4.1-dev`)
- Node.js ≥ 18（构建 web-ui）
- `cargo install tauri-cli --version '^2'`

```bash
cd apps/desktop
cargo tauri dev      # 开发（自动先跑 web-ui dev server）
cargo tauri build    # 产出 msi / dmg / appimage / deb
```

## 与自研 Windows minifilter 的关系

M2 阶段本壳先接入 **CfAPI**（微软原生占位符/状态图标，NTFS）。
`drivers/win` 的自研 minifilter 成熟后，通过 `cloudmount-vfs::MountBackend`
trait 平滑替换 —— 桌面壳与 web-ui 无需改动。
