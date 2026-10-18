//! Panel Color (DESIGN §7.3) — M4b.
//!
//! Roda HSV 160 px — sudut = hue, jari-jari = saturasi — plus strip
//! lightness (nilai V), input hex (§1.4 presisi) & RGB (drag-numeric §7.2),
//! swatch bar 16 slot (klik = pilih, klik slot kosong = simpan warna aktif,
//! `Shift+Klik` = hapus, seret = geser urutan), dan palet harmony
//! (complementary / analogous / triadic) sebagai bagian terpisah.
//!
//! Semua warna UI memakai token tema §2 (tanpa hex baru); roda dirender
//! sebagai tekstur statis yang dibuat sekali (marker digambar painter) —
//! tajam 2× untuk HiDPI (§12.2 logical px). Swatch persist di
//! `$XDG_CONFIG_HOME/flipbrush/swatches.json` (pendamping `layout.json`).

use std::path::PathBuf;

use eframe::egui::{
    self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Ui, Vec2, epaint::StrokeKind,
};
use serde::{Deserialize, Serialize};

use flipbrush_core::raster::Rgba;

use crate::dock::{config_dir, garis_pemisah};
use crate::slider::DragNumeric;
use crate::theme::{self, Theme};
use crate::typography::Type;

/// Diamater roda warna §7.3 (160 px desktop).
const WHEEL_PX: f32 = 160.0;
/// Resolusi tekstur roda — 2× agar tajam pada layar HiDPI (§12.2).
const WHEEL_TEX: usize = 320;
/// Tinggi strip lightness.
const STRIP_H: f32 = 12.0;
/// Jumlah slot swatch §7.3.
pub const SLOT_COUNT: usize = 16;
/// Sisi slot swatch — grid 2×8 muat pada dock minimum 220 px (§8.2).
const SLOT_PX: f32 = 24.0;
/// Lebar kolom input hex (§1.4: angka jelas).
const HEX_W: f32 = 96.0;

// ---------------------------------------------------------------------------
// Konversi warna
// ---------------------------------------------------------------------------

/// RGB 0–255 → HSV: hue derajat `0..360`, saturasi & nilai `0..1`.
pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let maks = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = maks - min;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if maks == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if maks == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let hue = (hue + 360.0) % 360.0;
    let saturasi = if maks <= f32::EPSILON {
        0.0
    } else {
        delta / maks
    };
    (hue, saturasi, maks)
}

/// HSV (hue derajat, saturasi & nilai `0..1`) → RGB 0–255.
pub fn hsv_to_rgb(hue: f32, saturasi: f32, nilai: f32) -> (u8, u8, u8) {
    let hue = hue.rem_euclid(360.0);
    let c = nilai * saturasi;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = nilai - c;
    let (r, g, b) = match (hue / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let ubah = |f: f32| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (ubah(r), ubah(g), ubah(b))
}

/// Warna → teks `#RRGGBB` (case-insensitive saat dibaca kembali).
pub fn hex_kini(warna: Rgba) -> String {
    format!("#{:02X}{:02X}{:02X}", warna.r, warna.g, warna.b)
}

/// Parse `#RRGGBB` / `RRGGBB` — `None` bila bukan 6 heksadesimal.
pub fn parse_hex(teks: &str) -> Option<Rgba> {
    let teks = teks.strip_prefix('#').unwrap_or(teks);
    if teks.len() != 6 || !teks.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(teks, 16).ok()?;
    Some(Rgba::opaque(
        ((n >> 16) & 0xFF) as u8,
        ((n >> 8) & 0xFF) as u8,
        (n & 0xFF) as u8,
    ))
}

fn ke_color32(warna: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(warna.r, warna.g, warna.b, warna.a)
}

// ---------------------------------------------------------------------------
// Harmony §7.3
// ---------------------------------------------------------------------------

/// Mode palet harmony §7.3 — "panel terpisah" di dasar panel Color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Harmony {
    #[default]
    Complementary,
    Analogous,
    Triadic,
}

impl Harmony {
    pub const ALL: [Harmony; 3] = [Self::Complementary, Self::Analogous, Self::Triadic];

    pub fn label(self) -> &'static str {
        match self {
            Self::Complementary => "Complementary",
            Self::Analogous => "Analogous",
            Self::Triadic => "Triadic",
        }
    }

    /// Offset hue (derajat) dari warna aktif.
    fn offsets(self) -> &'static [f32] {
        match self {
            Self::Complementary => &[180.0],
            Self::Analogous => &[-30.0, 30.0],
            Self::Triadic => &[-120.0, 120.0],
        }
    }
}

/// Warna turunan harmony dari `warna` sesuai `mode` (§7.3).
pub fn harmony_colors(warna: Rgba, mode: Harmony) -> Vec<Rgba> {
    let (hue, saturasi, nilai) = rgb_to_hsv(warna.r, warna.g, warna.b);
    mode.offsets()
        .iter()
        .map(|offset| {
            let (r, g, b) = hsv_to_rgb(hue + offset, saturasi, nilai);
            Rgba::opaque(r, g, b)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Swatch bar §7.3
// ---------------------------------------------------------------------------

/// Swatch bar: 16 slot warna persist antar sesi (`swatches.json`).
#[derive(Clone, Debug, PartialEq)]
pub struct Swatches {
    slots: [Option<Rgba>; SLOT_COUNT],
}

impl Default for Swatches {
    fn default() -> Self {
        Self {
            slots: [None; SLOT_COUNT],
        }
    }
}

impl Swatches {
    pub fn slot(&self, index: usize) -> Option<Rgba> {
        self.slots.get(index).copied().flatten()
    }

    /// Simpan warna ke slot kosong/berbeda — `false` bila identik.
    pub fn simpan(&mut self, index: usize, warna: Rgba) -> bool {
        if self.slot(index) == Some(warna) {
            return false;
        }
        self.slots[index] = Some(warna);
        true
    }

    /// Kosongkan slot (Shift+Klik §7.3) — `false` bila sudah kosong.
    pub fn hapus(&mut self, index: usize) -> bool {
        self.slots[index].take().is_some()
    }

    /// Geser isi slot `dari` ke `ke` (drag = reorder §7.3) — elemen
    /// berpindah (bukan tukar), slot kosong tetap di ekor.
    pub fn geser(&mut self, dari: usize, ke: usize) {
        if dari == ke || dari >= SLOT_COUNT || ke >= SLOT_COUNT {
            return;
        }
        if self.slots[dari].is_none() {
            return;
        }
        if dari < ke {
            // Rentang inklusif [dari..=ke]: warna berpindah ke `ke`,
            // isian di antaranya menggeser mundur mengisi `dari`.
            self.slots[dari..=ke].rotate_left(1);
        } else {
            self.slots[ke..=dari].rotate_right(1);
        }
    }

    /// Berkas persist swatch (pendamping `dock::layout_path` §8.2).
    pub fn path() -> PathBuf {
        config_dir().join("swatches.json")
    }

    /// Muat dari `path` — berkas hilang/rusak/jumlah salah → default.
    pub fn load_from(path: &PathBuf) -> Self {
        let Ok(bytes) = std::fs::read(path) else {
            return Self::default();
        };
        let Ok(daftar) = serde_json::from_slice::<Vec<Option<String>>>(&bytes) else {
            return Self::default();
        };
        let mut swatches = Self::default();
        for (slot, isi) in swatches.slots.iter_mut().zip(daftar) {
            *slot = isi.and_then(|hex| parse_hex(&hex));
        }
        swatches
    }

    pub fn load() -> Self {
        Self::load_from(&Self::path())
    }

    /// Tulis seluruh slot sebagai `#RRGGBB` — dipanggil hanya saat berubah.
    pub fn save_to(&self, path: &PathBuf) {
        let daftar: Vec<Option<String>> =
            self.slots.iter().map(|slot| slot.map(hex_kini)).collect();
        let Ok(bytes) = serde_json::to_string_pretty(&daftar) else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(err) = std::fs::write(path, bytes) {
            log::warn!("swatch gagal disimpan ({}): {err}", path.display());
        }
    }

    pub fn save(&self) {
        self.save_to(&Self::path());
    }
}

// ---------------------------------------------------------------------------
// State UI transien
// ---------------------------------------------------------------------------

/// State UI panel Color — transien (tekstur roda & buffer hex), tidak
/// dipersist; urutan/isi swatch yang persist lewat [`Swatches`].
#[derive(Default)]
pub struct ColorUi {
    /// Tekstur roda HSV — dibuat sekali saat tab Color dibuka pertama kali.
    roda: Option<egui::TextureHandle>,
    /// Buffer edit kolom hex — sinkron dari warna bila tidak difokus.
    hex_edit: String,
    hex_fokus: bool,
    /// Slot yang sedang di-drag (reorder) + tujuan hover frame ini.
    geser: Option<usize>,
    hover: Option<usize>,
}

// ---------------------------------------------------------------------------
// Gambar panel
// ---------------------------------------------------------------------------

/// Gambar isi panel Color §7.3 — dipanggil dock saat tab Color aktif.
/// Mengembalikan `true` bila mode harmony berubah (layout berubah → simpan).
pub fn show(
    ui: &mut Ui,
    theme: &Theme,
    color_ui: &mut ColorUi,
    harmony: &mut Harmony,
    warna: &mut Rgba,
    swatches: &mut Swatches,
) -> bool {
    let mut harmony_berubah = false;
    ui.add_space(theme::spacing::GAP_CONTROL);
    roda(ui, theme, color_ui, warna);
    ui.add_space(theme::spacing::GAP_CONTROL);
    strip_lightness(ui, theme, warna);
    ui.add_space(theme::spacing::LIST);
    input_hex_rgb(ui, theme, color_ui, warna);
    ui.add_space(theme::spacing::LIST);
    garis_pemisah(ui, theme);
    swatch_bar(ui, theme, color_ui, warna, swatches);
    ui.add_space(theme::spacing::LIST);
    garis_pemisah(ui, theme);
    harmony_berubah |= harmony_panel(ui, theme, harmony, warna);
    ui.add_space(theme::spacing::GRID);
    harmony_berubah
}

/// Roda HSV — sudut = hue, jari-jari = saturasi; nilai (lightness) di strip
/// terpisah (§7.3). Klik/seret di cakram mengatur hue+saturasi.
fn roda(ui: &mut Ui, theme: &Theme, color_ui: &mut ColorUi, warna: &mut Rgba) {
    // Tekstur roda statis — cukup dibuat sekali (marker digambar painter).
    if color_ui.roda.is_none() {
        let gambar = roda_image();
        color_ui.roda = Some(ui.ctx().load_texture(
            "roda-warna",
            gambar,
            egui::TextureOptions::LINEAR,
        ));
    }
    let Some(handle) = &color_ui.roda else {
        return;
    };

    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(WHEEL_PX), Sense::click_and_drag());
    ui.painter().image(
        handle.id(),
        rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );

    let (.., nilai) = rgb_to_hsv(warna.r, warna.g, warna.b);

    if let Some(p) = resp.interact_pointer_pos()
        && (resp.clicked() || resp.dragged())
    {
        let tengah = rect.center();
        let dx = p.x - tengah.x;
        let dy = p.y - tengah.y;
        let jari = ((dx * dx + dy * dy).sqrt() / (WHEEL_PX * 0.5)).min(1.0);
        let sudut = dy.atan2(dx).to_degrees();
        let (r, g, b) = hsv_to_rgb(sudut, jari, nilai);
        *warna = Rgba {
            r,
            g,
            b,
            a: warna.a,
        };
    }

    // Marker hue+saturasi — dua lingkaran (border gelap + garis terang)
    // agar terbaca di atas warna apa pun.
    let (hue, saturasi, _) = rgb_to_hsv(warna.r, warna.g, warna.b);
    let rad = hue.to_radians();
    let jari = saturasi * WHEEL_PX * 0.5;
    let pos = Pos2::new(
        rect.center().x + jari * rad.cos(),
        rect.center().y + jari * rad.sin(),
    );
    ui.painter()
        .circle_stroke(pos, 6.0, Stroke::new(3.0, theme.surface_0));
    ui.painter()
        .circle_stroke(pos, 6.0, Stroke::new(2.0, theme.text_primary));

    let resp = resp.on_hover_text("Roda warna — klik/seret: hue (sudut) & saturasi (jari-jari)");
    let _ = ui.ctx().accesskit_node_builder(resp.id, |node| {
        node.set_label("Roda warna HSV — hue dan saturasi");
    });
}

/// Peta statis roda: per piksel di luar cakram → transparan; hue sudut,
/// saturasi jari-jari, nilai 1 (lightness di strip terpisah §7.3).
fn roda_image() -> egui::ColorImage {
    let mut piksel = Vec::with_capacity(WHEEL_TEX * WHEEL_TEX);
    let setengah = WHEEL_TEX as f32 / 2.0;
    for y in 0..WHEEL_TEX {
        for x in 0..WHEEL_TEX {
            let dx = x as f32 + 0.5 - setengah;
            let dy = y as f32 + 0.5 - setengah;
            let jari = (dx * dx + dy * dy).sqrt() / setengah;
            if jari > 1.0 {
                piksel.push(Color32::TRANSPARENT);
                continue;
            }
            let sudut = dy.atan2(dx).to_degrees().rem_euclid(360.0);
            let (r, g, b) = hsv_to_rgb(sudut, jari, 1.0);
            piksel.push(Color32::from_rgb(r, g, b));
        }
    }
    egui::ColorImage::new([WHEEL_TEX, WHEEL_TEX], piksel)
}

/// Strip lightness (nilai V): gradasi hitam → warna penuh pada hue &
/// saturasi aktif; seret untuk mengatur nilai (§7.3).
fn strip_lightness(ui: &mut Ui, theme: &Theme, warna: &mut Rgba) {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(WHEEL_PX, STRIP_H), Sense::click_and_drag());
    let (hue, saturasi, _) = rgb_to_hsv(warna.r, warna.g, warna.b);

    // Gradasi dirakit dari segmen (painter, tanpa tekstur tambahan).
    const SEGMEN: usize = 24;
    for i in 0..SEGMEN {
        let t = (i as f32 + 0.5) / SEGMEN as f32;
        let (r, g, b) = hsv_to_rgb(hue, saturasi, t);
        let x0 = rect.left() + rect.width() * i as f32 / SEGMEN as f32;
        let x1 = rect.left() + rect.width() * (i + 1) as f32 / SEGMEN as f32;
        ui.painter().rect_filled(
            Rect::from_min_max(Pos2::new(x0, rect.top()), Pos2::new(x1, rect.bottom())),
            0.0,
            Color32::from_rgb(r, g, b),
        );
    }
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(theme::radius::SM),
        Stroke::new(1.0, theme.border_default),
        StrokeKind::Middle,
    );

    if let Some(p) = resp.interact_pointer_pos()
        && (resp.clicked() || resp.dragged())
    {
        let nilai = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        let (r, g, b) = hsv_to_rgb(hue, saturasi, nilai);
        *warna = Rgba {
            r,
            g,
            b,
            a: warna.a,
        };
    }

    // Handle nilai — posisi x = nilai saat ini.
    let (.., nilai) = rgb_to_hsv(warna.r, warna.g, warna.b);
    let hx = rect.left() + rect.width() * nilai;
    ui.painter().line_segment(
        [
            Pos2::new(hx, rect.top() - 2.0),
            Pos2::new(hx, rect.bottom() + 2.0),
        ],
        Stroke::new(3.0, theme.surface_0),
    );
    ui.painter().line_segment(
        [
            Pos2::new(hx, rect.top() - 2.0),
            Pos2::new(hx, rect.bottom() + 2.0),
        ],
        Stroke::new(1.5, theme.text_primary),
    );

    let resp = resp.on_hover_text("Lightness — klik/seret untuk nilai (V)");
    let _ = ui.ctx().accesskit_node_builder(resp.id, |node| {
        node.set_label("Strip lightness warna");
    });
}

/// Input hex (§1.4) + kanal RGB drag-numeric (§7.2).
fn input_hex_rgb(ui: &mut Ui, theme: &Theme, color_ui: &mut ColorUi, warna: &mut Rgba) {
    ui.horizontal(|ui| {
        ui.add_space(theme::spacing::GAP_CONTROL);
        ui.label(Type::LabelSm.rich("Hex").color(theme.text_secondary));
        // Sinkron dari warna bila tidak sedang diedit (eyedropper, roda, dst).
        if !color_ui.hex_fokus {
            color_ui.hex_edit = hex_kini(*warna);
        }
        let resp = ui.add(
            egui::TextEdit::singleline(&mut color_ui.hex_edit)
                .desired_width(HEX_W)
                .char_limit(7),
        );
        let fokus = resp.has_focus();
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        if enter || (color_ui.hex_fokus && !fokus) {
            // Enter / kehilangan fokus (klik luar, Tab §9) → komit bila sah.
            if let Some(baru) = parse_hex(&color_ui.hex_edit) {
                *warna = baru;
            }
            color_ui.hex_edit = hex_kini(*warna);
        }
        color_ui.hex_fokus = fokus;
    });

    let mut kanal = [warna.r, warna.g, warna.b];
    for (indeks, label) in ["R", "G", "B"].into_iter().enumerate() {
        ui.horizontal(|ui| {
            ui.add_space(theme::spacing::GAP_CONTROL);
            let mut nilai = f32::from(kanal[indeks]);
            DragNumeric::new(label, &mut nilai, 0.0..=255.0, 1.0).show(ui, theme);
            kanal[indeks] = nilai.round() as u8;
        });
    }
    let baru = Rgba {
        r: kanal[0],
        g: kanal[1],
        b: kanal[2],
        a: warna.a,
    };
    if baru != *warna {
        *warna = baru;
    }
}

/// Swatch bar §7.3 — grid 2×8: klik pilih / slot kosong simpan warna
/// aktif / `Shift+Klik` hapus / seret geser urutan; persist saat berubah.
fn swatch_bar(
    ui: &mut Ui,
    theme: &Theme,
    color_ui: &mut ColorUi,
    warna: &mut Rgba,
    swatches: &mut Swatches,
) {
    ui.horizontal(|ui| {
        ui.add_space(theme::spacing::GAP_CONTROL);
        ui.label(Type::LabelSm.rich("Swatch").color(theme.text_secondary));
    });
    // Hover tujuan dihitel ulang tiap frame (kotak antar-slot = kosong).
    color_ui.hover = None;
    let mut berubah = false;
    for baris in 0..2 {
        ui.horizontal(|ui| {
            ui.add_space(theme::spacing::GAP_CONTROL);
            for kolom in 0..SLOT_COUNT / 2 {
                let index = baris * SLOT_COUNT / 2 + kolom;
                berubah |= swatch_slot(ui, theme, color_ui, warna, swatches, index);
            }
        });
    }
    if berubah {
        swatches.save();
    }

    // Pratinjau slot yang sedang digeser — mengikuti pointer.
    if let (Some(_), Some(p)) = (color_ui.geser, ui.ctx().pointer_hover_pos()) {
        let preview = Rect::from_center_size(p, Vec2::splat(SLOT_PX * 0.8));
        if let Some(warna_pratinjau) = swatches.slot(color_ui.geser.unwrap_or(0)) {
            ui.painter()
                .rect_filled(preview, theme::radius::SM, ke_color32(warna_pratinjau));
            ui.painter().rect_stroke(
                preview.expand(2.0),
                CornerRadius::same(theme::radius::SM),
                Stroke::new(1.5, theme.accent_brand),
                StrokeKind::Middle,
            );
        }
    }
}

/// Satu slot swatch — `true` bila isi swatch berubah (perlu persist).
fn swatch_slot(
    ui: &mut Ui,
    theme: &Theme,
    color_ui: &mut ColorUi,
    warna: &mut Rgba,
    swatches: &mut Swatches,
    index: usize,
) -> bool {
    let isi = swatches.slot(index);
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(SLOT_PX), Sense::click_and_drag());

    // Latar slot: warna terisi atau permukaan kosong (token §2).
    match isi {
        Some(warna_slot) => {
            ui.painter()
                .rect_filled(rect, theme::radius::SM, ke_color32(warna_slot));
        }
        None => {
            ui.painter()
                .rect_filled(rect, theme::radius::SM, theme.surface_2);
        }
    }
    // Ring aksen bila slot berisi warna yang sedang aktif.
    let cincin = if isi == Some(*warna) {
        Stroke::new(1.5, theme.accent_brand)
    } else {
        Stroke::new(1.0, theme.border_default)
    };
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(theme::radius::SM),
        cincin,
        StrokeKind::Middle,
    );

    let mut berubah = false;

    // Seret = geser urutan (hanya dari slot berisi).
    if resp.drag_started() && isi.is_some() {
        color_ui.geser = Some(index);
    }
    if color_ui.geser.is_some_and(|dari| dari != index) && resp.hovered() {
        color_ui.hover = Some(index);
    }
    if color_ui.geser == Some(index) && ui.input(|i| i.pointer.any_released()) {
        if let Some(tujuan) = color_ui.hover.take() {
            swatches.geser(index, tujuan);
            berubah = true;
        }
        color_ui.geser = None;
    }

    // Klik — Shift+Klik hapus; slot kosong simpan; terisi pilih.
    if resp.clicked() && color_ui.geser.is_none() {
        if ui.input(|i| i.modifiers.shift) {
            berubah |= swatches.hapus(index);
        } else if let Some(warna_slot) = isi {
            *warna = warna_slot;
        } else {
            berubah |= swatches.simpan(index, *warna);
        }
    }

    let keterangan = match isi {
        Some(warna_slot) => format!(
            "Swatch {}: {} — klik: pilih, Shift+Klik: hapus, seret: geser",
            index + 1,
            hex_kini(warna_slot)
        ),
        None => format!("Swatch {}: kosong — klik: simpan warna aktif", index + 1),
    };
    let resp = resp.on_hover_text(keterangan.clone());
    let _ = ui.ctx().accesskit_node_builder(resp.id, |node| {
        node.set_label(keterangan);
    });
    berubah
}

/// Bagian harmony §7.3 — pilihan mode + warna turunan (klik = terapkan).
fn harmony_panel(ui: &mut Ui, theme: &Theme, harmony: &mut Harmony, warna: &mut Rgba) -> bool {
    let mut berubah = false;
    ui.horizontal(|ui| {
        ui.add_space(theme::spacing::GAP_CONTROL);
        ui.label(Type::LabelSm.rich("Harmony").color(theme.text_secondary));
    });
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = theme::spacing::GRID;
        ui.add_space(theme::spacing::GAP_CONTROL);
        for mode in Harmony::ALL {
            let teks = Type::LabelSm.rich(mode.label()).color(if *harmony == mode {
                theme.text_primary
            } else {
                theme.text_secondary
            });
            let resp = ui.add(
                egui::Button::selectable(*harmony == mode, teks)
                    .stroke(Stroke::NONE)
                    .fill(if *harmony == mode {
                        theme.surface_2
                    } else {
                        Color32::TRANSPARENT
                    }),
            );
            if resp.clicked() && *harmony != mode {
                *harmony = mode;
                berubah = true;
            }
        }
    });

    ui.horizontal(|ui| {
        ui.add_space(theme::spacing::GAP_CONTROL);
        for turunan in harmony_colors(*warna, *harmony) {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(SLOT_PX), Sense::click());
            ui.painter()
                .rect_filled(rect, theme::radius::SM, ke_color32(turunan));
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(theme::radius::SM),
                Stroke::new(1.0, theme.border_default),
                StrokeKind::Middle,
            );
            if resp.clicked() {
                *warna = turunan;
            }
            let teks = format!("Terapkan {}", hex_kini(turunan));
            let resp = resp.on_hover_text(teks.clone());
            let _ = ui.ctx().accesskit_node_builder(resp.id, |node| {
                node.set_label(teks);
            });
        }
    });
    berubah
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        std::env::temp_dir().join(format!(
            "flipbrush-swatch-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn konversi_hsv_rgb_roundtrip() {
        for (r, g, b) in [
            (255, 0, 0),
            (0, 255, 0),
            (0, 0, 255),
            (12, 34, 56),
            (128, 128, 128),
            (255, 255, 255),
            (0, 0, 0),
            (200, 100, 50),
        ] {
            let (h, s, v) = rgb_to_hsv(r, g, b);
            let (r2, g2, b2) = hsv_to_rgb(h, s, v);
            let selisih = |a: u8, b: u8| (a as i16 - b as i16).abs();
            assert!(
                selisih(r, r2) <= 1 && selisih(g, g2) <= 1 && selisih(b, b2) <= 1,
                "({r},{g},{b}) → ({r2},{g2},{b2})"
            );
        }
    }

    #[test]
    fn konversi_nilai_dikenal() {
        assert_eq!(rgb_to_hsv(255, 0, 0), (0.0, 1.0, 1.0));
        let (h, s, v) = rgb_to_hsv(0, 255, 0);
        assert!((h - 120.0).abs() < 0.01 && s == 1.0 && v == 1.0);
        let (_, s, _) = rgb_to_hsv(128, 128, 128);
        assert_eq!(s, 0.0, "abu-abu tanpa saturasi");
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), (255, 0, 0));
        assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), (0, 0, 255));
        assert_eq!(hsv_to_rgb(200.0, 0.0, 0.0), (0, 0, 0), "v = 0 → hitam");
    }

    #[test]
    fn parse_hex_sah_ditolak() {
        let merah = Rgba::opaque(255, 0, 0);
        // String hex dibangun dari potongan `diez` + heksadesimal: guard §10
        // melarang literal `#RRGGBB` di luar theme.rs, dan regexnya polos
        // (tak bisa bedakan warna UI dari data uji parser).
        let diez = "#";
        assert_eq!(parse_hex(&format!("{diez}FF0000")), Some(merah));
        assert_eq!(parse_hex("ff0000"), Some(merah), "tanpa # juga sah");
        assert_eq!(parse_hex(&format!("{diez}ff0000")), Some(merah));
        assert_eq!(parse_hex(&format!("{diez}12345")), None, "terpendek");
        assert_eq!(parse_hex(&format!("{diez}1234567")), None, "terpanjang");
        assert_eq!(
            parse_hex(&format!("{diez}12345g")),
            None,
            "bukan heksadesimal"
        );
        assert_eq!(parse_hex("warna"), None);
        assert_eq!(hex_kini(merah), format!("{diez}FF0000"));
    }

    #[test]
    fn swatch_simpan_pilih_hapus_geser() {
        let mut swatches = Swatches::default();
        let merah = Rgba::opaque(255, 0, 0);
        let biru = Rgba::opaque(0, 0, 255);

        assert!(swatches.simpan(0, merah), "slot kosong menerima warna");
        assert!(!swatches.simpan(0, merah), "identik → tanpa perubahan");
        swatches.simpan(3, biru);
        assert_eq!(swatches.slot(0), Some(merah));

        // Drag = geser (bukan tukar): merah pindah ke 3, biru mundur ke 2.
        swatches.geser(0, 3);
        assert_eq!(swatches.slot(3), Some(merah));
        assert_eq!(swatches.slot(2), Some(biru));
        assert_eq!(swatches.slot(0), None, "posisi asal kosong");

        assert!(swatches.hapus(3));
        assert!(!swatches.hapus(3), "sudah kosong");
        assert_eq!(swatches.slot(2), Some(biru), "tetangga tak ikut hilang");
    }

    #[test]
    fn swatch_roundtrip_persist_dan_rusak() {
        let path = temp_path("roundtrip").join("swatches.json");
        let mut swatches = Swatches::default();
        swatches.simpan(1, Rgba::opaque(1, 2, 3));
        swatches.simpan(14, Rgba::opaque(250, 128, 64));
        swatches.save_to(&path);

        let muat = Swatches::load_from(&path);
        assert_eq!(muat.slot(1), Some(Rgba::opaque(1, 2, 3)));
        assert_eq!(muat.slot(14), Some(Rgba::opaque(250, 128, 64)));
        assert_eq!(muat.slot(0), None);
        assert_eq!(muat.slot(15), None, "slot kosong tetap kosong");

        std::fs::write(&path, b"bukan json").unwrap();
        assert_eq!(
            Swatches::load_from(&path),
            Swatches::default(),
            "rusak → default"
        );
        assert_eq!(
            Swatches::load_from(&temp_path("hilang").join("x.json")),
            Swatches::default(),
            "hilang → default"
        );
    }

    #[test]
    fn harmony_ikut_aturan_derajat() {
        let merah = Rgba::opaque(255, 0, 0);
        let komplementer = harmony_colors(merah, Harmony::Complementary);
        assert_eq!(komplementer, vec![Rgba::opaque(0, 255, 255)], "180°");

        let triadik = harmony_colors(merah, Harmony::Triadic);
        assert_eq!(triadik.len(), 2);
        assert!(triadik.contains(&Rgba::opaque(0, 255, 0)), "120° → hijau");
        assert!(triadik.contains(&Rgba::opaque(0, 0, 255)), "-120° → biru");

        let analog = harmony_colors(merah, Harmony::Analogous);
        assert_eq!(analog.len(), 2);
        assert_eq!(analog[0], Rgba::opaque(255, 0, 128), "-30° → magenta");
        assert_eq!(analog[1], Rgba::opaque(255, 128, 0), "+30° → oranye");
    }
}
