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

use crate::raster::{MAX_DIMENSION, Raster, Rect};

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
#[derive(Clone)]
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

    /// Layer berdasarkan ID — panel Layers (§7.6) mengubah nama/visibilitas/
    /// opasitas tanpa mengandalkan indeks yang bisa bergeser.
    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|layer| layer.id == id)
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

    /// Geser layer ke indeks `to_index` (reorder panel Layers §7.6). Exposure
    /// merujuk ID layer — aman digeser; cel tidak ikut berubah. `false` bila
    /// id tak dikenal, tujuan di luar rentang, atau sudah berada di sana.
    pub fn move_layer(&mut self, id: LayerId, to_index: usize) -> bool {
        if to_index >= self.layers.len() {
            return false;
        }
        let Some(from) = self.layer_index(id) else {
            return false;
        };
        if from == to_index {
            return false;
        }
        let layer = self.layers.remove(from);
        self.layers.insert(to_index, layer);
        true
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

    // ------------------------------------------------------------------
    // Komposit
    // ------------------------------------------------------------------

    /// Gabungkan semua layer yang terlihat pada `frame` menjadi satu raster
    /// RGBA8 — dari bawah ke atas, src-over per opasitas layer (`layers[0]`
    /// paling atas). Dasar preview `.flip` (§13.2) dan ekspor M5 (§13.1).
    /// Cel yang masih berada di buffer editing diperlakukan kosong.
    pub fn flatten_frame(&self, frame: u32) -> Raster {
        let mut out = Raster::new(self.width, self.height);
        for layer in self.layers.iter().rev() {
            if !layer.visible {
                continue;
            }
            let Some(exp) = self.exposure_at(layer.id, frame) else {
                continue;
            };
            let Some(cel) = self.cel(exp.cel_id) else {
                continue;
            };
            composite_over(&mut out, cel, layer.opacity);
        }
        out
    }

    // ------------------------------------------------------------------
    // Persistensi (`.flip` §13.2 — lihat modul [`crate::flip`])
    // ------------------------------------------------------------------

    /// Semua exposure untuk simpan/muat (urut penambahan).
    pub fn exposures(&self) -> &[Exposure] {
        &self.exposures
    }

    /// Cel tersimpan `(id, raster)` untuk encoding `.flip` (cel yang sedang
    /// berada di buffer editing tidak ikut — flush dulu sebelum simpan).
    pub fn stored_cels(&self) -> impl Iterator<Item = (CelId, &Raster)> {
        self.cels.iter().map(|(id, raster)| (*id, raster))
    }

    /// Bangun proyek dari hasil dekode `.flip` (§13.2) — memvalidasi ukuran
    /// kanvas (1..=4096 §13.3), fps ≥ 1, layer (≥ 1, ≤ 16, ID unik), dan
    /// setiap exposure (layer dikenal, panjang ≥ 1, di bawah 600 frame,
    /// tanpa tumpang-tindih per layer, raster cel = ukuran kanvas).
    /// `next_layer`/`next_cel` dihitung ulang dari ID maksimum.
    pub fn from_saved(
        width: u32,
        height: u32,
        fps: u32,
        layers: Vec<Layer>,
        exposures: Vec<Exposure>,
        cels: HashMap<CelId, Raster>,
    ) -> Result<Self, SaveError> {
        if !(1..=MAX_DIMENSION).contains(&width) || !(1..=MAX_DIMENSION).contains(&height) {
            return Err(SaveError::Canvas(width, height));
        }
        if fps < 1 {
            return Err(SaveError::Fps(fps));
        }
        if layers.is_empty() {
            return Err(SaveError::NoLayers);
        }
        if layers.len() > MAX_LAYERS {
            return Err(SaveError::TooManyLayers(layers.len()));
        }
        for (i, layer) in layers.iter().enumerate() {
            if layers[..i].iter().any(|other| other.id == layer.id) {
                // ID kembar = referensi exposure tidak lagi deterministik.
                return Err(SaveError::DuplicateLayer(layer.id));
            }
        }
        for (i, exposure) in exposures.iter().enumerate() {
            if !layers.iter().any(|l| l.id == exposure.layer) {
                return Err(SaveError::UnknownLayer(exposure.layer));
            }
            if exposure.length < 1 {
                return Err(SaveError::Exposure(i, "panjang < 1"));
            }
            if exposure.end() > MAX_FRAMES {
                return Err(SaveError::Frame(exposure.end()));
            }
            if let Some(other) = exposures[..i].iter().find(|other| {
                other.layer == exposure.layer
                    && other.start < exposure.end()
                    && exposure.start < other.end()
            }) {
                return Err(SaveError::Overlap(exposure.layer, other.start));
            }
        }
        for (id, raster) in &cels {
            if raster.width != width || raster.height != height {
                return Err(SaveError::CelSize(*id, raster.width, raster.height));
            }
        }
        let next_layer = layers.iter().map(|l| l.id).max().unwrap_or(0) + 1;
        let next_cel = exposures
            .iter()
            .map(|e| e.cel_id)
            .chain(cels.keys().copied())
            .max()
            .unwrap_or(0)
            + 1;
        Ok(Self {
            width,
            height,
            fps,
            layers,
            exposures,
            cels,
            next_layer,
            next_cel,
        })
    }
}

/// Src-over `src` di atas `out` dengan opasitas layer `opacity` (0..=1) —
/// piksel RGBA8 tak-terali dihitung dalam 0..=255 per kanal. Area dilindungi
/// ukuran terkecil kedua raster (model tidak mewajibkan raster cel seukuran
/// kanvas di luar loader `.flip`).
fn composite_over(out: &mut Raster, src: &Raster, opacity: f32) {
    let op = (opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
    if op == 0 {
        return;
    }
    let w = src.width.min(out.width);
    let h = src.height.min(out.height);
    if w == 0 || h == 0 {
        return;
    }
    let rect = Rect { x: 0, y: 0, w, h };
    let mut dst_buf = out.region(rect);
    let src_buf = src.region(rect);
    for (d, s) in dst_buf
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(src_buf.as_chunks::<4>().0)
    {
        let sa = (u32::from(s[3]) * op) / 255;
        if sa == 0 {
            continue;
        }
        if sa == 255 {
            *d = *s;
            continue;
        }
        let inv = 255 - sa;
        let da = u32::from(d[3]);
        let merged = sa + (da * inv) / 255;
        if merged == 0 {
            *d = [0, 0, 0, 0];
            continue;
        }
        for c in 0..3 {
            let top = u32::from(s[c]) * sa + (u32::from(d[c]) * da * inv) / 255;
            d[c] = ((top + merged / 2) / merged) as u8;
        }
        d[3] = merged as u8;
    }
    out.write_region(rect, &dst_buf);
}

/// Alasan proyek tersimpan (`.flip`) ditolak saat dimuat — pesan jelas
/// sesuai §13.2/§13.3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// Ukuran kanvas di luar 1..=4096 px.
    Canvas(u32, u32),
    /// fps < 1.
    Fps(u32),
    /// Proyek tanpa layer.
    NoLayers,
    /// Jumlah layer melebihi [`MAX_LAYERS`].
    TooManyLayers(usize),
    /// ID layer kembar.
    DuplicateLayer(LayerId),
    /// Exposure tidak sah pada indeks (alasan singkat).
    Exposure(usize, &'static str),
    /// Exposure menunjuk layer yang tidak dikenal.
    UnknownLayer(LayerId),
    /// Exposure melewati batas [`MAX_FRAMES`] — angka = ujung (eksklusif).
    Frame(u32),
    /// Dua exposure satu layer tumpang-tindih — angka = `start` yang pertama.
    Overlap(LayerId, u32),
    /// Raster cel tidak seukuran kanvas — (id, lebar, tinggi raster).
    CelSize(CelId, u32, u32),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Canvas(w, h) => write!(
                f,
                "ukuran kanvas {w}×{h} di luar batas 1–{MAX_DIMENSION} px (§13.3)"
            ),
            Self::Fps(fps) => write!(f, "fps {fps} tidak sah (minimal 1)"),
            Self::NoLayers => write!(f, "proyek tanpa layer"),
            Self::TooManyLayers(n) => write!(f, "{n} layer melebihi batas {MAX_LAYERS} (§13.3)"),
            Self::DuplicateLayer(id) => write!(f, "ID layer kembar: {id}"),
            Self::Exposure(i, alasan) => write!(f, "exposure #{i} tidak sah: {alasan}"),
            Self::UnknownLayer(id) => write!(f, "exposure menunjuk layer tak dikenal: {id}"),
            Self::Frame(end) => write!(f, "exposure melewati batas {MAX_FRAMES} frame: {end}"),
            Self::Overlap(layer, start) => write!(
                f,
                "exposure tumpang-tindih di layer {layer} (mulai frame {start})"
            ),
            Self::CelSize(id, w, h) => write!(f, "cel {id} berukuran {w}×{h}, tidak sekanvas"),
        }
    }
}

impl std::error::Error for SaveError {}

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
    fn layer_mut_mengubah_tanpa_indeks() {
        let mut anim = Animation::new(8, 8);
        let id = anim.layer_id(0).unwrap();

        let layer = anim.layer_mut(id).expect("layer ada");
        layer.name = "Diganti".into();
        layer.visible = false;
        layer.opacity = 0.5;

        let layer = anim.layer_mut(id).unwrap();
        assert_eq!(layer.name, "Diganti");
        assert!(!layer.visible);
        assert_eq!(layer.opacity, 0.5);
        assert!(anim.layer_mut(999).is_none(), "id tak dikenal");
    }

    #[test]
    fn move_layer_menggeser_urutan_z() {
        let mut anim = Animation::new(8, 8);
        let a = anim.layer_id(0).unwrap();
        let b = anim.add_layer("B").unwrap();
        let c = anim.add_layer("C").unwrap();
        let urut = |anim: &Animation| {
            anim.layers()
                .iter()
                .map(|l| (l.id, l.name.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            urut(&anim),
            vec![
                (c, "C".to_owned()),
                (b, "B".to_owned()),
                (a, "Layer 1".to_owned())
            ],
            "posisi awal: C paling atas"
        );

        assert!(anim.move_layer(c, 2), "geser C ke dasar");
        assert_eq!(
            urut(&anim),
            vec![
                (b, "B".to_owned()),
                (a, "Layer 1".to_owned()),
                (c, "C".to_owned())
            ],
            "urutan Z berubah; ID ikut berpindah (exposure tetap sah)"
        );
        assert!(anim.move_layer(c, 0), "kembalikan C ke atas");
        assert_eq!(urut(&anim).first().map(|(id, _)| *id), Some(c));

        assert!(!anim.move_layer(c, 0), "sudah berada di tujuan");
        assert!(!anim.move_layer(c, 3), "tujuan di luar rentang");
        assert!(!anim.move_layer(999, 1), "id tak dikenal");
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

    // ------------------------------------------------------------------
    // Persistensi (`.flip` §13.2)
    // ------------------------------------------------------------------

    /// Layer sah untuk tes `from_saved`.
    fn layer(id: LayerId, name: &str) -> Layer {
        Layer {
            id,
            name: name.to_owned(),
            visible: true,
            opacity: 1.0,
            blend: Blend::default(),
        }
    }

    #[test]
    fn from_saved_valid_menghitung_ulang_id_berikutnya() {
        let layers = vec![layer(3, "Atas"), layer(1, DEFAULT_LAYER_NAME)];
        let exposures = vec![
            Exposure {
                layer: 1,
                start: 0,
                length: 3,
                cel_id: 7,
            },
            Exposure {
                layer: 3,
                start: 2,
                length: 1,
                cel_id: 9,
            },
        ];
        let mut cels = HashMap::new();
        cels.insert(9, Raster::new(8, 8));

        let mut anim = Animation::from_saved(8, 8, 24, layers, exposures, cels).expect("valid");
        assert_eq!(anim.fps, 24);
        assert_eq!(anim.frames(), 3);
        assert_eq!(anim.layer_id(0), Some(3), "urutan Z dipertahankan");
        assert_eq!(
            anim.cel_id_at(1, 1),
            Some(7),
            "exposure berbagi cel di seluruh rentang"
        );
        // ID berikutnya = maksimum + 1 (referensi lama tidak dipakai ulang).
        assert_eq!(anim.add_layer("Baru").unwrap(), 4);
        assert_eq!(anim.expose(1, 10), Some(10));
    }

    #[test]
    fn from_saved_menolak_input_tidak_sah() {
        let one_layer = || vec![layer(1, DEFAULT_LAYER_NAME)];
        let one_exposure = || {
            vec![Exposure {
                layer: 1,
                start: 0,
                length: 1,
                cel_id: 1,
            }]
        };
        let err = |r: Result<Animation, SaveError>| r.err();

        assert_eq!(
            err(Animation::from_saved(
                0,
                8,
                12,
                one_layer(),
                vec![],
                HashMap::new()
            )),
            Some(SaveError::Canvas(0, 8)),
            "kanvas minimal 1 px"
        );
        assert_eq!(
            err(Animation::from_saved(
                8,
                8,
                0,
                one_layer(),
                vec![],
                HashMap::new()
            )),
            Some(SaveError::Fps(0)),
            "fps minimal 1"
        );
        assert_eq!(
            err(Animation::from_saved(
                8,
                8,
                12,
                vec![],
                vec![],
                HashMap::new()
            )),
            Some(SaveError::NoLayers),
            "proyek wajib punya layer"
        );
        let too_many = (0..MAX_LAYERS + 1).map(|i| layer(i as u32, &format!("L{i}")));
        assert_eq!(
            err(Animation::from_saved(
                8,
                8,
                12,
                too_many.collect(),
                vec![],
                HashMap::new()
            )),
            Some(SaveError::TooManyLayers(MAX_LAYERS + 1)),
            "batas 16 layer §13.3"
        );
        assert_eq!(
            err(Animation::from_saved(
                8,
                8,
                12,
                vec![layer(1, "a"), layer(1, "b")],
                vec![],
                HashMap::new()
            )),
            Some(SaveError::DuplicateLayer(1)),
            "ID layer unik"
        );
        assert!(
            Animation::from_saved(8, 8, 12, one_layer(), one_exposure(), HashMap::new()).is_ok(),
            "cel tanpa raster = cel kosong, sah"
        );
        assert_eq!(
            err(Animation::from_saved(
                8,
                8,
                12,
                one_layer(),
                vec![Exposure {
                    layer: 99,
                    start: 0,
                    length: 1,
                    cel_id: 1
                }],
                HashMap::new()
            )),
            Some(SaveError::UnknownLayer(99)),
            "referensi layer harus ada"
        );
        assert_eq!(
            err(Animation::from_saved(
                8,
                8,
                12,
                one_layer(),
                vec![Exposure {
                    layer: 1,
                    start: 0,
                    length: 0,
                    cel_id: 1
                }],
                HashMap::new()
            )),
            Some(SaveError::Exposure(0, "panjang < 1")),
            "panjang exposure ≥ 1"
        );
        assert_eq!(
            err(Animation::from_saved(
                8,
                8,
                12,
                one_layer(),
                vec![Exposure {
                    layer: 1,
                    start: MAX_FRAMES - 1,
                    length: 2,
                    cel_id: 1
                }],
                HashMap::new()
            )),
            Some(SaveError::Frame(MAX_FRAMES + 1)),
            "batas 600 frame §13.3"
        );
        assert_eq!(
            err(Animation::from_saved(
                8,
                8,
                12,
                one_layer(),
                vec![
                    Exposure {
                        layer: 1,
                        start: 0,
                        length: 4,
                        cel_id: 1
                    },
                    Exposure {
                        layer: 1,
                        start: 3,
                        length: 1,
                        cel_id: 2
                    },
                ],
                HashMap::new()
            )),
            Some(SaveError::Overlap(1, 0)),
            "exposure satu layer tidak boleh tumpang-tindih"
        );
        let mut wrong_size = HashMap::new();
        wrong_size.insert(1, Raster::new(4, 4));
        assert_eq!(
            err(Animation::from_saved(
                8,
                8,
                12,
                one_layer(),
                vec![],
                wrong_size
            )),
            Some(SaveError::CelSize(1, 4, 4)),
            "raster cel harus seukuran kanvas"
        );
    }
}
