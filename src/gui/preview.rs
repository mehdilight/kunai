//! Quick preview inspector for PS4 PKG contents (param.sfo, icon0.png, eboot.bin, etc.).

use anyhow::Result;
use egui::{vec2, ScrollArea, Window};
use std::fs::File;

use super::theme::*;
use super::widgets::format_bytes;
use crate::core::psf::{Psf, PsfValue};
use crate::core::virtual_fs::{FsNode, VirtualFs};

pub enum PreviewContent {
    Image {
        texture: egui::TextureHandle,
        width: u32,
        height: u32,
    },
    Sfo {
        sfo: Psf,
    },
    Text {
        text: String,
    },
    Binary {
        first_bytes_hex: String,
        magic: String,
    },
}

pub struct PreviewModal {
    pub node: FsNode,
    pub content: PreviewContent,
    pub raw_bytes: Vec<u8>,
}

impl PreviewModal {
    pub fn load(
        vfs: &VirtualFs,
        pkg_file: &mut File,
        node: FsNode,
        ctx: &egui::Context,
    ) -> Result<Self> {
        let bytes = vfs.read_file_content(pkg_file, &node)?;

        let lower_name = node.name.to_ascii_lowercase();

        let content = if lower_name.ends_with(".png") || lower_name.ends_with(".jpg") || lower_name.ends_with(".jpeg") {
            if let Ok(img) = image::load_from_memory(&bytes) {
                let width = img.width();
                let height = img.height();
                let rgba = img.to_rgba8();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [width as usize, height as usize],
                    rgba.as_flat_samples().as_slice(),
                );
                let texture = ctx.load_texture(
                    format!("preview_{}", node.name),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                PreviewContent::Image { texture, width, height }
            } else {
                PreviewContent::Binary {
                    first_bytes_hex: format_hex_dump(&bytes, 256),
                    magic: format_magic(&bytes),
                }
            }
        } else if lower_name.ends_with(".sfo") {
            if let Ok(sfo) = Psf::parse(&bytes) {
                PreviewContent::Sfo { sfo }
            } else {
                PreviewContent::Binary {
                    first_bytes_hex: format_hex_dump(&bytes, 256),
                    magic: format_magic(&bytes),
                }
            }
        } else if lower_name.ends_with(".xml") || lower_name.ends_with(".json") || lower_name.ends_with(".txt") {
            let text = String::from_utf8_lossy(&bytes).to_string();
            PreviewContent::Text { text }
        } else {
            PreviewContent::Binary {
                first_bytes_hex: format_hex_dump(&bytes, 256),
                magic: format_magic(&bytes),
            }
        };

        Ok(Self {
            node,
            content,
            raw_bytes: bytes,
        })
    }

    /// Renders the preview window. Returns true if the window should remain open.
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut is_open = true;

        Window::new(format!("Preview: {}", self.node.name))
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_size(vec2(580.0, 480.0))
            .min_size(vec2(400.0, 300.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&self.node.name)
                            .strong()
                            .size(16.0)
                            .color(COLOR_TEXT_PRIMARY),
                    );
                    ui.label(
                        egui::RichText::new(format!("({})", format_bytes(self.node.size)))
                            .size(13.0)
                            .color(COLOR_TEXT_SECONDARY),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Save File...").clicked() {
                            if let Some(dest) = rfd::FileDialog::new()
                                .set_file_name(&self.node.name)
                                .save_file()
                            {
                                let _ = std::fs::write(dest, &self.raw_bytes);
                            }
                        }
                    });
                });

                ui.separator();

                match &self.content {
                    PreviewContent::Image { texture, width, height } => {
                        ui.label(
                            egui::RichText::new(format!("Resolution: {} x {} px", width, height))
                                .size(12.0)
                                .color(COLOR_TEXT_SECONDARY),
                        );
                        ui.add_space(8.0);

                        ScrollArea::both().show(ui, |ui| {
                            let available_size = ui.available_size();
                            let aspect = *width as f32 / *height as f32;
                            let mut render_w = available_size.x.min(*width as f32);
                            let mut render_h = render_w / aspect;

                            if render_h > available_size.y && available_size.y > 50.0 {
                                render_h = available_size.y;
                                render_w = render_h * aspect;
                            }

                            ui.centered_and_justified(|ui| {
                                ui.image((texture.id(), vec2(render_w, render_h)));
                            });
                        });
                    }

                    PreviewContent::Sfo { sfo } => {
                        ScrollArea::vertical().show(ui, |ui| {
                            ui.heading("param.sfo Entries");
                            ui.add_space(6.0);

                            egui::Grid::new("sfo_grid")
                                .striped(true)
                                .min_col_width(140.0)
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new("Key").strong().color(COLOR_ACCENT_BLUE));
                                    ui.label(egui::RichText::new("Type").strong().color(COLOR_TEXT_SECONDARY));
                                    ui.label(egui::RichText::new("Value").strong().color(COLOR_TEXT_PRIMARY));
                                    ui.end_row();

                                    for (key, val) in &sfo.entries {
                                        ui.label(egui::RichText::new(key).color(COLOR_TEXT_PRIMARY));
                                        let type_str = match val {
                                            PsfValue::Text(_) => "String",
                                            PsfValue::Integer(_) => "Integer (i32)",
                                            PsfValue::Binary(_) => "Binary Data",
                                        };
                                        ui.label(egui::RichText::new(type_str).color(COLOR_TEXT_SECONDARY));

                                        let val_str = match val {
                                            PsfValue::Text(s) => s.clone(),
                                            PsfValue::Integer(i) => format!("{} (0x{:X})", i, i),
                                            PsfValue::Binary(b) => format!("{} bytes", b.len()),
                                        };
                                        ui.label(egui::RichText::new(val_str).color(COLOR_TEXT_PRIMARY));
                                        ui.end_row();
                                    }
                                });
                        });
                    }

                    PreviewContent::Text { text } => {
                        ScrollArea::vertical().show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut text.as_str())
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .interactive(false),
                            );
                        });
                    }

                    PreviewContent::Binary { first_bytes_hex, magic } => {
                        ScrollArea::vertical().show(ui, |ui| {
                            if !magic.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!("Detected signature: {}", magic))
                                        .strong()
                                        .color(COLOR_ACCENT_BLUE),
                                );
                                ui.add_space(4.0);
                            }

                            ui.label(egui::RichText::new("Hex Preview (First 256 bytes):").color(COLOR_TEXT_SECONDARY));
                            ui.add_space(4.0);

                            ui.add(
                                egui::TextEdit::multiline(&mut first_bytes_hex.as_str())
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .interactive(false),
                            );
                        });
                    }
                }
            });

        is_open
    }
}

fn format_magic(bytes: &[u8]) -> String {
    if bytes.len() >= 4 && &bytes[0..4] == b"\x7FELF" {
        "ELF 64-bit Executable (PlayStation 4)".to_string()
    } else if bytes.len() >= 4 && &bytes[0..4] == b"\0PSF" {
        "PSF (PlayStation System File)".to_string()
    } else if bytes.len() >= 4 && &bytes[0..4] == b"\x89PNG" {
        "Portable Network Graphics (PNG)".to_string()
    } else {
        String::new()
    }
}

fn format_hex_dump(bytes: &[u8], max_len: usize) -> String {
    let limit = bytes.len().min(max_len);
    let mut out = String::new();

    for chunk_offset in (0..limit).step_by(16) {
        let chunk_end = (chunk_offset + 16).min(limit);
        let slice = &bytes[chunk_offset..chunk_end];

        out.push_str(&format!("{:04X}  ", chunk_offset));

        // Hex part
        for i in 0..16 {
            if i < slice.len() {
                out.push_str(&format!("{:02X} ", slice[i]));
            } else {
                out.push_str("   ");
            }
            if i == 7 {
                out.push(' ');
            }
        }

        out.push_str(" |");
        // ASCII part
        for b in slice {
            if b.is_ascii_graphic() || *b == b' ' {
                out.push(*b as char);
            } else {
                out.push('.');
            }
        }
        out.push_str("|\n");
    }

    out
}
