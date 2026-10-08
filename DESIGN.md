# Opimator Design System v1

Dokumen acuan tunggal untuk seluruh aspek visual dan interaksi Opimator.
Berlaku untuk build desktop (Linux, prioritas utama) dan mobile (Android/iOS).

---

## 1. Prinsip

1. **Canvas-first** — Kanvas adalah raja. UI hanya "chrome" setipis mungkin, dan bisa
   disembunyikan sepenuhnya (mode fokus menggambar dengan `Tab`).
2. **Netral agar mata tidak tertipu** — Tidak ada warna chromatic di belakang atau di
   sekeliling kanvas. Semua permukaan menggunakan skala abu-abu netral agar pemilihan
   warna oleh pengguna tidak terpengaruh lingkungan UI.
3. **Presisi** — Kontrol harus mendukung input presisi: drag-numeric pada slider,
   angka tabular, koordinat yang jelas.
4. **Input-aware** — Desktop: hover states, shortcut keyboard penuh, dense.
   Mobile: touch target ≥ 44 px, tanpa ketergantungan hover, gesture-driven.
5. **Aksen tunggal** — Hanya satu warna aksen (`accent/brand`) untuk chrome. Aksen tidak
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

---

## 7. Komponen

Semua komponen punya state: `default`, `hover`, `active/pressed`, `focused`,
`disabled`. Ukuran kontrol desktop: **28 px** tinggi (compact) / **32 px** (default).
Mobile: **44 px** minimum.

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

### 7.4 Tool rail (desktop)

- Lebar 48 px, kiri, `bg/surface-1`, item 40×40 px.
- Item aktif: `ghost-active` + strip aksen 2 px di tepi kiri rail.
- Grup dipisah divider `border/default`; tool dengan submenu panah kecil.

### 7.5 Options bar (desktop)

- Tinggi 40 px, di atas kanvas, berisi kontrol spesifik tool aktif
  (mis. brush: size, opacity, hardness, spacing, preset picker).
- Kontrol dipisah divider vertikal `border/default`.

### 7.6 Panel & dock

- `bg/surface-1`, radius `radius/md` bila floating, tanpa radius bila docked.
- Header panel: `label-md` + tombol collapse (chevron) + menu (⋮).
- Panel bisa di-collapse (hanya header) dan disembunyikan (`Tab` = fokus mode).

### 7.7 Timeline / X-sheet

- Grid: baris = layer, kolom = frame. Header kolom: `numeric`.
- Cell exposed: `timeline/cel-exposed`; awal exposure punya strip kiri 2 px lebih terang;
  cell lanjutan exposure hanya strip tanpa isi.
- Playhead: garis 1 px `text/primary` + kepala segitiga.
- Zoom timeline: `Ctrl+Scroll`; scrub: seret playhead; loop range: drag di bar atas.

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

### 7.10 Command palette (`Ctrl+K`)

- Modal terpusat, lebar 560 px, input `body-lg`, hasil `label-md` + shortcut kanan.
- Sumber: semua command + recent tools + file recent.

### 7.11 Empty state

- Ikon outline 32 px `text/disabled`, teks `body-lg` `text/secondary`,
  satu aksi `solid` bila relevan. Contoh: "Drag gambar ke sini untuk referensi".

---

## 8. Layout

### 8.1 Desktop (prioritas utama — Linux)

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

### 8.2 Mobile (Android → iOS)

- Kanvas full-screen; **bottom toolbar** (64 px) berisi tool utama (ikon 24 px).
- Panel (Color / Brush / Layers / Timeline) sebagai **bottom-sheet** yang bisa
  di-drag: peek (25%), half (50%), full.
- Gesture:
  | Gesture | Aksi |
  |---|---|
  | 1 jari (tool brush) | Menggambar |
  | 1 jari (tool pan) / 2 jari drag | Pan |
  | Pinch | Zoom |
  | 2 jari tap | Undo |
  | 3 jari tap | Redo |
  | Tap lama di kanvas | Eyedropper |
- Playback: tombol play besar 56 px mengambang di atas bottom toolbar.

---

## 9. Accessibility

- Kontras teks minimum **4.5:1** (body) / **3:1** (large text & ikon).
- Focus ring selalu terlihat: 2 px `accent/brand`, offset 2 px.
- Navigasi keyboard penuh di desktop (tab order logis, `Esc` selalu menutup).
- Onion skin default merah/biru → aman untuk buta warna merah-hijau.
- Semua kontrol ikon wajib punya accessible name (egui label/ARIA).
- `prefers-reduced-motion` dan `prefers-contrast` dihormati.

---

## 10. Konvensi Penamaan & Tema

- Token didefinisikan sekali di `opimator-app/src/theme.rs` sebagai konstanta bertipe
  (mis. `struct Theme { surface_0: Color32, ... }`), di-`resolve` saat startup.
- Tidak boleh ada hex literal di luar `theme.rs` — semua komponen memakai token.
- Tema (dark/light + accent) disimpan di `~/.config/opimator/settings.toml`.
- Ikon SVG dinamai `kebab-case.svg` sesuai nama tool, satu file satu ikon.

---

## 11. Checklis rilis design system

- [ ] Semua token terdefinisi & tidak ada hard-coded color di UI.
- [ ] Kontras teks lolos audit (axe/manual).
- [ ] Semua tombol ikon punya tooltip + shortcut.
- [ ] Motion dimatikan pada `prefers-reduced-motion`.
- [ ] Layout desktop tersimpan/dipulihkan antar sesi.
- [ ] Touch target ≥ 44 px di build mobile.
- [ ] Dark & light theme lolos uji visual di 100%/200% DPI.
