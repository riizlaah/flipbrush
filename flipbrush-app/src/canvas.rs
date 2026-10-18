//! Kanvas gambar M2/M3 — DESIGN.md §12.2 (§1, §6, §7.2, §8.2, §2.3).
//!
//! - **Viewport**: zoom (`Ctrl+Scroll` / pinch — egui menggabung keduanya ke
//!   [`egui::InputState::zoom_delta`]) dan pan (Space-drag, dua jari).
//! - **Gambar instan** (§6, §13.3): rasterisasi ke bitmap CPU per-event
//!   pointer, unggahan tekstur **parsial** per area kotor — tanpa blocking
//!   I/O di UI thread.
//! - **Papan catur** netral (§1.3: tidak ada chromatic di sekeliling kanvas)
//!   dari token `surface-1`/`surface-2`, sel 12 px layar stabil terhadap zoom.
//! - **HUD** koordinat + zoom (`numeric`) di kiri bawah (§1.4 presisi).
//! - **Klik-panjang** = eyedropper (§12.2) di tool mana pun.
//! - **Seleksi** (M4d): marquee (drag kotak), lasso (drag poligon) & wand
//!   (klik area warna → mask flood-fill) → marching ants + highlight
//!   ringan (§2.1); seluruh operasi gambar ter-clip area seleksi; klik
//!   polos marquee/lasso = hapus seleksi; `Delete` hapus isi (undo-able);
//!   **Move** (M4d tahap 3): drag pindah isi seleksi — pratinjau outline
//!   live, commit piksel + geometri sekali jadi dengan satu entri undo.
//! - **Shape** (M4d tahap 3): drag bentuk terisi persegi/elips (toggle di
//!   options bar) — warna aktif, opacity brush, tepi anti-alias SDF per
//!   piksel; outline putus-putus selama drag.
//! - **Onion skin** (§2.3, M3): frame sebelumnya merah / berikutnya biru —
//!   aman buta warna (§9) — di atas overlay light table (putih @ 8%).
//!
//! Undo/redo berbasis area kotor (before/after per aksi, §12.2 `Ctrl+Z` /
//! `Ctrl+Shift+Z`). Buffer aktif menyatu dengan model animasi M3: raster +
//! riwayat cel aktif dipertukarkan lewat [`Canvas::switch_cel`].

use std::collections::HashMap;

use eframe::egui::{
    self, Color32, CornerRadius, CursorIcon, Key, PointerButton, Pos2, Rect, Sense, Stroke,
    TextureFilter, TextureHandle, TextureOptions, TextureWrapMode, Ui, Vec2,
    epaint::{ColorImage, Shape, StrokeKind},
    pos2,
};
use flipbrush_core::anim::{Animation, CelId, LayerId};
use flipbrush_core::raster::{
    Brush, DEFAULT_HISTORY, History, Raster, Rect as PixelRect, RegionBuf, Rgba, StrokeSampler,
};
use flipbrush_core::seleksi::Seleksi;

use crate::theme::{Theme, radius, spacing};
use crate::tool::Tool;
use crate::typography::Type;

/// Zoom minimum kanvas (×).
pub const MIN_ZOOM: f32 = 0.1;
/// Zoom maksimum kanvas (×).
pub const MAX_ZOOM: f32 = 16.0;
/// Lebar satu tile papan catur di layar (2 sel 12 px; tekstur 2×2 sel 8 px).
const CHECKER_TILE_PX: f32 = 24.0;
/// Sel papan catur pada tekstur (px).
const CHECKER_CELL: usize = 8;
/// Ukuran tekstur papan catur (px).
const CHECKER_TEX: usize = 16;
/// Jeda klik-panjang → eyedropper (detik, §12.2).
const LONG_PRESS_S: f64 = 0.5;
/// Toleransi pergerakan agar klik-panjang tetap sah (px layar).
const LONG_PRESS_SLACK_PX: f32 = 6.0;
/// Opasitas siluet onion skin di atas light table (§2.3 — konstanta non-hex).
const ONION_ALPHA: f32 = 0.5;
/// Panjang segmen garis putus-putus marching ants seleksi (M4d, px layar).
const DASH_PX: f32 = 4.0;

// ---------------------------------------------------------------------------
// Viewport
// ---------------------------------------------------------------------------

/// Transformasi layar ⇄ dokumen: `layar = offset + dokumen × zoom`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Layar tempat piksel dokumen (0,0).
    pub offset: Vec2,
    /// Skala (1.0 = 1 piksel dokumen per piksel layar).
    pub zoom: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            offset: Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

impl Viewport {
    /// Titik layar untuk koordinat dokumen.
    pub fn to_screen(&self, doc: Vec2) -> Vec2 {
        self.offset + doc * self.zoom
    }

    /// Koordinat dokumen untuk titik layar.
    pub fn to_doc(&self, screen: Vec2) -> Vec2 {
        (screen - self.offset) / self.zoom
    }

    /// Persegi layar untuk seluruh dokumen.
    pub fn doc_rect(&self, doc_size: Vec2) -> Rect {
        Rect::from_min_size(self.to_screen(Vec2::ZERO).to_pos2(), doc_size * self.zoom)
    }

    /// Zoom sebesar `factor` **di sekitar `cursor`** — titik di bawah kursor
    /// tetap di tempatnya.
    pub fn zoom_at(&mut self, cursor: Vec2, factor: f32) {
        let new_zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let applied = new_zoom / self.zoom;
        self.offset = cursor - (cursor - self.offset) * applied;
        self.zoom = new_zoom;
    }

    /// Geser tampilan (pan).
    pub fn pan_by(&mut self, delta: Vec2) {
        self.offset += delta;
    }

    /// Fit awal (§1 canvas-first): seluruh dokumen terlihat, zoom ≤ 1.
    pub fn fit(&mut self, area: Rect, doc_size: Vec2) {
        if !area.is_positive() || doc_size.x <= 0.0 || doc_size.y <= 0.0 {
            return;
        }
        let zoom = (area.width() / doc_size.x)
            .min(area.height() / doc_size.y)
            .min(1.0)
            .clamp(MIN_ZOOM, MAX_ZOOM);
        self.zoom = zoom;
        self.offset = (area.center() - doc_size * zoom * 0.5).to_vec2();
    }
}

// ---------------------------------------------------------------------------
// State kanvas
// ---------------------------------------------------------------------------

/// Stroke yang sedang berlangsung: salinan piksel sebelum stroke (undo).
struct StrokeInProgress {
    before: RegionBuf,
}

/// Status tekan-pointer untuk deteksi klik-panjang.
struct Press {
    time: f64,
    pos: Pos2,
    /// Eyedropper klik-panjang sudah aktif (stroke di-rollback).
    long: bool,
}

/// Kanvas M2/M3: buffer editing satu cel (proyek default §13.3: 1080²),
/// viewport, riwayat. Raster di sini adalah **buffer kerja** cel aktif —
/// model [`Animation`] memegang cel-cel lain; lihat [`Canvas::switch_cel`].
pub struct Canvas {
    /// Isi piksel buffer cel aktif (RGBA8 lurus — `flipbrush_core::raster`).
    pub raster: Raster,
    /// Riwayat undo/redo buffer aktif (berbasis area kotor).
    pub history: History,
    /// Transformasi layar ⇄ dokumen.
    pub viewport: Viewport,

    /// Cel yang sedang diedit (None = buffer kosong, belum punya exposure).
    active_cel: Option<CelId>,
    /// Riwayat per cel — dipindahkan bersama raster saat buffer berpindah.
    histories: HashMap<CelId, History>,
    /// Ada piksel berubah di frame UI ini (sinyal: expose cel baru bila
    /// posisi playhead belum punya exposure).
    modified: bool,

    texture: Option<TextureHandle>,
    checker: Option<TextureHandle>,
    /// Overlay onion skin (§2.3): prev bertint `onion/prev`, next bertint
    /// `onion/next`, di atas light table — digambar di bawah kanvas aktif.
    onion: Option<TextureHandle>,
    /// Kunci cache overlay: (frame, layer, on) — hanya dibangun ulang saat
    /// berubah (playhead/layer/toggle), bukan tiap frame UI.
    onion_key: Option<(u32, usize, bool)>,
    /// Penyaring tekstur yang sedang dipakai (krispi saat zoom ≥ 1).
    tex_nearest: bool,
    /// Area kotor menunggu unggahan parsial.
    dirty: Option<PixelRect>,

    /// Seleksi aktif (M4d) — geometri dokumen, lintas cel; clipping semua
    /// operasi gambar + marching ants overlay. Transien (tak di-persist).
    pub selection: Option<Seleksi>,
    /// Titik awal drag tool seleksi (koordinat dokumen) — `None` saat
    /// tidak sedang drag; rilis memindah `sel_preview` ke `selection`.
    sel_drag: Option<Vec2>,
    /// Jalur drag lasso (koordinat dokumen, urutan jalur — M4d tahap 2).
    sel_lasso: Vec<Vec2>,
    /// Preview seleksi selama drag marquee/lasso (dipindah ke `selection`
    /// saat rilis; klik polos menghasilkan `None` = hapus seleksi).
    sel_preview: Option<Seleksi>,
    /// Geometri seleksi saat tekan tool Move (M4d tahap 3) — dasar
    /// hitung geser pratinjau & commit piksel saat rilis.
    move_awal: Option<Seleksi>,
    /// Titik awal drag shape (koordinat dokumen — M4d tahap 3); `None`
    /// di luar drag.
    shape_start: Option<Vec2>,

    stroke: Option<StrokeInProgress>,
    sampler: StrokeSampler,
    last_doc: Option<Vec2>,
    press: Option<Press>,
    was_down: bool,
    fitted: bool,
}

impl Canvas {
    /// Kanvas kosong `width × height` (maks 4096², §13.3).
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            raster: Raster::new(width, height),
            history: History::new(DEFAULT_HISTORY),
            viewport: Viewport::default(),
            active_cel: None,
            histories: HashMap::new(),
            modified: false,
            texture: None,
            checker: None,
            onion: None,
            onion_key: None,
            tex_nearest: false,
            dirty: None,
            selection: None,
            sel_drag: None,
            sel_lasso: Vec::new(),
            sel_preview: None,
            move_awal: None,
            shape_start: None,
            stroke: None,
            sampler: StrokeSampler::default(),
            last_doc: None,
            press: None,
            was_down: false,
            fitted: false,
        }
    }

    /// Ukuran dokumen dalam piksel.
    pub fn doc_size(&self) -> Vec2 {
        Vec2::new(self.raster.width as f32, self.raster.height as f32)
    }

    /// Tandai area perlu digambar ulang ke tekstur; sekaligus menandai
    /// buffer berubah (untuk expose cel baru — M3).
    fn mark_dirty(&mut self, rect: PixelRect) {
        if !rect.is_empty() {
            self.dirty = Some(self.dirty.unwrap_or(PixelRect::EMPTY).union(rect));
            self.modified = true;
        }
    }

    /// Cel yang sedang diedit buffer ini (None = belum punya exposure).
    pub fn active_cel(&self) -> Option<CelId> {
        self.active_cel
    }

    /// Sinyal "piksel berubah frame ini" — di-reset setelah dibaca; dipakai
    /// app untuk expose cel baru di posisi playhead yang masih kosong.
    pub fn take_modified(&mut self) -> bool {
        std::mem::take(&mut self.modified)
    }

    /// Daftarkan buffer aktif sebagai cel baru (exposure di posisi playhead
    /// yang sebelumnya kosong). Raster TIDAK disalin — tetap di buffer.
    pub fn adopt_cel(&mut self, cel: CelId) {
        self.active_cel = Some(cel);
    }

    /// Pindahkan buffer editing ke cel `target` (None = buffer kosong).
    ///
    /// Cel lama dikembalikan ke model beserta raster + riwayatnya; cel baru
    /// diambil dari model (atau dibuat kosong bila tak tersimpan — cel baru
    /// diekspos tapi belum pernah disimpan). Tekstur diunggah ulang penuh
    /// (isi berbeda, dimensi sama).
    pub fn switch_cel(&mut self, anim: &mut Animation, target: Option<CelId>) {
        if self.active_cel == target {
            return;
        }
        // Kembalikan cel lama ke model (bila punya), beserta riwayatnya.
        if let Some(old) = self.active_cel.take() {
            let old_raster =
                std::mem::replace(&mut self.raster, Raster::new(anim.width, anim.height));
            anim.put_cel(old, old_raster);
            let old_history = std::mem::replace(&mut self.history, History::new(DEFAULT_HISTORY));
            self.histories.insert(old, old_history);
        } else {
            self.raster = Raster::new(anim.width, anim.height);
            self.history = History::new(DEFAULT_HISTORY);
        }
        // Muat cel target (raster bisa belum tersimpan — buffer kosong).
        if let Some(id) = target {
            self.raster = anim
                .take_cel(id)
                .unwrap_or_else(|| Raster::new(anim.width, anim.height));
            self.history = self
                .histories
                .remove(&id)
                .unwrap_or_else(|| History::new(DEFAULT_HISTORY));
            self.active_cel = Some(id);
        }
        // Reset state interaksi + unggah ulang penuh. Viewport tetap —
        // dokumen tidak berubah ukuran, jadi tanpa refit. Seleksi dokumen
        // (M4d) ikut bertahan — berlaku lintas cel.
        self.stroke = None;
        self.press = None;
        self.last_doc = None;
        self.was_down = false;
        self.sel_drag = None;
        self.sel_lasso.clear();
        self.sel_preview = None;
        self.move_awal = None;
        self.shape_start = None;
        self.dirty = Some(PixelRect {
            x: 0,
            y: 0,
            w: self.raster.width,
            h: self.raster.height,
        });
    }

    /// Sinkronkan overlay onion skin (§2.3) untuk playhead `frame` pada baris
    /// layer `layer`; `enabled` = toggle dari timeline. Cukup 6 argumen +
    /// `self` (batas clippy 7).
    ///
    /// Tekstur hanya dibangun ulang saat kunci (frame, layer, enabled)
    /// berubah; saat mati/tanpa tetangga → overlay dilepas (tanpa light table).
    /// Handle di-reuse lewat [`TextureHandle::set`] agar bebas churn alokasi.
    pub fn update_onion(
        &mut self,
        ctx: &egui::Context,
        theme: &Theme,
        anim: &Animation,
        frame: u32,
        layer: usize,
        enabled: bool,
    ) {
        let key = (frame, layer, enabled);
        if self.onion_key == Some(key) {
            return;
        }
        self.onion_key = Some(key);
        let image = enabled
            .then(|| {
                anim.layer_id(layer)
                    .and_then(|id| onion_image(theme, anim, id, frame))
            })
            .flatten();
        match (image, self.onion.as_mut()) {
            (Some(img), Some(handle)) => handle.set(img, tex_options(false)),
            (Some(img), None) => {
                self.onion = Some(ctx.load_texture("flipbrush-onion", img, tex_options(false)));
            }
            (None, _) => self.onion = None,
        }
    }

    /// `true` bila overlay onion (siluet + light table) sedang ditampilkan.
    pub fn has_onion(&self) -> bool {
        self.onion.is_some()
    }

    /// Undo satu langkah (§12.2 `Ctrl+Z`). `true` bila ada yang di-undo.
    pub fn undo(&mut self) -> bool {
        if let Some(rect) = self.history.undo(&mut self.raster) {
            self.mark_dirty(rect);
            true
        } else {
            false
        }
    }

    /// Redo satu langkah (§12.2 `Ctrl+Shift+Z`).
    pub fn redo(&mut self) -> bool {
        if let Some(rect) = self.history.redo(&mut self.raster) {
            self.mark_dirty(rect);
            true
        } else {
            false
        }
    }

    /// Hapus piksel di dalam seleksi — shortcut `Delete` (M4d). Seleksi
    /// tetap ada (biasanya dibiarkan agar bisa mengulang); `true` bila ada
    /// piksel berubah (entri undo terbentuk).
    pub fn hapus_terpilih(&mut self) -> bool {
        let Some(sel) = self.selection.as_ref() else {
            return false;
        };
        let Some(result) = self.raster.clear_in(sel) else {
            return false;
        };
        let after = self.raster.region(result.rect);
        if result.before.data != after {
            self.history.push(result.rect, result.before.data, after);
            self.mark_dirty(result.rect);
            true
        } else {
            false
        }
    }

    // -------------------------------------------------------------------------
    // Render & interaksi
    // -------------------------------------------------------------------------

    /// Gambar kanvas + proses seluruh input pointer viewport/draw ke `ui`.
    /// `color` = warna kuas aktif; berubah saat eyedropper mengambil piksel.
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        tool: Tool,
        brush: Brush,
        tolerance: u8,
        ellipse: bool,
        color: &mut Rgba,
    ) {
        let area = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(area, Sense::click_and_drag());
        let ctx = ui.ctx().clone();
        let doc_size = self.doc_size();

        // Fit sekali saat pertama tampil (§1: canvas-first, langsung gambar).
        if !self.fitted && area.is_positive() {
            self.viewport.fit(area, doc_size);
            self.fitted = true;
        }

        self.ensure_textures(&ctx, theme);

        // — Zoom & pan viewport —
        let space_held = !ctx.text_edit_focused() && ctx.input(|i| i.key_down(Key::Space));
        let hover = response.hover_pos();
        let (zoom_delta, scroll) = ctx.input(|i| (i.zoom_delta(), i.smooth_scroll_delta()));
        if response.hovered() {
            // §12.2: Ctrl+Scroll / pinch → zoom; egui sudah menggabung keduanya.
            if (zoom_delta - 1.0).abs() > 1e-4
                && let Some(p) = hover
            {
                self.viewport.zoom_at(p.to_vec2(), zoom_delta);
            }
            // Dua jari (trackpad scroll) → pan.
            if scroll != Vec2::ZERO {
                self.viewport.pan_by(scroll);
            }
        }
        if space_held && response.dragged_by(PointerButton::Primary) {
            // §12.2: pan = space-drag.
            self.viewport.pan_by(response.drag_delta());
        }

        // — Input gambar —
        let mut new_dirty = PixelRect::EMPTY;
        let down =
            response.is_pointer_button_down_on() || response.dragged_by(PointerButton::Primary);
        let down_now = down && !self.was_down;
        let up_now = !down && self.was_down;
        let pointer_screen = response.interact_pointer_pos().or(hover);
        let pointer_doc = pointer_screen.map(|p| self.viewport.to_doc(p.to_vec2()));
        let drawable = matches!(tool, Tool::Brush | Tool::Eraser) && !space_held;

        // (1) Awal tekan: mulai stroke (brush/eraser) / ambil warna (eyedropper).
        if down_now
            && !space_held
            && let Some(pd) = pointer_doc
        {
            self.press = Some(Press {
                time: ctx.input(|i| i.time),
                pos: pointer_screen.unwrap_or(Pos2::ZERO),
                long: false,
            });
            self.last_doc = Some(pd);
            if !tool.is_seleksi() {
                // Drag seleksi dimulai di blok (5) — reset sisa state lama.
                self.sel_drag = None;
                self.sel_lasso.clear();
                self.sel_preview = None;
                self.move_awal = None;
            }
            if tool != Tool::Shape {
                self.shape_start = None;
            }
            if drawable {
                let erase = tool == Tool::Eraser;
                // Salin "sebelum" dulu (bbox prediksi ⊇ area tempa), baru tempa.
                let mut before = RegionBuf::empty();
                before.expand(&self.raster, predict_bbox(pd, pd, brush));
                let mut sampler = StrokeSampler::default();
                let rect = sampler.begin(
                    (pd.x, pd.y),
                    &mut self.raster,
                    brush,
                    *color,
                    erase,
                    self.selection.as_ref(),
                );
                self.sampler = sampler;
                self.stroke = Some(StrokeInProgress { before });
                new_dirty = new_dirty.union(rect);
            } else if tool == Tool::Eyedropper {
                sample_color(&self.raster, pd, color);
            }
        }

        // (2) Klik-panjang → eyedropper universal (§12.2).
        let mut long_fresh = false;
        if let Some(press) = &mut self.press
            && !press.long
        {
            let held = ctx.input(|i| i.time) - press.time;
            let moved = pointer_screen.map_or(f32::INFINITY, |p| (p - press.pos).length());
            if held >= LONG_PRESS_S && moved <= LONG_PRESS_SLACK_PX {
                press.long = true;
                long_fresh = true;
            }
        }
        if long_fresh {
            // Batalkan stroke yang sudah terlanjur (rollback "sebelum").
            if let Some(stroke) = self.stroke.take()
                && !stroke.before.rect.is_empty()
            {
                stroke.before.restore(&mut self.raster);
                new_dirty = new_dirty.union(stroke.before.rect);
            }
            self.last_doc = None;
        }
        let long_active = self.press.as_ref().is_some_and(|p| p.long);

        // (3) Sampling kontinu: klik-panjang aktif / tool eyedropper ditekan.
        if (long_active || (tool == Tool::Eyedropper && down))
            && let Some(pd) = pointer_doc
        {
            sample_color(&self.raster, pd, color);
        }

        // (4) Lanjut stroke (bila bukan panning / klik-panjang).
        if down
            && !space_held
            && !long_active
            && drawable
            && self.stroke.is_some()
            && let (Some(last), Some(pd)) = (self.last_doc, pointer_doc)
            && last != pd
        {
            let erase = tool == Tool::Eraser;
            // Perluas "sebelum" dengan bbox prediksi segmen SEBELUM tempa —
            // piksel di luar rect lama masih pristine (invarian RegionBuf).
            if let Some(stroke) = &mut self.stroke {
                stroke
                    .before
                    .expand(&self.raster, predict_bbox(last, pd, brush));
            }
            let rect = self.sampler.segment(
                (last.x, last.y),
                (pd.x, pd.y),
                &mut self.raster,
                brush,
                *color,
                erase,
                self.selection.as_ref(),
            );
            self.last_doc = Some(pd);
            new_dirty = new_dirty.union(rect);
        }

        // (5) Tool seleksi drag (M4d): marquee drag kotak / lasso drag
        // poligon / move geser seleksi — akumulasi selama drag aktif
        // (termasuk frame rilis, saat pointer masih terbaca via hover);
        // klik polos menghasilkan `None` = hapus seleksi (marquee/lasso;
        // move klik polos = tanpa perubahan). Wand klik saja tanpa drag
        // (blok 6b). Preview digambar sebagai marching ants live.
        if matches!(tool, Tool::Marquee | Tool::Lasso | Tool::Move) && !space_held {
            if down_now && let Some(pd) = pointer_doc {
                self.sel_drag = Some(pd);
                self.sel_lasso = vec![pd];
                self.sel_preview = None;
                if tool == Tool::Move {
                    // Geometri asal — commit rilis (blok 7) memakainya.
                    self.move_awal = self.selection.clone();
                }
            }
            if let Some(start) = self.sel_drag
                && let Some(pd) = pointer_doc
            {
                match tool {
                    Tool::Marquee => {
                        self.sel_preview = Seleksi::dari_drag((start.x, start.y), (pd.x, pd.y));
                    }
                    Tool::Lasso => {
                        // Titik jalur baru bila cukup jauh (≥ 1 px dokumen)
                        // — hindari akumulasi titik rapat saat pointer diam.
                        let dekat = self
                            .sel_lasso
                            .last()
                            .is_some_and(|l| (l.x - pd.x).abs() < 1.0 && (l.y - pd.y).abs() < 1.0);
                        if !dekat {
                            self.sel_lasso.push(pd);
                        }
                        self.sel_preview = Seleksi::dari_lasso(
                            self.sel_lasso.iter().map(|p| [p.x, p.y]).collect(),
                        );
                    }
                    Tool::Move => {
                        // Pratinjau geometri bergeser — piksel baru
                        // berpindah saat rilis (blok 7), satu entri undo.
                        if let Some(awal) = &self.move_awal {
                            let dx = (pd.x - start.x).floor() as i32;
                            let dy = (pd.y - start.y).floor() as i32;
                            self.sel_preview = awal.clone().digeser(dx, dy);
                        }
                    }
                    _ => {} // Wand: resolusi saat klik (blok 6b).
                }
            }
        }

        // (5b) Shape (M4d tahap 3): drag membentuk pratinjau outline
        // (persegi/elips — digambar di blok gambar); rilis mewarnai bentuk
        // dengan warna aktif & opacity brush — satu entri undo. Klik-panjang
        // (eyedropper) dibatalkan; klik polos = tanpa efek (rect 0 px).
        if tool == Tool::Shape && !space_held && self.press.as_ref().is_some_and(|p| !p.long) {
            if down_now && let Some(pd) = pointer_doc {
                self.shape_start = Some(pd);
            }
            if up_now
                && let Some(start) = self.shape_start.take()
                && let Some(pd) = pointer_doc
            {
                let rect = PixelRect::from_points(
                    start.x.min(pd.x).floor() as i32,
                    start.y.min(pd.y).floor() as i32,
                    start.x.max(pd.x).ceil() as i32,
                    start.y.max(pd.y).ceil() as i32,
                );
                if rect.w > 0
                    && rect.h > 0
                    && let Some(result) =
                        self.raster
                            .bentuk(rect, ellipse, *color, brush.opacity, false)
                {
                    let after = self.raster.region(result.rect);
                    if result.before.data != after {
                        self.history.push(result.rect, result.before.data, after);
                    }
                    new_dirty = new_dirty.union(result.rect);
                }
            }
        }

        // (6) Fill (§12.2): aktif saat klik (tekan–lepas diam — bukan
        // klik-panjang, yang berarti eyedropper); ter-clip seleksi (M4d).
        if response.clicked()
            && tool == Tool::Fill
            && self.press.as_ref().is_some_and(|p| !p.long)
            && let Some(pd) = pointer_doc
        {
            let gx = pd.x.floor() as i32;
            let gy = pd.y.floor() as i32;
            if let Some(result) =
                self.raster
                    .flood_fill_masked(gx, gy, *color, tolerance, self.selection.as_ref())
            {
                let after = self.raster.region(result.rect);
                if result.before.data != after {
                    self.history.push(result.rect, result.before.data, after);
                }
                new_dirty = new_dirty.union(result.rect);
            }
        }

        // (6b) Wand (M4d tahap 2): klik area warna → resolusi flood-fill
        // (toleransi kontrol yang sama dengan Fill) menjadi mask seleksi.
        // Klik-panjang tetap eyedropper (guard `!p.long`).
        if response.clicked()
            && tool == Tool::Wand
            && self.press.as_ref().is_some_and(|p| !p.long)
            && let Some(pd) = pointer_doc
        {
            let gx = pd.x.floor() as i32;
            let gy = pd.y.floor() as i32;
            self.selection = Seleksi::dari_wand(&self.raster, gx, gy, tolerance);
        }

        // (7) Rilis: commit stroke, seleksi, atau pindahan (Move — M4d
        // tahap 3: piksel + geometri seleksi bergeser dalam satu entri undo).
        if up_now {
            if let Some(stroke) = self.stroke.take() {
                commit_stroke(&mut self.history, &self.raster, stroke);
            }
            if self.sel_drag.take().is_some() {
                if let Some(awal) = self.move_awal.take() {
                    // Tool Move: geser isi seleksi mengikuti pratinjau.
                    let offset = self.sel_preview.as_ref().map(|s| {
                        (
                            s.rect().x as i32 - awal.rect().x as i32,
                            s.rect().y as i32 - awal.rect().y as i32,
                        )
                    });
                    if let Some((dx, dy)) = offset
                        && (dx != 0 || dy != 0)
                    {
                        if let Some(result) = self.raster.geser_isi(&awal, dx, dy) {
                            let after = self.raster.region(result.rect);
                            if result.before.data != after {
                                self.history.push(result.rect, result.before.data, after);
                            }
                            new_dirty = new_dirty.union(result.rect);
                        }
                        self.selection = self.sel_preview.take();
                    } else {
                        // Tanpa geseran (klik polos / offset 0) — seleksi
                        // dikembalikan apa adanya.
                        self.selection = Some(awal);
                    }
                } else {
                    self.selection = self.sel_preview.take();
                }
            }
            self.last_doc = None;
            self.press = None;
        }

        self.was_down = down;
        self.mark_dirty(new_dirty);

        // — Kursor (§9: crosshair menggambar, grab saat pan) —
        if response.hovered() {
            let cursor = if space_held {
                if response.dragged() {
                    CursorIcon::Grabbing
                } else {
                    CursorIcon::Grab
                }
            } else {
                CursorIcon::Crosshair
            };
            ctx.set_cursor_icon(cursor);
        }

        // — Unggah tekstur (parsial, hanya area kotor) —
        self.upload(&ctx);

        // — Gambar —
        let painter = ui.painter();
        // §2.1: area abu netral di sekeliling kanvas.
        painter.rect_filled(area, 0.0, theme.canvas_surround);
        let doc_rect = self.viewport.doc_rect(doc_size);
        // Papan catur: UV menempel pada asal dokumen (ikut pan), 24 px tile.
        if let Some(checker) = &self.checker {
            let uv = Rect::from_min_size(
                Pos2::new(
                    doc_rect.min.x / CHECKER_TILE_PX,
                    doc_rect.min.y / CHECKER_TILE_PX,
                ),
                doc_rect.size() / CHECKER_TILE_PX,
            );
            painter.image(checker.id(), doc_rect, uv, Color32::WHITE);
        }
        // §2.3: light table putih 8% + siluet onion (prev merah / next biru)
        // — di bawah kanvas aktif, jadi area opak kanvas menutupinya.
        if let Some(onion) = &self.onion {
            painter.rect_filled(doc_rect, 0.0, theme.onion_skin_fill);
            painter.image(
                onion.id(),
                doc_rect,
                Rect::from_min_size(Pos2::ZERO, Vec2::ONE),
                Color32::WHITE,
            );
        }
        // Tepi dokumen (border/strong agar terlihat di atas canvas-surround).
        painter.rect_stroke(
            doc_rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, theme.border_strong),
            StrokeKind::Outside,
        );
        if let Some(texture) = &self.texture {
            painter.image(
                texture.id(),
                doc_rect,
                Rect::from_min_size(Pos2::ZERO, Vec2::ONE),
                Color32::WHITE,
            );
        }
        // Seleksi (M4d): marching ants di atas isi kanvas — preview live
        // selama drag marquee/lasso/move, seleksi tersimpan setelah rilis.
        self.gambar_seleksi(
            painter,
            theme,
            self.sel_preview.as_ref().or(self.selection.as_ref()),
            doc_rect,
        );
        // Pratinjau shape (M4d tahap 3): outline putus-putus persegi/elips
        // selama drag — bentuk baru diwarnai saat rilis.
        let bentuk_prev = if tool == Tool::Shape {
            self.shape_start.and_then(|start| {
                pointer_doc.map(|pd| {
                    PixelRect::from_points(
                        start.x.min(pd.x).floor() as i32,
                        start.y.min(pd.y).floor() as i32,
                        start.x.max(pd.x).ceil() as i32,
                        start.y.max(pd.y).ceil() as i32,
                    )
                })
            })
        } else {
            None
        };
        self.gambar_preview_bentuk(painter, theme, bentuk_prev, ellipse, doc_rect);
        self.hud(ui, theme, area, hover);
    }

    /// Papan catur + tekstur kanvas (sekali, lalu sesuai kebutuhan).
    fn ensure_textures(&mut self, ctx: &egui::Context, theme: &Theme) {
        if self.checker.is_none() {
            let image = checker_image(theme);
            self.checker = Some(ctx.load_texture(
                "flipbrush-checker",
                image,
                TextureOptions {
                    magnification: TextureFilter::Nearest,
                    minification: TextureFilter::Nearest,
                    wrap_mode: TextureWrapMode::Repeat,
                    ..Default::default()
                },
            ));
        }
        if self.texture.is_none() {
            let image = full_image(&self.raster);
            self.tex_nearest = self.viewport.zoom >= 1.0;
            self.texture =
                Some(ctx.load_texture("flipbrush-canvas", image, tex_options(self.tex_nearest)));
            self.dirty = None;
        }
    }

    /// Unggah area kotor ke GPU (parsial). Bila penyaring berganti saat
    /// zoom melintasi 1.0, unggah ulang penuh sekali.
    fn upload(&mut self, _ctx: &egui::Context) {
        let Some(texture) = &mut self.texture else {
            return;
        };
        let want_nearest = self.viewport.zoom >= 1.0;
        if want_nearest != self.tex_nearest {
            self.tex_nearest = want_nearest;
            let image = full_image(&self.raster);
            texture.set(image, tex_options(want_nearest));
            self.dirty = None;
            return;
        }
        if let Some(rect) = self.dirty.take() {
            let rect = rect.intersect(PixelRect {
                x: 0,
                y: 0,
                w: self.raster.width,
                h: self.raster.height,
            });
            if !rect.is_empty() {
                let image = ColorImage::from_rgba_unmultiplied(
                    [rect.w as usize, rect.h as usize],
                    &self.raster.region(rect),
                );
                texture.set_partial(
                    [rect.x as usize, rect.y as usize],
                    image,
                    tex_options(self.tex_nearest),
                );
            }
        }
    }

    /// HUD kiri bawah: zoom% + koordinat dokumen (`numeric`, §1.4 presisi).
    fn hud(&self, ui: &mut Ui, theme: &Theme, area: Rect, hover: Option<Pos2>) {
        let coords = hover
            .map(|p| {
                let d = self.viewport.to_doc(p.to_vec2());
                format!("{}, {} px", d.x.floor() as i64, d.y.floor() as i64)
            })
            .unwrap_or_else(|| "—".to_owned());
        let text = format!("{:.0}%  ·  {coords}", self.viewport.zoom * 100.0);
        let galley = ui
            .ctx()
            .fonts_mut(|f| f.layout(text, Type::Numeric.font_id(), theme.text_secondary, 256.0));
        let pad = spacing::GAP_CONTROL * 0.5;
        let chip = Rect::from_min_size(
            Pos2::new(
                area.left() + spacing::PANEL,
                area.bottom() - spacing::PANEL - galley.size().y - spacing::GAP_CONTROL,
            ),
            galley.size() + Vec2::splat(spacing::GAP_CONTROL),
        );
        let painter = ui.painter();
        painter.rect_filled(chip, radius::SM, theme.surface_1);
        painter.rect_stroke(
            chip,
            CornerRadius::same(radius::SM),
            Stroke::new(1.0, theme.border_default),
            StrokeKind::Middle,
        );
        painter.galley(chip.min + Vec2::splat(pad), galley, theme.text_secondary);
    }

    /// Overlay marching ants seleksi (M4d): highlight ringan
    /// `accent/brand-subtle` (§2.1: "highlight seleksi ringan") di bawah
    /// garis putus-putus `border/strong` di atas isi kanvas, dipotong ke
    /// rect dokumen. Marquee = isi rect penuh; lasso = isi poligon
    /// (scanline even-odd — cekung aman) + jalur tertutup; wand = isi
    /// span baris mask (bentuk asli region) + bbox putus-putus.
    fn gambar_seleksi(
        &self,
        painter: &egui::Painter,
        theme: &Theme,
        sel: Option<&Seleksi>,
        doc_rect: Rect,
    ) {
        let Some(sel) = sel else {
            return;
        };
        let r = sel.rect();
        let min = self
            .viewport
            .to_screen(Vec2::new(r.x as f32, r.y as f32))
            .to_pos2();
        let rect = Rect::from_min_size(min, Vec2::new(r.w as f32, r.h as f32) * self.viewport.zoom)
            .intersect(doc_rect);
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        // Highlight seleksi ringan (§2.1) — di bawah garis putus-putus.
        let isi = theme.accent_brand_subtle;
        match sel {
            Seleksi::Kotak(_) => {
                painter.rect_filled(rect, 0.0, isi);
                marching_ants(
                    painter,
                    theme,
                    &[
                        rect.left_top(),
                        rect.right_top(),
                        rect.right_bottom(),
                        rect.left_bottom(),
                        rect.left_top(),
                    ],
                );
            }
            Seleksi::Poligon(titik) => {
                let layar: Vec<Pos2> = titik
                    .iter()
                    .map(|p| self.viewport.to_screen(Vec2::new(p[0], p[1])).to_pos2())
                    .collect();
                if layar.len() >= 3 {
                    isi_poligon(painter, &layar, isi, doc_rect);
                    let mut tertutup = layar.clone();
                    tertutup.push(layar[0]);
                    marching_ants(painter, theme, &tertutup);
                }
            }
            Seleksi::Mask { .. } => {
                for span in spans_mask(sel, &self.viewport) {
                    painter.rect_filled(span.intersect(doc_rect), 0.0, isi);
                }
                marching_ants(
                    painter,
                    theme,
                    &[
                        rect.left_top(),
                        rect.right_top(),
                        rect.right_bottom(),
                        rect.left_bottom(),
                        rect.left_top(),
                    ],
                );
            }
        }
    }

    /// Pratinjau outline shape (M4d tahap 3): putus-putus `border/strong`
    /// — persegi (4 sisi) atau elips (contoh 48 segmen, jalur tertutup);
    /// tanpa isi — pewarnaan baru terjadi saat rilis drag (blok 5b).
    fn gambar_preview_bentuk(
        &self,
        painter: &egui::Painter,
        theme: &Theme,
        rect: Option<PixelRect>,
        ellipse: bool,
        doc_rect: Rect,
    ) {
        let Some(r) = rect.filter(|r| r.w > 0 && r.h > 0) else {
            return;
        };
        let min = self
            .viewport
            .to_screen(Vec2::new(r.x as f32, r.y as f32))
            .to_pos2();
        let rect = Rect::from_min_size(min, Vec2::new(r.w as f32, r.h as f32) * self.viewport.zoom)
            .intersect(doc_rect);
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        let titik: Vec<Pos2> = if ellipse {
            let mut pts: Vec<Pos2> = (0..48)
                .map(|i| {
                    let t = i as f32 / 48.0 * std::f32::consts::TAU;
                    pos2(
                        rect.center().x + rect.width() * 0.5 * t.cos(),
                        rect.center().y + rect.height() * 0.5 * t.sin(),
                    )
                })
                .collect();
            let pertama = pts[0];
            pts.push(pertama);
            pts
        } else {
            vec![
                rect.left_top(),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
                rect.left_top(),
            ]
        };
        marching_ants(painter, theme, &titik);
    }
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Opsi penyaring tekstur: krispi (nearest) saat zoom ≥ 1 agar piksel tajam,
/// halus (linear) saat zoom < 1 agar tidak berkedip saat mengecil.
fn tex_options(nearest: bool) -> TextureOptions {
    if nearest {
        TextureOptions {
            magnification: TextureFilter::Nearest,
            minification: TextureFilter::Nearest,
            ..Default::default()
        }
    } else {
        TextureOptions::LINEAR
    }
}

/// Gambar seluruh raster ke `ColorImage` (RGBA lurus → egui).
fn full_image(raster: &Raster) -> ColorImage {
    ColorImage::from_rgba_unmultiplied(
        [raster.width as usize, raster.height as usize],
        &raster.region(PixelRect {
            x: 0,
            y: 0,
            w: raster.width,
            h: raster.height,
        }),
    )
}

/// Garis putus-putus marching ants seleksi (`border/strong` — terbaca di
/// atas isi terang maupun gelap, kontras §9).
fn marching_ants(painter: &egui::Painter, theme: &Theme, titik: &[Pos2]) {
    painter.add(Shape::dashed_line(
        titik,
        Stroke::new(1.0, theme.border_strong),
        DASH_PX,
        DASH_PX,
    ));
}

/// Isi poligon layar (scanline even-odd per baris piksel) — highlight
/// seleksi lasso; path epaint hanya mengisi poligon cekung, jadi baris
/// disilangkan sendiri (deterministik, terpotong `batas`).
fn isi_poligon(painter: &egui::Painter, titik: &[Pos2], warna: Color32, batas: Rect) {
    let n = titik.len();
    if n < 3 {
        return;
    }
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for p in titik {
        min_y = min_y.min(p.y);
        max_y = max_y.max(p.y);
    }
    let y_awal = min_y.max(batas.top()).floor() as i32;
    let y_akhir = max_y.min(batas.bottom()).ceil() as i32;
    let mut silang: Vec<f32> = Vec::with_capacity(n);
    for y in y_awal..y_akhir {
        let yc = y as f32 + 0.5;
        silang.clear();
        for i in 0..n {
            let a = titik[i];
            let b = titik[(i + 1) % n];
            if (a.y > yc) != (b.y > yc) {
                let t = (yc - a.y) / (b.y - a.y);
                silang.push(a.x + t * (b.x - a.x));
            }
        }
        silang.sort_by(f32::total_cmp);
        let mut j = 0;
        while j + 1 < silang.len() {
            let x0 = silang[j].max(batas.left());
            let x1 = silang[j + 1].min(batas.right());
            if x1 > x0 {
                painter.rect_filled(
                    Rect::from_min_max(pos2(x0, y as f32), pos2(x1, y as f32 + 1.0)),
                    0.0,
                    warna,
                );
            }
            j += 2;
        }
    }
}

/// Span baris hasil run-length mask wand (koordinat layar) — highlight
/// mengikuti bentuk asli region; satu baris menyatu jadi satu rect.
fn spans_mask(sel: &Seleksi, viewport: &Viewport) -> Vec<Rect> {
    let Seleksi::Mask { rect, bits } = sel else {
        return Vec::new();
    };
    let mut hasil = Vec::new();
    for row in 0..rect.h {
        let dasar = (row * rect.w) as usize;
        let mut x = 0u32;
        while x < rect.w {
            if bits[dasar + x as usize] == 1 {
                let mulai = x;
                while x < rect.w && bits[dasar + x as usize] == 1 {
                    x += 1;
                }
                let min = viewport
                    .to_screen(Vec2::new((rect.x + mulai) as f32, (rect.y + row) as f32))
                    .to_pos2();
                let maks = viewport
                    .to_screen(Vec2::new((rect.x + x) as f32, (rect.y + row + 1) as f32))
                    .to_pos2();
                hasil.push(Rect::from_min_max(min, maks));
            } else {
                x += 1;
            }
        }
    }
    hasil
}

/// Papan catur 2×2 sel dari token netral (§1.3: tanpa chromatic; selalu
/// memakai `surface-1`/`surface-2` dari tema aktif).
fn checker_image(theme: &Theme) -> ColorImage {
    let mut pixels = vec![Color32::TRANSPARENT; CHECKER_TEX * CHECKER_TEX];
    for y in 0..CHECKER_TEX {
        for x in 0..CHECKER_TEX {
            let cell_a = (x / CHECKER_CELL + y / CHECKER_CELL).is_multiple_of(2);
            pixels[y * CHECKER_TEX + x] = if cell_a {
                theme.surface_1
            } else {
                theme.surface_2
            };
        }
    }
    ColorImage::new([CHECKER_TEX, CHECKER_TEX], pixels)
}

/// Citra overlay onion (§2.3): frame sebelumnya bertint `onion/prev`, frame
/// berikutnya bertint `onion/next` (dua-duanya aman buta warna §9), blend
/// src-over dengan opasitas [`ONION_ALPHA`] × alpha cel. `None` bila tak ada
/// tetangga playhead yang punya cel — overlay & light table tidak ditampilkan.
fn onion_image(theme: &Theme, anim: &Animation, layer: LayerId, frame: u32) -> Option<ColorImage> {
    let prev = frame
        .checked_sub(1)
        .and_then(|f| anim.cel_id_at(layer, f))
        .and_then(|id| anim.cel(id));
    let next = frame
        .checked_add(1)
        .and_then(|f| anim.cel_id_at(layer, f))
        .and_then(|id| anim.cel(id));
    if prev.is_none() && next.is_none() {
        return None;
    }
    let w = anim.width as usize;
    let h = anim.height as usize;
    let mut pixels = vec![Color32::TRANSPARENT; w * h];
    for (cel, tint) in [(prev, theme.onion_prev), (next, theme.onion_next)] {
        let Some(cel) = cel else {
            continue;
        };
        // Semua cel seukuran dokumen; potong bila tidak (region memotong ke
        // batas raster) — baris tetap rapat ke kiri `stride`.
        let rect = PixelRect {
            x: 0,
            y: 0,
            w: cel.width.min(anim.width),
            h: cel.height.min(anim.height),
        };
        let stride = rect.w as usize;
        let data = cel.region(rect);
        for (i, px) in data.as_chunks::<4>().0.iter().enumerate() {
            let alpha = (f32::from(px[3]) * ONION_ALPHA).round() as u8;
            if alpha == 0 {
                continue;
            }
            let on_top = Color32::from_rgba_unmultiplied(tint.r(), tint.g(), tint.b(), alpha);
            let idx = (i / stride) * w + i % stride;
            pixels[idx] = pixels[idx].blend(on_top);
        }
    }
    Some(ColorImage::new([w, h], pixels))
}

/// Bbox prediksi area yang akan tersentuh tempa dari `a` ke `b` (+ margin 2
/// px di luar radius) — dipakai untuk menyalin "sebelum" **sebelum** tempa.
fn predict_bbox(a: Vec2, b: Vec2, brush: Brush) -> PixelRect {
    let reach = (brush.size * 0.5 + 2.0).ceil();
    PixelRect::from_points(
        (a.x.min(b.x) - reach).floor() as i32,
        (a.y.min(b.y) - reach).floor() as i32,
        (a.x.max(b.x) + reach).ceil() as i32,
        (a.y.max(b.y) + reach).ceil() as i32,
    )
}

/// Ambil warna di titik dokumen (eyedropper §12.2).
fn sample_color(raster: &Raster, doc: Vec2, out: &mut Rgba) {
    if let Some(px) = raster.pixel(doc.x.floor() as i32, doc.y.floor() as i32) {
        *out = px;
    }
}

/// Commit stroke yang selesai ke riwayat (bila piksel benar-benar berubah).
fn commit_stroke(history: &mut History, raster: &Raster, stroke: StrokeInProgress) {
    let rect = stroke.before.rect;
    if rect.is_empty() {
        return;
    }
    let after = raster.region(rect);
    // Tanpa perubahan nyata (mis. penghapus di kanvas kosong) → tanpa entri.
    if stroke.before.data != after {
        history.push(rect, stroke.before.data, after);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_roundtrip_layar_dokumen() {
        let vp = Viewport {
            offset: Vec2::new(100.0, 50.0),
            zoom: 2.0,
        };
        let doc = Vec2::new(30.0, 40.0);
        let back = vp.to_doc(vp.to_screen(doc));
        assert!((back - doc).length() < 1e-4);
    }

    #[test]
    fn zoom_at_menjaga_titik_di_bawah_kursor() {
        let mut vp = Viewport::default();
        let cursor = Vec2::new(400.0, 300.0);
        let before = vp.to_doc(cursor);
        vp.zoom_at(cursor, 1.5);
        assert!((vp.zoom - 1.5).abs() < 1e-5);
        let after = vp.to_doc(cursor);
        assert!(
            (before - after).length() < 1e-3,
            "titik di bawah kursor harus diam"
        );
    }

    #[test]
    fn zoom_dilimit_min_maks() {
        let mut vp = Viewport::default();
        vp.zoom_at(Vec2::ZERO, 1000.0);
        assert_eq!(vp.zoom, MAX_ZOOM);
        vp.zoom_at(Vec2::ZERO, 0.0001);
        assert_eq!(vp.zoom, MIN_ZOOM);
    }

    #[test]
    fn fit_awal_kanvas_terlihat_penuh() {
        let mut vp = Viewport::default();
        let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0));
        vp.fit(area, Vec2::new(1080.0, 1080.0));
        let r = vp.doc_rect(Vec2::new(1080.0, 1080.0));
        assert!(r.width() <= area.width() + 1.0 && r.height() <= area.height() + 1.0);
        assert!(r.center().distance(area.center()) < 1.0, "terpusat");
        assert!(vp.zoom <= 1.0);
    }

    #[test]
    fn prediksi_bbox_menutupi_radius() {
        let brush = Brush {
            size: 20.0,
            ..Brush::default()
        };
        let r = predict_bbox(Vec2::new(50.0, 50.0), Vec2::new(80.0, 50.0), brush);
        // radius 10 + margin ≥ 12 px di kiri/kanan garis.
        assert!(r.x <= 38 && r.x + r.w >= 92);
        assert!(r.y <= 38 && r.y + r.h >= 62);
    }

    #[test]
    fn onion_image_prev_merah_next_biru_alpha_50() {
        let theme = Theme::dark();
        let mut anim = Animation::new(8, 8);
        let layer = anim.layer_id(0).unwrap();

        // Frame 0 (prev): tempa opak di (2, 4); frame 1 = playhead (tanpa
        // isi — hanya penanda posisi); frame 2 (next): di (6, 4).
        let cel0 = anim.expose(layer, 0).unwrap();
        let mut prev_raster = Raster::new(8, 8);
        prev_raster.stamp(
            (2.0, 4.0),
            Brush {
                size: 4.0,
                ..Brush::default()
            },
            Rgba::opaque(0, 0, 0),
            false,
        );
        anim.put_cel(cel0, prev_raster);
        anim.expose(layer, 1).unwrap();
        let cel2 = anim.expose(layer, 2).unwrap();
        let mut next_raster = Raster::new(8, 8);
        next_raster.stamp(
            (6.0, 4.0),
            Brush {
                size: 4.0,
                ..Brush::default()
            },
            Rgba::opaque(0, 0, 0),
            false,
        );
        anim.put_cel(cel2, next_raster);

        let img = onion_image(&theme, &anim, layer, 1).expect("ada tetangga → overlay");
        let prev_px = img.pixels[4 * 8 + 2];
        // Alpha = 255 × ONION_ALPHA (0.5) → 128 (bulat); tint onion/prev
        // merah #FF6B6B → premultiplied: r > g.
        assert_eq!(prev_px.a(), 128, "opasitas siluet 50% §2.3");
        assert!(prev_px.r() > prev_px.g(), "prev bertint merah");
        let next_px = img.pixels[4 * 8 + 6];
        assert_eq!(next_px.a(), 128, "opasitas sama untuk next");
        assert!(
            next_px.b() > next_px.r(),
            "next bertint biru (aman buta warna §9)"
        );

        // Frame tanpa tetangga → tanpa overlay (light table pun ikut absen).
        assert!(
            onion_image(&theme, &anim, layer, 5).is_none(),
            "tanpa cel tetangga → None"
        );
    }
}
