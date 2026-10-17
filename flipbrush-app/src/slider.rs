//! Slider drag-numeric — DESIGN.md §7.2.
//!
//! - Track 4 px `surface-3`, isi `accent/brand`, thumb bulat 12 px
//!   `text/primary`.
//! - Label kiri (`label-md`), nilai kanan (`numeric`): **nilai bisa diseret**
//!   (presisi) dan **diklik untuk ketik manual**.
//! - Saat fokus: `←/→` ±1 step, `Shift+←/→` ±10 step.
//!
//! Komponen murni egui (bukan widget bawaan) agar seluruh visual diambil
//! dari token tema §2/§4 — tidak ada warna hard-coded (§10).

use std::ops::RangeInclusive;

use eframe::egui::{
    self, CornerRadius, EventFilter, Key, Modifiers, Pos2, Rect, Response, Sense, Ui, Vec2,
    epaint::StrokeKind,
};

use crate::theme::{Theme, radius};
use crate::typography::Type;

/// Tinggi baris slider (kontrol desktop compact §7).
const ROW_H: f32 = 28.0;
/// Lebar kolom label (§7.2: label kiri, `label-md`).
const LABEL_W: f32 = 76.0;
/// Lebar kolom nilai (§7.2: nilai kanan, `numeric`).
const VALUE_W: f32 = 52.0;
/// Lebar total satu slider — cukup untuk ukuran/opacity/hardness berjajar
/// di options bar §7.5.
const SLIDER_W: f32 = 240.0;
/// Ketebalan track (§7.2: 4 px).
const TRACK_H: f32 = 4.0;
/// Diameter thumb (§7.2: 12 px).
const THUMB_PX: f32 = 12.0;
/// Sensitivitas drag nilai (presisi): fraksi rentang per piksel gerakan.
const VALUE_DRAG_SPAN: f32 = 300.0;
/// Pengali langkah `Shift` (§7.2: ±10 step).
const SHIFT_MULTIPLIER: f32 = 10.0;

/// State mode ketik manual (§7.2: nilai diklik lalu diketik).
#[derive(Clone, Default)]
struct EditState {
    buf: String,
    /// Masih pada frame pertama — minta fokus ke `TextEdit`.
    fresh: bool,
}

/// Slider dengan nilai yang bisa diseret/diketik (§7.2).
///
/// ```ignore
/// slider::DragNumeric::new("Ukuran", &mut size, 1.0..=200.0, 1.0)
///     .suffix(" px")
///     .show(ui, &theme);
/// ```
pub struct DragNumeric<'a> {
    label: &'a str,
    value: &'a mut f32,
    range: RangeInclusive<f32>,
    step: f32,
    suffix: &'static str,
    decimals: u8,
}

impl<'a> DragNumeric<'a> {
    /// Slider `label` dengan rentang `range` dan langkah keyboard `step`.
    pub fn new(label: &'a str, value: &'a mut f32, range: RangeInclusive<f32>, step: f32) -> Self {
        Self {
            label,
            value,
            range,
            step,
            suffix: "",
            decimals: 0,
        }
    }

    /// Satuan tampilan (mis. `" px"`, `"%"`) — §7.5 options bar.
    pub fn suffix(mut self, suffix: &'static str) -> Self {
        self.suffix = suffix;
        self
    }

    /// Digit desimal tampilan (0 = bilangan bulat).
    pub fn decimals(mut self, decimals: u8) -> Self {
        self.decimals = decimals;
        self
    }

    /// Gambar slider (§7.2). Nilai di-clamp ke rentang. Mengembalikan
    /// `Response` track (fokusable — panah keyboard §7.2).
    pub fn show(self, ui: &mut Ui, theme: &Theme) -> Response {
        let mut response = None;
        ui.allocate_ui_with_layout(
            Vec2::new(SLIDER_W, ROW_H),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                response = Some(self.inner(ui, theme));
            },
        );
        // allocate_ui_with_layout selalu menjalankan closure tepat sekali.
        response.expect("closure slider selalu dijalankan")
    }

    fn inner(self, ui: &mut Ui, theme: &Theme) -> Response {
        let min = *self.range.start();
        let max = *self.range.end();
        let span = (max - min).max(f32::EPSILON);
        let step = self.step.max(f32::EPSILON);
        *self.value = (*self.value).clamp(min, max);
        let value = *self.value;

        // — Label kiri (§7.2: `label-md`) —
        ui.add_sized(
            Vec2::new(LABEL_W, ROW_H),
            egui::Label::new(Type::LabelMd.rich(self.label).color(theme.text_secondary)),
        );

        // — Track (fokusable: `Sense::click_and_drag`) —
        let track_w = ui.available_width() - VALUE_W - ui.spacing().item_spacing.x;
        let (track_rect, track) =
            ui.allocate_exact_size(Vec2::new(track_w.max(40.0), ROW_H), Sense::click_and_drag());
        let t = ((value - min) / span).clamp(0.0, 1.0);
        let bar =
            Rect::from_center_size(track_rect.center(), Vec2::new(track_rect.width(), TRACK_H));
        let thumb_x = bar.left() + bar.width() * t;
        let painter = ui.painter();
        painter.rect_filled(bar, radius::PILL, theme.surface_3);
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(bar.left(), bar.top()),
                Pos2::new(thumb_x, bar.bottom()),
            ),
            radius::PILL,
            theme.accent_brand,
        );
        painter.circle_filled(
            Pos2::new(thumb_x, bar.center().y),
            THUMB_PX * 0.5,
            theme.text_primary,
        );
        if track.has_focus() {
            // §9: focus ring 2 px accent/brand, offset 2 px.
            let grow = theme.focus_ring_offset + theme.focus_ring.width * 0.5;
            painter.rect_stroke(
                track_rect.expand(grow),
                CornerRadius::same(radius::SM),
                theme.focus_ring,
                StrokeKind::Middle,
            );
        }

        // — Nilai kanan (§7.2: `numeric`, diseret untuk presisi, diklik
        //   untuk ketik manual) —
        let edit_id = ui.make_persistent_id(("drag-numeric", self.label));
        let text_id = ui.make_persistent_id(("drag-numeric-edit", self.label));
        if let Some(mut st) = ui.data_mut(|d| d.get_temp::<EditState>(edit_id)) {
            let edit = ui.add_sized(
                Vec2::new(VALUE_W, ROW_H),
                egui::TextEdit::singleline(&mut st.buf)
                    .font(Type::Numeric.font_id())
                    .id(text_id)
                    .desired_width(VALUE_W - 4.0)
                    .margin(egui::Margin::symmetric(2, 2)),
            );
            if st.fresh {
                edit.request_focus();
                st.fresh = false;
                // §7.2: ketik manual mengganti seluruh nilai. Pola egui
                // (lihat `drag_value`): pilih-seluruh disimpan SETELAH widget
                // berjalan pada frame fokus diminta, dan bertahan ke frame
                // ketikan berikutnya sehingga ketikan mengganti isi.
                let mut text_state =
                    egui::TextEdit::load_state(ui.ctx(), text_id).unwrap_or_default();
                text_state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::default(),
                        egui::text::CCursor::new(st.buf.chars().count()),
                    )));
                text_state.store(ui.ctx(), text_id);
            }
            let esc = ui.input(|i| i.key_pressed(Key::Escape));
            let done = edit.lost_focus();
            ui.data_mut(|d| d.insert_temp(edit_id, st.clone()));
            if done {
                // Commit saat blur/Enter; Esc membatalkan (nilai tak berubah).
                if !esc && let Ok(parsed) = st.buf.trim().parse::<f32>() {
                    *self.value = parsed.clamp(min, max);
                }
                ui.data_mut(|d| d.remove_temp::<EditState>(edit_id));
            }
        } else {
            let (rect, value_response) =
                ui.allocate_exact_size(Vec2::new(VALUE_W, ROW_H), Sense::CLICK | Sense::DRAG);
            ui.painter().text(
                rect.right_center(),
                egui::Align2::RIGHT_CENTER,
                self.format(value),
                Type::Numeric.font_id(),
                theme.text_primary,
            );
            if value_response.dragged() {
                // §7.2: nilai diseret untuk presisi (lebih halus dari track).
                let fine = span / VALUE_DRAG_SPAN;
                *self.value = (value + value_response.drag_delta().x * fine).clamp(min, max);
            } else if value_response.clicked() {
                // §7.2: diklik untuk ketik manual (tanpa satuan).
                let state = EditState {
                    buf: format!("{:.*}", self.decimals as usize, value),
                    fresh: true,
                };
                ui.data_mut(|d| d.insert_temp(edit_id, state));
            }
        }

        // — Drag track: posisi pointer = nilai (§7.2) —
        if track.dragged()
            && let Some(pos) = track.interact_pointer_pos()
        {
            let t = ((pos.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
            *self.value = (min + t * span).clamp(min, max);
        }

        // — Keyboard saat fokus: ±1 step, Shift ±10 step (§7.2) —
        if track.has_focus() {
            // Kunci arrow horizontal agar tidak dipakai navigasi fokus egui.
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    track.id,
                    EventFilter {
                        horizontal_arrows: true,
                        ..EventFilter::default()
                    },
                )
            });
            let delta = ui.input_mut(|i| {
                // `matches_logically` mengabaikan Shift ekstra, jadi cek
                // Shift dulu lalu polos (konsumsi: shortcut spesifik dulu).
                let mut d = 0.0;
                if i.consume_key(Modifiers::SHIFT, Key::ArrowLeft) {
                    d -= step * SHIFT_MULTIPLIER;
                } else if i.consume_key(Modifiers::NONE, Key::ArrowLeft) {
                    d -= step;
                }
                if i.consume_key(Modifiers::SHIFT, Key::ArrowRight) {
                    d += step * SHIFT_MULTIPLIER;
                } else if i.consume_key(Modifiers::NONE, Key::ArrowRight) {
                    d += step;
                }
                d
            });
            if delta != 0.0 {
                // Snap ke grid langkah agar nilai bulat tetap bulat.
                let snapped = ((value + delta) / step).round() * step;
                *self.value = snapped.clamp(min, max);
            }
        }

        track
    }

    fn format(&self, value: f32) -> String {
        format!("{:.*}{}", self.decimals as usize, value, self.suffix)
    }
}

/// Lihat [`DragNumeric::new`] — helper ringkas tanpa satuan/desimal.
pub fn drag_numeric(
    ui: &mut Ui,
    theme: &Theme,
    label: &str,
    value: &mut f32,
    range: RangeInclusive<f32>,
    step: f32,
) -> Response {
    DragNumeric::new(label, value, range, step).show(ui, theme)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Setup egui headless dengan font §3 + gaya teks (seperti startup).
    fn test_ctx() -> egui::Context {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::typography::definitions());
        let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
        crate::typography::apply_text_styles(&mut style);
        ctx.set_style_of(egui::Theme::Dark, style);
        ctx
    }

    /// Frame dengan `events`; buang texture delta (lihat tes M1: TexturesDelta
    /// di-drop tidak boleh panic).
    fn frame_with(
        ctx: &egui::Context,
        value: &mut f32,
        events: Vec<egui::Event>,
        range: RangeInclusive<f32>,
    ) -> Rect {
        let theme = Theme::dark();
        let mut track_rect = Rect::NOTHING;
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    let r = DragNumeric::new("Ukuran", value, range.clone(), 1.0).show(ui, &theme);
                    track_rect = r.rect;
                });
        });
        output.textures_delta.clear();
        track_rect
    }

    #[test]
    fn format_nilai_dengan_suffix() {
        let mut v = 12.0;
        assert_eq!(
            DragNumeric::new("Ukuran", &mut v, 1.0..=200.0, 1.0).format(12.0),
            "12"
        );
        assert_eq!(
            DragNumeric::new("Opasitas", &mut v, 0.0..=100.0, 1.0)
                .suffix("%")
                .format(85.0),
            "85%"
        );
    }

    #[test]
    fn keyboard_panah_1_dan_shift_10() {
        let ctx = test_ctx();
        let mut value = 12.0;

        // Frame 1: render; frame 2: request_focus pada track.
        frame_with(&ctx, &mut value, vec![], 1.0..=200.0);
        let mut input = egui::RawInput::default();
        let theme = Theme::dark();
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let r = drag_numeric(ui, &theme, "Ukuran", &mut value, 1.0..=200.0, 1.0);
                r.request_focus();
            });
        });
        output.textures_delta.clear();

        // Frame: ArrowRight → +1.
        input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
        output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let _ = drag_numeric(ui, &theme, "Ukuran", &mut value, 1.0..=200.0, 1.0);
            });
        });
        output.textures_delta.clear();
        assert_eq!(value, 13.0, "→ ±1 step (§7.2)");

        // Frame: Shift+ArrowLeft → −10.
        input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        });
        output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let _ = drag_numeric(ui, &theme, "Ukuran", &mut value, 1.0..=200.0, 1.0);
            });
        });
        output.textures_delta.clear();
        assert_eq!(value, 3.0, "Shift+← ±10 step (§7.2)");

        // Frame: Shift+ArrowRight → +10.
        input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        });
        output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let _ = drag_numeric(ui, &theme, "Ukuran", &mut value, 1.0..=200.0, 1.0);
            });
        });
        output.textures_delta.clear();
        assert_eq!(value, 13.0, "Shift+→ +10 step (§7.2)");
    }

    #[test]
    fn drag_track_mengubah_nilai_proporsional() {
        let ctx = test_ctx();
        let mut value = 0.0;
        // Frame 1: catat rect track.
        let track = frame_with(&ctx, &mut value, vec![], 0.0..=100.0);

        // Frame 2–4: tekan di 25% track, geser ke 75%, lepas.
        let y = track.center().y;
        let x0 = track.left() + track.width() * 0.25;
        let x1 = track.left() + track.width() * 0.75;
        for (i, x) in [x0, x1, x1].into_iter().enumerate() {
            let pos = Pos2::new(x, y);
            let mut events = vec![egui::Event::PointerMoved(pos)];
            if i == 0 {
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                });
            }
            if i == 2 {
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                });
            }
            frame_with(&ctx, &mut value, events, 0.0..=100.0);
        }
        assert!(
            (value - 75.0).abs() < 2.0,
            "drag ke 75% track → nilai ≈75 (§7.2), dapat {value}"
        );
    }

    #[test]
    fn drag_nilai_lebih_halus_dari_track() {
        let ctx = test_ctx();
        let mut value = 10.0;
        let track = frame_with(&ctx, &mut value, vec![], 1.0..=200.0);
        // Kolom nilai = kanan track + item spacing (LEBAR_KOLOM di tes).
        let spacing_x = ctx.style_of(egui::Theme::Dark).spacing.item_spacing.x;
        let value_center = Pos2::new(track.right() + spacing_x + VALUE_W * 0.5, track.center().y);

        // Geser 10 px di kolom nilai (tanpa lepas): presisi ≈ +6.6.
        for (i, dx) in [0.0, 5.0, 10.0].into_iter().enumerate() {
            let pos = Pos2::new(value_center.x + dx, value_center.y);
            let mut events = vec![egui::Event::PointerMoved(pos)];
            if i == 0 {
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                });
            }
            frame_with(&ctx, &mut value, events, 1.0..=200.0);
        }
        let delta = value - 10.0;
        assert!(
            delta > 0.0 && delta < 12.0,
            "drag nilai presisi: 10 px ≈ +6.6 (rentang 199/300 per px), dapat {delta}"
        );
    }

    #[test]
    fn klik_nilai_lalu_ketik_manual() {
        let ctx = test_ctx();
        let mut value = 12.0;
        let track = frame_with(&ctx, &mut value, vec![], 1.0..=200.0);
        let spacing_x = ctx.style_of(egui::Theme::Dark).spacing.item_spacing.x;
        let center = Pos2::new(track.right() + spacing_x + VALUE_W * 0.5, track.center().y);

        // Klik kolom nilai.
        let mut events = vec![egui::Event::PointerMoved(center)];
        for pressed in [true, false] {
            events.push(egui::Event::PointerButton {
                pos: center,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        frame_with(&ctx, &mut value, events, 1.0..=200.0);
        // Frame kosong: TextEdit muncul + request_focus.
        frame_with(&ctx, &mut value, vec![], 1.0..=200.0);
        // Ketik "64" lalu Enter.
        frame_with(
            &ctx,
            &mut value,
            vec![egui::Event::Text("6".into()), egui::Event::Text("4".into())],
            1.0..=200.0,
        );
        frame_with(
            &ctx,
            &mut value,
            vec![egui::Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            1.0..=200.0,
        );
        assert_eq!(value, 64.0, "ketik manual (§7.2) meng-commit nilai");
    }

    #[test]
    fn nilai_dilimit_ke_rentang() {
        let ctx = test_ctx();
        let mut value = 500.0;
        frame_with(&ctx, &mut value, vec![], 1.0..=200.0);
        assert_eq!(value, 200.0, "nilai di-clamp ke rentang atas");
    }
}
