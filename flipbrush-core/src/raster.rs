//! Inti stroke M2 (DESIGN §12.2 — "stroke core"): bitmap RGBA8, kuas
//! lembut, penghapus, fill, dan riwayat undo/redo.
//!
//! Crate ini murni komputasi — tanpa UI (egui) dan tanpa I/O, sehingga
//! stroke selalu instan di UI thread (§6, §13.3: latensi ≤ 1 frame).
//!
//! Konvensi: koordinat dalam piksel dokumen; titik float ditinjau pada
//! pusat piksel (`x + 0.5`). Warna disimpan RGBA8 **lurus** (bukan
//! premultiplied) agar siap diekspor sebagai PNG pada M5 (§13.1).

use std::collections::VecDeque;

use crate::seleksi::Seleksi;

/// Batas ukuran kanvas v1 (DESIGN §13.3): di atas ini proyek ditolak
/// dengan pesan yang jelas.
pub const MAX_DIMENSION: u32 = 4096;

/// Jarak antar-tempa sebagai fraksi ukuran kuas (10% — lembut tapi hemat).
const SPACING_FRACTION: f32 = 0.1;
/// Jarak tempa minimum dalam piksel.
const MIN_SPACING: f32 = 0.4;
/// Feather tepi (px) agar garis tidak bergerigi.
const FEATHER_PX: f32 = 1.0;
/// Alpha di bawah nilai ini dianggap transparan (hemat & anti "kotor").
const ALPHA_EPSILON: f32 = 0.002;

/// Area piksel integer (titik-atas-kiri, lebar-tinggi) di dalam [`Raster`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    /// Rect tanpa area.
    pub const EMPTY: Self = Self {
        x: 0,
        y: 0,
        w: 0,
        h: 0,
    };

    /// Rect dari dua titik mana pun (koordinat negatif dipotong ke 0 —
    /// potongan ke batas kanvas dilakukan pemanggil via `intersect`).
    pub fn from_points(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        let (x0, x1) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        let (y0, y1) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
        let x0 = x0.max(0);
        let y0 = y0.max(0);
        let x1 = x1.max(0);
        let y1 = y1.max(0);
        Self {
            x: x0 as u32,
            y: y0 as u32,
            w: (x1 - x0) as u32,
            h: (y1 - y0) as u32,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    /// Gabung dua rect (akumulator area kotor).
    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = (self.x + self.w).max(other.x + other.w);
        let y1 = (self.y + self.h).max(other.y + other.h);
        Self {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
    }

    /// Perpotongan dua rect (kosong bila tak bertemu).
    pub fn intersect(self, other: Self) -> Self {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = (self.x + self.w).min(other.x + other.w);
        let y1 = (self.y + self.h).min(other.y + other.h);
        if x1 <= x0 || y1 <= y0 {
            Self::EMPTY
        } else {
            Self {
                x: x0,
                y: y0,
                w: x1 - x0,
                h: y1 - y0,
            }
        }
    }
}

/// Warna RGBA8 lurus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    /// Warna opak.
    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

/// Parameter kuas/penghapus (§7.5 — ukuran dalam piksel dokumen).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brush {
    /// Diameter tanda dalam piksel dokumen.
    pub size: f32,
    /// Opasitas 0..1 (alpha per-tempa).
    pub opacity: f32,
    /// Kekerasan 0..1 (1 = tepi tajam; 0 = pudar merata sampai tepi).
    pub hardness: f32,
}

impl Default for Brush {
    fn default() -> Self {
        Self {
            size: 12.0,
            opacity: 1.0,
            hardness: 0.8,
        }
    }
}

/// Bitmap RGBA8 lurus — isi satu cel (M3 akan memakai ini per frame/layer).
#[derive(Clone, PartialEq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pixels: Vec<u8>,
}

impl Raster {
    /// Bitmap transparan `width × height`; menolak ukuran di luar v1 (§13.3).
    pub fn new(width: u32, height: u32) -> Self {
        assert!(width >= 1 && height >= 1, "ukuran kanvas minimal 1×1 px");
        assert!(
            width <= MAX_DIMENSION && height <= MAX_DIMENSION,
            "kanvas maksimal {MAX_DIMENSION}×{MAX_DIMENSION} px (DESIGN §13.3), dapat {width}×{height}"
        );
        Self {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
        }
    }

    fn bounds(&self) -> Rect {
        Rect {
            x: 0,
            y: 0,
            w: self.width,
            h: self.height,
        }
    }

    /// Ambil piksel (tanpa cek batas — hanya untuk panggilan internal yang
    /// sudah dijaga).
    fn get(&self, x: u32, y: u32) -> Rgba {
        let i = ((y * self.width + x) * 4) as usize;
        Rgba {
            r: self.pixels[i],
            g: self.pixels[i + 1],
            b: self.pixels[i + 2],
            a: self.pixels[i + 3],
        }
    }

    /// Ambil piksel; `None` bila di luar kanvas.
    pub fn pixel(&self, x: i32, y: i32) -> Option<Rgba> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        Some(self.get(x as u32, y as u32))
    }

    /// Tempel satu bulatan lembut di `center` dengan `brush` (`erase` =
    /// mode penghapus: alpha keluar, RGB tak berubah). Mengembalikan area
    /// yang tersentuh (sudah dipotong ke kanvas).
    pub fn stamp(&mut self, center: (f32, f32), brush: Brush, color: Rgba, erase: bool) -> Rect {
        self.stamp_masked(center, brush, color, erase, None)
    }

    /// Seperti [`Self::stamp`] dengan mask seleksi (M4d): piksel di luar
    /// area seleksi tidak disentuh — stroke ter-clip pada marching ants.
    pub fn stamp_masked(
        &mut self,
        center: (f32, f32),
        brush: Brush,
        color: Rgba,
        erase: bool,
        mask: Option<&Seleksi>,
    ) -> Rect {
        let opacity = brush.opacity.clamp(0.0, 1.0);
        if opacity <= 0.0 {
            return Rect::EMPTY;
        }
        let radius = (brush.size * 0.5).max(0.5);
        let hardness = brush.hardness.clamp(0.0, 1.0);
        let core = radius * hardness;

        let reach = radius + FEATHER_PX;
        let rect = Rect::from_points(
            (center.0 - reach).floor() as i32,
            (center.1 - reach).floor() as i32,
            (center.0 + reach).ceil() as i32,
            (center.1 + reach).ceil() as i32,
        )
        .intersect(self.bounds());
        if rect.is_empty() {
            return Rect::EMPTY;
        }

        for y in rect.y..rect.y + rect.h {
            for x in rect.x..rect.x + rect.w {
                if let Some(mask) = mask
                    && !mask.contains(x as i32, y as i32)
                {
                    continue;
                }
                let dx = x as f32 + 0.5 - center.0;
                let dy = y as f32 + 0.5 - center.1;
                let dist = (dx * dx + dy * dy).sqrt();

                // Feather tepi 1 px (AA): pudar linear di luar radius.
                let aa = ((radius + FEATHER_PX - dist) / FEATHER_PX).clamp(0.0, 1.0);
                if aa <= 0.0 {
                    continue;
                }
                // Inti padat + pudar smoothstep dari `core` sampai tepi
                // luar (radius + feather).
                let hard = if dist <= core {
                    1.0
                } else {
                    let outer = radius + FEATHER_PX;
                    let t = ((outer - dist) / (outer - core).max(1e-6)).clamp(0.0, 1.0);
                    t * t * (3.0 - 2.0 * t)
                };
                let alpha = opacity * hard * aa;
                if alpha <= ALPHA_EPSILON {
                    continue;
                }

                let i = ((y * self.width + x) * 4) as usize;
                if erase {
                    let da = f32::from(self.pixels[i + 3]) / 255.0;
                    self.pixels[i + 3] = (da * (1.0 - alpha) * 255.0).round() as u8;
                } else {
                    let sa = alpha * (f32::from(color.a) / 255.0);
                    let da = f32::from(self.pixels[i + 3]) / 255.0;
                    let out_a = sa + da * (1.0 - sa);
                    if out_a <= ALPHA_EPSILON {
                        self.pixels[i] = color.r;
                        self.pixels[i + 1] = color.g;
                        self.pixels[i + 2] = color.b;
                        self.pixels[i + 3] = 0;
                        continue;
                    }
                    let blend = |s: u8, d: u8| -> u8 {
                        let v = (f32::from(s) * sa + f32::from(d) * da * (1.0 - sa)) / out_a;
                        v.round().clamp(0.0, 255.0) as u8
                    };
                    self.pixels[i] = blend(color.r, self.pixels[i]);
                    self.pixels[i + 1] = blend(color.g, self.pixels[i + 1]);
                    self.pixels[i + 2] = blend(color.b, self.pixels[i + 2]);
                    self.pixels[i + 3] = (out_a * 255.0).round() as u8;
                }
            }
        }
        rect
    }

    /// Isi area kotor dari piksel yang ada (untuk unggahan tekstur parsial).
    pub fn region(&self, rect: Rect) -> Vec<u8> {
        let rect = rect.intersect(self.bounds());
        let mut out = Vec::with_capacity((rect.w * rect.h * 4) as usize);
        for y in rect.y..rect.y + rect.h {
            let row = ((y * self.width + rect.x) * 4) as usize;
            let len = (rect.w * 4) as usize;
            out.extend_from_slice(&self.pixels[row..row + len]);
        }
        out
    }

    /// Tulis balik area dari [`Self::region`]. Panjang `data` harus persis.
    pub fn write_region(&mut self, rect: Rect, data: &[u8]) {
        if rect.is_empty() {
            return;
        }
        let rect = rect.intersect(self.bounds());
        assert_eq!(
            data.len(),
            (rect.w * rect.h * 4) as usize,
            "ukuran data region tidak cocok"
        );
        for y in rect.y..rect.y + rect.h {
            let row = ((y * self.width + rect.x) * 4) as usize;
            let len = (rect.w * 4) as usize;
            self.pixels[row..row + len]
                .copy_from_slice(&data[((y - rect.y) * rect.w * 4) as usize..][..len]);
        }
    }

    /// Flood fill 4-connected dari `(x, y)` dengan `color` (§7 — tool
    /// fill). `tolerance` = selisih kanal maksimum 0..255. Mengembalikan
    /// area terisi + salinan pikel *sebelum* fill untuk undo.
    pub fn flood_fill(&mut self, x: i32, y: i32, color: Rgba, tolerance: u8) -> Option<FillResult> {
        self.flood_fill_masked(x, y, color, tolerance, None)
    }

    /// Seperti [`Self::flood_fill`] dengan mask seleksi (M4d): isian hanya
    /// mengalir di dalam area seleksi; benih di luar seleksi = tanpa isi.
    pub fn flood_fill_masked(
        &mut self,
        x: i32,
        y: i32,
        color: Rgba,
        tolerance: u8,
        mask: Option<&Seleksi>,
    ) -> Option<FillResult> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        if mask.is_some_and(|m| !m.contains(x, y)) {
            return None; // benih di luar seleksi — tak ada yang bisa diisi
        }
        let seed = self.get(x as u32, y as u32);
        if seed == color {
            return None; // tak ada perubahan
        }

        let matches = |p: Rgba| {
            p != color
                && p.r.abs_diff(seed.r) <= tolerance
                && p.g.abs_diff(seed.g) <= tolerance
                && p.b.abs_diff(seed.b) <= tolerance
                && p.a.abs_diff(seed.a) <= tolerance
        };
        // Batas mask seleksi — isi hanya mengalir di dalam area (M4d).
        let di_seleksi = |x: i32, y: i32| mask.is_none_or(|m| m.contains(x, y));

        let mut rect = Rect::EMPTY;
        let mut before = RegionBuf::empty();
        let mut stack = vec![(x, y)];
        while let Some((sx, sy)) = stack.pop() {
            // Rentang horizontal berisi di baris `sy`.
            let mut x0 = sx;
            while x0 > 0 && di_seleksi(x0 - 1, sy) && matches(self.get((x0 - 1) as u32, sy as u32))
            {
                x0 -= 1;
            }
            let mut x1 = sx;
            while x1 + 1 < self.width as i32
                && di_seleksi(x1 + 1, sy)
                && matches(self.get((x1 + 1) as u32, sy as u32))
            {
                x1 += 1;
            }
            let span = Rect {
                x: x0 as u32,
                y: sy as u32,
                w: (x1 - x0 + 1) as u32,
                h: 1,
            };
            before.expand(self, span);
            for x in x0..=x1 {
                let i = ((sy as u32 * self.width + x as u32) * 4) as usize;
                self.pixels[i] = color.r;
                self.pixels[i + 1] = color.g;
                self.pixels[i + 2] = color.b;
                self.pixels[i + 3] = color.a;
            }
            rect = rect.union(span);

            // Rentang tetangga di baris atas & bawah.
            for ny in [sy - 1, sy + 1] {
                if ny < 0 || ny >= self.height as i32 {
                    continue;
                }
                let mut xi = x0;
                while xi <= x1 {
                    if di_seleksi(xi, ny) && matches(self.get(xi as u32, ny as u32)) {
                        stack.push((xi, ny));
                        while xi <= x1
                            && di_seleksi(xi, ny)
                            && matches(self.get(xi as u32, ny as u32))
                        {
                            xi += 1;
                        }
                    } else {
                        xi += 1;
                    }
                }
            }
        }
        Some(FillResult { rect, before })
    }

    /// Hapus piksel di dalam area seleksi — shortcut `Delete` (M4d).
    /// Mengembalikan area + piksel sebelumnya untuk undo; `None` bila area
    /// seleksi di luar kanvas.
    pub fn clear_in(&mut self, sel: &Seleksi) -> Option<FillResult> {
        let rect = sel.rect().intersect(self.bounds());
        if rect.is_empty() {
            return None;
        }
        let before = RegionBuf::capture(self, rect);
        for y in rect.y..rect.y + rect.h {
            for x in rect.x..rect.x + rect.w {
                if !sel.contains(x as i32, y as i32) {
                    continue;
                }
                let i = ((y * self.width + x) * 4) as usize;
                self.pixels[i..i + 4].fill(0);
            }
        }
        Some(FillResult { rect, before })
    }

    /// Pindahkan piksel di dalam area seleksi ke offset `(dx, dy)` — tool
    /// Move (M4d tahap 3). Area kotor = posisi lama ∪ posisi baru (untuk
    /// undo utuh); piksel yang terdorong keluar kanvas dibuang.
    pub fn geser_isi(&mut self, sel: &Seleksi, dx: i32, dy: i32) -> Option<FillResult> {
        let lama = sel.rect().intersect(self.bounds());
        if lama.is_empty() || (dx == 0 && dy == 0) {
            return None;
        }
        let baru = Rect::from_points(
            lama.x as i32 + dx,
            lama.y as i32 + dy,
            (lama.x + lama.w) as i32 + dx,
            (lama.y + lama.h) as i32 + dy,
        )
        .intersect(self.bounds());
        let luas = lama.union(baru);
        if luas.is_empty() {
            return None;
        }
        let before = RegionBuf::capture(self, luas);

        // Salin isi terpilih dulu — baru tempatkan di posisi baru.
        let mut salinan: Vec<(i32, i32, [u8; 4])> = Vec::new();
        for y in lama.y..lama.y + lama.h {
            for x in lama.x..lama.x + lama.w {
                if !sel.contains(x as i32, y as i32) {
                    continue;
                }
                let i = ((y * self.width + x) * 4) as usize;
                salinan.push((
                    x as i32,
                    y as i32,
                    [
                        self.pixels[i],
                        self.pixels[i + 1],
                        self.pixels[i + 2],
                        self.pixels[i + 3],
                    ],
                ));
                self.pixels[i..i + 4].fill(0);
            }
        }
        for (x, y, px) in salinan {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= self.width as i32 || ny >= self.height as i32 {
                continue;
            }
            let i = ((ny as u32 * self.width + nx as u32) * 4) as usize;
            self.pixels[i..i + 4].copy_from_slice(&px);
        }
        Some(FillResult { rect: luas, before })
    }

    /// Rasterisasi bentuk terisi dari `rect` dokumen — tool Shape (M4d
    /// tahap 3). `ellipse` memilih elips; tepi dihaluskan coverage per
    /// piksel (SDF — tepi tak bergerigi, §6); `erase` = mode penghapus
    /// (alpha keluar, RGB tak berubah) seperti [`Self::stamp`].
    pub fn bentuk(
        &mut self,
        rect: Rect,
        ellipse: bool,
        color: Rgba,
        opacity: f32,
        erase: bool,
    ) -> Option<FillResult> {
        let opacity = opacity.clamp(0.0, 1.0);
        if opacity <= 0.0 {
            return None;
        }
        let area = rect.intersect(self.bounds());
        if area.is_empty() {
            return None;
        }
        let before = RegionBuf::capture(self, area);

        // Pusat & setengah-ukuran dalam koordinat piksel.
        let cx = rect.x as f32 + rect.w as f32 * 0.5;
        let cy = rect.y as f32 + rect.h as f32 * 0.5;
        let hw = rect.w as f32 * 0.5;
        let hh = rect.h as f32 * 0.5;
        let skala = hw.min(hh).max(0.5);

        for y in area.y..area.y + area.h {
            for x in area.x..area.x + area.w {
                let px = x as f32 + 0.5 - cx;
                let py = y as f32 + 0.5 - cy;
                // Jarak bertanda (negatif di dalam) — AA = clamp(0.5 − d).
                let sd = if ellipse {
                    let nx = px / hw.max(0.5);
                    let ny = py / hh.max(0.5);
                    ((nx * nx + ny * ny).sqrt() - 1.0) * skala
                } else {
                    let dx = px.abs() - hw;
                    let dy = py.abs() - hh;
                    let luar = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
                    luar + dx.max(dy).min(0.0)
                };
                let coverage = (0.5 - sd).clamp(0.0, 1.0);
                let alpha = coverage * opacity;
                if alpha <= ALPHA_EPSILON {
                    continue;
                }
                let i = ((y * self.width + x) * 4) as usize;
                if erase {
                    let da = f32::from(self.pixels[i + 3]) / 255.0;
                    self.pixels[i + 3] = (da * (1.0 - alpha) * 255.0).round() as u8;
                } else {
                    // Src-over lurus — identik dengan [`Self::stamp`].
                    let sa = alpha * (f32::from(color.a) / 255.0);
                    let da = f32::from(self.pixels[i + 3]) / 255.0;
                    let out_a = sa + da * (1.0 - sa);
                    if out_a <= ALPHA_EPSILON {
                        self.pixels[i] = color.r;
                        self.pixels[i + 1] = color.g;
                        self.pixels[i + 2] = color.b;
                        self.pixels[i + 3] = 0;
                        continue;
                    }
                    let blend = |s: u8, d: u8| -> u8 {
                        let v = (f32::from(s) * sa + f32::from(d) * da * (1.0 - sa)) / out_a;
                        v.round().clamp(0.0, 255.0) as u8
                    };
                    self.pixels[i] = blend(color.r, self.pixels[i]);
                    self.pixels[i + 1] = blend(color.g, self.pixels[i + 1]);
                    self.pixels[i + 2] = blend(color.b, self.pixels[i + 2]);
                    self.pixels[i + 3] = (out_a * 255.0).round() as u8;
                }
            }
        }
        Some(FillResult { rect: area, before })
    }
}

/// Hasil flood fill: area terisi + piksel sebelumnya (untuk undo).
pub struct FillResult {
    pub rect: Rect,
    pub before: RegionBuf,
}

/// Penjaga jarak tempa antar-segmen: stempel pertama segmen berikutnya
/// dilanjutkan dari sisa jarak segmen sebelumnya sehingga titik sambung
/// tidak dobel-tempa (penting bila opasitas < 1).
#[derive(Default)]
pub struct StrokeSampler {
    carry: f32,
}

impl StrokeSampler {
    /// Tempel satu titik awal (awal tekan). `mask` = clipping seleksi
    /// (M4d) — diteruskan ke [`Raster::stamp_masked`].
    pub fn begin(
        &mut self,
        point: (f32, f32),
        raster: &mut Raster,
        brush: Brush,
        color: Rgba,
        erase: bool,
        mask: Option<&Seleksi>,
    ) -> Rect {
        self.carry = 0.0;
        raster.stamp_masked(point, brush, color, erase, mask)
    }

    /// Lanjutkan stroke dari `from` ke `to`. Kosong bila `from == to`.
    #[allow(clippy::too_many_arguments)]
    pub fn segment(
        &mut self,
        from: (f32, f32),
        to: (f32, f32),
        raster: &mut Raster,
        brush: Brush,
        color: Rgba,
        erase: bool,
        mask: Option<&Seleksi>,
    ) -> Rect {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let dist = (dx * dx + dy * dy).sqrt();
        if dist <= 0.0 {
            return Rect::EMPTY;
        }
        let spacing = (brush.size.max(1.0) * SPACING_FRACTION).max(MIN_SPACING);

        let mut dirty = Rect::EMPTY;
        let mut s = spacing - self.carry;
        while s <= dist {
            let t = s / dist;
            let point = (from.0 + dx * t, from.1 + dy * t);
            dirty = dirty.union(raster.stamp_masked(point, brush, color, erase, mask));
            s += spacing;
        }
        self.carry = (self.carry + dist) % spacing;
        dirty
    }
}

/// Salinan piksel persegi untuk undo/redo (before/after suatu aksi).
#[derive(Debug)]
pub struct RegionBuf {
    pub rect: Rect,
    pub data: Vec<u8>,
}

impl RegionBuf {
    pub fn empty() -> Self {
        Self {
            rect: Rect::EMPTY,
            data: Vec::new(),
        }
    }

    /// Salin area `rect` dari `raster` (dipotong ke kanvas).
    pub fn capture(raster: &Raster, rect: Rect) -> Self {
        let rect = rect.intersect(raster.bounds());
        Self {
            data: raster.region(rect),
            rect,
        }
    }

    /// Perluas cakupan agar mencakup `add`. Piksel luar area lama disalin
    /// dari `raster` **saat ini** — aman karena area di luar rect lama belum
    /// tersentuh aksi yang sedang berlangsung.
    pub fn expand(&mut self, raster: &Raster, add: Rect) {
        let new_rect = self.rect.union(add).intersect(raster.bounds());
        if new_rect == self.rect {
            return;
        }
        let mut data = raster.region(new_rect);
        // Tempel salinan lama ke posisi lamanya di atas data baru.
        for y in 0..self.rect.h {
            let src = (y * self.rect.w * 4) as usize;
            let dst = (((y + self.rect.y - new_rect.y) * new_rect.w + (self.rect.x - new_rect.x))
                * 4) as usize;
            let len = (self.rect.w * 4) as usize;
            data[dst..dst + len].copy_from_slice(&self.data[src..src + len]);
        }
        self.rect = new_rect;
        self.data = data;
    }

    /// Tuliskan balik ke `raster` (undo/redo).
    pub fn restore(&self, raster: &mut Raster) {
        raster.write_region(self.rect, &self.data);
    }
}

/// Satu langkah aksi yang bisa di-undo: area + piksel sebelum & sesudah.
#[derive(Debug)]
struct Action {
    rect: Rect,
    before: Vec<u8>,
    after: Vec<u8>,
}

/// Riwayat undo/redo berbasis area kotor (hemat memori: hanya area yang
/// berubah, bukan seluruh kanvas).
pub struct History {
    undo: VecDeque<Action>,
    redo: VecDeque<Action>,
    max_entries: usize,
}

/// Jumlah langkah undo default.
pub const DEFAULT_HISTORY: usize = 50;

impl History {
    pub fn new(max_entries: usize) -> Self {
        assert!(max_entries >= 1, "riwayat minimal 1 langkah");
        Self {
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            max_entries,
        }
    }

    /// Catat satu aksi (dipanggil setelah piksel berubah). Aksi baru
    /// membatalkan tumpukan redo.
    pub fn push(&mut self, rect: Rect, before: Vec<u8>, after: Vec<u8>) {
        if rect.is_empty() {
            return;
        }
        self.redo.clear();
        self.undo.push_back(Action {
            rect,
            before,
            after,
        });
        while self.undo.len() > self.max_entries {
            self.undo.pop_front();
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Mundur satu langkah; mengembalikan area yang perlu digambar ulang.
    pub fn undo(&mut self, raster: &mut Raster) -> Option<Rect> {
        let action = self.undo.pop_back()?;
        action.restore_before(raster);
        let rect = action.rect;
        self.redo.push_back(action);
        Some(rect)
    }

    /// Maju satu langkah; mengembalikan area yang perlu digambar ulang.
    pub fn redo(&mut self, raster: &mut Raster) -> Option<Rect> {
        let action = self.redo.pop_back()?;
        action.restore_after(raster);
        let rect = action.rect;
        self.undo.push_back(action);
        Some(rect)
    }
}

impl Action {
    fn restore_before(&self, raster: &mut Raster) {
        raster.write_region(self.rect, &self.before);
    }

    fn restore_after(&self, raster: &mut Raster) {
        raster.write_region(self.rect, &self.after);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink() -> Rgba {
        Rgba::opaque(10, 20, 30)
    }

    #[test]
    fn rect_union_dan_intersect() {
        let a = Rect {
            x: 2,
            y: 2,
            w: 4,
            h: 4,
        };
        let b = Rect {
            x: 5,
            y: 1,
            w: 2,
            h: 2,
        };
        assert_eq!(
            a.union(b),
            Rect {
                x: 2,
                y: 1,
                w: 5,
                h: 5
            }
        );
        assert_eq!(
            a.intersect(b),
            Rect {
                x: 5,
                y: 2,
                w: 1,
                h: 1
            }
        );
        assert!(Rect::EMPTY.union(a) == a);
        assert!(Rect::EMPTY.intersect(a).is_empty());
    }

    #[test]
    fn raster_baru_transparan_dan_batas_v1() {
        let r = Raster::new(4, 4);
        assert_eq!(r.pixel(1, 1), Some(Rgba::default()));
        assert_eq!(r.pixel(-1, 0), None);
        assert_eq!(r.pixel(4, 0), None);
        let result = std::panic::catch_unwind(|| Raster::new(MAX_DIMENSION + 1, 8));
        assert!(result.is_err(), "4097 px harus ditolak (§13.3)");
    }

    #[test]
    fn stamp_keras_opasitas_penuh_di_inti() {
        let mut r = Raster::new(32, 32);
        let brush = Brush {
            size: 10.0,
            opacity: 1.0,
            hardness: 1.0,
        };
        let rect = r.stamp((16.0, 16.0), brush, ink(), false);
        assert!(!rect.is_empty());
        let c = r.pixel(16, 16).unwrap();
        assert_eq!((c.r, c.g, c.b, c.a), (10, 20, 30, 255));
        // Tepi 1 px feather: sekitar radius 5 masih punya alpha parsial.
        assert!(r.pixel(21, 16).unwrap().a > 0 && r.pixel(21, 16).unwrap().a < 255);
        // Di luar radius + feather: transparan.
        assert_eq!(r.pixel(24, 16).unwrap().a, 0);
    }

    #[test]
    fn stamp_opasitas_setengah() {
        let mut r = Raster::new(32, 32);
        let brush = Brush {
            size: 10.0,
            opacity: 0.5,
            hardness: 1.0,
        };
        r.stamp((16.0, 16.0), brush, ink(), false);
        let c = r.pixel(16, 16).unwrap();
        assert!((124..=131).contains(&c.a), "alpha ≈ 128, dapat {}", c.a);
    }

    #[test]
    fn stamp_lembut_pudar_ke_tepi() {
        let mut r = Raster::new(32, 32);
        let brush = Brush {
            size: 20.0,
            opacity: 1.0,
            hardness: 0.0,
        };
        r.stamp((16.0, 16.0), brush, ink(), false);
        let center = r.pixel(16, 16).unwrap().a;
        let mid = r.pixel(22, 16).unwrap().a;
        let edge = r.pixel(25, 16).unwrap().a;
        assert!(
            center > mid && mid > edge,
            "pudar merata: {center} > {mid} > {edge}"
        );
    }

    #[test]
    fn penghapus_mengeluarkan_alpha_saja() {
        let mut r = Raster::new(32, 32);
        let brush = Brush::default();
        r.stamp((16.0, 16.0), brush, ink(), false);
        assert_eq!(r.pixel(16, 16).unwrap().a, 255);
        let erase = Brush {
            opacity: 1.0,
            ..brush
        };
        r.stamp((16.0, 16.0), erase, Rgba::default(), true);
        let c = r.pixel(16, 16).unwrap();
        assert_eq!(c.a, 0);
        assert_eq!((c.r, c.g, c.b), (10, 20, 30), "RGB tidak berubah");
    }

    #[test]
    fn sampler_sambungan_identik_dengan_stroke_lurus() {
        // Stroke bersegi (dua segmen) harus menempa di posisi yang sama
        // dengan satu segmen lurus sepanjang jalur identik — titik sambungan
        // tidak boleh ditempa dua kali (penting bila opasitas < 1).
        let brush = Brush {
            size: 8.0,
            opacity: 0.5,
            hardness: 1.0,
        };
        let mut lurus = Raster::new(64, 16);
        let mut s = StrokeSampler::default();
        s.begin((4.0, 8.0), &mut lurus, brush, ink(), false, None);
        s.segment(
            (4.0, 8.0),
            (20.0, 8.0),
            &mut lurus,
            brush,
            ink(),
            false,
            None,
        );

        let mut bersegi = Raster::new(64, 16);
        let mut s2 = StrokeSampler::default();
        s2.begin((4.0, 8.0), &mut bersegi, brush, ink(), false, None);
        s2.segment(
            (4.0, 8.0),
            (7.0, 8.0),
            &mut bersegi,
            brush,
            ink(),
            false,
            None,
        );
        s2.segment(
            (7.0, 8.0),
            (20.0, 8.0),
            &mut bersegi,
            brush,
            ink(),
            false,
            None,
        );

        for y in 0..16 {
            for x in 0..64 {
                let a = lurus.pixel(x, y).unwrap();
                let b = bersegi.pixel(x, y).unwrap();
                let dekat = [a.r, a.g, a.b, a.a]
                    .iter()
                    .zip([b.r, b.g, b.b, b.a])
                    .all(|(x, y)| x.abs_diff(y) <= 1);
                assert!(
                    dekat,
                    "piksel ({x},{y}) identik: lurus {a:?} vs bersegi {b:?}"
                );
            }
        }
    }

    #[test]
    fn fill_tepat_sasaran_dan_batal_bila_identik() {
        // Baris 1D 16 px: dinding ber­tinta di x=8 memblokir fill dari x=0.
        let mut r = Raster::new(16, 1);
        r.stamp((8.0, 0.5), Brush::default(), Rgba::opaque(200, 0, 0), false);
        let fill = r.flood_fill(0, 0, ink(), 0).unwrap();
        assert!(
            fill.rect.w < 16,
            "fill terhalang dinding, w = {}",
            fill.rect.w
        );
        assert_eq!(r.pixel(0, 0).unwrap(), ink(), "seed ikut terisi");
        assert_eq!(
            r.pixel(15, 0).unwrap().a,
            0,
            "sisi kanan dinding tak tersentuh"
        );
        assert_ne!(r.pixel(8, 0).unwrap(), ink(), "piksel dinding tak berubah");
        // Warna sama → tanpa aksi.
        assert!(r.flood_fill(0, 0, ink(), 0).is_none());
    }

    #[test]
    fn fill_tolerance_menjangkau_warna_mirip() {
        // Dasar (100,100,100,255); x=1 selisih 5 (mirip), x=2 selisih 100.
        let mut r = Raster::new(8, 1);
        r.pixels[0..4].copy_from_slice(&[100, 100, 100, 255]);
        r.pixels[4..8].copy_from_slice(&[105, 100, 100, 255]);
        r.pixels[8..12].copy_from_slice(&[200, 100, 100, 255]);
        let fill = r.flood_fill(0, 0, ink(), 10).unwrap();
        assert_eq!(
            fill.rect.w, 2,
            "toleransi 10 menjangkau selisih 5, berhenti di selisih 100"
        );
        assert_eq!(r.pixel(1, 0).unwrap(), ink(), "warna mirip ikut terisi");
        assert_ne!(r.pixel(2, 0).unwrap(), ink(), "warna jauh tak ikut");
    }

    #[test]
    fn region_buf_expand_pertahankan_sebelum_dan_tangkap_area_baru() {
        let mut r = Raster::new(16, 16);
        let brush = Brush {
            size: 4.0,
            ..Brush::default()
        };
        // Tinta di (2,2); "sebelum" = snapshot area (0,0,4,4).
        r.stamp((2.0, 2.0), brush, ink(), false);
        let mut buf = RegionBuf::capture(
            &r,
            Rect {
                x: 0,
                y: 0,
                w: 4,
                h: 4,
            },
        );
        // Perluas cakupan SEBELUM operasi baru menyentuh area baru —
        // invarian: piksel di luar rect lama masih pristine.
        buf.expand(
            &r,
            Rect {
                x: 8,
                y: 8,
                w: 8,
                h: 8,
            },
        );
        // Operasi aksi: ubah area lama (merah) + area baru (hijau).
        r.stamp((2.0, 2.0), brush, Rgba::opaque(200, 0, 0), false);
        r.stamp((12.0, 12.0), brush, Rgba::opaque(0, 128, 64), false);
        assert_eq!(
            buf.rect,
            Rect {
                x: 0,
                y: 0,
                w: 16,
                h: 16
            }
        );
        // Isi "sebelum" tidak berubah oleh operasi (area lama tetap tinta).
        assert_eq!(buf.pixel_ke(2, 2), ink());
        assert_eq!(
            buf.pixel_ke(12, 12),
            Rgba::default(),
            "area baru tertangkap pristine (sebelum dioperasi)"
        );
        // Restore membalik KEDUA perubahan.
        buf.restore(&mut r);
        assert_eq!(r.pixel(2, 2).unwrap(), ink(), "area lama dipulihkan");
        assert_eq!(
            r.pixel(12, 12).unwrap(),
            Rgba::default(),
            "area baru dikembalikan transparan"
        );
    }

    #[test]
    fn history_undo_redo_dan_batas() {
        let mut r = Raster::new(16, 16);
        let mut h = History::new(2);
        assert!(!h.can_undo());

        // Aksi 1: stamp.
        let rect = r.stamp((8.0, 8.0), Brush::default(), ink(), false);
        let before = vec![0; (rect.w * rect.h * 4) as usize];
        let after = r.region(rect);
        h.push(rect, before, after);
        assert!(r.pixel(8, 8).unwrap().a == 255);

        // Undo mengembalikan transparan; redo mengembalikan isi.
        let undone = h.undo(&mut r).unwrap();
        assert_eq!(undone, rect);
        assert_eq!(r.pixel(8, 8).unwrap().a, 0);
        assert!(h.redo(&mut r).is_some());
        assert_eq!(r.pixel(8, 8).unwrap().a, 255);

        // Aksi baru membatalkan redo.
        let rect2 = r.stamp((2.0, 2.0), Brush::default(), ink(), false);
        h.push(
            rect2,
            vec![0; (rect2.w * rect2.h * 4) as usize],
            r.region(rect2),
        );
        assert!(!h.can_redo());

        // Batas: 2 entri, yang terlama dibuang.
        let rect3 = r.stamp((12.0, 12.0), Brush::default(), ink(), false);
        h.push(
            rect3,
            vec![0; (rect3.w * rect3.h * 4) as usize],
            r.region(rect3),
        );
        let rect4 = r.stamp((14.0, 14.0), Brush::default(), ink(), false);
        h.push(
            rect4,
            vec![0; (rect4.w * rect4.h * 4) as usize],
            r.region(rect4),
        );
        assert!(h.undo(&mut r).is_some());
        assert!(h.undo(&mut r).is_some());
        assert!(!h.can_undo(), "entri terlama dibuang melewati batas");
    }

    // -------------------------------------------------------------------------
    // M4d — clipping seleksi (marquee tahap 1)
    // -------------------------------------------------------------------------

    #[test]
    fn stamp_masked_terbatas_area_seleksi() {
        let mut r = Raster::new(16, 16);
        let brush = Brush {
            size: 6.0,
            ..Brush::default()
        };
        let sel = Seleksi::Kotak(Rect {
            x: 4,
            y: 4,
            w: 4,
            h: 4,
        });
        let rect = r.stamp_masked((6.0, 6.0), brush, ink(), false, Some(&sel));
        assert!(rect.w > 0 && rect.h > 0, "ada area kotor");
        assert!(r.pixel(6, 6).unwrap().a > 0, "inti kuas di dalam seleksi");
        // Radius 3 (+ feather): (3, 6) masih dalam jangkauan kuas TAPI di
        // luar seleksi (x < 4) — wajib tetap transparan.
        assert_eq!(r.pixel(3, 6).unwrap().a, 0, "luar seleksi tak tersentuh");
        assert_eq!(r.pixel(6, 3).unwrap().a, 0, "luar seleksi tak tersentuh");
    }

    #[test]
    fn flood_fill_masked_mengalir_hanya_dalam_seleksi() {
        let mut r = Raster::new(8, 1);
        let sel = Seleksi::Kotak(Rect {
            x: 2,
            y: 0,
            w: 1,
            h: 1,
        });
        let hasil = r
            .flood_fill_masked(2, 0, Rgba::opaque(255, 0, 0), 0, Some(&sel))
            .expect("benih di dalam seleksi");
        assert_eq!(r.pixel(2, 0).unwrap().r, 255, "benih terisi");
        assert_eq!(r.pixel(1, 0).unwrap().a, 0, "tetangga di luar seleksi utuh");
        assert_eq!(r.pixel(3, 0).unwrap().a, 0, "tetangga di luar seleksi utuh");
        assert_eq!(
            hasil.rect,
            Rect {
                x: 2,
                y: 0,
                w: 1,
                h: 1
            },
            "area hasil = mask yang terisi"
        );
    }

    #[test]
    fn flood_fill_masked_benih_di_luar_seleksi_diam() {
        let mut r = Raster::new(8, 1);
        let sel = Seleksi::Kotak(Rect {
            x: 4,
            y: 0,
            w: 1,
            h: 1,
        });
        assert!(
            r.flood_fill_masked(0, 0, Rgba::opaque(255, 0, 0), 0, Some(&sel))
                .is_none(),
            "benih di luar seleksi = tanpa isi"
        );
        assert_eq!(r.pixel(4, 0).unwrap().a, 0, "tak ada efek samping");
    }

    #[test]
    fn clear_in_menghapus_hanya_dalam_seleksi_dan_bisa_undo() {
        let mut r = Raster::new(4, 4);
        r.stamp(
            (2.0, 2.0),
            Brush {
                size: 4.0,
                ..Brush::default()
            },
            ink(),
            false,
        );
        let sel = Seleksi::Kotak(Rect {
            x: 0,
            y: 0,
            w: 2,
            h: 2,
        });
        let hasil = r.clear_in(&sel).expect("area seleksi di dalam kanvas");
        assert_eq!(r.pixel(1, 1).unwrap().a, 0, "dalam seleksi: transparan");
        assert!(r.pixel(3, 1).unwrap().a > 0, "luar seleksi: utuh");
        // Data "sebelum" mengembalikan isi seleksi (jalur undo).
        hasil.before.restore(&mut r);
        assert!(r.pixel(1, 1).unwrap().a > 0, "isi pulih dari data sebelum");
    }

    #[test]
    fn clear_in_seleksi_di_luar_kanvas_kosong() {
        let mut r = Raster::new(4, 4);
        let sel = Seleksi::Kotak(Rect {
            x: 10,
            y: 10,
            w: 2,
            h: 2,
        });
        assert!(r.clear_in(&sel).is_none(), "tak ada area tersentuh");
    }

    #[test]
    fn geser_isi_memindahkan_hanya_isi_seleksi_dan_bisa_undo() {
        let mut r = Raster::new(16, 16);
        // Blok opak di sekitar (3, 3) — terpotong seleksi 2×2 di (2,2).
        r.stamp(
            (3.0, 3.0),
            Brush {
                size: 4.0,
                ..Brush::default()
            },
            ink(),
            false,
        );
        let sel = Seleksi::Kotak(Rect {
            x: 2,
            y: 2,
            w: 2,
            h: 2,
        });
        let hasil = r.geser_isi(&sel, 5, 0).expect("pindahan sah");
        assert_eq!(r.pixel(3, 3).unwrap().a, 0, "posisi lama terangkat");
        assert!(r.pixel(8, 3).unwrap().a > 0, "isi pindah +5 px");
        assert_eq!(
            r.pixel(9, 3).unwrap().a,
            0,
            "di luar lebar seleksi tetap kosong"
        );
        // Data "sebelum" mencakup area lama ∪ baru — undo memulihkan.
        hasil.before.restore(&mut r);
        assert!(r.pixel(3, 3).unwrap().a > 0, "isi pulih setelah undo");
    }

    #[test]
    fn bentuk_kotak_terisi_dan_ellipse_tepi_halus() {
        let mut r = Raster::new(16, 16);
        r.bentuk(
            Rect {
                x: 4,
                y: 4,
                w: 8,
                h: 8,
            },
            false,
            ink(),
            1.0,
            false,
        )
        .expect("area dalam kanvas");
        assert!(r.pixel(8, 8).unwrap().a > 200, "inti kotak penuh");
        assert_eq!(r.pixel(2, 8).unwrap().a, 0, "di luar kotak kosong");

        let mut e = Raster::new(16, 16);
        e.bentuk(
            Rect {
                x: 2,
                y: 2,
                w: 12,
                h: 12,
            },
            true,
            ink(),
            1.0,
            false,
        )
        .expect("area dalam kanvas");
        assert!(e.pixel(8, 8).unwrap().a > 200, "inti ellipse penuh");
        assert_eq!(e.pixel(2, 2).unwrap().a, 0, "sudut ellipse kosong");
        let tepi = e.pixel(11, 12).unwrap().a;
        assert!(
            (1..250).contains(&tepi),
            "tepi ellipse ter-anti-alias (parsial): {tepi}"
        );
    }

    #[test]
    fn bentuk_erase_mengurangi_alpha_dan_opasitas_nol_tanpa_efek() {
        let mut r = Raster::new(8, 8);
        r.stamp((4.0, 4.0), Brush::default(), ink(), false);
        assert!(r.pixel(4, 4).unwrap().a > 0, "terisi dulu");
        r.bentuk(
            Rect {
                x: 2,
                y: 2,
                w: 4,
                h: 4,
            },
            false,
            ink(),
            1.0,
            true,
        )
        .expect("area sah");
        assert_eq!(r.pixel(4, 4).unwrap().a, 0, "erase menghabiskan alpha");

        let mut kosong = Raster::new(8, 8);
        assert!(
            kosong
                .bentuk(
                    Rect {
                        x: 1,
                        y: 1,
                        w: 4,
                        h: 4,
                    },
                    false,
                    ink(),
                    0.0,
                    false
                )
                .is_none(),
            "opacity 0 = tanpa perubahan"
        );
    }

    // Kecil: akses piksel snapshot untuk asersi di atas.
    impl RegionBuf {
        fn pixel_ke(&self, x: u32, y: u32) -> Rgba {
            let i = ((y - self.rect.y) * self.rect.w + (x - self.rect.x)) * 4;
            Rgba {
                r: self.data[i as usize],
                g: self.data[i as usize + 1],
                b: self.data[i as usize + 2],
                a: self.data[i as usize + 3],
            }
        }
    }
}
