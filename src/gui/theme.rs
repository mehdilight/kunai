//! GNOME Adwaita Dark theme styling and design tokens for Kunai.

use egui::{
    epaint::Shadow,
    style::WidgetVisuals,
    Color32, Margin, Rounding, Stroke, TextStyle, Visuals,
};
use std::collections::BTreeMap;

// GNOME Adwaita Dark Palette (libadwaita 1.4+)
pub const COLOR_BG_WINDOW: Color32 = Color32::from_rgb(36, 36, 36);         // #242424 base
pub const COLOR_BG_VIEW: Color32 = Color32::from_rgb(30, 30, 30);           // #1e1e1e content
pub const COLOR_BG_HEADERBAR: Color32 = Color32::from_rgb(46, 46, 46);      // #2e2e2e header
pub const COLOR_BG_SIDEBAR: Color32 = Color32::from_rgb(38, 38, 38);        // #262626 sidebar
pub const COLOR_BG_CARD: Color32 = Color32::from_rgb(48, 48, 48);           // #303030 card
pub const COLOR_BG_CARD_HOVER: Color32 = Color32::from_rgb(58, 58, 58);     // #3a3a3a card hover
pub const COLOR_BG_PILL: Color32 = Color32::from_rgba_premultiplied(35, 35, 35, 245);

// Adwaita Blue Accents
pub const COLOR_ACCENT_BLUE: Color32 = Color32::from_rgb(53, 132, 228);     // #3584e4
pub const COLOR_ACCENT_HOVER: Color32 = Color32::from_rgb(74, 144, 226);    // #4a90e2
pub const COLOR_ACCENT_ACTIVE: Color32 = Color32::from_rgb(28, 113, 216);   // #1c71d8
pub const COLOR_ACCENT_SUBTLE: Color32 = Color32::from_rgba_premultiplied(7, 17, 29, 32);

// Highlights & Hover (premultiplied: for white with alpha A, rgb is A, A, A)
pub const COLOR_HOVER_BG: Color32 = Color32::from_rgba_premultiplied(14, 14, 14, 14);
pub const COLOR_ACTIVE_BG: Color32 = Color32::from_rgba_premultiplied(24, 24, 24, 24);
pub const COLOR_SELECTION_BG: Color32 = Color32::from_rgba_premultiplied(11, 28, 49, 55);

// Borders & Dividers
pub const COLOR_BORDER_SUBTLE: Color32 = Color32::from_rgba_premultiplied(20, 20, 20, 20);
pub const COLOR_BORDER_HEADER: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 120);
pub const COLOR_BORDER_ACCENT: Color32 = Color32::from_rgba_premultiplied(31, 78, 134, 150);

// Text Typography
pub const COLOR_TEXT_PRIMARY: Color32 = Color32::from_rgb(255, 255, 255);
pub const COLOR_TEXT_SECONDARY: Color32 = Color32::from_rgb(175, 175, 175); // #afafaf
pub const COLOR_TEXT_MUTED: Color32 = Color32::from_rgb(125, 125, 125);     // #7d7d7d

// Adwaita Status Badges
pub const COLOR_BADGE_BASE: Color32 = Color32::from_rgb(38, 162, 105);     // Adwaita Green #26a269
pub const COLOR_BADGE_PATCH: Color32 = Color32::from_rgb(229, 165, 10);    // Adwaita Orange #e5a50a
pub const COLOR_BADGE_DLC: Color32 = Color32::from_rgb(145, 65, 172);      // Adwaita Purple #9141ac

pub fn apply_adwaita_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(COLOR_TEXT_PRIMARY);
    visuals.panel_fill = COLOR_BG_WINDOW;
    visuals.window_fill = COLOR_BG_CARD;
    visuals.faint_bg_color = COLOR_BG_VIEW;
    visuals.extreme_bg_color = COLOR_BG_VIEW;
    visuals.code_bg_color = Color32::from_rgb(22, 22, 22);

    visuals.window_rounding = Rounding::same(12.0);
    visuals.window_stroke = Stroke::new(1.0, COLOR_BORDER_SUBTLE);
    visuals.window_shadow = Shadow {
        offset: egui::vec2(0.0, 6.0),
        blur: 20.0,
        spread: 0.0,
        color: Color32::from_black_alpha(110),
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

    // Inactive widgets (buttons, fields)
    visuals.widgets.inactive = WidgetVisuals {
        bg_fill: Color32::from_rgba_premultiplied(16, 16, 16, 16),
        weak_bg_fill: Color32::from_rgba_premultiplied(10, 10, 10, 10),
        bg_stroke: Stroke::new(1.0, Color32::from_rgba_premultiplied(22, 22, 22, 22)),
        rounding: Rounding::same(7.0),
        fg_stroke: Stroke::new(1.0, COLOR_TEXT_PRIMARY),
        expansion: 0.0,
    };

    // Hovered widgets
    visuals.widgets.hovered = WidgetVisuals {
        bg_fill: Color32::from_rgba_premultiplied(28, 28, 28, 28),
        weak_bg_fill: COLOR_HOVER_BG,
        bg_stroke: Stroke::new(1.0, Color32::from_rgba_premultiplied(40, 40, 40, 40)),
        rounding: Rounding::same(7.0),
        fg_stroke: Stroke::new(1.0, Color32::WHITE),
        expansion: 0.0,
    };

    // Active / Pressed widgets
    visuals.widgets.active = WidgetVisuals {
        bg_fill: COLOR_ACCENT_ACTIVE,
        weak_bg_fill: COLOR_ACCENT_ACTIVE,
        bg_stroke: Stroke::new(1.0, COLOR_ACCENT_HOVER),
        rounding: Rounding::same(7.0),
        fg_stroke: Stroke::new(1.0, Color32::WHITE),
        expansion: 0.0,
    };

    // Open menus / comboboxes
    visuals.widgets.open = WidgetVisuals {
        bg_fill: COLOR_BG_CARD,
        weak_bg_fill: COLOR_BG_CARD,
        bg_stroke: Stroke::new(1.0, COLOR_BORDER_SUBTLE),
        rounding: Rounding::same(8.0),
        fg_stroke: Stroke::new(1.0, Color32::WHITE),
        expansion: 0.0,
    };

    visuals.selection.bg_fill = COLOR_ACCENT_BLUE;
    visuals.selection.stroke = Stroke::new(1.0, COLOR_ACCENT_HOVER);

    ctx.set_visuals(visuals);

    // Adwaita Typography scale
    let mut text_styles = BTreeMap::new();
    text_styles.insert(TextStyle::Heading, egui::FontId::proportional(16.0));
    text_styles.insert(TextStyle::Name("HeaderTitle".into()), egui::FontId::proportional(14.0));
    text_styles.insert(TextStyle::Body, egui::FontId::proportional(13.0));
    text_styles.insert(TextStyle::Button, egui::FontId::proportional(12.5));
    text_styles.insert(TextStyle::Small, egui::FontId::proportional(11.0));
    text_styles.insert(TextStyle::Monospace, egui::FontId::monospace(11.5));
    let mut style = (*ctx.style()).clone();
    style.text_styles = text_styles;
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.window_margin = Margin::same(12.0);
    ctx.set_style(style);
}
