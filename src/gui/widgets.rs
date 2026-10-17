//! Custom Adwaita-styled widgets matching the GNOME Nautilus design language.

use egui::{
    pos2, vec2, Color32, FontId, Pos2, Rect, Response, Rounding, Sense, Stroke, Ui,
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

/// Render a badge pill (e.g. "Base Game", "Patch", "DLC") with Adwaita styling.
pub fn render_badge(ui: &mut Ui, label: &str, color: Color32) {
    let padding = vec2(8.0, 3.0);
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::proportional(11.0),
        color,
    );

    let desired_size = galley.size() + padding * 2.0;
    let (rect, _) = ui.allocate_exact_size(desired_size, Sense::hover());

    let bg_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 38);
    let border_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 90);

    ui.painter().rect_filled(rect, Rounding::same(10.0), bg_color);
    ui.painter().rect_stroke(rect, Rounding::same(10.0), Stroke::new(1.0, border_color));

    let text_pos = Pos2::new(rect.min.x + padding.x, rect.min.y + padding.y);
    ui.painter().galley(text_pos, galley, color);
}

/// Paints the signature Adwaita blue folder icon.
pub fn paint_folder_icon(painter: &egui::Painter, rect: Rect) {
    let w = rect.width();
    let h = rect.height();

    // Darker blue back folder tab
    let tab_w = w * 0.44;
    let tab_h = h * 0.32;
    let tab_rect = Rect::from_min_size(
        rect.min,
        vec2(tab_w, tab_h),
    );
    let tab_color = Color32::from_rgb(43, 114, 196); // #2b72c4
    painter.rect_filled(
        tab_rect,
        Rounding { nw: 3.0, ne: 3.0, sw: 0.0, se: 0.0 },
        tab_color,
    );

    // Front folder body
    let body_top = rect.min.y + h * 0.24;
    let body_rect = Rect::from_min_max(
        pos2(rect.min.x, body_top),
        rect.max,
    );
    let body_color = COLOR_ACCENT_BLUE; // #3584e4
    painter.rect_filled(body_rect, Rounding::same(3.0), body_color);

    // Top highlight rim
    let rim_color = Color32::from_rgba_unmultiplied(255, 255, 255, 60);
    painter.line_segment(
        [pos2(body_rect.min.x + 2.0, body_top + 1.0), pos2(body_rect.max.x - 2.0, body_top + 1.0)],
        Stroke::new(1.0, rim_color),
    );
}

/// Paints an authentic Adwaita document icon with a folded corner and custom type emblem.
pub fn paint_file_icon(painter: &egui::Painter, rect: Rect, name: &str) {
    let lower = name.to_ascii_lowercase();
    let w = rect.width();
    let h = rect.height();

    let dogear = (w * 0.28).min(h * 0.28).clamp(4.0, 10.0);

    // Draw main sheet polygon with top-right corner clipped
    let p_top_left = rect.min;
    let p_dogear_left = pos2(rect.max.x - dogear, rect.min.y);
    let p_dogear_bottom = pos2(rect.max.x, rect.min.y + dogear);
    let p_bottom_right = rect.max;
    let p_bottom_left = pos2(rect.min.x, rect.max.y);

    let sheet_points = vec![
        p_top_left,
        p_dogear_left,
        p_dogear_bottom,
        p_bottom_right,
        p_bottom_left,
    ];

    let sheet_bg = Color32::from_rgb(46, 46, 46);
    let sheet_border = Color32::from_rgba_unmultiplied(255, 255, 255, 30);

    painter.add(egui::Shape::convex_polygon(
        sheet_points,
        sheet_bg,
        Stroke::new(1.0, sheet_border),
    ));

    // Dogear fold triangle
    let p_dogear_corner = pos2(rect.max.x - dogear, rect.min.y + dogear);
    let dogear_triangle = vec![p_dogear_left, p_dogear_corner, p_dogear_bottom];
    let dogear_color = Color32::from_rgba_unmultiplied(255, 255, 255, 25);
    painter.add(egui::Shape::convex_polygon(
        dogear_triangle,
        dogear_color,
        Stroke::new(1.0, sheet_border),
    ));

    // Inner Emblem
    let center = rect.center();
    if lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        // Image icon: Green picture frame with mountain/sun
        let frame_rect = Rect::from_center_size(
            pos2(center.x, center.y + 1.0),
            vec2(w * 0.55, h * 0.42),
        );
        let green = Color32::from_rgb(38, 162, 105);
        painter.rect_stroke(frame_rect, Rounding::same(2.0), Stroke::new(1.2, green));

        // Sun dot
        let sun_center = pos2(frame_rect.min.x + frame_rect.width() * 0.75, frame_rect.min.y + frame_rect.height() * 0.32);
        painter.circle_filled(sun_center, (w * 0.08).clamp(1.5, 3.0), green);

        // Mountain peak
        let mtn_peak = pos2(frame_rect.min.x + frame_rect.width() * 0.45, frame_rect.min.y + frame_rect.height() * 0.45);
        let mtn_left = pos2(frame_rect.min.x + 2.0, frame_rect.max.y - 1.0);
        let mtn_right = pos2(frame_rect.max.x - 2.0, frame_rect.max.y - 1.0);
        painter.line_segment([mtn_left, mtn_peak], Stroke::new(1.2, green));
        painter.line_segment([mtn_peak, mtn_right], Stroke::new(1.2, green));
    } else if lower.ends_with(".sfo") {
        // SFO / Settings icon: Orange data bars & tag
        let orange = Color32::from_rgb(229, 165, 10);
        let bar_w = w * 0.52;
        let start_x = center.x - bar_w / 2.0;

        let y1 = center.y - h * 0.12;
        let y2 = center.y + h * 0.06;
        let y3 = center.y + h * 0.24;

        painter.line_segment([pos2(start_x, y1), pos2(start_x + bar_w, y1)], Stroke::new(1.5, orange));
        painter.line_segment([pos2(start_x, y2), pos2(start_x + bar_w * 0.75, y2)], Stroke::new(1.5, orange));
        painter.line_segment([pos2(start_x, y3), pos2(start_x + bar_w * 0.5, y3)], Stroke::new(1.5, orange));
    } else if lower.ends_with(".bin") || lower.ends_with(".self") || lower.ends_with(".elf") || lower.ends_with(".prx") {
        // Executable / Binary icon: Purple terminal prompt `>_`
        let violet = Color32::from_rgb(145, 65, 172);
        let p1 = pos2(center.x - w * 0.22, center.y - h * 0.14);
        let p2 = pos2(center.x - w * 0.08, center.y);
        let p3 = pos2(center.x - w * 0.22, center.y + h * 0.14);

        painter.line_segment([p1, p2], Stroke::new(1.4, violet));
        painter.line_segment([p2, p3], Stroke::new(1.4, violet));

        // Underscore cursor
        let cur_x1 = center.x + w * 0.02;
        let cur_x2 = center.x + w * 0.22;
        let cur_y = center.y + h * 0.14;
        painter.line_segment([pos2(cur_x1, cur_y), pos2(cur_x2, cur_y)], Stroke::new(1.4, violet));
    } else {
        // Generic document: clean muted lines
        let muted = Color32::from_rgb(120, 120, 120);
        let bar_w = w * 0.5;
        let start_x = center.x - bar_w / 2.0;
        let y1 = center.y - h * 0.08;
        let y2 = center.y + h * 0.1;
        let y3 = center.y + h * 0.26;

        painter.line_segment([pos2(start_x, y1), pos2(start_x + bar_w, y1)], Stroke::new(1.2, muted));
        painter.line_segment([pos2(start_x, y2), pos2(start_x + bar_w * 0.8, y2)], Stroke::new(1.2, muted));
        painter.line_segment([pos2(start_x, y3), pos2(start_x + bar_w * 0.5, y3)], Stroke::new(1.2, muted));
    }
}

/// Renders a file or folder icon appropriately.
pub fn render_file_icon(ui: &mut Ui, rect: Rect, is_dir: bool, name: &str) {
    if is_dir {
        paint_folder_icon(ui.painter(), rect);
    } else {
        paint_file_icon(ui.painter(), rect, name);
    }
}

/// Linked Nautilus navigation pill buttons (`<` and `>`).
pub fn linked_navigation_buttons(
    ui: &mut Ui,
    can_back: bool,
    can_forward: bool,
) -> (bool, bool) {
    let total_size = vec2(66.0, 32.0);
    let (rect, _) = ui.allocate_exact_size(total_size, Sense::hover());

    let half_w = total_size.x / 2.0;
    let back_rect = Rect::from_min_size(rect.min, vec2(half_w, total_size.y));
    let fwd_rect = Rect::from_min_size(pos2(rect.min.x + half_w, rect.min.y), vec2(half_w, total_size.y));

    let back_resp = ui.interact(back_rect, ui.id().with("nav_back"), Sense::click());
    let fwd_resp = ui.interact(fwd_rect, ui.id().with("nav_fwd"), Sense::click());

    // Background pill container
    ui.painter().rect_filled(
        rect,
        Rounding::same(7.0),
        Color32::from_rgba_unmultiplied(255, 255, 255, 14),
    );
    ui.painter().rect_stroke(
        rect,
        Rounding::same(7.0),
        Stroke::new(1.0, COLOR_BORDER_SUBTLE),
    );

    // Hover / Active states
    if can_back && back_resp.hovered() {
        ui.painter().rect_filled(
            back_rect,
            Rounding { nw: 7.0, sw: 7.0, ne: 0.0, se: 0.0 },
            COLOR_HOVER_BG,
        );
    }
    if can_forward && fwd_resp.hovered() {
        ui.painter().rect_filled(
            fwd_rect,
            Rounding { nw: 0.0, sw: 0.0, ne: 7.0, se: 7.0 },
            COLOR_HOVER_BG,
        );
    }

    // Middle divider line
    ui.painter().line_segment(
        [pos2(rect.min.x + half_w, rect.min.y + 4.0), pos2(rect.min.x + half_w, rect.max.y - 4.0)],
        Stroke::new(1.0, COLOR_BORDER_SUBTLE),
    );

    // Vector Chevron Left
    let back_col = if can_back {
        if back_resp.hovered() { Color32::WHITE } else { COLOR_TEXT_PRIMARY }
    } else {
        COLOR_TEXT_MUTED
    };
    let bc = back_rect.center();
    ui.painter().line_segment([pos2(bc.x + 2.0, bc.y - 5.0), pos2(bc.x - 3.0, bc.y)], Stroke::new(1.8, back_col));
    ui.painter().line_segment([pos2(bc.x - 3.0, bc.y), pos2(bc.x + 2.0, bc.y + 5.0)], Stroke::new(1.8, back_col));

    // Vector Chevron Right
    let fwd_col = if can_forward {
        if fwd_resp.hovered() { Color32::WHITE } else { COLOR_TEXT_PRIMARY }
    } else {
        COLOR_TEXT_MUTED
    };
    let fc = fwd_rect.center();
    ui.painter().line_segment([pos2(fc.x - 2.0, fc.y - 5.0), pos2(fc.x + 3.0, fc.y)], Stroke::new(1.8, fwd_col));
    ui.painter().line_segment([pos2(fc.x + 3.0, fc.y), pos2(fc.x - 2.0, fc.y + 5.0)], Stroke::new(1.8, fwd_col));

    (can_back && back_resp.clicked(), can_forward && fwd_resp.clicked())
}

/// Up directory button with vector upward arrow.
pub fn up_directory_button(ui: &mut Ui, enabled: bool) -> bool {
    let size = vec2(32.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());

    let bg_color = if enabled && resp.hovered() {
        Color32::from_rgba_unmultiplied(255, 255, 255, 24)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 14)
    };

    ui.painter().rect_filled(rect, Rounding::same(7.0), bg_color);
    ui.painter().rect_stroke(rect, Rounding::same(7.0), Stroke::new(1.0, COLOR_BORDER_SUBTLE));

    let col = if enabled {
        if resp.hovered() { Color32::WHITE } else { COLOR_TEXT_PRIMARY }
    } else {
        COLOR_TEXT_MUTED
    };

    let c = rect.center();
    ui.painter().line_segment([pos2(c.x - 5.0, c.y + 2.0), pos2(c.x, c.y - 3.0)], Stroke::new(1.8, col));
    ui.painter().line_segment([pos2(c.x, c.y - 3.0), pos2(c.x + 5.0, c.y + 2.0)], Stroke::new(1.8, col));

    enabled && resp.clicked()
}

/// Pathbar breadcrumb item with Nautilus styling.
pub fn breadcrumb_button(ui: &mut Ui, label: &str, is_current: bool, has_icon: bool) -> Response {
    let font_id = if is_current {
        FontId::proportional(13.0)
    } else {
        FontId::proportional(12.5)
    };
    let text_color = if is_current {
        COLOR_TEXT_PRIMARY
    } else {
        COLOR_TEXT_SECONDARY
    };

    let galley = ui.painter().layout_no_wrap(label.to_string(), font_id.clone(), text_color);
    let icon_w = if has_icon { 22.0 } else { 0.0 };
    let arrow_w = if is_current { 16.0 } else { 0.0 };
    let total_w = galley.size().x + icon_w + arrow_w + 18.0;

    let (rect, response) = ui.allocate_exact_size(vec2(total_w, 28.0), Sense::click());

    if is_current {
        ui.painter().rect_filled(
            rect,
            Rounding::same(6.0),
            Color32::from_rgba_unmultiplied(255, 255, 255, 22),
        );
    } else if response.hovered() {
        ui.painter().rect_filled(
            rect,
            Rounding::same(6.0),
            COLOR_HOVER_BG,
        );
    }

    let mut cur_x = rect.min.x + 8.0;

    if has_icon {
        let icon_rect = Rect::from_center_size(
            pos2(cur_x + 7.0, rect.center().y),
            vec2(14.0, 14.0),
        );
        paint_folder_icon(ui.painter(), icon_rect);
        cur_x += 18.0;
    }

    ui.painter().text(
        pos2(cur_x, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        font_id,
        text_color,
    );

    // Dropdown arrow for current segment
    if is_current {
        let arrow_x = rect.max.x - 9.0;
        let cy = rect.center().y;
        ui.painter().line_segment(
            [pos2(arrow_x - 3.0, cy - 1.5), pos2(arrow_x, cy + 1.5)],
            Stroke::new(1.3, COLOR_TEXT_MUTED),
        );
        ui.painter().line_segment(
            [pos2(arrow_x, cy + 1.5), pos2(arrow_x + 3.0, cy - 1.5)],
            Stroke::new(1.3, COLOR_TEXT_MUTED),
        );
    }

    response
}

/// Segmented List / Grid view switcher with vector icons.
pub fn view_mode_switcher(ui: &mut Ui, is_grid: bool) -> Option<bool> {
    let size = vec2(68.0, 32.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());

    let bg = Color32::from_rgba_unmultiplied(255, 255, 255, 14);
    let border = Color32::from_rgba_unmultiplied(255, 255, 255, 24);
    ui.painter().rect_filled(rect, Rounding::same(7.0), bg);
    ui.painter().rect_stroke(rect, Rounding::same(7.0), Stroke::new(1.0, border));

    let half_w = size.x / 2.0;
    let list_rect = Rect::from_min_size(rect.min, vec2(half_w, size.y));
    let grid_rect = Rect::from_min_size(pos2(rect.min.x + half_w, rect.min.y), vec2(half_w, size.y));

    let list_resp = ui.interact(list_rect, ui.id().with("toggle_list"), Sense::click());
    let grid_resp = ui.interact(grid_rect, ui.id().with("toggle_grid"), Sense::click());

    // Highlight active
    let active_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 30);
    if !is_grid {
        ui.painter().rect_filled(
            list_rect,
            Rounding { nw: 6.0, sw: 6.0, ne: 0.0, se: 0.0 },
            active_fill,
        );
    } else {
        ui.painter().rect_filled(
            grid_rect,
            Rounding { nw: 0.0, sw: 0.0, ne: 6.0, se: 6.0 },
            active_fill,
        );
    }

    // Paint List Icon (3 horizontal bars)
    let lc = list_rect.center();
    let l_color = if !is_grid { Color32::WHITE } else { COLOR_TEXT_MUTED };
    for dy in &[-4.0, 0.0, 4.0] {
        ui.painter().line_segment(
            [pos2(lc.x - 6.0, lc.y + dy), pos2(lc.x + 6.0, lc.y + dy)],
            Stroke::new(1.6, l_color),
        );
    }

    // Paint Grid Icon (2x2 square blocks)
    let gc = grid_rect.center();
    let g_color = if is_grid { Color32::WHITE } else { COLOR_TEXT_MUTED };
    let block_sz = 3.5;
    for (dx, dy) in &[(-4.5, -4.5), (1.5, -4.5), (-4.5, 1.5), (1.5, 1.5)] {
        let b_rect = Rect::from_min_size(pos2(gc.x + dx, gc.y + dy), vec2(block_sz, block_sz));
        ui.painter().rect_filled(b_rect, Rounding::same(1.0), g_color);
    }

    if list_resp.clicked() && is_grid {
        Some(false)
    } else if grid_resp.clicked() && !is_grid {
        Some(true)
    } else {
        None
    }
}

/// Search entry pill with vector magnifying glass and clear button.
pub fn search_entry_pill(ui: &mut Ui, search_text: &mut String) {
    let size = vec2(190.0, 32.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());

    let bg_color = Color32::from_rgba_unmultiplied(255, 255, 255, 14);
    let border_color = Color32::from_rgba_unmultiplied(255, 255, 255, 24);
    ui.painter().rect_filled(rect, Rounding::same(16.0), bg_color);
    ui.painter().rect_stroke(rect, Rounding::same(16.0), Stroke::new(1.0, border_color));

    // Vector Magnifying Glass
    let mag_c = pos2(rect.min.x + 15.0, rect.center().y - 1.0);
    let r = 4.5;
    ui.painter().circle_stroke(mag_c, r, Stroke::new(1.4, COLOR_TEXT_MUTED));
    ui.painter().line_segment(
        [pos2(mag_c.x + 3.2, mag_c.y + 3.2), pos2(mag_c.x + 7.5, mag_c.y + 7.5)],
        Stroke::new(1.6, COLOR_TEXT_MUTED),
    );

    // TextEdit area
    let text_w = size.x - 52.0;
    let edit_rect = Rect::from_min_size(
        pos2(rect.min.x + 28.0, rect.min.y + 5.0),
        vec2(text_w, 22.0),
    );

    let edit = egui::TextEdit::singleline(search_text)
        .hint_text("Search...")
        .desired_width(text_w)
        .frame(false);
    ui.put(edit_rect, edit);

    // Clear '✕' button
    if !search_text.is_empty() {
        let clear_rect = Rect::from_center_size(
            pos2(rect.max.x - 14.0, rect.center().y),
            vec2(16.0, 16.0),
        );
        let clear_resp = ui.interact(clear_rect, ui.id().with("search_clear"), Sense::click());
        let col = if clear_resp.hovered() { Color32::WHITE } else { COLOR_TEXT_MUTED };

        let c = clear_rect.center();
        let s = 3.5;
        ui.painter().line_segment([pos2(c.x - s, c.y - s), pos2(c.x + s, c.y + s)], Stroke::new(1.4, col));
        ui.painter().line_segment([pos2(c.x + s, c.y - s), pos2(c.x - s, c.y + s)], Stroke::new(1.4, col));

        if clear_resp.clicked() {
            search_text.clear();
        }
    }
}

/// Suggested action button (solid Adwaita Blue).
pub fn suggested_action_button(ui: &mut Ui, label: &str) -> Response {
    let desired_size = vec2(ui.available_width(), 36.0);
    let (rect, resp) = ui.allocate_exact_size(desired_size, Sense::click());

    let (bg, border) = if resp.is_pointer_button_down_on() {
        (COLOR_ACCENT_ACTIVE, COLOR_ACCENT_ACTIVE)
    } else if resp.hovered() {
        (COLOR_ACCENT_HOVER, COLOR_ACCENT_HOVER)
    } else {
        (COLOR_ACCENT_BLUE, COLOR_ACCENT_BLUE)
    };

    ui.painter().rect_filled(rect, Rounding::same(7.0), bg);
    ui.painter().rect_stroke(rect, Rounding::same(7.0), Stroke::new(1.0, border));

    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(13.0),
        Color32::WHITE,
    );

    resp
}

/// Nautilus-style sidebar row with active indicator.
pub fn sidebar_item(
    ui: &mut Ui,
    label: &str,
    is_active: bool,
    is_dir: bool,
) -> Response {
    let desired_size = vec2(ui.available_width(), 34.0);
    let (rect, resp) = ui.allocate_exact_size(desired_size, Sense::click());

    if is_active {
        // Active item: soft Adwaita highlight + left blue accent bar
        let fill = Color32::from_rgba_unmultiplied(255, 255, 255, 22);
        ui.painter().rect_filled(rect, Rounding::same(6.0), fill);

        let bar_rect = Rect::from_min_size(
            pos2(rect.min.x + 2.0, rect.min.y + 6.0),
            vec2(3.0, rect.height() - 12.0),
        );
        ui.painter().rect_filled(bar_rect, Rounding::same(1.5), COLOR_ACCENT_BLUE);
    } else if resp.hovered() {
        ui.painter().rect_filled(
            rect,
            Rounding::same(6.0),
            COLOR_HOVER_BG,
        );
    }

    // Icon
    let icon_rect = Rect::from_center_size(
        pos2(rect.min.x + 18.0, rect.center().y),
        vec2(16.0, 16.0),
    );
    if is_dir {
        paint_folder_icon(ui.painter(), icon_rect);
    } else {
        paint_file_icon(ui.painter(), icon_rect, label);
    }

    // Label
    let text_color = if is_active {
        Color32::WHITE
    } else if resp.hovered() {
        COLOR_TEXT_PRIMARY
    } else {
        COLOR_TEXT_SECONDARY
    };

    let font_id = if is_active {
        FontId::proportional(13.0)
    } else {
        FontId::proportional(12.5)
    };

    ui.painter().text(
        pos2(rect.min.x + 36.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        font_id,
        text_color,
    );

    resp
}

/// Nautilus-style floating status pill at the bottom of the view.
pub fn render_floating_pill(ui: &mut Ui, parent_rect: Rect, text: &str) {
    let padding = vec2(16.0, 6.0);
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        FontId::proportional(12.0),
        COLOR_TEXT_SECONDARY,
    );

    let pill_size = vec2(galley.size().x + padding.x * 2.0, 28.0);
    let pill_rect = Rect::from_min_size(
        pos2(
            parent_rect.center().x - pill_size.x / 2.0,
            parent_rect.max.y - pill_size.y - 14.0,
        ),
        pill_size,
    );

    ui.painter().rect_filled(pill_rect, Rounding::same(14.0), COLOR_BG_PILL);
    ui.painter().rect_stroke(
        pill_rect,
        Rounding::same(14.0),
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 28)),
    );

    let text_pos = pos2(
        pill_rect.center().x - galley.size().x / 2.0,
        pill_rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(text_pos, galley, COLOR_TEXT_SECONDARY);
}
