//! Shell aplikasi Flipbrush: entry point, wiring eframe, dan state UI.
//!
//! Layout mengikuti DESIGN.md §8 — desktop-first (Linux prioritas utama);
//! shell Android (M6) memakai komponen yang sama dengan layout berbeda.
//!
//! M1 (§12.2): tipografi §3 terpasang penuh, token §2/§4/§6 ter-resolve, dan
//! galeri 25 ikon §5 menjadi permukaan verifikasi.

pub mod icon;
pub mod theme;
pub mod typography;

use eframe::egui;

use theme::Theme;
use typography::Type;

/// Nama aplikasi (juga dipakai sebagai app id persistensi egui).
pub const APP_NAME: &str = "Flipbrush";
/// Tagline (DESIGN.md §1).
pub const TAGLINE: &str = "Flip. Draw. Loop.";

/// Aplikasi Flipbrush — canvas-first, chrome setipis mungkin.
pub struct FlipbrushApp {
    theme: Theme,
    /// Galeri M1: ikon yang sedang "aktif" (state tool riil mulai M4 §12.3).
    selected_tool: Option<icon::Icon>,
}

impl FlipbrushApp {
    /// Dipanggil sekali saat startup: resolve tema & token (§10), pasang font
    /// §3, dan terapkan gaya egui dari token.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // §10: token di-resolve saat startup. Default gelap (§2.5; Linux).
        let theme = Theme::dark();

        // §3: font variabel Inter + JetBrains Mono per-weight + fallback.
        cc.egui_ctx.set_fonts(typography::definitions());

        // §2.5: default gelap (Linux). Pemilihan tema (dark/light + accent)
        // menyusul lewat settings (§10, M5) — saat ini ditanam gelap.
        cc.egui_ctx.set_theme(egui::Theme::Dark);

        // §2 (warna) + §4 (spacing/radius/elevation) + §7.9 (tooltip delay),
        // lalu §3 (sembilan gaya teks) — dipasang untuk kedua mode §2.5.
        for (egui_theme, resolved) in [
            (egui::Theme::Dark, theme.clone()),
            (egui::Theme::Light, Theme::light()),
        ] {
            let mut style = (*cc.egui_ctx.style_of(egui_theme)).clone();
            resolved.apply_style(&mut style);
            typography::apply_text_styles(&mut style);
            cc.egui_ctx.set_style_of(egui_theme, style);
        }

        Self {
            theme,
            selected_tool: Some(icon::Icon::Brush),
        }
    }
}

impl eframe::App for FlipbrushApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // M1: permukaan verifikasi design system (§12.2) — shell penuh
        // (tool rail §7.4, options bar §7.5, dock §7.6) menyusul di M4 (§8.2).
        // Catatan: panel spesimen ini sengaja menampilkan >2 ukuran font
        // (aturan §3 berlaku untuk panel UI normal, bukan lembar spesimen).
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("m1-specimen")
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(theme::spacing::LARGE);

                            ui.label(
                                Type::HeadingLg
                                    .rich(APP_NAME)
                                    .color(self.theme.text_primary),
                            );
                            ui.label(Type::BodyLg.rich(TAGLINE).color(self.theme.text_secondary));
                            ui.add_space(theme::spacing::SECTION);

                            // — Spesimen tipografi §3 —
                            ui.label(
                                Type::LabelSm
                                    .badge("TIPOGRAFI §3")
                                    .color(self.theme.accent_brand),
                            );
                            ui.add_space(theme::spacing::GAP_CONTROL);
                            ui.label(
                                Type::Body
                                    .rich("body — 13/400 Inter, line-height 1.4")
                                    .color(self.theme.text_primary),
                            );
                            ui.label(
                                Type::BodyLg
                                    .rich("body-lg — 14/400 Inter (dialog, empty state)")
                                    .color(self.theme.text_secondary),
                            );
                            ui.label(
                                Type::LabelMd
                                    .rich("label-md — 12/500 Inter (label kontrol, tab)")
                                    .color(self.theme.text_secondary),
                            );
                            ui.label(
                                Type::HeadingSm
                                    .rich("heading-sm — 16/600 Inter")
                                    .color(self.theme.text_primary),
                            );
                            ui.label(
                                Type::Numeric
                                    .rich("numeric — 0123456789 tabular (slider, timecode)")
                                    .color(self.theme.text_primary),
                            );
                            ui.label(
                                Type::NumericLg
                                    .rich("numeric-lg — 13/500 JBM · frame 0084 / 0240 @12fps")
                                    .color(self.theme.accent_brand),
                            );
                            ui.add_space(theme::spacing::SECTION);

                            // — Galeri ikon §5 (permukaan verifikasi M1) —
                            ui.label(
                                Type::LabelSm
                                    .badge("IKON §5 — 25")
                                    .color(self.theme.accent_brand),
                            );
                            ui.add_space(theme::spacing::GAP_CONTROL);
                            ui.horizontal_wrapped(|ui| {
                                for tool in icon::Icon::ALL {
                                    let selected = self.selected_tool == Some(tool);
                                    let response = icon::icon_button(
                                        ui,
                                        &self.theme,
                                        tool,
                                        tool.name(),
                                        selected,
                                    );
                                    if response.clicked() {
                                        self.selected_tool = Some(tool);
                                    }
                                }
                            });
                            ui.add_space(theme::spacing::SECTION);

                            ui.label(
                                Type::Numeric
                                    .rich(format!(
                                        "v{} · {} ikon · tema gelap · aksen {}",
                                        env!("CARGO_PKG_VERSION"),
                                        icon::Icon::ALL.len(),
                                        self.theme.accent.label(),
                                    ))
                                    .color(self.theme.text_disabled),
                            );
                            ui.add_space(theme::spacing::LARGE);
                        });
                    });
            });
    }
}

/// Jalankan eframe dengan opsi yang diberikan (dipanggil dari bin desktop
/// maupun entry Android).
pub fn run(options: eframe::NativeOptions) -> eframe::Result {
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(FlipbrushApp::new(cc)))),
    )
}

/// Entry point Android — dipanggil runtime NativeActivity.
/// `cdylib` wajib untuk target Android (lihat `crate-type` di Cargo.toml).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
    log::info!("Flipbrush android_main");

    let options = eframe::NativeOptions {
        android_app: Some(app),
        ..Default::default()
    };
    if let Err(err) = run(options) {
        log::error!("eframe error: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M1 (§12.2): startup wiring — fonts §3 terpasang dan sembilan gaya
    /// terdaftar di style egui.
    #[test]
    fn startup_memasang_font_dan_gaya_section_3() {
        let theme = Theme::dark();
        let ctx = egui::Context::default();
        ctx.set_fonts(typography::definitions());
        let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
        theme.apply_style(&mut style);
        typography::apply_text_styles(&mut style);

        assert!(
            style.text_styles.contains_key(&egui::TextStyle::Body),
            "gaya body §3 harus terdaftar"
        );
        assert_eq!(
            style.text_styles.get(&egui::TextStyle::Heading),
            Some(&Type::HeadingMd.font_id())
        );
        // §2.5/§7.9: token non-font ikut terpasang lewat apply_style.
        assert_eq!(style.interaction.tooltip_delay, 0.5);
        assert_eq!(style.spacing.button_padding.x, theme::spacing::GAP_CONTROL);
        assert_eq!(style.visuals.panel_fill, theme.surface_0);
    }
}
