//! Token tema Flipbrush — satu-satunya tempat hex literal diizinkan
//! (DESIGN.md §10: "Tidak boleh ada hex literal di luar `theme.rs`").
//!
//! Cakupan M1 (DESIGN.md §12.2): seluruh token §2 (warna), §4 (radius &
//! elevation), dan §6 (motion) sebagai konstanta bertipe yang di-resolve
//! saat startup (§10). Konsumen: `typography.rs` (§3), `icon.rs` (§5), dan
//! seluruh komponen UI — tidak ada warna hard-coded di luar file ini
//! (ditegakkan `scripts/check-hex-literals.sh` di CI).

use eframe::egui::{Color32, CornerRadius, Margin, Stroke, Style, Vec2, Visuals};

// ---------------------------------------------------------------------------
// §2.4 — Accent picker (tema kustom)
// ---------------------------------------------------------------------------

/// Pilihan aksen chrome (DESIGN.md §2.4) — mengubah chrome, bukan kanvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accent {
    Indigo,
    Teal,
    Magenta,
    Orange,
}

impl Accent {
    /// Aksen default (§2.4).
    pub const DEFAULT: Self = Self::Indigo;

    /// Semua pilihan untuk accent picker (§2.4; dipakai mulai M5).
    pub const ALL: [Accent; 4] = [Self::Indigo, Self::Teal, Self::Magenta, Self::Orange];

    /// Nama tampilan (§2.4).
    pub const fn label(self) -> &'static str {
        match self {
            Self::Indigo => "Indigo",
            Self::Teal => "Teal",
            Self::Magenta => "Magenta",
            Self::Orange => "Orange",
        }
    }

    /// Nilai dasar aksen (§2.4).
    pub const fn base(self) -> Color32 {
        match self {
            Self::Indigo => Color32::from_rgb(0x6E, 0x7B, 0xF2),
            Self::Teal => Color32::from_rgb(0x2B, 0xC4, 0xB6),
            Self::Magenta => Color32::from_rgb(0xD6, 0x40, 0x9F),
            Self::Orange => Color32::from_rgb(0xE8, 0x74, 0x3B),
        }
    }

    /// `accent/brand-hover` (§2.2). Nilai indigo persis seperti spesifikasi;
    /// aksen lain di-derive (campuran ke putih) agar proporsinya konsisten.
    pub fn hover(self) -> Color32 {
        match self {
            Self::Indigo => Color32::from_rgb(0x82, 0x89, 0xF5),
            other => mix(other.base(), Color32::WHITE, 0.18),
        }
    }

    /// `accent/brand-subtle` — aksen @ 12% (§2.2).
    pub fn subtle(self) -> Color32 {
        with_alpha(self.base(), 0.12)
    }
}

// ---------------------------------------------------------------------------
// §2.5 — Mode tema
// ---------------------------------------------------------------------------

/// Mode tema (§2.5). Linux default gelap; Android mengikuti sistem (M6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    /// Resolve tema penuh untuk mode + aksen ini (§10: resolve saat startup).
    pub fn resolve(self, accent: Accent) -> Theme {
        match self {
            Self::Dark => Theme::dark_with(accent),
            Self::Light => Theme::light_with(accent),
        }
    }
}

// ---------------------------------------------------------------------------
// Helper campur warna (internal — agar turunan §2.5 tidak menghasilkan hex baru)
// ---------------------------------------------------------------------------

/// Campur dua warna: `t = 0` → `a`, `t = 1` → `b`.
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let lerp = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()))
}

/// Warna dengan alpha 0..1 (notasi "@" di DESIGN §2, mis. "brand @ 12%").
pub fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied_const(c.r(), c.g(), c.b(), (a * 255.0).round() as u8)
}

// ---------------------------------------------------------------------------
// §2 — Token warna (di-resolve)
// ---------------------------------------------------------------------------

/// Token tema yang sudah di-resolve untuk satu sesi aplikasi (§10).
#[derive(Clone, Debug)]
pub struct Theme {
    /// Mode tema ini (§2.5).
    pub mode: ThemeMode,
    /// Aksen chrome aktif (§2.4).
    pub accent: Accent,

    // §2.1 — skala netral
    /// Area abu di sekeliling kanvas (netral, bukan chromatic).
    pub canvas_surround: Color32,
    /// Background aplikasi.
    pub surface_0: Color32,
    /// Panel, dock, toolbar.
    pub surface_1: Color32,
    /// Input field, cell timeline, kontrol.
    pub surface_2: Color32,
    /// Hover, raised, popover.
    pub surface_3: Color32,
    /// Garis pemisah antar panel.
    pub border_default: Color32,
    /// Border kontrol, divider aktif.
    pub border_strong: Color32,
    /// Teks utama (kontras AA 4.5:1 di atas surface-1).
    pub text_primary: Color32,
    /// Label, teks sekunder, ikon nonaktif.
    pub text_secondary: Color32,
    /// Elemen nonaktif.
    pub text_disabled: Color32,

    // §2.2 — aksen & semantik
    /// Tool aktif, tab aktif, fokus, tombol primer.
    pub accent_brand: Color32,
    /// Hover aksen.
    pub accent_brand_hover: Color32,
    /// Latar tool aktif, highlight seleksi ringan (aksen @ 12%).
    pub accent_brand_subtle: Color32,
    /// Delete, error, destructive action.
    pub danger: Color32,
    /// Auto-save gagal, peringatan recover.
    pub warning: Color32,
    /// Export sukses, autosave OK.
    pub success: Color32,
    /// Focus ring: `accent/brand` 2 px dengan offset 2 px (§2.2, §9).
    pub focus_ring: Stroke,
    /// Offset focus ring dari tepi kontrol (§9: 2 px).
    pub focus_ring_offset: f32,

    // §2.3 — warna animasi & timeline
    /// Onion skin frame sebelumnya (merah — aman buta warna, §9).
    pub onion_prev: Color32,
    /// Onion skin frame berikutnya (biru — aman buta warna, §9).
    pub onion_next: Color32,
    /// Overlay light table (putih @ 8%).
    pub onion_skin_fill: Color32,
    /// Cell kosong.
    pub cel_empty: Color32,
    /// Cell berisi bitmap (biru redup, bukan aksen).
    pub cel_exposed: Color32,
    /// Cell / rentang terpilih (aksen @ 30%).
    pub cel_selected: Color32,
    /// Awal exposure; identik dengan `cel_exposed`, cell lanjutan hanya strip.
    pub cel_shared: Color32,
    /// Garis playhead (identik dengan `text/primary`).
    pub playhead: Color32,
    /// Marker & loop range (identik dengan `warning`).
    pub marker: Color32,
    /// Overlay quick mask (merah @ 50%).
    pub quickmask: Color32,
}

impl Theme {
    /// Tema gelap default (§2.1–2.3) dengan aksen default (§2.4).
    pub fn dark() -> Self {
        Self::dark_with(Accent::DEFAULT)
    }

    /// Tema gelap (§2.1–2.3) dengan aksen pilihan (§2.2, §2.4).
    pub fn dark_with(accent: Accent) -> Self {
        let accent_brand = accent.base();
        let text_primary = Color32::from_rgb(0xED, 0xED, 0xEF);
        let warning = Color32::from_rgb(0xF5, 0xA5, 0x24);
        let cel_exposed = Color32::from_rgb(0x3E, 0x4C, 0x63);
        Self {
            mode: ThemeMode::Dark,
            accent,
            // §2.1
            canvas_surround: Color32::from_rgb(0x3B, 0x3B, 0x3E),
            surface_0: Color32::from_rgb(0x1C, 0x1C, 0x1F),
            surface_1: Color32::from_rgb(0x26, 0x26, 0x29),
            surface_2: Color32::from_rgb(0x2F, 0x2F, 0x33),
            surface_3: Color32::from_rgb(0x3A, 0x3A, 0x40),
            border_default: Color32::from_rgb(0x35, 0x35, 0x3A),
            border_strong: Color32::from_rgb(0x4A, 0x4A, 0x51),
            text_primary,
            text_secondary: Color32::from_rgb(0x9A, 0x9A, 0xA1),
            text_disabled: Color32::from_rgb(0x6A, 0x6A, 0x70),
            // §2.2
            accent_brand,
            accent_brand_hover: accent.hover(),
            accent_brand_subtle: accent.subtle(),
            danger: Color32::from_rgb(0xE5, 0x48, 0x4D),
            warning,
            success: Color32::from_rgb(0x30, 0xA4, 0x6C),
            focus_ring: Stroke::new(2.0, accent_brand),
            focus_ring_offset: 2.0,
            // §2.3
            onion_prev: Color32::from_rgb(0xFF, 0x6B, 0x6B),
            onion_next: Color32::from_rgb(0x4E, 0xA8, 0xFF),
            onion_skin_fill: with_alpha(Color32::WHITE, 0.08),
            cel_empty: Color32::from_rgb(0x2A, 0x2A, 0x2E),
            cel_exposed,
            cel_selected: with_alpha(accent_brand, 0.30),
            cel_shared: cel_exposed,
            playhead: text_primary,
            marker: warning,
            quickmask: with_alpha(Color32::from_rgb(0xFF, 0x00, 0x00), 0.50),
        }
    }

    /// Tema terang (§2.5) dengan aksen default (§2.4).
    pub fn light() -> Self {
        Self::light_with(Accent::DEFAULT)
    }

    /// Tema terang (§2.5) dengan aksen pilihan.
    ///
    /// §2.5: "Aksen & warna animasi tidak berubah antar tema" — hanya token
    /// netral yang berganti; token yang tidak didefinisikan §2.5 di-derive
    /// dari yang ada (tanpa hex baru).
    pub fn light_with(accent: Accent) -> Self {
        let accent_brand = accent.base();
        let text_primary = Color32::from_rgb(0x17, 0x17, 0x1A);
        let text_secondary = Color32::from_rgb(0x63, 0x63, 0x6B);
        let surface_1 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
        let border_default = Color32::from_rgb(0xDC, 0xDC, 0xE2);
        let warning = Color32::from_rgb(0xF5, 0xA5, 0x24);
        let cel_exposed = Color32::from_rgb(0x3E, 0x4C, 0x63);
        Self {
            mode: ThemeMode::Light,
            accent,
            // §2.5 (netral)
            canvas_surround: Color32::from_rgb(0xC9, 0xC9, 0xCE),
            surface_0: Color32::from_rgb(0xF5, 0xF5, 0xF7),
            surface_1,
            surface_2: Color32::from_rgb(0xED, 0xED, 0xF0),
            surface_3: Color32::from_rgb(0xE2, 0xE2, 0xE7),
            border_default,
            border_strong: mix(border_default, text_secondary, 0.45),
            text_primary,
            text_secondary,
            text_disabled: mix(text_secondary, surface_1, 0.50),
            // §2.2 — aksen & semantik tidak berubah (§2.5)
            accent_brand,
            accent_brand_hover: accent.hover(),
            accent_brand_subtle: accent.subtle(),
            danger: Color32::from_rgb(0xE5, 0x48, 0x4D),
            warning,
            success: Color32::from_rgb(0x30, 0xA4, 0x6C),
            focus_ring: Stroke::new(2.0, accent_brand),
            focus_ring_offset: 2.0,
            // §2.3 — warna animasi tidak berubah (§2.5)
            onion_prev: Color32::from_rgb(0xFF, 0x6B, 0x6B),
            onion_next: Color32::from_rgb(0x4E, 0xA8, 0xFF),
            onion_skin_fill: with_alpha(Color32::WHITE, 0.08),
            cel_empty: Color32::from_rgb(0x2A, 0x2A, 0x2E),
            cel_exposed,
            cel_selected: with_alpha(accent_brand, 0.30),
            cel_shared: cel_exposed,
            playhead: text_primary,
            marker: warning,
            quickmask: with_alpha(Color32::from_rgb(0xFF, 0x00, 0x00), 0.50),
        }
    }

    /// Terapkan token warna §2 ke `egui::Visuals` — komponen memakai token,
    /// bukan hex (§10).
    pub fn apply(&self, visuals: &mut Visuals) {
        // Latar: aplikasi = surface-0; panel/window = surface-1 (§2.1, §7.6).
        visuals.panel_fill = self.surface_0;
        visuals.window_fill = self.surface_1;
        visuals.extreme_bg_color = self.surface_2;
        visuals.faint_bg_color = self.surface_2;
        visuals.override_text_color = Some(self.text_primary);
        visuals.hyperlink_color = self.accent_brand;
        visuals.selection.bg_fill = self.accent_brand;
        visuals.selection.stroke = Stroke::new(1.0, self.text_primary);
        visuals.window_stroke = Stroke::new(1.0, self.border_default);
        visuals.window_corner_radius = CornerRadius::same(radius::MD);
        visuals.menu_corner_radius = CornerRadius::same(radius::SM);
        visuals.window_shadow = elevation::MODAL;
        visuals.popup_shadow = elevation::POPOVER;

        // §7.1 — state tombol (ghost / ghost-active) + border kontrol §2.1.
        let w = &mut visuals.widgets;
        w.noninteractive.bg_fill = self.surface_2;
        w.noninteractive.weak_bg_fill = self.surface_2;
        w.noninteractive.bg_stroke = Stroke::new(1.0, self.border_default);
        w.noninteractive.fg_stroke = Stroke::new(1.0, self.text_disabled);
        w.noninteractive.corner_radius = CornerRadius::same(radius::SM);

        w.inactive.bg_fill = self.surface_2;
        w.inactive.weak_bg_fill = Color32::TRANSPARENT; // ghost: transparan
        w.inactive.bg_stroke = Stroke::new(1.0, self.border_strong);
        w.inactive.fg_stroke = Stroke::new(1.0, self.text_secondary);
        w.inactive.corner_radius = CornerRadius::same(radius::SM);

        w.hovered.bg_fill = self.surface_2;
        w.hovered.weak_bg_fill = self.surface_3; // ghost → surface-3 saat hover
        w.hovered.bg_stroke = Stroke::new(1.0, self.border_strong);
        w.hovered.fg_stroke = Stroke::new(1.0, self.text_primary);
        w.hovered.corner_radius = CornerRadius::same(radius::SM);

        w.active.bg_fill = self.surface_2;
        w.active.weak_bg_fill = self.surface_2;
        w.active.bg_stroke = Stroke::new(1.0, self.border_strong);
        w.active.fg_stroke = Stroke::new(1.0, self.accent_brand);
        w.active.corner_radius = CornerRadius::same(radius::SM);
    }

    /// Terapkan token §2 (warna), §4 (spacing/radius/elevation), dan aturan
    /// §6/§7.9 yang berupa pengaturan `Style` — dipanggil sekali saat
    /// startup (§10).
    pub fn apply_style(&self, style: &mut Style) {
        self.apply(&mut style.visuals);

        // §4.1 — grid spacing basis 4 px.
        style.spacing.item_spacing = Vec2::new(spacing::GAP_CONTROL, spacing::GRID);
        style.spacing.button_padding = Vec2::new(spacing::GAP_CONTROL, spacing::GRID);
        style.spacing.window_margin = Margin::symmetric(spacing::PANEL as i8, spacing::PANEL as i8);

        // §7.9 — tooltip delay 500 ms.
        style.interaction.tooltip_delay = 0.5;
    }
}

// ---------------------------------------------------------------------------
// §4.1 — Grid spacing (basis 4 px)
// ---------------------------------------------------------------------------

/// Grid spacing basis 4 px (DESIGN.md §4.1).
pub mod spacing {
    /// Langkah dasar grid.
    pub const GRID: f32 = 4.0;
    /// Semua langkah grid (§4.1).
    pub const STEPS: [f32; 6] = [4.0, 8.0, 12.0, 16.0, 24.0, 32.0];
    /// Jarak ikon ke label.
    pub const ICON_LABEL: f32 = GRID;
    /// Padding internal kontrol; jarak antar kontrol dalam satu baris.
    pub const GAP_CONTROL: f32 = 8.0;
    /// Padding item list; jarak antar grup kontrol.
    pub const LIST: f32 = 12.0;
    /// Padding panel; padding dialog.
    pub const PANEL: f32 = 16.0;
    /// Jarak antar section dalam dialog.
    pub const SECTION: f32 = 24.0;
    /// Spasi besar (empty state, splash).
    pub const LARGE: f32 = 32.0;
}

// ---------------------------------------------------------------------------
// §4.2 — Radius
// ---------------------------------------------------------------------------

/// Radius sudut (DESIGN.md §4.2).
pub mod radius {
    /// Button, input, cell timeline, tooltip.
    pub const SM: u8 = 4;
    /// Panel, modal, popover, card.
    pub const MD: u8 = 8;
    /// Token `radius/pill` = 999 px; `CornerRadius` memakai `u8`, jadi
    /// disimpan sebagai 255 — tetap ≥ setengah tinggi elemen apa pun,
    /// sehingga hasil visual identik (pill penuh).
    pub const PILL: u8 = 255;
}

// ---------------------------------------------------------------------------
// §4.3 — Elevation
// ---------------------------------------------------------------------------

/// Elevation (DESIGN.md §4.3) — dark theme mayoritas memakai border, bukan
/// shadow; shadow hanya untuk overlay terapung.
pub mod elevation {
    use eframe::egui::{Color32, Shadow};

    /// `0 4px 12px rgba(0,0,0,0.35)` — context menu, dropdown.
    pub const POPOVER: Shadow = Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_rgba_unmultiplied_const(0, 0, 0, 89), // 0.35 * 255
    };

    /// `0 8px 24px rgba(0,0,0,0.40)` — dialog, command palette.
    pub const MODAL: Shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_rgba_unmultiplied_const(0, 0, 0, 102), // 0.40 * 255
    };

    /// `0 2px 8px rgba(0,0,0,0.30)` — item yang di-drag (ghost).
    pub const DRAG: Shadow = Shadow {
        offset: [0, 2],
        blur: 8,
        spread: 0,
        color: Color32::from_rgba_unmultiplied_const(0, 0, 0, 77), // 0.30 * 255
    };
}

// ---------------------------------------------------------------------------
// §6 — Motion
// ---------------------------------------------------------------------------

/// Token motion (DESIGN.md §6): hanya untuk feedback fungsional — tidak ada
/// dekorasi; semua motion dimatikan saat `prefers-reduced-motion` (audit M5);
/// tidak ada animasi yang menunda input (stroke selalu instan).
pub mod motion {
    use std::time::Duration;

    /// Hover, toggle, tool aktif — 120 ms ease-out.
    pub const FAST: Duration = Duration::from_millis(120);
    /// Panel slide, tab switch, toast masuk/keluar — 180 ms ease-out.
    pub const BASE: Duration = Duration::from_millis(180);
    /// Playback sinkron FPS proyek — durasi per frame ditentukan FPS
    /// proyek (model animasi, M3); jangan dipakai easing statis.
    ///
    /// Haptics (mobile, M6): getaran ~10 ms hanya pada ganti tool,
    /// langkah frame, undo/redo, toggle onion — bisa dimatikan di settings.
    pub const PLAYBACK_NOTE: &str = "sinkron FPS proyek (§6)";
}

// ---------------------------------------------------------------------------
// §9 — Audit kontras (juga dipakai tes)
// ---------------------------------------------------------------------------

/// Rasio kontras WCAG 2.1 antara dua warna (DESIGN.md §9: min 4.5:1 body,
/// 3:1 large text & ikon).
pub fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    fn relative_luma(c: Color32) -> f32 {
        let channel = |v: u8| {
            let s = f32::from(v) / 255.0;
            if s <= 0.040_45 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
    }
    let l1 = relative_luma(a);
    let l2 = relative_luma(b);
    (l1.max(l2) + 0.05) / (l1.min(l2) + 0.05)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn dark_theme_has_distinct_surfaces() {
        let t = Theme::dark();
        assert_ne!(t.surface_0, t.surface_1);
        assert_ne!(t.surface_1, t.surface_2);
        assert_ne!(t.surface_2, t.surface_3);
    }

    #[test]
    fn light_theme_has_distinct_surfaces() {
        let t = Theme::light();
        assert_ne!(t.surface_0, t.surface_1);
        assert_ne!(t.surface_1, t.surface_2);
        assert_ne!(t.surface_2, t.surface_3);
        assert_ne!(t.border_default, t.surface_1);
    }

    #[test]
    fn dark_text_contrast_aa() {
        // DESIGN.md §9: kontras minimal 4.5:1 untuk body text.
        let t = Theme::dark();
        let primary = contrast_ratio(t.text_primary, t.surface_1);
        assert!(
            primary >= 4.5,
            "dark text/primary vs surface/1 = {primary:.2}:1"
        );
        let secondary = contrast_ratio(t.text_secondary, t.surface_1);
        assert!(
            secondary >= 4.5,
            "dark text/secondary vs surface/1 = {secondary:.2}:1"
        );
    }

    #[test]
    fn light_text_contrast_aa() {
        let t = Theme::light();
        let primary = contrast_ratio(t.text_primary, t.surface_1);
        assert!(
            primary >= 4.5,
            "light text/primary vs surface/1 = {primary:.2}:1"
        );
        let secondary = contrast_ratio(t.text_secondary, t.surface_1);
        assert!(
            secondary >= 4.5,
            "light text/secondary vs surface/1 = {secondary:.2}:1"
        );
        let secondary_on_2 = contrast_ratio(t.text_secondary, t.surface_2);
        assert!(
            secondary_on_2 >= 4.5,
            "light text/secondary vs surface/2 = {secondary_on_2:.2}:1"
        );
    }

    #[test]
    fn dark_icon_contrast_large_text() {
        // §9: ikon minimal 3:1 terhadap latar tombolnya.
        let t = Theme::dark();
        let icon = contrast_ratio(t.text_secondary, t.surface_3);
        assert!(
            icon >= 3.0,
            "ikon (secondary) vs hover (surface/3) = {icon:.2}:1"
        );
        let icon_hover = contrast_ratio(t.text_primary, t.surface_3);
        assert!(
            icon_hover >= 3.0,
            "ikon (primary) vs hover (surface/3) = {icon_hover:.2}:1"
        );
    }

    #[test]
    fn accent_subtle_12_percent() {
        // §2.2: accent/brand-subtle = aksen @ 12%.
        let t = Theme::dark();
        assert_eq!(t.accent_brand_subtle.a(), 31, "12% dari 255 = 30.6 → 31");
        // Color32 menyimpan premultiplied alpha — kembalikan ke kanal penuh
        // sebelum dibandingkan dengan kanal aksen dasar (toleransi pembulatan).
        let base = t.accent_brand;
        for (got, want) in [
            (t.accent_brand_subtle.r(), base.r()),
            (t.accent_brand_subtle.g(), base.g()),
            (t.accent_brand_subtle.b(), base.b()),
        ] {
            let un = (f32::from(got) * 255.0 / f32::from(t.accent_brand_subtle.a())).round();
            assert!(
                (un - f32::from(want)).abs() <= 5.0,
                "kanal premultiplied {got} → {un}, diharapkan ≈{want}"
            );
        }
        // §2.3: cel-selected = aksen @ 30%.
        assert_eq!(t.cel_selected.a(), 77, "30% dari 255 = 76.5 → 77");
        // §2.3: onion/skin-fill = putih @ 8%.
        assert_eq!(t.onion_skin_fill.a(), 20, "8% dari 255 = 20.4 → 20");
    }

    #[test]
    fn token_ekivalen_antar_tabel() {
        // §2.3: playhead = text/primary; marker = warning; cel-shared = cel-exposed.
        let t = Theme::dark();
        assert_eq!(t.playhead, t.text_primary);
        assert_eq!(t.marker, t.warning);
        assert_eq!(t.cel_shared, t.cel_exposed);
        // §2.2: focus/ring memakai accent/brand 2 px.
        assert_eq!(t.focus_ring.width, 2.0);
        assert_eq!(t.focus_ring.color, t.accent_brand);
        assert_eq!(t.focus_ring_offset, 2.0);
    }

    #[test]
    fn light_theme_animasi_tidak_berubah() {
        // §2.5: "Aksen & warna animasi tidak berubah antar tema".
        let d = Theme::dark();
        let l = Theme::light();
        assert_eq!(d.accent_brand, l.accent_brand);
        assert_eq!(d.danger, l.danger);
        assert_eq!(d.warning, l.warning);
        assert_eq!(d.success, l.success);
        assert_eq!(d.onion_prev, l.onion_prev);
        assert_eq!(d.onion_next, l.onion_next);
        assert_eq!(d.cel_empty, l.cel_empty);
        assert_eq!(d.cel_exposed, l.cel_exposed);
        assert_eq!(d.quickmask, l.quickmask);
    }

    #[test]
    fn token_dimiliki_mode_terang() {
        // §2.5 mendaftarkan token terang yang wajib ada.
        let l = Theme::light();
        assert_eq!(l.mode, ThemeMode::Light);
        assert_eq!(l.surface_0, Color32::from_rgb(0xF5, 0xF5, 0xF7));
        assert_eq!(l.surface_1, Color32::from_rgb(0xFF, 0xFF, 0xFF));
        assert_eq!(l.text_primary, Color32::from_rgb(0x17, 0x17, 0x1A));
        assert_eq!(l.canvas_surround, Color32::from_rgb(0xC9, 0xC9, 0xCE));
    }

    #[test]
    fn radius_elevation_motion_sesuai_spesifikasi() {
        // §4.2
        assert_eq!(radius::SM, 4);
        assert_eq!(radius::MD, 8);
        assert_eq!(radius::PILL, 255);
        // §4.1
        assert_eq!(spacing::STEPS, [4.0, 8.0, 12.0, 16.0, 24.0, 32.0]);
        // §4.3
        assert_eq!(elevation::POPOVER.offset, [0, 4]);
        assert_eq!(elevation::POPOVER.blur, 12);
        assert_eq!(elevation::MODAL.offset, [0, 8]);
        assert_eq!(elevation::MODAL.blur, 24);
        assert_eq!(elevation::DRAG.offset, [0, 2]);
        assert_eq!(elevation::DRAG.blur, 8);
        // §6
        assert_eq!(motion::FAST, Duration::from_millis(120));
        assert_eq!(motion::BASE, Duration::from_millis(180));
        // §7.9
        let mut style = Style::default();
        Theme::dark().apply_style(&mut style);
        assert_eq!(style.interaction.tooltip_delay, 0.5);
    }

    #[test]
    fn aksen_terpilih_mengubah_token_aksen() {
        // §2.4: accent picker mengubah chrome — semua token aksen ikut.
        for accent in Accent::ALL {
            let t = Theme::dark_with(accent);
            assert_eq!(t.accent_brand, accent.base());
            assert_eq!(t.accent_brand_subtle, accent.subtle());
            assert_eq!(t.focus_ring.color, accent.base());
            assert_eq!(t.accent, accent);
        }
        // Nilai indigo persis §2.2/§2.4.
        assert_eq!(Accent::Indigo.base(), Color32::from_rgb(0x6E, 0x7B, 0xF2));
        assert_eq!(Accent::Indigo.hover(), Color32::from_rgb(0x82, 0x89, 0xF5));
    }
}
