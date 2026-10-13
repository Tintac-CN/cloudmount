//! Windows 服务集成（SCM 协议 + 安装/卸载/启停）。
//!
//! 仅 Windows 编译。依赖 `windows-service` crate：
//! - [`run_service`]：SCM 入口（`define_windows_service!`）；
//!   停止事件 → [`SERVICE_EXIT`] 标志，引擎循环自行退出。
//! - [`install`]：`ServiceManager::create_service` 注册
//!   （OWN_PROCESS + AutoStart；启动参数携带 --sync-root 等配置）。
//!
//! ## 与引擎循环的协议
//!
//! 服务模式下 main.rs 调 [`should_exit`] 决定是否退出循环
//! （引擎分段 sleep 时检查，SCM Stop 事件置位 [`SERVICE_EXIT`]）。
//! [`service_main`] 解析 SCM 传入的 launch_arguments（install 时烘焙的
//! --sync-root 等 flag），构造 Provider 与配置后驱动引擎。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use anyhow::Context as _;

pub const SERVICE_NAME: &str = "CloudMount";
pub const SERVICE_DISPLAY_NAME: &str = "CloudMount 云挂服务";

/// 服务停止标志（SCM Stop 事件置位；引擎循环每秒检查）。
pub static SERVICE_EXIT: AtomicBool = AtomicBool::new(false);

/// 当前 exe 的绝对路径（服务 binPath 用）。
pub fn current_exe() -> anyhow::Result<std::path::PathBuf> {
    std::env::current_exe().context("定位当前 exe 失败")
}

/// `service install`：注册 Windows 服务（开机自启）。
///
/// `launch_args` 是服务启动时传给 cloudmountd 的参数
/// （如 `--placeholder-mount --sync-root D:\Cloud`）。
pub fn install(launch_args: &[String]) -> anyhow::Result<()> {
    use windows_service::{
        service::{ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType},
        service_manager::{ServiceManager, ServiceManagerAccess},
    };

    let exe = current_exe()?;
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )?;

    // 幂等：已存在先删
    let _ = uninstall();

    let mut launch: Vec<String> = vec!["service".into(), "run".into()];
    launch.extend_from_slice(launch_args);

    let info = ServiceInfo {
        name: SERVICE_NAME.into(),
        display_name: SERVICE_DISPLAY_NAME.into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe,
        launch_arguments: launch.into_iter().map(std::ffi::OsString::from).collect(),
        dependencies: Vec::new(),
        account_name: None, // LocalSystem
        account_password: None,
    };

    let service = manager.create_service(&info, ServiceAccess::CHANGE_CONFIG)?;
    service.set_description("CloudMount 115 网盘同步服务（双向同步/占位符挂载）")?;

    tracing::info!(service = SERVICE_NAME, "Windows 服务已安装（开机自启）");
    Ok(())
}

/// `service uninstall`：停止并删除服务。
pub fn uninstall() -> anyhow::Result<()> {
    service_stop()?;
    use windows_service::{
        service::{ServiceAccess, ServiceState},
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::ALL_ACCESS)?;
    let service =
        manager.open_service(SERVICE_NAME, ServiceAccess::STOP | ServiceAccess::DELETE)?;

    if let Ok(status) = service.query_status() {
        if status.current_state == ServiceState::Running {
            service.stop()?;
            for _ in 0..20 {
                if service.query_status()?.current_state == ServiceState::Stopped {
                    break;
                }
                std::thread::sleep(Duration::from_millis(300));
            }
        }
    }
    service.delete()?;
    tracing::info!(service = SERVICE_NAME, "Windows 服务已删除");
    Ok(())
}

/// `service start`。
pub fn service_start() -> anyhow::Result<()> {
    use windows_service::{
        service::{ServiceAccess, ServiceState},
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = manager.open_service(SERVICE_NAME, ServiceAccess::START)?;
    service.start(&[] as &[&std::ffi::OsStr])?;
    for _ in 0..20 {
        if service.query_status()?.current_state == ServiceState::Running {
            tracing::info!(service = SERVICE_NAME, "服务已启动");
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    anyhow::bail!("服务启动超时")
}

/// `service stop`。
pub fn service_stop() -> anyhow::Result<()> {
    use windows_service::{
        service::ServiceAccess,
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = manager.open_service(SERVICE_NAME, ServiceAccess::STOP)?;
    match service.stop() {
        Ok(status) => {
            tracing::info!(service = SERVICE_NAME, state = ?status.current_state, "服务停止中");
            Ok(())
        }
        Err(e) if e.to_string().contains("1062") => Ok(()), // 未运行
        Err(e) => Err(anyhow::anyhow!("停止服务失败: {e}")),
    }
}

/// 服务停止标志（引擎循环每秒检查；SCM Stop 事件置位）。
pub fn should_exit() -> bool {
    SERVICE_EXIT.load(Ordering::SeqCst)
}

// ---- SCM 运行入口 ----

windows_service::define_windows_service!(ffi_service_main, service_main);

#[allow(dead_code)] // 由 define_windows_service! 宏经 FFI 调用
fn service_main(args: Vec<std::ffi::OsString>) {
    // args[0] 为 exe 名，args[1..] 为 install 时烘焙的 launch_arguments。
    // 仅解析同步模式所需的最小 flag 集（与 build_launch_args 输出对齐）。
    let mut sync_root = ".".to_string();
    let mut remote_root = "0".to_string();
    let mut app_id: Option<String> = None;
    let mut interval = 30u64;
    let mut propagate_remote = false;
    let mut propagate_local = false;

    let mut it = args.into_iter().skip(1);
    while let Some(arg) = it.next() {
        let Some(arg) = arg.to_str() else { continue };
        // 注意：arg 已是 &str（to_str shadow），勿再调 as_str（&str::as_str 不稳定）
        let takes_value = matches!(
            arg,
            "--sync-root" | "--remote-root" | "--app-id" | "--interval"
        );
        let mut value = String::new();
        if takes_value {
            if let Some(v) = it.next() {
                value = v.to_string_lossy().to_string();
            }
        }
        match arg {
            "--sync-root" => sync_root = value,
            "--remote-root" => remote_root = value,
            "--app-id" => app_id = Some(value),
            "--interval" => interval = value.parse().unwrap_or(30),
            "--propagate-remote-deletes" => propagate_remote = true,
            "--propagate-local-deletes" => propagate_local = true,
            _ => {}
        }
    }

    // Provider 构造（与命令行模式一致：从系统凭据库恢复 token）
    let Some(app_id) = app_id.as_deref() else {
        tracing::error!("服务缺少 AppID");
        return;
    };
    let provider: Arc<dyn cloudmount_core::Provider> =
        match crate::build_provider_from_app_id(app_id) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(error = %e, "Provider 构造失败（服务退出）");
                return;
            }
        };

    let mut cfg = cloudmount_sync::SyncEngineConfig::new(&sync_root);
    cfg.remote_root = cloudmount_core::RemoteId(remote_root);
    cfg.policy.propagate_remote_deletes = propagate_remote;
    cfg.policy.propagate_local_deletes = propagate_local;

    let cfg_for_engine = cfg.clone();
    let provider_for_engine = provider.clone();
    if let Err(e) = run_service_scm_executor(
        cfg_for_engine,
        provider_for_engine,
        interval.max(1),
        move |stop| crate::run_service_engine_blocking(cfg, provider, interval.max(1), stop),
    ) {
        tracing::error!(error = %e, "服务运行失败");
    }
}

pub fn run_service_scm_executor(
    _cfg: cloudmount_sync::SyncEngineConfig,
    _provider: std::sync::Arc<dyn cloudmount_core::Provider>,
    _interval_secs: u64,
    engine_loop: impl FnOnce(crate::service::StopSignal) + Send + 'static,
) -> windows_service::Result<()> {
    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

    let (tx, rx) = std::sync::mpsc::channel::<()>();

    let handler = move |control: ServiceControl| match control {
        ServiceControl::Stop | ServiceControl::Shutdown => {
            SERVICE_EXIT.store(true, Ordering::SeqCst);
            let _ = tx.send(());
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(60), // 引擎轮次最长可超 30s
        process_id: None,
    })?;

    tracing::info!("服务进入 Running（等待 SCM 停止事件）");

    // 引擎循环：由 main.rs 注入；SERVICE_EXIT/停止通道由其自行消费
    engine_loop(crate::service::StopSignal(rx));

    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(1),
        process_id: None,
    });

    Ok(())
}

/// SCM 停止信号（`run_service` 注入给引擎循环）。
pub struct StopSignal(pub(crate) mpsc::Receiver<()>);

impl StopSignal {
    /// 非阻塞检查是否收到停止事件。
    pub fn should_stop(&self) -> bool {
        matches!(
            self.0.try_recv(),
            Ok(()) | Err(mpsc::TryRecvError::Disconnected)
        )
    }
}
