//! # CloudMount 桌面应用（纯 Rust）
//!
//! egui + wgpu + rfd——**无 WebView、无 HTML/JS**。服务端/边缘模式仍走
//! web-ui（`cloudmountd --serve`），两端 UI 各自独立。
//!
//! 职责：
//! - **状态页**：引擎统计 + 云端配额
//! - **缓存页**：占用/命中率 + 一键清理
//! - **设置页**：同步根（rfd 目录选择）、暂停/恢复、日志导出
//! - **登录页**：扫码二维码渲染（qrcode → egui 纹理）
//!
//! 引擎线程与 daemon 同构：`Arc<Mutex<SyncEngine>>` 句柄槽 +
//! `AtomicBool` 暂停 + 日志环形缓冲。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use cloudmount_auth::Authenticator as _;

/// 引擎线程与 UI 共享的状态快照。
#[derive(Debug, Clone, Default)]
pub struct StatusSnapshot {
    pub version: String,
    pub logged_in: bool,
    pub syncing: bool,
    pub paused: bool,
    pub total_uploaded: u64,
    pub total_downloaded: u64,
    pub conflicts: u64,
    pub errors: u64,
    pub last_message: String,
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

/// 内存日志环形缓冲（2000 行）。
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

/// tracing 写入器：stderr + 环形缓冲。
struct SharedWriter {
    buffer: Arc<LogBuffer>,
}

impl std::io::Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
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

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Status,
    Cache,
    Settings,
    Login,
}

/// egui 应用。
struct CloudMountApp {
    state: Arc<AppState>,
    tab: Tab,
    /// 登录页的二维码纹理。
    qr_texture: Option<egui::TextureHandle>,
    /// 登录挑战（轮询用）。
    login_challenge: Option<cloudmount_auth::LoginChallenge>,
    /// 登录进行中。
    login_polling: bool,
    /// 引擎线程（首帧启动）。
    engine_thread: Option<std::thread::JoinHandle<()>>,
}

impl CloudMountApp {
    fn new(_cc: &eframe::CreationContext<'_>, state: Arc<AppState>) -> Self {
        Self {
            state,
            tab: Tab::Status,
            qr_texture: None,
            login_challenge: None,
            login_polling: false,
            engine_thread: None,
        }
    }

    fn login_auth(&self) -> cloudmount_auth::cookie::CookieAuth {
        let policy = cloudmount_auth::cookie::CookiePolicy {
            enabled: true,
            consent_acknowledged: true,
        };
        let store = cloudmount_auth::store::default_store().expect("凭据存储");
        cloudmount_auth::cookie::CookieAuth::new(policy, store)
    }

    fn engine(&self) -> Option<Arc<Mutex<cloudmount_sync::SyncEngine>>> {
        self.state.engine.lock().ok().and_then(|g| g.clone())
    }
}

impl eframe::App for CloudMountApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 引擎线程首帧启动（egui 主线程不能阻塞）
        if self.engine_thread.is_none() {
            let state = self.state.clone();
            self.engine_thread = Some(
                std::thread::Builder::new()
                    .name("cm-engine".into())
                    .spawn(move || engine_loop(state))
                    .expect("引擎线程启动"),
            );
        }

        egui::TopBottomPanel::top("nav").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("CloudMount 云挂");
                ui.separator();
                ui.selectable_value(&mut self.tab, Tab::Status, "状态");
                ui.selectable_value(&mut self.tab, Tab::Cache, "缓存");
                ui.selectable_value(&mut self.tab, Tab::Settings, "设置");
                ui.selectable_value(&mut self.tab, Tab::Login, "登录");
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.tab {
                Tab::Status => self.status_panel(ui),
                Tab::Cache => self.cache_panel(ui),
                Tab::Settings => self.settings_panel(ui),
                Tab::Login => self.login_panel(ui, ctx),
            }
        });

        // 登录轮询（每帧检查——轮询接口是短轮询立即返回，不阻塞）
        if self.login_polling {
            if let Some(challenge) = &self.login_challenge {
                let auth = self.login_auth();
                match auth.poll_login(challenge) {
                    Ok(Some(_)) => {
                        self.login_polling = false;
                        self.login_challenge = None;
                        self.qr_texture = None;
                        if let Ok(mut s) = self.state.status.lock() {
                            s.logged_in = true;
                            s.last_message = "登录成功（Cookie 已入独立容器）".into();
                        }
                    }
                    Ok(None) => {} // 继续等待
                    Err(e) => {
                        self.login_polling = false;
                        self.login_challenge = None;
                        self.qr_texture = None;
                        if let Ok(mut s) = self.state.status.lock() {
                            s.last_message = format!("登录失败: {e}");
                        }
                    }
                }
            }
        }

        // 状态刷新节奏（登录轮询时更快）
        ctx.request_repaint_after(if self.login_polling {
            Duration::from_millis(500)
        } else {
            Duration::from_secs(1)
        });
    }
}

impl CloudMountApp {
    fn status_panel(&self, ui: &mut egui::Ui) {
        let s = self
            .state
            .status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default();
        ui.heading("状态");
        ui.separator();

        egui::Grid::new("status").num_columns(2).show(ui, |ui| {
            ui.strong("版本").on_hover_text("CloudMount 版本");
            ui.label(&s.version);
            ui.end_row();
            ui.strong("登录");
            ui.label(if s.logged_in { "已登录" } else { "未登录" });
            ui.end_row();
            ui.strong("同步");
            ui.label(if s.paused {
                "已暂停"
            } else if s.syncing {
                "进行中"
            } else {
                "空闲"
            });
            ui.end_row();
            ui.strong("累计上传");
            ui.label(format!("{}", s.total_uploaded));
            ui.end_row();
            ui.strong("累计下载");
            ui.label(format!("{}", s.total_downloaded));
            ui.end_row();
            ui.strong("冲突");
            ui.label(format!("{}", s.conflicts));
            ui.end_row();
            ui.strong("错误");
            ui.label(format!("{}", s.errors));
            ui.end_row();
        });

        if !s.last_message.is_empty() {
            ui.separator();
            ui.weak(format!("最近：{}", s.last_message));
        }

        // 配额（引擎可用时）
        if let Some(engine) = self.engine() {
            if let Ok(q) = engine.lock().unwrap().provider().quota() {
                if q.total > 0 {
                    ui.separator();
                    ui.heading("云端配额");
                    let ratio = q.used as f32 / q.total as f32;
                    ui.add(
                        egui::ProgressBar::new(ratio).text(format!(
                            "{}/{}",
                            human_bytes(q.used),
                            human_bytes(q.total)
                        )),
                    );
                }
            }
        }
    }

    fn cache_panel(&self, ui: &mut egui::Ui) {
        ui.heading("内容缓存");
        ui.separator();

        if let Some(engine) = self.engine() {
            if let Ok(stats) = engine.lock().unwrap().cache_stats() {
                let total = stats.hits + stats.misses;
                let hit_rate = if total > 0 {
                    stats.hits as f32 / total as f32
                } else {
                    0.0
                };
                egui::Grid::new("cache").num_columns(2).show(ui, |ui| {
                    ui.strong("占用");
                    ui.label(format!(
                        "{} / {}",
                        human_bytes(stats.bytes),
                        human_bytes(stats.limit_bytes)
                    ));
                    ui.end_row();
                    ui.strong("命中率");
                    ui.label(format!(
                        "{:.1}%（{}/{}）",
                        hit_rate * 100.0,
                        stats.hits,
                        total
                    ));
                    ui.end_row();
                    ui.strong("写入");
                    ui.label(format!("{}", stats.inserts));
                    ui.end_row();
                    ui.strong("淘汰");
                    ui.label(format!("{}", stats.evictions));
                    ui.end_row();
                });
                let ratio = if stats.limit_bytes > 0 {
                    stats.bytes as f32 / stats.limit_bytes as f32
                } else {
                    0.0
                };
                ui.add_space(8.0);
                ui.add(egui::ProgressBar::new(ratio).text(human_bytes(stats.bytes)));

                ui.add_space(12.0);
                if ui.button("一键清理").clicked() {
                    let _ = engine.lock().unwrap().cache().clear();
                }
            }
        } else {
            ui.weak("引擎未运行（未配置同步根或未登录）");
        }
    }

    fn settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("同步控制");
        ui.separator();
        let paused = self.state.paused.load(Ordering::SeqCst);
        if ui
            .button(if paused { "▶ 恢复同步" } else { "⏸ 暂停同步" })
            .clicked()
        {
            let was = self.state.paused.fetch_xor(true, Ordering::SeqCst);
            if let Ok(mut s) = self.state.status.lock() {
                s.paused = !was;
            }
        }

        ui.add_space(16.0);
        ui.heading("同步根目录");
        ui.separator();
        let root = self.state.sync_root.lock().unwrap().clone();
        ui.label(
            root.as_ref()
                .map(|r| r.display().to_string())
                .unwrap_or_else(|| "（未设置）".into()),
        );
        if ui.button("选择目录…").clicked() {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                *self.state.sync_root.lock().unwrap() = Some(dir.clone());
                // 持久化到配置文件
                if let Ok(mut cfg) = cloudmount_core::config::load() {
                    cfg.sync_root = Some(dir.display().to_string());
                    let _ = cloudmount_core::config::save(&cfg);
                }
            }
        }

        ui.add_space(16.0);
        ui.heading("日志");
        ui.separator();
        if ui.button("导出日志…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name("cloudmount-log.txt")
                .save_file()
            {
                let lines = self.state.logs.snapshot();
                let _ = std::fs::write(&path, lines.join("\n"));
            }
        }
        ui.weak(format!("最近 {} 行（INFO 及以上）", self.state.logs.snapshot().len()));
    }

    fn login_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("登录（Cookie 补充通道）");
        ui.weak("该通道违反 115 服务协议（可能封号/互踢）——详见 README 风险声明");
        ui.separator();

        if self.login_polling {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("等待手机确认…");
            });
            if ui.button("取消").clicked() {
                self.login_polling = false;
                self.login_challenge = None;
                self.qr_texture = None;
            }
        } else if ui.button("开始扫码登录").clicked() {
            let auth = self.login_auth();
            match auth.begin_login() {
                Ok(challenge) => {
                    // 二维码 → egui 纹理
                    let qr = qrcode::QrCode::new(challenge.qr_content.as_bytes())
                        .expect("二维码编码");
                    let size = qr.width();
                    let colors = qr.to_colors();
                    let mut pixels = Vec::with_capacity(size * size);
                    for c in &colors {
                        pixels.push(match c {
                            qrcode::Color::Dark => egui::Color32::BLACK,
                            qrcode::Color::Light => egui::Color32::WHITE,
                        });
                    }
                    let image = egui::ColorImage {
                        size: [size, size],
                        pixels,
                    };
                    let texture =
                        ctx.load_texture("login-qr", image, egui::TextureOptions::LINEAR);
                    self.qr_texture = Some(texture);
                    self.login_challenge = Some(challenge);
                    self.login_polling = true;
                }
                Err(e) => {
                    if let Ok(mut s) = self.state.status.lock() {
                        s.last_message = format!("发起登录失败: {e}");
                    }
                }
            }
        }

        if let Some(texture) = &self.qr_texture {
            ui.add_space(8.0);
            // 白底（深色主题下二维码可扫）
            egui::Frame::new()
                .fill(egui::Color32::WHITE)
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(280.0, 280.0)));
                });
        }
    }
}

/// 引擎线程（与 daemon 同构：配置文件 → Provider → SyncEngine 循环）。
fn engine_loop(state: Arc<AppState>) {
    let cfg = cloudmount_core::config::load_effective().unwrap_or_default();
    let app_id = cfg.app_id.clone();
    let sync_root = cfg.sync_root.clone().map(std::path::PathBuf::from);

    let Some(sync_root) = sync_root else {
        if let Ok(mut s) = state.status.lock() {
            s.last_message = "未配置同步根（设置页选择目录）".into();
        }
        return;
    };
    if let Ok(mut s) = state.status.lock() {
        s.syncing = true;
    }

    let provider: Option<Arc<dyn cloudmount_core::Provider>> = app_id.and_then(|id| {
        let store = cloudmount_auth::store::default_store().ok()?;
        let auth = Arc::new(cloudmount_auth::open::OpenApiAuth::new(id, store));
        if !auth.is_logged_in() {
            if let Ok(mut s) = state.status.lock() {
                s.logged_in = false;
                s.last_message = "未登录：请在登录页扫码".into();
            }
            return None;
        }
        if let Ok(mut s) = state.status.lock() {
            s.logged_in = true;
        }
        Some(Arc::new(cloudmount_driver115::Open115Provider::new(auth))
            as Arc<dyn cloudmount_core::Provider>)
    });

    let Some(provider) = provider else {
        return;
    };

    let mut engine_cfg = cloudmount_sync::SyncEngineConfig::new(&sync_root);
    engine_cfg.scheduler.paused = state.paused.clone();
    let Ok(engine) = cloudmount_sync::SyncEngine::new(engine_cfg, provider) else {
        if let Ok(mut s) = state.status.lock() {
            s.last_message = "引擎初始化失败".into();
        }
        return;
    };
    let engine = Arc::new(Mutex::new(engine));
    if let Ok(mut slot) = state.engine.lock() {
        *slot = Some(engine.clone());
    }

    loop {
        if state.paused.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        }
        let result = engine.lock().map(|mut e| e.run_once());
        match result {
            Ok(Ok(stats)) => {
                if let Ok(mut s) = state.status.lock() {
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

/// 字节数人性化。
fn human_bytes(n: u64) -> String {
    if n < 1024 {
        return format!("{n} B");
    }
    let units = ["KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64 / 1024.0;
    let mut i = 0;
    while v >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{:.2} {}", v, units[i])
}

fn main() -> eframe::Result<()> {
    let logs = Arc::new(LogBuffer::new(2000));
    let writer = SharedWriter { buffer: logs.clone() };
    tracing_subscriber::fmt()
        .with_writer(std::sync::Mutex::new(writer))
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let state = Arc::new(AppState {
        paused: Arc::new(AtomicBool::new(false)),
        status: Arc::new(Mutex::new(StatusSnapshot {
            version: cloudmount_core::VERSION.to_string(),
            ..Default::default()
        })),
        sync_root: Mutex::new(
            cloudmount_core::config::load_effective()
                .ok()
                .and_then(|c| c.sync_root.map(std::path::PathBuf::from)),
        ),
        engine: Arc::new(Mutex::new(None)),
        logs,
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([520.0, 560.0])
            .with_min_inner_size([400.0, 400.0])
            .with_title("CloudMount 云挂"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "CloudMount 云挂",
        options,
        Box::new(move |cc| Ok(Box::new(CloudMountApp::new(cc, state)))),
    )
}
