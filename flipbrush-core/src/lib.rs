//! Model domain Flipbrush: proyek animasi, frame, layer, dan cel.
//!
//! Crate ini murni domain logic — tanpa UI (egui) dan tanpa I/O platform.
//! Milestone pengisian: M2 (inti stroke — [`raster`]), M3 (model animasi +
//! format `.flip` lihat DESIGN.md §12.2, §13.2).

/// Model animasi M3: layer, frame, cel, exposure (DESIGN.md §12.2, §7.7).
pub mod anim;

/// Format `.flip` M3: encode/decode container ZIP in-memory (DESIGN.md §13.2).
pub mod flip;

/// Inti stroke M2: bitmap RGBA8, kuas, penghapus, fill, undo/redo.
pub mod raster;

/// Model seleksi piksel M4d: geometri marquee/lasso/wand untuk clipping
/// operasi gambar (DESIGN.md §12.2 toolset lanjutan).
pub mod seleksi;

/// Ekstensi berkas proyek Flipbrush (DESIGN.md §1).
pub const PROJECT_EXTENSION: &str = "flip";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_extension_is_lowercase() {
        assert_eq!(PROJECT_EXTENSION, "flip");
    }
}
