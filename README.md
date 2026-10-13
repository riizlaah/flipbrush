# Flipbrush

> **Flip. Draw. Loop.** — aplikasi animasi 2D frame-by-frame.

Dokumen acuan tunggal (design system, spesifikasi teknis, roadmap fase
M0–M7): **[DESIGN.md](DESIGN.md)**.

## Status

| Fase | Isi | Status |
|---|---|---|
| M0 | Fondasi proyek: workspace Cargo, CI, toolchain docs | ✅ |
| M1 | Shell, tema (§2–§4), ikon (§5) | ⬜ |
| M2 | Kanvas & stroke core | ⬜ |
| M3 | Model animasi, timeline, `.flip` | ⬜ |
| M4 | UI mobile lengkap (sheet, toolbar, color, haptics) | ⬜ |
| M5 | Rilis Android 1.0 (light theme, ekspor, a11y) | ⬜ |
| M6 | Desktop Linux (tool rail, docks, command palette) | ⬜ |
| M7 | iOS & distribusi | ⬜ |

Rincian tiap fase: DESIGN.md §12.

## Struktur

```
flipbrush-app/       shell eframe/egui + theme.rs (satu-satunya hex literal)
flipbrush-core/      model domain (proyek, frame, layer) — tanpa UI/I/O
flipbrush-platform/  jembatan platform (Android JNI, desktop Linux)
scripts/             check mutu (hex literal §10)
docs/TOOLCHAIN.md    setup Fedora + Android SDK/NDK + cargo-apk
```

## Mulai cepat

Ikuti [docs/TOOLCHAIN.md](docs/TOOLCHAIN.md), lalu:

```bash
cargo run -p flipbrush-app        # desktop
cargo apk run -p flipbrush-app --lib   # Android (device via adb)
```

## Lisensi

MIT OR Apache-2.0.
