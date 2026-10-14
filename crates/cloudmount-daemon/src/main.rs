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
use clap::{CommandFactory, FromArgMatches, Parser};
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
#[derive(Clone, Parser)]
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

    /// 数据通道：auto（官方为主，Cookie 兜底，默认）/ open（仅官方）/
    /// cookie（仅补充通道，须已 `cloudmount login --with-cookie-channel`）。
    #[arg(long, default_value = "auto")]
    channel: String,

    /// [仅 mock feature] 使用内置 MockRemote 云端（免凭据自检/演示）。
    #[cfg(feature = "mock")]
    #[arg(long)]
    mock: bool,

    /// [M4] 边缘网关模式：WebDAV 出口（/dav/）+ web-ui 静态托管。
    #[arg(long)]
    serve: bool,

    /// [serve] 监听地址。
    #[arg(long, default_value = "127.0.0.1:8384")]
    listen: String,

    /// [serve] HTTP Basic 认证（"user:pass"；缺省无认证，仅建议内网）。
    #[arg(long)]
    auth: Option<String>,

    /// [serve] web-ui 静态目录（缺省不托管 UI）。
    #[arg(long)]
    web_ui_dir: Option<std::path::PathBuf>,

    /// [serve] TLS 证书 PEM（设置后走 HTTPS；需与 --tls-key 成对）。
    #[arg(long)]
    tls_cert: Option<std::path::PathBuf>,

    /// [serve] TLS 私钥 PEM。
    #[arg(long)]
    tls_key: Option<std::path::PathBuf>,

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

    // 配置文件 → clap 默认值（优先级：CLI flag > 环境变量 > 配置文件 > 内置默认；
    // clap 的 env 属性天然高于 default，文件值替换 default 即插入正确层级）
    // F-13 fail-fast：坏配置拒绝启动（错误链含「解析失败」+文件路径），
    // 绝不静默回退默认值（错误配置下跑同步/删除传播风险不对称）；
    // 文件缺失走默认值（load_from 既有语义）。
    let file_cfg = load_config_or_fail(&cloudmount_core::config::config_path()?)?;
    let cli =
        Cli::from_arg_matches(&apply_config_defaults(&file_cfg).get_matches()).expect("clap 解析");

    // Windows 服务子命令（安装/启停在 SCM 层，无需 tokio）
    #[cfg(windows)]
    if let Some(cmd) = &cli.service {
        let launch_args = assemble_service_args(&cli);
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
                // F-12：按 --placeholder-mount 分派（决策逻辑在
                // dispatch_service_run——T-21 可注入两支路工厂单测）；
                // provider 构造失败 → 进程非零退出（与前台路径一致）。
                let provider = build_provider(&cli, &file_cfg)?;
                let result = dispatch_service_run(
                    &cli,
                    &file_cfg,
                    // 占位符支路：SCM 版占位符挂载循环（与前台同一主循环，
                    // stop_check = SCM 停止信号）
                    |plan| {
                        let cli = cli.clone();
                        let plan = plan.clone();
                        let provider = provider.clone();
                        service::run_service_scm_executor(move |stop| {
                            if let Err(e) =
                                run_service_placeholder_blocking(provider, cli, plan, stop)
                            {
                                tracing::error!(error = %e, "占位符挂载服务循环失败");
                            }
                        })
                        .map_err(|e| anyhow::anyhow!("服务运行失败: {e}"))
                    },
                    // 引擎支路：既有 run_service_engine_blocking
                    |plan| {
                        let cfg = engine_config_from_plan(plan);
                        let interval = plan.interval_secs;
                        let provider = provider.clone();
                        service::run_service_scm_executor(move |stop| {
                            run_service_engine_blocking(cfg, provider, interval, stop)
                        })
                        .map_err(|e| anyhow::anyhow!("服务运行失败: {e}"))
                    },
                );
                if let Err(e) = result {
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

    rt.block_on(async_main(cli, file_cfg))
}

/// 配置加载（F-13 fail-fast）：存在但损坏 → Err，错误链含 load_from 的
/// 「配置文件解析失败 {path}」+ 本层恢复提示；仅文件不存在回内置默认值
/// （首次运行是正常路径）。
///
/// 位于 main 入口、先于一切子命令分派 → 作用于全部子命令
/// （含 `service stop/uninstall` 等管理命令；sc.exe 直接操作服务是用户侧兜底）。
fn load_config_or_fail(
    path: &std::path::Path,
) -> anyhow::Result<cloudmount_core::config::Config> {
    cloudmount_core::config::load_from(path)
        .context("配置文件加载失败（修正该文件，或删除后重启以恢复默认配置）")
}

/// 配置文件值注入 clap 默认值（优先级：CLI > env > 文件 > 内置）。
///
/// clap 的 env 属性高于 default_value——把文件值替换 default 即自然插入
/// "env 之下、内置之上"的层级。布尔安全开关（propagate_*）不走此路径
/// （flag 无 default 概念），在引擎配置处用 `flag || 文件值` 合并。
fn apply_config_defaults(cfg: &cloudmount_core::config::Config) -> clap::Command {
    // mut_arg 闭包要求 'static——全部克隆为 owned 后 move 捕获
    let mut cmd = Cli::command();
    if let Some(v) = cfg.app_id.clone() {
        cmd = cmd.mut_arg("app_id", move |a| a.default_value(v));
    }
    if let Some(v) = cfg.sync_root.clone() {
        cmd = cmd.mut_arg("sync_root", move |a| {
            a.default_value(std::ffi::OsString::from(v))
        });
    }
    let v = cfg.remote_root.clone();
    cmd = cmd.mut_arg("remote_root", move |a| {
        a.default_value(std::ffi::OsString::from(v))
    });
    let v = cfg.interval.to_string();
    cmd = cmd.mut_arg("interval", move |a| {
        a.default_value(std::ffi::OsString::from(v))
    });
    let v = cfg.channel.clone();
    cmd = cmd.mut_arg("channel", move |a| {
        a.default_value(std::ffi::OsString::from(v))
    });
    let v = cfg.listen.clone();
    cmd = cmd.mut_arg("listen", move |a| {
        a.default_value(std::ffi::OsString::from(v))
    });
    if let Some(v) = cfg.serve.auth.clone() {
        cmd = cmd.mut_arg("auth", move |a| {
            a.default_value(std::ffi::OsString::from(v))
        });
    }
    if let Some(v) = cfg.serve.web_ui_dir.clone() {
        cmd = cmd.mut_arg("web_ui_dir", move |a| {
            a.default_value(std::ffi::OsString::from(v))
        });
    }
    if let Some(v) = cfg.serve.tls_cert.clone() {
        cmd = cmd.mut_arg("tls_cert", move |a| {
            a.default_value(std::ffi::OsString::from(v))
        });
    }
    if let Some(v) = cfg.serve.tls_key.clone() {
        cmd = cmd.mut_arg("tls_key", move |a| {
            a.default_value(std::ffi::OsString::from(v))
        });
    }
    cmd
}

/// 服务 binPath 启动参数组装（F-12/T-29：**跨平台纯函数**——纯 `Vec<String>`、
/// 无 windows API；windows 专属执行面在 service.rs / main 服务分支）。
///
/// T-29/T-21 单测无门控（M-R2-3：提取消除原 `#[cfg(windows)]` 使 Linux CI
/// 直接编译失败的面）。`--interval` **无条件烘焙**（MA9 修复：值 = clap
/// 解析结果，已含配置文件默认注入——修复"服务模式恒默认 30s"潜伏 bug）；
/// `--placeholder-mount` 显式时烘焙（服务模式由 Run 分支按它分派）；
/// `--once/--hold-secs` 不参与服务路径（服务即常驻），无需烘焙。
#[cfg_attr(not(windows), allow(dead_code))] // 仅服务分支调用；跨平台提取供 T-29 无门控单测
fn assemble_service_args(cli: &Cli) -> Vec<String> {
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
    // 无条件烘焙（MA9）：SCM 侧解析由 clap 统一承担（死入口的手写解析
    // 已删除），缺省时烘焙的是解析后的实际值（含文件默认注入）
    args.extend(["--interval".into(), cli.interval.to_string()]);
    if cli.propagate_remote_deletes {
        args.push("--propagate-remote-deletes".into());
    }
    if cli.propagate_local_deletes {
        args.push("--propagate-local-deletes".into());
    }
    args
}

/// 服务/前台共用运行计划（F-12：两路径产出配置一致由同源构造保证）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct ServiceRunPlan {
    /// 本地同步根（前台为 required 校验后的值；服务模式缺省 "."）
    sync_root: std::path::PathBuf,
    /// 远端根目录 ID（clap 解析结果，含配置文件默认注入）
    remote_root: String,
    /// 秒（clap 解析结果，含配置文件默认注入；前台循环与 binPath 烘焙共用）
    interval_secs: u64,
    /// flag（本次运行显式开启）或文件持久值，任一为真即生效
    propagate_remote_deletes: bool,
    /// flag（本次运行显式开启）或文件持久值，任一为真即生效
    propagate_local_deletes: bool,
    /// F-12 分派依据：true = 占位符挂载支路
    placeholder_mount: bool,
}

/// F-12 决策核心（跨平台纯函数）：Cli + 文件配置 + 同步根 → 运行计划。
///
/// 前台引擎路径与 SCM `service run` 分支共用此构造（T-21 一致性锚：
/// 两路径产出配置一致不是靠测试碰巧对齐，而是同源构造）。
fn service_run_plan(
    cli: &Cli,
    file_cfg: &cloudmount_core::config::Config,
    sync_root: std::path::PathBuf,
) -> ServiceRunPlan {
    ServiceRunPlan {
        sync_root,
        remote_root: cli.remote_root.clone(),
        interval_secs: cli.interval.max(1),
        propagate_remote_deletes: cli.propagate_remote_deletes
            || file_cfg.sync.propagate_remote_deletes,
        propagate_local_deletes: cli.propagate_local_deletes
            || file_cfg.sync.propagate_local_deletes,
        placeholder_mount: cli.placeholder_mount,
    }
}

/// 从运行计划构造引擎配置（前台引擎路径与 SCM 引擎支路同源）。
fn engine_config_from_plan(plan: &ServiceRunPlan) -> SyncEngineConfig {
    let mut cfg = SyncEngineConfig::new(&plan.sync_root);
    cfg.remote_root = RemoteId(plan.remote_root.clone());
    cfg.policy.propagate_remote_deletes = plan.propagate_remote_deletes;
    cfg.policy.propagate_local_deletes = plan.propagate_local_deletes;
    cfg
}

/// F-12：SCM `service run` 分派决策——按 `--placeholder-mount` 选择支路。
///
/// 跨平台纯决策（无 windows API，T-21 无门控单测）：true → 占位符支路，
/// false → 引擎支路。两支路均经工厂注入（生产侧 = SCM 执行体接线，
/// 测试侧 = 捕获调用的桩），共享 [`service_run_plan`] 构造的运行计划——
/// 两路径产出配置一致由同源构造保证，非测试巧合对齐。
#[cfg_attr(not(windows), allow(dead_code))] // 仅服务分支调用；跨平台提取供 T-21 无门控单测
fn dispatch_service_run(
    cli: &Cli,
    file_cfg: &cloudmount_core::config::Config,
    run_placeholder: impl FnOnce(&ServiceRunPlan) -> anyhow::Result<()>,
    run_engine: impl FnOnce(&ServiceRunPlan) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    // 服务模式缺省同步根 "."（前台路径为 required 校验，不走此处）
    let sync_root = cli.sync_root.clone().unwrap_or_else(|| ".".into());
    let plan = service_run_plan(cli, file_cfg, sync_root);
    if plan.placeholder_mount {
        run_placeholder(&plan)
    } else {
        run_engine(&plan)
    }
}

/// 以 AppID 构造 Provider（供服务模式/占位符模式的非 mock 通道）。
fn build_provider_from_app_id(app_id: &str) -> anyhow::Result<Arc<dyn Provider>> {
    Ok(Arc::new(build_open_provider(app_id)?))
}

/// 从配置 `[cookie]` 段构造 Cookie 通道策略（F-05）。
///
/// daemon 是消费者、永不自产同意：策略完全由配置
/// `enabled`/`consent_acknowledged` 决定（缺段 = 双 false = 关闭）；
/// 同意仅由登录交互（CLI / desktop）持久化进 config.toml。
fn cookie_policy_from_config(
    cookie: &cloudmount_core::config::CookieChannelConfig,
) -> cloudmount_auth::cookie::CookiePolicy {
    cloudmount_auth::cookie::CookiePolicy {
        enabled: cookie.enabled,
        consent_acknowledged: cookie.consent_acknowledged,
    }
}

/// 构造官方通道 Provider（具体类型，供路由器装箱）。
fn build_open_provider(app_id: &str) -> anyhow::Result<Open115Provider> {
    let store = cloudmount_auth::store::default_store()?;
    let auth = Arc::new(OpenApiAuth::new(app_id, store));
    if !auth.is_logged_in() {
        anyhow::bail!("未登录：请先执行 `cloudmount login` 完成扫码授权");
    }
    Ok(Open115Provider::new(auth))
}

/// 阻塞式构建 Provider（mock 通道或官方 OpenAPI）。
#[cfg(windows)]
fn build_provider(
    cli: &Cli,
    file_cfg: &cloudmount_core::config::Config,
) -> anyhow::Result<Arc<dyn Provider>> {
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

    build_channel_provider(cli, file_cfg)
}

/// 按通道选择构建 Provider（open/cookie/auto；跨平台，同步与挂载共用）。
///
/// - `cookie`：仅补充通道（显式 opt-in；须已 `cloudmount login
///   --with-cookie-channel` 完成开启与知情同意，配置 `[cookie]` 双真）；
/// - `auto`（默认）：官方为主，Cookie 兜底——官方**认证失效**时无感
///   切换（tracing::warn 可见 + `last_switch` 供 UI 常驻提示）；
/// - `open`：仅官方。
fn build_channel_provider(
    cli: &Cli,
    file_cfg: &cloudmount_core::config::Config,
) -> anyhow::Result<Arc<dyn Provider>> {
    // mock 通道（自检/演示；与占位符挂载模式同一套示例数据）
    #[cfg(feature = "mock")]
    if cli.mock {
        tracing::warn!("使用 MockRemote 云端（演示/自检；内容为内置示例）");
        let mock = cloudmount_sync::testutil::MockRemote::new();
        mock.put_file(
            "welcome.txt",
            b"Welcome to CloudMount! This file is hydrated on demand.",
        );
        mock.put_dir("docs");
        mock.put_file("docs/demo.txt", b"Nested placeholder hydration works.");
        return Ok(Arc::new(mock));
    }

    if cli.channel == "cookie" {
        // F-05：策略从配置 `[cookie]` 构造（硬编码双真已废除）——未双真
        // 时 fail-fast 并提示登录入口，绝不静默放行。
        let policy = cookie_policy_from_config(&file_cfg.cookie);
        if let Err(e) = policy.check_enabled() {
            anyhow::bail!(
                "Cookie 通道未启用：{e}；请先执行 `cloudmount login --with-cookie-channel` 完成显式开启与知情同意"
            );
        }
        tracing::warn!(
            "使用 Cookie 补充通道（违反 115 服务协议）。失效探测只读不踢线，失效时请重新扫码。"
        );
        let store = cloudmount_auth::store::default_store()?;
        let auth = CookieAuth::new(policy, store);
        let provider = cloudmount_driver115::Cookie115Provider::try_new(policy, auth)?;
        return Ok(Arc::new(provider));
    }

    let app_id = cli
        .app_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("缺少 AppID：请设置 CLOUDMOUNT_APP_ID 或传 --app-id"))?;

    // auto：官方为主，Cookie 兜底
    if cli.channel == "auto" {
        // F-05：配置未双真 → 静默跳过 Cookie 兜底（纯官方），info 日志可观测
        if cookie_policy_from_config(&file_cfg.cookie)
            .check_enabled()
            .is_err()
        {
            tracing::info!("Cookie 通道未启用（配置 [cookie]），auto 模式纯官方");
        }
        if let Some(cookie) = try_cookie_provider(&file_cfg.cookie) {
            let open = build_open_provider(&app_id)?;
            tracing::info!("双通道路由就绪：官方为主，Cookie 兜底（认证失效时自动切换）");
            return Ok(Arc::new(cloudmount_driver115::ProviderRouter::new(
                Box::new(open),
                Some(Box::new(cookie)),
            )));
        }
    }

    build_provider_from_app_id(&app_id)
}

/// 尝试构造 Cookie 补充通道 Provider（未启用/未登录时 None）。
///
/// 配置 `[cookie]` 未双真时静默返回 None（auto 兜底纯官方，不报错）；
/// 已登录 Cookie 失效由路由器语义处理（官方**认证失效**自动切 Cookie，
/// `tracing::warn` 可见 + `last_switch` 供 UI 常驻提示；Cookie 失效回退
/// 官方；频控/网络错误不切换）。
fn try_cookie_provider(
    cookie_cfg: &cloudmount_core::config::CookieChannelConfig,
) -> Option<cloudmount_driver115::Cookie115Provider> {
    let store = cloudmount_auth::store::default_store().ok()?;
    try_cookie_provider_with(cookie_cfg, store)
}

/// [`try_cookie_provider`] 的存储注入变体（T-08 `#[cfg(test)]` 直测三态用）。
fn try_cookie_provider_with(
    cookie_cfg: &cloudmount_core::config::CookieChannelConfig,
    store: std::sync::Arc<dyn cloudmount_auth::store::SecretStore>,
) -> Option<cloudmount_driver115::Cookie115Provider> {
    // 配置未双真 → 静默跳过（即使凭据容器已有 Cookie 也不放行）
    let policy = cookie_policy_from_config(cookie_cfg);
    policy.check_enabled().ok()?;
    let auth = CookieAuth::new(policy, store);
    cloudmount_driver115::Cookie115Provider::try_new(policy, auth).ok()
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

async fn async_main(cli: Cli, file_cfg: cloudmount_core::config::Config) -> anyhow::Result<()> {
    // 边缘网关模式（M4）：WebDAV 出口 + web-ui 托管（不需要本地同步根）
    if cli.serve {
        let provider = build_channel_provider(&cli, &file_cfg)?;
        let auth = cli.auth.as_deref().and_then(|s| {
            s.split_once(':')
                .map(|(u, p)| (u.to_string(), p.to_string()))
        });
        let tls = match (&cli.tls_cert, &cli.tls_key) {
            (Some(c), Some(k)) => Some(cloudmount_daemon::webdav::TlsConfig {
                cert_path: c.clone(),
                key_path: k.clone(),
            }),
            (None, None) => None,
            _ => anyhow::bail!("--tls-cert 与 --tls-key 必须成对提供"),
        };
        return cloudmount_daemon::webdav::serve(
            provider,
            &cli.listen,
            auth,
            cli.web_ui_dir.clone(),
            tls,
        )
        .await;
    }

    let sync_root = cli.sync_root.clone().ok_or_else(|| {
        anyhow::anyhow!("缺少本地同步根目录：请设置 CLOUDMOUNT_SYNC_ROOT 或传 --sync-root")
    })?;

    // 占位符挂载模式
    if cli.placeholder_mount {
        return run_placeholder_mount(&cli, &sync_root, &file_cfg);
    }

    // ---- 同步模式（默认）----
    let provider = build_channel_provider(&cli, &file_cfg)?;

    // F-12：前台与服务模式共用同一运行计划构造（两路径产出配置一致由同源保证）
    let plan = service_run_plan(&cli, &file_cfg, sync_root);
    let cfg = engine_config_from_plan(&plan);

    // 同步引擎是阻塞实现：放进专用阻塞线程，避免占用 tokio 工作线程
    let interval = Duration::from_secs(plan.interval_secs);
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

/// 占位符挂载模式（前台入口）：远端视图 → 占位符投影 → 按需水合。
#[cfg(windows)]
fn run_placeholder_mount(
    cli: &Cli,
    sync_root: &std::path::Path,
    file_cfg: &cloudmount_core::config::Config,
) -> anyhow::Result<()> {
    let provider = build_provider(cli, file_cfg)?;
    // 前台无外部停止源：stop_check 恒 false——轮次间/分段 sleep 处的检查
    // 退化为直通（与 SCM 版 [`run_service_placeholder_blocking`] 共用主循环）
    run_placeholder_mount_inner(provider, cli, sync_root, &|| false)
}

/// SCM 版占位符挂载循环（F-12：服务模式与前台行为一致）。
///
/// 与前台共用 [`run_placeholder_mount_inner`] 主循环；stop_check = SCM 停止
/// 信号（控制通道 Stop/Shutdown 的 [`service::StopSignal`]）或 SCM 执行体
/// 置位的退出标志，任一触发即在轮次间/分段 sleep 处结束循环并干净卸载
/// （control.stop + Arc::try_unwrap + unmount）。`--once/--hold-secs` 不参与
/// 服务路径（SCM binPath 不烘焙），循环常驻直到 SCM 停止。
#[cfg(windows)]
fn run_service_placeholder_blocking(
    provider: Arc<dyn Provider>,
    cli: Cli,
    plan: ServiceRunPlan,
    stop: service::StopSignal,
) -> anyhow::Result<()> {
    let stop_check = move || stop.should_stop() || crate::service::should_exit();
    run_placeholder_mount_inner(provider, &cli, &plan.sync_root, &stop_check)
}

/// 占位符挂载主循环（F-12 抽取：前台与 SCM 服务共用同一循环体）。
///
/// `stop_check` 是停止感知 seam：前台传 `|| false`；SCM 传
/// `stop.should_stop() || service::should_exit()`。检查点：轮次之间与
/// 分段 sleep（500ms 步长）处。停止/`--once`/保持时长结束后的退出路径
/// 相同：先停控制通道（其 handler 持有挂载句柄的克隆，`Arc::try_unwrap`
/// 需要它释放），再断开连接 + 双注销（内核 + Shell）干净卸载。
#[cfg(windows)]
fn run_placeholder_mount_inner(
    provider: Arc<dyn Provider>,
    cli: &Cli,
    sync_root: &std::path::Path,
    stop_check: &dyn Fn() -> bool,
) -> anyhow::Result<()> {
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
    // --once（无 hold）：跑一轮即退出；常驻（前台无 flag / 服务路径）：一直保持
    let deadline = (cli.once && cli.hold_secs > 0)
        .then(|| std::time::Instant::now() + Duration::from_secs(cli.hold_secs));
    let exit_after_cycle = cli.once && deadline.is_none();

    // 挂载循环是阻塞实现：直接在调用线程上跑
    loop {
        if stop_check() {
            tracing::info!("收到停止信号，卸载退出");
            break;
        }
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
        // 分段 sleep（500ms 步长）及时响应停止；前台 stop_check 恒 false
        // → 与整段 sleep 行为等价
        let mut remaining = interval;
        while !remaining.is_zero() {
            if stop_check() {
                break;
            }
            let step = Duration::from_millis(500).min(remaining);
            std::thread::sleep(step);
            remaining -= step;
        }
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
fn run_placeholder_mount(
    _cli: &Cli,
    _sync_root: &std::path::Path,
    _file_cfg: &cloudmount_core::config::Config,
) -> anyhow::Result<()> {
    anyhow::bail!("占位符挂载当前仅支持 Windows（macOS FileProvider / Linux 方案见 ROADMAP）")
}

/// F-05 / T-08：Cookie 通道组装决策三态（单元代理；MA11 auto 断言面 + 拒绝语义）。
#[cfg(test)]
mod t08_cookie_gate {
    use super::*;
    use cloudmount_auth::store::{MemoryStore, SecretKind, SecretStore};
    use cloudmount_core::config::{Config, CookieChannelConfig};

    /// 有凭据的注入存储（jar 可恢复 → `is_logged_in` = true）。
    fn seeded_store() -> std::sync::Arc<dyn SecretStore> {
        let store = std::sync::Arc::new(MemoryStore::default());
        let jar = cloudmount_auth::cookie::CookieJar {
            uid: "test-uid".into(),
            cid: "test-cid".into(),
            seid: "test-seid".into(),
            kid: None,
        };
        store
            .put(SecretKind::CookieJar, &serde_json::to_vec(&jar).unwrap())
            .expect("写入测试凭据");
        store
    }

    /// 状态一：默认关 + `--channel cookie` → 拒绝并报错（不静默放行），
    /// 错误信息指明需先 `cloudmount login --with-cookie-channel` 开启并同意。
    #[test]
    fn channel_cookie_default_off_rejected_with_hint() {
        let cli = Cli::parse_from(["cloudmountd", "--channel", "cookie"]);
        let err = match build_channel_provider(&cli, &Config::default()) {
            Err(e) => e,
            Ok(_) => panic!("默认关 + --channel cookie 必须被拒绝，不得静默放行"),
        };
        let msg = format!("{err:#}");
        assert!(
            msg.contains("login --with-cookie-channel"),
            "错误应指明先执行 login --with-cookie-channel: {msg}"
        );
        assert!(
            msg.contains("开启") || msg.contains("同意"),
            "错误应指明显式开启/知情同意语义: {msg}"
        );
    }

    /// 状态二：配置双真 → 策略放行（check_enabled Ok）且构造成功
    /// （既有协议风险 warn 保留在通过把关之后）。
    #[test]
    fn double_true_constructs_provider() {
        let cfg = CookieChannelConfig {
            enabled: true,
            consent_acknowledged: true,
        };
        cookie_policy_from_config(&cfg)
            .check_enabled()
            .expect("双真配置必须放行");
        let provider = try_cookie_provider_with(&cfg, seeded_store());
        assert!(
            provider.is_some(),
            "双真配置 + 已登录凭据 → Cookie Provider 构造成功"
        );
    }

    /// 状态三：auto + 配置未双真 → 静默跳过 Cookie 兜底（纯官方）——
    /// 即使凭据容器已有 Cookie 也不得放行（防硬编码 {true,true} 回潮）。
    #[test]
    fn auto_not_double_true_silently_skips() {
        let states = [
            CookieChannelConfig {
                enabled: false,
                consent_acknowledged: false,
            },
            CookieChannelConfig {
                enabled: true,
                consent_acknowledged: false,
            },
            CookieChannelConfig {
                enabled: false,
                consent_acknowledged: true,
            },
        ];
        for cfg in states {
            assert!(
                try_cookie_provider_with(&cfg, seeded_store()).is_none(),
                "配置未双真（{cfg:?}）必须静默跳过 Cookie 兜底"
            );
        }
    }

    /// check_enabled 拒绝语义：错误链指明"显式开启"与"知情同意"。
    #[test]
    fn check_enabled_rejection_semantics() {
        let off = cookie_policy_from_config(&CookieChannelConfig {
            enabled: false,
            consent_acknowledged: false,
        });
        let msg = format!("{}", off.check_enabled().unwrap_err());
        assert!(msg.contains("开启"), "未开启应指明显式开启: {msg}");

        let no_consent = cookie_policy_from_config(&CookieChannelConfig {
            enabled: true,
            consent_acknowledged: false,
        });
        let msg = format!("{}", no_consent.check_enabled().unwrap_err());
        assert!(msg.contains("同意"), "未同意应指明知情同意: {msg}");
    }
}

/// U12（F-12 + F-13）验收测试：T-21 / T-22 / T-29。
///
/// - T-21：SCM Run 分支决策（[`dispatch_service_run`] 注入两支路工厂捕获
///   调用；两路径产出配置一致锚定 [`service_run_plan`] 同源构造）
///   以及 service.rs 死 FFI 入口删除的结构性断言（MA10：
///   `#[allow(dead_code)]` 使"无告警"判据无判别力，故检源码文本）；
/// - T-22：坏 config.toml → fail-fast（错误链含「解析失败」+ 文件路径），
///   文件不存在 → 默认值回归护栏（路径注入，无环境变量触面）；
/// - T-29：[`assemble_service_args`] 跨平台纯函数快照（`--interval`
///   无条件烘焙；缺省时烘焙解析后的默认值）。
///
/// 全部无 `#[cfg(windows)]` 门控（M-R2-3：参数组装/决策为跨平台纯函数，
/// Linux CI 可执行）。
#[cfg(test)]
mod u12_scm_config {
    use super::*;
    use cloudmount_core::config::{Config, SyncConfig};

    /// env 是进程级的：clap `env =` 属性读取 CLOUDMOUNT_*，与任何显式
    /// set/remove 环境变量的测试必须互斥（config.rs ENV_LOCK 先例）。
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// 清空影响 clap 解析的环境变量（仅测试进程内生效，退出即消失）。
    fn clear_mount_env(_guard: &std::sync::MutexGuard<'_, ()>) {
        std::env::remove_var("CLOUDMOUNT_SYNC_ROOT");
        std::env::remove_var("CLOUDMOUNT_APP_ID");
        std::env::remove_var("CLOUDMOUNT_CHANNEL");
    }

    fn u12_temp_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "cm-u12-{tag}-{}-{:?}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ))
    }

    // ---- T-21：SCM Run 分支决策 + 死入口结构性断言 ----

    /// 结构性断言：service.rs 不得再含死 FFI 入口符号
    /// （`define_windows_service!` + `service_main` 手写解析与 clap
    /// 双份解析，正是 F-12 病灶之一）。
    #[test]
    fn t21_dead_ffi_entry_symbols_removed() {
        let src = include_str!("service.rs");
        assert!(
            !src.contains("define_windows_service!"),
            "死 FFI 入口 define_windows_service! 必须删除（全仓无 service_dispatcher::start）"
        );
        assert!(
            !src.contains("service_main"),
            "死入口 service_main 符号必须不复存在（含文档引用）"
        );
    }

    /// placeholder_mount=true → 占位符支路执行、引擎支路不被调用
    /// （现状 Run 分支不分派 → 红）。
    #[test]
    fn t21_placeholder_true_dispatches_placeholder_branch() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_mount_env(&_guard);
        let cli = Cli::parse_from(["cloudmountd", "--placeholder-mount", "--sync-root", "C:/Cloud"]);
        let calls = Arc::new(std::sync::Mutex::new(Vec::<&'static str>::new()));
        let seen = calls.clone();
        dispatch_service_run(
            &cli,
            &Config::default(),
            |_| {
                seen.lock().unwrap().push("placeholder");
                Ok(())
            },
            |_| {
                seen.lock().unwrap().push("engine");
                Ok(())
            },
        )
        .expect("分派本身不得失败");
        assert_eq!(
            *calls.lock().unwrap(),
            ["placeholder"],
            "placeholder_mount=true 必须且只能走占位符支路（现状静默跑同步模式）"
        );
    }

    /// placeholder_mount=false → 常规引擎支路（SCM 现有行为不回退）。
    #[test]
    fn t21_placeholder_false_dispatches_engine_branch() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_mount_env(&_guard);
        let cli = Cli::parse_from(["cloudmountd", "--sync-root", "C:/Cloud"]);
        let calls = Arc::new(std::sync::Mutex::new(Vec::<&'static str>::new()));
        let seen = calls.clone();
        dispatch_service_run(
            &cli,
            &Config::default(),
            |_| {
                seen.lock().unwrap().push("placeholder");
                Ok(())
            },
            |_| {
                seen.lock().unwrap().push("engine");
                Ok(())
            },
        )
        .expect("分派本身不得失败");
        assert_eq!(
            *calls.lock().unwrap(),
            ["engine"],
            "placeholder_mount=false 必须且只能走常规引擎支路"
        );
    }

    /// 两路径产出配置一致：经 apply_config_defaults 的真实优先级链解析
    /// （文件值注入 clap default）后，placeholder 两种取值下除分派标志外
    /// 全部计划字段一致，且与前台引擎路径同源构造（engine_config_from_plan）。
    #[test]
    fn t21_plan_matches_foreground_construction() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_mount_env(&_guard);
        let file_cfg = Config {
            remote_root: "42".into(),
            interval: 120,
            sync: SyncConfig {
                propagate_remote_deletes: true,
                propagate_local_deletes: false,
            },
            ..Default::default()
        };
        let parse = |args: &[&str]| -> Cli {
            Cli::from_arg_matches(&apply_config_defaults(&file_cfg).get_matches_from(args))
                .expect("clap 解析")
        };
        let root = std::path::PathBuf::from("C:/Cloud");
        let plan_on = service_run_plan(
            &parse(&[
                "cloudmountd",
                "--placeholder-mount",
                "--sync-root",
                "C:/Cloud",
                "--interval",
                "15",
                "--propagate-local-deletes",
            ]),
            &file_cfg,
            root.clone(),
        );
        let plan_off = service_run_plan(
            &parse(&[
                "cloudmountd",
                "--sync-root",
                "C:/Cloud",
                "--interval",
                "15",
                "--propagate-local-deletes",
            ]),
            &file_cfg,
            root.clone(),
        );

        // 分派标志正确
        assert!(plan_on.placeholder_mount);
        assert!(!plan_off.placeholder_mount);

        // 与前台引擎路径同源：engine_config_from_plan 产出的配置
        let cfg_on = engine_config_from_plan(&plan_on);
        let cfg_off = engine_config_from_plan(&plan_off);
        assert_eq!(plan_on.interval_secs, 15, "显式 CLI flag 胜出（clap > 文件）");
        assert_eq!(plan_off.interval_secs, 15);
        assert_eq!(plan_on.remote_root, "42", "文件值经 clap default 注入");
        assert_eq!(plan_off.remote_root, "42");
        assert_eq!(plan_on.sync_root, root);
        assert_eq!(cfg_on.local_root, root);
        assert_eq!(cfg_on.remote_root.0, "42");
        assert!(
            cfg_on.policy.propagate_remote_deletes,
            "文件持久值生效（flag || 文件值，任一为真）"
        );
        assert!(
            cfg_on.policy.propagate_local_deletes,
            "显式 flag 生效（flag || 文件值，任一为真）"
        );
        // 两路径（除分派标志外）产出配置一致
        assert_eq!(cfg_off.local_root, cfg_on.local_root);
        assert_eq!(cfg_off.remote_root, cfg_on.remote_root);
        assert_eq!(
            cfg_off.policy.propagate_remote_deletes,
            cfg_on.policy.propagate_remote_deletes
        );
        assert_eq!(
            cfg_off.policy.propagate_local_deletes,
            cfg_on.policy.propagate_local_deletes
        );
        assert_eq!(plan_off.interval_secs, plan_on.interval_secs);
        assert_eq!(plan_off.remote_root, plan_on.remote_root);
    }

    /// 未显式给 interval → 计划取配置文件默认（服务模式不再恒内置 30s
    /// 的语义锚；与 T-29 烘焙配套）。
    #[test]
    fn t21_plan_takes_interval_from_config_default() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_mount_env(&_guard);
        let file_cfg = Config {
            interval: 120,
            ..Default::default()
        };
        let cli = Cli::from_arg_matches(
            &apply_config_defaults(&file_cfg).get_matches_from(["cloudmountd"]),
        )
        .expect("clap 解析");
        let plan = service_run_plan(&cli, &file_cfg, std::path::PathBuf::from("."));
        assert_eq!(
            plan.interval_secs, 120,
            "文件默认注入后计划取 120（不是内置 30）"
        );
    }

    // ---- T-29：assemble_service_args 跨平台纯函数快照 ----

    /// 全组合快照：`--interval 120` 必须出现在输出向量中（现状不烘焙 → 红）。
    #[test]
    fn t29_snapshot_full_combo_with_interval() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_mount_env(&_guard);
        let cli = Cli::try_parse_from([
            "cloudmountd",
            "--placeholder-mount",
            "--sync-root",
            "D:/Cloud",
            "--remote-root",
            "42",
            "--app-id",
            "9001",
            "--interval",
            "120",
            "--propagate-remote-deletes",
            "--propagate-local-deletes",
        ])
        .expect("解析");
        assert_eq!(
            assemble_service_args(&cli),
            vec![
                "--placeholder-mount".to_string(),
                "--sync-root".to_string(),
                "D:/Cloud".to_string(),
                "--remote-root".to_string(),
                "42".to_string(),
                "--app-id".to_string(),
                "9001".to_string(),
                "--interval".to_string(),
                "120".to_string(),
                "--propagate-remote-deletes".to_string(),
                "--propagate-local-deletes".to_string(),
            ],
            "整向量快照：--interval 120 必须无条件烘焙（MA9：现状缺 --interval）"
        );
    }

    /// 未显式给 interval → 烘焙内置默认值（30）。
    #[test]
    fn t29_bakes_builtin_default_interval() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_mount_env(&_guard);
        let cli = Cli::parse_from(["cloudmountd"]);
        assert_eq!(
            assemble_service_args(&cli),
            vec!["--interval".to_string(), "30".to_string()],
            "缺省 interval 也必须烘焙（快照固化：无\"不烘焙\"分支）"
        );
    }

    /// 未显式给 interval 但配置文件给了 → 烘焙文件默认
    /// （apply_config_defaults 注入 clap default 后的解析结果）。
    #[test]
    fn t29_bakes_config_file_interval_default() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_mount_env(&_guard);
        let file_cfg = Config {
            interval: 120,
            ..Default::default()
        };
        let cli = Cli::from_arg_matches(
            &apply_config_defaults(&file_cfg).get_matches_from(["cloudmountd"]),
        )
        .expect("解析");
        assert_eq!(
            assemble_service_args(&cli),
            vec!["--interval".to_string(), "120".to_string()],
            "烘焙值 = clap 解析结果（含配置文件默认注入）"
        );
    }

    /// 部分组合：缺省段不烘焙（remote_root=0 / 无 sync_root / propagate 关）。
    #[test]
    fn t29_snapshot_partial_combo_omits_unset() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_mount_env(&_guard);
        let cli = Cli::parse_from(["cloudmountd", "--sync-root", "E:/Sync"]);
        assert_eq!(
            assemble_service_args(&cli),
            vec![
                "--sync-root".to_string(),
                "E:/Sync".to_string(),
                "--interval".to_string(),
                "30".to_string(),
            ]
        );
    }

    // ---- T-22：配置 fail-fast（F-13）----

    /// 语法错误 → Err：错误链含「解析失败」统一子串 + 文件路径 + daemon
    /// 侧恢复提示；Err 本身即"默认值不生效"（现状静默回退 → 红）。
    #[test]
    fn t22_malformed_config_fails_fast_with_parse_failure_and_path() {
        let path = u12_temp_path("t22-syntax");
        std::fs::write(&path, "not = [valid").unwrap();
        let err = load_config_or_fail(&path).expect_err("坏配置必须拒绝启动（fail-fast）");
        let chain = format!("{err:#}");
        assert!(
            chain.contains("解析失败"),
            "错误链须含统一子串「解析失败」（断言锚 = load_from 消息）: {chain}"
        );
        assert!(
            chain.contains(&path.display().to_string()),
            "错误链须含文件路径: {chain}"
        );
        assert!(
            chain.contains("删除后重启"),
            "daemon context 须提示恢复默认配置的途径: {chain}"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// 未知字段（deny_unknown_fields 防拼写设计）→ 同样 fail-fast。
    #[test]
    fn t22_unknown_field_fails_fast() {
        let path = u12_temp_path("t22-unknown");
        std::fs::write(&path, "no_such_field = 1\n").unwrap();
        let err = load_config_or_fail(&path).expect_err("未知字段必须拒绝启动");
        let chain = format!("{err:#}");
        assert!(chain.contains("解析失败"), "错误链须含「解析失败」: {chain}");
        assert!(
            chain.contains(&path.display().to_string()),
            "错误链须含文件路径: {chain}"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// 路径指向目录（"存在但不可读"的跨平台构造）→ Err（拒绝启动）。
    #[test]
    fn t22_directory_path_fails_fast() {
        let dir = u12_temp_path("t22-dir");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(
            load_config_or_fail(&dir).is_err(),
            "存在但不可读（目录）必须拒绝启动"
        );
        let _ = std::fs::remove_dir(&dir);
    }

    /// 回归护栏：文件不存在 → 默认值（首次运行正常路径不受 fail-fast 影响）。
    #[test]
    fn t22_missing_file_yields_defaults() {
        let path = u12_temp_path("t22-absent");
        let cfg = load_config_or_fail(&path).expect("文件不存在必须回默认值");
        assert_eq!(cfg, Config::default());
        assert_eq!(cfg.channel, "auto");
        assert_eq!(cfg.interval, 30);
    }
}
