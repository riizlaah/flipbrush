//! Tool aktif & tool rail desktop — DESIGN.md §7.4 (M2: brush, eraser,
//! fill, eyedropper; tool lain menyusul di M4).
//!
//! - Rail 48 px di kiri, `bg/surface-1`, item 40×40.
//! - Item aktif: `ghost-active` + **strip aksen 2 px di tepi kiri rail**.
//! - Shortcut (§7.1 icon-only wajib punya tooltip + shortcut; §7.9):
//!   `B` brush, `E` eraser, `G` fill, `I` eyedropper.

use eframe::egui::{Key, Rect, Ui, Vec2, pos2};

use crate::icon::{self, Icon};
use crate::theme::{Theme, spacing};

/// Lebar rail tool (§7.4: 48 px).
pub const RAIL_W: f32 = 48.0;
/// Sisi item rail (§7.4: 40×40 px).
const ITEM_PX: f32 = 40.0;
/// Strip aksen item aktif (§7.4: 2 px di tepi kiri rail).
const ACTIVE_STRIP_PX: f32 = 2.0;

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

    /// Label tooltip: `"Brush (B)"` (§7.9).
    pub fn tooltip(self) -> String {
        let letter = match self.key() {
            Key::B => "B",
            Key::E => "E",
            Key::G => "G",
            Key::I => "I",
            _ => "?",
        };
        format!("{} ({letter})", self.name())
    }
}

/// Gambar isi rail ke `ui` (panel sudah dialokasikan pemanggil dengan
/// lebar [`RAIL_W`]); mengembalikan tool yang dipilih (klik).
pub fn show(ui: &mut Ui, theme: &Theme, current: Tool) -> Tool {
    let mut selected = current;
    let mut active_rect: Option<Rect> = None;

    ui.vertical_centered(|ui| {
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

    selected
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
    }
}
