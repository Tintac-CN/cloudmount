//! # cloudmountd
//!
//! CloudMount 常驻服务。同一份二进制承担三种角色（按平台/配置选择）：
//!
//! 1. **桌面服务**（Windows/macOS/Linux）：后台同步引擎 + 供桌面壳（Tauri/
//!    平台挂载后端）调用的本地接口。
//! 2. **边缘网关**（ARM Linux/NAS）：WebDAV 出口 + 后台同步，低内存预设。
//! 3. **无头服务**：仅跑同步（当前已实现）。
//!
//! ## 运行（M1 现状）
//!
//! ```text
//! export CLOUDMOUNT_APP_ID=<开放平台 AppID>
//! export CLOUDMOUNT_SYNC_ROOT=/path/to/local/folder   # 本地同步根
//! cloudmountd --interval 30                           # 每 30 秒一轮
//! cloudmountd --once                                  # 只跑一轮（脚本/验证）
//! ```
//!
//! ## 低内存纪律（目标空闲 RSS 5–15MB）
//! - tokio 用 `new_current_thread()` 单线程运行时（比多线程省约 8MB）。
//! - 同步引擎是**同步阻塞**实现，跑在独立阻塞线程里；tokio 只承载未来的 WebDAV/UI 面。
//! - 阻塞线程数有上限；缓冲区小而有限；缓存落盘不占 RAM。
//! - 发布构建 `opt-level="z" + lto + panic="abort"`。

use anyhow::Context;
use clap::Parser;
use cloudmount_auth::open::OpenApiAuth;
use cloudmount_auth::store::default_store;
use cloudmount_core::RemoteId;
use cloudmount_driver115::Open115Provider;
use cloudmount_sync::{SyncEngine, SyncEngineConfig};
use std::sync::Arc;
use std::time::Duration;

/// 命令行参数。
#[derive(Parser)]
#[command(
    name = "cloudmountd",
    version,
    about = "CloudMount 常驻服务（同步引擎 / 边缘网关）"
)]
struct Cli {
    /// 本地同步根目录（也可用环境变量 CLOUDMOUNT_SYNC_ROOT）。
    #[arg(long, env = "CLOUDMOUNT_SYNC_ROOT")]
    sync_root: Option<std::path::PathBuf>,

    /// 115 开放平台 AppID（也可用环境变量 CLOUDMOUNT_APP_ID）。
    #[arg(long, env = "CLOUDMOUNT_APP_ID")]
    app_id: Option<String>,

    /// 远端根目录 ID（默认 0 = 账号根）。
    #[arg(long, default_value = "0")]
    remote_root: String,

    /// 两轮同步之间的间隔（秒）。
    #[arg(long, default_value_t = 30)]
    interval: u64,

    /// 只跑一轮就退出（便于脚本与验证）。
    #[arg(long)]
    once: bool,

    /// 数据目录（默认按平台惯例，见 `cloudmount_core::paths`）。
    #[arg(long, env = "CLOUDMOUNT_DATA_DIR")]
    data_dir: Option<std::path::PathBuf>,

    /// 是否传播远端删除（**默认关闭**：本地保留并重新上传）。
    #[arg(long)]
    propagate_remote_deletes: bool,

    /// 是否传播本地删除（**默认关闭**：远端保留并重新下载）。
    #[arg(long)]
    propagate_local_deletes: bool,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();

    // 单线程运行时：内存最优。
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(4) // 限制阻塞线程数，控内存
        .build()
        .context("build tokio runtime")?;

    tracing::info!(version = cloudmount_core::VERSION, "cloudmountd starting");

    rt.block_on(async_main(cli))
}

async fn async_main(cli: Cli) -> anyhow::Result<()> {
    let sync_root = cli.sync_root.clone().ok_or_else(|| {
        anyhow::anyhow!("缺少本地同步根目录：请设置 CLOUDMOUNT_SYNC_ROOT 或传 --sync-root")
    })?;
    let app_id = cli.app_id.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "缺少 AppID：请设置 CLOUDMOUNT_APP_ID 或传 --app-id（申请入口 https://open.115.com/）"
        )
    })?;

    // 数据目录：优先 CLI/环境变量，其次平台默认
    if let Some(dir) = &cli.data_dir {
        std::env::set_var("CLOUDMOUNT_DATA_DIR", dir);
    }

    let store = default_store()?;
    let auth = Arc::new(OpenApiAuth::new(app_id, store));
    if !auth.is_logged_in() {
        anyhow::bail!("未登录：请先执行 `cloudmount login` 完成扫码授权");
    }
    let provider = Arc::new(Open115Provider::new(auth));

    let mut cfg = SyncEngineConfig::new(&sync_root);
    cfg.remote_root = RemoteId(cli.remote_root.clone());
    cfg.policy.propagate_remote_deletes = cli.propagate_remote_deletes;
    cfg.policy.propagate_local_deletes = cli.propagate_local_deletes;

    // 同步引擎是阻塞实现：放进专用阻塞线程，避免占用 tokio 工作线程
    let interval = Duration::from_secs(cli.interval.max(1));
    let once = cli.once;
    let engine_task = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let mut engine = SyncEngine::new(cfg, provider)?;
        loop {
            match engine.run_once() {
                Ok(stats) => {
                    if stats.has_changes() || stats.errors > 0 {
                        tracing::info!(
                            uploaded = stats.uploaded,
                            downloaded = stats.downloaded,
                            conflicts = stats.conflicts,
                            errors = stats.errors,
                            deleted_local = stats.deleted_local,
                            deleted_remote = stats.deleted_remote,
                            "同步完成（有变更）"
                        );
                    } else {
                        tracing::debug!(
                            remote_unchanged = stats.remote_unchanged,
                            "同步完成（无变更）"
                        );
                    }
                    if stats.errors > 0 {
                        tracing::warn!(errors = stats.errors, "本轮有失败操作，下一轮将重试");
                    }
                }
                Err(e) => tracing::error!(error = %e, "同步轮次失败（将继续下一轮）"),
            }
            if once {
                return Ok(());
            }
            std::thread::sleep(interval);
        }
    });

    // TODO(M4): 边缘模式下并行启动 dav-server-rs 的 WebDAV 出口与 web-ui 托管。
    // TODO(M2): 桌面模式按平台启动挂载后端（Windows: CfAPI/自研 minifilter；
    //           macOS: FileProvider；Linux: fuser），通过平台 IPC 暴露给 Tauri 壳。

    engine_task
        .await
        .context("sync task panicked")?
        .context("sync engine failed")?;
    Ok(())
}
