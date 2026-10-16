//! Tipografi Flipbrush — DESIGN.md §3.
//!
//! Font **variabel** Inter + JetBrains Mono dibundel di `assets/fonts/`
//! (lisensi OFL ikut di repo). egui tidak punya konsep berat teks — weight
//! 400/500/600 dicapai dengan mendaftarkan keluarga font terpisah per weight
//! (`FontFamily::Name`) di mana tiap entri `font_data` adalah file font yang
//! sama dengan `FontTweak::coords` sumbu `wght` berbeda.
//!
//! Catatan: `RichText::strong()` di egui hanya mengubah warna — jangan
//! dipakai untuk weight; gunakan [`Type::rich`] / [`Type::font_id`].

use std::sync::{Arc, LazyLock};

use eframe::egui::{
    FontData, FontFamily, FontId, FontTweak, RichText, Style, TextStyle,
    epaint::text::VariationCoords,
};

/// Inter — font variabel UI (§3), lisensi OFL.
const INTER: &[u8] = include_bytes!("../../assets/fonts/InterVariable.ttf");
/// JetBrains Mono — font variabel numeric, selalu tabular (§3).
const JETBRAINS_MONO: &[u8] = include_bytes!("../../assets/fonts/JetBrainsMonoVariable.ttf");

/// Weight Inter yang dipakai §3: 400 (body), 500 (label), 600 (heading).
const INTER_WEIGHTS: [u16; 3] = [400, 500, 600];
/// Weight JetBrains Mono yang dipakai §3: 400 (numeric), 500 (numeric-lg).
const JBM_WEIGHTS: [u16; 2] = [400, 500];

/// Fallback glyph bawaan egui (emoji, simbol) — dipakai ulang setelah font kita.
const FALLBACK: [&str; 4] = [
    "NotoEmoji-Regular",
    "emoji-icon-font",
    "Hack",
    "Ubuntu-Light",
];

fn font_key(base: &str, weight: u16) -> String {
    format!("{base}-{weight}")
}

// Keluarga per weight — LazyLock agar `FontId` dibuat tanpa alokasi String
// (dipakai tiap label, tiap frame).
static INTER_400: LazyLock<FontFamily> = LazyLock::new(|| FontFamily::Name(Arc::from("Inter 400")));
static INTER_500: LazyLock<FontFamily> = LazyLock::new(|| FontFamily::Name(Arc::from("Inter 500")));
static INTER_600: LazyLock<FontFamily> = LazyLock::new(|| FontFamily::Name(Arc::from("Inter 600")));
static JBM_400: LazyLock<FontFamily> =
    LazyLock::new(|| FontFamily::Name(Arc::from("JetBrains Mono 400")));
static JBM_500: LazyLock<FontFamily> =
    LazyLock::new(|| FontFamily::Name(Arc::from("JetBrains Mono 500")));

/// Keluarga `FontFamily::Name` untuk weight Inter (§3).
pub fn inter_family(weight: u16) -> FontFamily {
    match weight {
        400 => (*INTER_400).clone(),
        500 => (*INTER_500).clone(),
        600 => (*INTER_600).clone(),
        _ => unreachable!("weight Inter di luar tabel §3: {weight}"),
    }
}

/// Keluarga `FontFamily::Name` untuk weight JetBrains Mono (§3).
pub fn mono_family(weight: u16) -> FontFamily {
    match weight {
        400 => (*JBM_400).clone(),
        500 => (*JBM_500).clone(),
        _ => unreachable!("weight JetBrains Mono di luar tabel §3: {weight}"),
    }
}

// ---------------------------------------------------------------------------
// §3 — Sembilan gaya teks
// ---------------------------------------------------------------------------

/// Sembilan gaya teks tabel §3. Semua komponen memakai ini — bukan ukuran
/// font lepas-lepas (aturan §3: maksimum 2 ukuran font per panel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    /// 11/500 Inter — label panel, badge.
    LabelSm,
    /// 12/500 Inter — label kontrol, tab.
    LabelMd,
    /// 13/400 Inter — teks UI umum, tooltip.
    Body,
    /// 14/400 Inter — dialog, empty state.
    BodyLg,
    /// 16/600 Inter — judul dialog kecil.
    HeadingSm,
    /// 20/600 Inter — judul panel besar.
    HeadingMd,
    /// 24/600 Inter — splash / onboarding.
    HeadingLg,
    /// 12/400 JetBrains Mono — nilai slider, timecode, koordinat (tabular).
    Numeric,
    /// 13/500 JetBrains Mono — angka besar (frame saat ini, FPS).
    NumericLg,
}

impl Type {
    /// Semua gaya §3, urut sesuai tabel.
    pub const ALL: [Type; 9] = [
        Self::LabelSm,
        Self::LabelMd,
        Self::Body,
        Self::BodyLg,
        Self::HeadingSm,
        Self::HeadingMd,
        Self::HeadingLg,
        Self::Numeric,
        Self::NumericLg,
    ];

    /// (size px, weight, line-height, monospace?) — tabel §3.
    const fn spec(self) -> (f32, u16, f32, bool) {
        match self {
            Self::LabelSm => (11.0, 500, 1.2, false),
            Self::LabelMd => (12.0, 500, 1.2, false),
            Self::Body => (13.0, 400, 1.4, false),
            Self::BodyLg => (14.0, 400, 1.4, false),
            Self::HeadingSm => (16.0, 600, 1.2, false),
            Self::HeadingMd => (20.0, 600, 1.2, false),
            Self::HeadingLg => (24.0, 600, 1.2, false),
            Self::Numeric => (12.0, 400, 1.2, true),
            Self::NumericLg => (13.0, 500, 1.2, true),
        }
    }

    /// `FontId` untuk gaya ini (family per-weight, §3).
    pub fn font_id(self) -> FontId {
        let (size, weight, _, mono) = self.spec();
        let family = if mono {
            mono_family(weight)
        } else {
            inter_family(weight)
        };
        FontId::new(size, family)
    }

    /// Tinggi baris absolut dalam poin (size × line-height, tabel §3).
    pub fn line_height(self) -> f32 {
        let (size, _, ratio, _) = self.spec();
        size * ratio
    }

    /// `RichText` dengan ukuran, weight, dan line-height §3.
    pub fn rich(self, text: impl Into<String>) -> RichText {
        let (size, weight, ratio, mono) = self.spec();
        let family = if mono {
            mono_family(weight)
        } else {
            inter_family(weight)
        };
        RichText::new(text)
            .size(size)
            .family(family)
            .line_height(Some(size * ratio))
    }

    /// Badge / shortcut hint: uppercase dengan tracking `+0.4 px`
    /// (§3: uppercase hanya untuk badge/shortcut hint).
    pub fn badge(self, text: impl AsRef<str>) -> RichText {
        self.rich(text.as_ref().to_uppercase())
            .extra_letter_spacing(0.4)
    }

    /// Nama gaya untuk `TextStyle::Name` (dipakai bila sebuah widget
    /// membutuhkan `TextStyle` yang tidak punya slot bawaan egui).
    pub const fn style_name(self) -> &'static str {
        match self {
            Self::LabelSm => "label-sm",
            Self::LabelMd => "label-md",
            Self::Body => "body",
            Self::BodyLg => "body-lg",
            Self::HeadingSm => "heading-sm",
            Self::HeadingMd => "heading-md",
            Self::HeadingLg => "heading-lg",
            Self::Numeric => "numeric",
            Self::NumericLg => "numeric-lg",
        }
    }

    /// `TextStyle` egui untuk gaya ini. Lima gaya memakai slot bawaan egui
    /// (`Small`/`Button`/`Body`/`Monospace`/`Heading`), sisanya `Name`.
    pub fn text_style(self) -> TextStyle {
        match self {
            Self::LabelSm => TextStyle::Small,
            Self::LabelMd => TextStyle::Button,
            Self::Body => TextStyle::Body,
            Self::Numeric => TextStyle::Monospace,
            Self::HeadingMd => TextStyle::Heading,
            other => TextStyle::Name(Arc::from(other.style_name())),
        }
    }
}

// ---------------------------------------------------------------------------
// Pemasangan font & gaya
// ---------------------------------------------------------------------------

/// Definisi font §3: Inter + JetBrains Mono (variabel, per-weight) di atas
/// fallback bawaan egui (emoji & simbol). Panggil
/// `ctx.set_fonts(typography::definitions())` sekali saat startup.
pub fn definitions() -> eframe::egui::FontDefinitions {
    let mut defs = eframe::egui::FontDefinitions::default();

    for &weight in &INTER_WEIGHTS {
        defs.font_data.insert(
            font_key("inter", weight),
            Arc::new(FontData::from_static(INTER).tweak(FontTweak {
                coords: VariationCoords::new([(b"wght", f32::from(weight))]),
                ..FontTweak::default()
            })),
        );
    }
    for &weight in &JBM_WEIGHTS {
        defs.font_data.insert(
            font_key("jbm", weight),
            Arc::new(FontData::from_static(JETBRAINS_MONO).tweak(FontTweak {
                coords: VariationCoords::new([(b"wght", f32::from(weight))]),
                ..FontTweak::default()
            })),
        );
    }

    for &weight in &INTER_WEIGHTS {
        let mut chain = vec![font_key("inter", weight)];
        chain.extend(FALLBACK.map(str::to_owned));
        defs.families.insert(inter_family(weight), chain);
    }
    for &weight in &JBM_WEIGHTS {
        let mut chain = vec![font_key("jbm", weight), "Hack".to_owned()];
        chain.extend(FALLBACK.map(str::to_owned));
        defs.families.insert(mono_family(weight), chain);
    }

    // Arahkan family standar egui ke font kita (widget bawaan egui tetap
    // konsisten dengan §3; emoji & simbol jatuh ke fallback).
    let mut proportional = vec![font_key("inter", 400)];
    proportional.extend(FALLBACK.map(str::to_owned));
    defs.families.insert(FontFamily::Proportional, proportional);

    let mut monospace = vec![font_key("jbm", 400), "Hack".to_owned()];
    monospace.extend(FALLBACK.map(str::to_owned));
    defs.families.insert(FontFamily::Monospace, monospace);

    defs
}

/// Daftarkan sembilan gaya §3 ke `style.text_styles` (termasuk slot bawaan
/// egui). Panggil setelah [`crate::theme::Theme::apply_style`].
pub fn apply_text_styles(style: &mut Style) {
    style
        .text_styles
        .insert(TextStyle::Small, Type::LabelSm.font_id());
    style
        .text_styles
        .insert(TextStyle::Body, Type::Body.font_id());
    style
        .text_styles
        .insert(TextStyle::Monospace, Type::Numeric.font_id());
    style
        .text_styles
        .insert(TextStyle::Button, Type::LabelMd.font_id());
    style
        .text_styles
        .insert(TextStyle::Heading, Type::HeadingMd.font_id());
    for t in Type::ALL {
        style.text_styles.insert(t.text_style(), t.font_id());
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use super::*;

    #[test]
    fn tabel_section_3_sesuai_spesifikasi() {
        // (size, weight, line-height ratio, mono)
        let expected = [
            (Type::LabelSm, 11.0, 500, 1.2, false),
            (Type::LabelMd, 12.0, 500, 1.2, false),
            (Type::Body, 13.0, 400, 1.4, false),
            (Type::BodyLg, 14.0, 400, 1.4, false),
            (Type::HeadingSm, 16.0, 600, 1.2, false),
            (Type::HeadingMd, 20.0, 600, 1.2, false),
            (Type::HeadingLg, 24.0, 600, 1.2, false),
            (Type::Numeric, 12.0, 400, 1.2, true),
            (Type::NumericLg, 13.0, 500, 1.2, true),
        ];
        for (t, size, weight, ratio, mono) in expected {
            let (s, w, r, m) = t.spec();
            assert_eq!((s, w, r, m), (size, weight, ratio, mono), "gaya {t:?}");
            assert_eq!(t.line_height(), size * ratio, "line-height {t:?}");
            let font = t.font_id();
            assert_eq!(font.size, size, "ukuran {t:?}");
            if mono {
                assert!(
                    matches!(font.family, FontFamily::Name(_)),
                    "{t:?} harus JetBrains Mono"
                );
            }
        }
        assert_eq!(Type::ALL.len(), 9);
    }

    #[test]
    fn font_variabel_mendukung_sumbu_wght() {
        let defs = definitions();
        for key in ["inter-400", "inter-500", "inter-600", "jbm-400", "jbm-500"] {
            let data = defs
                .font_data
                .get(key)
                .unwrap_or_else(|| panic!("{key} ada"));
            let axes = data.variation_axes();
            assert!(
                axes.iter().any(|a| a.name.as_deref() == Some("Weight")),
                "{key} harus punya sumbu Weight, dapat: {:?}",
                axes.iter().map(|a| &a.name).collect::<Vec<_>>()
            );
        }
        // Family per-weight terdaftar.
        for t in Type::ALL {
            let font = t.font_id();
            assert!(
                defs.families.contains_key(&font.family),
                "family {:?} tidak terdaftar",
                font.family
            );
        }
    }

    #[test]
    fn bobot_600_lebih_lebar_dari_400() {
        // Bobot harus benar-benar mengubah rendering (bukan cuma warna).
        let ctx = egui::Context::default();
        ctx.set_fonts(definitions());
        // Inisialisasi font (butuh satu frame). Texture delta tidak dipakai
        // di tes — buang agar tidak panic saat TexturesDelta di-drop.
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();

        let (w400, w600) = ctx.fonts_mut(|fonts| {
            let mut layout = |family: FontFamily| {
                fonts
                    .layout(
                        "Flipbrush Widget".to_owned(),
                        FontId::new(13.0, family),
                        eframe::egui::Color32::WHITE,
                        4096.0,
                    )
                    .rect
                    .width()
            };
            (layout(inter_family(400)), layout(inter_family(600)))
        });
        assert!(
            w600 > w400,
            "wght=600 ({w600:.2}) harus lebih lebar dari wght=400 ({w400:.2})"
        );
    }

    #[test]
    fn badge_uppercase_dengan_tracking() {
        let dbg = format!("{:?}", Type::LabelSm.badge("Prev Frame"));
        // §3: uppercase hanya untuk badge/shortcut hint, tracking +0.4 px.
        assert!(dbg.contains("PREV FRAME"), "harus uppercase: {dbg}");
        assert!(
            dbg.contains("extra_letter_spacing: 0.4"),
            "tracking badge +0.4 px: {dbg}"
        );
        // Gaya biasa tidak di-uppercase.
        let plain = format!("{:?}", Type::Body.rich("Prev Frame"));
        assert!(
            plain.contains("Prev Frame"),
            "body tetap lowercase: {plain}"
        );
    }

    #[test]
    fn line_height_egui_dipasang_per_gaya() {
        // `Type::rich` harus membawa line-height eksplisit §3.
        let rt = Type::Body.rich("x");
        let dbg = format!("{rt:?}");
        // 13.0 × 1.4 = 18.199999 (pembulatan f32) — hitung ekspektasi dengan
        // ekspresi yang sama agar perbandingannya stabil.
        let expected = Type::Body.line_height();
        assert!(
            dbg.contains(&format!("line_height: Some({expected})")),
            "body 13 px × 1.4 = {expected}: {dbg}"
        );
    }

    #[test]
    fn slot_bawaan_egui_mengarah_ke_gaya_kita() {
        let mut style = Style::default();
        apply_text_styles(&mut style);
        assert_eq!(
            style.text_styles.get(&TextStyle::Body),
            Some(&Type::Body.font_id())
        );
        assert_eq!(
            style.text_styles.get(&TextStyle::Heading),
            Some(&Type::HeadingMd.font_id())
        );
        assert_eq!(
            style.text_styles.get(&TextStyle::Monospace),
            Some(&Type::Numeric.font_id())
        );
        assert_eq!(
            style
                .text_styles
                .get(&TextStyle::Name(Arc::from("heading-lg"))),
            Some(&Type::HeadingLg.font_id())
        );
    }
}
