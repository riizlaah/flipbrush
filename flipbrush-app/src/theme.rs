//! Token tema Flipbrush — satu-satunya tempat hex literal diizinkan
//! (DESIGN.md §10: "Tidak boleh ada hex literal di luar `theme.rs`").
//!
//! Milestone M0 memuat skala netral dark + aksen dasar (DESIGN.md §2.1–2.2).
//! Kelengkapan token (light theme §2.5, warna animasi §2.3, accent picker
//! §2.4) menyusul di M1/M5.

use eframe::egui::{Color32, Stroke};

/// Token tema yang sudah di-resolve untuk satu sesi aplikasi.
pub struct Theme {
    // §2.1 — skala netral (dark, default)
    pub canvas_surround: Color32,
    pub surface_0: Color32,
    pub surface_1: Color32,
    pub surface_2: Color32,
    pub surface_3: Color32,
    pub border_default: Color32,
    pub border_strong: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_disabled: Color32,
    // §2.2 — aksen (satu aksen tunggal untuk chrome)
    pub accent_brand: Color32,
}

impl Theme {
    /// Resolve tema dark default saat startup (DESIGN.md §10).
    pub fn dark() -> Self {
        Self {
            canvas_surround: Color32::from_rgb(0x3B, 0x3B, 0x3E),
            surface_0: Color32::from_rgb(0x1C, 0x1C, 0x1F),
            surface_1: Color32::from_rgb(0x26, 0x26, 0x29),
            surface_2: Color32::from_rgb(0x2F, 0x2F, 0x33),
            surface_3: Color32::from_rgb(0x3A, 0x3A, 0x40),
            border_default: Color32::from_rgb(0x35, 0x35, 0x3A),
            border_strong: Color32::from_rgb(0x4A, 0x4A, 0x51),
            text_primary: Color32::from_rgb(0xED, 0xED, 0xEF),
            text_secondary: Color32::from_rgb(0x9A, 0x9A, 0xA1),
            text_disabled: Color32::from_rgb(0x6A, 0x6A, 0x70),
            accent_brand: Color32::from_rgb(0x6E, 0x7B, 0xF2),
        }
    }

    /// Terapkan token ke `egui::Visuals` — komponen memakai token, bukan hex.
    pub fn apply(&self, visuals: &mut eframe::egui::Visuals) {
        visuals.panel_fill = self.surface_1;
        visuals.window_fill = self.surface_1;
        visuals.extreme_bg_color = self.surface_2;
        visuals.faint_bg_color = self.surface_2;
        visuals.override_text_color = Some(self.text_primary);
        visuals.hyperlink_color = self.accent_brand;
        visuals.selection.bg_fill = self.accent_brand;
        visuals.window_stroke = Stroke::new(1.0, self.border_default);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_theme_has_distinct_surfaces() {
        let t = Theme::dark();
        assert_ne!(t.surface_0, t.surface_1);
        assert_ne!(t.surface_1, t.surface_2);
        assert_ne!(t.surface_2, t.surface_3);
    }

    #[test]
    fn text_primary_contrasts_against_surface_1() {
        // DESIGN.md §9: kontras minimal 4.5:1 untuk body text.
        fn relative_luma(c: Color32) -> f32 {
            let channel = |v: u8| {
                let s = v as f32 / 255.0;
                if s <= 0.040_45 {
                    s / 12.92
                } else {
                    ((s + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
        }
        let t = Theme::dark();
        let l1 = relative_luma(t.text_primary);
        let l2 = relative_luma(t.surface_1);
        let ratio = (l1.max(l2) + 0.05) / (l1.min(l2) + 0.05);
        assert!(
            ratio >= 4.5,
            "kontras text/primary di atas surface/1 = {ratio:.2}:1 (min 4.5:1)"
        );
    }
}
