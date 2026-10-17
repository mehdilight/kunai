//! Main application state and Nautilus-styled UI rendering for Kunai.

use anyhow::{anyhow, Result};
use eframe::egui::{self, pos2, vec2, Align2, Color32, Rect, Rounding, ScrollArea, Stroke, Ui};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use super::preview::PreviewModal;
use super::theme::*;
use super::widgets::*;
use crate::core::pkg::{Pkg, PkgType};
use crate::core::virtual_fs::{FsNode, VirtualFs};
use crate::platform::current_platform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    List,
    Grid,
}

pub enum ExtractionEvent {
    Progress { current: usize, total: usize, file_name: String },
    Done(Result<PathBuf, String>),
}

pub struct KunaiApp {
    pub pkg_path: Option<PathBuf>,
    pub pkg: Option<Pkg>,
    pub vfs: Option<VirtualFs>,

    pub current_path: String,
    pub history_back: Vec<String>,
    pub history_forward: Vec<String>,

    pub search_query: String,
    pub view_mode: ViewMode,
    pub selected_node_path: Option<String>,

    pub preview: Option<PreviewModal>,
    pub icon_texture: Option<egui::TextureHandle>,

    pub extraction_rx: Option<Receiver<ExtractionEvent>>,
    pub is_extracting: bool,
    pub extraction_progress: (usize, usize, String),
    pub extraction_complete_msg: Option<String>,

    pub status_message: Option<(String, std::time::Instant)>,
}

impl KunaiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_adwaita_theme(&cc.egui_ctx);

        Self {
            pkg_path: None,
            pkg: None,
            vfs: None,
            current_path: String::new(),
            history_back: Vec::new(),
            history_forward: Vec::new(),
            search_query: String::new(),
            view_mode: ViewMode::List,
            selected_node_path: None,
            preview: None,
            icon_texture: None,
            extraction_rx: None,
            is_extracting: false,
            extraction_progress: (0, 0, String::new()),
            extraction_complete_msg: None,
            status_message: None,
        }
    }

    pub fn load_pkg(&mut self, path: PathBuf, ctx: &egui::Context) {
        if !path.exists() {
            self.set_status(format!("File not found: {}", path.display()));
            return;
        }

        match Pkg::open(&path) {
            Ok(mut pkg) => {
                match VirtualFs::build(&mut pkg) {
                    Ok(vfs) => {
                        let mut icon_tex = None;
                        if let Some(icon_node) = vfs.find_node("sce_sys/icon0.png") {
                            if let Ok(bytes) = vfs.read_file_content(pkg.file_mut(), icon_node) {
                                if let Ok(img) = image::load_from_memory(&bytes) {
                                    let size = [img.width() as usize, img.height() as usize];
                                    let rgba = img.to_rgba8();
                                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                        size,
                                        rgba.as_flat_samples().as_slice(),
                                    );
                                    icon_tex = Some(ctx.load_texture(
                                        "game_icon0",
                                        color_image,
                                        egui::TextureOptions::LINEAR,
                                    ));
                                }
                            }
                        }

                        self.pkg_path = Some(path);
                        self.pkg = Some(pkg);
                        self.vfs = Some(vfs);
                        self.icon_texture = icon_tex;
                        self.current_path = String::new();
                        self.history_back.clear();
                        self.history_forward.clear();
                        self.selected_node_path = None;
                        self.preview = None;
                        self.set_status("Package loaded successfully".to_string());
                    }
                    Err(e) => {
                        self.set_status(format!("Error building VFS: {}", e));
                    }
                }
            }
            Err(e) => {
                self.set_status(format!("Failed to open PKG: {}", e));
            }
        }
    }

    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some((msg, std::time::Instant::now()));
    }

    pub fn navigate_to(&mut self, path: String) {
        if self.current_path != path {
            self.history_back.push(self.current_path.clone());
            self.history_forward.clear();
            self.current_path = path;
            self.selected_node_path = None;
        }
    }

    pub fn navigate_back(&mut self) {
        if let Some(prev) = self.history_back.pop() {
            self.history_forward.push(self.current_path.clone());
            self.current_path = prev;
            self.selected_node_path = None;
        }
    }

    pub fn navigate_forward(&mut self) {
        if let Some(next) = self.history_forward.pop() {
            self.history_back.push(self.current_path.clone());
            self.current_path = next;
            self.selected_node_path = None;
        }
    }

    pub fn navigate_up(&mut self) {
        if self.current_path.is_empty() {
            return;
        }

        let up = match self.current_path.rfind('/') {
            Some(idx) => self.current_path[..idx].to_string(),
            None => String::new(),
        };
        self.navigate_to(up);
    }

    fn open_preview_for_node(&mut self, node: FsNode, ctx: &egui::Context) {
        if node.is_dir {
            self.navigate_to(node.relative_path);
            return;
        }

        if let (Some(vfs), Some(pkg)) = (&self.vfs, &mut self.pkg) {
            match PreviewModal::load(vfs, pkg.file_mut(), node, ctx) {
                Ok(modal) => {
                    self.preview = Some(modal);
                }
                Err(e) => {
                    self.set_status(format!("Failed to preview: {}", e));
                }
            }
        }
    }

    fn trigger_extract_selected(&mut self) {
        let selected_path = match &self.selected_node_path {
            Some(p) => p.clone(),
            None => return,
        };

        let node = match self.vfs.as_ref().and_then(|v| v.find_node(&selected_path)) {
            Some(n) => n.clone(),
            None => return,
        };

        if node.is_dir {
            if let Some(dest_dir) = rfd::FileDialog::new().pick_folder() {
                self.start_extraction(dest_dir, Some(node.relative_path));
            }
        } else if let Some(dest_file) = rfd::FileDialog::new().set_file_name(&node.name).save_file() {
            if let (Some(vfs), Some(pkg)) = (&self.vfs, &mut self.pkg) {
                match vfs.read_file_content(pkg.file_mut(), &node) {
                    Ok(bytes) => {
                        let _ = std::fs::write(&dest_file, bytes);
                        self.set_status(format!("Saved {}", dest_file.display()));
                    }
                    Err(e) => {
                        self.set_status(format!("Error extracting file: {}", e));
                    }
                }
            }
        }
    }

    fn start_extraction(&mut self, target_dir: PathBuf, _filter_prefix: Option<String>) {
        let pkg_path = match &self.pkg_path {
            Some(p) => p.clone(),
            None => return,
        };

        let (tx, rx): (Sender<ExtractionEvent>, Receiver<ExtractionEvent>) = channel();
        self.extraction_rx = Some(rx);
        self.is_extracting = true;
        self.extraction_progress = (0, 100, "Starting...".to_string());
        self.extraction_complete_msg = None;

        thread::spawn(move || {
            let res = (|| -> Result<PathBuf> {
                let mut pkg = Pkg::open(&pkg_path)?;
                let dest = pkg.extract(&target_dir, |current, total, file_name| {
                    let _ = tx.send(ExtractionEvent::Progress {
                        current,
                        total,
                        file_name: file_name.to_string(),
                    });
                })?;
                Ok(dest)
            })();

            match res {
                Ok(dest) => {
                    let _ = tx.send(ExtractionEvent::Done(Ok(dest)));
                }
                Err(e) => {
                    let _ = tx.send(ExtractionEvent::Done(Err(e.to_string())));
                }
            }
        });
    }

    fn check_extraction_events(&mut self) {
        let mut events = Vec::new();
        if let Some(rx) = &self.extraction_rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }

        for event in events {
            match event {
                ExtractionEvent::Progress { current, total, file_name } => {
                    self.extraction_progress = (current, total, file_name);
                }
                ExtractionEvent::Done(result) => {
                    self.is_extracting = false;
                    match result {
                        Ok(dest) => {
                            self.extraction_complete_msg = Some(format!(
                                "Successfully extracted to:\n{}",
                                dest.display()
                            ));
                            self.set_status(format!("Extracted to {}", dest.display()));
                        }
                        Err(e) => {
                            self.extraction_complete_msg = Some(format!("Extraction failed: {}", e));
                            self.set_status(format!("Extraction failed: {}", e));
                        }
                    }
                }
            }
        }
    }

    fn render_headerbar(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let mut nav_target = None;
        let mut open_root = false;

        egui::Frame::none()
            .fill(COLOR_BG_HEADERBAR)
            .inner_margin(egui::Margin::symmetric(14.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // 1. Linked navigation buttons (< and >)
                    let can_back = !self.history_back.is_empty();
                    let can_fwd = !self.history_forward.is_empty();
                    let (back_clicked, fwd_clicked) = linked_navigation_buttons(ui, can_back, can_fwd);
                    if back_clicked {
                        self.navigate_back();
                    }
                    if fwd_clicked {
                        self.navigate_forward();
                    }

                    // 2. Up Directory Button
                    let can_up = !self.current_path.is_empty();
                    if up_directory_button(ui, can_up) {
                        self.navigate_up();
                    }

                    ui.add_space(8.0);

                    let pathbar_bg = Color32::from_white_alpha(14);
                    egui::Frame::none()
                        .fill(pathbar_bg)
                        .rounding(Rounding::same(8.0))
                        .stroke(Stroke::new(1.0, COLOR_BORDER_SUBTLE))
                        .inner_margin(egui::Margin::symmetric(4.0, 2.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let is_root = self.current_path.is_empty();
                                if breadcrumb_button(ui, "Root", is_root, true).clicked() {
                                    open_root = true;
                                }

                                if !is_root {
                                    let segments: Vec<String> = self
                                        .current_path
                                        .split('/')
                                        .filter(|s| !s.is_empty())
                                        .map(|s| s.to_string())
                                        .collect();
                                    let mut accum = String::new();

                                    for (i, seg) in segments.iter().enumerate() {
                                        // Vector chevron separator
                                        let (sep_rect, _) = ui.allocate_exact_size(vec2(10.0, 28.0), egui::Sense::hover());
                                        let sc = sep_rect.center();
                                        ui.painter().line_segment(
                                            [pos2(sc.x - 2.0, sc.y - 4.0), pos2(sc.x + 2.0, sc.y)],
                                            Stroke::new(1.2, COLOR_TEXT_MUTED),
                                        );
                                        ui.painter().line_segment(
                                            [pos2(sc.x + 2.0, sc.y), pos2(sc.x - 2.0, sc.y + 4.0)],
                                            Stroke::new(1.2, COLOR_TEXT_MUTED),
                                        );

                                        if !accum.is_empty() {
                                            accum.push('/');
                                        }
                                        accum.push_str(seg);

                                        let is_last = i == segments.len() - 1;
                                        let target = accum.clone();
                                        if breadcrumb_button(ui, seg, is_last, false).clicked() {
                                            nav_target = Some(target);
                                        }
                                    }
                                }
                            });
                        });

                    // Right-side controls
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Open PKG Button
                        if ui.add(egui::Button::new("Open PKG...").min_size(vec2(0.0, 32.0))).clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("PS4 Package", &["pkg"])
                                .pick_file()
                            {
                                self.load_pkg(path, ctx);
                            }
                        }

                        // View mode switcher (List / Grid)
                        if let Some(is_grid) = view_mode_switcher(ui, self.view_mode == ViewMode::Grid) {
                            self.view_mode = if is_grid { ViewMode::Grid } else { ViewMode::List };
                        }

                        ui.add_space(4.0);

                        // Search entry pill
                        search_entry_pill(ui, &mut self.search_query);
                    });
                });
            });

        // 1px headerbar bottom divider
        let header_rect = ui.min_rect();
        ui.painter().line_segment(
            [pos2(header_rect.min.x, header_rect.max.y), pos2(header_rect.max.x, header_rect.max.y)],
            Stroke::new(1.0, COLOR_BORDER_HEADER),
        );

        if open_root {
            self.navigate_to(String::new());
        } else if let Some(t) = nav_target {
            self.navigate_to(t);
        }
    }

    fn render_sidebar(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.vertical(|ui| {
            // Game Info Card
            if let (Some(pkg), Some(path)) = (&self.pkg, &self.pkg_path) {
                let pkg_type = pkg.detect_type();

                egui::Frame::none()
                    .fill(COLOR_BG_CARD)
                    .rounding(Rounding::same(10.0))
                    .stroke(Stroke::new(1.0, COLOR_BORDER_SUBTLE))
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if let Some(tex) = &self.icon_texture {
                                ui.image((tex.id(), vec2(54.0, 54.0)));
                            } else {
                                let (rect, _) = ui.allocate_exact_size(vec2(54.0, 54.0), egui::Sense::hover());
                                ui.painter().rect_filled(rect, Rounding::same(8.0), COLOR_BG_VIEW);
                                ui.painter().rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.0, COLOR_BORDER_SUBTLE));
                            }

                            ui.vertical(|ui| {
                                let title = pkg.sfo.as_ref()
                                    .and_then(|s| s.title())
                                    .unwrap_or_else(|| path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown"));

                                ui.label(
                                    egui::RichText::new(title)
                                        .strong()
                                        .size(13.5)
                                        .color(COLOR_TEXT_PRIMARY),
                                );

                                ui.label(
                                    egui::RichText::new(pkg.title_id())
                                        .monospace()
                                        .size(11.0)
                                        .color(COLOR_TEXT_SECONDARY),
                                );

                                let (badge_label, badge_col) = match pkg_type {
                                    PkgType::BaseGame => ("Base Game", COLOR_BADGE_BASE),
                                    PkgType::Patch => ("Patch", COLOR_BADGE_PATCH),
                                    PkgType::Dlc => ("DLC", COLOR_BADGE_DLC),
                                };
                                render_badge(ui, badge_label, badge_col);
                            });
                        });

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(4.0);

                        egui::Grid::new("sidebar_meta_grid")
                            .num_columns(2)
                            .spacing(vec2(8.0, 4.0))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("Size:").size(11.0).color(COLOR_TEXT_MUTED));
                                ui.label(egui::RichText::new(format_bytes(pkg.header.pkg_size)).size(11.0));
                                ui.end_row();

                                ui.label(egui::RichText::new("Files:").size(11.0).color(COLOR_TEXT_MUTED));
                                let file_count = self.vfs.as_ref().map(|v| v.nodes.len()).unwrap_or(0);
                                ui.label(egui::RichText::new(format!("{}", file_count)).size(11.0));
                                ui.end_row();

                                if let Some(sfo) = &pkg.sfo {
                                    if let Some(app_ver) = sfo.app_ver() {
                                        ui.label(egui::RichText::new("Version:").size(11.0).color(COLOR_TEXT_MUTED));
                                        ui.label(egui::RichText::new(app_ver).size(11.0));
                                        ui.end_row();
                                    }
                                }
                            });
                    });
            } else {
                egui::Frame::none()
                    .fill(COLOR_BG_CARD)
                    .rounding(Rounding::same(10.0))
                    .stroke(Stroke::new(1.0, COLOR_BORDER_SUBTLE))
                    .inner_margin(egui::Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new("No Package Loaded").strong().color(COLOR_TEXT_SECONDARY));
                            ui.add_space(6.0);
                            if ui.button("Select .pkg").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("PS4 Package", &["pkg"])
                                    .pick_file()
                                {
                                    self.load_pkg(path, ctx);
                                }
                            }
                        });
                    });
            }

            ui.add_space(14.0);

            // Places / Quick Navigation
            ui.label(
                egui::RichText::new("PLACES")
                    .size(10.0)
                    .strong()
                    .color(COLOR_TEXT_MUTED),
            );
            ui.add_space(4.0);

            let is_root = self.current_path.is_empty();
            if sidebar_item(ui, "Root Directory", is_root, true).clicked() {
                self.navigate_to(String::new());
            }

            let is_sce_sys = self.current_path == "sce_sys";
            if sidebar_item(ui, "System (sce_sys)", is_sce_sys, true).clicked() {
                self.navigate_to("sce_sys".to_string());
            }

            // List top-level folders
            let top_dirs: Vec<(String, String)> = if let Some(vfs) = &self.vfs {
                vfs.list_dir("")
                    .into_iter()
                    .filter(|n| n.is_dir && n.name != "sce_sys")
                    .map(|n| (n.name.clone(), n.relative_path.clone()))
                    .collect()
            } else {
                Vec::new()
            };

            for (dir_name, dir_path) in top_dirs {
                let is_active = self.current_path == dir_path;
                if sidebar_item(ui, &dir_name, is_active, true).clicked() {
                    self.navigate_to(dir_path);
                }
            }

            ui.add_space(16.0);

            // Actions & shadPS4 Installation
            ui.label(
                egui::RichText::new("ACTIONS")
                    .size(10.0)
                    .strong()
                    .color(COLOR_TEXT_MUTED),
            );
            ui.add_space(6.0);

            let platform = current_platform();
            let shadps4_dir_text = platform
                .default_shadps4_dir()
                .map(|d| d.display().to_string())
                .unwrap_or_else(|| "Default shadPS4 directory".to_string());
            let has_pkg = self.pkg.is_some();

            ui.add_enabled_ui(has_pkg && !self.is_extracting, |ui| {
                if suggested_action_button(ui, "Install to shadPS4").on_hover_text(format!("Installs package directly into {}", shadps4_dir_text)).clicked() {
                    if let Some(pkg) = &self.pkg {
                        let pkg_type = pkg.detect_type();
                        let title_id = pkg.title_id();
                        let entitlement = pkg.entitlement_label();
                        let target = platform.resolve_target_dir(
                            pkg_type,
                            title_id,
                            entitlement.as_deref(),
                            None,
                            None,
                        );
                        self.start_extraction(target, None);
                    }
                }

                ui.add_space(6.0);

                let extract_all_btn = egui::Button::new("Extract All...").min_size(vec2(ui.available_width(), 34.0));
                if ui.add(extract_all_btn).clicked() {
                    if let Some(dest) = rfd::FileDialog::new().pick_folder() {
                        self.start_extraction(dest, None);
                    }
                }

                if self.selected_node_path.is_some() {
                    ui.add_space(6.0);
                    let extract_sel_btn = egui::Button::new("Extract Selected...").min_size(vec2(ui.available_width(), 34.0));
                    if ui.add(extract_sel_btn).clicked() {
                        self.trigger_extract_selected();
                    }
                }
            });
        });
    }

    fn render_main_view(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let parent_rect = ui.available_rect_before_wrap();

        if self.vfs.is_none() {
            // Welcome empty state
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    // Vector package emblem
                    let (icon_rect, _) = ui.allocate_exact_size(vec2(72.0, 72.0), egui::Sense::hover());
                    paint_folder_icon(ui.painter(), icon_rect);

                    ui.add_space(12.0);
                    ui.heading("Drop a PlayStation 4 .pkg file here");
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Inspect contents in-memory, preview game icons and param.sfo, or install directly into shadPS4.")
                            .color(COLOR_TEXT_SECONDARY),
                    );
                    ui.add_space(16.0);
                    if suggested_action_button(ui, "Open Package File...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PS4 Package", &["pkg"])
                            .pick_file()
                        {
                            self.load_pkg(path, ctx);
                        }
                    }
                });
            });
            return;
        }

        // Collect entries to display
        let entries: Vec<FsNode> = {
            let vfs = self.vfs.as_ref().unwrap();
            if self.search_query.trim().is_empty() {
                vfs.list_dir(&self.current_path).into_iter().cloned().collect()
            } else {
                let query = self.search_query.trim().to_ascii_lowercase();
                vfs.nodes
                    .iter()
                    .filter(|n| n.name.to_ascii_lowercase().contains(&query))
                    .cloned()
                    .collect()
            }
        };

        let mut node_to_open: Option<FsNode> = None;
        let mut context_menu_extract: Option<FsNode> = None;

        ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
            if self.view_mode == ViewMode::List {
                // Nautilus List View
                egui::Frame::none()
                    .fill(COLOR_BG_VIEW)
                    .inner_margin(egui::Margin::symmetric(14.0, 8.0))
                    .show(ui, |ui| {
                        let total_w = ui.available_width();
                        let size_col_w = 110.0;
                        let type_col_w = 130.0;

                        // Header row
                        let (header_rect, _) = ui.allocate_exact_size(vec2(total_w, 24.0), egui::Sense::hover());
                        ui.painter().text(
                            pos2(header_rect.min.x + 38.0, header_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            "Name",
                            egui::FontId::proportional(11.5),
                            COLOR_TEXT_MUTED,
                        );
                        ui.painter().text(
                            pos2(header_rect.max.x - size_col_w - type_col_w, header_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            "Size",
                            egui::FontId::proportional(11.5),
                            COLOR_TEXT_MUTED,
                        );
                        ui.painter().text(
                            pos2(header_rect.max.x - type_col_w, header_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            "Type",
                            egui::FontId::proportional(11.5),
                            COLOR_TEXT_MUTED,
                        );

                        ui.separator();
                        ui.add_space(2.0);

                        for node in &entries {
                            let is_selected = self.selected_node_path.as_deref() == Some(&node.relative_path);
                            let row_height = 36.0;
                            let desired_size = vec2(total_w, row_height);
                            let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

                            // Hover & selection backgrounds
                            if is_selected {
                                ui.painter().rect_filled(rect, Rounding::same(6.0), COLOR_SELECTION_BG);
                                ui.painter().rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.0, COLOR_BORDER_ACCENT));
                            } else if response.hovered() {
                                ui.painter().rect_filled(rect, Rounding::same(6.0), COLOR_HOVER_BG);
                            }

                            if response.clicked() {
                                self.selected_node_path = Some(node.relative_path.clone());
                            }

                            if response.double_clicked() {
                                node_to_open = Some(node.clone());
                            }

                            response.context_menu(|ui| {
                                if !node.is_dir {
                                    if ui.button("Quick Preview").clicked() {
                                        node_to_open = Some(node.clone());
                                        ui.close_menu();
                                    }
                                }
                                if ui.button("Extract to...").clicked() {
                                    context_menu_extract = Some(node.clone());
                                    ui.close_menu();
                                }
                            });

                            // Icon (centered vertically)
                            let icon_rect = Rect::from_center_size(
                                pos2(rect.min.x + 18.0, rect.center().y),
                                vec2(20.0, 20.0),
                            );
                            render_file_icon(ui, icon_rect, node.is_dir, &node.name);

                            // Name Column
                            let (name_color, font_id) = if node.is_dir {
                                (Color32::WHITE, egui::FontId::proportional(13.0))
                            } else {
                                (COLOR_TEXT_PRIMARY, egui::FontId::proportional(12.5))
                            };
                            ui.painter().text(
                                pos2(rect.min.x + 38.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                &node.name,
                                font_id,
                                name_color,
                            );

                            // Size Column
                            let size_str = if node.is_dir {
                                "-".to_string()
                            } else {
                                format_bytes(node.size)
                            };
                            ui.painter().text(
                                pos2(rect.max.x - size_col_w - type_col_w, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                &size_str,
                                egui::FontId::proportional(12.0),
                                COLOR_TEXT_SECONDARY,
                            );

                            // Type Column
                            let type_str = file_type_description(&node.name, node.is_dir);
                            ui.painter().text(
                                pos2(rect.max.x - type_col_w, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                type_str,
                                egui::FontId::proportional(12.0),
                                COLOR_TEXT_MUTED,
                            );
                        }
                    });
            } else {
                // Nautilus Grid View
                egui::Frame::none()
                    .fill(COLOR_BG_VIEW)
                    .inner_margin(egui::Margin::same(16.0))
                    .show(ui, |ui| {
                        let card_width = 110.0;
                        let card_height = 112.0;
                        let spacing = 12.0;

                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(spacing, spacing);

                            for node in &entries {
                                let is_selected = self.selected_node_path.as_deref() == Some(&node.relative_path);
                                let (rect, response) = ui.allocate_exact_size(vec2(card_width, card_height), egui::Sense::click());

                                if is_selected {
                                    ui.painter().rect_filled(rect, Rounding::same(9.0), COLOR_SELECTION_BG);
                                    ui.painter().rect_stroke(rect, Rounding::same(9.0), Stroke::new(1.0, COLOR_BORDER_ACCENT));
                                } else if response.hovered() {
                                    ui.painter().rect_filled(rect, Rounding::same(9.0), COLOR_HOVER_BG);
                                }

                                if response.clicked() {
                                    self.selected_node_path = Some(node.relative_path.clone());
                                }

                                if response.double_clicked() {
                                    node_to_open = Some(node.clone());
                                }

                                response.context_menu(|ui| {
                                    if !node.is_dir {
                                        if ui.button("Quick Preview").clicked() {
                                            node_to_open = Some(node.clone());
                                            ui.close_menu();
                                        }
                                    }
                                    if ui.button("Extract to...").clicked() {
                                        context_menu_extract = Some(node.clone());
                                        ui.close_menu();
                                    }
                                });

                                // Center Icon (larger in Grid View)
                                let icon_rect = Rect::from_center_size(
                                    pos2(rect.center().x, rect.min.y + 40.0),
                                    vec2(44.0, 44.0),
                                );
                                render_file_icon(ui, icon_rect, node.is_dir, &node.name);

                                // Truncated Name
                                let name_galley = ui.painter().layout(
                                    node.name.clone(),
                                    egui::FontId::proportional(11.5),
                                    COLOR_TEXT_PRIMARY,
                                    card_width - 12.0,
                                );
                                let name_pos = pos2(
                                    rect.center().x - name_galley.size().x / 2.0,
                                    rect.min.y + 70.0,
                                );
                                ui.painter().galley(name_pos, name_galley, COLOR_TEXT_PRIMARY);

                                // Size badge below
                                if !node.is_dir {
                                    let size_str = format_bytes(node.size);
                                    let size_galley = ui.painter().layout_no_wrap(
                                        size_str,
                                        egui::FontId::proportional(9.5),
                                        COLOR_TEXT_MUTED,
                                    );
                                    let size_pos = pos2(
                                        rect.center().x - size_galley.size().x / 2.0,
                                        rect.max.y - 14.0,
                                    );
                                    ui.painter().galley(size_pos, size_galley, COLOR_TEXT_MUTED);
                                }
                            }
                        });
                    });
            }
        });

        // Nautilus Floating Status Bar
        let status_text = if let Some(sel_path) = &self.selected_node_path {
            if let Some(vfs) = &self.vfs {
                if let Some(sel_node) = vfs.find_node(sel_path) {
                    if sel_node.is_dir {
                        format!("Selected folder: {}", sel_node.name)
                    } else {
                        format!("{} selected ({})", sel_node.name, format_bytes(sel_node.size))
                    }
                } else {
                    format!("{} items", entries.len())
                }
            } else {
                format!("{} items", entries.len())
            }
        } else {
            format!("{} items in folder", entries.len())
        };

        render_floating_pill(ui, parent_rect, &status_text);

        if let Some(node) = node_to_open {
            self.open_preview_for_node(node, ctx);
        }

        if let Some(node) = context_menu_extract {
            self.selected_node_path = Some(node.relative_path.clone());
            self.trigger_extract_selected();
        }
    }

    fn render_extraction_modal(&mut self, ctx: &egui::Context) {
        if self.is_extracting {
            egui::Window::new("Extracting Package")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                .default_size(vec2(420.0, 160.0))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        let (current, total, ref file_name) = self.extraction_progress;
                        let progress = if total > 0 {
                            current as f32 / total as f32
                        } else {
                            0.0
                        };

                        let pb = egui::ProgressBar::new(progress)
                            .show_percentage()
                            .text(format!("{}/{} files", current, total));
                        ui.add(pb);

                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(file_name)
                                .size(11.0)
                                .monospace()
                                .color(COLOR_TEXT_SECONDARY),
                        );
                    });
                });
        }

        if let Some(msg) = &self.extraction_complete_msg {
            let mut close = false;
            egui::Window::new("Extraction Status")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                .default_size(vec2(380.0, 140.0))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(msg).color(COLOR_TEXT_PRIMARY));
                        ui.add_space(16.0);
                        if ui.button("OK").clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                self.extraction_complete_msg = None;
            }
        }
    }
}

impl eframe::App for KunaiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.check_extraction_events();

        // Drag and drop PKG files into window
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                if let Some(file) = i.raw.dropped_files.first() {
                    if let Some(path) = &file.path {
                        self.load_pkg(path.clone(), ctx);
                    }
                }
            }
        });

        // Top Headerbar
        egui::TopBottomPanel::top("header_panel")
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                self.render_headerbar(ui, ctx);
            });

        // Left Sidebar (Nautilus style)
        egui::SidePanel::left("sidebar_panel")
            .frame(egui::Frame::none().fill(COLOR_BG_SIDEBAR).inner_margin(egui::Margin::same(12.0)))
            .default_width(260.0)
            .width_range(220.0..=340.0)
            .show(ctx, |ui| {
                self.render_sidebar(ui, ctx);
            });

        // Central Nautilus View
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(COLOR_BG_VIEW))
            .show(ctx, |ui| {
                self.render_main_view(ui, ctx);
            });

        // Preview inspector window
        if let Some(mut preview) = self.preview.take() {
            if preview.show(ctx) {
                self.preview = Some(preview);
            }
        }

        // Extraction modal
        self.render_extraction_modal(ctx);

        // Repaint if actively extracting
        if self.is_extracting {
            ctx.request_repaint();
        }
    }
}

/// Runs the native eframe GUI application.
pub fn run_gui(initial_pkg: Option<PathBuf>) -> Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1060.0, 690.0])
            .with_min_inner_size([760.0, 500.0])
            .with_title("Kunai - PlayStation 4 Package Explorer"),
        ..Default::default()
    };

    eframe::run_native(
        "Kunai",
        native_options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let mut app = KunaiApp::new(cc);
            if let Some(path) = initial_pkg {
                app.load_pkg(path, &cc.egui_ctx);
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow!("Failed to run GUI: {}", e))
}
