# Flipbrush

> **Flip. Draw. Loop.** — aplikasi animasi 2D frame-by-frame.

Dokumen acuan tunggal (design system, spesifikasi teknis, roadmap fase
M0–M7): **[DESIGN.md](DESIGN.md)**.

## Status

| Fase | Isi | Status |
|---|---|---|
| M0 | Fondasi proyek: workspace Cargo, CI, toolchain docs | ✅ |
| M1 | Shell, tema (§2–§4), ikon (§5) | ✅ |
| M2 | Kanvas & stroke core | ✅ |
| M3 | Model animasi, timeline, `.flip` | ✅ (model + timeline + playback/onion + `.flip`/autosave) |
| M4 | UI desktop lengkap (tool rail, docks, command palette, color) | ⬜ |
| M5 | Rilis Linux 1.0 (light theme, ekspor, a11y) | ⬜ |
| M6 | Port Android (sheet, toolbar, gesture, haptics) | ⬜ |
| M7 | iOS & distribusi | ⬜ |

Rincian tiap fase: DESIGN.md §12.

## Struktur

```
flipbrush-app/       shell eframe/egui; theme.rs = satu-satunya hex literal;
                     typography.rs (gaya §3); icon.rs + build.rs (pipeline §5);
                     timeline.rs (X-sheet §7.7)
assets/              font variabel Inter + JetBrains Mono (lisensi OFL) dan
                     25 SVG ikon outline (§5)
flipbrush-core/      model domain (anim.rs: proyek, layer, frame, cel,
                     exposure) + stroke core (raster RGBA8, kuas/penghapus/fill,
                     riwayat undo) — tanpa UI/I/O
flipbrush-platform/  jembatan platform (desktop Linux, Android JNI)
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

MIT
