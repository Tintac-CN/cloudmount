//! # CloudMount 桌面应用（纯 Rust，ArtCraft 风格）
//!
//! egui + wgpu + rfd——**无 WebView、无 HTML/JS**。UI 参考
//! storytold/artcraft 的 brutalist 暗色系统（近黑表面、发丝线、
//! 3px 圆角、HUD 等宽标签、左侧导航栏）。
//!
//! 布局：左侧导航栏（chrome 底色）+ 右侧内容面板（BG 底色）。
//!
//! 引擎线程与 daemon 同构：`Arc<Mutex<SyncEngine>>` 句柄槽 +
//! `AtomicBool` 暂停 + 日志环形缓冲。

mod theme;
mod widgets;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cloudmount_auth::Authenticator as _;
use eframe::egui;
use theme::*;

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
    engine: Arc<Mutex<Option<Arc<Mutex<cloudmount_sync::SyncEngine>>>>>,
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
enum Page {
    Status,
    Cache,
    Settings,
    Login,
}

impl Page {
    fn label(self) -> &'static str {
        match self {
            Page::Status => "状态",
            Page::Cache => "缓存",
            Page::Settings => "设置",
            Page::Login => "登录",
        }
    }
}

/// 侧边栏宽度。
const SIDEBAR_W: f32 = 180.0;

/// egui 应用。
struct CloudMountApp {
    state: Arc<AppState>,
    page: Page,
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
    fn new(cc: &eframe::CreationContext<'_>, state: Arc<AppState>) -> Self {
        theme::apply(&cc.egui_ctx);
        Self {
            state,
            page: Page::Status,
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

        // ---- 左侧导航栏（chrome 底色，同 artcraft） ----
        let chrome = egui::Frame::NONE.fill(CHROME);
        egui::SidePanel::left("sidebar")
            .exact_width(SIDEBAR_W)
            .resizable(false)
            .frame(chrome)
            .show(ctx, |ui| {
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.heading("CloudMount");
                });
                let ver = self.state.status.lock().map(|s| s.version.clone()).unwrap_or_default();
                ui.colored_label(FAINT, format!("云挂 · v{ver}"));
                ui.add_space(16.0);

                // 导航
                ui.spacing_mut().item_spacing.y = 2.0;
                let pages = [Page::Status, Page::Cache, Page::Settings, Page::Login];
                for p in pages {
                    if widgets::nav_row(ui, p.label(), self.page == p) {
                        self.page = p;
                    }
                }

                // 底部状态指示（同步状态灯）
                let bottom = egui::Rect::from_min_max(
                    egui::pos2(12.0, ui.max_rect().bottom() - 60.0),
                    egui::pos2(ui.max_rect().right() - 12.0, ui.max_rect().bottom() - 12.0),
                );
                let s = self.state.status.lock().map(|s| s.clone()).unwrap_or_default();
                let (color, label) = if s.paused {
                    (BADGE_BETA, "已暂停")
                } else if s.syncing {
                    (ACCENT_INK, "同步中")
                } else {
                    (MUTED, "空闲")
                };
                let p = ui.painter();
                p.rect(bottom, RADIUS, WASH, hairline(), egui::StrokeKind::Inside);
                p.circle_filled(
                    egui::pos2(bottom.left() + 16.0, bottom.center().y),
                    4.0,
                    color,
                );
                p.text(
                    egui::pos2(bottom.left() + 28.0, bottom.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::new(13.0, egui::FontFamily::Proportional),
                    INK,
                );
            });

        // ---- 右侧内容面板（BG 底色） ----
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BG).inner_margin(egui::Margin::same(24)))
            .show(ctx, |ui| {
                match self.page {
                    Page::Status => self.status_page(ui),
                    Page::Cache => self.cache_page(ui),
                    Page::Settings => self.settings_page(ui),
                    Page::Login => self.login_page(ui, ctx),
                }
            });

        // 登录轮询（每帧检查——短轮询立即返回）
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
                    Ok(None) => {}
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

        // 状态刷新节奏
        ctx.request_repaint_after(if self.login_polling {
            Duration::from_millis(500)
        } else {
            Duration::from_secs(1)
        });
    }
}

// ============ 页面 ============

impl CloudMountApp {
    fn status_page(&self, ui: &mut egui::Ui) {
        let s = self.state.status.lock().map(|s| s.clone()).unwrap_or_default();
        widgets::hud(ui, "Status", FAINT);
        ui.add_space(8.0);
        ui.heading("状态");
        ui.add_space(12.0);

        widgets::kv_row(ui, "版本", &s.version);
        widgets::kv_row(ui, "登录", if s.logged_in { "已登录" } else { "未登录" });
        widgets::kv_row(
            ui,
            "同步",
            if s.paused {
                "已暂停"
            } else if s.syncing {
                "进行中"
            } else {
                "空闲"
            },
        );
        widgets::kv_row(ui, "累计上传", &format!("{}", s.total_uploaded));
        widgets::kv_row(ui, "累计下载", &format!("{}", s.total_downloaded));
        widgets::kv_row(ui, "冲突", &format!("{}", s.conflicts));
        widgets::kv_row(ui, "错误", &format!("{}", s.errors));

        if !s.last_message.is_empty() {
            ui.add_space(12.0);
            ui.colored_label(MUTED, format!("最近：{}", s.last_message));
        }

        // 配额（引擎可用时）
        if let Some(engine) = self.engine() {
            if let Ok(q) = engine.lock().unwrap().provider().quota() {
                if q.total > 0 {
                    ui.add_space(20.0);
                    widgets::hud(ui, "Cloud Quota", FAINT);
                    ui.add_space(8.0);
                    let ratio = q.used as f32 / q.total as f32;
                    widgets::progress(
                        ui,
                        ratio,
                        &format!("{}/{}", human_bytes(q.used), human_bytes(q.total)),
                    );
                }
            }
        }
    }

    fn cache_page(&self, ui: &mut egui::Ui) {
        widgets::hud(ui, "Cache", FAINT);
        ui.add_space(8.0);
        ui.heading("内容缓存");
        ui.add_space(12.0);

        if let Some(engine) = self.engine() {
            if let Ok(stats) = engine.lock().unwrap().cache_stats() {
                let total = stats.hits + stats.misses;
                let hit_rate = if total > 0 {
                    stats.hits as f32 / total as f32
                } else {
                    0.0
                };
                widgets::kv_row(
                    ui,
                    "占用",
                    &format!("{}/{}", human_bytes(stats.bytes), human_bytes(stats.limit_bytes)),
                );
                widgets::kv_row(
                    ui,
                    "命中率",
                    &format!("{:.1}%（{}/{}）", hit_rate * 100.0, stats.hits, total),
                );
                widgets::kv_row(ui, "写入", &format!("{}", stats.inserts));
                widgets::kv_row(ui, "淘汰", &format!("{}", stats.evictions));

                ui.add_space(12.0);
                let ratio = if stats.limit_bytes > 0 {
                    stats.bytes as f32 / stats.limit_bytes as f32
                } else {
                    0.0
                };
                widgets::progress(ui, ratio, &human_bytes(stats.bytes));

                ui.add_space(16.0);
                if widgets::button(ui, "一键清理", widgets::Kind::Secondary, 120.0, 36.0)
                    .clicked()
                {
                    let _ = engine.lock().unwrap().cache().clear();
                }
            }
        } else {
            ui.colored_label(MUTED, "引擎未运行（未配置同步根或未登录）");
        }
    }

    fn settings_page(&mut self, ui: &mut egui::Ui) {
        widgets::hud(ui, "Settings", FAINT);
        ui.add_space(8.0);
        ui.heading("设置");
        ui.add_space(12.0);

        // 同步控制
        widgets::section_label(ui, "Sync", SIDEBAR_INK);
        ui.add_space(4.0);
        let paused = self.state.paused.load(Ordering::SeqCst);
        if widgets::button(
            ui,
            if paused { "恢复同步" } else { "暂停同步" },
            widgets::Kind::Primary,
            140.0,
            36.0,
        )
        .clicked()
        {
            let was = self.state.paused.fetch_xor(true, Ordering::SeqCst);
            if let Ok(mut s) = self.state.status.lock() {
                s.paused = !was;
            }
        }

        ui.add_space(20.0);

        // 同步根
        widgets::section_label(ui, "Root", SIDEBAR_INK);
        ui.add_space(4.0);
        let root = self.state.sync_root.lock().unwrap().clone();
        ui.colored_label(
            INK,
            root.as_ref()
                .map(|r| r.display().to_string())
                .unwrap_or_else(|| "（未设置）".into()),
        );
        ui.add_space(4.0);
        if widgets::button(ui, "选择目录", widgets::Kind::Secondary, 120.0, 36.0).clicked() {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                *self.state.sync_root.lock().unwrap() = Some(dir.clone());
                if let Ok(mut cfg) = cloudmount_core::config::load() {
                    cfg.sync_root = Some(dir.display().to_string());
                    let _ = cloudmount_core::config::save(&cfg);
                }
            }
        }

        ui.add_space(20.0);

        // 日志
        widgets::section_label(ui, "Logs", SIDEBAR_INK);
        ui.add_space(4.0);
        if widgets::button(ui, "导出日志", widgets::Kind::Secondary, 120.0, 36.0).clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name("cloudmount-log.txt")
                .save_file()
            {
                let lines = self.state.logs.snapshot();
                let _ = std::fs::write(&path, lines.join("\n"));
            }
        }
        ui.colored_label(
            FAINT,
            format!("最近 {} 行（INFO 及以上）", self.state.logs.snapshot().len()),
        );
    }

    fn login_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        widgets::hud(ui, "Login", FAINT);
        ui.add_space(8.0);
        ui.heading("登录");
        ui.add_space(4.0);
        ui.colored_label(
            MUTED,
            "Cookie 补充通道——违反 115 服务协议（详见 README）",
        );
        ui.add_space(16.0);

        if self.login_polling {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.colored_label(INK, "等待手机确认…");
            });
            ui.add_space(8.0);
            if widgets::button(ui, "取消", widgets::Kind::Secondary, 100.0, 32.0).clicked() {
                self.login_polling = false;
                self.login_challenge = None;
                self.qr_texture = None;
            }
        } else if widgets::button(ui, "开始扫码登录", widgets::Kind::Accent, 180.0, 40.0)
            .clicked()
        {
            let auth = self.login_auth();
            match auth.begin_login() {
                Ok(challenge) => {
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
            ui.add_space(16.0);
            // 白底 Frame（深色主题下二维码可扫）
            egui::Frame::new()
                .fill(egui::Color32::WHITE)
                .inner_margin(egui::Margin::same(16))
                .corner_radius(4)
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
            .with_inner_size([720.0, 560.0])
            .with_min_inner_size([560.0, 420.0])
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
