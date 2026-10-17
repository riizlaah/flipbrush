//! Shell aplikasi Flipbrush: entry point, wiring eframe, dan state UI.
//!
//! Layout mengikuti DESIGN.md §8 — desktop-first (Linux prioritas utama);
//! shell Android (M6) memakai komponen yang sama dengan layout berbeda.
//!
//! M1 (§12.2): tipografi §3 terpasang penuh, token §2/§4/§6 ter-resolve,
//! galeri 25 ikon §5 sebagai permukaan verifikasi.
//! M2 (§12.2): kanvas & stroke core — tool rail §7.4 (48 px), options bar
//! §7.5 (40 px) dengan slider drag-numeric §7.2, kanvas zoom/pan/undo/redo,
//! mode fokus `Tab` (§8.2), stroke selalu instan (§6, §13.3).
//! M3 tahap 1 (§12.2): model animasi (layer/frame/exposure — core `anim`),
//! timeline dock X-sheet §7.7 (200 px, playhead, prev/next, zoom
//! `Ctrl+Scroll`), buffer cel aktif di kanvas (riwayat per cel).

pub mod canvas;
pub mod icon;
pub mod slider;
pub mod theme;
pub mod timeline;
pub mod tool;
pub mod typography;

use eframe::egui;

use flipbrush_core::{
    anim::Animation,
    raster::{Brush, Rgba},
};

use canvas::Canvas;
use theme::Theme;
use timeline::Timeline;
use tool::Tool;
use typography::Type;

/// Nama aplikasi (juga dipakai sebagai app id persistensi egui).
pub const APP_NAME: &str = "Flipbrush";
/// Tagline (DESIGN.md §1).
pub const TAGLINE: &str = "Flip. Draw. Loop.";

/// Tinggi options bar (§7.5: 40 px).
pub const OPTIONS_BAR_H: f32 = 40.0;
/// Dimensi kanvas dokumen default (§13.3: proyek default 1080²).
const DEFAULT_CANVAS: u32 = 1080;
/// Nilai awal toleransi fill (0–100) — cukup longgar untuk tepi antialias.
const DEFAULT_FILL_TOLERANCE: f32 = 12.0;

/// Aplikasi Flipbrush — canvas-first, chrome setipis mungkin.
pub struct FlipbrushApp {
    theme: Theme,
    /// Tool aktif (M2: brush/eraser/fill/eyedropper).
    tool: Tool,
    /// Parameter kuas; penghapus memakai parameter yang sama (§7.5).
    brush: Brush,
    /// Toleransi fill 0–100 (§7.5 kontrol spesifik tool).
    fill_tolerance: f32,
    /// Warna kuas aktif; berubah saat eyedropper mengambil piksel.
    color: Rgba,
    /// Mode fokus `Tab` (§1, §8.2): sembunyikan chrome, hanya kanvas.
    focus_mode: bool,
    /// Model animasi M3: layer, frame, cel, exposure (DESIGN §12.2).
    anim: Animation,
    /// State timeline M3: playhead, layer aktif, zoom kolom (§7.7).
    timeline: Timeline,
    /// Kanvas + viewport + riwayat (§12.2 M2/M3).
    canvas: Canvas,
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

        // Tinta awal = netral mengikuti tema (bukan token chrome — warna
        // kuas adalah data pengguna, bukan warna UI).
        let ink = theme.text_primary;
        Self {
            theme,
            tool: Tool::Brush,
            brush: Brush::default(),
            fill_tolerance: DEFAULT_FILL_TOLERANCE,
            color: Rgba {
                r: ink.r(),
                g: ink.g(),
                b: ink.b(),
                a: 255,
            },
            focus_mode: false,
            anim: Animation::new(DEFAULT_CANVAS, DEFAULT_CANVAS),
            timeline: Timeline::default(),
            canvas: Canvas::new(DEFAULT_CANVAS, DEFAULT_CANVAS),
        }
    }

    /// Satu frame UI (dipisah dari `eframe::App::ui` agar tes headless bisa
    /// memanggil tanpa `eframe::Frame`).
    fn ui_inner(&mut self, ui: &mut egui::Ui) {
        self.handle_shortcuts(ui.ctx());

        if self.focus_mode {
            // §1/§8.2: mode fokus — hanya kanvas (+ HUD koordinat §1.4).
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| self.show_canvas(ui));
            return;
        }

        // §7.4: rail 48 px kiri — separator bawaan panel = divider
        // `border/default` (visuals sudah di-set dari token di `apply_style`).
        egui::Panel::left("tool-rail")
            .exact_size(tool::RAIL_W)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(self.theme.surface_1))
            .show(ui, |ui| {
                self.tool = tool::show(ui, &self.theme, self.tool);
            });

        // §7.5: options bar 40 px di atas kanvas (kontrol tool aktif).
        egui::Panel::top("options-bar")
            .exact_size(OPTIONS_BAR_H)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(self.theme.surface_1))
            .show(ui, |ui| {
                self.options_bar(ui);
            });

        // §8.2: timeline dock 200 px di bawah kanvas (grid X-sheet §7.7).
        egui::Panel::bottom("timeline")
            .exact_size(timeline::TIMELINE_H)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(self.theme.surface_1))
            .show(ui, |ui| {
                self.timeline.show(ui, &self.theme, &self.anim);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.show_canvas(ui));
    }

    /// Isi options bar §7.5: kontrol spesifik tool + swatch warna kanan.
    fn options_bar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme.clone();
        ui.add_space(theme::spacing::GRID);
        ui.horizontal(|ui| {
            ui.add_space(theme::spacing::PANEL);
            ui.spacing_mut().item_spacing.x = theme::spacing::GAP_CONTROL;

            match self.tool {
                Tool::Brush | Tool::Eraser => {
                    let mut size = self.brush.size;
                    slider::DragNumeric::new("Ukuran", &mut size, 1.0..=200.0, 1.0)
                        .suffix(" px")
                        .show(ui, &theme);
                    self.brush.size = size;
                    divider(ui, &theme);

                    let mut opacity = self.brush.opacity * 100.0;
                    slider::DragNumeric::new("Opasitas", &mut opacity, 1.0..=100.0, 1.0)
                        .suffix("%")
                        .show(ui, &theme);
                    self.brush.opacity = (opacity / 100.0).clamp(0.01, 1.0);
                    divider(ui, &theme);

                    let mut hardness = self.brush.hardness * 100.0;
                    slider::DragNumeric::new("Kekerasan", &mut hardness, 0.0..=100.0, 1.0)
                        .suffix("%")
                        .show(ui, &theme);
                    self.brush.hardness = (hardness / 100.0).clamp(0.0, 1.0);
                }
                Tool::Fill => {
                    let mut tolerance = self.fill_tolerance;
                    slider::DragNumeric::new("Toleransi", &mut tolerance, 0.0..=100.0, 1.0)
                        .show(ui, &theme);
                    self.fill_tolerance = tolerance;
                }
                Tool::Eyedropper => {
                    ui.label(
                        Type::Body
                            .rich("Klik-panjang di kanvas untuk mengambil warna")
                            .color(theme.text_secondary),
                    );
                }
            }

            // Kanan bar: swatch warna aktif + hex (§1.4 presisi — angka jelas).
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (rect, _) =
                    ui.allocate_exact_size(egui::Vec2::splat(20.0), egui::Sense::hover());
                ui.painter().rect_filled(
                    rect,
                    theme::radius::SM,
                    egui::Color32::from_rgba_unmultiplied(
                        self.color.r,
                        self.color.g,
                        self.color.b,
                        self.color.a,
                    ),
                );
                ui.painter().rect_stroke(
                    rect,
                    egui::CornerRadius::same(theme::radius::SM),
                    egui::Stroke::new(1.0, theme.border_strong),
                    egui::epaint::StrokeKind::Middle,
                );
                ui.label(
                    Type::Numeric
                        .rich(format!(
                            "#{:02X}{:02X}{:02X}",
                            self.color.r, self.color.g, self.color.b
                        ))
                        .color(theme.text_secondary),
                );
            });
        });
    }

    fn show_canvas(&mut self, ui: &mut egui::Ui) {
        // M3: buffer kanvas mengikuti (playhead × layer aktif) — pindah cel
        // dulu (menukar raster + riwayat), baru proses input.
        let layer_id = self.anim.layer_id(self.timeline.layer);
        let target = layer_id.and_then(|l| self.anim.cel_id_at(l, self.timeline.frame));
        self.canvas.switch_cel(&mut self.anim, target);

        let tool = self.tool;
        let brush = self.brush;
        let tolerance = self.fill_tolerance.round() as u8;
        self.canvas
            .show(ui, &self.theme, tool, brush, tolerance, &mut self.color);

        // Menggambar di posisi playhead yang masih kosong → expose cel baru
        // (raster tetap di buffer; model hanya mencatat exposure §7.7).
        if self.canvas.take_modified()
            && self.canvas.active_cel().is_none()
            && let Some(layer_id) = layer_id
            && let Some(cel) = self.anim.expose(layer_id, self.timeline.frame)
        {
            self.canvas.adopt_cel(cel);
        }
    }

    /// Shortcut desktop §12.2: tool `B/E/G/I`, undo `Ctrl+Z`, redo
    /// `Ctrl+Shift+Z`, mode fokus `Tab` (§8.2). Dikonsumsi hanya bila tidak
    /// sedang mengetik (agar edit nilai §7.2 tidak terganggu).
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.text_edit_focused() {
            return;
        }
        let (switch, undo, redo, focus) = ctx.input_mut(|i| {
            let switch = Tool::ALL
                .into_iter()
                .find(|tool| i.consume_key(egui::Modifiers::NONE, tool.key()));
            // Urutan penting: `matches_logically` mengabaikan Shift, jadi
            // combo +Shift dicek sebelum combo polos.
            let redo = i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Z);
            let undo = i.consume_key(egui::Modifiers::CTRL, egui::Key::Z);
            let focus = i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
            (switch, undo, redo, focus)
        });
        if let Some(tool) = switch {
            self.tool = tool;
        }
        if undo {
            self.canvas.undo();
        }
        if redo {
            self.canvas.redo();
        }
        if focus {
            self.focus_mode = !self.focus_mode;
        }
    }
}

/// Pemisah vertikal kontrol di options bar (§7.5: divider `border/default`).
fn divider(ui: &mut egui::Ui, theme: &Theme) {
    let (rect, _) = ui.allocate_exact_size(
        egui::Vec2::new(1.0, theme::spacing::SECTION),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(rect, 0.0, theme.border_default);
}

impl eframe::App for FlipbrushApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui_inner(ui);
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
    use eframe::egui::{Modifiers, PointerButton, Pos2, RawInput, Vec2, pos2};

    const SCREEN: [f32; 2] = [1280.0, 800.0];

    /// Setup egui headless: font §3 + gaya teks + mode gelap (seperti `new`).
    fn test_ctx() -> egui::Context {
        let ctx = egui::Context::default();
        ctx.set_fonts(typography::definitions());
        ctx.set_theme(egui::Theme::Dark);
        let theme = Theme::dark();
        for (egui_theme, resolved) in [
            (egui::Theme::Dark, theme),
            (egui::Theme::Light, Theme::light()),
        ] {
            let mut style = (*ctx.style_of(egui_theme)).clone();
            resolved.apply_style(&mut style);
            typography::apply_text_styles(&mut style);
            ctx.set_style_of(egui_theme, style);
        }
        ctx
    }

    /// Aplikasi uji (state langsung — `new()` butuh `CreationContext`).
    fn test_app() -> FlipbrushApp {
        let theme = Theme::dark();
        let ink = theme.text_primary;
        FlipbrushApp {
            theme,
            tool: Tool::Brush,
            brush: Brush::default(),
            fill_tolerance: DEFAULT_FILL_TOLERANCE,
            color: Rgba {
                r: ink.r(),
                g: ink.g(),
                b: ink.b(),
                a: 255,
            },
            focus_mode: false,
            anim: Animation::new(DEFAULT_CANVAS, DEFAULT_CANVAS),
            timeline: Timeline::default(),
            canvas: Canvas::new(DEFAULT_CANVAS, DEFAULT_CANVAS),
        }
    }

    /// Jalankan satu frame; buang texture delta (lihat tes M1).
    fn frame(app: &mut FlipbrushApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        let input = RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(SCREEN[0], SCREEN[1]),
            )),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| app.ui_inner(ui));
        output.textures_delta.clear();
    }

    /// Frame dengan waktu `time` (detik) — untuk klik-panjang.
    fn frame_at(app: &mut FlipbrushApp, ctx: &egui::Context, time: f64, events: Vec<egui::Event>) {
        let input = RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(SCREEN[0], SCREEN[1]),
            )),
            time: Some(time),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| app.ui_inner(ui));
        output.textures_delta.clear();
    }

    /// Posisi layar untuk titik dokumen.
    fn screen_of(app: &FlipbrushApp, doc: Vec2) -> Pos2 {
        app.canvas.viewport.to_screen(doc).to_pos2()
    }

    /// Urutan gambar: tekan → geser → lepas.
    fn stroke(app: &mut FlipbrushApp, ctx: &egui::Context, from: Pos2, to: Pos2) {
        frame(app, ctx, vec![egui::Event::PointerMoved(from)]);
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(from),
                egui::Event::PointerButton {
                    pos: from,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(app, ctx, vec![egui::Event::PointerMoved(to)]);
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(to),
                egui::Event::PointerButton {
                    pos: to,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }

    fn key(key: egui::Key, modifiers: Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    /// Klik tunggal: tekan di satu frame, lepas di frame berikutnya (egui
    /// hanya menghitung klik bila tekan dan lepas berada di frame berbeda).
    fn click(app: &mut FlipbrushApp, ctx: &egui::Context, pos: Pos2) {
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }

    // -------------------------------------------------------------------------
    // M1 (§12.2) — startup wiring
    // -------------------------------------------------------------------------

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

    // -------------------------------------------------------------------------
    // M2 (§12.2) — shortcut desktop responsif
    // -------------------------------------------------------------------------

    #[test]
    fn shortcut_tool_b_e_g_i() {
        let mut app = test_app();
        let ctx = test_ctx();
        assert_eq!(app.tool, Tool::Brush);
        for (k, expected) in [
            (egui::Key::E, Tool::Eraser),
            (egui::Key::G, Tool::Fill),
            (egui::Key::I, Tool::Eyedropper),
            (egui::Key::B, Tool::Brush),
        ] {
            frame(&mut app, &ctx, vec![key(k, Modifiers::NONE)]);
            assert_eq!(app.tool, expected, "shortcut {k:?}");
        }
    }

    #[test]
    fn shortcut_undo_dan_redo_ctrl_z() {
        let mut app = test_app();
        let ctx = test_ctx();
        // Frame pertama: fit kanvas (viewport masih default sebelum fit).
        frame(&mut app, &ctx, vec![]);
        // Gambar satu stroke pendek di tengah kanvas.
        let from = screen_of(&app, Vec2::new(500.0, 500.0));
        let to = screen_of(&app, Vec2::new(540.0, 500.0));
        stroke(&mut app, &ctx, from, to);
        assert!(app.canvas.history.can_undo(), "stroke ter-commit");
        let painted = app.canvas.raster.pixel(520, 500).expect("di kanvas");
        assert!(painted.a > 0, "stroke mengecat piksel (instan §6)");

        // Ctrl+Z → transparan lagi.
        frame(&mut app, &ctx, vec![key(egui::Key::Z, Modifiers::CTRL)]);
        assert!(!app.canvas.history.can_undo());
        assert_eq!(
            app.canvas.raster.pixel(520, 500).unwrap().a,
            0,
            "undo mengembalikan piksel (§12.2)"
        );

        // Ctrl+Shift+Z → ulang.
        frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Z, Modifiers::CTRL | Modifiers::SHIFT)],
        );
        assert!(app.canvas.raster.pixel(520, 500).unwrap().a > 0, "redo");
        assert!(app.canvas.history.can_undo());
    }

    #[test]
    fn shortcut_tab_toggle_mode_fokus() {
        let mut app = test_app();
        let ctx = test_ctx();
        assert!(!app.focus_mode);
        frame(&mut app, &ctx, vec![key(egui::Key::Tab, Modifiers::NONE)]);
        assert!(
            app.focus_mode,
            "Tab → mode fokus (§8.2): chrome tersembunyi"
        );
        frame(&mut app, &ctx, vec![key(egui::Key::Tab, Modifiers::NONE)]);
        assert!(!app.focus_mode, "Tab sekali lagi → chrome kembali");
    }

    // -------------------------------------------------------------------------
    // M2 (§12.2) — kanvas: zoom, pan, gambar, fill, eyedropper
    // -------------------------------------------------------------------------

    #[test]
    fn zoom_pinch_dan_ctrl_scroll_di_sekitar_kursor() {
        let mut app = test_app();
        let ctx = test_ctx();
        // Frame pertama: fit kanvas.
        frame(&mut app, &ctx, vec![]);
        let zoom0 = app.canvas.viewport.zoom;

        // Arahkan kursor ke area kanvas lalu pinch zoom (Event::Zoom egui).
        let cursor = pos2(SCREEN[0] * 0.5, SCREEN[1] * 0.6);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(cursor)]);
        frame(&mut app, &ctx, vec![egui::Event::Zoom(2.0)]);
        assert!(
            (app.canvas.viewport.zoom - zoom0 * 2.0).abs() < 1e-3,
            "pinch zoom ×2 (§12.2): {} → {}",
            zoom0,
            app.canvas.viewport.zoom
        );

        // Ctrl+Scroll juga mengubah zoom (egui menggabung ke zoom_delta;
        // delta y > 0 = memperbesar).
        let zoom1 = app.canvas.viewport.zoom;
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(cursor),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: Vec2::new(0.0, 1.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: Modifiers::CTRL,
                },
            ],
        );
        assert!(
            app.canvas.viewport.zoom > zoom1,
            "Ctrl+Scroll memperbesar (§12.2): {zoom1} → {}",
            app.canvas.viewport.zoom
        );
    }

    #[test]
    fn pan_space_drag_menggeser_tanpa_menggambar() {
        let mut app = test_app();
        let ctx = test_ctx();
        frame(&mut app, &ctx, vec![]); // fit
        let offset0 = app.canvas.viewport.offset;
        let p = pos2(SCREEN[0] * 0.5, SCREEN[1] * 0.6);
        let p2 = pos2(p.x + 60.0, p.y + 30.0);

        // Space ditekan → drag.
        frame(&mut app, &ctx, vec![key(egui::Key::Space, Modifiers::NONE)]);
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(p2)]);
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(p2),
                egui::Event::PointerButton {
                    pos: p2,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );

        let moved = app.canvas.viewport.offset - offset0;
        assert!(
            moved.length() > 10.0,
            "space-drag menggeser viewport (§12.2), delta {moved:?}"
        );
        assert!(
            !app.canvas.history.can_undo(),
            "pan tidak menghasilkan stroke"
        );
    }

    #[test]
    fn fill_tool_mengisi_area_kosong() {
        let mut app = test_app();
        let ctx = test_ctx();
        frame(&mut app, &ctx, vec![]); // fit
        app.tool = Tool::Fill;
        app.color = Rgba::opaque(200, 40, 40);

        let p = screen_of(&app, Vec2::new(100.0, 100.0));
        click(&mut app, &ctx, p);

        let px = app.canvas.raster.pixel(100, 100).unwrap();
        assert_eq!((px.r, px.g, px.b), (200, 40, 40), "fill mengecat seed");
        assert_eq!(
            app.canvas.raster.pixel(1079, 1079).unwrap().b,
            40,
            "kanvas kosong 1080² terisi penuh"
        );
        assert!(app.canvas.history.can_undo(), "fill bisa di-undo");
    }

    #[test]
    fn eyedropper_klik_mengambil_warna() {
        let mut app = test_app();
        let ctx = test_ctx();
        // Isi kanvas dengan warna merah via fill.
        frame(&mut app, &ctx, vec![]);
        app.tool = Tool::Fill;
        app.color = Rgba::opaque(200, 40, 40);
        let p = screen_of(&app, Vec2::new(300.0, 300.0));
        click(&mut app, &ctx, p);

        // Ambil warna dengan tool eyedropper (klik biasa).
        app.tool = Tool::Eyedropper;
        app.color = Rgba::opaque(10, 20, 30);
        let q = screen_of(&app, Vec2::new(500.0, 500.0));
        click(&mut app, &ctx, q);
        assert_eq!(
            (app.color.r, app.color.g, app.color.b),
            (200, 40, 40),
            "eyedropper mengambil warna dari kanvas (§12.2)"
        );
    }

    #[test]
    fn klik_panjang_mengambil_warna_dan_membatalkan_stroke() {
        let mut app = test_app();
        let ctx = test_ctx();
        frame(&mut app, &ctx, vec![]);
        // Isi kanvas merah (fill).
        app.tool = Tool::Fill;
        app.color = Rgba::opaque(200, 40, 40);
        let p = screen_of(&app, Vec2::new(400.0, 400.0));
        click(&mut app, &ctx, p);
        let entries_before = app.canvas.history.can_undo() as usize;

        // Tekan dengan brush (warna berbeda), tahan 0,6 detik tanpa gerak.
        app.tool = Tool::Brush;
        app.color = Rgba::opaque(10, 20, 30);
        frame_at(
            &mut app,
            &ctx,
            0.0,
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        // Stroke sempat mulai (menggunakan warna biru) — cek dulu.
        frame_at(&mut app, &ctx, 0.6, vec![]);
        // Lepas.
        frame_at(
            &mut app,
            &ctx,
            0.7,
            vec![egui::Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        );

        assert_eq!(
            (app.color.r, app.color.g, app.color.b),
            (200, 40, 40),
            "klik-panjang mengambil warna (§12.2)"
        );
        let px = app.canvas.raster.pixel(400, 400).unwrap();
        assert_eq!(
            (px.r, px.g, px.b),
            (200, 40, 40),
            "stroke brush dibatalkan saat klik-panjang (rollback before)"
        );
        assert_eq!(
            app.canvas.history.can_undo() as usize,
            entries_before,
            "rollback tidak menciptakan entri riwayat"
        );
    }

    #[test]
    fn stroke_tercommit_dan_unggahan_tekstur_parsial() {
        let mut app = test_app();
        let ctx = test_ctx();
        frame(&mut app, &ctx, vec![]);
        let from = screen_of(&app, Vec2::new(200.0, 200.0));
        let to = screen_of(&app, Vec2::new(260.0, 220.0));
        stroke(&mut app, &ctx, from, to);

        // Piksel tengah stroke terisi (koordinat dokumen 200,200→260,220).
        assert!(app.canvas.raster.pixel(230, 210).unwrap().a > 0);

        // Riwayat: satu entri stroke; redo kosong sampai ada aksi baru.
        assert!(app.canvas.history.can_undo());
        assert!(!app.canvas.history.can_redo());
    }

    #[test]
    fn eraser_menghapus_tinta() {
        let mut app = test_app();
        let ctx = test_ctx();
        frame(&mut app, &ctx, vec![]);
        // Isi merah lalu hapus di titik itu.
        app.tool = Tool::Fill;
        app.color = Rgba::opaque(120, 60, 30);
        let p = screen_of(&app, Vec2::new(500.0, 500.0));
        click(&mut app, &ctx, p);
        assert_eq!(app.canvas.raster.pixel(500, 500).unwrap().a, 255);

        app.tool = Tool::Eraser;
        let from = screen_of(&app, Vec2::new(480.0, 500.0));
        let to = screen_of(&app, Vec2::new(540.0, 500.0));
        stroke(&mut app, &ctx, from, to);
        let px = app.canvas.raster.pixel(510, 500).unwrap();
        assert!(px.a < 255, "eraser mengurangi alpha ({} < 255)", px.a);
    }

    // ---------------------------------------------------------------------
    // M3 tahap 1 (§12.2) — model animasi + timeline X-sheet + buffer cel
    // ---------------------------------------------------------------------

    /// Posisi layar: kiri grid X-sheet (rail 48 + label layer 112).
    const GRID_LEFT: f32 = 48.0 + 112.0;
    /// Atas area grid (timeline 200 px dari bawah + transport 32 + header 20).
    const GRID_TOP: f32 = SCREEN[1] - timeline::TIMELINE_H + 32.0 + timeline::HEADER_H;
    /// Lebar sel default timeline.
    const TL_CELL_W: f32 = timeline::CELL_W;

    /// Titik tengah cell (frame, baris layer) di grid X-sheet.
    fn cell_pos(frame: u32, row: usize) -> Pos2 {
        pos2(
            GRID_LEFT + frame as f32 * TL_CELL_W + TL_CELL_W * 0.5,
            GRID_TOP + row as f32 * timeline::ROW_H + timeline::ROW_H * 0.5,
        )
    }

    /// Posisi tengah tombol transport (prev = pertama, next = kedua).
    fn transport_btn(index: usize) -> Pos2 {
        let x = 48.0 + theme::spacing::PANEL + index as f32 * (32.0 + theme::spacing::GAP_CONTROL);
        pos2(x + 16.0, SCREEN[1] - timeline::TIMELINE_H + 16.0)
    }

    #[test]
    fn timeline_klik_cell_menggerakkan_playhead() {
        let mut app = test_app();
        let ctx = test_ctx();
        frame(&mut app, &ctx, vec![]);
        assert_eq!((app.timeline.frame, app.timeline.layer), (0, 0));

        // Klik cell frame 5 baris 0 → playhead pindah.
        click(&mut app, &ctx, cell_pos(5, 0));
        assert_eq!(
            app.timeline.frame, 5,
            "klik cell menggerakkan playhead §7.7"
        );
        assert_eq!(app.timeline.layer, 0);

        // Klik tombol next/prev di transport (§8.2).
        click(&mut app, &ctx, transport_btn(1));
        assert_eq!(app.timeline.frame, 6, "next frame");
        click(&mut app, &ctx, transport_btn(0));
        assert_eq!(app.timeline.frame, 5, "prev frame");
    }

    #[test]
    fn timeline_klik_cell_layer_kedua_memilih_baris() {
        let mut app = test_app();
        let ctx = test_ctx();
        frame(&mut app, &ctx, vec![]);
        // Tambah layer lewat model (UI tambah layer menyusul M4).
        app.anim.add_layer("Line").unwrap();
        frame(&mut app, &ctx, vec![]);

        // Baris 0 = layer atas (baru), baris 1 = layer default.
        click(&mut app, &ctx, cell_pos(2, 1));
        assert_eq!(app.timeline.frame, 2);
        assert_eq!(app.timeline.layer, 1, "klik baris memilih layer");
        assert_eq!(
            app.anim.layer_id(app.timeline.layer),
            Some(1),
            "baris 1 = layer default (id 1)"
        );
    }

    #[test]
    fn menggambar_mengekspos_cel_dan_pindah_frame_menukar_buffer() {
        let mut app = test_app();
        let ctx = test_ctx();
        frame(&mut app, &ctx, vec![]);

        // Gambar merah di frame 0.
        app.color = Rgba::opaque(200, 40, 40);
        let from = screen_of(&app, Vec2::new(300.0, 300.0));
        let to = screen_of(&app, Vec2::new(360.0, 300.0));
        stroke(&mut app, &ctx, from, to);

        let layer = app.anim.layer_id(0).unwrap();
        let cel0 = app
            .anim
            .cel_id_at(layer, 0)
            .expect("menggambar mengekspos cel baru (§7.7)");
        assert_eq!(app.canvas.active_cel(), Some(cel0), "buffer = cel frame 0");

        // Pindah ke frame 1 (masih kosong) → buffer kosong, cel 0 di model.
        click(&mut app, &ctx, cell_pos(1, 0));
        assert_eq!(app.timeline.frame, 1);
        assert_eq!(
            app.canvas.active_cel(),
            None,
            "frame 1 kosong — buffer tanpa cel"
        );
        assert!(
            app.anim.cel(cel0).is_some(),
            "cel frame 0 dikembalikan ke model (raster tidak hilang)"
        );
        assert_eq!(
            app.canvas.raster.pixel(330, 300).unwrap().a,
            0,
            "buffer fresh transparan"
        );

        // Gambar biru di frame 1 → cel terpisah.
        app.color = Rgba::opaque(40, 40, 200);
        let from = screen_of(&app, Vec2::new(300.0, 300.0));
        let to = screen_of(&app, Vec2::new(360.0, 300.0));
        stroke(&mut app, &ctx, from, to);
        let cel1 = app.anim.cel_id_at(layer, 1).expect("ekspos cel frame 1");
        assert_ne!(cel0, cel1, "tiap frame punya cel unik (§13.2)");

        // Kembali ke frame 0 → isi merah utuh (buffer diambil dari model).
        click(&mut app, &ctx, cell_pos(0, 0));
        let px = app.canvas.raster.pixel(330, 300).unwrap();
        assert_eq!((px.r, px.g, px.b), (200, 40, 40), "isi frame 0 pulih");

        // Frame 1 masih biru di model.
        let back = app.anim.cel(cel1).expect("cel frame 1 tersimpan");
        assert_eq!(
            (
                back.pixel(330, 300).unwrap().r,
                back.pixel(330, 300).unwrap().b
            ),
            (40, 200),
            "isi frame 1 utuh di model"
        );
    }

    #[test]
    fn riwayat_undo_terpisah_per_cel() {
        let mut app = test_app();
        let ctx = test_ctx();
        frame(&mut app, &ctx, vec![]);

        // Stroke di frame 0 → riwayatnya milik cel 0.
        let from = screen_of(&app, Vec2::new(200.0, 200.0));
        let to = screen_of(&app, Vec2::new(260.0, 200.0));
        stroke(&mut app, &ctx, from, to);
        assert!(app.canvas.history.can_undo());

        // Pindah ke frame 1 → buffer fresh, tanpa riwayat.
        click(&mut app, &ctx, cell_pos(1, 0));
        assert!(
            !app.canvas.history.can_undo(),
            "riwayat ikut cel — frame 1 bersih"
        );
        frame(&mut app, &ctx, vec![key(egui::Key::Z, Modifiers::CTRL)]);
        assert_eq!(
            app.canvas.raster.pixel(230, 200).unwrap().a,
            0,
            "undo di frame kosong tidak berbuat apa-apa"
        );

        // Kembali ke frame 0 → riwayat cel 0 pulih, undo berfungsi.
        click(&mut app, &ctx, cell_pos(0, 0));
        assert!(app.canvas.history.can_undo(), "riwayat cel 0 pulih");
        frame(&mut app, &ctx, vec![key(egui::Key::Z, Modifiers::CTRL)]);
        assert_eq!(
            app.canvas.raster.pixel(230, 200).unwrap().a,
            0,
            "undo menghapus stroke frame 0 (per-cel)"
        );
    }

    #[test]
    fn zoom_timeline_ctrl_scroll() {
        let mut app = test_app();
        let ctx = test_ctx();
        frame(&mut app, &ctx, vec![]);

        let hover = cell_pos(3, 0);
        let w0 = app.timeline.cell_w;
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(hover),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: Vec2::new(0.0, 1.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: Modifiers::CTRL,
                },
            ],
        );
        assert!(
            app.timeline.cell_w > w0,
            "Ctrl+Scroll memperbesar zoom timeline (§7.7): {w0} → {}",
            app.timeline.cell_w
        );

        let w1 = app.timeline.cell_w;
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(hover),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: Vec2::new(0.0, -1.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: Modifiers::CTRL,
                },
            ],
        );
        assert!(
            app.timeline.cell_w < w1,
            "Ctrl+Scroll negatif mengecilkan zoom timeline"
        );
    }

    #[test]
    fn mode_fokus_menyembunyikan_timeline() {
        let mut app = test_app();
        let ctx = test_ctx();
        frame(&mut app, &ctx, vec![]);
        // Di mode fokus, klik area timeline tidak menggerakkan playhead
        // (panel tidak dirender).
        frame(&mut app, &ctx, vec![key(egui::Key::Tab, Modifiers::NONE)]);
        assert!(app.focus_mode);
        click(&mut app, &ctx, cell_pos(4, 0));
        assert_eq!(
            app.timeline.frame, 0,
            "timeline tersembunyi di mode fokus §8.2"
        );
    }
}
