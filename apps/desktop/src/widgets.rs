//! ArtCraft 风格控件：HUD 标签、侧边栏导航行、按钮、进度条。

use crate::theme;
use eframe::egui::{self as egui, Align2, Color32, FontFamily, FontId, Rect, Response, Sense, Stroke, Ui, pos2, vec2};

/// 文本大写 + 等宽 + 宽字距（HUD 风格）。
pub fn hud(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let galley = ui.painter().layout_job(egui::text::LayoutJob::single_section(
        text.to_uppercase(),
        egui::TextFormat {
            font_id: FontId::new(theme::HUD_SIZE, FontFamily::Monospace),
            color,
            extra_letter_spacing: theme::HUD_SPACING,
            ..Default::default()
        },
    ));
    let (rect, resp) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, color);
    resp
}

/// 章节标签（侧边栏分组标题）。
pub fn section_label(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let galley = ui.painter().layout_job(egui::text::LayoutJob::single_section(
        text.to_uppercase(),
        egui::TextFormat {
            font_id: FontId::new(theme::HUD_SIZE, FontFamily::Monospace),
            color,
            extra_letter_spacing: theme::LABEL_SPACING,
            ..Default::default()
        },
    ));
    let (rect, resp) = ui.allocate_exact_size(
        vec2(ui.available_width(), 32.0),
        Sense::hover(),
    );
    let pos = pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(pos, galley, color);
    resp
}

/// 侧边栏导航行（选中 = white/8 填充 + 左侧白色指示条）。
pub fn nav_row(ui: &mut Ui, label: &str, selected: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(
        vec2(ui.available_width(), 36.0),
        Sense::click(),
    );
    let hovered = resp.hovered();
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, theme::RADIUS, theme::WASH_SELECTED);
        p.rect_filled(
            Rect::from_min_size(pos2(rect.left() - 8.0, rect.top() + 9.0), vec2(2.0, 18.0)),
            0.0,
            Color32::WHITE,
        );
    } else if hovered {
        p.rect_filled(rect, theme::RADIUS, theme::WASH);
    }
    let ink = if selected || hovered {
        Color32::WHITE
    } else {
        theme::SIDEBAR_INK
    };
    p.text(
        pos2(rect.left() + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::new(14.0, FontFamily::Proportional),
        ink,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// 按钮类型。
pub enum Kind {
    /// 反色主按钮（白底黑字）。
    Primary,
    /// 品牌蓝。
    Accent,
    /// 透明底 + 发丝线。
    Secondary,
}

/// ArtCraft 风格按钮：HUD 等宽标签 + 3px 圆角。
pub fn button(ui: &mut Ui, text: &str, kind: Kind, width: f32, height: f32) -> Response {
    let enabled = ui.is_enabled();
    let galley = ui.painter().layout_job(egui::text::LayoutJob::single_section(
        text.to_uppercase(),
        egui::TextFormat {
            font_id: FontId::new(theme::HUD_SIZE, FontFamily::Monospace),
            color: theme::INK,
            extra_letter_spacing: theme::LABEL_SPACING,
            ..Default::default()
        },
    ));
    let size = vec2((galley.size().x + 40.0).max(width), height);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered() && enabled;
    let (fill, stroke, ink) = match kind {
        Kind::Primary => (
            theme::fade(theme::INVERT_BG, if hovered { 0.88 } else { 1.0 }),
            Stroke::NONE,
            theme::INVERT_FG,
        ),
        Kind::Accent => (
            if hovered { theme::ACCENT_400 } else { theme::ACCENT },
            Stroke::NONE,
            Color32::WHITE,
        ),
        Kind::Secondary if hovered => (theme::INVERT_BG, Stroke::NONE, theme::INVERT_FG),
        Kind::Secondary => (
            Color32::TRANSPARENT,
            Stroke::new(1.0_f32, theme::LINE_STRONG),
            theme::INK,
        ),
    };
    let dim = |c: Color32| if enabled { c } else { theme::fade(c, 0.5) };
    let p = ui.painter();
    p.rect(
        rect,
        theme::RADIUS,
        dim(fill),
        Stroke::new(stroke.width, dim(stroke.color)),
        egui::StrokeKind::Inside,
    );
    p.galley_with_override_text_color(
        pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        dim(ink),
    );
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, text));
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// 键值行（状态页用）：左标签右值，发丝线分隔。
pub fn kv_row(ui: &mut Ui, key: &str, value: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(
        vec2(ui.available_width(), 32.0),
        Sense::hover(),
    );
    let p = ui.painter();
    p.hline(rect.left()..=rect.right(), rect.bottom(), theme::hairline());
    p.text(
        pos2(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        key,
        FontId::new(13.0, FontFamily::Proportional),
        theme::MUTED,
    );
    p.text(
        pos2(rect.right(), rect.center().y),
        Align2::RIGHT_CENTER,
        value,
        FontId::new(14.0, FontFamily::Proportional),
        theme::INK,
    );
    resp
}

/// 进度条（品牌蓝填充 + 发丝线底 + HUD 文本）。
pub fn progress(ui: &mut Ui, ratio: f32, text: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(
        vec2(ui.available_width(), 24.0),
        Sense::hover(),
    );
    let p = ui.painter();
    p.rect(rect, theme::RADIUS, theme::SUNKEN, theme::hairline(), egui::StrokeKind::Inside);
    let fill = Rect::from_min_size(rect.min, vec2(rect.width() * ratio.clamp(0.0, 1.0), rect.height()));
    if fill.width() > 0.0 {
        p.rect_filled(fill, theme::RADIUS, theme::ACCENT);
    }
    let galley = p.layout_job(egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat {
            font_id: FontId::new(11.0, FontFamily::Monospace),
            color: theme::INK,
            extra_letter_spacing: 0.8,
            ..Default::default()
        },
    ));
    p.galley(rect.center() - galley.size() / 2.0, galley, theme::INK);
    resp
}
