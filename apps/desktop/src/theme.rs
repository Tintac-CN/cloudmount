//! ArtCraft 风格主题：brutalist 暗色系统。
//!
//! 移植自 storytold/artcraft 的 `theme.rs`（MIT）——近黑表面、
//! white/15 发丝线、3px 圆角、HUD 等宽标签。字体用系统栈
//! （Inter/Archivo/Geist Mono 需要字体文件，此处用 egui 默认 +
//! 等宽字体做 HUD 标签）。

use eframe::egui::{
    self as egui, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals,
};

const fn hex(rgb: u32) -> Color32 {
    let [_, r, g, b] = rgb.to_be_bytes();
    Color32::from_rgb(r, g, b)
}

const fn alpha(rgb: u32, a: u8) -> Color32 {
    let [_, r, g, b] = rgb.to_be_bytes();
    Color32::from_rgba_premultiplied(r, g, b, a)
}

/// 页面背景。
pub const BG: Color32 = hex(0x121316);
/// 面板（顶栏、模态）。
pub const PANEL: Color32 = hex(0x101014);
/// 控件（侧边栏、工具栏按钮）。
pub const CONTROLS: Color32 = hex(0x1e1f22);
/// 窗口铬（标题栏、侧边栏底）。
pub const CHROME: Color32 = hex(0x0a0a0c);
/// 凹陷（输入框、代码块）。
pub const SUNKEN: Color32 = hex(0x060607);

/// 正文。
pub const INK: Color32 = hex(0xf2f1ee);
/// 强调文本。
pub const INK_STRONG: Color32 = Color32::WHITE;
/// 次要文本。
pub const MUTED: Color32 = alpha(0xf2f1ee, 153);
/// 更弱的文本。
pub const FAINT: Color32 = alpha(0xf2f1ee, 102);
/// 侧边栏链接。
pub const SIDEBAR_INK: Color32 = alpha(0xffffff, 184);

/// 发丝线（white/15）。
pub const LINE: Color32 = alpha(0xffffff, 38);
/// 强发丝线（base-fg/40）。
pub const LINE_STRONG: Color32 = alpha(0xf2f1ee, 102);
/// 安静填充（white/5）。
pub const WASH: Color32 = alpha(0xffffff, 13);
/// 选中行（white/8）。
pub const WASH_SELECTED: Color32 = alpha(0xffffff, 20);
/// 悬停行（white/10）。
pub const WASH_HOVER: Color32 = alpha(0xffffff, 26);

/// 品牌蓝。
pub const ACCENT: Color32 = hex(0x2d81ff);
/// 品牌蓝 400（进度条、选中环）。
pub const ACCENT_400: Color32 = hex(0x59a7ff);
/// 蓝色文本。
pub const ACCENT_INK: Color32 = hex(0x74aaff);

/// 反色按钮（白底黑字）。
pub const INVERT_BG: Color32 = Color32::WHITE;
pub const INVERT_FG: Color32 = hex(0x0b0b0c);

/// 失败文本。
pub const BAD: Color32 = hex(0xf87171);
/// 危险填充。
/// 调色板令牌（预留：错误/危险态；当前 UI 未使用）。
#[allow(dead_code)]
pub const DANGER: Color32 = hex(0xd33242);
/// 徽章色。
pub const BADGE_BETA: Color32 = hex(0xffc14d);

/// 3px 圆角。
pub const RADIUS: CornerRadius = CornerRadius::same(3);

/// 控件高度。
pub const CONTROL_H: f32 = 34.0;
/// HUD 标签尺寸（11px 等宽、大写、宽字距）。
pub const HUD_SIZE: f32 = 11.0;
pub const HUD_SPACING: f32 = HUD_SIZE * 0.15;
/// 章节标签字距（0.12em）。
pub const LABEL_SPACING: f32 = HUD_SIZE * 0.12;

/// 发丝线描边。
pub fn hairline() -> Stroke {
    Stroke::new(1.0_f32, LINE)
}

/// 颜色按比例降透明度（0–1）。
pub fn fade(c: Color32, a: f32) -> Color32 {
    c.gamma_multiply(a.clamp(0.0, 1.0))
}

fn visuals() -> Visuals {
    let mut v = Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.extreme_bg_color = SUNKEN;
    v.faint_bg_color = CONTROLS;
    v.code_bg_color = SUNKEN;
    v.window_stroke = hairline();
    v.window_corner_radius = RADIUS;
    v.menu_corner_radius = RADIUS;
    v.override_text_color = Some(INK);
    v.hyperlink_color = ACCENT_INK;
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    v.warn_fg_color = BADGE_BETA;
    v.error_fg_color = BAD;
    v.striped = false;
    for (w, fill, stroke, fg) in [
        (&mut v.widgets.noninteractive, CONTROLS, LINE, MUTED),
        (&mut v.widgets.inactive, CONTROLS, LINE, INK),
        (&mut v.widgets.hovered, WASH_HOVER, LINE_STRONG, INK_STRONG),
        (&mut v.widgets.active, WASH_HOVER, LINE_STRONG, INK_STRONG),
        (&mut v.widgets.open, WASH_HOVER, LINE_STRONG, INK_STRONG),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0_f32, stroke);
        w.corner_radius = RADIUS;
        w.fg_stroke = Stroke::new(1.0_f32, fg);
        w.expansion = 0.0;
    }
    v
}

/// 安装主题（暗色 only，同 artcraft）。
pub fn apply(ctx: &egui::Context) {
    ctx.set_visuals_of(egui::Theme::Dark, visuals());
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size.y = CONTROL_H;
        style.spacing.window_margin = egui::Margin::same(16);
        style.text_styles = [
            (
                TextStyle::Heading,
                FontId::new(24.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(14.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(12.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(13.0, FontFamily::Monospace),
            ),
        ]
        .into();
    });
}
