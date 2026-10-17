//! Timeline dock M3 — DESIGN.md §7.7 (X-sheet), §8.2 (dock 200 px).
//!
//! - Grid X-sheet: baris = layer (indeks 0 = paling atas), kolom = frame;
//!   header kolom `numeric`, angka 1-based (kebiasaan X-sheet).
//! - State cell §2.3/§7.7: `cel-empty`; awal exposure `cel-exposed` + strip
//!   kiri 2 px lebih terang; kelanjutan exposure hanya strip (tanpa isi);
//!   cell terpilih `cel-selected` (aksen @ 30%).
//! - Playhead `text/primary`: garis 1 px + kepala segitiga di header;
//!   scrub = seret playhead di header kolom; klik cell = pilih frame + layer.
//! - Zoom timeline `Ctrl+Scroll` (§7.7, §12.2 M3), clamp 8–64 px per kolom.
//! - Transport (§8.2): prev/next + counter `1-based` + label fps proyek.

use eframe::egui::{
    self, PointerButton, Rect, Sense, Stroke, Ui, Vec2, epaint::PathShape, pos2, vec2,
};

use flipbrush_core::anim::Animation;

use crate::icon::{self, Icon};
use crate::theme::{self, Theme};
use crate::typography::Type;

/// Tinggi dock timeline (§8.2: 200 px).
pub const TIMELINE_H: f32 = 200.0;
/// Tinggi bar transport (ikon 32 px §7).
pub(crate) const TRANSPORT_H: f32 = 32.0;
/// Lebar kolom label layer.
pub(crate) const LABEL_W: f32 = 112.0;
/// Tinggi header kolom (angka frame).
pub(crate) const HEADER_H: f32 = 20.0;
/// Tinggi satu baris layer (kelipatan grid 4 px §4.1).
pub(crate) const ROW_H: f32 = 24.0;
/// Lebar sel frame default — zoom timeline (§7.7).
pub(crate) const CELL_W: f32 = 28.0;

/// Jumlah kolom minimum yang selalu ditampilkan (proyek 1 frame tetap
/// menyediakan ruang menggambar 2 detik @ 12 fps).
const MIN_COLUMNS: u32 = 24;
/// Clamp zoom timeline (px per kolom).
const CELL_W_MIN: f32 = 8.0;
const CELL_W_MAX: f32 = 64.0;
/// Strip kiri awal exposure: 2 px (§7.7).
const STRIP_PX: f32 = 2.0;
/// Pencerahan strip awal exposure di atas `cel-exposed` (token, bukan hex).
const STRIP_BRIGHTEN: f32 = 1.5;
/// Tampilkan angka header tiap kolom bila lebar kolom ≥ ini; selebihnya
/// tiap 5 kolom agar tidak tumpang tindih.
const HEADER_FULL_W: f32 = 16.0;

/// State timeline (playhead, layer aktif, zoom kolom, scroll horizontal).
pub struct Timeline {
    /// Playhead — indeks frame 0-based.
    pub frame: u32,
    /// Indeks baris layer aktif (0 = paling atas; mengikuti urutan model).
    pub layer: usize,
    /// Lebar satu kolom frame (px) — zoom timeline §7.7.
    pub cell_w: f32,
    scroll_x: f32,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            frame: 0,
            layer: 0,
            cell_w: CELL_W,
            scroll_x: 0.0,
        }
    }
}

impl Timeline {
    /// Transport bar + grid X-sheet (§7.7) ke dalam dock bawah.
    pub fn show(&mut self, ui: &mut Ui, theme: &Theme, anim: &Animation) {
        self.transport(ui, theme, anim);
        // Grid menempel persis di bawah transport (tanpa jarak baris).
        ui.spacing_mut().item_spacing.y = 0.0;

        let area = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(area, Sense::click_and_drag());
        let rect = area;
        if !rect.is_positive() {
            return;
        }
        let ctx = ui.ctx().clone();

        let grid_left = rect.left() + LABEL_W;
        let view_w = (rect.width() - LABEL_W).max(1.0);
        let grid_top = rect.top() + HEADER_H;
        let visible_cols = ((view_w / self.cell_w).ceil() as u32).max(1);
        let cols = anim.frames().max(MIN_COLUMNS).max(visible_cols);
        let max_scroll = (cols as f32 * self.cell_w - view_w).max(0.0);

        // — Zoom (Ctrl+Scroll) & scroll horizontal (§7.7) —
        if resp.hovered() {
            let (zoom, scroll) = ctx.input(|i| (i.zoom_delta(), i.smooth_scroll_delta()));
            if (zoom - 1.0).abs() > 1e-4 {
                self.cell_w = (self.cell_w * zoom).clamp(CELL_W_MIN, CELL_W_MAX);
            } else if scroll != Vec2::ZERO {
                self.scroll_x = (self.scroll_x - scroll.x - scroll.y).clamp(0.0, max_scroll);
            }
        }

        // — Klik: header = scrub ke kolom; label layer = pilih baris;
        //   cell = pilih frame + layer (§7.7) —
        if resp.clicked_by(PointerButton::Primary)
            && let Some(p) = resp.interact_pointer_pos()
        {
            if p.x >= grid_left {
                self.frame = frame_at(p.x, grid_left, self.scroll_x, self.cell_w, cols);
                if p.y >= grid_top
                    && let Some(row) = row_at(p.y, grid_top, anim.layers().len())
                {
                    self.layer = row;
                }
            } else if let Some(row) = row_at(p.y, grid_top, anim.layers().len()) {
                self.layer = row;
            }
        }
        // — Scrub: seret playhead di header kolom —
        let press_origin = ctx.input(|i| i.pointer.press_origin());
        if resp.dragged_by(PointerButton::Primary)
            && let Some(origin) = press_origin
            && origin.y < grid_top
            && origin.x >= grid_left
            && let Some(p) = resp.interact_pointer_pos()
        {
            self.frame = frame_at(p.x, grid_left, self.scroll_x, self.cell_w, cols);
        }

        // Playhead selalu terlihat setelah navigasi.
        let ph_left = self.frame as f32 * self.cell_w - self.scroll_x;
        if ph_left < 0.0 {
            self.scroll_x = self.frame as f32 * self.cell_w;
        } else if ph_left + self.cell_w > view_w {
            self.scroll_x = (self.frame as f32 + 1.0) * self.cell_w - view_w;
        }
        self.scroll_x = self.scroll_x.clamp(0.0, max_scroll);

        let first_col = (self.scroll_x / self.cell_w).floor() as u32;
        let last_visible = ((self.scroll_x + view_w) / self.cell_w).ceil() as u32;
        self.paint(ui, theme, anim, rect, first_col, last_visible.min(cols));
    }

    /// Bar transport §8.2: prev/next + counter + fps (tinggi pasti 32 px).
    fn transport(&mut self, ui: &mut Ui, theme: &Theme, anim: &Animation) {
        let max_frame = anim.frames().max(MIN_COLUMNS) - 1;
        ui.allocate_ui(Vec2::new(ui.available_width(), TRANSPORT_H), |ui| {
            ui.horizontal(|ui| {
                ui.add_space(theme::spacing::PANEL);
                ui.spacing_mut().item_spacing.x = theme::spacing::GAP_CONTROL;
                if icon::icon_button(ui, theme, Icon::PrevFrame, "Frame sebelumnya", false)
                    .clicked()
                {
                    self.frame = self.frame.saturating_sub(1);
                }
                if icon::icon_button(ui, theme, Icon::NextFrame, "Frame berikutnya", false)
                    .clicked()
                {
                    self.frame = (self.frame + 1).min(max_frame);
                }
                ui.label(
                    Type::Numeric
                        .rich(format!("{} / {}", self.frame + 1, anim.frames()))
                        .color(theme.text_primary),
                );
                ui.label(
                    Type::Numeric
                        .rich(format!("{} fps", anim.fps))
                        .color(theme.text_secondary),
                );
            });
        });
    }

    /// Gambar seluruh isi dock: label layer, header kolom, cell, playhead.
    fn paint(
        &self,
        ui: &mut Ui,
        theme: &Theme,
        anim: &Animation,
        rect: Rect,
        first_col: u32,
        last_visible: u32,
    ) {
        let grid_left = rect.left() + LABEL_W;
        let grid_top = rect.top() + HEADER_H;
        let painter = ui.painter();
        // §2.3: kolom kosong = cel-empty; chrome = surface-1.
        painter.rect_filled(rect, 0.0, theme.surface_1);
        painter.rect_filled(
            Rect::from_min_max(pos2(grid_left, grid_top), rect.max),
            0.0,
            theme.cel_empty,
        );

        // Baris layer: label kiri + cell exposure.
        for row in 0..anim.layers().len() {
            let Some(layer_id) = anim.layer_id(row) else {
                continue;
            };
            let y = grid_top + row as f32 * ROW_H;
            for f in first_col..last_visible {
                let x = grid_left + f as f32 * self.cell_w - self.scroll_x;
                let cell = Rect::from_min_size(pos2(x, y), vec2(self.cell_w, ROW_H));
                let strip = Rect::from_min_size(cell.min, vec2(STRIP_PX.min(self.cell_w), ROW_H));
                if let Some(exposure) = anim.exposure_at(layer_id, f) {
                    if exposure.start == f {
                        // Awal exposure: isi + strip kiri 2 px lebih terang.
                        painter.rect_filled(cell.shrink2(Vec2::splat(0.5)), 0.0, theme.cel_exposed);
                        painter.rect_filled(
                            strip,
                            0.0,
                            theme.cel_exposed.gamma_multiply(STRIP_BRIGHTEN),
                        );
                    } else {
                        // Kelanjutan exposure: hanya strip penghubung.
                        painter.rect_filled(strip, 0.0, theme.cel_shared);
                    }
                }
                // Cell terpilih (playhead × layer aktif) — aksen @ 30%.
                if f == self.frame && row == self.layer {
                    painter.rect_filled(cell.shrink2(Vec2::splat(0.5)), 0.0, theme.cel_selected);
                }
            }
            // Pemisah baris + label nama layer.
            painter.line_segment(
                [pos2(grid_left, y + ROW_H), pos2(rect.right(), y + ROW_H)],
                Stroke::new(1.0, theme.border_default),
            );
            if row == self.layer {
                painter.rect_filled(
                    Rect::from_min_size(pos2(rect.left(), y), vec2(LABEL_W - 1.0, ROW_H)),
                    0.0,
                    theme.surface_3,
                );
            }
            let name = &anim.layers()[row].name;
            painter.text(
                pos2(rect.left() + theme::spacing::GAP_CONTROL, y + ROW_H * 0.5),
                egui::Align2::LEFT_CENTER,
                name,
                Type::LabelSm.font_id(),
                if row == self.layer {
                    theme.text_primary
                } else {
                    theme.text_secondary
                },
            );
        }

        // Header kolom: angka frame `numeric` 1-based.
        painter.rect_filled(
            Rect::from_min_max(rect.min, pos2(rect.right(), grid_top)),
            0.0,
            theme.surface_1,
        );
        painter.line_segment(
            [pos2(grid_left, grid_top), pos2(rect.right(), grid_top)],
            Stroke::new(1.0, theme.border_default),
        );
        let per_number = self.cell_w >= HEADER_FULL_W;
        for f in first_col..last_visible {
            if !per_number && !f.is_multiple_of(5) {
                continue;
            }
            let x = grid_left + f as f32 * self.cell_w - self.scroll_x;
            painter.text(
                pos2(x + self.cell_w * 0.5, rect.top() + HEADER_H * 0.5),
                egui::Align2::CENTER_CENTER,
                format!("{}", f + 1),
                Type::Numeric.font_id(),
                if f == self.frame {
                    theme.text_primary
                } else {
                    theme.text_secondary
                },
            );
        }

        // Playhead §7.7: garis 1 px + kepala segitiga di header.
        let x = grid_left + self.frame as f32 * self.cell_w - self.scroll_x;
        painter.line_segment(
            [pos2(x + 0.5, grid_top), pos2(x + 0.5, rect.bottom())],
            Stroke::new(1.0, theme.playhead),
        );
        let head_top = rect.top() + 2.0;
        painter.add(PathShape::convex_polygon(
            vec![
                pos2(x, head_top),
                pos2(x + 7.0, head_top + 8.0),
                pos2(x - 7.0, head_top + 8.0),
            ],
            theme.playhead,
            Stroke::NONE,
        ));
    }
}

/// Indeks kolom untuk posisi x layar.
fn frame_at(x: f32, grid_left: f32, scroll_x: f32, cell_w: f32, cols: u32) -> u32 {
    let f = ((x - grid_left + scroll_x) / cell_w).floor() as i32;
    f.clamp(0, cols as i32 - 1) as u32
}

/// Baris layer untuk posisi y layar.
fn row_at(y: f32, grid_top: f32, rows: usize) -> Option<usize> {
    if y < grid_top {
        return None;
    }
    let row = ((y - grid_top) / ROW_H) as usize;
    (row < rows).then_some(row)
}
