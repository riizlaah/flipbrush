//! Entry point desktop (Linux — target utama; dipakai untuk
//! development harian sejak M0).

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(flipbrush_app::APP_NAME)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };
    flipbrush_app::run(options)
}
