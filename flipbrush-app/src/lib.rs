//! Shell aplikasi Flipbrush: entry point, wiring eframe, dan state UI.
//!
//! Layout mengikuti DESIGN.md §8 — desktop-first (Linux prioritas utama);
//! shell Android (M6) memakai komponen yang sama dengan layout berbeda.

pub mod theme;

use eframe::egui;

use theme::Theme;

/// Nama aplikasi (juga dipakai sebagai app id persistensi egui).
pub const APP_NAME: &str = "Flipbrush";
/// Tagline (DESIGN.md §1).
pub const TAGLINE: &str = "Flip. Draw. Loop.";

/// Aplikasi Flipbrush — canvas-first, chrome setipis mungkin.
pub struct FlipbrushApp {
    theme: Theme,
}

impl FlipbrushApp {
    /// Dipanggil sekali saat startup: resolve tema (§10) dan terapkan ke egui.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let theme = Theme::dark();
        let mut visuals = egui::Visuals::dark();
        theme.apply(&mut visuals);
        cc.egui_ctx.set_visuals(visuals);
        Self { theme }
    }
}

impl eframe::App for FlipbrushApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // M0: chrome minimum di atas kanvas. Bottom toolbar (§7.12) dan
        // timeline strip (§8.1) menyusul di M2–M4.
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(32.0);
                    ui.label(
                        egui::RichText::new(APP_NAME)
                            .size(24.0)
                            .strong()
                            .color(self.theme.text_primary),
                    );
                    ui.label(
                        egui::RichText::new(TAGLINE)
                            .size(14.0)
                            .color(self.theme.text_secondary),
                    );
                    ui.add_space(24.0);
                    ui.label(
                        egui::RichText::new(env!("CARGO_PKG_VERSION"))
                            .monospace()
                            .size(12.0)
                            .color(self.theme.text_disabled),
                    );
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
