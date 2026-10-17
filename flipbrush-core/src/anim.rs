//! Model animasi M3 (DESIGN §12.2): layer, frame, cel, dan exposure.
//!
//! Konsep X-sheet (§7.7): **exposure** = rentang frame `[start, start+length)`
//! pada satu layer yang menunjuk satu **cel** (bitmap unik). Exposure
//! berbagi cel — tidak diduplikasi (§13.2). Canvas & timeline bekerja di
//! atas model ini; modul murni domain, tanpa UI dan tanpa I/O.
//!
//! Batas proyek v1 (§13.3): 16 layer, 600 frame, kanvas 4096² — ditolak
//! dengan pesan jelas di atas itu.

use std::collections::HashMap;

use crate::raster::Raster;

/// ID layer yang stabil (referensi exposure — bukan indeks, agar urutan
/// lapisan boleh berubah tanpa merusak exposure).
pub type LayerId = u32;

/// ID cel unik (referensi exposure; `cels/<id>.png` pada format `.flip` §13.2).
pub type CelId = u32;

/// Jumlah layer maksimum v1 (§13.3).
pub const MAX_LAYERS: usize = 16;
/// Jumlah frame maksimum v1 (§13.3).
pub const MAX_FRAMES: u32 = 600;
/// FPS default proyek (§13.3: playback 12/24 — default 12).
pub const DEFAULT_FPS: u32 = 12;
/// Nama layer pertama.
pub const DEFAULT_LAYER_NAME: &str = "Layer 1";

/// Mode campur layer — v1 hanya normal; field ini disimpan sesuai manifest
/// `.flip` (§13.2 `layers[].blend`) agar format tidak berubah di M5+.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Blend {
    #[default]
    Normal,
}

/// Satu lapisan: metadata untuk timeline (§7.7) dan komposit (§12.2).
///
/// Urutan vektor = urutan Z: indeks `[0]` paling **atas** (panel Layers
/// dan baris teratas X-sheet menampilkan layer atas lebih dulu).
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    /// Visibilitas (kolom toggle di M4; timeline menampilkan semuanya di M3).
    pub visible: bool,
    /// Opasitas komposit 0..1.
    pub opacity: f32,
    pub blend: Blend,
}

/// Exposure: rentang frame pada satu layer yang menunjuk satu cel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exposure {
    pub layer: LayerId,
    /// Frame pertama (inklusif).
    pub start: u32,
    /// Panjang exposure dalam frame (≥ 1).
    pub length: u32,
    /// Cel yang ditunjuk (berbagi antar frame dalam exposure).
    pub cel_id: CelId,
}

impl Exposure {
    /// Frame terakhir (eksklusif).
    pub fn end(&self) -> u32 {
        self.start + self.length
    }

    /// `frame` berada di dalam exposure ini.
    pub fn covers(&self, frame: u32) -> bool {
        self.start <= frame && frame < self.end()
    }
}

/// Proyek animasi: kanvas + daftar layer + exposure + cel (bitmap RGBA8).
pub struct Animation {
    /// Lebar kanvas dokumen (px, ≤ 4096 §13.3 — dijaga `Raster::new`).
    pub width: u32,
    /// Tinggi kanvas dokumen (px).
    pub height: u32,
    /// Frame per detik (§13.3: 12/24).
    pub fps: u32,
    layers: Vec<Layer>,
    exposures: Vec<Exposure>,
    /// Cel unik yang tersimpan; cel yang sedang berada di buffer editing
    /// kanvas tidak ada di sini sampai dikembalikan (lihat [`Self::take_cel`]).
    cels: HashMap<CelId, Raster>,
    next_layer: LayerId,
    next_cel: CelId,
}

impl Animation {
    /// Proyek baru: kanvas `width × height`, satu layer default, kosong.
    pub fn new(width: u32, height: u32) -> Self {
        assert!(width >= 1 && height >= 1, "ukuran kanvas minimal 1×1 px");
        let first = Layer {
            id: 1,
            name: DEFAULT_LAYER_NAME.to_owned(),
            visible: true,
            opacity: 1.0,
            blend: Blend::default(),
        };
        Self {
            width,
            height,
            fps: DEFAULT_FPS,
            layers: vec![first],
            exposures: Vec::new(),
            cels: HashMap::new(),
            next_layer: 2,
            next_cel: 1,
        }
    }

    // ------------------------------------------------------------------
    // Layer
    // ------------------------------------------------------------------

    /// Semua layer, urut Z (indeks `[0]` = paling atas).
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// ID layer pada indeks baris timeline.
    pub fn layer_id(&self, index: usize) -> Option<LayerId> {
        self.layers.get(index).map(|layer| layer.id)
    }

    /// Indeks baris timeline untuk layer (None bila tak dikenal).
    pub fn layer_index(&self, id: LayerId) -> Option<usize> {
        self.layers.iter().position(|layer| layer.id == id)
    }

    /// Tambah layer baru di posisi paling atas (setelah layer default yang
    /// sudah ada). `Err` bila sudah [`MAX_LAYERS`] (§13.3).
    pub fn add_layer(&mut self, name: impl Into<String>) -> Result<LayerId, usize> {
        if self.layers.len() >= MAX_LAYERS {
            return Err(MAX_LAYERS);
        }
        let id = self.next_layer;
        self.next_layer += 1;
        self.layers.insert(
            0,
            Layer {
                id,
                name: name.into(),
                visible: true,
                opacity: 1.0,
                blend: Blend::default(),
            },
        );
        Ok(id)
    }

    /// Hapus layer beserta exposure-nya; cel yang tak lagi dirujuk exposure
    /// mana pun ikut dibuang. `false` bila layer terakhir tidak boleh
    /// dihapus (proyek wajib punya ≥ 1 layer).
    pub fn remove_layer(&mut self, id: LayerId) -> bool {
        if self.layers.len() <= 1 || !self.layers.iter().any(|layer| layer.id == id) {
            return false;
        }
        self.layers.retain(|layer| layer.id != id);
        self.exposures.retain(|exposure| exposure.layer != id);
        self.prune_cels();
        true
    }

    /// Buang cel yang tak lagi dirujuk exposure mana pun.
    fn prune_cels(&mut self) {
        let used: Vec<CelId> = self.exposures.iter().map(|e| e.cel_id).collect();
        self.cels.retain(|id, _| used.contains(id));
    }

    // ------------------------------------------------------------------
    // Frame & exposure
    // ------------------------------------------------------------------

    /// Durasi proyek: ujung exposure terpanjang, minimal 1 frame.
    /// (Timeline boleh menampilkan lebih banyak kolom kosong — lihat
    /// `timeline.rs`; angka ini adalah durasi "nyata".)
    pub fn frames(&self) -> u32 {
        self.exposures
            .iter()
            .map(Exposure::end)
            .max()
            .unwrap_or(1)
            .max(1)
    }

    /// Exposure yang menutupi `(layer, frame)`.
    pub fn exposure_at(&self, layer: LayerId, frame: u32) -> Option<&Exposure> {
        self.exposures
            .iter()
            .find(|e| e.layer == layer && e.covers(frame))
    }

    /// Cel yang menutupi `(layer, frame)` — `None` bila frame kosong atau
    /// cel-nya sedang berada di buffer editing kanvas.
    pub fn cel_id_at(&self, layer: LayerId, frame: u32) -> Option<CelId> {
        self.exposure_at(layer, frame).map(|e| e.cel_id)
    }

    /// Pastikan `(layer, frame)` punya exposure: bila sudah ada → cel
    /// yang sudah dipakai; bila kosong → exposure baru `length` 1 dengan cel
    /// baru (raster BELUM dimasukkan ke penyimpanan — ekspos dulu via
    /// [`Self::take_cel`] saat cel dibutuhkan, atau [`Self::put_cel`]
    /// saat dikembalikan dari buffer kanvas).
    ///
    /// `None` bila layer tak dikenal atau `frame` di atas batas 600 (§13.3).
    pub fn expose(&mut self, layer: LayerId, frame: u32) -> Option<CelId> {
        if frame >= MAX_FRAMES || !self.layers.iter().any(|l| l.id == layer) {
            return None;
        }
        if let Some(exposure) = self
            .exposures
            .iter()
            .find(|e| e.layer == layer && e.covers(frame))
        {
            return Some(exposure.cel_id);
        }
        let cel_id = self.next_cel;
        self.next_cel += 1;
        self.exposures.push(Exposure {
            layer,
            start: frame,
            length: 1,
            cel_id,
        });
        Some(cel_id)
    }

    /// Perpanjang exposure terakhir `(layer, frame)` sepanjang `extend`
    /// frame (untuk "tahan exposure" di timeline; `None` bila bukan
    /// kelanjutan exposure yang ada).
    pub fn extend_exposure(&mut self, layer: LayerId, frame: u32, extend: u32) -> Option<CelId> {
        if extend == 0 {
            return self.exposure_at(layer, frame).map(|e| e.cel_id);
        }
        let (index, start, end, cel_id) = {
            let index = self
                .exposures
                .iter()
                .position(|e| e.layer == layer && e.covers(frame))?;
            let exposure = &self.exposures[index];
            (index, exposure.start, exposure.end(), exposure.cel_id)
        };
        let new_end = (end + extend).min(MAX_FRAMES);
        // Batasi oleh exposure berikutnya di layer yang sama — perpanjangan
        // berhenti tepat sebelumnya (menahan exposure, §7.7).
        let limit = self
            .exposures
            .iter()
            .filter(|other| other.layer == layer && other.cel_id != cel_id && other.start >= end)
            .map(|other| other.start)
            .min()
            .unwrap_or(MAX_FRAMES);
        let new_end = new_end.min(limit);
        if new_end > end {
            self.exposures[index].length = new_end - start;
        }
        Some(cel_id)
    }

    // ------------------------------------------------------------------
    // Penyimpanan cel
    // ------------------------------------------------------------------

    /// Ambil raster cel dari penyimpanan (mengeluarkannya — cel pindah ke
    /// buffer editing kanvas). `None` bila cel tidak ada di penyimpanan
    /// (tak dikenal, atau memang sedang berada di buffer).
    pub fn take_cel(&mut self, id: CelId) -> Option<Raster> {
        self.cels.remove(&id)
    }

    /// Kembalikan/simpan raster cel ke penyimpanan.
    pub fn put_cel(&mut self, id: CelId, raster: Raster) {
        self.cels.insert(id, raster);
    }

    /// Cel tersimpan (untuk tes & penggunaan read-only).
    pub fn cel(&self, id: CelId) -> Option<&Raster> {
        self.cels.get(&id)
    }

    /// Jumlah cel tersimpan (termasuk yang sedang di buffer? tidak — hanya
    /// yang ada di penyimpanan).
    pub fn stored_cel_count(&self) -> usize {
        self.cels.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proyek_baru_minimal_dan_dinamis() {
        let mut anim = Animation::new(64, 32);
        assert_eq!(anim.layers().len(), 1);
        assert_eq!(anim.layers()[0].name, DEFAULT_LAYER_NAME);
        assert_eq!(anim.frames(), 1, "tanpa exposure → durasi minimal 1");
        assert_eq!(anim.fps, DEFAULT_FPS);
        assert_eq!(anim.layer_id(0), Some(1));

        let a = anim.add_layer("Line").unwrap();
        let b = anim.add_layer("Color").unwrap();
        assert_eq!(anim.layers().len(), 3);
        // Layer baru masuk posisi atas (indeks 0).
        assert_eq!(anim.layer_id(0), Some(b));
        assert_eq!(anim.layer_id(1), Some(a));
        assert_eq!(anim.layer_index(a), Some(1));
    }

    #[test]
    fn expose_membuat_exposure_reuse_dan_memperpanjang_durasi() {
        let mut anim = Animation::new(64, 64);
        let layer = anim.layer_id(0).unwrap();

        let cel0 = anim.expose(layer, 0).unwrap();
        assert_eq!(anim.frames(), 1);
        // Frame yang sama → cel yang sama (reuse, tidak diduplikasi).
        assert_eq!(anim.expose(layer, 0), Some(cel0));
        // Frame kosong → cel baru, durasi melebar.
        let cel5 = anim.expose(layer, 5).unwrap();
        assert_ne!(cel0, cel5);
        assert_eq!(anim.frames(), 6);
        // Antar exposure tidak tumpang tindih; frame 1..4 tetap kosong.
        assert!(anim.exposure_at(layer, 1).is_none());
        assert!(anim.exposure_at(layer, 4).is_none());
        assert_eq!(anim.cel_id_at(layer, 5), Some(cel5));
        // Exposure yang ada tidak tertimpa oleh expose di frame terisi.
        assert_eq!(anim.expose(layer, 5), Some(cel5));
    }

    #[test]
    fn expose_layer_tak_dikenal_dan_frame_batas_gagal() {
        let mut anim = Animation::new(8, 8);
        assert!(anim.expose(99, 0).is_none(), "layer tak dikenal ditolak");
        assert!(
            anim.expose(1, MAX_FRAMES).is_none(),
            "frame ≥ 600 ditolak (§13.3)"
        );
    }

    #[test]
    fn batas_enam_belas_layer() {
        let mut anim = Animation::new(8, 8);
        for i in 1..MAX_LAYERS {
            anim.add_layer(format!("L{i}")).unwrap();
        }
        assert_eq!(anim.layers().len(), MAX_LAYERS);
        assert_eq!(anim.add_layer("L17"), Err(MAX_LAYERS), "16 layer maks");
    }

    #[test]
    fn remove_layer_membersihkan_exposure_dan_cel_yang_tak_dipakai() {
        let mut anim = Animation::new(8, 8);
        let keep = anim.layer_id(0).unwrap();
        let extra = anim.add_layer("Extra").unwrap();

        let cel_keep = anim.expose(keep, 0).unwrap();
        let cel_extra = anim.expose(extra, 0).unwrap();
        // Simulasikan cel tersimpan di penyimpanan.
        anim.put_cel(cel_keep, Raster::new(8, 8));
        anim.put_cel(cel_extra, Raster::new(8, 8));
        assert_eq!(anim.stored_cel_count(), 2);

        assert!(anim.remove_layer(extra), "layer non-terakhir terhapus");
        assert_eq!(anim.layers().len(), 1);
        assert!(
            anim.exposure_at(extra, 0).is_none(),
            "exposure layer terhapus ikut hilang"
        );
        assert!(
            anim.cel(cel_keep).is_some(),
            "cel yang masih dirujuk bertahan"
        );
        assert!(
            anim.cel(cel_extra).is_none(),
            "cel yatim ikut dibuang (hemat memori §13.3)"
        );

        assert!(
            !anim.remove_layer(keep),
            "layer terakhir tidak bisa dihapus"
        );
    }

    #[test]
    fn take_dan_put_cel_roundtrip() {
        let mut anim = Animation::new(16, 16);
        let layer = anim.layer_id(0).unwrap();
        let id = anim.expose(layer, 3).unwrap();

        // Baru diekspos: exposure ada, raster belum pernah disimpan.
        assert!(anim.cel(id).is_none());
        assert!(anim.take_cel(id).is_none(), "belum pernah disimpan");
        assert!(anim.exposure_at(layer, 3).is_some());

        // Simulasi buffer editing kanvas: simpan, lalu keluarkan lagi.
        anim.put_cel(id, Raster::new(16, 16));
        let raster = anim.take_cel(id).expect("tersimpan");
        let _ = raster; // "buffer kanvas" memegangnya
        assert!(anim.cel(id).is_none(), "di buffer, bukan di penyimpanan");
        assert!(
            anim.exposure_at(layer, 3).is_some(),
            "exposure tidak hilang cel-nya di buffer"
        );

        anim.put_cel(id, Raster::new(16, 16));
        assert!(anim.cel(id).is_some(), "kembali setelah disimpan");
        assert_eq!(anim.cel_id_at(layer, 3), Some(id));
    }

    #[test]
    fn extend_exposure_tidak_menabrak_exposure_berikutnya() {
        let mut anim = Animation::new(8, 8);
        let layer = anim.layer_id(0).unwrap();
        let a = anim.expose(layer, 0).unwrap();
        let b = anim.expose(layer, 3).unwrap();

        // Perpanjang exposure pertama: berhenti sebelum exposure kedua.
        anim.extend_exposure(layer, 0, 10).unwrap();
        let exp = anim.exposure_at(layer, 0).unwrap();
        assert_eq!(exp.end(), 3, "terhenti di frame 3 milik cel lain");
        // Tetap berbagi cel yang sama untuk kelanjutannya.
        assert_eq!(anim.cel_id_at(layer, 1), Some(a));
        assert_eq!(anim.cel_id_at(layer, 2), Some(a));
        assert_eq!(anim.cel_id_at(layer, 3), Some(b), "cel B utuh");
    }
}
