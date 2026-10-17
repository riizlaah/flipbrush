//! Persistensi `.flip` & autosave (DESIGN §13.2) — satu-satunya lapisan
//! aplikasi yang menyentuh disk.
//!
//! Encode/decode container ada di `flipbrush_core::flip` (murni memori);
//! modul ini menangani direktori data (XDG), tulis aman `*.tmp` → rename
//! atomik, thread autosave background (debounce **30 detik setelah
//! perubahan terakhir**), dan deteksi autosave pemulihan saat startup.
//! Snapshot dikirim lewat kanal lalu di-encode + ditulis di thread sendiri —
//! UI tidak pernah menunggu I/O autosave (§13.3: < 500 ms, background).

use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flipbrush_core::anim::Animation;
use flipbrush_core::flip;

use crate::APP_NAME;

/// Debounce autosave §13.2: tulis **setelah** perubahan terakhir tenang 30
/// detik — bukan interval tetap — agar tak memaksa I/O saat menggambar.
pub const AUTOSAVE_DELAY: Duration = Duration::from_secs(30);
/// File proyek sesi (belum ada dialog simpan/buka — M5); autosave memakai
/// pola `<nama>.flip.autosave` §13.2.
const SESSION_FILE: &str = "Tanpa Judul.flip";

/// Direktori data aplikasi — `$XDG_DATA_HOME` atau `~/.local/share`
/// (Linux desktop §13.2; jalur Android menyusul M6).
pub fn data_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(xdg).join(APP_NAME);
    }
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join(".local/share")
        .join(APP_NAME)
}

/// Path autosave sesi di `dir` — `<nama>.flip.autosave` (§13.2).
pub fn autosave_path(dir: &Path) -> PathBuf {
    dir.join(format!("{SESSION_FILE}.autosave"))
}

/// Detik Unix sekarang — stempel `created`/`modified` manifest (§13.2).
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Tulis aman §13.2: buang dulu ke `*.tmp` lalu rename atomik — pembaca
/// tak pernah melihat file setengah jadi. Folder orang tua dibuat bila perlu.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Autosave layak dipulihkan bila ada isi & lebih baru dari file utama (atau
/// file utama belum ada) — `Some(path)` untuk dialog pemulihan §13.2.
pub fn find_recovery(main: Option<&Path>, autosave: &Path) -> Option<PathBuf> {
    let meta = std::fs::metadata(autosave).ok()?;
    if meta.len() == 0 {
        return None;
    }
    let autosave_time = meta.modified().ok()?;
    let main_time = main
        .and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok());
    match main_time {
        Some(waktu) if autosave_time <= waktu => None,
        _ => Some(autosave.to_owned()),
    }
}

// ---------------------------------------------------------------------------
// Thread autosave background
// ---------------------------------------------------------------------------

/// Pekerjaan antrean autosave — FIFO, termasuk permintaan hapus agar file
/// tak "hidup lagi" setelah simpan manual sukses (§13.2).
enum Job {
    /// Encode snapshot lalu tulis `path`.
    Simpan {
        anim: Animation,
        path: PathBuf,
        created_unix: u64,
        /// Kanal ack tes — dikirim setelah tulis selesai.
        ack: Option<SyncSender<()>>,
    },
    /// Hapus file — dieksekusi setelah semua simpan di depannya selesai.
    Hapus {
        path: PathBuf,
        ack: Option<SyncSender<()>>,
    },
    /// Penanda FIFO: ack dikirim setelah semua pekerjaan di depannya tuntas.
    Ping(SyncSender<()>),
}

/// Autosave background §13.2 — snapshot `Animation` (bukan referensi: model
/// bisa berubah lagi saat thread bekerja) dikirim lewat kanal, di-encode +
/// ditulis di thread sendiri. `submit` asinkron; varian `*_sync` dipakai tes
/// (dan `remove_sync` saat simpan manual) — menunggu lewat ack FIFO.
pub struct Autosave {
    tx: Option<Sender<Job>>,
    handle: Option<JoinHandle<()>>,
}

impl Autosave {
    /// Spawn thread worker (nama memudahkan pelacakan di debug).
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<Job>();
        let handle = std::thread::Builder::new()
            .name("flipbrush-autosave".to_owned())
            .spawn(move || worker(rx))
            .expect("thread autosave ter-spawn");
        Self {
            tx: Some(tx),
            handle: Some(handle),
        }
    }

    /// Kirim snapshot untuk ditulis — asinkron, pemanggil tak menunggu.
    pub fn submit(&self, anim: Animation, path: PathBuf, created_unix: u64) {
        self.send(Job::Simpan {
            anim,
            path,
            created_unix,
            ack: None,
        });
    }

    /// [`Self::submit`] lalu tunggu tulisnya selesai (FIFO) — untuk tes.
    pub fn submit_sync(&self, anim: Animation, path: PathBuf, created_unix: u64) {
        let (ack, tunggu) = mpsc::sync_channel(0);
        self.send(Job::Simpan {
            anim,
            path,
            created_unix,
            ack: Some(ack),
        });
        let _ = tunggu.recv();
    }

    /// Hapus file — menunggu seluruh simpan yang masih antri selesai lebih
    /// dulu, sehingga autosave dijamin raip setelah ini kembali.
    pub fn remove_sync(&self, path: PathBuf) {
        let (ack, tunggu) = mpsc::sync_channel(0);
        self.send(Job::Hapus {
            path,
            ack: Some(ack),
        });
        let _ = tunggu.recv();
    }

    /// Tunggu seluruh pekerjaan yang sudah terkirim selesai (FIFO) — untuk
    /// tes & keluar bersih tanpa tulisan tertinggal.
    pub fn wait_idle(&self) {
        let (ack, tunggu) = mpsc::sync_channel(0);
        self.send(Job::Ping(ack));
        let _ = tunggu.recv();
    }

    fn send(&self, job: Job) {
        // Kanal tertutup hanya saat `Autosave` di-drop — worker selalu hidup.
        if let Some(tx) = &self.tx {
            let _ = tx.send(job);
        }
    }
}

impl Drop for Autosave {
    fn drop(&mut self) {
        // Tutup kanal → worker merampungkan antrian lalu keluar; join agar
        // tidak ada tulisan yang tertinggal setelah aplikasi "selesai".
        drop(self.tx.take());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Loop worker autosave — kegagalan dicatat ke log (toast §7.8 menyusul M5).
fn worker(rx: Receiver<Job>) {
    for job in rx {
        match job {
            Job::Simpan {
                anim,
                path,
                created_unix,
                ack,
            } => {
                let hasil =
                    flip::encode(&anim, created_unix, unix_now(), env!("CARGO_PKG_VERSION"))
                        .map_err(|e| e.to_string())
                        .and_then(|bytes| atomic_write(&path, &bytes).map_err(|e| e.to_string()));
                if let Err(pesan) = hasil {
                    log::warn!("autosave {} gagal: {pesan}", path.display());
                }
                if let Some(ack) = ack {
                    let _ = ack.send(());
                }
            }
            Job::Hapus { path, ack } => {
                if let Err(e) = std::fs::remove_file(&path)
                    && e.kind() != io::ErrorKind::NotFound
                {
                    log::warn!("gagal menghapus autosave {}: {e}", path.display());
                }
                if let Some(ack) = ack {
                    let _ = ack.send(());
                }
            }
            Job::Ping(ack) => {
                let _ = ack.send(());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// State persistensi aplikasi
// ---------------------------------------------------------------------------

/// State persistensi aplikasi (§13.2): file utama, jadwal autosave, dan
/// autosave pemulihan yang menunggu keputusan. Dirinya sendiri tidak
/// melakukan decode — aplikasi memanggil `flipbrush_core::flip` + metode
/// `note_*` di sini untuk buku catatan.
pub struct State {
    /// Direktori data (produksi [`data_dir`]; tes menyuntikkan temp dir).
    dir: PathBuf,
    /// File `.flip` utama — `None` selama belum ada simpan/buka (M5).
    path: Option<PathBuf>,
    /// Detik Unix pembuatan proyek — dipertahankan lintas simpan.
    created_unix: u64,
    /// Perubahan konten terakhir — `None` = tak ada yang tertunda.
    changed_since: Option<Instant>,
    /// Delay debounce (produksi [`AUTOSAVE_DELAY`]; dikecilkan di tes).
    delay: Duration,
    /// Thread autosave background.
    autosave: Autosave,
    /// Autosave lebih baru dari file utama — menunggu dialog §13.2.
    recovery: Option<PathBuf>,
}

impl State {
    /// State baru: deteksi autosave pemulihan (§13.2) + spawn thread.
    /// Folder data belum dibuat — dibuat saat tulis pertama.
    pub fn new(dir: PathBuf, delay: Duration) -> Self {
        let autosave = autosave_path(&dir);
        let recovery = find_recovery(None, &autosave);
        Self {
            dir,
            path: None,
            created_unix: unix_now(),
            changed_since: None,
            delay,
            autosave: Autosave::spawn(),
            recovery,
        }
    }

    /// Tandai konten berubah — jadwal autosave dihitung dari perubahan
    /// **terakhir** (debounce §13.2).
    pub fn mark_changed(&mut self) {
        self.changed_since = Some(Instant::now());
    }

    /// `true` bila debounce tercapai — pemanggil mengirim snapshot via
    /// [`Self::autosave_send`].
    pub fn autosave_due(&self) -> bool {
        self.autosave_in().is_some_and(|sisa| sisa.is_zero())
    }

    /// Sisa waktu menuju autosave berikutnya — `None` bila tak ada perubahan
    /// tertunda, `Some(0)` bila sudah waktunya. Dipakai UI untuk
    /// `request_repaint_after` — membangunkan UI event-driven (tanpa poll
    /// kontinu §6) tepat saat debounce tercapai.
    pub fn autosave_in(&self) -> Option<Duration> {
        self.changed_since
            .map(|sejak| self.delay.saturating_sub(sejak.elapsed()))
    }

    /// Kirim snapshot (isi model utuh — cel buffer sudah di-flush pemanggil)
    /// ke thread autosave; jadwal dikosongkan sampai perubahan berikutnya.
    pub fn autosave_send(&mut self, snapshot: Animation) {
        let path = autosave_path(&self.dir);
        self.autosave.submit(snapshot, path, self.created_unix);
        self.changed_since = None;
    }

    /// Path autosave sesi.
    pub fn autosave_file(&self) -> PathBuf {
        autosave_path(&self.dir)
    }

    /// Detik Unix pembuatan — untuk `encode` saat simpan manual.
    pub fn created_unix(&self) -> u64 {
        self.created_unix
    }

    /// File `.flip` utama — `None` selama belum pernah simpan/buka (M5).
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Setelah muat proyek (buka/pulih): `created` dari manifest, `path` file
    /// utama (bila disimpan manual), jadwal autosave dikosongkan.
    pub fn adopt(&mut self, created_unix: u64, path: Option<PathBuf>) {
        self.created_unix = created_unix;
        self.path = path;
        self.changed_since = None;
    }

    /// Setelah simpan manual sukses: jadwal kosong + autosave **dihapus**
    /// (§13.2) — menunggu simpan yang masih antri lewat FIFO.
    pub fn note_saved(&mut self, path: PathBuf) {
        self.adopt(self.created_unix, Some(path));
        self.autosave.remove_sync(autosave_path(&self.dir));
    }

    /// Path autosave pemulihan yang menunggu keputusan (§13.2).
    pub fn recovery_path(&self) -> Option<PathBuf> {
        self.recovery.clone()
    }

    /// Ambil keputusan atas dialog pemulihan — memori dialog dikosongkan
    /// (pemanggil menentukan nasib filenya).
    pub fn take_recovery(&mut self) -> Option<PathBuf> {
        self.recovery.take()
    }

    #[cfg(test)]
    /// Delay debounce uji (tes memicu autosave instan/tak-aktif sesuai kebutuhan).
    pub fn set_delay(&mut self, delay: Duration) {
        self.delay = delay;
    }

    #[cfg(test)]
    /// Ada perubahan konten yang belum memicu autosave (untuk tes §13.2).
    pub fn has_pending_change(&self) -> bool {
        self.changed_since.is_some()
    }

    /// Tunggu seluruh autosave terkirim tuntas (FIFO) — dipakai tes untuk
    /// membaca isi file tanpa menebak waktu thread.
    pub fn autosave_wait(&self) {
        self.autosave.wait_idle();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use flipbrush_core::raster::Raster;

    /// Folder uji unik per pemanggilan (dibersihkan di akhir tes).
    fn temp_dir() -> PathBuf {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        std::env::temp_dir().join(format!(
            "flipbrush-project-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    }

    /// Animasi kecil tapi berisi — untuk roundtrip autosave.
    fn sample() -> Animation {
        let mut anim = Animation::new(64, 48);
        anim.fps = 24;
        let base = anim.layer_id(0).unwrap();
        let cel = anim.expose(base, 0).unwrap();
        anim.put_cel(cel, Raster::new(64, 48));
        anim
    }

    #[test]
    fn tulis_aman_membuat_folder_dan_tidak_meninggalkan_tmp() {
        let dir = temp_dir();
        let path = dir.join("proyek.flip");
        atomic_write(&path, b"isi").expect("tulis");
        assert_eq!(fs::read(&path).expect("ada"), b"isi");
        assert!(
            !path.with_extension("tmp").exists(),
            "file .tmp sementara sudah di-rename"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn autosave_path_memakai_pola_nama_file() {
        let path = autosave_path(Path::new("/tmp/uji"));
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            "Tanpa Judul.flip.autosave",
            "§13.2: <nama>.flip.autosave"
        );
    }

    /// Setel mtime file secara deterministik (determinisme tes §6).
    fn set_mtime(path: &Path, waktu: SystemTime) {
        let file = fs::OpenOptions::new()
            .write(true)
            .open(path)
            .expect("buka untuk mtime");
        file.set_modified(waktu).expect("set mtime");
    }

    #[test]
    fn find_recovery_kondisi_file_utama_vs_autosave() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).expect("folder");
        let main = dir.join("proyek.flip");
        let autosave = autosave_path(&dir);
        let lama = UNIX_EPOCH + Duration::from_secs(1_000);
        let baru = UNIX_EPOCH + Duration::from_secs(2_000);

        // Belum ada file apa pun.
        assert_eq!(find_recovery(Some(&main), &autosave), None);

        // Autosave ada (tanpa file utama) → pulihkan.
        fs::write(&autosave, b"isi").expect("tulis");
        set_mtime(&autosave, lama);
        assert_eq!(
            find_recovery(Some(&main), &autosave),
            Some(autosave.clone())
        );
        assert_eq!(find_recovery(None, &autosave), Some(autosave.clone()));

        // File utama lebih baru → tidak usah dipulihkan.
        fs::write(&main, b"isi").expect("tulis");
        set_mtime(&main, baru);
        assert_eq!(find_recovery(Some(&main), &autosave), None);

        // Autosave lebih baru → dipulihkan.
        set_mtime(&autosave, baru + Duration::from_secs(1));
        assert_eq!(
            find_recovery(Some(&main), &autosave),
            Some(autosave.clone())
        );

        // Autosave kosong (0 byte) → abaikan.
        fs::write(&autosave, b"").expect("kosong");
        assert_eq!(find_recovery(None, &autosave), None);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn autosave_in_menghitung_sisa_debounce() {
        let mut state = State::new(temp_dir(), Duration::from_secs(3600));
        assert_eq!(state.autosave_in(), None, "tanpa perubahan = tanpa jadwal");

        state.mark_changed();
        let sisa = state.autosave_in().expect("terjadwal");
        assert!(
            sisa > Duration::from_secs(3590) && sisa <= Duration::from_secs(3600),
            "nyaris penuh setelah ditandai: {sisa:?}"
        );
        assert!(!state.autosave_due(), "belum waktunya");

        state.set_delay(Duration::ZERO);
        assert_eq!(state.autosave_in(), Some(Duration::ZERO), "sudah waktunya");
        assert!(state.autosave_due(), "debounce terpenuhi");

        state.autosave_send(sample());
        assert_eq!(state.autosave_in(), None, "setelah dikirim, jadwal bersih");
    }

    #[test]
    fn worker_autosave_tulis_lalu_hapus_fifo() {
        let dir = temp_dir();
        let path = autosave_path(&dir);
        let autosave = Autosave::spawn();

        // Tulis sinkron → file valid (roundtrip lewat thread).
        autosave.submit_sync(sample(), path.clone(), 1_700_000_000);
        let bytes = fs::read(&path).expect("autosave tertulis");
        let project = flip::decode(&bytes).expect("autosave valid");
        assert_eq!(project.anim.width, 64);
        assert_eq!(project.anim.fps, 24);
        assert_eq!(project.created_unix, 1_700_000_000);

        // Job async dulu, lalu hapus sinkron — FIFO menjamin file raip.
        autosave.submit(sample(), path.clone(), 0);
        autosave.remove_sync(path.clone());
        assert!(!path.exists(), "hapus dieksekusi setelah simpan antrian");
        assert!(!path.with_extension("tmp").exists(), "tanpa sisa tmp");

        drop(autosave);
        let _ = fs::remove_dir_all(&dir);
    }
}
