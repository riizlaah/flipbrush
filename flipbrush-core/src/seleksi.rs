//! Model seleksi piksel — DESIGN §12.2 (M4d: toolset lanjutan).
//!
//! Seleksi adalah geometri murni di ruang piksel dokumen (tanpa warna,
//! tanpa rendering): operasi gambar memakainya sebagai mask clipping
//! ([`crate::raster::Raster::stamp_masked`], `flood_fill_masked`,
//! [`crate::raster::Raster::clear_in`]), dan UI kanvas menggambar
//! marching-ants overlay dari [`Seleksi::rect`]. Varian marquee dulu
//! (M4d tahap 1); lasso & wand menyusul pada enum yang sama.

use crate::raster::Rect;

/// Area seleksi piksel dokumen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seleksi {
    /// Marquee: persegi hasil drag tool Marquee.
    Kotak(Rect),
}

impl Seleksi {
    /// Bbox area seleksi — dasar clipping & overlay.
    pub fn rect(self) -> Rect {
        match self {
            Self::Kotak(r) => r,
        }
    }

    /// `true` bila piksel `(x, y)` di dalam area seleksi (batas kanan/
    /// bawah eksklusif).
    pub fn contains(self, x: i32, y: i32) -> bool {
        let r = self.rect();
        x >= r.x as i32 && y >= r.y as i32 && x < (r.x + r.w) as i32 && y < (r.y + r.h) as i32
    }

    /// Seleksi hasil drag dua titik koordinat dokumen. `None` bila geseran
    /// kurang dari 1 px — klik polos marquee berarti "hapus seleksi"
    /// (kebiasaan tool seleksi umum). Arah drag tak berpengaruh: semua
    /// piksel yang tersentuh kursor ikut tercakup (floor min → ceil max).
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_mengikuti_bbox_kotak() {
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
            "floor awal → ceil akhir"
        );
        // Drag terbalik hasilnya sama (urutan titik bebas).
        assert_eq!(
            Seleksi::dari_drag((15.9, 13.1), (10.2, 10.7)),
            Some(s),
            "arah drag tak berpengaruh"
        );
    }
}
