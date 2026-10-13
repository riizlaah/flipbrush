# Flipbrush Design System v1

Dokumen acuan tunggal untuk seluruh aspek visual dan interaksi **Flipbrush**
— aplikasi animasi 2D frame-by-frame, tagline _"Flip. Draw. Loop."_

Berlaku untuk build **Android (prioritas utama)**, serta desktop Linux
(target sekunder, M6) dan iOS (menyusul). Package Android: `app.flipbrush`,
ekstensi proyek: `.flip`.

---

## 1. Prinsip

1. **Canvas-first** — Kanvas adalah raja. UI hanya "chrome" setipis mungkin, dan bisa
   disembunyikan sepenuhnya (mode fokus menggambar: desktop `Tab`; mobile chrome
   menyusut menjadi tombol playhead mengambang).
2. **Mobile-first** — Android adalah platform utama. Setiap keputusan UI divalidasi
   dulu pada layar sentuh; shell desktop (M6) memakai komponen yang sama dengan
   layout berbeda.
3. **Netral agar mata tidak tertipu** — Tidak ada warna chromatic di belakang atau di
   sekeliling kanvas. Semua permukaan menggunakan skala abu-abu netral agar pemilihan
   warna oleh pengguna tidak terpengaruh lingkungan UI.
4. **Presisi** — Kontrol harus mendukung input presisi: drag-numeric pada slider,
   angka tabular, koordinat yang jelas; di mobile ditambah edge-scrub (mengubah
   ukuran/hardness brush langsung di tepi layar tanpa membuka panel).
5. **Input-aware** — Mobile: touch target ≥ 44 px, tanpa ketergantungan hover,
   gesture-driven. Desktop (sekunder): hover states, shortcut keyboard penuh, dense.
6. **Aksen tunggal** — Hanya satu warna aksen (`accent/brand`) untuk chrome. Aksen tidak
   pernah mewarnai area kanvas atau permukaan besar.

---

## 2. Warna

### 2.1 Skala netral (dark theme — default)

| Token | Nilai | Pemakaian |
|---|---|---|
| `bg/canvas-surround` | `#3B3B3E` | Area abu di sekeliling kanvas (netral, bukan chromatic) |
| `bg/surface-0` | `#1C1C1F` | Background aplikasi |
| `bg/surface-1` | `#262629` | Panel, dock, toolbar |
| `bg/surface-2` | `#2F2F33` | Input field, cell timeline, kontrol |
| `bg/surface-3` | `#3A3A40` | Hover, raised, popover |
| `border/default` | `#35353A` | Garis pemisah antar panel |
| `border/strong` | `#4A4A51` | Border kontrol, divider aktif |
| `text/primary` | `#EDEDEF` | Teks utama (kontras AA 4.5:1 di atas surface-1) |
| `text/secondary` | `#9A9AA1` | Label, teks sekunder, ikon nonaktif |
| `text/disabled` | `#6A6A70` | Elemen nonaktif |

### 2.2 Aksen & semantik

| Token | Nilai | Pemakaian |
|---|---|---|
| `accent/brand` | `#6E7BF2` (indigo) | Tool aktif, tab aktif, fokus, tombol primer |
| `accent/brand-hover` | `#8289F5` | Hover aksen |
| `accent/brand-subtle` | `#6E7BF2` @ 12% | Latar tool aktif, highlight seleksi ringan |
| `danger` | `#E5484D` | Delete, error, destructive action |
| `warning` | `#F5A524` | Auto-save gagal, peringatan recover |
| `success` | `#30A46C` | Export sukses, autosave OK |
| `focus/ring` | `accent/brand`, 2 px | Focus ring (offset 2 px) |

### 2.3 Warna animasi & timeline

| Token | Nilai | Pemakaian |
|---|---|---|
| `onion/prev` | `#FF6B6B` | Onion skin frame sebelumnya |
| `onion/next` | `#4EA8FF` | Onion skin frame berikutnya |
| `onion/skin-fill` | `#FFFFFF` @ 8% | Overlay light table |
| `timeline/cel-empty` | `#2A2A2E` | Cell kosong |
| `timeline/cel-exposed` | `#3E4C63` | Cell berisi bitmap (biru redup, bukan aksen) |
| `timeline/cel-selected` | `accent/brand` @ 30% | Cell / rentang terpilih |
| `timeline/cel-shared` | `#3E4C63` + strip kiri | Awal exposure; cell lanjutan hanya strip tipis |
| `timeline/playhead` | `text/primary` | Garis playhead |
| `timeline/marker` | `#F5A524` | Marker & loop range |
| `selection/quickmask` | `#FF0000` @ 50% | Overlay quick mask |

### 2.4 Accent picker (tema kustom)

Pengguna dapat memilih aksen chrome — **hanya mengubah chrome, tidak kanvas**:

| Nama | Nilai |
|---|---|
| Indigo (default) | `#6E7BF2` |
| Teal | `#2BC4B6` |
| Magenta | `#D6409F` |
| Orange | `#E8743B` |

### 2.5 Light theme (opsional, tersedia di M5)

| Token | Nilai |
|---|---|
| `bg/surface-0` | `#F5F5F7` |
| `bg/surface-1` | `#FFFFFF` |
| `bg/surface-2` | `#EDEDF0` |
| `bg/surface-3` | `#E2E2E7` |
| `border/default` | `#DCDCE2` |
| `text/primary` | `#17171A` |
| `text/secondary` | `#63636B` |
| `bg/canvas-surround` | `#C9C9CE` (tetap netral) |

Aksen & warna animasi tidak berubah antar tema.

Di Android, tema gelap/terang **mengikuti sistem** secara default (setting
"Ikut sistem"); desktop selalu default gelap.

---

## 3. Tipografi

- **UI font**: Inter (fallback: `system-ui, sans-serif`).
- **Numeric font**: JetBrains Mono, selalu *tabular figures* (angka tidak "melompat"
  saat nilai berubah pada slider/timecode/koordinat).

| Gaya | Font | Size | Weight | Line-height | Pemakaian |
|---|---|---|---|---|---|
| `label-sm` | Inter | 11 px | 500 | 1.2 | Label panel, badge |
| `label-md` | Inter | 12 px | 500 | 1.2 | Label kontrol, tab |
| `body` | Inter | 13 px | 400 | 1.4 | Teks UI umum, tooltip |
| `body-lg` | Inter | 14 px | 400 | 1.4 | Dialog, empty state |
| `heading-sm` | Inter | 16 px | 600 | 1.2 | Judul dialog kecil |
| `heading-md` | Inter | 20 px | 600 | 1.2 | Judul panel besar |
| `heading-lg` | Inter | 24 px | 600 | 1.2 | Splash / onboarding |
| `numeric` | JetBrains Mono | 12 px | 400 | 1.2 | Nilai slider, timecode, koordinat |
| `numeric-lg` | JetBrains Mono | 13 px | 500 | 1.2 | Angka besar (frame saat ini, FPS) |

Aturan:
- Letter-spacing `0` di semua gaya.
- Huruf besar (uppercase) hanya untuk badge/shortcut hint, dengan tracking `+0.4 px`.
- Maksimum 2 ukuran font berbeda dalam satu panel.

---

## 4. Spacing, Radius, Elevation

### 4.1 Grid spacing — basis 4 px

`4 / 8 / 12 / 16 / 24 / 32`

- `4` — jarak ikon ke label, padding cell timeline.
- `8` — padding internal kontrol, jarak antar kontrol dalam satu baris.
- `12` — padding item list, jarak antar grup kontrol.
- `16` — padding panel, padding dialog.
- `24` — jarak antar section dalam dialog.
- `32` — spasi besar (empty state, splash).

### 4.2 Radius

| Token | Nilai | Pemakaian |
|---|---|---|
| `radius/sm` | 4 px | Button, input, cell timeline, tooltip |
| `radius/md` | 8 px | Panel, modal, popover, card |
| `radius/pill` | 999 px | Chip, badge, toggle track |

### 4.3 Elevation

Dark theme mayoritas memakai **border**, bukan shadow:

| Token | Nilai | Pemakaian |
|---|---|---|
| `shadow/popover` | `0 4px 12px rgba(0,0,0,0.35)` | Context menu, dropdown |
| `shadow/modal` | `0 8px 24px rgba(0,0,0,0.40)` | Dialog, command palette |
| `shadow/drag` | `0 2px 8px rgba(0,0,0,0.30)` | Item yang di-drag (ghost) |

---

## 5. Ikonografi

- Style: **outline**, stroke **1.5 px**, linecap/linejoin `round`.
- Ukuran: 16 px (inline, cell timeline), 20 px (default toolbar), 24 px (mobile toolbar).
- Satu sumber: file SVG di `assets/icons/`, di-build ke tessellator egui saat compile.
- Warna mengikuti `text/secondary`; `text/primary` saat hover; `accent/brand` saat aktif.
- Set minimum wajib: brush, eraser, fill, wand, lasso, marquee, eyedropper, move,
  transform, text, shape, pan, zoom, play, pause, prev-frame, next-frame, onion,
  layer-add, layer-delete, undo, redo, save, export, settings.

---

## 6. Motion

| Token | Nilai | Pemakaian |
|---|---|---|
| `motion/fast` | 120 ms, `ease-out` | Hover, toggle, Tool aktif |
| `motion/base` | 180 ms, `ease-out` | Panel slide, tab switch, toast masuk/keluar |
| `motion/playback` | sinkron FPS | Playhead, flipbook |

Aturan:
- Motion hanya untuk **feedback fungsional** — tidak ada dekorasi.
- Semua motion dimatikan saat `prefers-reduced-motion: reduce`.
- Tidak ada animasi yang menunda input pengguna (stroke selalu instan).

**Haptics (mobile):** getaran ringan ~10 ms hanya pada: ganti tool, langkah frame
(`prev`/`next`), undo/redo, toggle onion. Tanpa getaran saat menggambar, scrub, atau
ketuk biasa. Semua haptics bisa dimatikan di settings (accessibility).

---

## 7. Komponen

Semua komponen punya state: `default`, `hover` (desktop), `active/pressed`,
`focused`, `disabled`. State mobile tambahan: `sheet-peek/half/full` (bottom-sheet) dan
`gesture-modifier` (saat gesture multi-jari aktif, chrome redup ke 40% dan stroke
disuspend). Ukuran kontrol: mobile **44 px** minimum (touch target), desktop **28 px**
(compact) / **32 px** (default).

### 7.1 Button

| Variant | Latar | Teks/ikon | Pemakaian |
|---|---|---|---|
| `ghost` | transparan → `surface-3` hover | `text/secondary` → `primary` hover | Tool rail, toolbar |
| `ghost-active` | `accent/brand-subtle` | `accent/brand` | Tool terpilih, toggle aktif |
| `solid` | `accent/brand` | putih | Aksi primer (Save, Export) |
| `danger` | `danger` | putih | Hapus layer / destructive |
| `icon-only` | seperti ghost | ikon 20 px | Toolbar (wajib punya tooltip + shortcut) |

### 7.2 Slider (drag-numeric)

- Track 4 px, `surface-3`; isi `accent/brand`; thumb bulat 12 px, `text/primary`.
- Label kiri (`label-md`), nilai kanan (`numeric`) — **nilai bisa diseret** untuk
  presisi, dan diklik untuk ketik manual.
- Shortcut saat fokus: `←/→` ±1 step, `Shift+←/→` ±10 step.

### 7.3 Color wheel & swatches

- Wheel HSV (diameter 160 px desktop) + lightness strip + hex/RGB input.
- Swatch bar: 16 slot, klik = pilih, drag = reorder, `Shift+Klik` = hapus.
- Palet harmony (complementary, analogous, triadic) sebagai panel terpisah.

### 7.4 Tool rail (desktop sekunder — M6)

- Lebar 48 px, kiri, `bg/surface-1`, item 40×40 px.
- Item aktif: `ghost-active` + strip aksen 2 px di tepi kiri rail.
- Grup dipisah divider `border/default`; tool dengan submenu panah kecil.

### 7.5 Options bar (desktop sekunder — M6)

- Tinggi 40 px, di atas kanvas, berisi kontrol spesifik tool aktif
  (mis. brush: size, opacity, hardness, spacing, preset picker).
- Kontrol dipisah divider vertikal `border/default`.

### 7.6 Panel & dock

- `bg/surface-1`, radius `radius/md` bila floating, tanpa radius bila docked.
- Header panel: `label-md` + tombol collapse (chevron) + menu (⋮).
- Panel bisa di-collapse (hanya header) dan disembunyikan (`Tab` = fokus mode).

### 7.7 Timeline / X-sheet

- **Mobile (utama)**: tampilan harian berupa **strip frame horizontal** (lihat §8.1);
  grid X-sheet penuh dibuka dari bottom-sheet timeline.
- Grid desktop: baris = layer, kolom = frame. Header kolom: `numeric`.
- Cell exposed: `timeline/cel-exposed`; awal exposure punya strip kiri 2 px lebih terang;
  cell lanjutan exposure hanya strip tanpa isi.
- Playhead: garis 1 px `text/primary` + kepala segitiga.
- Zoom timeline: pinch (mobile) / `Ctrl+Scroll` (desktop); scrub: seret playhead;
  loop range: drag di bar atas.

### 7.8 Menu, dialog, toast

- Context menu: `bg/surface-3`, radius `radius/sm`, item 28 px, shortcut hint kanan
  (`numeric`, `text/disabled`).
- Dialog: max-width 480 px, `bg/surface-1`, radius `radius/md`, `shadow/modal`,
  padding 24 px; aksi di kanan bawah (batal kiri, primer kanan).
- Toast: bawah kanan, `bg/surface-3`, auto-dismiss 3 s, max 3 tumpuk.

### 7.9 Tooltip

- `bg/surface-3`, `body` 12 px, radius `radius/sm`, delay 500 ms (0 ms saat sudah
  pernah muncul untuk elemen yang sama).
- Selalu menyertakan shortcut bila ada.

### 7.10 Command palette (`Ctrl+K`, desktop sekunder — M6)

- Modal terpusat, lebar 560 px, input `body-lg`, hasil `label-md` + shortcut kanan.
- Sumber: semua command + recent tools + file recent.

### 7.11 Empty state

- Ikon outline 32 px `text/disabled`, teks `body-lg` `text/secondary`,
  satu aksi `solid` bila relevan. Contoh: "Drag gambar ke sini untuk referensi".

### 7.12 Bottom toolbar (mobile — primary)

- Tinggi 64 px + safe-area inset, `bg/surface-1`, border atas `border/default`.
- Ikon 24 px, touch target ≥ 48 px; tool aktif: `ghost-active`.
- Maksimal 5 item utama + tombol "lebih banyak" (⋯) yang membuka sheet tool lengkap.
- Saat menggambar chrome menyusut (lihat §8.1); saat gesture multi-jari aktif seluruh
  chrome redup ke 40% opasitas (`gesture-modifier`).

### 7.13 Bottom-sheet (mobile — primary)

- Radius atas `radius/md`, drag-handle 36×4 px `border/strong` di tengah atas.
- Tiga state snap: **peek 25% / half 50% / full 90%** — drag untuk pindah state,
  ketuk handle cycle peek → half → full, swipe turun dari full = tutup.
- Konten: Color / Brush / Layers / Timeline — tampil sebagai tab saat sheet `full`.
- Hanya handle yang menangkap gesture sheet; area konten yang discroll tidak
  "mencuri" gesture dari kanvas, dan sebaliknya gesture kanvas tidak masuk ke sheet.

---

## 8. Layout

### 8.1 Mobile (Android — prioritas utama)

```
┌─────────────────────────────────┐
│ Sistem bar (edge-to-edge,       │
│ mengikuti tema)                 │
│                                 │
│         K A N V A S             │
│                                 │
│┌───────────────────────────────┐│
│ │ Timeline strip (72 px):      ││
│ │ ▭▭▭ playhead  ⏮  ▶  ⏭      ││
│ └───────────────────────────────┘│
│┌───────────────────────────────┐│
│ │ Bottom toolbar (64 px +      ││
│ │ safe-area): ✎ ⬤ ◨ ⌫ ⋯      ││
│ └───────────────────────────────┘│
└─────────────────────────────────┘
```

- Kanvas full-screen; **bottom toolbar** (§7.12) berisi tool utama (ikon 24 px).
- **Timeline strip horizontal** di atas toolbar (tinggi 72 px): thumbnail frame,
  playhead, tombol ± frame. Ketuk strip → **bottom-sheet timeline** (half/full)
  berisi grid X-sheet & editing exposure.
- Panel Color / Brush / Layers sebagai **bottom-sheet** (§7.13).
- Sistem bar (status & navigasi) **edge-to-edge** dan mengikuti tema; kanvas dan
  chrome menyisipkan safe-area inset (notch/punch-hole/gesture nav).
- Gesture:

  | Gesture | Aksi |
  |---|---|
  | 1 jari (tool brush) | Menggambar |
  | 1 jari (tool pan) / 2 jari drag | Pan |
  | Pinch | Zoom kanvas (pinch di strip = zoom timeline) |
  | 2 jari tap | Undo |
  | 3 jari tap | Redo |
  | Tap lama di kanvas | Eyedropper |
  | Edge-scrub di tepi layar | Ukuran / hardness brush |

- Playback: tombol play besar 56 px mengambang di atas bottom toolbar.
- Mode fokus menggambar: chrome menyusut menjadi tombol playhead mengambang;
  ketuk tombol tersebut untuk mengembalikan chrome.
- Font-scale sistem dihormati hingga 130% — layout tidak terpotong.

### 8.2 Desktop (target sekunder — M6, Linux)

```
┌──────────────────────────────────────────────────────────────┐
│ Menu bar / title (32 px)                                     │
├────┬─────────────────────────────────────┬───────────────────┤
│ T  │ Options bar (40 px)                 │ Panel dock        │
│ o  ├─────────────────────────────────────┤ (Color, Brush,    │
│ o  │                                     │  Layers)          │
│ l  │           K A N V A S               │ (280 px, bisa     │
│ r  │        (+ navigator mini,           │  di-collapse)     │
│ a  │          kanan bawah)               │                   │
│ i  │                                     │                   │
│ l  ├─────────────────────────────────────┴───────────────────┤
│48px│ Timeline dock (bisa di-tab dengan Layers) — 200 px      │
└────┴─────────────────────────────────────────────────────────┘
```

- Semua panel bisa disembunyikan (`Tab` → mode fokus: hanya kanvas + floating toolbar
  mini playhead).
- Ukuran dock bisa di-resize; state layout persist per proyek.
- HiDPI: semua ukuran dalam logical px, skala mengikuti faktor DPI sistem.
- Komponen desktop (tool rail §7.4, options bar §7.5, command palette §7.10) aktif
  di sini; token & komponen mobile tetap dipakai ulang oleh shell desktop.

---

## 9. Accessibility

- Kontras teks minimum **4.5:1** (body) / **3:1** (large text & ikon).
- Focus ring selalu terlihat: 2 px `accent/brand`, offset 2 px.
- Navigasi keyboard penuh di desktop (tab order logis, `Esc` selalu menutup).
- Onion skin default merah/biru → aman untuk buta warna merah-hijau.
- Semua kontrol ikon wajib punya accessible name (egui label/ARIA).
- `prefers-reduced-motion` dan `prefers-contrast` dihormati.
- Sistem font-scale dihormati hingga 130% (mobile) — layout tidak terpotong.
- Haptics bisa dimatikan di settings (accessibility).

---

## 10. Konvensi Penamaan & Tema

- Token didefinisikan sekali di `flipbrush-app/src/theme.rs` sebagai konstanta bertipe
  (mis. `struct Theme { surface_0: Color32, ... }`), di-`resolve` saat startup.
- Tidak boleh ada hex literal di luar `theme.rs` — semua komponen memakai token.
- Tema (dark/light + accent): desktop di `~/.config/flipbrush/settings.toml`;
  Android lewat settings app, default ikut sistem.
- Ikon SVG dinamai `kebab-case.svg` sesuai nama tool, satu file satu ikon.

---

## 11. Checklis rilis design system

- [ ] Semua token terdefinisi & tidak ada hard-coded color di UI.
- [ ] Kontras teks lolos audit (axe/manual).
- [ ] Semua tombol ikon punya accessible name; desktop juga tooltip + shortcut.
- [ ] Motion dimatikan pada `prefers-reduced-motion`.
- [ ] Dark mode mengikuti sistem di Android; settings tema persist.
- [ ] Safe-area inset & gesture-nav Android teruji (notch/punch-hole).
- [ ] Sistem font-scale hingga 130% tidak merusak layout.
- [ ] Touch target ≥ 44 px di seluruh UI mobile.
- [ ] Haptics bekerja dan bisa dimatikan di settings.
- [ ] Layout desktop tersimpan/dipulihkan antar sesi.
- [ ] Dark & light theme lolos uji visual di 100%/200% DPI.

Pemetaan item ke fase verifikasi ada di §12.3.

---

## 12. Fase Pengembangan

Setiap fase diakhiri dengan build yang bisa dijalankan (APK di device nyata / binary
Fedora) dan checklist §11 diverifikasi pada fase yang ditentukan. CI menjalankan
`clippy`, `rustfmt`, test, dan test dilarangnya hex literal di luar `theme.rs`.

### 12.1 Ringkasan

| Fase | Nama | Rujukan § | Keluaran |
|---|---|---|---|
| **M0** | Fondasi proyek | — | CI hijau, APK hello-world ter-install |
| **M1** | Shell, tema, ikon | §2–§6, §8.1 | Design system terpasang, 25 ikon, jalan di Android + Fedora |
| **M2** | Kanvas & stroke core | §1, §7.2, §8.1 (gesture) | Menggambar instan, gesture dasar, edge-scrub |
| **M3** | Model animasi | §7.7, §8.1 (strip), §2.3 | Frame/timeline/playback/onion, `.flip` + autosave |
| **M4** | UI mobile lengkap | §7, §6 (haptics) | Sheet, toolbar, color, panel, toolset lengkap |
| **M5** | Rilis Android 1.0 | §2.4–2.5, §9, §11 | Light theme, ekspor, audit a11y — **seluruh §11 lolos** |
| **M6** | Desktop Linux | §7.4–7.5, §7.10, §8.2 | Tool rail, options bar, docks, command palette |
| **M7** | iOS & distribusi | §1 | Port iOS; wrapper Gradle untuk AAB bila perlu toko aplikasi |

### 12.2 Rincian per fase

**M0 — Fondasi proyek**

- Workspace Cargo (`flipbrush-app`, `flipbrush-core`, `flipbrush-platform`),
  `rust-toolchain.toml`, CI (clippy/rustfmt/test).
- Dokumentasi toolchain Fedora + Android SDK/NDK + `cargo-apk`.
- **Selesai**: `cargo apk build` menghasilkan APK yang ter-install via `adb` di
  device nyata.

**M1 — Shell, tema, ikon**

- `theme.rs`: seluruh token §2–§4 sebagai konstanta bertipe, di-resolve saat
  startup (§10).
- Tipografi §3 (Inter + JetBrains Mono, tabular); pipeline ikon §5 (`build.rs`
  usvg → tessellator egui).
- Shell `eframe`/wgpu: Android (`android-native-activity`) + Linux; edge-to-edge &
  safe-area §8.1; motion token §6.
- **Selesai**: ✔ §11 "token & tanpa hard-coded color" (ditegakkan CI), ✔ ikon punya
  accessible name, jendela Fedora + APK Android jalan.

**M2 — Kanvas & stroke core**

- Kanvas full-screen: pinch-zoom, pan, 2-finger drag, 2-tap undo, 3-tap redo,
  tap-lama eyedropper (§8.1).
- Tool: brush, eraser, fill, eyedropper; slider drag-numeric §7.2; edge-scrub
  ukuran/hardness (§1.4); mode fokus chrome menyusut (§8.1).
- **Selesai**: stroke selalu instan (§6 — tidak ada I/O blocking di UI thread);
  touch target ≥ 44 px. *Prototype arbitrase gesture sheet↔kanvas dimulai di sini
  (risiko M4)*.

**M3 — Model animasi**

- Model frame/layer/exposure; timeline strip 72 px + playhead + tombol ± frame +
  tombol play 56 px (§8.1); grid X-sheet di bottom-sheet dengan state cell §7.7.
- Onion skin (§2.3, merah/biru aman buta warna); playback sinkron FPS.
- Format `.flip` + autosave/recovery (lihat §13.2).
- **Selesai**: loop stabil, recover setelah kill proses, zoom timeline
  pinch/`Ctrl+Scroll`.

**M4 — UI mobile lengkap** *(fase terbesar; boleh dipecah M4a/M4b tanpa menggeser
M5/M6)*

- Bottom-sheet infra §7.13 (peek/half/full, drag-handle, gesture disambiguation);
  bottom toolbar §7.12 + sheet tool; panel Brush/Layers §7.6;
  color wheel/swatch/harmony §7.3; komponen §7.1/7.8/7.9/7.11; haptics §6 +
  `gesture-modifier` 40%.
- Toolset lanjutan: lasso, marquee, wand, shape, transform, text.
- **Selesai**: semua komponen mobile punya state lengkap (§7); haptics sesuai
  aturan §6 (tanpa getar saat menggambar/scrub).

**M5 — Rilis Android 1.0**

- Light theme §2.5 + accent picker §2.4; settings ikut sistem (§2.5); ekspor
  (§13.1); share/open intent `.flip`.
- Toggle haptics (aksesibilitas §9); uji `prefers-reduced-motion`/
  `prefers-contrast`; font-scale 130%; audit kontras 4.5:1/3:1; uji
  notch/punch-hole.
- **Selesai**: **seluruh checklist §11 lolos** (bagian mobile) + uji visual
  100%/200% DPI.

**M6 — Desktop Linux**

- Tool rail 48 px §7.4; options bar 40 px §7.5; dock 280 px + timeline dock 200 px,
  resize & persist per proyek §8.2; command palette `Ctrl+K` §7.10; hover, shortcut
  penuh, dense (§1.5); `Tab` mode fokus; HiDPI logical px.
- **Selesai**: token dipakai ulang tanpa hex baru; layout restore antar sesi;
  keyboard nav penuh + `Esc` menutup (§9).

**M7 — iOS & distribusi**

- Port iOS (macOS/Xcode — *menyusul* §1); bila perlu rilis toko aplikasi: wrapper
  Gradle tipis untuk AAB (`cargo-apk` hanya menghasilkan APK sideload).

### 12.3 Definition of Done & pemetaan §11 → fase

| Checklist §11 | Fase verifikasi |
|---|---|
| Token terdefinisi, tanpa hard-coded color | M1 (CI) |
| Ikon punya accessible name (+tooltip/shortcut desktop) | M1 / M6 |
| Touch target ≥ 44 px; haptics bekerja | M4 |
| Kontras, reduced-motion, font-scale 130%, light+dark DPI | M5 |
| Dark ikut sistem Android; safe-area notch teruji | M5 |
| Layout desktop tersimpan/dipulihkan | M6 |

---

## 13. Spesifikasi Teknis

### 13.1 Ekspor (dikerjakan M5; video di M7)

| Format | Prioritas | Detail |
|---|---|---|
| **PNG per frame** (deret, di-ZIP) | M5 | RGBA, skala 50/100/200% |
| **GIF** | M5 | Loop, fps = fps proyek, dithering opsional, palet adaptif; latar transparan→putih |
| **APNG** | M5 | RGBA, loop, fps = fps proyek |
| **WebM (VP9)** | M7 | Opsional; butuh encoder terpisah |

- Opsi dialog ekspor: rentang frame, fps (default = fps proyek), skala, latar
  (transparan/putih/warna), onion skin **tidak** ikut diekspor.
- Output: `<nama-proyek>_<timestamp>.<ext>` — Android via MediaStore (Downloads),
  desktop via dialog simpan.
- Progres → panel ekspor; hasil sukses/gagal memakai token `success`/`danger` +
  toast §7.8; ekspor berjalan di thread terpisah, tidak memblokir kanvas.

### 13.2 Format `.flip` & autosave (M3)

- **Container**: ZIP (deflate) — mudah di-debug dan tahan korupsi parsial.
- **Isi**:
  - `manifest.json` — `format_version` (int, mulai `1`), `app_version`,
    `canvas {width, height}`, `fps`, `layers[] {id, name, visible, opacity,
    blend}`, `exposures[] {layer, start, length, cel_id}`, `cels[] {id, path,
    bounds}`, `created`/`modified`.
  - `cels/<cel_id>.png` — RGBA8 per cel **unik**; exposure berbagi referensi,
    tidak diduplikasi.
  - `thumb/preview.png` — 256 px untuk daftar file terbaru.
  - `checksums.txt` — SHA-256 tiap file, untuk deteksi korupsi.
- **Kompatibilitas**: reader menolak `format_version` lebih besar dengan pesan
  jelas; field tambahan yang tidak dikenal diabaikan (forward-compatible).
- **Tulis aman**: selalu tulis ke `*.tmp` lalu atomic-rename.
- **Autosave/recovery**: file `<nama>.flip.autosave` (container sama) — disimpan
  di background thread setiap **30 detik setelah perubahan terakhir**; dihapus saat
  save manual sukses; saat startup, bila autosave lebih baru dari file utama →
  dialog pemulihan memakai token `warning` §2.2. Autosave tidak pernah memblokir
  stroke (§6).

### 13.3 Target performa & baseline perangkat (M1–M5)

| Metrik | Target |
|---|---|
| Latensi stroke (input → piksel pertama) | ≤ 1 frame (≤ 16 ms @ 60 Hz), nol blocking I/O di UI thread |
| FPS kanvas | 60 fps stabil: onion skin 2 frame + 3 layer |
| Playback | Tepat fps proyek (12/24), tanpa dropped frame |
| Baseline device | Android 9 (API 28), Vulkan 1.1, RAM 4 GB, layar 1080p (mid-range ~2021) |
| Batas proyek v1 | Kanvas maks **4096×4096**, **16 layer**, **600 frame** — di atas itu ditolak dengan pesan jelas |
| Memori (proyek default 1080², 24 frame, 4 layer) | Working set ≤ 256 MB |
| Cold start → kanvas siap gambar | ≤ 2,5 s di baseline |
| Autosave | < 500 ms per save (background), tanpa frame drop |
| Idle | Tanpa render loop idle (on-demand repaint) — hemat baterai/thermal |
| Ukuran APK rilis | ≤ 30 MB |
