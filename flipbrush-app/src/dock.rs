//! Panel dock kanan (DESIGN §7.6 & §8.2) — M4.
//!
//! Dock 280 px di tepi kanan (rentang resize 220–460 px §8.2) dengan tiga
//! tab sesuai diagram §8.2: Color, Brush, Layers. Header panel memakai
//! `label-md` + chevron collapse + menu ⋮ (§7.6); strip aksen 2 px menandai
//! tab aktif (kiasan §7.4). Seluruh state layout **persist antar sesi** di
//! `$XDG_CONFIG_HOME/flipbrush/layout.json` — file kecil yang hanya
//! ditulis saat ada perubahan + pointer dilepas (tanpa I/O per frame).
//!
//! Konten panel digambar lewat [`show`] dengan state dipinjam dari
//! aplikasi; [`DockOutcome`] memberi tahu aplikasi bila ada perubahan
//! konten proyek (autosave §13.2) atau permintaan mode fokus (§8.2).

use std::path::{Path, PathBuf};

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Layout, Pos2, Rect, Response, Sense, Stroke, Ui,
    Vec2,
};
use serde::{Deserialize, Serialize};

use flipbrush_core::anim::{Animation, LayerId, MAX_LAYERS};
use flipbrush_core::raster::{Brush, Rgba};

use crate::color::{self, ColorUi, Harmony, Swatches};
use crate::icon::{self, Icon};
use crate::slider::DragNumeric;
use crate::theme::{self, Theme};
use crate::timeline::Timeline;
use crate::typography::Type;

/// Lebar default dock §8.2.
pub const DOCK_W: f32 = 280.0;
/// Lebar minimum dock (§8.2: ukuran dock bisa di-resize).
const DOCK_MIN: f32 = 220.0;
/// Lebar maksimum dock — sisanya tetap kanvas (§1: canvas-first).
const DOCK_MAX: f32 = 460.0;
/// ID panel egui — juga kunci state ukuran egui (reset lebar menu ⋮).
const ID_PANEL: &str = "panel-dock";
/// Tinggi item menu §7.8.
const MENU_ITEM_H: f32 = 28.0;
/// Tinggi baris daftar layer — kelipatan spacing §4.
const ROW_H: f32 = theme::spacing::SECTION;
/// Sisi tombol glyph (chevron/titik menu).
const GLYPH_PX: f32 = 20.0;
/// Lebar isi menu §7.8.
const MENU_W: f32 = 200.0;

// ---------------------------------------------------------------------------
// Layout persist (§8.2)
// ---------------------------------------------------------------------------

/// Tab panel di dock — urut sesuai diagram §8.2 (Color, Brush, Layers).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelTab {
    Color,
    #[default]
    Brush,
    Layers,
}

impl PanelTab {
    /// Urutan tampil di strip tab (diagram §8.2).
    pub const ALL: [PanelTab; 3] = [Self::Color, Self::Brush, Self::Layers];

    pub fn label(self) -> &'static str {
        match self {
            Self::Color => "Color",
            Self::Brush => "Brush",
            Self::Layers => "Layers",
        }
    }
}

/// State dock + layout persist per sesi (§8.2: "state layout persist").
/// Default per-field (bukan struktural) — serde tak boleh memindah sebagian
/// struct yang punya `Drop` (flush keluar program).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dock {
    /// Lebar efektif — ikut hasil drag pemisah, disimpan antar sesi.
    #[serde(default = "lebar_default")]
    pub width: f32,
    /// Tab panel yang tampil.
    #[serde(default = "tab_default")]
    pub tab: PanelTab,
    /// Header saja (§7.6: panel bisa di-collapse).
    #[serde(default)]
    pub collapsed: bool,
    /// Mode palet harmony §7.3 — preferensi panel, ikut layout persist.
    #[serde(default = "harmony_default")]
    pub harmony: Harmony,
    /// Penamaan inline `(id, nama_asli)` — Esc membatalkan (§9). Transien,
    /// tidak ikut dipersist.
    #[serde(skip)]
    renaming: Option<(LayerId, String)>,
    /// Ada perubahan yang belum ditulis ke [`Self::path`].
    #[serde(skip)]
    dirty: bool,
    /// Berkas layout — produksi memakai [`layout_path`]; tes menyuntikkan
    /// folder temp agar tidak menyentuh `~/.config`.
    #[serde(skip)]
    path: PathBuf,
}

impl Default for Dock {
    fn default() -> Self {
        Self {
            width: lebar_default(),
            tab: tab_default(),
            collapsed: false,
            harmony: harmony_default(),
            renaming: None,
            dirty: false,
            path: layout_path(),
        }
    }
}

fn lebar_default() -> f32 {
    DOCK_W
}

fn tab_default() -> PanelTab {
    PanelTab::Brush
}

fn harmony_default() -> Harmony {
    Harmony::default()
}

impl Dock {
    /// Muat layout sesi sebelumnya — file hilang/rusak → default (§8.2).
    pub fn load() -> Self {
        Self::load_from(&layout_path())
    }

    /// Muat dari `path` tertentu — field asing/rusak diabaikan (serde);
    /// lebar di-clamp agar file korup tak membuat dock tak terpakai.
    pub fn load_from(path: &Path) -> Self {
        let mut dock = std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Dock>(&bytes).ok())
            .unwrap_or_default();
        dock.path = path.to_owned();
        dock.width = dock.width.clamp(DOCK_MIN, DOCK_MAX);
        dock
    }

    /// Tulis bila ada perubahan (kegagalan dicatat, `dirty` bertahan agar
    /// percobaan berikutnya jalan).
    fn persist(&mut self) {
        if !self.dirty {
            return;
        }
        let bytes = match serde_json::to_string_pretty(&*self) {
            Ok(bytes) => bytes,
            Err(err) => {
                log::warn!("layout dock gagal diserialisasi: {err}");
                return;
            }
        };
        if let Some(parent) = self.path.parent()
            && let Err(err) = std::fs::create_dir_all(parent)
        {
            log::warn!("folder config layout gagal dibuat: {err}");
            return;
        }
        match std::fs::write(&self.path, bytes) {
            Ok(()) => self.dirty = false,
            Err(err) => log::warn!(
                "layout dock gagal disimpan ({}): {err}",
                self.path.display()
            ),
        }
    }

    /// Simpan tertunda saat pointer baru dilepas — aksi resize/tab selalu
    /// berawal dari klik, jadi I/O maksimal sekali per aksi (bukan per
    /// frame, §13.3).
    fn persist_when_idle(&mut self, ctx: &egui::Context) {
        if self.dirty && ctx.input(|i| i.pointer.any_released()) {
            self.persist();
        }
    }

    /// Paksa simpan tertunda (menu + Drop keluar program).
    pub fn flush(&mut self) {
        self.persist();
    }

    /// Tampilkan `tab` dari luar dock (mis. klik kotak warna di kepala
    /// tool rail §7.4 → tab Color §7.3): header yang sedang di-collapse
    /// dibuka lagi agar panel benar-benar terlihat; tandai `dirty` supaya
    /// pilihan tab ikut persist (§8.2).
    pub fn buka_tab(&mut self, tab: PanelTab) {
        self.tab = tab;
        self.collapsed = false;
        self.dirty = true;
    }
}

impl Drop for Dock {
    fn drop(&mut self) {
        self.persist();
    }
}

/// Direktori konfigurasi (`$XDG_CONFIG_HOME` atau `~/.config`) + `flipbrush`
/// — pendamping [`crate::project::data_dir`] (XDG Base Directory).
pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("flipbrush")
}

/// Berkas layout persist dock (§8.2: restore antar sesi).
pub fn layout_path() -> PathBuf {
    config_dir().join("layout.json")
}

// ---------------------------------------------------------------------------
// Interface ke aplikasi
// ---------------------------------------------------------------------------

/// State panel yang dipinjam dari aplikasi selama satu frame.
pub struct PanelState<'a> {
    pub anim: &'a mut Animation,
    pub timeline: &'a mut Timeline,
    pub brush: &'a mut Brush,
    /// Warna aktif (kuas) — diedit Panel Color §7.3.
    pub color: &'a mut Rgba,
    /// Swatch bar §7.3 — persist `swatches.json`.
    pub swatches: &'a mut Swatches,
    /// State UI transien Panel Color (tekstur roda, buffer hex).
    pub color_ui: &'a mut ColorUi,
}

/// Kabar balik dari dock ke aplikasi (§13.2 autosave, §8.2 mode fokus).
#[derive(Default)]
pub struct DockOutcome {
    /// Konten proyek berubah (mis. visibilitas/nama layer) → autosave §13.2.
    pub content_changed: bool,
    /// Pengguna memilih "Sembunyikan dock" → mode fokus §8.2.
    pub enter_focus: bool,
}

// ---------------------------------------------------------------------------
// Gambar dock
// ---------------------------------------------------------------------------

/// Gambar dock kanan — dipanggil aplikasi setelah options bar & sebelum
/// timeline (urutan panel §8.2: dock memanen kolom kanan penuh).
pub fn show(ui: &mut Ui, theme: &Theme, dock: &mut Dock, state: PanelState<'_>) -> DockOutcome {
    let mut outcome = DockOutcome::default();
    let inner = egui::Panel::right(ID_PANEL)
        .resizable(true)
        .default_size(dock.width)
        .min_size(DOCK_MIN)
        .max_size(DOCK_MAX)
        .frame(egui::Frame::NONE.fill(theme.surface_1))
        .show(ui, |ui| {
            ui.add_space(theme::spacing::GRID);
            if dock.collapsed {
                // §7.6: collapse = hanya header.
                show_header(ui, theme, dock, state.anim, &mut outcome);
            } else {
                show_tabs(ui, theme, dock, state.anim, &mut outcome);
                ui.add_space(theme::spacing::GRID);
                show_header(ui, theme, dock, state.anim, &mut outcome);
                ui.add_space(theme::spacing::GAP_CONTROL);
                match dock.tab {
                    PanelTab::Brush => brush_panel(ui, theme, state.brush),
                    PanelTab::Color => {
                        if color::show(
                            ui,
                            theme,
                            state.color_ui,
                            &mut dock.harmony,
                            state.color,
                            state.swatches,
                        ) {
                            dock.dirty = true;
                        }
                    }
                    PanelTab::Layers => {
                        layers_panel(ui, theme, dock, state.anim, state.timeline, &mut outcome)
                    }
                }
            }
        });

    // §8.2: hasil drag pemisah → state layout (disimpan saat pointer
    // dilepas; lihat `persist_when_idle`).
    let lebar = inner.response.rect.width();
    if (lebar - dock.width).abs() > 0.5 {
        dock.width = lebar;
        dock.dirty = true;
    }
    dock.persist_when_idle(ui.ctx());
    outcome
}

/// Strip tab §7.6 — tab aktif `label-md` `text/primary` + strip aksen 2 px
/// di tepi bawah (kiasan §7.4); divider `border/default` di bawah strip.
fn show_tabs(
    ui: &mut Ui,
    theme: &Theme,
    dock: &mut Dock,
    anim: &mut Animation,
    outcome: &mut DockOutcome,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for tab in PanelTab::ALL {
            let aktif = dock.tab == tab;
            let teks = Type::LabelMd.rich(tab.label()).color(if aktif {
                theme.text_primary
            } else {
                theme.text_secondary
            });
            let tombol = egui::Button::new(teks)
                .fill(if aktif {
                    theme.surface_2
                } else {
                    Color32::TRANSPARENT
                })
                .stroke(Stroke::NONE)
                .corner_radius(CornerRadius::ZERO);
            let resp = ui.add(tombol);
            if resp.clicked() && !aktif {
                // Penamaan inline yang terputus → komit (lihat `finish_renaming`).
                finish_renaming(dock, anim, outcome);
                dock.tab = tab;
                dock.dirty = true;
            }
            if aktif {
                let strip = Rect::from_min_size(
                    Pos2::new(resp.rect.left(), resp.rect.bottom() - 2.0),
                    Vec2::new(resp.rect.width(), 2.0),
                );
                ui.painter().rect_filled(strip, 0.0, theme.accent_brand);
            }
        }
    });
    garis_pemisah(ui, theme);
}

/// Header panel §7.6: `label-md` nama panel + chevron collapse + menu ⋮.
fn show_header(
    ui: &mut Ui,
    theme: &Theme,
    dock: &mut Dock,
    anim: &mut Animation,
    outcome: &mut DockOutcome,
) {
    ui.horizontal(|ui| {
        ui.add_space(theme::spacing::GAP_CONTROL);
        ui.label(
            Type::LabelMd
                .rich(dock.tab.label())
                .color(theme.text_primary),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            menu_panel(ui, theme, dock, outcome);
            let arah = if dock.collapsed {
                Arah::Kanan
            } else {
                Arah::Bawah
            };
            let resp = tombol_glyph(
                ui,
                theme,
                true,
                |painter, rect, warna| chevron(painter, rect, warna, arah),
                "Ciutkan panel",
            );
            if resp.clicked() {
                finish_renaming(dock, anim, outcome);
                dock.collapsed = !dock.collapsed;
                dock.dirty = true;
            }
        });
    });
    garis_pemisah(ui, theme);
}

/// Menu ⋮ header panel (§7.6/§7.8) di atas tombol glyph sendiri — popup
/// egui menangani buka/tutup/klik-luar.
fn menu_panel(ui: &mut Ui, theme: &Theme, dock: &mut Dock, outcome: &mut DockOutcome) {
    let resp = tombol_glyph(ui, theme, true, titik_tiga, "Menu panel");
    let _ = egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(MENU_W);
        let reset = menu_item(ui, theme, "Atur ulang lebar", None);
        if reset.clicked() {
            dock.width = DOCK_W;
            dock.dirty = true;
            // State ukuran panel egui juga harus disetel ulang — tanpa
            // ini `default_size` baru diabaikan oleh panel yang sudah ada.
            if let Some(mut state) = egui::PanelState::load(ui.ctx(), egui::Id::new(ID_PANEL)) {
                let mut rect = state.outer_rect;
                rect.max.x = rect.min.x + DOCK_W;
                state.outer_rect = rect;
                ui.ctx()
                    .data_mut(|d| d.insert_persisted(egui::Id::new(ID_PANEL), state));
            }
            ui.close();
        }
        let sembunyi = menu_item(ui, theme, "Sembunyikan dock", Some("Tab"));
        if sembunyi.clicked() {
            outcome.enter_focus = true;
            ui.close();
        }
    });
}

/// Item menu §7.8: tinggi [`MENU_ITEM_H`], teks `label-md` di kiri, petunjuk
/// shortcut `numeric`/`text-disabled` di kanan.
fn menu_item(ui: &mut Ui, theme: &Theme, teks: &str, shortcut: Option<&str>) -> Response {
    let lebar = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(lebar, MENU_ITEM_H), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, theme::radius::SM, theme.accent_brand_subtle);
    }
    ui.painter().text(
        rect.left_center() + Vec2::new(theme::spacing::GAP_CONTROL, 0.0),
        Align2::LEFT_CENTER,
        teks,
        Type::LabelMd.font_id(),
        theme.text_primary,
    );
    if let Some(s) = shortcut {
        ui.painter().text(
            rect.right_center() - Vec2::new(theme::spacing::GAP_CONTROL, 0.0),
            Align2::RIGHT_CENTER,
            s,
            Type::Numeric.font_id(),
            theme.text_disabled,
        );
    }
    resp
}

/// Arah glyph chevron.
#[derive(Clone, Copy)]
enum Arah {
    Kanan,
    Atas,
    Bawah,
}

/// Chevron v dua ruas — digambar sendiri (galeri §5 tetap 25 ikon; glif
/// font tidak dipakai agar bebas cakupan huruf).
fn chevron(painter: &egui::Painter, rect: Rect, warna: Color32, arah: Arah) {
    let c = rect.center();
    let s = 4.0;
    let titik = match arah {
        Arah::Kanan => [
            Pos2::new(c.x - 2.0, c.y - s),
            Pos2::new(c.x + 3.0, c.y),
            Pos2::new(c.x - 2.0, c.y + s),
        ],
        Arah::Atas => [
            Pos2::new(c.x - s, c.y + 2.0),
            Pos2::new(c.x, c.y - 3.0),
            Pos2::new(c.x + s, c.y + 2.0),
        ],
        Arah::Bawah => [
            Pos2::new(c.x - s, c.y - 2.0),
            Pos2::new(c.x, c.y + 3.0),
            Pos2::new(c.x + s, c.y - 2.0),
        ],
    };
    painter.add(egui::epaint::Shape::line(
        titik.to_vec(),
        Stroke::new(1.5, warna),
    ));
}

/// Glyph⋮ — tiga titik vertikal (§7.6: menu ⋮ di header panel).
fn titik_tiga(painter: &egui::Painter, rect: Rect, warna: Color32) {
    let c = rect.center();
    for dy in [-4.0_f32, 0.0, 4.0] {
        painter.circle_filled(Pos2::new(c.x, c.y + dy), 1.5, warna);
    }
}

/// Tombol ikon kecil tanpa teks: glif digambar sendiri, hover `surface/3`,
/// focus ring §9, tooltip §7.9, dan accessible name §9 — mengikuti pola
/// `icon::icon_button`.
fn tombol_glyph(
    ui: &mut Ui,
    theme: &Theme,
    aktif: bool,
    gambar: impl FnOnce(&egui::Painter, Rect, Color32),
    nama: &str,
) -> Response {
    let sense = if aktif {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(GLYPH_PX), sense);
    if aktif && (resp.hovered() || resp.has_focus()) {
        ui.painter()
            .rect_filled(rect, theme::radius::SM, theme.surface_3);
    }
    let warna = if !aktif {
        theme.text_disabled
    } else if resp.hovered() {
        theme.text_primary
    } else {
        theme.text_secondary
    };
    gambar(ui.painter(), rect, warna);

    if aktif && resp.has_focus() {
        let grow = theme.focus_ring_offset + theme.focus_ring.width * 0.5;
        ui.painter().rect_stroke(
            rect.expand(grow),
            CornerRadius::same(theme::radius::SM),
            theme.focus_ring,
            egui::epaint::StrokeKind::Middle,
        );
    }
    let resp = resp.on_hover_text(nama);
    let _ = ui.ctx().accesskit_node_builder(resp.id, |node| {
        node.set_label(nama);
    });
    resp
}

/// Divider horizontal `border/default` selebar konten.
pub(crate) fn garis_pemisah(ui: &mut Ui, theme: &Theme) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, theme.border_default);
}

// ---------------------------------------------------------------------------
// Isi panel
// ---------------------------------------------------------------------------

/// Panel Brush — kontrol parameter kuas §7.2 (duplikat wajar dengan options
/// bar §7.5: panel = tempat telaten, options bar = cepat).
fn brush_panel(ui: &mut Ui, theme: &Theme, brush: &mut Brush) {
    ui.horizontal(|ui| {
        ui.add_space(theme::spacing::GAP_CONTROL);
        let mut size = brush.size;
        DragNumeric::new("Ukuran", &mut size, 1.0..=200.0, 1.0)
            .suffix(" px")
            .show(ui, theme);
        brush.size = size;
    });
    ui.horizontal(|ui| {
        ui.add_space(theme::spacing::GAP_CONTROL);
        let mut opacity = brush.opacity * 100.0;
        DragNumeric::new("Opasitas", &mut opacity, 1.0..=100.0, 1.0)
            .suffix("%")
            .show(ui, theme);
        brush.opacity = (opacity / 100.0).clamp(0.01, 1.0);
    });
    ui.horizontal(|ui| {
        ui.add_space(theme::spacing::GAP_CONTROL);
        let mut hardness = brush.hardness * 100.0;
        DragNumeric::new("Kekerasan", &mut hardness, 0.0..=100.0, 1.0)
            .suffix("%")
            .show(ui, theme);
        brush.hardness = (hardness / 100.0).clamp(0.0, 1.0);
    });
    ui.add_space(theme::spacing::GRID);
}

/// Satu baris daftar layer — dipisah agar peminjaman `&mut` tidak menumpuk.
struct BarisLayer<'a> {
    /// Indeks baris timeline (== indeks model; `[0]` = paling atas).
    index: usize,
    id: LayerId,
    nama: &'a str,
    terlihat: bool,
}

/// Panel Layers §7.6: daftar baris (visibilitas, nama, pilih) + aksi layer.
fn layers_panel(
    ui: &mut Ui,
    theme: &Theme,
    dock: &mut Dock,
    anim: &mut Animation,
    timeline: &mut Timeline,
    outcome: &mut DockOutcome,
) {
    // Aksi baris — tombol ikon §5 (layer-add/layer-delete) + reorder chevron.
    ui.horizontal(|ui| {
        ui.add_space(theme::spacing::GAP_CONTROL);
        if icon::icon_button(ui, theme, Icon::LayerAdd, "Tambah layer", false).clicked()
            && anim.layers().len() < MAX_LAYERS
        {
            aksi_tambah(anim, timeline, outcome);
        }
        if icon::icon_button(ui, theme, Icon::LayerDelete, "Hapus layer terpilih", false).clicked()
        {
            aksi_hapus(anim, timeline, outcome);
        }
        let bisa_naik = anim
            .layer_id(timeline.layer)
            .is_some_and(|id| anim.layer_index(id).is_some_and(|i| i > 0));
        let resp = tombol_glyph(
            ui,
            theme,
            bisa_naik,
            |painter, rect, warna| chevron(painter, rect, warna, Arah::Atas),
            "Naikkan layer",
        );
        if resp.clicked() {
            aksi_geser(anim, timeline, -1, outcome);
        }
        let bisa_turun = anim.layer_id(timeline.layer).is_some_and(|id| {
            let n = anim.layers().len();
            anim.layer_index(id).is_some_and(|i| i + 1 < n)
        });
        let resp = tombol_glyph(
            ui,
            theme,
            bisa_turun,
            |painter, rect, warna| chevron(painter, rect, warna, Arah::Bawah),
            "Turunkan layer",
        );
        if resp.clicked() {
            aksi_geser(anim, timeline, 1, outcome);
        }
    });

    // Opasitas layer terpilih — kontrol §7.2 sebelum daftar agar selalu
    // terlihat (daftar memakai sisa tinggi panel lewat ScrollArea).
    if let Some(id) = anim.layer_id(timeline.layer) {
        ui.horizontal(|ui| {
            ui.add_space(theme::spacing::GAP_CONTROL);
            let mut opasitas = anim
                .layer_mut(id)
                .map_or(0.0, |layer| layer.opacity * 100.0);
            DragNumeric::new("Opasitas", &mut opasitas, 0.0..=100.0, 1.0)
                .suffix("%")
                .show(ui, theme);
            if let Some(layer) = anim.layer_mut(id)
                && (layer.opacity * 100.0 - opasitas).abs() > 0.5
            {
                layer.opacity = (opasitas / 100.0).clamp(0.0, 1.0);
                outcome.content_changed = true;
            }
        });
    }

    // Salinan baris (≤16) — renovasi nama menulis balik via `layer_mut`
    // tanpa mempertahankan peminjaman koleksi.
    let rows: Vec<(usize, LayerId, String, bool)> = anim
        .layers()
        .iter()
        .enumerate()
        .map(|(index, layer)| (index, layer.id, layer.name.clone(), layer.visible))
        .collect();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, id, nama, terlihat) in &rows {
                row_layer(
                    ui,
                    theme,
                    dock,
                    anim,
                    timeline,
                    outcome,
                    BarisLayer {
                        index: *index,
                        id: *id,
                        nama: nama.as_str(),
                        terlihat: *terlihat,
                    },
                );
            }
        });
    ui.add_space(theme::spacing::GRID);
}

/// Baris daftar layer: checkbox visibilitas + nama (klik = pilih baris,
/// klik ganda = namai inline — Esc membatalkan §9).
#[allow(clippy::too_many_arguments)]
fn row_layer(
    ui: &mut Ui,
    theme: &Theme,
    dock: &mut Dock,
    anim: &mut Animation,
    timeline: &mut Timeline,
    outcome: &mut DockOutcome,
    baris: BarisLayer<'_>,
) {
    let BarisLayer {
        index,
        id,
        nama,
        terlihat,
    } = baris;
    let dipilih = timeline.layer == index;
    ui.horizontal(|ui| {
        ui.add_space(theme::spacing::GAP_CONTROL);

        // Visibilitas — checkbox egui (tick vektor bawaan); nama aksesibel
        // + tooltip §7.9.
        let mut tampil = terlihat;
        let resp = ui.checkbox(&mut tampil, "");
        if resp.changed() {
            if let Some(layer) = anim.layer_mut(id) {
                layer.visible = tampil;
            }
            outcome.content_changed = true;
        }
        let resp = resp.on_hover_text(format!("Visibilitas {nama}"));
        let _ = ui.ctx().accesskit_node_builder(resp.id, |node| {
            node.set_label(format!("Visibilitas {nama}"));
        });

        if dock.renaming.as_ref().is_some_and(|(rid, _)| *rid == id) {
            // Penamaan inline — Enter/klik luar komit, Esc batalkan (§9).
            let gantian = anim
                .layer_mut(id)
                .map(|layer| {
                    ui.add_sized(
                        Vec2::new(ui.available_width(), ROW_H),
                        egui::TextEdit::singleline(&mut layer.name).desired_width(f32::INFINITY),
                    )
                })
                .expect("layer yang dinamai ada");
            if !gantian.has_focus() {
                gantian.request_focus();
            }
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
            if esc {
                batalkan_renaming(dock, anim);
            } else if enter || gantian.lost_focus() {
                finish_renaming(dock, anim, outcome);
            }
        } else {
            let sisa = ui.available_width();
            let warna = if terlihat {
                theme.text_primary
            } else {
                theme.text_secondary
            };
            let mut diklik = false;
            let mut ganda = false;
            // Nama rata-kiri (konvensi daftar layer) — sisa lebar baris
            // dialokasikan dengan sense klik agar target seleksi penuh.
            ui.allocate_ui_with_layout(
                Vec2::new(sisa, ROW_H),
                Layout::left_to_right(Align::Center),
                |ui| {
                    let resp = ui.selectable_label(dipilih, Type::Body.rich(nama).color(warna));
                    diklik = resp.clicked();
                    ganda = resp.double_clicked();
                    let (_, sisa_resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), ROW_H),
                        Sense::click(),
                    );
                    diklik |= sisa_resp.clicked();
                },
            );
            if diklik {
                finish_renaming(dock, anim, outcome);
                timeline.layer = index;
            }
            if ganda {
                finish_renaming(dock, anim, outcome);
                dock.renaming = Some((id, nama.to_owned()));
                timeline.layer = index;
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Aksi layer (dipisah dari gambar agar bisa diuji langsung)
// ---------------------------------------------------------------------------

/// Tambah layer baru di paling atas; langsung jadi layer aktif.
fn aksi_tambah(anim: &mut Animation, timeline: &mut Timeline, outcome: &mut DockOutcome) {
    let nama = format!("Layer {}", anim.layers().len() + 1);
    if anim.add_layer(nama).is_ok() {
        timeline.layer = 0;
        outcome.content_changed = true;
    }
}

/// Hapus layer terpilih; bila terhapus, pilih tetangga terdekat.
fn aksi_hapus(anim: &mut Animation, timeline: &mut Timeline, outcome: &mut DockOutcome) {
    let Some(id) = anim.layer_id(timeline.layer) else {
        return;
    };
    if anim.remove_layer(id) {
        timeline.layer = timeline.layer.min(anim.layers().len().saturating_sub(1));
        outcome.content_changed = true;
    }
}

/// Geser layer terpilih `delta` baris (reorder §7.6) — layer aktif ikut
/// berpindah posisi.
fn aksi_geser(
    anim: &mut Animation,
    timeline: &mut Timeline,
    delta: i32,
    outcome: &mut DockOutcome,
) {
    let Some(id) = anim.layer_id(timeline.layer) else {
        return;
    };
    let ke = timeline.layer as i32 + delta;
    if ke < 0 || ke >= anim.layers().len() as i32 {
        return;
    }
    if anim.move_layer(id, ke as usize) {
        timeline.layer = ke as usize;
        outcome.content_changed = true;
    }
}

/// Esc (§9): penamaan dibatalkan — nama dikembalikan seperti semula.
fn batalkan_renaming(dock: &mut Dock, anim: &mut Animation) {
    if let Some((id, asal)) = dock.renaming.take()
        && let Some(layer) = anim.layer_mut(id)
    {
        layer.name = asal;
    }
}

/// Akhiri penamaan inline: nama kosong → kembalikan asli; selain itu komit
/// (konten berubah → autosave §13.2).
fn finish_renaming(dock: &mut Dock, anim: &mut Animation, outcome: &mut DockOutcome) {
    let Some((id, asal)) = dock.renaming.take() else {
        return;
    };
    let Some(layer) = anim.layer_mut(id) else {
        return;
    };
    if layer.name.trim().is_empty() {
        layer.name = asal;
    } else if layer.name != asal {
        outcome.content_changed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Folder temp unik — tes tak boleh menyentuh `~/.config`.
    fn temp_dir(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        std::env::temp_dir().join(format!(
            "flipbrush-dock-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn layout_roundtrip_simpan_lalu_muat() {
        let path = temp_dir("roundtrip").join("layout.json");
        let mut dock = Dock::load_from(&path);
        dock.width = 340.0;
        dock.tab = PanelTab::Layers;
        dock.collapsed = true;
        dock.dirty = true;
        dock.flush();

        let muat = Dock::load_from(&path);
        assert_eq!(muat.width, 340.0);
        assert_eq!(muat.tab, PanelTab::Layers);
        assert!(muat.collapsed, "collapse ikut dipersist (§8.2)");
    }

    #[test]
    fn berkas_hilang_rusak_atau_di_luar_rentang_pakai_default() {
        let path = temp_dir("rusak").join("layout.json");
        assert!(!path.exists(), "belum ada file");
        let dock = Dock::load_from(&path);
        assert_eq!(dock.width, DOCK_W);
        assert_eq!(dock.tab, PanelTab::Brush);
        assert!(!dock.collapsed);

        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"{ bukan json").unwrap();
        let dock = Dock::load_from(&path);
        assert_eq!(dock.width, DOCK_W, "rusak → default");

        std::fs::write(&path, br#"{"width": -999.0}"#).unwrap();
        let dock = Dock::load_from(&path);
        assert_eq!(dock.width, DOCK_MIN, "lebar korup di-clamp");
    }

    #[test]
    fn berkas_dengan_field_asing_tetap_terbaca() {
        let path = temp_dir("asing").join("layout.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            br#"{"width": 300.0, "tab": "layers", "field_masa_depan": true}"#,
        )
        .unwrap();
        let dock = Dock::load_from(&path);
        assert_eq!(dock.width, 300.0);
        assert_eq!(dock.tab, PanelTab::Layers, "field asing diabaikan");
    }

    #[test]
    fn aksi_layer_pertahankan_layer_aktif() {
        let mut anim = Animation::new(8, 8);
        let mut timeline = Timeline::default();
        let mut outcome = DockOutcome::default();

        aksi_tambah(&mut anim, &mut timeline, &mut outcome);
        assert_eq!(anim.layers().len(), 2);
        assert_eq!(timeline.layer, 0, "layer baru di paling atas jadi aktif");
        assert!(outcome.content_changed, "autosave §13.2");

        // Pilih layer lama (dasar) lalu geser ke atas.
        timeline.layer = 1;
        let mut outcome = DockOutcome::default();
        aksi_geser(&mut anim, &mut timeline, -1, &mut outcome);
        assert_eq!(timeline.layer, 0, "aktif mengikuti layer yang digeser");
        assert!(outcome.content_changed);

        // Hapus yang aktif → indeks di-clamp ke layer tersisa.
        let mut outcome = DockOutcome::default();
        aksi_hapus(&mut anim, &mut timeline, &mut outcome);
        assert_eq!(anim.layers().len(), 1);
        assert_eq!(timeline.layer, 0);
        assert!(outcome.content_changed);

        // Layer terakhir tak bisa dihapus (§13.3: ≥1 layer).
        let mut outcome = DockOutcome::default();
        aksi_hapus(&mut anim, &mut timeline, &mut outcome);
        assert!(!outcome.content_changed);
        assert_eq!(anim.layers().len(), 1);

        // Geser di batas atas/bawah tidak berbuat apa-apa.
        let mut outcome = DockOutcome::default();
        aksi_geser(&mut anim, &mut timeline, -1, &mut outcome);
        aksi_geser(&mut anim, &mut timeline, 1, &mut outcome);
        assert!(!outcome.content_changed);
    }

    #[test]
    fn penamaan_inline_komit_kosong_atau_batal() {
        let mut anim = Animation::new(8, 8);
        let mut dock = Dock::default();
        let mut outcome = DockOutcome::default();
        let id = anim.layer_id(0).unwrap();

        // Komit: nama berubah → konten berubah.
        dock.renaming = Some((id, "Layer 1".into()));
        anim.layer_mut(id).unwrap().name = "Kening".into();
        finish_renaming(&mut dock, &mut anim, &mut outcome);
        assert!(dock.renaming.is_none());
        assert_eq!(anim.layer_mut(id).unwrap().name, "Kening");
        assert!(outcome.content_changed);

        // Nama kosong → kembalikan asli, tanpa perubahan konten.
        dock.renaming = Some((id, "Kening".into()));
        anim.layer_mut(id).unwrap().name = "   ".into();
        let mut outcome = DockOutcome::default();
        finish_renaming(&mut dock, &mut anim, &mut outcome);
        assert_eq!(anim.layer_mut(id).unwrap().name, "Kening");
        assert!(!outcome.content_changed);

        // Esc (§9) → batalkan.
        dock.renaming = Some((id, "Kening".into()));
        anim.layer_mut(id).unwrap().name = "Sekali".into();
        batalkan_renaming(&mut dock, &mut anim);
        assert_eq!(anim.layer_mut(id).unwrap().name, "Kening");
        assert!(dock.renaming.is_none());
    }
}
