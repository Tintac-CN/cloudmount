#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! CloudMount 桌面壳（Tauri 2）。
//!
//! 职责：
//! - 托管共享 web-ui（系统 WebView 渲染）；
//! - 系统托盘：状态角标（同步中/空闲/错误）+ 右键菜单（暂停/恢复、缓存清理、设置）；
//! - 实现 web-ui `src/api.ts` 契约中的 `#[tauri::command]`（与 cloudmountd 的
//!   REST `/api/*` 保持同名同参）；
//! - M2：接入平台挂载后端（Windows: CfAPI → 自研 minifilter；macOS: FileProvider）。
//!
//! 注意：命令实现应尽量薄，转发给 cloudmount-core；重逻辑一律在核心 crate，
//! 保证与边缘 cloudmountd 行为一致。

fn main() {
    tauri::Builder::default()
        .setup(|_app| {
            // TODO(M2): 创建系统托盘（TrayIconBuilder），图标按同步状态动态切换：
            //   idle / syncing(角标) / error(红点)；菜单项：打开主界面、暂停同步、
            //   清理缓存、退出。Linux 注意：托盘点击事件不触发（WebKitGTK 已知限制），
            //   菜单设置后无法移除 —— 菜单结构保持稳定，仅更新菜单项文本/勾选态。
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            begin_login,
            list_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 查询同步与账号状态（对应 web-ui `getStatus`）。
#[tauri::command]
fn get_status() -> serde_json::Value {
    // TODO(M2): 从 cloudmount-core 的引擎状态聚合。
    serde_json::json!({
        "version": cloudmount_core::VERSION,
        "loggedIn": false,
        "channel": "open",
        "syncing": false,
        "pendingUploads": 0,
        "pendingDownloads": 0,
        "cacheBytes": 0,
        "cacheLimitBytes": 10_000_000_000u64,
    })
}

/// 发起扫码登录（对应 web-ui `beginLogin`）。
#[tauri::command]
fn begin_login(method: String) -> Result<serde_json::Value, String> {
    match method.as_str() {
        "open" => Err("待 M0/M2：官方 OpenAPI 设备码扫码".into()),
        "cookie" => Err("Cookie 补充通道默认关闭，且需知情同意（M3）".into()),
        other => Err(format!("unknown login method: {other}")),
    }
}

/// 列出远端目录（对应 web-ui `listDir`）。
#[tauri::command]
fn list_dir(_dir_id: String) -> Result<serde_json::Value, String> {
    Err("待 M0/M2".into())
}
