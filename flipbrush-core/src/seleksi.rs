//! Model seleksi piksel — DESIGN §12.2 (M4d: toolset lanjutan).
//!
//! Seleksi adalah geometri di ruang piksel dokumen (tanpa warna, tanpa
//! rendering): operasi gambar memakainya sebagai mask clipping
//! ([`crate::raster::Raster::stamp_masked`], `flood_fill_masked`,
//! [`crate::raster::Raster::clear_in`]), dan UI kanvas menggambar
//! marching-ants overlay dari [`Seleksi::rect`] (+ bentuk aslinya).
//!
//! Varian: marquee (`Kotak`, tahap 1), lasso (`Poligon`) & wand
//! (`Mask` — region flood-fill yang sudah diresolusi jadi bitmask,
//! tahap 2).

use crate::raster::{Raster, Rect};

/// Area seleksi piksel dokumen.
#[derive(Clone, Debug, PartialEq)]
pub enum Seleksi {
    /// Marquee: persegi hasil drag tool Marquee.
    Kotak(Rect),
    /// Lasso: poligon tertutup dari jalur drag bebas (koordinat dokumen,
    /// urutan jalur; tepi terakhir–pertama ditutup otomatis).
    Poligon(Vec<[f32; 2]>),
    /// Mask hasil resolusi flood-fill (wand): bbox + 1 byte per piksel
    /// (1 = terpilih), baris-major. Diresolusi sekali saat klik —
    /// operasi clipping cukup cek bit.
    Mask { rect: Rect, bits: Vec<u8> },
}

impl Seleksi {
    /// Bbox area seleksi — dasar clipping & overlay.
    pub fn rect(&self) -> Rect {
        match self {
            Self::Kotak(r) => *r,
            Self::Poligon(points) => bbox_poligon(points),
            Self::Mask { rect, .. } => *rect,
        }
    }

    /// `true` bila piksel `(x, y)` di dalam area seleksi (batas kanan/
    /// bawah eksklusif; poligon diuji pada pusat piksel).
    pub fn contains(&self, x: i32, y: i32) -> bool {
        match self {
            Self::Kotak(r) => {
                x >= r.x as i32
                    && y >= r.y as i32
                    && x < (r.x + r.w) as i32
                    && y < (r.y + r.h) as i32
            }
            Self::Poligon(points) => poligon_memuat(points, x as f32 + 0.5, y as f32 + 0.5),
            Self::Mask { rect, bits } => {
                let (rx, ry) = (rect.x as i32, rect.y as i32);
                if x < rx || y < ry || x >= rx + rect.w as i32 || y >= ry + rect.h as i32 {
                    return false;
                }
                let idx = ((y - ry) * rect.w as i32 + (x - rx)) as usize;
                bits.get(idx).is_some_and(|&b| b == 1)
            }
        }
    }

    /// Seleksi marquee hasil drag dua titik koordinat dokumen. `None` bila
    /// geseran kurang dari 1 px — klik polos marquee berarti "hapus
    /// seleksi". Arah drag tak berpengaruh (floor min → ceil max).
    pub fn dari_drag(start: (f32, f32), end: (f32, f32)) -> Option<Self> {
        if (start.0 - end.0).abs() < 1.0 && (start.1 - end.1).abs() < 1.0 {
            return None;
        }
        let r = Rect::from_points(
            start.0.min(end.0).floor() as i32,
            start.1.min(end.1).floor() as i32,
            start.0.max(end.0).ceil() as i32,
            start.1.max(end.1).ceil() as i32,
        );
        (!r.is_empty()).then_some(Self::Kotak(r))
    }

    /// Seleksi lasso dari jalur drag (koordinat dokumen, urutan jalur —
    /// tepi terakhir–pertama ditutup otomatis). `None` bila kurang dari 3
    /// titik atau bbox-nya kosong (klik polos / goresan nyaris lurus
    /// sempit = hapus seleksi).
    pub fn dari_lasso(points: Vec<[f32; 2]>) -> Option<Self> {
        if points.len() < 3 {
            return None;
        }
        let r = bbox_poligon(&points);
        if r.is_empty() {
            return None;
        }
        Some(Self::Poligon(points))
    }

    /// Seleksi wand: resolusi flood-fill 4-connected dari `(x, y)` pada
    /// `raster` (biasanya cel aktif) menjadi mask piksel — `toleransi`
    /// = selisih kanal maksimum 0..255 (sama dengan tool fill). `None`
    /// bila benih di luar kanvas atau tak ada piksel cocok.
    pub fn dari_wand(raster: &Raster, x: i32, y: i32, tolerance: u8) -> Option<Self> {
        let (w, h) = (raster.width as i32, raster.height as i32);
        if x < 0 || y < 0 || x >= w || y >= h {
            return None;
        }
        let seed = raster.pixel(x, y)?;
        let cocok = |p: crate::raster::Rgba| {
            p.r.abs_diff(seed.r) <= tolerance
                && p.g.abs_diff(seed.g) <= tolerance
                && p.b.abs_diff(seed.b) <= tolerance
                && p.a.abs_diff(seed.a) <= tolerance
        };

        let mut mask = vec![0u8; (w * h) as usize];
        let mut rect = Rect::EMPTY;
        let mut stack = vec![(x, y)];
        while let Some((sx, sy)) = stack.pop() {
            let i = (sy * w + sx) as usize;
            if mask[i] != 0 {
                continue;
            }
            // Rentang horizontal berisi di baris `sy`.
            let mut x0 = sx;
            while x0 > 0
                && mask[(sy * w + x0 - 1) as usize] == 0
                && cocok(raster.pixel(x0 - 1, sy)?)
            {
                x0 -= 1;
            }
            let mut x1 = sx;
            while x1 + 1 < w
                && mask[(sy * w + x1 + 1) as usize] == 0
                && cocok(raster.pixel(x1 + 1, sy)?)
            {
                x1 += 1;
            }
            mask[(sy * w + sx) as usize] = 1;
            for x in x0..=x1 {
                mask[(sy * w + x) as usize] = 1;
            }
            rect = rect.union(Rect {
                x: x0 as u32,
                y: sy as u32,
                w: (x1 - x0 + 1) as u32,
                h: 1,
            });
            // Rentang tetangga di baris atas & bawah.
            for ny in [sy - 1, sy + 1] {
                if ny < 0 || ny >= h {
                    continue;
                }
                let mut xi = x0;
                while xi <= x1 {
                    if mask[(ny * w + xi) as usize] == 0 && cocok(raster.pixel(xi, ny)?) {
                        stack.push((xi, ny));
                        while xi <= x1
                            && mask[(ny * w + xi) as usize] == 0
                            && cocok(raster.pixel(xi, ny)?)
                        {
                            xi += 1;
                        }
                    } else {
                        xi += 1;
                    }
                }
            }
        }
        if rect.is_empty() {
            return None;
        }
        // Potong mask penuh ke bbox → bits baris-major.
        let mut bits = Vec::with_capacity((rect.w * rect.h) as usize);
        for y in rect.y..rect.y + rect.h {
            let row = (y as i32 * w + rect.x as i32) as usize;
            bits.extend_from_slice(&mask[row..row + rect.w as usize]);
        }
        Some(Self::Mask { rect, bits })
    }

    /// Geser geometri seleksi `(dx, dy)` — tool Move (M4d tahap 3).
    /// Potongan keluar kanvas dipotong ke batas (koordinat unsigned:
    /// `Rect::from_points` memangkas ke ≥ 0); `None` bila hasilnya habis
    /// seluruhnya di luar kanvas.
    pub fn digeser(self, dx: i32, dy: i32) -> Option<Self> {
        match self {
            Self::Kotak(r) => {
                let baru = Rect::from_points(
                    r.x as i32 + dx,
                    r.y as i32 + dy,
                    (r.x + r.w) as i32 + dx,
                    (r.y + r.h) as i32 + dy,
                );
                (!baru.is_empty()).then_some(Self::Kotak(baru))
            }
            Self::Poligon(points) => Some(Self::Poligon(
                points
                    .iter()
                    .map(|p| [p[0] + dx as f32, p[1] + dy as f32])
                    .collect(),
            )),
            Self::Mask { rect, bits } => {
                let baru = Rect::from_points(
                    rect.x as i32 + dx,
                    rect.y as i32 + dy,
                    (rect.x + rect.w) as i32 + dx,
                    (rect.y + rect.h) as i32 + dy,
                );
                if baru.is_empty() {
                    return None;
                }
                let mut bits_baru = vec![0u8; (baru.w * baru.h) as usize];
                for y in baru.y..baru.y + baru.h {
                    for x in baru.x..baru.x + baru.w {
                        let sumber = ((y as i32 - dy - rect.y as i32) * rect.w as i32
                            + (x as i32 - dx - rect.x as i32))
                            as usize;
                        if bits.get(sumber).is_some_and(|&b| b == 1) {
                            let tujuan = ((y - baru.y) * baru.w + (x - baru.x)) as usize;
                            bits_baru[tujuan] = 1;
                        }
                    }
                }
                Some(Self::Mask {
                    rect: baru,
                    bits: bits_baru,
                })
            }
        }
    }
}

/// Bbox poligon (floor min → ceil max); kosong bila tak ada titik.
fn bbox_poligon(points: &[[f32; 2]]) -> Rect {
    let Some(first) = points.first() else {
        return Rect::EMPTY;
    };
    let (mut x0, mut y0, mut x1, mut y1) = (first[0], first[1], first[0], first[1]);
    for p in points {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    Rect::from_points(
        x0.floor() as i32,
        y0.floor() as i32,
        x1.ceil() as i32,
        y1.ceil() as i32,
    )
}

/// Uji titik-in-poligon (even-odd ray casting horizontal); poligon kurang
/// dari 3 titik tak memuat apa pun.
fn poligon_memuat(points: &[[f32; 2]], x: f32, y: f32) -> bool {
    let n = points.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        if (a[1] > y) != (b[1] > y) {
            let t = (y - a[1]) / (b[1] - a[1]);
            if a[0] + t * (b[0] - a[0]) > x {
                inside = !inside;
            }
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::{Brush, Rgba};

    #[test]
    fn kotak_contains_mengikuti_bbox() {
        let s = Seleksi::Kotak(Rect {
            x: 2,
            y: 3,
            w: 4,
            h: 5,
        });
        assert!(s.contains(2, 3), "titik kiri-atas ikut");
        assert!(s.contains(5, 7), "titik kanan-bawah ikut");
        assert!(!s.contains(6, 3), "batas kanan eksklusif");
        assert!(!s.contains(2, 8), "batas bawah eksklusif");
        assert!(!s.contains(1, 3), "di luar kiri");
    }

    #[test]
    fn dari_drag_klik_pola_kosong_drag_membentuk_kotak() {
        assert_eq!(
            Seleksi::dari_drag((10.0, 10.0), (10.4, 10.2)),
            None,
            "klik polos = hapus seleksi"
        );
        let s = Seleksi::dari_drag((10.2, 10.7), (15.9, 13.1)).expect("drag sah");
        assert_eq!(
            s.rect(),
            Rect {
                x: 10,
                y: 10,
                w: 6,
                h: 4
            },
            "floor min → ceil max"
        );
        // Drag terbalik hasilnya sama (urutan titik bebas).
        assert_eq!(
            Seleksi::dari_drag((15.9, 13.1), (10.2, 10.7)),
            Some(s),
            "arah drag tak berpengaruh"
        );
    }

    #[test]
    fn poligon_memuat_dalam_luar_dan_degenerate() {
        // Persegi (1,1)–(4,4).
        let segi_empat = [[1.0, 1.0], [4.0, 1.0], [4.0, 4.0], [1.0, 4.0]];
        assert!(poligon_memuat(&segi_empat, 2.5, 2.5), "di dalam");
        assert!(!poligon_memuat(&segi_empat, 0.5, 2.5), "di luar kiri");
        assert!(!poligon_memuat(&segi_empat, 2.5, 4.5), "di luar bawah");
        assert!(
            !poligon_memuat(&[[0.0, 0.0], [1.0, 1.0]], 0.5, 0.5),
            "kurang dari 3 titik tak memuat"
        );
    }

    #[test]
    fn dari_lasso_klik_pola_kosong_jalur_sah_membentuk_poligon() {
        assert_eq!(Seleksi::dari_lasso(vec![]), None, "tanpa titik");
        assert_eq!(
            Seleksi::dari_lasso(vec![[2.0, 2.0], [2.0, 2.1]]),
            None,
            "klik polos / nyaris titik"
        );
        let s = Seleksi::dari_lasso(vec![[1.0, 1.0], [5.0, 1.5], [3.0, 4.0], [1.2, 3.9]])
            .expect("jalur lasso sah");
        assert!(s.contains(2, 2), "titik di dalam poligon");
        assert!(!s.contains(8, 8), "titik di luar poligon");
        let r = s.rect();
        assert!(r.x <= 1 && r.y <= 1 && r.x + r.w >= 5 && r.y + r.h >= 4);
    }

    #[test]
    fn dari_wand_mengikuti_bentuk_area_warna() {
        let mut r = Raster::new(16, 16);
        // Blok opak (5,5)–(9,9); sisanya transparan.
        r.stamp(
            (7.0, 7.0),
            Brush {
                size: 4.0,
                ..Brush::default()
            },
            Rgba::opaque(200, 40, 40),
            false,
        );
        let s = Seleksi::dari_wand(&r, 7, 7, 0).expect("benih di dalam blok");
        assert!(s.contains(7, 7), "inti blok terpilih");
        assert!(!s.contains(0, 0), "area transparan di luar blok");
        assert!(!s.contains(15, 15), "pojok kanvas ikut tak terpilih");
        // Benih di luar blok (transparan) → hanya area transparan yang
        // terpilih — blok opak jadi "pulau" yang tak ditembus.
        let luar = Seleksi::dari_wand(&r, 0, 0, 0).expect("benih transparan");
        assert!(!luar.contains(7, 7), "blok opak tak ditembus");
        assert!(luar.contains(0, 15), "area transparan terpilih");
    }

    #[test]
    fn dari_wand_toleransi_dan_batas_kanvas() {
        let mut r = Raster::new(8, 1);
        r.stamp(
            (3.0, 0.5),
            Brush {
                size: 3.0,
                ..Brush::default()
            },
            Rgba::opaque(100, 100, 100),
            false,
        );
        // Toleransi 0: blok (2..5) tak menyatu dengan sisa transparan.
        let ketat = Seleksi::dari_wand(&r, 3, 0, 0).expect("benih valid");
        assert!(!ketat.contains(7, 0), "toleransi ketat berhenti di blok");
        // Toleransi longgar (alpha beda 255 ≤ 255): seluruh baris satu
        // area — blok & transparan menyatu.
        let longgar = Seleksi::dari_wand(&r, 3, 0, 255).expect("benih valid");
        assert!(longgar.contains(7, 0), "toleransi longgar menyatukan");
        assert!(Seleksi::dari_wand(&r, -1, 0, 0).is_none(), "benih di luar");
        assert!(Seleksi::dari_wand(&r, 8, 0, 0).is_none(), "benih di luar");
    }

    #[test]
    fn digeser_kotak_dan_poligon() {
        let kotak = Seleksi::Kotak(Rect {
            x: 2,
            y: 2,
            w: 4,
            h: 4,
        });
        assert_eq!(
            kotak.clone().digeser(3, -2).map(|s| s.rect()),
            Some(Rect {
                x: 5,
                y: 0,
                w: 4,
                h: 4
            }),
            "kotak: y negatif memangkas ke 0"
        );
        let poli = Seleksi::Poligon(vec![[1.0, 1.0], [3.0, 1.0], [2.0, 3.0]]);
        assert_eq!(
            poli.digeser(2, 2).map(|s| s.rect()),
            Some(Rect {
                x: 3,
                y: 3,
                w: 2,
                h: 2
            }),
            "poligon: semua titik ikut bergeser"
        );
    }

    #[test]
    fn digeser_mask_memindah_bits() {
        // Mask 3×1 di x=2 — hanya kolom tengah terpilih.
        let sel = Seleksi::Mask {
            rect: Rect {
                x: 2,
                y: 0,
                w: 3,
                h: 1,
            },
            bits: vec![0, 1, 0],
        };
        let hasil = sel.digeser(2, 0).expect("masih di kanvas");
        assert_eq!(
            hasil.rect(),
            Rect {
                x: 4,
                y: 0,
                w: 3,
                h: 1
            }
        );
        assert!(!hasil.contains(4, 0), "kolom lama ikut bergeser");
        assert!(hasil.contains(5, 0), "kolom terpilih pindah +2 (x=3 → x=5)");
        assert!(!hasil.contains(6, 0), "tetangga tak terpilih tetap kosong");
    }
}
