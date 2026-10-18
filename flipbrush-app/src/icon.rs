//! Ikonografi Flipbrush — DESIGN.md §5.
//!
//! Geometri berasal dari pipeline `build.rs` (`assets/icons/*.svg` → usvg →
//! polyline flatten) dan dirender sebagai `epaint::PathShape` — tessellator
//! egui, tanpa rasterisasi bitmap. Stroke outline selalu 1,5 px layar tanpa
//! mengikuti skala (§5).
//!
//! Warna ikon (§5): `text/secondary` → `text/primary` saat hover →
//! `accent/brand` saat aktif; `text/disabled` bila nonaktif.

use eframe::egui::{
    self, Color32, CornerRadius, Key, Painter, Pos2, Rect, Response, Stroke, Ui, Vec2,
    epaint::{PathShape, PathStroke, StrokeKind},
};

use crate::theme::{Theme, radius};

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

/// viewBox semua ikon §5 (24×24).
const VIEWBOX: f32 = 24.0;
/// Stroke ikon §5 — selalu 1,5 px layar, tidak ikut diskalakan.
const STROKE_PX: f32 = 1.5;
/// Ukuran ikon pada toolbar (§5: default 20 px).
const ICON_PX: f32 = 20.0;
/// Ukuran tombol kontrol desktop (§7: default 32 px).
const BUTTON_PX: f32 = 32.0;

/// Gambar ikon ke `rect`: muat penuh (contain) dan terpusat tanpa distorsi,
/// stroke [`STROKE_PX`] px layar (§5).
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let side = rect.width().min(rect.height());
    if side <= 0.0 {
        return;
    }
    let scale = side / VIEWBOX;
    let origin = rect.center() - Vec2::splat(side * 0.5);
    for sub in icon.def().paths {
        if sub.points.len() < 2 {
            continue;
        }
        let points: Vec<Pos2> = sub
            .points
            .iter()
            .map(|&(x, y)| origin + Vec2::new(x * scale, y * scale))
            .collect();
        let stroke = PathStroke::new(STROKE_PX, color);
        if sub.closed {
            painter.add(PathShape::closed_line(points, stroke));
        } else {
            painter.add(PathShape::line(points, stroke));
        }
    }
}

/// Tombol ikon-only (§7.1 `icon-only`): ghost → latar `surface/3` saat
/// hover; ghost-active (`selected`) → latar `accent/brand-subtle` + ikon
/// `accent/brand`. Tanpa border (ghost) — isi di-override lokal sehingga
/// gaya `selected` bawaan egui (seleksi solid) tidak dipakai.
///
/// Selalu punya tooltip (§7.9, delay 500 ms), focus ring (§9), dan
/// accessible name via AccessKit (§9).
pub fn icon_button(ui: &mut Ui, theme: &Theme, icon: Icon, name: &str, selected: bool) -> Response {
    icon_button_sized(ui, theme, icon, name, selected, BUTTON_PX)
}

/// Seperti [`icon_button`] dengan sisi tombol pilihan — dipakai tool rail
/// §7.4 (item 40 px) tanpa mengubah ukuran ikon (§5: 20 px).
pub fn icon_button_sized(
    ui: &mut Ui,
    theme: &Theme,
    icon: Icon,
    name: &str,
    selected: bool,
    button_px: f32,
) -> Response {
    let mut button = egui::Button::new("")
        // `selected` → info toggle untuk pembaca layar; frame-nya sendiri
        // di-override di bawah sesuai §7.1.
        .selected(selected)
        .stroke(Stroke::NONE);
    if selected {
        button = button.fill(theme.accent_brand_subtle);
    }
    // §5: ikon 20 px; proporsional bila tombol dikecilkan/dibesarkan.
    let icon_px = (button_px * 0.625).clamp(12.0, ICON_PX);
    let response = ui.add_sized(Vec2::splat(button_px), button);
    // §9: ikut tab order — `Tab`/`Shift+Tab` berpindah antar tombol ikon;
    // aktivasi keyboard lewat [`teraktivasi`] (fokus ring di bawah).
    ui.ctx()
        .memory_mut(|m| m.interested_in_focus(response.id, response.layer_id));

    let icon_color = if !response.enabled() {
        theme.text_disabled
    } else if selected {
        theme.accent_brand
    } else if response.hovered() {
        theme.text_primary
    } else {
        theme.text_secondary
    };
    let icon_rect = Rect::from_center_size(response.rect.center(), Vec2::splat(icon_px));
    paint(ui.painter(), icon_rect, icon, icon_color);

    if response.has_focus() {
        // §9: focus ring 2 px accent/brand, offset 2 px dari tepi kontrol.
        let grow = theme.focus_ring_offset + theme.focus_ring.width * 0.5;
        ui.painter().rect_stroke(
            response.rect.expand(grow),
            CornerRadius::same(radius::SM),
            theme.focus_ring,
            StrokeKind::Middle,
        );
    }

    // §7.9: tooltip wajib untuk kontrol ikon (delay 500 ms, lihat Theme::apply_style).
    let response = response.on_hover_text(name);
    // §9: accessible name — tombol kosong tidak punya label bawaan, tulis
    // setelah Button::ui agar tidak ditimpa oleh WidgetInfo tombol.
    let _ = ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_label(name);
    });
    response
}

/// §9: aktivasi keyboard — `true` bila diklik pointer ATAU `Enter`/`Space`
/// saat tombol berfokus keyboard (tombol egui tak punya aktivasi bawaan;
/// fokus sendiri datang dari tab order — lihat `icon_button_sized`).
pub fn teraktivasi(ui: &Ui, response: &Response) -> bool {
    response.clicked()
        || (response.has_focus()
            && ui.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Space)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    const WAJIB_SECTION_5: [&str; 25] = [
        "brush",
        "eraser",
        "fill",
        "wand",
        "lasso",
        "marquee",
        "eyedropper",
        "move",
        "transform",
        "text",
        "shape",
        "pan",
        "zoom",
        "play",
        "pause",
        "prev-frame",
        "next-frame",
        "onion",
        "layer-add",
        "layer-delete",
        "undo",
        "redo",
        "save",
        "export",
        "settings",
    ];

    #[test]
    fn set_wajib_25_ikon_section_5() {
        assert_eq!(Icon::ALL.len(), 25);
        let mut names: Vec<&str> = Icon::ALL.iter().map(|icon| icon.name()).collect();
        names.sort_unstable();
        let mut expected = WAJIB_SECTION_5;
        expected.sort_unstable();
        assert_eq!(names, expected);
    }

    #[test]
    fn setiap_ikon_punya_geometri_valid() {
        for icon in Icon::ALL {
            let def = icon.def();
            assert!(!def.paths.is_empty(), "{}: tanpa geometri", icon.name());
            for sub in def.paths {
                assert!(
                    sub.points.len() >= 2,
                    "{}: subpath cuma {} titik",
                    icon.name(),
                    sub.points.len()
                );
                for &(x, y) in sub.points {
                    assert!(
                        (0.0..=24.0).contains(&x) && (0.0..=24.0).contains(&y),
                        "{}: titik ({x}, {y}) di luar viewBox 24×24",
                        icon.name()
                    );
                }
                if sub.closed {
                    assert!(
                        sub.points.len() >= 3,
                        "{}: subpath tertutup butuh ≥3 titik",
                        icon.name()
                    );
                }
            }
        }
    }

    #[test]
    fn nama_file_kebab_case_section_10() {
        for icon in Icon::ALL {
            let name = icon.name();
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "nama ikon harus kebab-case: {name}"
            );
            assert!(!name.contains("--"), "tanpa ganda tanda hubung: {name}");
        }
    }

    #[test]
    fn tombol_ikon_punya_accessible_name_section_9() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let theme = Theme::dark();

        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let _ = icon_button(ui, &theme, Icon::Brush, "brush", true);
            });
        });
        // Texture delta tidak dipakai di tes — buang agar tidak panic saat
        // TexturesDelta di-drop.
        output.textures_delta.clear();

        let update = output
            .platform_output
            .accesskit_update
            .expect("egui harus menghasilkan update AccessKit");
        let labels: Vec<&str> = update
            .nodes
            .iter()
            .filter_map(|(_, node)| node.label())
            .collect();
        assert!(
            labels.contains(&"brush"),
            "tombol ikon wajib punya accessible name (§9); label ditemukan: {labels:?}"
        );
    }
}
