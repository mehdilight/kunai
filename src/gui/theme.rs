//! GNOME Adwaita Dark theme styling and design tokens for Kunai.

use egui::{
    epaint::Shadow,
    style::WidgetVisuals,
    Color32, Margin, Rounding, Stroke, Visuals,
};

// GNOME Adwaita Dark Palette
pub const COLOR_BG_WINDOW: Color32 = Color32::from_rgb(36, 36, 36);
pub const COLOR_BG_VIEW: Color32 = Color32::from_rgb(30, 30, 30);
pub const COLOR_BG_HEADERBAR: Color32 = Color32::from_rgb(48, 48, 48);
pub const COLOR_BG_SIDEBAR: Color32 = Color32::from_rgb(40, 40, 40);
pub const COLOR_BG_CARD: Color32 = Color32::from_rgb(56, 56, 56);
pub const COLOR_BG_CARD_HOVER: Color32 = Color32::from_rgb(66, 66, 66);
pub const COLOR_BG_PILL: Color32 = Color32::from_rgba_premultiplied(36, 36, 36, 235);

pub const COLOR_ACCENT_BLUE: Color32 = Color32::from_rgb(53, 132, 228);
pub const COLOR_ACCENT_HOVER: Color32 = Color32::from_rgb(74, 144, 226);
pub const COLOR_ACCENT_ACTIVE: Color32 = Color32::from_rgb(28, 113, 216);

pub const COLOR_SELECTION_BG: Color32 = Color32::from_rgba_premultiplied(53, 132, 228, 60);
pub const COLOR_HOVER_BG: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 14);

pub const COLOR_BORDER_SUBTLE: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 22);
pub const COLOR_BORDER_ACCENT: Color32 = Color32::from_rgba_premultiplied(53, 132, 228, 140);

pub const COLOR_TEXT_PRIMARY: Color32 = Color32::from_rgb(255, 255, 255);
pub const COLOR_TEXT_SECONDARY: Color32 = Color32::from_rgb(154, 153, 150);
pub const COLOR_TEXT_MUTED: Color32 = Color32::from_rgb(120, 120, 120);

pub const COLOR_BADGE_BASE: Color32 = Color32::from_rgb(46, 160, 67);
pub const COLOR_BADGE_PATCH: Color32 = Color32::from_rgb(227, 98, 9);
pub const COLOR_BADGE_DLC: Color32 = Color32::from_rgb(137, 87, 229);

pub fn apply_adwaita_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(COLOR_TEXT_PRIMARY);
    visuals.panel_fill = COLOR_BG_WINDOW;
    visuals.window_fill = COLOR_BG_CARD;
    visuals.faint_bg_color = COLOR_BG_VIEW;
    visuals.extreme_bg_color = COLOR_BG_VIEW;
    visuals.code_bg_color = Color32::from_rgb(24, 24, 24);

    visuals.window_rounding = Rounding::same(10.0);
    visuals.window_stroke = Stroke::new(1.0, COLOR_BORDER_SUBTLE);
    visuals.window_shadow = Shadow {
        offset: egui::vec2(0.0, 4.0),
        blur: 16.0,
        spread: 0.0,
        color: Color32::from_black_alpha(80),
    };

    // Non-interactive widgets
    visuals.widgets.noninteractive = WidgetVisuals {
        bg_fill: Color32::TRANSPARENT,
        weak_bg_fill: Color32::TRANSPARENT,
        bg_stroke: Stroke::NONE,
        rounding: Rounding::same(6.0),
        fg_stroke: Stroke::new(1.0, COLOR_TEXT_PRIMARY),
        expansion: 0.0,
    };

    // Inactive widgets (buttons, text edits)
    visuals.widgets.inactive = WidgetVisuals {
        bg_fill: COLOR_BG_CARD,
        weak_bg_fill: Color32::from_rgba_premultiplied(255, 255, 255, 12),
        bg_stroke: Stroke::new(1.0, COLOR_BORDER_SUBTLE),
        rounding: Rounding::same(6.0),
        fg_stroke: Stroke::new(1.0, COLOR_TEXT_PRIMARY),
        expansion: 0.0,
    };

    // Hovered widgets
    visuals.widgets.hovered = WidgetVisuals {
        bg_fill: COLOR_BG_CARD_HOVER,
        weak_bg_fill: COLOR_HOVER_BG,
        bg_stroke: Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 40)),
        rounding: Rounding::same(6.0),
        fg_stroke: Stroke::new(1.0, Color32::WHITE),
        expansion: 0.0,
    };

    // Active / Pressed widgets
    visuals.widgets.active = WidgetVisuals {
        bg_fill: COLOR_ACCENT_ACTIVE,
        weak_bg_fill: COLOR_ACCENT_ACTIVE,
        bg_stroke: Stroke::new(1.0, COLOR_ACCENT_HOVER),
        rounding: Rounding::same(6.0),
        fg_stroke: Stroke::new(1.0, Color32::WHITE),
        expansion: 0.0,
    };

    // Open menus / comboboxes
    visuals.widgets.open = WidgetVisuals {
        bg_fill: COLOR_BG_CARD,
        weak_bg_fill: COLOR_BG_CARD,
        bg_stroke: Stroke::new(1.0, COLOR_BORDER_SUBTLE),
        rounding: Rounding::same(6.0),
        fg_stroke: Stroke::new(1.0, Color32::WHITE),
        expansion: 0.0,
    };

    visuals.selection.bg_fill = COLOR_ACCENT_BLUE;
    visuals.selection.stroke = Stroke::new(1.0, COLOR_ACCENT_HOVER);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.window_margin = Margin::same(12.0);
    ctx.set_style(style);
}
