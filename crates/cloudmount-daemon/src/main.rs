//! # cloudmountd
//!
//! CloudMount 常驻服务。同一份二进制承担三种角色（按平台/配置选择）：
//!
//! 1. **桌面服务**（Windows/macOS/Linux）：后台同步引擎 + 供桌面壳（Tauri/
//!    平台挂载后端）调用的本地接口。
//! 2. **边缘网关**（ARM Linux/NAS）：WebDAV 出口 + 后台同步，低内存预设。
//! 3. **无头 CLI 服务**：仅跑同步。
//!
//! ## 低内存纪律（目标空闲 RSS 5–15MB）
//! - tokio 用 `new_current_thread()` 单线程运行时（比多线程省约 8MB）。
//! - 阻塞型 Provider 调用走 `spawn_blocking`，线程池有上限。
//! - 缓冲区小而有限（4MB chunk），缓存落盘不占 RAM。
//! - 发布构建 `opt-level="z" + lto + panic="abort"`。

use anyhow::Context;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // 单线程运行时：内存最优。WebDAV 服务与同步循环都跑在这上面。
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(4) // 限制 spawn_blocking 线程数，控内存
        .build()
        .context("build tokio runtime")?;

    tracing::info!(
        version = cloudmount_core::VERSION,
        "cloudmountd starting (edge/daemon mode)"
    );

    rt.block_on(async_main())
}

async fn async_main() -> anyhow::Result<()> {
    // TODO(M1): 初始化 SQLite 元数据库 → SyncEngine 循环（轮询 usertime + 本地 watcher）。
    // TODO(M2): 桌面模式：按平台启动挂载后端（Windows: CfAPI/自研驱动；macOS: FileProvider；
    //           Linux: fuser），通过平台 IPC 暴露给 Tauri 壳。
    // TODO(M4): 边缘模式：dav-server-rs 挂 WebDAV 出口（rustls TLS），绑定 listen 地址。
    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    Ok(())
}
