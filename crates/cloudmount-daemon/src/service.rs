//! Windows 服务集成（SCM 协议 + 安装/卸载/启停）。
//!
//! 仅 Windows 编译。依赖 `windows-service` crate：
//! - [`run_service_scm_executor`]：SCM 运行执行体（手动注册控制处理器；
//!   SCM 实际入口是 main.rs 的 `service run` clap 分支）；
//!   停止事件 → [`SERVICE_EXIT`] 标志，注入的引擎/挂载循环自行退出。
//! - [`install`]：`ServiceManager::create_service` 注册
//!   （OWN_PROCESS + AutoStart；启动参数由 main.rs `assemble_service_args`
//!   烘焙，launch_arguments 一律由 clap 统一解析）。
//!
//! ## 与引擎/挂载循环的协议
//!
//! 服务模式下 main.rs 调 [`should_exit`] 决定是否退出循环
//! （循环分段 sleep 时检查，SCM Stop 事件置位 [`SERVICE_EXIT`]）。
//! 循环体（引擎循环 / 占位符挂载循环，F-12 两支路）由 main.rs 注入
//! [`run_service_scm_executor`]。
//!
//! 注：不使用 windows-service 的 FFI 服务入口宏——历史上的双份参数解析已
//! 删除（F-12：全仓无 `service_dispatcher::start`，死入口与 clap 解析漂移）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
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

// ---- SCM 运行执行体 ----

/// SCM 运行执行体：注册控制处理器 → Running → 注入循环 → Stopped。
///
/// SCM 实际入口是 main.rs 的 `service run` clap 分支（launch_arguments 由
/// clap 统一解析；历史上的 FFI 入口与手写参数解析已删除——F-12 死入口，
/// 消除与 clap 的双份解析漂移）。循环体由调用方注入（引擎循环 /
/// 占位符挂载循环，F-12 两支路共用本执行体）；SCM Stop/Shutdown 事件
/// 置位 [`SERVICE_EXIT`] 并经停止通道唤醒循环。
pub fn run_service_scm_executor(
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

/// SCM 停止信号（[`run_service_scm_executor`] 注入给引擎/挂载循环）。
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
