//! # CloudMount 桌面应用（纯 Rust，ArtCraft 风格）
//!
//! egui + wgpu + rfd——**无 WebView、无 HTML/JS**。UI 参考
//! storytold/artcraft 的 brutalist 暗色系统（近黑表面、发丝线、
//! 3px 圆角、HUD 等宽标签、左侧导航栏）。
//!
//! 布局：左侧导航栏（chrome 底色）+ 右侧内容面板（BG 底色）。
//!
//! 系统托盘：tray-icon 0.21，菜单恰两项「显示主窗口/退出」，关闭按钮=隐藏到托盘
//! （Q1 裁决）——集成形态与实测约束（含「伪隐藏」架构成因）见下方「系统托盘」段注释。
//!
//! 引擎线程与 daemon 同构：`Arc<Mutex<SyncEngine>>` 句柄槽 +
//! `AtomicBool` 暂停 + 日志环形缓冲。

mod theme;
mod widgets;

// gen_icons.py 生成的品牌 RGBA 常量（32×32；tray-icon `Icon::from_rgba` /
// egui `IconData` 消费——零 crate 依赖解码链，gate1 r2 裁决）。
include!("../icons/icons.rs");

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cloudmount_auth::Authenticator as _;
use eframe::egui;
#[cfg(windows)]
use raw_window_handle::HasWindowHandle;
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
    /// 登录页同意勾选框（F-05：初值取配置 `[cookie].consent_acknowledged`——
    /// 已同意过的用户不重复打扰；未勾选时登录按钮禁用）。
    consent_checked: bool,
    /// 会话级 CookieAuth（F-05：点击登录时构造一次，`update()` 轮询复用——
    /// 消除现状每帧重建的低效）。
    login_auth_session: Option<cloudmount_auth::cookie::CookieAuth>,
    /// 引擎线程（首帧启动）。
    engine_thread: Option<std::thread::JoinHandle<()>>,
    /// 托盘句柄（None = 未初始化或初始化失败降级；保活即托盘常驻——drop 即消失）。
    tray: Option<tray_icon::TrayIcon>,
    /// 托盘是否已尝试初始化（首帧一次；失败降级不重试）。
    tray_init_done: bool,
    /// 托盘「退出」已请求：关闭流程不再取消（真退出）。
    quit_requested: bool,
    /// 伪隐藏前的窗口位置（Some = 当前处于托盘隐藏态；「显示主窗口」据此复位）。
    restore_pos: Option<egui::Pos2>,
}

impl CloudMountApp {
    fn new(cc: &eframe::CreationContext<'_>, state: Arc<AppState>) -> Self {
        theme::apply(&cc.egui_ctx);
        let consent_checked = cloudmount_core::config::load()
            .map(|c| c.cookie.consent_acknowledged)
            .unwrap_or(false);
        Self {
            state,
            page: Page::Status,
            qr_texture: None,
            login_challenge: None,
            login_polling: false,
            consent_checked,
            login_auth_session: None,
            engine_thread: None,
            tray: None,
            tray_init_done: false,
            quit_requested: false,
            restore_pos: None,
        }
    }

    fn engine(&self) -> Option<Arc<Mutex<cloudmount_sync::SyncEngine>>> {
        self.state.engine.lock().ok().and_then(|g| g.clone())
    }
}

impl eframe::App for CloudMountApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
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

        // ---- 系统托盘（AC2；首帧惰性初始化——主线程 + 事件循环已运行，macOS 约束满足） ----
        if !self.tray_init_done {
            self.tray_init_done = true;
            self.tray = init_tray();
        }

        // 托盘菜单事件轮询（全局 receiver；恰两项）。帧节奏驱动——伪隐藏下帧流不断
        // （见「系统托盘」段实测注释），故与既有 1s request_repaint_after 兼容。
        while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            match event.id.as_ref() {
                "show" => {
                    if let Some(pos) = self.restore_pos.take() {
                        platform_set_tool_window(frame, false);
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
                        tracing::info!("托盘「显示主窗口」：窗口复位");
                    }
                    // 可见态亦聚焦唤前（AC2②「显示主窗口」=唤起/聚焦）
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                "quit" => {
                    self.quit_requested = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    tracing::info!("托盘「退出」：请求终止进程");
                }
                _ => {}
            }
        }

        // 关闭语义（Q1 裁决）：默认拦截 + 隐藏到托盘（伪隐藏——见段注释）；托盘「退出」
        // 已请求 → 放行（真退出）；托盘不可用（初始化失败降级）→ 放行（无托盘可恢复）。
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.quit_requested || self.tray.is_none() {
                tracing::info!(
                    quit = self.quit_requested,
                    tray = self.tray.is_some(),
                    "关闭请求：放行（真退出）"
                );
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                let pos = ctx
                    .input(|i| i.viewport().outer_rect)
                    .map(|r| r.min)
                    .unwrap_or(egui::pos2(120.0, 120.0));
                self.restore_pos = Some(pos);
                platform_set_tool_window(frame, true);
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
                    HIDE_POS, HIDE_POS,
                )));
                tracing::info!("关闭请求：已拦截——隐藏到托盘（托盘「显示主窗口」复位/「退出」真退）");
            }
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

        // 登录轮询（每帧检查——短轮询立即返回；复用会话级 auth，不再每帧重建）
        if self.login_polling {
            let poll = match (&self.login_challenge, self.login_auth_session.as_ref()) {
                (Some(challenge), Some(auth)) => Some(auth.poll_login(challenge)),
                _ => None,
            };
            if let Some(result) = poll {
                match result {
                    Ok(Some(_)) => {
                        self.login_polling = false;
                        self.login_challenge = None;
                        self.qr_texture = None;
                        self.login_auth_session = None;
                        if let Ok(mut s) = self.state.status.lock() {
                            s.logged_in = true;
                            s.last_message = "登录成功（Cookie 已入独立容器）".into();
                        }
                        // F-05：登录成功写回 `[cookie]`（同意持久化闭环；经
                        // core 纯函数 + 原子写。desktop 接线归人工清单⑦）
                        persist_cookie_consent();
                    }
                    Ok(None) => {}
                    Err(e) => {
                        self.login_polling = false;
                        self.login_challenge = None;
                        self.qr_texture = None;
                        self.login_auth_session = None;
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
        // 风险声明完整版（对齐 CLI login --with-cookie-channel 四条）
        ui.colored_label(
            MUTED,
            "⚠️  Cookie 补充通道风险声明（务必阅读）：\n  \
             1. 该方式违反《115 生活用户服务协议》相关条款\n     \
             （非授权客户端/接口、反向工程）；\n  \
             2. 可能导致账号冻结、限速、数据访问受限；\n  \
             3. 扫码登录会与其它设备会话互踢；\n  \
             4. 无稳定性承诺，115 可随时变更协议导致失效。\n\
             官方 OpenAPI 已覆盖同步盘全部核心能力（申请入口 https://open.115.com/ ）。",
        );
        ui.add_space(12.0);
        // 知情同意勾选框（CLI「输入 yes」的 GUI 等价物；初值取配置 [cookie]——
        // 同意仅由登录交互产生并持久化，引擎/组装是消费者、不自产同意）
        ui.checkbox(
            &mut self.consent_checked,
            "我已阅读并知悉上述风险，同意启用 Cookie 通道",
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
                self.login_auth_session = None;
            }
        } else if !self.consent_checked {
            // 未勾选同意：登录按钮禁用（同意是登录的前置交互）
            ui.disable();
            widgets::button(ui, "开始扫码登录", widgets::Kind::Secondary, 180.0, 40.0);
        } else if widgets::button(ui, "开始扫码登录", widgets::Kind::Accent, 180.0, 40.0)
            .clicked()
        {
            // 会话级 CookieAuth：一次构造、轮询复用（消除每帧重建）
            let policy = cookie_policy_from_login_state(self.consent_checked);
            let store = cloudmount_auth::store::default_store().expect("凭据存储");
            let auth = cloudmount_auth::cookie::CookieAuth::new(policy, store);
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
                    self.login_auth_session = Some(auth);
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

// ============ 系统托盘（AC2：tray-icon 0.21 + Q1 关闭=隐藏到托盘） ============
//
// 集成形态（首日验证① 定稿；配套探针实证见 U2 提交信息）：
//   - 初始化落点 = 首次 update()：主线程且事件循环已运行——满足 tray-icon 对 macOS
//     「主线程 + 事件循环运行中」的硬约束（Windows 亦要求与事件循环同线程）。
//   - 菜单事件轮询 = update() 内全局 receiver 轮询（`MenuEvent::receiver()` + `try_recv`），
//     与既有 1s request_repaint_after 帧节奏兼容。
//   - 关闭按钮 = 拦截 +「伪隐藏」（Q1 语义：窗口隐藏、托盘常驻）：
//     `ViewportCommand::CancelClose`（首日验证④：eframe 0.31 API 实证可行）+ 窗口移出
//     屏幕（`OuterPosition`）+ `WS_EX_TOOLWINDOW`（Windows：去任务栏/Alt-Tab；macOS 无
//     此步——Dock 图标留存为已知差异，记入 U2 回报）。
//   - 为何不用 `ViewportCommand::Visible(false)`（实测缺陷所迫，两项独立探针）：
//     ① 真隐藏后窗口不再收到 WM_PAINT（winit 源码注释佐证：WM_PAINT 投递要求窗口可见，
//        隐藏窗口帧饥饿）→ eframe 停发 RedrawRequested → update() 停帧——菜单轮询与
//        恢复命令全部失联；
//     ② 即使改以专用线程轮询菜单（绕开①），最后一个重绘到期后 eframe 事件循环进入
//        `ControlFlow::Poll`，且 egui 重绘请求在无帧期被抑制（delay 不递减不回调）——
//        实测隐藏态 CPU 100% 单核自旋，防自旋补请求无效。
//     「伪隐藏」保持 WS_VISIBLE（帧流不断、无自旋），用户不可见由离屏 + TOOLWINDOW 达成；
//     恢复 = 位置复位 + 去 TOOLWINDOW + Focus。语义与 AC2④ 一致（窗口隐藏、托盘常驻、
//     托盘「退出」真退出/关闭按钮隐藏），仅隐藏物理形态不同（实测缺陷所迫，回报留痕）。
//   - 托盘初始化失败 → 降级无托盘运行（关闭即退出，不 panic；prd AC2 边界）。

/// 伪隐藏位置（远离任何桌面区域/多显示器布局）。
const HIDE_POS: f32 = -32000.0;

/// Windows 直连 FFI（user32——伪隐藏去任务栏所需最小面，零新 crate 依赖）。
#[cfg(windows)]
mod win32 {
    use std::ffi::c_void;
    pub const GWL_EXSTYLE: i32 = -20;
    pub const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
    pub const SWP_NOSIZE: u32 = 0x0001;
    pub const SWP_NOMOVE: u32 = 0x0002;
    pub const SWP_NOZORDER: u32 = 0x0004;
    pub const SWP_NOACTIVATE: u32 = 0x0010;
    pub const SWP_FRAMECHANGED: u32 = 0x0020;
    #[link(name = "user32")]
    extern "system" {
        pub fn GetWindowLongPtrW(hwnd: *mut c_void, index: i32) -> isize;
        pub fn SetWindowLongPtrW(hwnd: *mut c_void, index: i32, value: isize) -> isize;
        pub fn SetWindowPos(
            hwnd: *mut c_void,
            insert_after: *mut c_void,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
    }
}

/// 伪隐藏辅助：切换 WS_EX_TOOLWINDOW（离屏后去/回任务栏与 Alt-Tab）。
/// 返回是否已执行（无句柄/非 Win32 = false——egui 离屏移动仍跨平台生效）。
#[cfg(windows)]
fn platform_set_tool_window(frame: &eframe::Frame, on: bool) -> bool {
    let Ok(handle) = frame.window_handle() else {
        return false;
    };
    let raw_window_handle::RawWindowHandle::Win32(w) = handle.as_raw() else {
        return false;
    };
    let hwnd = w.hwnd.get() as *mut std::ffi::c_void;
    unsafe {
        let ex = win32::GetWindowLongPtrW(hwnd, win32::GWL_EXSTYLE);
        let new_ex = if on {
            ex | win32::WS_EX_TOOLWINDOW
        } else {
            ex & !win32::WS_EX_TOOLWINDOW
        };
        win32::SetWindowLongPtrW(hwnd, win32::GWL_EXSTYLE, new_ex);
        win32::SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            win32::SWP_NOMOVE
                | win32::SWP_NOSIZE
                | win32::SWP_NOZORDER
                | win32::SWP_NOACTIVATE
                | win32::SWP_FRAMECHANGED,
        );
    }
    true
}

/// 非 Windows：无 TOOLWINDOW 概念（离屏移动由 egui `OuterPosition` 跨平台完成）。
#[cfg(not(windows))]
fn platform_set_tool_window(_frame: &eframe::Frame, _on: bool) -> bool {
    false
}

/// 托盘初始化（失败返回 None——降级无托盘运行，不 panic；prd AC2 边界）：
/// 菜单恰两项「显示主窗口」/「退出」（id=show/quit，不多不少）；图标 = icons.rs 同源。
fn init_tray() -> Option<tray_icon::TrayIcon> {
    let show = tray_icon::menu::MenuItem::with_id("show", "显示主窗口", true, None);
    let quit = tray_icon::menu::MenuItem::with_id("quit", "退出", true, None);
    let menu = tray_icon::menu::Menu::new();
    if let Err(e) = menu.append(&show).and_then(|()| menu.append(&quit)) {
        tracing::warn!(error = %e, "托盘菜单构建失败——降级无托盘运行");
        return None;
    }
    let icon =
        match tray_icon::Icon::from_rgba(ICON_32_RGBA.to_vec(), ICON_32_WIDTH, ICON_32_HEIGHT) {
            Ok(icon) => icon,
            Err(e) => {
                tracing::warn!(error = %e, "托盘图标解码失败（icons.rs）——降级无托盘运行");
                return None;
            }
        };
    match tray_icon::TrayIconBuilder::new()
        .with_tooltip("CloudMount 云挂")
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .build()
    {
        Ok(tray) => Some(tray),
        Err(e) => {
            tracing::warn!(error = %e, "托盘初始化失败——降级无托盘运行（应用主体可用）");
            None
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
            .with_title("CloudMount 云挂")
            // 窗口/任务栏图标：icons.rs 同源（gen_icons 单一管线，AC1）
            .with_icon(egui::IconData {
                rgba: ICON_32_RGBA.to_vec(),
                width: ICON_32_WIDTH,
                height: ICON_32_HEIGHT,
            }),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "CloudMount 云挂",
        options,
        Box::new(move |cc| Ok(Box::new(CloudMountApp::new(cc, state)))),
    )
}

/// F-05：登录成功写回 `[cookie]`（同意持久化闭环；desktop 接线归人工清单⑦）。
///
/// `config::load` → core 纯函数 [`set_cookie_consent`] → `config::save`
/// （内部走 save_to 原子写：tmp + rename）。load 失败不再静默跳过（warn：
/// 同意记录丢失，下次启动需重新同意）。与设置页 `sync_root` 写回同构
/// （文件内既有先例）。
fn persist_cookie_consent() {
    match cloudmount_core::config::load() {
        Ok(mut cfg) => {
            cloudmount_core::config::set_cookie_consent(&mut cfg); // core 纯函数：cookie 双字段置真
            if let Err(e) = cloudmount_core::config::save(&cfg) {
                tracing::warn!(error = %e, "Cookie 同意标志持久化失败（下次启动需重新同意）");
            }
        }
        Err(e) => tracing::warn!(
            error = %e,
            "登录成功但 config::load 失败，Cookie 同意标志未写回（下次启动需重新同意）"
        ),
    }
}

/// T-28：登录态 → CookiePolicy（Gate#1 r2 人类裁决口径）。
///
/// 登录页勾选「同意启用 Cookie 通道」后点击"开始扫码登录"时构造：
/// enabled=true 恒真（点击即显式开启）、consent_acknowledged=勾选框状态。
/// 配置 [cookie] 不参与 policy 构造（仅驱动勾选框初值与登录成功后的写回）——
/// 否则全新安装无 [cookie] 段时 `check_enabled` 首行即拒绝，扫码登录闭环死锁。
// 点击即同意：登录会话构造点（T-30 豁免面标注；引擎/组装路径一律经配置）
fn cookie_policy_from_login_state(checked: bool) -> cloudmount_auth::cookie::CookiePolicy {
    // 点击即同意：enabled=true 恒真、consent_acknowledged=勾选状态
    // （勾选框未勾选时按钮禁用，可点击即蕴含已勾选）。
    cloudmount_auth::cookie::CookiePolicy {
        enabled: true,
        consent_acknowledged: checked,
    }
}

/// T-28：`cookie_policy_from_login_state` 纯函数单测（本地 `cargo test -p
/// cloudmount-desktop` 跑；desktop 被 workspace exclude，CI 不可见——
/// 接线归人工清单⑦兜底）。
#[cfg(test)]
mod t28_policy_tests {
    use super::*;

    /// 勾选同意 → 双真（显式开启 + 知情同意，与 CLI/daemon 组装语义一致）。
    #[test]
    fn checked_is_double_true() {
        let p = cookie_policy_from_login_state(true);
        assert!(p.enabled, "enabled 恒真（点击即显式开启）");
        assert!(p.consent_acknowledged, "勾选后必须为同意");
        p.check_enabled().expect("勾选后必须通过 check_enabled");
    }

    /// 未勾选 → {true, false}——`check_enabled` 拒绝（与 CLI/daemon 一致）。
    #[test]
    fn unchecked_is_rejected() {
        let p = cookie_policy_from_login_state(false);
        assert!(p.enabled, "enabled 恒真");
        assert!(!p.consent_acknowledged, "未勾选不得视为同意");
        assert!(
            p.check_enabled().is_err(),
            "未勾选同意必须被 check_enabled 拒绝"
        );
    }
}

/// T-30：结构性断言（AC16；组装/引擎路径硬编码双真零命中 + 登录构造点注释）。
#[cfg(test)]
mod t30_structural {
    /// 断言对象 = 本文件源码。
    const SOURCE: &str = include_str!("main.rs");

    /// 组装/引擎路径零命中：desktop 源码不允许出现「consent_acknowledged
    /// 字段硬编码置真」的 CookiePolicy 字面量构造（登录会话构造点取勾选
    /// 状态、非字面量）——防硬编码双真绕过"配置驱动勾选 + 点击构造 + 写回"
    /// 回潮。needle 经 format! 动态拼接，避免本测试源码自身命中。
    #[test]
    fn no_hardcoded_double_true_policy_literal() {
        let needle = format!("consent_acknowledged: {}", "true");
        assert_eq!(
            SOURCE.matches(&needle).count(),
            0,
            "发现硬编码双真 CookiePolicy 字面量（组装必须经 cookie_policy_from_login_state / 配置）"
        );
    }

    /// 豁免处注释断言：登录会话构造点所在函数体必须含"点击即同意"注释串。
    #[test]
    fn login_construction_point_has_click_consent_comment() {
        let start = SOURCE
            .find("fn cookie_policy_from_login_state")
            .expect("登录会话构造函数必须存在（cookie_policy_from_login_state）");
        let body = &SOURCE[start..];
        let end = body
            .find("\n#[cfg(test)]")
            .expect("tests 模块必须紧随构造函数之后");
        assert!(
            body[..end].contains("点击即同意"),
            "登录会话构造点必须带「点击即同意」注释（T-30 豁免面标注）"
        );
    }
}
