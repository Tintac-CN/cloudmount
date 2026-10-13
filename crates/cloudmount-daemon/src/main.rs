//! # cloudmountd
//!
//! CloudMount 常驻服务。运行模式：
//!
//! ## 1. 同步模式（默认）—— 双向同步引擎
//!
//! ```text
//! cloudmountd --interval 30
//! cloudmountd --once
//! ```
//!
//! ## 2. 占位符挂载模式（`--placeholder-mount`）—— 类 OneDrive 按需视图
//!
//! ```text
//! cloudmountd --placeholder-mount --sync-root D:\Cloud --interval 15
//! cloudmountd --placeholder-mount --mock --once --hold-secs 5   # 免凭据自检
//! ```
//!
//! ## 3. Windows 服务管理（`service` 子命令）
//!
//! ```text
//! cloudmountd service install --sync-root D:\Cloud   # 注册（开机自启）
//! cloudmountd service start / stop / uninstall
//! ```
//!
//! ## 低内存纪律（目标空闲 RSS 5–15MB）
//! - tokio 用 `new_current_thread()` 单线程运行时。
//! - 同步/挂载循环是**同步阻塞**实现，跑在独立阻塞线程里。
//! - 发布构建 `opt-level="z" + lto + panic="abort"`。

use anyhow::Context;
use clap::Parser;
#[cfg(windows)]
use cloudmount_auth::cookie::CookieAuth;
use cloudmount_auth::open::OpenApiAuth;
use cloudmount_core::{Provider, RemoteId};
use cloudmount_driver115::Open115Provider;
#[cfg(windows)]
use cloudmount_sync::remote::RemoteViewTracker;
#[cfg(all(feature = "mock", windows))]
use cloudmount_sync::testutil::MockRemote;
use cloudmount_sync::{SyncEngine, SyncEngineConfig};
#[cfg(windows)]
use cloudmount_vfs::LocalChangeUploader;
#[cfg(windows)]
use cloudmount_vfs::PlaceholderSync;
use std::sync::Arc;
use std::time::Duration;

#[cfg(windows)]
mod service;

/// 命令行参数。
#[derive(Parser)]
#[command(
    name = "cloudmountd",
    version,
    about = "CloudMount 常驻服务（同步引擎 / 占位符挂载 / 边缘网关）"
)]
struct Cli {
    /// 运行模式：占位符挂载（类 OneDrive 按需视图；默认为双向同步模式）。
    #[arg(long)]
    placeholder_mount: bool,

    /// 本地同步根目录（也可用环境变量 CLOUDMOUNT_SYNC_ROOT）。
    #[arg(long, env = "CLOUDMOUNT_SYNC_ROOT")]
    sync_root: Option<std::path::PathBuf>,

    /// 115 开放平台 AppID（也可用环境变量 CLOUDMOUNT_APP_ID）。
    #[arg(long, env = "CLOUDMOUNT_APP_ID")]
    app_id: Option<String>,

    /// 远端根目录 ID（默认 0 = 账号根）。
    #[arg(long, default_value = "0")]
    remote_root: String,

    /// 两轮之间的间隔（秒）。
    #[arg(long, default_value_t = 30)]
    interval: u64,

    /// 只跑一轮就退出（便于脚本与验证）。
    #[arg(long)]
    once: bool,

    /// [占位符模式] 挂载保持时长（秒），配合 --once 做自动化验证；0 = 一直保持。
    #[arg(long, default_value_t = 0)]
    hold_secs: u64,

    /// 是否传播远端删除（**默认关闭**：本地保留并重新上传）。
    #[arg(long)]
    propagate_remote_deletes: bool,

    /// 是否传播本地删除（**默认关闭**：远端保留并重新下载）。
    #[arg(long)]
    propagate_local_deletes: bool,

    /// 数据通道：open（官方 OpenAPI，默认）/ cookie（补充通道，须已
    /// `cloudmount login --with-cookie-channel` 登录；只读能力）。
    #[arg(long, default_value = "open")]
    channel: String,

    /// [仅 mock feature] 使用内置 MockRemote 云端（免凭据自检/演示）。
    #[cfg(feature = "mock")]
    #[arg(long)]
    mock: bool,

    /// [仅 Windows] 服务管理子命令（install/start/stop/uninstall/run）。
    #[cfg(windows)]
    #[command(subcommand)]
    service: Option<ServiceCommands>,
}

/// Windows 服务管理子命令。
#[cfg(windows)]
#[derive(Debug, Clone, clap::Subcommand)]
enum ServiceCommands {
    /// 注册 Windows 服务（开机自启；启动参数由本次其余 flag 决定）。
    Install,
    /// 启动服务。
    Start,
    /// 停止服务。
    Stop,
    /// 删除服务。
    Uninstall,
    /// SCM 入口（由服务控制管理器调用，勿手动执行）。
    #[command(hide = true)]
    Run,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();

    // Windows 服务子命令（安装/启停在 SCM 层，无需 tokio）
    #[cfg(windows)]
    if let Some(cmd) = &cli.service {
        let launch_args = build_launch_args(&cli);
        match cmd {
            ServiceCommands::Install => {
                service::install(&launch_args)?;
                println!(
                    "服务已安装（开机自启）。启动：sc start {} 或重启",
                    service::SERVICE_NAME
                );
            }
            ServiceCommands::Start => {
                service::service_start()?;
                println!("服务已启动");
            }
            ServiceCommands::Stop => {
                service::service_stop()?;
                println!("服务已停止");
            }
            ServiceCommands::Uninstall => {
                service::uninstall()?;
                println!("服务已删除");
            }
            ServiceCommands::Run => {
                // SCM 入口：同步引擎循环（阻塞直到 SCM Stop）
                let sync_root = cli.sync_root.clone().unwrap_or_else(|| ".".into());
                let provider = build_provider(&cli)?;
                let mut cfg = SyncEngineConfig::new(&sync_root);
                cfg.remote_root = RemoteId(cli.remote_root.clone());
                cfg.policy.propagate_remote_deletes = cli.propagate_remote_deletes;
                cfg.policy.propagate_local_deletes = cli.propagate_local_deletes;
                let interval = cli.interval.max(1);
                // cfg/provider 会被闭包捕获，先克隆供 SCM 执行体用
                let cfg_for_closure = cfg.clone();
                let provider_for_closure = provider.clone();
                if let Err(e) =
                    service::run_service_scm_executor(cfg, provider, interval, move |stop| {
                        run_service_engine_blocking(
                            cfg_for_closure,
                            provider_for_closure,
                            interval,
                            stop,
                        )
                    })
                {
                    tracing::error!(error = %e, "服务运行失败");
                }
            }
        }
        return Ok(());
    }

    // 单线程运行时：内存最优。
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(4)
        .build()
        .context("build tokio runtime")?;

    tracing::info!(version = cloudmount_core::VERSION, "cloudmountd starting");

    rt.block_on(async_main(cli))
}

/// 从 Cli 提取服务启动参数（install 时烘焙进 binPath）。
#[cfg(windows)]
fn build_launch_args(cli: &Cli) -> Vec<String> {
    let mut args = Vec::new();
    if cli.placeholder_mount {
        args.push("--placeholder-mount".into());
    }
    if let Some(root) = &cli.sync_root {
        args.extend(["--sync-root".into(), root.to_string_lossy().to_string()]);
    }
    if cli.remote_root != "0" {
        args.extend(["--remote-root".into(), cli.remote_root.clone()]);
    }
    if let Some(id) = &cli.app_id {
        args.extend(["--app-id".into(), id.clone()]);
    }
    if cli.propagate_remote_deletes {
        args.push("--propagate-remote-deletes".into());
    }
    if cli.propagate_local_deletes {
        args.push("--propagate-local-deletes".into());
    }
    args
}

/// 以 AppID 构造 Provider（供服务模式/占位符模式的非 mock 通道）。
#[cfg(windows)]
fn build_provider_from_app_id(app_id: &str) -> anyhow::Result<Arc<dyn Provider>> {
    let store = cloudmount_auth::store::default_store()?;
    let auth = Arc::new(OpenApiAuth::new(app_id, store));
    if !auth.is_logged_in() {
        anyhow::bail!("未登录：请先执行 `cloudmount login` 完成扫码授权");
    }
    Ok(Arc::new(Open115Provider::new(auth)))
}

/// 阻塞式构建 Provider（mock 通道或官方 OpenAPI）。
#[cfg(windows)]
fn build_provider(cli: &Cli) -> anyhow::Result<Arc<dyn Provider>> {
    #[cfg(feature = "mock")]
    if cli.mock {
        tracing::warn!("使用 MockRemote 云端（演示/自检；内容为内置示例）");
        let mock = MockRemote::new();
        mock.put_file(
            "welcome.txt",
            b"Welcome to CloudMount! This file is hydrated on demand.",
        );
        mock.put_dir("docs");
        mock.put_file("docs/demo.txt", b"Nested placeholder hydration works.");
        return Ok(Arc::new(mock));
    }

    // Cookie 补充通道（显式 opt-in；知情同意已在 login 时完成）
    if cli.channel == "cookie" {
        tracing::warn!(
            "使用 Cookie 补充通道（违反 115 服务协议；只读能力，上传未实现）。             失效探测只读不踢线，失效时请重新扫码。"
        );
        let policy = cloudmount_auth::cookie::CookiePolicy {
            enabled: true,
            consent_acknowledged: true,
        };
        let store = cloudmount_auth::store::default_store()?;
        let auth = CookieAuth::new(policy, store);
        let provider = cloudmount_driver115::Cookie115Provider::try_new(policy, auth)?;
        return Ok(Arc::new(provider));
    }

    let app_id = cli
        .app_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("缺少 AppID：请设置 CLOUDMOUNT_APP_ID 或传 --app-id"))?;
    build_provider_from_app_id(&app_id)
}

/// 服务模式引擎循环（阻塞直到 SCM 停止；仅 Windows 服务模式）。
#[cfg(windows)]
fn run_service_engine_blocking(
    cfg: SyncEngineConfig,
    provider: Arc<dyn Provider>,
    interval_secs: u64,
    stop: crate::service::StopSignal,
) {
    let Ok(mut engine) = SyncEngine::new(cfg, provider) else {
        return;
    };
    loop {
        if stop.should_stop() || crate::service::should_exit() {
            return;
        }
        match engine.run_once() {
            Ok(stats) => {
                if stats.has_changes() || stats.errors > 0 {
                    tracing::info!(
                        uploaded = stats.uploaded,
                        downloaded = stats.downloaded,
                        conflicts = stats.conflicts,
                        errors = stats.errors,
                        "同步完成（有变更）"
                    );
                }
            }
            Err(e) => tracing::error!(error = %e, "同步轮次失败（继续）"),
        }
        // 分段 sleep，及时响应停止
        for _ in 0..(interval_secs.max(1) * 2) {
            if stop.should_stop() || crate::service::should_exit() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }
}

async fn async_main(cli: Cli) -> anyhow::Result<()> {
    let sync_root = cli.sync_root.clone().ok_or_else(|| {
        anyhow::anyhow!("缺少本地同步根目录：请设置 CLOUDMOUNT_SYNC_ROOT 或传 --sync-root")
    })?;

    // 占位符挂载模式
    if cli.placeholder_mount {
        return run_placeholder_mount(&cli, &sync_root);
    }

    // ---- 同步模式（默认）----
    let app_id = cli.app_id.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "缺少 AppID：请设置 CLOUDMOUNT_APP_ID 或传 --app-id（申请入口 https://open.115.com/）"
        )
    })?;
    let store = cloudmount_auth::store::default_store()?;
    let auth = Arc::new(OpenApiAuth::new(app_id, store));
    if !auth.is_logged_in() {
        anyhow::bail!("未登录：请先执行 `cloudmount login` 完成扫码授权");
    }
    let provider: Arc<dyn Provider> = Arc::new(Open115Provider::new(auth));

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

    engine_task
        .await
        .context("sync task panicked")?
        .context("sync engine failed")?;
    Ok(())
}

/// 占位符挂载模式：远端视图 → 占位符投影 → 按需水合。
#[cfg(windows)]
fn run_placeholder_mount(cli: &Cli, sync_root: &std::path::Path) -> anyhow::Result<()> {
    let provider = build_provider(cli)?;

    if !cloudmount_vfs::cfapi::platform_available() {
        anyhow::bail!("cldflt 不可用：占位符挂载需要 Windows 10 1709+ 且为本地 NTFS 卷");
    }

    // 初始远端视图（首轮全量；后续轮探测短路）
    let mut tracker = RemoteViewTracker::new(20);
    let remote_root = RemoteId(cli.remote_root.clone());
    let initial = tracker
        .next_view(provider.as_ref(), &remote_root)?
        .ok_or_else(|| anyhow::anyhow!("首轮必须产出远端视图"))?;
    tracing::info!(entries = initial.len(), "初始远端视图就绪");

    let backend = cloudmount_vfs::CfapiBackend::new(provider.clone()).with_shell_integration();
    let mount = Arc::new(backend.mount_concrete(sync_root, &initial)?);
    tracing::info!(
        root = %mount.root().display(),
        entries = initial.len(),
        "占位符挂载完成（读文件将按需从云端拉取）"
    );

    // 自诊断：同进程读一次（验证连接是否真正可用）
    #[cfg(feature = "mock")]
    if cli.mock {
        match std::fs::read_to_string(mount.root().join("welcome.txt")) {
            Ok(content) => tracing::info!(len = content.len(), "自诊断：同进程水合成功"),
            Err(e) => tracing::error!(error = %e, "自诊断：同进程水合失败"),
        }
    }

    // 双向：本地变更（用户修改/新建）→ 上传
    let mut uploader =
        LocalChangeUploader::new(mount.clone(), provider.clone(), remote_root.clone())?;

    // 控制通道：daemon 持有 CfAPI 连接，是唯一能执行 provider 专属操作
    // （CfDehydratePlaceholder 等）的进程；CLI 经本地回环 TCP 转发请求。
    let mount_for_ctl = mount.clone();
    let root_canon = sync_root
        .canonicalize()
        .unwrap_or_else(|_| sync_root.to_path_buf());
    let control = cloudmount_vfs::control::ControlServer::start(
        &cloudmount_vfs::control::default_port_file()?,
        sync_root,
        move |cmd, arg| match cmd {
            "PING" => format!("OK {}", cloudmount_core::VERSION),
            "DEHYDRATE" => dehydrate_via_mount(&mount_for_ctl, &root_canon, arg),
            _ => "ERR unknown-command".into(),
        },
    )?;
    tracing::info!(port = control.port(), "控制通道已监听（PING / DEHYDRATE）");

    let mut sync = PlaceholderSync::new(mount.clone(), &initial);
    let interval = Duration::from_secs(cli.interval.max(1));
    // --once + --hold-secs：保持 hold_secs 再退出（供自动化验证/演示）
    // --once（无 hold）：跑一轮即退出；常驻：一直保持
    let deadline = (cli.once && cli.hold_secs > 0)
        .then(|| std::time::Instant::now() + Duration::from_secs(cli.hold_secs));
    let exit_after_cycle = cli.once && deadline.is_none();

    // 挂载循环是阻塞实现：直接在调用线程上跑
    loop {
        // 先处理本地→远端（用户修改上传），再处理远端→视图（投影/删除）
        match uploader.poll_and_upload() {
            Ok(stats) if stats.uploaded > 0 || stats.errors > 0 => tracing::info!(
                uploaded = stats.uploaded,
                skipped_hydration = stats.skipped_hydration,
                errors = stats.errors,
                "本地变更已处理"
            ),
            Ok(_) => {}
            Err(e) => tracing::error!(error = %e, "本地变更轮询失败（下一轮重试）"),
        }
        match tracker.next_view(provider.as_ref(), &remote_root) {
            Ok(Some(view)) => match sync.apply_remote_view(&view) {
                Ok(stats) => tracing::info!(
                    added = stats.added,
                    removed = stats.removed,
                    content_updated = stats.content_updated,
                    content_pending = stats.content_changes_pending,
                    "远端视图已应用"
                ),
                Err(e) => tracing::error!(error = %e, "应用远端视图失败（下一轮重试）"),
            },
            Ok(None) => tracing::debug!("远端未变化"),
            Err(e) => tracing::error!(error = %e, "远端探测失败（下一轮重试）"),
        }

        if let Some(d) = deadline {
            if std::time::Instant::now() >= d {
                tracing::info!("保持时长结束，卸载退出");
                break;
            }
        }
        if exit_after_cycle {
            break;
        }
        std::thread::sleep(interval);
    }

    // 先停控制通道（其 handler 持有挂载句柄的克隆，Arc::try_unwrap 需要它释放），
    // 再干净退出：断开连接 + 双注销（内核 + Shell）
    control.stop();
    if let Ok(m) = Arc::try_unwrap(mount) {
        cloudmount_vfs::Mount::unmount(Box::new(m)).ok();
    }
    Ok(())
}

/// 控制通道 DEHYDRATE 处理：绝对路径 → 校验在挂载根内 → 脱水。
#[cfg(windows)]
fn dehydrate_via_mount(
    mount: &Arc<cloudmount_vfs::CfapiMount>,
    root_canon: &std::path::Path,
    arg: &str,
) -> String {
    let canon = std::path::PathBuf::from(arg)
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from(arg));
    let Ok(rel) = canon.strip_prefix(root_canon) else {
        return format!("ERR 路径不在挂载根内: {arg}");
    };
    match mount.dehydrate_rel(&rel.to_string_lossy()) {
        Ok(()) => "OK".into(),
        Err(e) => format!("ERR {e}"),
    }
}

/// 非 Windows：占位符挂载不可用（cldflt 为 Windows 组件）。
#[cfg(not(windows))]
fn run_placeholder_mount(_cli: &Cli, _sync_root: &std::path::Path) -> anyhow::Result<()> {
    anyhow::bail!("占位符挂载当前仅支持 Windows（macOS FileProvider / Linux 方案见 ROADMAP）")
}
