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
    Color32, CornerRadius, Id, Key, Rect, Response, Sense, Stroke, Ui, Vec2, epaint::StrokeKind,
    pos2,
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

/// Tool yang tersedia (M2: brush/eraser/fill/eyedropper; M4d: shape,
/// marquee, lasso, wand, move — §12.2 toolset lanjutan).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Brush,
    Eraser,
    Fill,
    Eyedropper,
    /// Bentuk terisi persegi/elips (M4d tahap 3) — drag; warna aktif,
    /// opacity brush; toggle Persegi/Ellipse di options bar.
    Shape,
    /// Seleksi persegi (M4d tahap 1) — drag membentuk marching ants; klik
    /// polos menghapus seleksi.
    Marquee,
    /// Seleksi poligon bebas (M4d tahap 2) — drag jalur lasso; klik polos
    /// menghapus seleksi.
    Lasso,
    /// Seleksi region flood-fill (M4d tahap 2) — klik area warna; kontrol
    /// toleransi di options bar (sama dengan Fill).
    Wand,
    /// Transformasi pindah (M4d tahap 3) — drag isi seleksi ke posisi
    /// baru (satu entri undo; geometri seleksi ikut bergeser).
    Move,
}

impl Tool {
    /// Urutan item di rail §7.4 — bentuk bersama tool gambar, keluarga
    /// seleksi + move di belakang divider sendiri.
    pub const ALL: [Tool; 9] = [
        Self::Brush,
        Self::Eraser,
        Self::Fill,
        Self::Eyedropper,
        Self::Shape,
        Self::Marquee,
        Self::Lasso,
        Self::Wand,
        Self::Move,
    ];

    /// Ikon tool (§5).
    pub const fn icon(self) -> Icon {
        match self {
            Self::Brush => Icon::Brush,
            Self::Eraser => Icon::Eraser,
            Self::Fill => Icon::Fill,
            Self::Eyedropper => Icon::Eyedropper,
            Self::Shape => Icon::Shape,
            Self::Marquee => Icon::Marquee,
            Self::Lasso => Icon::Lasso,
            Self::Wand => Icon::Wand,
            Self::Move => Icon::Move,
        }
    }

    /// Nama tampilan (§7.9 tooltip).
    pub const fn name(self) -> &'static str {
        match self {
            Self::Brush => "Brush",
            Self::Eraser => "Eraser",
            Self::Fill => "Fill",
            Self::Eyedropper => "Eyedropper",
            Self::Shape => "Shape",
            Self::Marquee => "Marquee",
            Self::Lasso => "Lasso",
            Self::Wand => "Wand",
            Self::Move => "Move",
        }
    }

    /// Shortcut keyboard desktop (§7.1: icon-only wajib shortcut).
    pub const fn key(self) -> Key {
        match self {
            Self::Brush => Key::B,
            Self::Eraser => Key::E,
            Self::Fill => Key::G,
            Self::Eyedropper => Key::I,
            Self::Shape => Key::U,
            Self::Marquee => Key::M,
            Self::Lasso => Key::L,
            Self::Wand => Key::W,
            Self::Move => Key::X,
        }
    }

    /// Huruf shortcut (§7.9: tooltip menyertakan shortcut).
    pub const fn shortcut(self) -> &'static str {
        match self {
            Self::Brush => "B",
            Self::Eraser => "E",
            Self::Fill => "G",
            Self::Eyedropper => "I",
            Self::Shape => "U",
            Self::Marquee => "M",
            Self::Lasso => "L",
            Self::Wand => "W",
            Self::Move => "X",
        }
    }

    /// `true` bila tool keluarga seleksi/transform (M4d: marquee/lasso/
    /// wand/move) — rail mengelompokkannya di balik divider §7.4.
    pub const fn is_seleksi(self) -> bool {
        matches!(self, Self::Marquee | Self::Lasso | Self::Wand | Self::Move)
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
        // kotak warna aktif, klik = tampilkan tab Color di dock §7.6);
        // §9: `Enter`/`Space` mengaktifkan saat kotak berfokus keyboard.
        let resp_warna = kotak_warna(ui, theme, warna);
        warna_diklik = icon::teraktivasi(ui, &resp_warna);
        ui.add_space(spacing::GAP_CONTROL);
        divider_grup(ui, theme);
        ui.add_space(spacing::GAP_CONTROL);
        // Grup seleksi (M4d) di balik divider sendiri (§7.4: grup
        // dipisah divider); tool gambar dulu, baru tool seleksi.
        let mut grup_sebelum: Option<bool> = None;
        for tool in Tool::ALL {
            let seleksi = tool.is_seleksi();
            if grup_sebelum == Some(false) && seleksi {
                ui.add_space(spacing::GAP_CONTROL);
                divider_grup(ui, theme);
                ui.add_space(spacing::GAP_CONTROL);
            }
            grup_sebelum = Some(seleksi);
            let response = icon::icon_button_sized(
                ui,
                theme,
                tool.icon(),
                &tool.tooltip(),
                tool == current,
                ITEM_PX,
            );
            // §9: aktivasi `Enter`/`Space` = klik (terdaftar tab order di
            // `icon_button_sized`).
            if icon::teraktivasi(ui, &response) {
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
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(ITEM_PX), Sense::hover());
    // Id global (tanpa prefiks rantai Ui): tab order §9 dan tes headless
    // bisa menyasar kotak ini secara deterministik.
    let resp = ui.interact(rect, Id::new("kotak-warna"), Sense::click());
    // §9: ikut tab order — `Tab`/`Shift+Tab` berpindah ke sini; aktivasi
    // `Enter`/`Space` lewat `icon::teraktivasi` (lihat `show`).
    ui.ctx()
        .memory_mut(|m| m.interested_in_focus(resp.id, resp.layer_id));
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
