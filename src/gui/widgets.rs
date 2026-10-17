//! Custom Adwaita-styled widgets matching the GNOME Nautilus design language.

use egui::{
    vec2, Color32, FontId, Pos2, Rect, Response, Rounding, Sense, Stroke, Ui,
};

use super::theme::*;

/// Formats byte counts into human-readable strings (B, KB, MB, GB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Returns a human-friendly file type description.
pub fn file_type_description(name: &str, is_dir: bool) -> &'static str {
    if is_dir {
        return "Folder";
    }

    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "Image"
    } else if lower.ends_with(".sfo") {
        "SFO Metadata"
    } else if lower.ends_with(".bin") || lower.ends_with(".self") || lower.ends_with(".elf") || lower.ends_with(".prx") {
        "Executable"
    } else if lower.ends_with(".xml") || lower.ends_with(".json") || lower.ends_with(".txt") {
        "Document"
    } else if lower.ends_with(".dat") || lower.ends_with(".pak") || lower.ends_with(".arc") {
        "Package Archive"
    } else if lower.ends_with(".trp") {
        "Trophy Package"
    } else {
        "File"
    }
}

/// Render a badge pill (e.g. "Base Game", "Patch", "DLC").
pub fn render_badge(ui: &mut Ui, label: &str, color: Color32) {
    let padding = vec2(7.0, 2.5);
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::proportional(11.0),
        Color32::WHITE,
    );

    let desired_size = galley.size() + padding * 2.0;
    let (rect, _response) = ui.allocate_exact_size(desired_size, Sense::hover());

    ui.painter().rect_filled(rect, Rounding::same(10.0), color);
    let text_pos = Pos2::new(
        rect.min.x + padding.x,
        rect.min.y + padding.y,
    );
    ui.painter().galley(text_pos, galley, Color32::WHITE);
}

/// Pathbar breadcrumb item.
pub fn breadcrumb_button(ui: &mut Ui, label: &str, is_current: bool) -> Response {
    let padding = vec2(8.0, 4.0);
    let font_id = FontId::proportional(13.0);
    let text_color = if is_current {
        COLOR_TEXT_PRIMARY
    } else {
        COLOR_TEXT_SECONDARY
    };

    let galley = ui.painter().layout_no_wrap(label.to_string(), font_id, text_color);
    let desired_size = galley.size() + padding * 2.0;
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            Rounding::same(6.0),
            COLOR_HOVER_BG,
        );
    }

    if is_current {
        ui.painter().rect_filled(
            rect,
            Rounding::same(6.0),
            Color32::from_rgba_premultiplied(255, 255, 255, 18),
        );
    }

    let text_pos = Pos2::new(rect.min.x + padding.x, rect.min.y + padding.y);
    ui.painter().galley(text_pos, galley, text_color);

    response
}

/// Nautilus-style floating status pill at the bottom of the view.
pub fn render_floating_pill(ui: &mut Ui, parent_rect: Rect, text: &str) {
    let padding = vec2(14.0, 6.0);
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        FontId::proportional(12.0),
        COLOR_TEXT_SECONDARY,
    );

    let pill_size = galley.size() + padding * 2.0;
    let pill_rect = Rect::from_min_size(
        Pos2::new(
            parent_rect.center().x - pill_size.x / 2.0,
            parent_rect.max.y - pill_size.y - 12.0,
        ),
        pill_size,
    );

    // Pill background & border
    ui.painter().rect_filled(pill_rect, Rounding::same(14.0), COLOR_BG_PILL);
    ui.painter().rect_stroke(
        pill_rect,
        Rounding::same(14.0),
        Stroke::new(1.0, COLOR_BORDER_SUBTLE),
    );

    let text_pos = Pos2::new(pill_rect.min.x + padding.x, pill_rect.min.y + padding.y);
    ui.painter().galley(text_pos, galley, COLOR_TEXT_SECONDARY);
}

/// Renders a clean icon symbol (folder, file, image, code) using vector primitives.
pub fn render_file_icon(ui: &mut Ui, rect: Rect, is_dir: bool, name: &str) {
    let lower = name.to_ascii_lowercase();

    if is_dir {
        // Nautilus folder icon (clean blue folder)
        let folder_color = COLOR_ACCENT_BLUE;
        let tab_rect = Rect::from_min_max(
            Pos2::new(rect.min.x, rect.min.y + 2.0),
            Pos2::new(rect.min.x + rect.width() * 0.45, rect.min.y + 7.0),
        );
        let body_rect = Rect::from_min_max(
            Pos2::new(rect.min.x, rect.min.y + 6.0),
            Pos2::new(rect.max.x, rect.max.y - 2.0),
        );
        ui.painter().rect_filled(tab_rect, Rounding::same(2.0), folder_color);
        ui.painter().rect_filled(body_rect, Rounding::same(3.0), folder_color);
        return;
    }

    let (icon_color, label) = if lower.ends_with(".png") || lower.ends_with(".jpg") {
        (Color32::from_rgb(46, 160, 67), "IMG")
    } else if lower.ends_with(".sfo") {
        (Color32::from_rgb(227, 98, 9), "SFO")
    } else if lower.ends_with(".bin") || lower.ends_with(".self") || lower.ends_with(".elf") {
        (Color32::from_rgb(137, 87, 229), "BIN")
    } else if lower.ends_with(".xml") || lower.ends_with(".json") || lower.ends_with(".txt") {
        (Color32::from_rgb(53, 132, 228), "DOC")
    } else {
        (COLOR_TEXT_MUTED, "FILE")
    };

    // Document page shape
    let page_rect = Rect::from_min_max(
        Pos2::new(rect.min.x + 2.0, rect.min.y + 2.0),
        Pos2::new(rect.max.x - 2.0, rect.max.y - 2.0),
    );

    ui.painter().rect_filled(page_rect, Rounding::same(3.0), COLOR_BG_CARD);
    ui.painter().rect_stroke(
        page_rect,
        Rounding::same(3.0),
        Stroke::new(1.0, icon_color),
    );

    let text_galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::monospace(8.0),
        icon_color,
    );
    let center = page_rect.center();
    let text_pos = Pos2::new(center.x - text_galley.size().x / 2.0, center.y - text_galley.size().y / 2.0);
    ui.painter().galley(text_pos, text_galley, icon_color);
}
