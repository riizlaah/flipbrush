//! Format `.flip` (DESIGN §13.2): container ZIP (deflate) berisi
//! `manifest.json` (metadata + struktur layer/exposure), PNG per cel unik
//! `cels/<id>.png` (exposure **berbagi** referensi — tidak diduplikasi),
//! preview `thumb/preview.png` (≤ 256 px, untuk daftar file terbaru), dan
//! `checksums.txt` (SHA-256 per file, format `sha256sum` — bisa dicek dengan
//! `sha256sum -c`).
//!
//! Modul ini **murni memori**: [`encode`] menghasilkan bytes dan [`decode`]
//! menerima bytes — I/O file (tulis aman `*.tmp` → rename atomik, autosave)
//! ada di lapisan aplikasi (`flipbrush-app::project`). Kompatibilitas maju:
//! field tak dikenal diabaikan; `format_version` lebih tinggi dari
//! [`FORMAT_VERSION`] ditolak dengan pesan jelas; semua file yang tercatat
//! di `checksums.txt` diverifikasi sebelum dipakai (file korup terdeteksi).

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::anim::{Animation, Blend, CelId, Exposure, Layer};
use crate::raster::{MAX_DIMENSION, Raster, Rect};

/// Versi format `.flip` — file dengan versi lebih tinggi ditolak (§13.2).
pub const FORMAT_VERSION: u32 = 1;
/// Entri manifest di container.
pub const MANIFEST_PATH: &str = "manifest.json";
/// Entri checksum — format `sha256sum`: `<hex>  <path>`.
pub const CHECKSUMS_PATH: &str = "checksums.txt";
/// Folder PNG cel unik — `cels/<id>.png`.
pub const CEL_DIR: &str = "cels";
/// Preview daftar file terbaru (§13.2).
pub const PREVIEW_PATH: &str = "thumb/preview.png";
/// Sisi terpanjang preview dalam piksel (§13.2).
pub const PREVIEW_MAX: u32 = 256;

/// Hasil [`decode`] — model plus metadata manifest yang dipertahankan untuk
/// simpan ulang.
#[derive(Clone)]
pub struct Project {
    /// Model animasi hasil validasi menyeluruh (§13.3).
    pub anim: Animation,
    /// Detik Unix pembuatan proyek (manifest `created`) — dipakai ulang pada
    /// simpan berikutnya.
    pub created_unix: u64,
    /// Versi aplikasi yang menyimpan file terakhir (debug/riwayat).
    pub app_version: String,
}

/// Kegagalan membaca/menulis container `.flip` — pesan jelas (§13.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlipError {
    /// `format_version` lebih tinggi dari [`FORMAT_VERSION`] — angka = versi file.
    UnsupportedVersion(u32),
    /// File bukan ZIP / rusak / entri gagal dibaca — alasan teknis.
    Zip(String),
    /// Entri wajib tidak ada — nama entri.
    Missing(String),
    /// `manifest.json` / `checksums.txt` tidak valid — alasan.
    Manifest(String),
    /// SHA-256 tidak cocok (file korup) — nama entri bermasalah.
    Checksum(String),
    /// PNG cel tidak sah — (id cel, alasan).
    Cel(CelId, String),
    /// PNG preview tidak sah — alasan.
    Preview(String),
    /// Model hasil dekode tidak valid (§13.3) — pesan [`crate::anim::SaveError`].
    Invalid(crate::anim::SaveError),
}

impl std::fmt::Display for FlipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion(v) => write!(
                f,
                "format .flip versi {v} lebih baru dari yang didukung ({FORMAT_VERSION}) — perbarui Flipbrush"
            ),
            Self::Zip(pesan) => write!(f, "container .flip rusak: {pesan}"),
            Self::Missing(nama) => write!(f, "entri wajib tidak ada: {nama}"),
            Self::Manifest(pesan) => write!(f, "manifest tidak valid: {pesan}"),
            Self::Checksum(nama) => write!(f, "checksum tidak cocok (file korup): {nama}"),
            Self::Cel(id, pesan) => write!(f, "cel {id} tidak sah: {pesan}"),
            Self::Preview(pesan) => write!(f, "preview tidak sah: {pesan}"),
            Self::Invalid(sebab) => write!(f, "proyek tidak valid: {sebab}"),
        }
    }
}

impl std::error::Error for FlipError {}

// ---------------------------------------------------------------------------
// Manifest (serde mengabaikan field tak dikenal secara bawaan — §13.2)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct Manifest {
    format_version: u32,
    app_version: String,
    canvas: CanvasM,
    fps: u32,
    layers: Vec<LayerM>,
    exposures: Vec<ExposureM>,
    cels: Vec<CelM>,
    created: String,
    modified: String,
}

#[derive(Serialize, Deserialize)]
struct CanvasM {
    width: u32,
    height: u32,
}

#[derive(Serialize, Deserialize)]
struct LayerM {
    id: u32,
    name: String,
    visible: bool,
    opacity: f32,
    blend: BlendM,
}

/// Mode blend di manifest — `"normal"`. Nilai tak dikenal dari versi masa
/// depan ditolak dengan jelas (file semestinya sudah ditolak lebih dulu oleh
/// `format_version`).
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum BlendM {
    Normal,
}

impl From<&Blend> for BlendM {
    fn from(_: &Blend) -> Self {
        Self::Normal
    }
}

impl From<BlendM> for Blend {
    fn from(_: BlendM) -> Self {
        Self::Normal
    }
}

#[derive(Serialize, Deserialize)]
struct ExposureM {
    layer: u32,
    start: u32,
    length: u32,
    cel_id: u32,
}

#[derive(Serialize, Deserialize)]
struct CelM {
    id: u32,
    path: String,
    bounds: BoundsM,
}

#[derive(Serialize, Deserialize)]
struct BoundsM {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

/// Probe minimal untuk menolak file lebih baru **sebelum** parse penuh —
/// manifest versi masa depan bisa berubah radikal tetapi pesan "perbarui
/// Flipbrush" tetap muncul (§13.2).
#[derive(Deserialize)]
struct VersionProbe {
    #[serde(default)]
    format_version: u32,
}

impl From<LayerM> for Layer {
    fn from(m: LayerM) -> Self {
        Self {
            id: m.id,
            name: m.name,
            visible: m.visible,
            opacity: m.opacity,
            blend: m.blend.into(),
        }
    }
}

impl From<ExposureM> for Exposure {
    fn from(m: ExposureM) -> Self {
        Self {
            layer: m.layer,
            start: m.start,
            length: m.length,
            cel_id: m.cel_id,
        }
    }
}

// ---------------------------------------------------------------------------
// Encode / decode
// ---------------------------------------------------------------------------

/// Encode proyek menjadi bytes container `.flip`.
///
/// Urutan tulis: PNG per cel unik → preview frame 0 → `manifest.json` →
/// `checksums.txt` (SHA-256 seluruh file lain, urutan path agar deterministik).
/// `created_unix`/`modified_unix` detik Unix; `app_version` dicatat di
/// manifest. Cel yang sedang berada di buffer editing **tidak ikut** — flush
/// dulu (`switch_cel(None)`) sebelum memanggil.
pub fn encode(
    anim: &Animation,
    created_unix: u64,
    modified_unix: u64,
    app_version: &str,
) -> Result<Vec<u8>, FlipError> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut hashes: Vec<(String, String)> = Vec::new();

    // 1. PNG per cel unik — exposure berbagi referensi (§13.2).
    let mut stored: Vec<(CelId, &Raster)> = anim.stored_cels().collect();
    stored.sort_unstable_by_key(|(id, _)| *id);
    let mut cel_entries = Vec::with_capacity(stored.len());
    for (id, raster) in stored {
        let path = format!("{CEL_DIR}/{id}.png");
        let bytes = png_bytes(raster).map_err(|pesan| FlipError::Cel(id, pesan))?;
        put(&mut writer, opts, &path, &bytes, &mut hashes)?;
        cel_entries.push(CelM {
            id,
            path,
            bounds: BoundsM {
                x: 0,
                y: 0,
                w: raster.width,
                h: raster.height,
            },
        });
    }

    // 2. Preview frame 0 (seluruh layer tampak) ≤ PREVIEW_MAX px.
    let preview =
        png_bytes(&fit(anim.flatten_frame(0), PREVIEW_MAX)).map_err(FlipError::Preview)?;
    put(&mut writer, opts, PREVIEW_PATH, &preview, &mut hashes)?;

    // 3. Manifest.
    let manifest = Manifest {
        format_version: FORMAT_VERSION,
        app_version: app_version.to_owned(),
        canvas: CanvasM {
            width: anim.width,
            height: anim.height,
        },
        fps: anim.fps,
        layers: anim
            .layers()
            .iter()
            .map(|l| LayerM {
                id: l.id,
                name: l.name.clone(),
                visible: l.visible,
                opacity: l.opacity,
                blend: BlendM::from(&l.blend),
            })
            .collect(),
        exposures: anim
            .exposures()
            .iter()
            .map(|e| ExposureM {
                layer: e.layer,
                start: e.start,
                length: e.length,
                cel_id: e.cel_id,
            })
            .collect(),
        cels: cel_entries,
        created: rfc3339(created_unix),
        modified: rfc3339(modified_unix),
    };
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|e| FlipError::Manifest(e.to_string()))?;
    put(
        &mut writer,
        opts,
        MANIFEST_PATH,
        &manifest_bytes,
        &mut hashes,
    )?;

    // 4. checksums.txt — format sha256sum, path terurut agar deterministik.
    hashes.sort_by(|a, b| a.1.cmp(&b.1));
    let mut sums = String::new();
    for (hex, path) in &hashes {
        sums.push_str(&format!("{hex}  {path}\n"));
    }
    writer
        .start_file(CHECKSUMS_PATH, opts)
        .map_err(|e| FlipError::Zip(e.to_string()))?;
    writer
        .write_all(sums.as_bytes())
        .map_err(|e| FlipError::Zip(e.to_string()))?;

    let cursor = writer.finish().map_err(|e| FlipError::Zip(e.to_string()))?;
    Ok(cursor.into_inner())
}

/// Decode container `.flip` — verifikasi SHA-256 semua file tercatat, tolak
/// `format_version` lebih tinggi, abaikan field tak dikenal, lalu validasi
/// model menyeluruh via [`Animation::from_saved`] (§13.2, §13.3).
pub fn decode(bytes: &[u8]) -> Result<Project, FlipError> {
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|e| FlipError::Zip(e.to_string()))?;

    // 1. Verifikasi checksum — file korup ketahuan sebelum dipakai.
    for (hex, path) in parse_checksums(&read_entry(&mut archive, CHECKSUMS_PATH)?)? {
        let data = read_entry(&mut archive, &path)?;
        let digest = format!("{:x}", Sha256::digest(&data));
        if !digest.eq_ignore_ascii_case(&hex) {
            return Err(FlipError::Checksum(path));
        }
    }

    // 2. Manifest — probe versi dulu agar pesan "perbarui" selalu jelas.
    let manifest_bytes = read_entry(&mut archive, MANIFEST_PATH)?;
    let probe: VersionProbe =
        serde_json::from_slice(&manifest_bytes).map_err(|e| FlipError::Manifest(e.to_string()))?;
    if probe.format_version > FORMAT_VERSION {
        return Err(FlipError::UnsupportedVersion(probe.format_version));
    }
    let manifest: Manifest =
        serde_json::from_slice(&manifest_bytes).map_err(|e| FlipError::Manifest(e.to_string()))?;

    // 3. PNG cel → raster (RGBA8; ukuran divalidasi `from_saved`).
    let mut cels = HashMap::with_capacity(manifest.cels.len());
    for cel in &manifest.cels {
        let data = read_entry(&mut archive, &cel.path)?;
        cels.insert(cel.id, decode_png(&data, cel.id)?);
    }

    // 4. Model + validasi menyeluruh (§13.3).
    let anim = Animation::from_saved(
        manifest.canvas.width,
        manifest.canvas.height,
        manifest.fps,
        manifest.layers.into_iter().map(Layer::from).collect(),
        manifest.exposures.into_iter().map(Exposure::from).collect(),
        cels,
    )
    .map_err(FlipError::Invalid)?;

    Ok(Project {
        anim,
        created_unix: parse_rfc3339(&manifest.created)?,
        app_version: manifest.app_version,
    })
}

// ---------------------------------------------------------------------------
// Helper container / PNG
// ---------------------------------------------------------------------------

/// Tulis satu entri + catat SHA-256-nya untuk `checksums.txt`.
fn put(
    writer: &mut ZipWriter<Cursor<Vec<u8>>>,
    opts: SimpleFileOptions,
    path: &str,
    bytes: &[u8],
    hashes: &mut Vec<(String, String)>,
) -> Result<(), FlipError> {
    writer
        .start_file(path, opts)
        .map_err(|e| FlipError::Zip(e.to_string()))?;
    writer
        .write_all(bytes)
        .map_err(|e| FlipError::Zip(e.to_string()))?;
    hashes.push((format!("{:x}", Sha256::digest(bytes)), path.to_owned()));
    Ok(())
}

/// Baca satu entri container — entri hilang → [`FlipError::Missing`].
fn read_entry(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>, FlipError> {
    let mut file = archive.by_name(name).map_err(|e| match e {
        zip::result::ZipError::FileNotFound => FlipError::Missing(name.to_owned()),
        lain => FlipError::Zip(lain.to_string()),
    })?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| FlipError::Zip(e.to_string()))?;
    Ok(buf)
}

/// Parse `checksums.txt`: tiap baris `<hex-sha256>  <path>` (format
/// `sha256sum`); baris kosong dilewati, selain itu harus valid.
fn parse_checksums(bytes: &[u8]) -> Result<Vec<(String, String)>, FlipError> {
    let teks = std::str::from_utf8(bytes)
        .map_err(|e| FlipError::Manifest(format!("checksums.txt bukan UTF-8: {e}")))?;
    let mut out = Vec::new();
    for (i, baris) in teks.lines().enumerate() {
        if baris.trim().is_empty() {
            continue;
        }
        let mut bagian = baris.split_whitespace();
        let (Some(hex), Some(path)) = (bagian.next(), bagian.next()) else {
            return Err(FlipError::Manifest(format!(
                "checksums.txt baris {} tidak valid",
                i + 1
            )));
        };
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(FlipError::Manifest(format!(
                "checksums.txt baris {}: SHA-256 tidak sah",
                i + 1
            )));
        }
        out.push((hex.to_owned(), path.to_owned()));
    }
    Ok(out)
}

/// Encode raster → PNG RGBA8 in-memory. Encoder menulis ke `Vec` — untuk
/// dimensi yang dijamin `Raster` (1..=4096 §13.3) header tak pernah gagal.
fn png_bytes(raster: &Raster) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut buf, raster.width, raster.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        let full = Rect {
            x: 0,
            y: 0,
            w: raster.width,
            h: raster.height,
        };
        writer
            .write_image_data(&raster.region(full))
            .map_err(|e| e.to_string())?;
    }
    Ok(buf)
}

/// Decode PNG RGBA8 → `Raster`; menolak warna/dalam lain dan ukuran di luar
/// v1 (§13.3) — tanpa panic.
fn decode_png(bytes: &[u8], id: CelId) -> Result<Raster, FlipError> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|e| FlipError::Cel(id, e.to_string()))?;
    let mut buf = vec![
        0;
        reader.output_buffer_size().ok_or_else(|| FlipError::Cel(
            id,
            "ukuran buffer PNG tidak diketahui".to_owned()
        ))?
    ];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| FlipError::Cel(id, e.to_string()))?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(FlipError::Cel(id, "bukan PNG RGBA8".to_owned()));
    }
    if !(1..=MAX_DIMENSION).contains(&info.width) || !(1..=MAX_DIMENSION).contains(&info.height) {
        return Err(FlipError::Cel(
            id,
            "ukuran di luar batas v1 (§13.3)".to_owned(),
        ));
    }
    let expected = info.width as usize * info.height as usize * 4;
    if info.line_size != info.width as usize * 4 || buf.len() < expected {
        return Err(FlipError::Cel(id, "baris PNG tidak padat RGBA8".to_owned()));
    }
    let mut raster = Raster::new(info.width, info.height);
    raster.write_region(
        Rect {
            x: 0,
            y: 0,
            w: info.width,
            h: info.height,
        },
        &buf[..expected],
    );
    Ok(raster)
}

/// Skala turun nearest-neighbor agar sisi terpanjang ≤ `max`; tidak pernah
/// memperbesar — preview daftar file terbaru (§13.2).
fn fit(src: Raster, max: u32) -> Raster {
    let terpanjang = src.width.max(src.height);
    if terpanjang <= max {
        return src;
    }
    let dw = (src.width * max / terpanjang).max(1);
    let dh = (src.height * max / terpanjang).max(1);
    let src_buf = src.region(Rect {
        x: 0,
        y: 0,
        w: src.width,
        h: src.height,
    });
    let mut out_buf = Vec::with_capacity(dw as usize * dh as usize * 4);
    for y in 0..dh {
        let sy = y * src.height / dh;
        for x in 0..dw {
            let i = ((sy * src.width + x * src.width / dw) * 4) as usize;
            out_buf.extend_from_slice(&src_buf[i..i + 4]);
        }
    }
    let mut out = Raster::new(dw, dh);
    out.write_region(
        Rect {
            x: 0,
            y: 0,
            w: dw,
            h: dh,
        },
        &out_buf,
    );
    out
}

// ---------------------------------------------------------------------------
// Stempel waktu — RFC 3339 UTC tanpa dependensi (manifest `created`/`modified`)
// ---------------------------------------------------------------------------

/// Detik Unix → `YYYY-MM-DDTHH:MM:SSZ` (UTC, algoritma sipil Hinnant).
fn rfc3339(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let detik = unix % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        detik / 3600,
        detik % 3600 / 60,
        detik % 60
    )
}

/// [`rfc3339`] sebaliknya — format persis 20 karakter; nilai di luar rentang
/// kalender (bulan 13, jam 25, …) ditolak.
fn parse_rfc3339(teks: &str) -> Result<u64, FlipError> {
    let tidak_sah = || FlipError::Manifest(format!("stempel waktu tidak sah: {teks}"));
    let b = teks.as_bytes();
    if b.len() != 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'Z'
    {
        return Err(tidak_sah());
    }
    let angka = |r: std::ops::Range<usize>| -> Result<u64, FlipError> {
        teks[r].parse().map_err(|_| tidak_sah())
    };
    let (y, mo, d) = (angka(0..4)?, angka(5..7)?, angka(8..10)?);
    let (h, mi, s) = (angka(11..13)?, angka(14..16)?, angka(17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 59 {
        return Err(tidak_sah());
    }
    Ok(days_from_civil(y as i64, mo, d) as u64 * 86_400 + h * 3600 + mi * 60 + s)
}

/// Hari sejak 1970-01-01 dari tahun/bulan/tanggal sipil (Hinnant).
fn days_from_civil(y: i64, m: u64, d: u64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (if m > 2 { m - 3 } else { m + 9 }) as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Tahun/bulan/tanggal sipil dari hari sejak 1970-01-01 (Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Proyek contoh: 2 layer, cel dibagi antar-exposure, satu exposure
    /// tanpa raster tersimpan (masih "di buffer"), piksel khas per cel.
    fn sample() -> Animation {
        let mut anim = Animation::new(32, 24);
        let base = anim.layer_id(0).unwrap();
        let a = anim.expose(base, 0).unwrap();
        anim.expose(base, 3).unwrap();
        anim.expose(base, 5).unwrap(); // cel ini tidak pernah disimpan
        let atas = anim.add_layer("Garis").unwrap();
        let d = anim.expose(atas, 1).unwrap();
        anim.put_cel(a, filled(32, 24, [200, 50, 30, 255]));
        anim.put_cel(d, filled(32, 24, [20, 90, 220, 255]));
        anim
    }

    /// Raster solid ukuran `w × h`.
    fn filled(w: u32, h: u32, rgba: [u8; 4]) -> Raster {
        let mut r = Raster::new(w, h);
        let mut buf = vec![0u8; (w * h * 4) as usize];
        for px in buf.as_chunks_mut::<4>().0 {
            *px = rgba;
        }
        r.write_region(Rect { x: 0, y: 0, w, h }, &buf);
        r
    }

    /// [`decode`] yang mengembalikan saja errornya — `Project` tidak `Debug`.
    fn decode_err(bytes: &[u8]) -> FlipError {
        decode(bytes).map(|_| ()).unwrap_err()
    }

    /// Baca semua entri container tanpa verifikasi — untuk tes manipulasi.
    fn entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("zip valid");
        let names: Vec<String> = archive.file_names().map(str::to_owned).collect();
        names
            .into_iter()
            .map(|name| {
                let mut file = archive.by_name(&name).expect("entri ada");
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).expect("baca entri");
                (name, buf)
            })
            .collect()
    }

    /// Tulis entri-entri balik apa adanya (checksums **tidak** dihitung ulang
    /// — dipakai tes yang sengaja merusak/menyunting isinya).
    fn rebuild(list: &[(String, Vec<u8>)]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, bytes) in list {
            writer.start_file(name.as_str(), opts).expect("tulis entri");
            writer.write_all(bytes).expect("isi entri");
        }
        writer.finish().expect("selesai").into_inner()
    }

    /// Hitung ulang `checksums.txt` dari seluruh entri lain (path terurut).
    fn refresh_checksums(list: &mut Vec<(String, Vec<u8>)>) {
        let mut names: Vec<String> = list
            .iter()
            .filter(|(n, _)| n != CHECKSUMS_PATH)
            .map(|(n, _)| n.clone())
            .collect();
        names.sort();
        let mut sums = String::new();
        for name in &names {
            let data = &list.iter().find(|(n, _)| n == name).expect("ada").1;
            sums.push_str(&format!("{:x}  {name}\n", Sha256::digest(data)));
        }
        if let Some(pos) = list.iter().position(|(n, _)| n == CHECKSUMS_PATH) {
            list[pos].1 = sums.into_bytes();
        } else {
            list.push((CHECKSUMS_PATH.to_owned(), sums.into_bytes()));
        }
    }

    #[test]
    fn roundtrip_memertahankan_model_dan_metadata() {
        let anim = sample();
        let penuh = Rect {
            x: 0,
            y: 0,
            w: 32,
            h: 24,
        };
        let frames_sebelum = anim.frames();
        let cel_sebelum = anim.stored_cel_count();

        let bytes = encode(&anim, 1_700_000_000, 1_760_000_123, "0.1.0-tes").expect("encode");
        let project = decode(&bytes).expect("decode");
        let got = &project.anim;

        assert_eq!(project.created_unix, 1_700_000_000);
        assert_eq!(project.app_version, "0.1.0-tes");
        assert_eq!(got.width, 32);
        assert_eq!(got.height, 24);
        assert_eq!(got.fps, anim.fps);
        assert_eq!(got.frames(), frames_sebelum);

        // Layer: urutan Z, nama, visibilitas, opasitas.
        assert_eq!(got.layers().len(), anim.layers().len());
        for (a, b) in got.layers().iter().zip(anim.layers()) {
            assert_eq!(a.id, b.id, "ID layer");
            assert_eq!(a.name, b.name, "nama layer");
            assert_eq!(a.visible, b.visible, "visibilitas layer");
            assert_eq!(a.opacity, b.opacity, "opasitas layer");
        }

        // Exposure identik — cel unik dibagikan, tidak diduplikasi.
        assert_eq!(got.exposures().len(), anim.exposures().len());
        for (a, b) in got.exposures().iter().zip(anim.exposures()) {
            assert_eq!(
                (a.layer, a.start, a.length, a.cel_id),
                (b.layer, b.start, b.length, b.cel_id),
                "exposure"
            );
        }

        // Piksel cel utuh + jumlah cel unik sama (cel "di buffer" tak ikut).
        assert_eq!(got.stored_cel_count(), cel_sebelum);
        for (id, raster) in anim.stored_cels() {
            assert_eq!(
                got.cel(id).expect("cel ikut tersimpan").region(penuh),
                raster.region(penuh),
                "piksel cel {id}"
            );
        }

        // Exposure tanpa raster tetap ada — cel kosong, bukan hilang.
        let base = got.layer_id(1).expect("layer bawah");
        let id_kosong = got.cel_id_at(base, 5).expect("exposure ke-3");
        assert!(got.cel(id_kosong).is_none(), "cel buffer = kosong");
    }

    #[test]
    fn proyek_tanpa_cel_tersimpan_roundtrip() {
        let mut anim = Animation::new(16, 16);
        let base = anim.layer_id(0).unwrap();
        anim.expose(base, 0).unwrap();
        let bytes = encode(&anim, 0, 0, "t").expect("encode");
        let project = decode(&bytes).expect("decode semua cel di buffer");
        assert_eq!(project.anim.stored_cel_count(), 0);
        assert_eq!(project.anim.exposures().len(), 1);
    }

    #[test]
    fn container_punya_enti_wajib_dan_cel_unik_saja() {
        let anim = sample();
        let bytes = encode(&anim, 0, 0, "t").expect("encode");
        let list = entries(&bytes);
        let nama: Vec<&str> = list.iter().map(|(n, _)| n.as_str()).collect();
        assert!(nama.contains(&MANIFEST_PATH), "manifest");
        assert!(nama.contains(&CHECKSUMS_PATH), "checksums");
        assert!(nama.contains(&PREVIEW_PATH), "preview");
        let cels: Vec<&str> = nama
            .iter()
            .copied()
            .filter(|n| n.starts_with(CEL_DIR))
            .collect();
        assert_eq!(
            cels.len(),
            anim.stored_cel_count(),
            "satu PNG per cel unik — bukan per exposure"
        );
    }

    #[test]
    fn checksums_memakai_format_sha256sum() {
        let bytes = encode(&sample(), 0, 0, "t").expect("encode");
        let list = entries(&bytes);
        let sums = &list
            .iter()
            .find(|(n, _)| n == CHECKSUMS_PATH)
            .expect("checksums ada")
            .1;
        let teks = std::str::from_utf8(sums).expect("UTF-8");
        let baris: Vec<&str> = teks.lines().collect();
        assert!(!baris.is_empty());
        for baris in baris {
            let (hex, path) = baris.split_once("  ").expect("`<hex>  <path>`");
            assert_eq!(hex.len(), 64, "SHA-256 64 hex");
            assert!(hex.bytes().all(|b| b.is_ascii_hexdigit()), "hex murni");
            assert!(!path.is_empty(), "path tidak kosong");
        }
    }

    #[test]
    fn format_version_lebih_tinggi_ditolak_pesan_jelas() {
        let mut list = entries(&encode(&sample(), 0, 0, "t").expect("encode"));
        let manifest = list
            .iter_mut()
            .find(|(n, _)| n == MANIFEST_PATH)
            .expect("manifest ada");
        let mut v: serde_json::Value = serde_json::from_slice(&manifest.1).expect("json");
        v["format_version"] = serde_json::json!(FORMAT_VERSION + 1);
        manifest.1 = serde_json::to_vec(&v).expect("to_vec");
        refresh_checksums(&mut list);

        let err = decode_err(&rebuild(&list));
        assert_eq!(err, FlipError::UnsupportedVersion(FORMAT_VERSION + 1));
        assert!(
            err.to_string().contains("perbarui Flipbrush"),
            "pesan jelas: {}",
            err
        );
    }

    #[test]
    fn field_manifest_tak_dikenal_diabaikan() {
        let mut list = entries(&encode(&sample(), 0, 0, "t").expect("encode"));
        let manifest = list
            .iter_mut()
            .find(|(n, _)| n == MANIFEST_PATH)
            .expect("manifest ada");
        let mut v: serde_json::Value = serde_json::from_slice(&manifest.1).expect("json");
        v["filter_masa_depan"] = serde_json::json!({ "aktif": true });
        manifest.1 = serde_json::to_vec(&v).expect("to_vec");
        refresh_checksums(&mut list);

        let project = decode(&rebuild(&list)).expect("field tak dikenal diabaikan");
        assert_eq!(project.anim.fps, sample().fps);
        assert_eq!(project.anim.stored_cel_count(), sample().stored_cel_count());
    }

    #[test]
    fn checksum_korup_ditolak_sebelum_dipakai() {
        let mut list = entries(&encode(&sample(), 0, 0, "t").expect("encode"));
        refresh_checksums(&mut list);
        // Ubah satu digit hex pertama pada checksums.txt (file itu sendiri
        // tidak di-hash — kesalahan terdeteksi saat memverifikasi file lain).
        let sums = list
            .iter_mut()
            .find(|(n, _)| n == CHECKSUMS_PATH)
            .expect("checksums ada");
        let pos = sums
            .1
            .iter()
            .position(|b| b.is_ascii_hexdigit())
            .expect("hex");
        sums.1[pos] = if sums.1[pos] == b'0' { b'1' } else { b'0' };

        let err = decode_err(&rebuild(&list));
        assert!(
            matches!(err, FlipError::Checksum(_)),
            "seharusnya Checksum, dapat: {err}"
        );
    }

    #[test]
    fn tanpa_checksums_atau_bukan_zip_ditolak() {
        let mut list = entries(&encode(&sample(), 0, 0, "t").expect("encode"));
        list.retain(|(n, _)| n != CHECKSUMS_PATH);
        let err = decode_err(&rebuild(&list));
        assert_eq!(err, FlipError::Missing(CHECKSUMS_PATH.to_owned()));

        let err = decode_err(b"bukan zip sama sekali");
        assert!(matches!(err, FlipError::Zip(_)), "bukan ZIP: {err}");
    }

    #[test]
    fn preview_ditulis_dan_diskalakan_maks_256() {
        // Kanvas kecil: tidak diperbesar, sisi asli dipertahankan.
        let kecil = entries(&encode(&sample(), 0, 0, "t").expect("encode"));
        let pv = &kecil
            .iter()
            .find(|(n, _)| n == PREVIEW_PATH)
            .expect("preview ada")
            .1;
        assert_eq!(ihdr(pv), (32, 24), "preview = ukuran asli bila ≤ 256");

        // Kanvas besar: sisi terpanjang tepat 256, rasio dipertahankan.
        let besar = encode(&Animation::new(1080, 540), 0, 0, "t").expect("encode");
        let pv = entries(&besar)
            .into_iter()
            .find(|(n, _)| n == PREVIEW_PATH)
            .expect("preview ada")
            .1;
        assert_eq!(ihdr(&pv), (256, 128), "rasio 2:1 dipertahankan");
    }

    /// Baca (width, height) dari header IHDR PNG (offset tetap §9.3).
    fn ihdr(png: &[u8]) -> (u32, u32) {
        let w = u32::from_be_bytes(png[16..20].try_into().expect("width"));
        let h = u32::from_be_bytes(png[20..24].try_into().expect("height"));
        (w, h)
    }

    #[test]
    fn stempel_waktu_rfc3339_bulat_sintetis() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(86_400), "1970-01-02T00:00:00Z");
        assert_eq!(rfc3339(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(rfc3339(2_000_000_000), "2033-05-18T03:33:20Z");
        for unix in [
            0,
            1,
            59,
            86_399,
            86_400,
            1_700_000_000,
            1_760_000_123,
            2_000_000_000,
        ] {
            let teks = rfc3339(unix);
            assert_eq!(
                parse_rfc3339(&teks).expect("format sendiri valid"),
                unix,
                "roundtrip {teks}"
            );
        }
        for tidak_sah in [
            "bukan-timestamp",
            "1970-01-01 00:00:00Z",
            "1970-13-01T00:00:00Z",
            "1970-01-01T24:00:00Z",
        ] {
            assert!(
                parse_rfc3339(tidak_sah).is_err(),
                "harus ditolak: {tidak_sah}"
            );
        }
    }
}
