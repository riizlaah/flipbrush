//! Tool aktif & tool rail desktop — DESIGN.md §7.4 (M2: brush, eraser,
//! fill, eyedropper; tool lain menyusul di M4).
//!
//! - Rail 48 px di kiri, `bg/surface-1`, item 40×40.
//! - Kepala rail: **kotak warna aktif** — klik = tampilkan tab Color di
//!   dock kanan (§7.6); tanpa kode hex (angka ada di panel Color §1.4).
//! - Item aktif: `ghost-active` + **strip aksen 2 px di tepi kiri rail**.
//! - Grup (kotak warna | tool) dipisah divider `border/default` (§7.4).
//! - Shortcut (§7.1 icon-only wajib punya tooltip + shortcut; §7.9):
//!   `B` brush, `E` eraser, `G` fill, `I` eyedropper.

use eframe::egui::{
    Color32, CornerRadius, Key, Rect, Response, Sense, Stroke, Ui, Vec2, epaint::StrokeKind, pos2,
};
use flipbrush_core::raster::Rgba;

use crate::icon::{self, Icon};
use crate::theme::{Theme, radius, spacing};

/// Lebar rail tool (§7.4: 48 px).
pub const RAIL_W: f32 = 48.0;
/// Sisi item rail (§7.4: 40×40 px).
const ITEM_PX: f32 = 40.0;
/// Strip aksen item aktif (§7.4: 2 px di tepi kiri rail).
const ACTIVE_STRIP_PX: f32 = 2.0;
/// Jarak isi kotak warna ke tepi slot — swatch 32 px di slot 40 px.
const KOTAK_PAD_PX: f32 = 4.0;

/// Tool yang tersedia di M2 (§12.2 — tool: brush, eraser, fill, eyedropper).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Brush,
    Eraser,
    Fill,
    Eyedropper,
}

impl Tool {
    /// Urutan item di rail §7.4.
    pub const ALL: [Tool; 4] = [Self::Brush, Self::Eraser, Self::Fill, Self::Eyedropper];

    /// Ikon tool (§5).
    pub const fn icon(self) -> Icon {
        match self {
            Self::Brush => Icon::Brush,
            Self::Eraser => Icon::Eraser,
            Self::Fill => Icon::Fill,
            Self::Eyedropper => Icon::Eyedropper,
        }
    }

    /// Nama tampilan (§7.9 tooltip).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Brush => "Brush",
            Self::Eraser => "Eraser",
            Self::Fill => "Fill",
            Self::Eyedropper => "Eyedropper",
        }
    }

    /// Shortcut keyboard desktop (§7.1: icon-only wajib shortcut).
    pub const fn key(self) -> Key {
        match self {
            Self::Brush => Key::B,
            Self::Eraser => Key::E,
            Self::Fill => Key::G,
            Self::Eyedropper => Key::I,
        }
    }

    /// Huruf shortcut (§7.9: tooltip menyertakan shortcut).
    pub const fn shortcut(self) -> &'static str {
        match self {
            Self::Brush => "B",
            Self::Eraser => "E",
            Self::Fill => "G",
            Self::Eyedropper => "I",
        }
    }

    /// Label tooltip: `"Brush (B)"` (§7.9).
    pub fn tooltip(self) -> String {
        format!("{} ({})", self.name(), self.shortcut())
    }
}

/// Hasil gambar rail [`show`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RailOutcome {
    /// Tool yang dipilih (klik).
    pub tool: Tool,
    /// `true` bila kotak warna di kepala rail diklik — pemanggil
    /// menampilkan tab Color di dock kanan (§7.6).
    pub warna_diklik: bool,
}

/// Gambar isi rail ke `ui` (panel sudah dialokasikan pemanggil dengan
/// lebar [`RAIL_W`]); mengembalikan tool yang dipilih (klik) dan sinyal
/// klik kotak warna.
pub fn show(ui: &mut Ui, theme: &Theme, current: Tool, warna: Rgba) -> RailOutcome {
    let mut selected = current;
    let mut active_rect: Option<Rect> = None;
    let mut warna_diklik = false;

    ui.vertical_centered(|ui| {
        ui.add_space(spacing::GAP_CONTROL);
        // §7.4: grup terpisah — kotak warna di paling atas rail (§7.3:
        // kotak warna aktif, klik = tampilkan tab Color di dock §7.6).
        warna_diklik = kotak_warna(ui, theme, warna).clicked();
        ui.add_space(spacing::GAP_CONTROL);
        divider_grup(ui, theme);
        ui.add_space(spacing::GAP_CONTROL);
        for tool in Tool::ALL {
            let response = icon::icon_button_sized(
                ui,
                theme,
                tool.icon(),
                &tool.tooltip(),
                tool == current,
                ITEM_PX,
            );
            if response.clicked() {
                selected = tool;
            }
            if tool == current {
                active_rect = Some(response.rect);
            }
            ui.add_space(spacing::ICON_LABEL);
        }
    });

    // §7.4: strip aksen 2 px di tepi kiri rail untuk item aktif.
    if let Some(rect) = active_rect {
        let strip = Rect::from_min_size(
            pos2(ui.max_rect().left(), rect.top()),
            Vec2::new(ACTIVE_STRIP_PX, rect.height()),
        );
        ui.painter().rect_filled(strip, 0.0, theme.accent_brand);
    }

    RailOutcome {
        tool: selected,
        warna_diklik,
    }
}

/// Kotak warna aktif di kepala rail (§7.3/§7.4) — slot 40×40 seperti item
/// rail, isi = warna aktif **tanpa kode hex** (§1.4: angka jelas ada di
/// input panel Color). Klik = tampilkan tab Color di dock kanan (§7.6);
/// tooltip §7.9, focus ring §9, accessible name §9.
fn kotak_warna(ui: &mut Ui, theme: &Theme, warna: Rgba) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(ITEM_PX), Sense::click());
    let kotak = rect.shrink(KOTAK_PAD_PX);
    ui.painter().rect_filled(
        kotak,
        radius::SM,
        Color32::from_rgba_unmultiplied(warna.r, warna.g, warna.b, warna.a),
    );
    // Border kontras agar swatch tetap terbaca di atas `surface_1`
    // (termasuk warna gelap); lebih terang saat hover sebagai sinyal klik.
    let border = if resp.hovered() {
        theme.text_primary
    } else {
        theme.border_strong
    };
    ui.painter().rect_stroke(
        kotak,
        CornerRadius::same(radius::SM),
        Stroke::new(1.0, border),
        StrokeKind::Middle,
    );
    if resp.has_focus() {
        // §9: focus ring 2 px `accent/brand`, offset 2 px dari tepi.
        let grow = theme.focus_ring_offset + theme.focus_ring.width * 0.5;
        ui.painter().rect_stroke(
            rect.expand(grow),
            CornerRadius::same(radius::SM),
            theme.focus_ring,
            StrokeKind::Middle,
        );
    }
    let resp = resp.on_hover_text("Warna aktif — klik untuk memilih warna");
    let _ = ui.ctx().accesskit_node_builder(resp.id, |node| {
        node.set_label("Kotak warna aktif — buka panel Color");
    });
    resp
}

/// Divider antar grup di rail (§7.4: `border/default`).
fn divider_grup(ui: &mut Ui, theme: &Theme) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, theme.border_default);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_menyertakan_shortcut_section_7_9() {
        assert_eq!(Tool::Brush.tooltip(), "Brush (B)");
        assert_eq!(Tool::Eraser.tooltip(), "Eraser (E)");
        assert_eq!(Tool::Fill.tooltip(), "Fill (G)");
        assert_eq!(Tool::Eyedropper.tooltip(), "Eyedropper (I)");
    }

    #[test]
    fn rail_mengikuti_section_7_4() {
        assert_eq!(RAIL_W, 48.0);
        assert_eq!(ITEM_PX, 40.0);
        assert_eq!(ACTIVE_STRIP_PX, 2.0);
        assert_eq!(KOTAK_PAD_PX, 4.0);
    }
}
