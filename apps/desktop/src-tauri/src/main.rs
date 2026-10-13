#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! # CloudMount 桌面壳（Tauri 2）
//!
//! 职责：
//! - **系统托盘**：状态角标（同步中/空闲/暂停/未登录）+ 菜单（暂停/恢复、
//!   打开同步目录、退出）；
//! - **主窗口**：托管共享 web-ui（状态/缓存/设置/日志页）；
//! - **命令面**：实现 web-ui `src/api.ts` 契约（与 cloudmountd 的 REST
//!   同名同参）：状态/缓存统计/清缓存/配额/暂停/同步根/日志导出；
//! - **同步引擎线程**：SyncEngine 循环（与 cloudmountd 相同逻辑），
//!   引擎经 `Arc<Mutex>` 与命令面共享（统计查询/清缓存/配额），
//!   暂停经 `AtomicBool`。
//!
//! ## 平台注意（来自调研，开发时对照）
//! - Linux/WebKitGTK：托盘**点击事件不触发**（菜单仍可用）→ 菜单结构固定，
//!   仅更新项文本；不依赖点击事件。
//! - Windows：用户缺 WebView2 时无法运行 → 安装器选 downloadBootstrapper。

use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WindowEvent};

/// 命令面与引擎线程共享的状态快照。
///
/// 序列化为 camelCase——与 web-ui `src/api.ts` 的 `StatusInfo` 契约对齐
/// （snake_case 会让前端读到 undefined：队列显示 ↑undefined、
/// lastMessage 静默丢失）。
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    pub version: String,
    pub logged_in: bool,
    pub syncing: bool,
    pub paused: bool,
    pub pending_uploads: u64,
    pub pending_downloads: u64,
    pub conflicts: u64,
    pub errors: u64,
    pub last_message: String,
    /// 累计上传（引擎线程每轮累加）。
    pub total_uploaded: u64,
    /// 累计下载。
    pub total_downloaded: u64,
}

/// 缓存统计快照（web-ui `CacheInfo` 契约，camelCase）。
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheSnapshot {
    pub bytes: u64,
    pub limit_bytes: u64,
    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub evictions: u64,
}

impl From<cloudmount_sync::CacheStats> for CacheSnapshot {
    fn from(s: cloudmount_sync::CacheStats) -> Self {
        Self {
            bytes: s.bytes,
            limit_bytes: s.limit_bytes,
            hits: s.hits,
            misses: s.misses,
            inserts: s.inserts,
            evictions: s.evictions,
        }
    }
}

/// 配额快照（web-ui `QuotaInfo` 契约）。
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaSnapshot {
    pub total: u64,
    pub used: u64,
    pub remaining: u64,
}

impl From<cloudmount_core::Quota> for QuotaSnapshot {
    fn from(q: cloudmount_core::Quota) -> Self {
        Self {
            total: q.total,
            used: q.used,
            remaining: q.remaining,
        }
    }
}

/// 应用共享状态。
struct AppState {
    paused: Arc<AtomicBool>,
    status: Arc<Mutex<StatusSnapshot>>,
    sync_root: Mutex<Option<std::path::PathBuf>>,
    /// 引擎句柄（引擎线程启动后注入；None = 引擎不可用）。
    engine: Arc<Mutex<Option<Arc<Mutex<cloudmount_sync::SyncEngine>>>>>,
    /// 日志环形缓冲（导出用）。
    logs: Arc<LogBuffer>,
}

/// 内存日志环形缓冲（tracing 写入，导出命令读取）。
///
/// 容量按行计（2000 行 × 平均 150B ≈ 300KB）。
struct LogBuffer {
    lines: Mutex<Vec<String>>,
    capacity: usize,
}

impl LogBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            lines: Mutex::new(Vec::with_capacity(capacity)),
            capacity,
        }
    }

    fn push(&self, line: String) {
        if let Ok(mut v) = self.lines.lock() {
            if v.len() >= self.capacity {
                v.remove(0);
            }
            v.push(line);
        }
    }

    fn snapshot(&self) -> Vec<String> {
        self.lines.lock().map(|v| v.clone()).unwrap_or_default()
    }
}

/// tracing 写入器：同时写 stderr 与环形缓冲（日志导出）。
struct SharedWriter {
    buffer: Arc<LogBuffer>,
}

impl std::io::Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use std::io::Write as _;
        let _ = std::io::stderr().write_all(buf);
        for line in String::from_utf8_lossy(buf).lines() {
            if !line.trim().is_empty() {
                self.buffer.push(line.to_string());
            }
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn main() {
    let logs = Arc::new(LogBuffer::new(2000));
    let writer = SharedWriter { buffer: logs.clone() };
    tracing_subscriber::fmt()
        .with_writer(std::sync::Mutex::new(writer))
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    tauri::Builder::default()
        .setup(move |app| {
            // ---- 共享状态 ----
            let paused = Arc::new(AtomicBool::new(false));
            let status = Arc::new(Mutex::new(StatusSnapshot {
                version: cloudmount_core::VERSION.to_string(),
                ..Default::default()
            }));
            let engine_slot: Arc<Mutex<Option<Arc<Mutex<cloudmount_sync::SyncEngine>>>>> =
                Arc::new(Mutex::new(None));
            app.manage(AppState {
                paused: paused.clone(),
                status: status.clone(),
                sync_root: Mutex::new(None),
                engine: engine_slot.clone(),
                logs,
            });

            // ---- 托盘 ----
            let open = MenuItem::with_id(app, "open", "打开主界面", true, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "暂停同步", true, None::<&str>)?;
            let folder = MenuItem::with_id(app, "folder", "打开同步目录", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &toggle, &folder, &quit])?;
            // 菜单项克隆进事件闭包（Linux WebKitGTK 托盘菜单不可移除，只能改文本）
            let toggle_item = toggle.clone();

            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().expect("应用图标").clone())
                .menu(&menu)
                .tooltip("CloudMount 云挂")
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "open" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "toggle" => {
                        let now_paused = {
                            let state = app.state::<AppState>();
                            let p = state.paused.fetch_xor(true, Ordering::SeqCst);
                            if let Ok(mut s) = state.status.lock() {
                                s.paused = !p;
                            }
                            p
                        };
                        let label = if now_paused { "恢复同步" } else { "暂停同步" };
                        let _ = toggle_item.set_text(label);
                    }
                    "folder" => {
                        let root = app
                            .state::<AppState>()
                            .sync_root
                            .lock()
                            .ok()
                            .and_then(|g| g.clone());
                        if let Some(root) = root {
                            let _ = open_folder(&root);
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            // ---- 同步引擎线程 ----
            // 与 cloudmountd 相同的循环；未登录/未配置时仅更新状态不启动。
            std::thread::Builder::new()
                .name("cm-engine".into())
                .spawn(move || engine_loop(paused, status, engine_slot))?;

            Ok(())
        })
        .on_window_event(|window, event| {
            // 关窗 = 隐藏到托盘（不退出）
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_cache_stats,
            clear_cache,
            get_quota,
            set_sync_root,
            toggle_pause,
            open_sync_folder,
            export_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 引擎循环（与 cloudmountd --interval 相同语义）。
fn engine_loop(
    paused: Arc<AtomicBool>,
    status: Arc<Mutex<StatusSnapshot>>,
    engine_slot: Arc<Mutex<Option<Arc<Mutex<cloudmount_sync::SyncEngine>>>>>,
) {
    // 读取配置：AppID + 同步根（环境变量；正式版从配置文件读）
    let app_id = std::env::var("CLOUDMOUNT_APP_ID").ok();
    let sync_root = std::env::var("CLOUDMOUNT_SYNC_ROOT")
        .map(std::path::PathBuf::from)
        .ok();

    let Some(sync_root) = sync_root else {
        if let Ok(mut s) = status.lock() {
            s.last_message = "未配置同步根（CLOUDMOUNT_SYNC_ROOT）".into();
        }
        return;
    };
    if let Ok(mut s) = status.lock() {
        s.syncing = true;
    }

    let provider: Option<Arc<dyn cloudmount_core::Provider>> = app_id.and_then(|id| {
        let store = cloudmount_auth::store::default_store().ok()?;
        let auth = Arc::new(cloudmount_auth::open::OpenApiAuth::new(id, store));
        if !auth.is_logged_in() {
            if let Ok(mut s) = status.lock() {
                s.logged_in = false;
                s.last_message = "未登录：请在主界面扫码".into();
            }
            return None;
        }
        if let Ok(mut s) = status.lock() {
            s.logged_in = true;
        }
        Some(Arc::new(cloudmount_driver115::Open115Provider::new(auth)) as Arc<dyn cloudmount_core::Provider>)
    });

    let Some(provider) = provider else {
        return;
    };

    let mut cfg = cloudmount_sync::SyncEngineConfig::new(&sync_root);
    cfg.scheduler.paused = paused.clone();
    let Ok(engine) = cloudmount_sync::SyncEngine::new(cfg, provider) else {
        if let Ok(mut s) = status.lock() {
            s.last_message = "引擎初始化失败".into();
        }
        return;
    };
    let engine = Arc::new(Mutex::new(engine));
    // 注入共享句柄（命令面可查询统计/清缓存/配额）
    if let Ok(mut slot) = engine_slot.lock() {
        *slot = Some(engine.clone());
    }

    loop {
        if paused.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        }
        let result = engine.lock().map(|mut e| e.run_once());
        match result {
            Ok(Ok(stats)) => {
                if let Ok(mut s) = status.lock() {
                    s.total_uploaded += stats.uploaded;
                    s.total_downloaded += stats.downloaded;
                    s.conflicts = stats.conflicts;
                    s.errors = stats.errors;
                    if stats.has_changes() {
                        s.last_message = format!(
                            "上传 {} · 下载 {} · 冲突 {} · 错误 {}",
                            stats.uploaded, stats.downloaded, stats.conflicts, stats.errors
                        );
                    }
                }
            }
            Ok(Err(e)) => tracing::error!(error = %e, "同步轮次失败（将继续下一轮）"),
            Err(_) => tracing::error!("引擎锁中毒"),
        }
        std::thread::sleep(Duration::from_secs(30));
    }
}

/// 打开本地目录（资源管理器/Finder/xdg-open）。
fn open_folder(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open").arg(path).spawn()?;
        Ok(())
    }
}

// ============ 命令面（web-ui src/api.ts 契约） ============

/// 查询状态（对应 web-ui `getStatus`）。
#[tauri::command]
fn get_status(state: tauri::State<AppState>) -> StatusSnapshot {
    state.status.lock().map(|s| s.clone()).unwrap_or_default()
}

/// 缓存统计（占用/命中率/写入/淘汰）。
#[tauri::command]
fn get_cache_stats(state: tauri::State<AppState>) -> Result<CacheSnapshot, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?.clone();
    let Some(engine) = engine else {
        return Err("引擎未运行".into());
    };
    let guard = engine.lock().map_err(|e| e.to_string())?;
    guard
        .cache_stats()
        .map(CacheSnapshot::from)
        .map_err(|e| e.to_string())
}

/// 清空内容缓存（一键清理；元数据不受影响）。
#[tauri::command]
fn clear_cache(state: tauri::State<AppState>) -> Result<(), String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?.clone();
    let Some(engine) = engine else {
        return Err("引擎未运行".into());
    };
    let guard = engine.lock().map_err(|e| e.to_string())?;
    guard.cache().clear().map_err(|e| e.to_string())
}

/// 云端配额。
#[tauri::command]
fn get_quota(state: tauri::State<AppState>) -> Result<QuotaSnapshot, String> {
    let engine = state.engine.lock().map_err(|e| e.to_string())?.clone();
    let Some(engine) = engine else {
        return Err("引擎未运行".into());
    };
    let guard = engine.lock().map_err(|e| e.to_string())?;
    guard
        .provider()
        .quota()
        .map(QuotaSnapshot::from)
        .map_err(|e| e.to_string())
}

/// 设置同步根目录（设置页）。
#[tauri::command]
fn set_sync_root(state: tauri::State<AppState>, path: String) -> Result<(), String> {
    let root = std::path::PathBuf::from(&path);
    if !root.is_dir() {
        return Err(format!("目录不存在: {path}"));
    }
    *state
        .sync_root
        .lock()
        .map_err(|e| e.to_string())? = Some(root);
    Ok(())
}

/// 暂停/恢复（托盘与设置页共用）。
#[tauri::command]
fn toggle_pause(state: tauri::State<AppState>) -> Result<bool, String> {
    let was = state.paused.fetch_xor(true, Ordering::SeqCst);
    Ok(!was) // 返回新状态（true=已暂停）
}

/// 打开同步目录（资源管理器）。
#[tauri::command]
fn open_sync_folder(state: tauri::State<AppState>) -> Result<(), String> {
    let root = state
        .sync_root
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    let Some(root) = root else {
        return Err("未设置同步根目录".into());
    };
    open_folder(&root).map_err(|e| e.to_string())
}

/// 导出最近日志（环形缓冲快照，前端组装下载）。
#[tauri::command]
fn export_logs(state: tauri::State<AppState>) -> Result<Vec<String>, String> {
    Ok(state.logs.snapshot())
}
