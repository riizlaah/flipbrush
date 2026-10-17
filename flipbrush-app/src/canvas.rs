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
    epaint::{ColorImage, StrokeKind},
};
use flipbrush_core::anim::{Animation, CelId, LayerId};
use flipbrush_core::raster::{
    Brush, DEFAULT_HISTORY, History, Raster, Rect as PixelRect, RegionBuf, Rgba, StrokeSampler,
};

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
        // dokumen tidak berubah ukuran, jadi tanpa refit.
        self.stroke = None;
        self.press = None;
        self.last_doc = None;
        self.was_down = false;
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

    // -------------------------------------------------------------------------
    // Render & interaksi
    // -------------------------------------------------------------------------

    /// Gambar kanvas + proses seluruh input pointer viewport/draw ke `ui`.
    /// `color` = warna kuas aktif; berubah saat eyedropper mengambil piksel.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        tool: Tool,
        brush: Brush,
        tolerance: u8,
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
            if drawable {
                let erase = tool == Tool::Eraser;
                // Salin "sebelum" dulu (bbox prediksi ⊇ area tempa), baru tempa.
                let mut before = RegionBuf::empty();
                before.expand(&self.raster, predict_bbox(pd, pd, brush));
                let mut sampler = StrokeSampler::default();
                let rect = sampler.begin((pd.x, pd.y), &mut self.raster, brush, *color, erase);
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
            );
            self.last_doc = Some(pd);
            new_dirty = new_dirty.union(rect);
        }

        // (5) Fill (§12.2): aktif saat klik (tekan–lepas diam — bukan
        // klik-panjang, yang berarti eyedropper).
        if response.clicked()
            && tool == Tool::Fill
            && self.press.as_ref().is_some_and(|p| !p.long)
            && let Some(pd) = pointer_doc
        {
            let gx = pd.x.floor() as i32;
            let gy = pd.y.floor() as i32;
            if let Some(result) = self.raster.flood_fill(gx, gy, *color, tolerance) {
                let after = self.raster.region(result.rect);
                if result.before.data != after {
                    self.history.push(result.rect, result.before.data, after);
                }
                new_dirty = new_dirty.union(result.rect);
            }
        }

        // (6) Rilis: commit stroke ke riwayat.
        if up_now {
            if let Some(stroke) = self.stroke.take() {
                commit_stroke(&mut self.history, &self.raster, stroke);
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
