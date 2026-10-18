//! Command palette — DESIGN §7.10 (M4c).
//!
//! Modal terpusat 560 px (`Ctrl+K`): input `body-lg`, hasil `label-md` +
//! shortcut di kanan. Sumber: semua command + recent tools sesi (§7.10);
//! "file recent" berbentuk struktur — daftar berkasnya menyusul bersama
//! M5 saat benar-benar ada berkas yang bisa dibuka.
//!
//! `Esc` / klik backdrop menutup (modal egui, lapisan modal memblokir
//! input di bawahnya), `↑/↓` memilih baris (wrap), `Enter` menjalankan
//! baris terpilih lalu menutup. Hasil kosong memakai empty state §7.11.

use eframe::egui::{self, Align2, CornerRadius, Id, Rect, Sense, Ui, Vec2};

use crate::dock::PanelTab;
use crate::icon::{self, Icon};
use crate::theme::{Theme, elevation, radius, spacing};
use crate::tool::Tool;
use crate::typography::Type;

/// Lebar modal §7.10.
const LEBAR: f32 = 560.0;
/// Tinggi baris hasil — kiasan item menu §7.8 (28 px).
const BARIS_H: f32 = 28.0;
/// Jumlah entri grup "Terakhir dipakai" (recent tools §7.10).
const RECENT_MAX: usize = 5;
/// Strip aksen baris terpilih — kiasan strip aksen rail §7.4.
const STRIP_PX: f32 = 2.0;

/// Id area modal (stabil antar frame).
fn id_area() -> Id {
    Id::new("command-palette")
}

/// Aksi yang dijalankan palette — diterapkan pemanggil (`lib.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aksi {
    /// Pindah tool (`B`/`E`/`G`/`I`, juga grup "Terakhir dipakai").
    Tool(Tool),
    /// `Ctrl+Z`.
    Urungkan,
    /// `Ctrl+Shift+Z`.
    Ulangi,
    /// Saklar mode fokus `Tab` (§8.2).
    ModeFokus,
    /// Tampilkan tab dock (§7.6).
    Tab(PanelTab),
    /// Putar / jeda playback timeline (§7.7).
    PutarJeda,
}

/// State palette — transien, tidak dipersist (§12.2).
#[derive(Default)]
pub struct Palette {
    /// Modal terbuka (`Ctrl+K`).
    pub open: bool,
    /// Filter ketik.
    query: String,
    /// Indeks baris terpilih pada hasil terfilter.
    selected: usize,
    /// Riwayat tool sesi (§7.10 "recent tools") — terbaru di depan.
    recent: Vec<Tool>,
    /// Minta fokus ke input sekali saat baru dibuka.
    fokus_input: bool,
}

impl Palette {
    /// Buka modal — reset filter & seleksi, fokus ke input pencarian.
    pub fn buka(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
        self.fokus_input = true;
    }

    /// Tutup modal.
    pub fn tutup(&mut self) {
        self.open = false;
    }

    /// Riwayat tool sesi (§7.10 recent tools) — terbaru di depan.
    pub fn recent(&self) -> &[Tool] {
        &self.recent
    }

    /// Catat tool yang baru dipakai (§7.10 recent tools) — terbaru di
    /// depan, tanpa duplikat, dibatasi [`RECENT_MAX`].
    pub fn catat_tool(&mut self, tool: Tool) {
        self.recent.retain(|&t| t != tool);
        self.recent.insert(0, tool);
        self.recent.truncate(RECENT_MAX);
    }
}

/// Satu baris hasil: label + shortcut kanan + grup + aksi.
struct Entri {
    label: String,
    shortcut: Option<&'static str>,
    grup: &'static str,
    aksi: Aksi,
}

impl Entri {
    fn baru(
        label: impl Into<String>,
        shortcut: Option<&'static str>,
        grup: &'static str,
        aksi: Aksi,
    ) -> Self {
        Self {
            label: label.into(),
            shortcut,
            grup,
            aksi,
        }
    }
}

/// Entri tool — label nama tool, shortcut huruf (§7.1).
fn entri_tool(tool: Tool, grup: &'static str) -> Entri {
    Entri::baru(tool.name(), Some(tool.shortcut()), grup, Aksi::Tool(tool))
}

/// Seluruh command (§7.10) — urut per grup.
fn daftar(recent: &[Tool], playing: bool) -> Vec<Entri> {
    let mut e: Vec<Entri> = Vec::new();
    // §7.10: recent tools tampil dulu sebagai jalan pintas.
    for &tool in recent {
        e.push(entri_tool(tool, "Terakhir dipakai"));
    }
    for tool in Tool::ALL {
        e.push(entri_tool(tool, "Tool"));
    }
    e.push(Entri::baru(
        "Urungkan",
        Some("Ctrl+Z"),
        "Umum",
        Aksi::Urungkan,
    ));
    e.push(Entri::baru(
        "Ulangi",
        Some("Ctrl+Shift+Z"),
        "Umum",
        Aksi::Ulangi,
    ));
    e.push(Entri::baru(
        "Mode fokus",
        Some("Tab"),
        "Umum",
        Aksi::ModeFokus,
    ));
    for tab in PanelTab::ALL {
        e.push(Entri::baru(
            format!("Panel {}", tab.label()),
            None,
            "Panel",
            Aksi::Tab(tab),
        ));
    }
    e.push(Entri::baru(
        if playing { "Jeda" } else { "Putar" },
        None,
        "Playback",
        Aksi::PutarJeda,
    ));
    // §7.10 "file recent": daftar berkas menyusul bersama M5 — grup
    // disembunyikan selama daftarnya masih kosong.
    e
}

/// Filter — cocok label ATAU shortcut, tanpa memedulikan huruf besar.
fn filter<'a>(semua: &'a [Entri], query: &str) -> Vec<&'a Entri> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return semua.iter().collect();
    }
    semua
        .iter()
        .filter(|t| {
            t.label.to_lowercase().contains(&q)
                || t.shortcut.is_some_and(|s| s.to_lowercase().contains(&q))
        })
        .collect()
}

/// Gambar modal (§7.10); kembalikan aksi yang dijalankan pemanggil.
pub fn show(
    ctx: &egui::Context,
    theme: &Theme,
    palette: &mut Palette,
    playing: bool,
) -> Option<Aksi> {
    if !palette.open {
        return None;
    }

    // Navigasi keyboard dikonsumsi dulu — sebelum TextEdit memakai
    // tombonya (↑/↓/Enter bukan bagian dari kursor teks baris tunggal).
    // `Esc` & klik-backdrop diserahkan ke modal egui (`should_close`).
    let mut langkah = 0i32;
    let mut jalankan = false;
    ctx.input_mut(|i| {
        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
            langkah = 1;
        }
        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
            langkah = -1;
        }
        if i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
            jalankan = true;
        }
    });

    let semua = daftar(&palette.recent, playing);
    let tampil = filter(&semua, &palette.query);

    // Seleksi: wrap melingkar; reset ke baris pertama saat query berubah.
    if langkah != 0 && !tampil.is_empty() {
        let n = tampil.len();
        palette.selected = if langkah > 0 {
            (palette.selected + 1) % n
        } else {
            (palette.selected + n - 1) % n
        };
    }
    let pilih = if tampil.is_empty() {
        0
    } else {
        palette.selected.min(tampil.len() - 1)
    };
    palette.selected = pilih;

    let mut aksi: Option<Aksi> = None;
    let query_sebelum = palette.query.clone();

    let frame = egui::Frame::NONE
        .fill(theme.surface_1)
        .corner_radius(CornerRadius::same(radius::MD))
        .shadow(elevation::MODAL)
        .inner_margin(spacing::PANEL);

    let modal = egui::Modal::new(id_area()).frame(frame).show(ctx, |ui| {
        // §7.10: modal 560 px total — konten di dalam inner margin.
        let lebar = LEBAR - 2.0 * spacing::PANEL;
        ui.set_min_width(lebar);
        ui.set_max_width(lebar);
        aksi = isi(ui, theme, palette, &tampil, pilih);
    });

    if modal.should_close() {
        palette.tutup();
    }
    if aksi.is_none() && jalankan && !tampil.is_empty() {
        aksi = Some(tampil[pilih].aksi);
    }
    if aksi.is_some() {
        // Menjalankan baris = tutup modal (predictable; buka lagi `Ctrl+K`).
        palette.tutup();
    }
    if palette.query != query_sebelum {
        palette.selected = 0;
    }
    aksi
}

/// Isi modal: input + hasil (atau empty state §7.11).
fn isi(
    ui: &mut Ui,
    theme: &Theme,
    palette: &mut Palette,
    tampil: &[&Entri],
    pilih: usize,
) -> Option<Aksi> {
    // Input pencarian — §7.10 `body-lg`.
    let teks = egui::TextEdit::singleline(&mut palette.query)
        .font(Type::BodyLg.text_style())
        .hint_text("Cari perintah…")
        .desired_width(ui.available_width());
    let resp_input = ui.add(teks);
    if palette.fokus_input {
        resp_input.request_focus();
        palette.fokus_input = false;
    }
    let _ = ui.ctx().accesskit_node_builder(resp_input.id, |node| {
        node.set_label("Cari perintah");
    });

    ui.add_space(spacing::LIST);

    if tampil.is_empty() {
        empty_state(ui, theme);
        return None;
    }
    hasil(ui, theme, palette, tampil, pilih)
}

/// Daftar hasil — grup dengan header, baris terpilih dapat strip aksen.
fn hasil(
    ui: &mut Ui,
    theme: &Theme,
    palette: &mut Palette,
    tampil: &[&Entri],
    pilih: usize,
) -> Option<Aksi> {
    let mut aksi = None;
    let mut grup_sebelum = "";
    for (i, entri) in tampil.iter().enumerate() {
        if entri.grup != grup_sebelum {
            grup_sebelum = entri.grup;
            judul_grup(ui, theme, entri.grup);
        }
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), BARIS_H), Sense::click());
        if resp.hovered() {
            palette.selected = i;
        }
        if resp.clicked() {
            aksi = Some(entri.aksi);
        }
        let terpilih = i == pilih;
        if terpilih {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(radius::SM), theme.surface_2);
            let strip = Rect::from_min_size(rect.min, Vec2::new(STRIP_PX, rect.height()));
            ui.painter().rect_filled(strip, 0.0, theme.accent_brand);
        }
        let baseline = rect.center();
        ui.painter().text(
            egui::pos2(rect.left() + spacing::LIST, baseline.y),
            Align2::LEFT_CENTER,
            &entri.label,
            Type::LabelMd.font_id(),
            if terpilih {
                theme.text_primary
            } else {
                theme.text_secondary
            },
        );
        if let Some(sc) = entri.shortcut {
            ui.painter().text(
                egui::pos2(rect.right() - spacing::LIST, baseline.y),
                Align2::RIGHT_CENTER,
                sc,
                Type::Numeric.font_id(),
                theme.text_disabled,
            );
        }
        let _ = ui.ctx().accesskit_node_builder(resp.id, |node| {
            node.set_label(entri.label.as_str());
        });
    }
    aksi
}

/// Header grup — teks `label-md` sekunder (huruf kecil, §5.1).
fn judul_grup(ui: &mut Ui, theme: &Theme, grup: &str) {
    ui.add_space(spacing::GAP_CONTROL);
    let tinggi = Type::LabelMd.line_height();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), tinggi), Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left() + spacing::LIST, rect.center().y),
        Align2::LEFT_CENTER,
        grup,
        Type::LabelMd.font_id(),
        theme.text_secondary,
    );
    ui.add_space(spacing::ICON_LABEL);
}

/// Empty state §7.11: ikon outline 32 px + teks `body-lg`.
fn empty_state(ui: &mut Ui, theme: &Theme) {
    ui.vertical_centered(|ui| {
        ui.add_space(spacing::SECTION);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
        icon::paint(ui.painter(), rect, Icon::Zoom, theme.text_disabled);
        ui.add_space(spacing::GAP_CONTROL);
        let teks = Type::BodyLg
            .rich("Tidak ada perintah yang cocok")
            .color(theme.text_secondary);
        ui.label(teks);
        ui.add_space(spacing::SECTION);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_cocok_label_dan_shortcut() {
        let semua = daftar(&[], false);
        assert!(
            filter(&semua, "urung")
                .iter()
                .any(|e| e.label == "Urungkan"),
            "label case-insensitive"
        );
        assert!(
            filter(&semua, "CTRL+SHIFT+Z")
                .iter()
                .any(|e| e.label == "Ulangi"),
            "shortcut ikut dicocokkan"
        );
        assert!(filter(&semua, "qqq").is_empty());
    }

    #[test]
    fn daftar_mencakup_semua_command_section_7_10() {
        let semua = daftar(&[Tool::Eyedropper], true);
        let label: Vec<&str> = semua.iter().map(|e| e.label.as_str()).collect();
        for diharapkan in [
            "Eyedropper", // recent (juga muncul di grup Tool)
            "Brush",
            "Eraser",
            "Fill",
            "Urungkan",
            "Ulangi",
            "Mode fokus",
            "Panel Color",
            "Panel Brush",
            "Panel Layers",
            "Jeda", // playing = true → label "Jeda"
        ] {
            assert!(label.contains(&diharapkan), "kurang: {diharapkan}");
        }
        // Recent tampil paling depan sebagai jalan pintas §7.10.
        assert_eq!(semua[0].grup, "Terakhir dipakai");
        // Bermain saat idle → "Putar".
        let idle = daftar(&[], false);
        assert!(idle.iter().any(|e| e.label == "Putar"));
    }

    #[test]
    fn recent_tool_tanpa_duplikat_dibatasi() {
        let mut p = Palette::default();
        p.catat_tool(Tool::Brush);
        p.catat_tool(Tool::Eraser);
        p.catat_tool(Tool::Brush); // pindah ke depan, tak diduplikat
        assert_eq!(p.recent, vec![Tool::Brush, Tool::Eraser]);
        for tool in Tool::ALL {
            p.catat_tool(tool);
        }
        // Tak ada tool kelima — riwayat = semua tool unik terakhir.
        assert_eq!(p.recent.len(), Tool::ALL.len());
        assert_eq!(p.recent[0], Tool::Eyedropper);
    }

    #[test]
    fn buka_reset_query_dan_seleksi() {
        let mut p = Palette::default();
        p.buka();
        p.query = "xyz".into();
        p.selected = 3;
        p.tutup();
        assert!(!p.open);
        p.buka();
        assert!(p.query.is_empty());
        assert_eq!(p.selected, 0);
        assert!(p.fokus_input, "input dikembalikan fokusnya");
    }
}
