# Toolchain Flipbrush

Acuan setup environment untuk membangun Flipbrush (DESIGN.md §12.2 M0).
Host & target utama: **Linux desktop (Fedora)**; target sekunder **Android**
(build via `cargo-apk`, M6).

## 1. Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

`rust-toolchain.toml` di root repo otomatis memasang komponen `clippy`/`rustfmt`
dan target Android (`aarch64-linux-android`, `armv7-linux-androideabi`,
`x86_64-linux-android`) begitu Anda menjalankan `cargo` pertama kali.

## 2. Dependensi sistem (Fedora)

```bash
sudo dnf install gcc make pkgconf-pkg-config cmake git \
  libxkbcommon-devel wayland-devel libX11-devel libxcb-devel \
  libXrandr-devel libXi-devel

# Vulkan (renderer wgpu) — sesuaikan dengan GPU
sudo dnf install vulkan-tools mesa-vulkan-drivers   # AMD/Intel
# atau driver NVIDIA resmi + vulkan-tools

# Java — hanya untuk keytool + apksigner (signing APK)
sudo dnf install java-17-openjdk-devel

# Debug perangkat (opsional)
sudo dnf install scrcpy   # mirror layar; adb ikut terpasang
```

Cek: `vulkaninfo --summary` (perlu `vulkan-tools`).

## 3. Android SDK & NDK *(opsional — hanya untuk M6)*

Butuh minimal:

| Komponen | Fungsi |
|---|---|
| `cmdline-tools` (`sdkmanager`) | mengelola SDK |
| `platform-tools` (`adb`) | install & debug APK |
| `build-tools` (`zipalign`, `apksigner`) | align + sign APK |
| NDK r28 | cross-compile `.so` |

```bash
# Contoh layout (SDK di $HOME/Android/Sdk):
#   cmdline-tools/ platform-tools/ build-tools/35.0.1/ ndk/28.1.13356709/
export ANDROID_HOME="$HOME/Android/Sdk"
export ANDROID_NDK_ROOT="$ANDROID_HOME/ndk/28.1.13356709"
export PATH="$PATH:$ANDROID_HOME/platform-tools:$ANDROID_HOME/build-tools/35.0.1"
```

Tambahkan export di atas ke `~/.bashrc` agar tidak hilang antar sesi.

## 4. cargo-apk *(opsional — hanya untuk M6)*

```bash
cargo install cargo-apk --locked
```

Manifest Android dibaca dari `[package.metadata.android]` di
`flipbrush-app/Cargo.toml` (package `app.flipbrush`, SDK, build targets) —
tidak perlu proyek Gradle.

## 5. Perintah build

```bash
# Desktop (development harian)
cargo run -p flipbrush-app

# Mutu kode (sama dengan CI)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/check-hex-literals.sh        # §10: hex hanya di theme.rs

# Android (opsional/M6; device terhubung via adb)
# --lib: hanya target cdylib (bin desktop tidak relevan di Android;
# cargo-apk panik "Bin is not compatible with Cdylib" tanpa flag ini)
cargo apk build -p flipbrush-app --lib   # APK di target/debug/apk/
cargo apk run -p flipbrush-app --lib     # build + install + jalankan
```

## Troubleshooting

- **`vulkan` tidak ditemukan saat runtime** — pastikan `mesa-vulkan-drivers`
  (atau driver GPU vendor) terpasang; cek `vulkaninfo --summary`.
- **`ANDROID_NDK_ROOT` tidak dikenal** — cargo-apk membaca env var tersebut;
  set versi NDK sesuai folder di `$ANDROID_HOME/ndk/`.
- **APK terpasang tapi crash di awal** — lihat log:
  `adb logcat -s RustStdoutStderr android-activity`.
