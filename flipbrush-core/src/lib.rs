//! Model domain Flipbrush: proyek animasi, frame, layer, dan cel.
//!
//! Crate ini murni domain logic — tanpa UI (egui) dan tanpa I/O platform.
//! Milestone pengisian: M2 (inti stroke — [`raster`]), M3 (model animasi,
//! lihat DESIGN.md §12.2).

/// Model animasi M3: layer, frame, cel, exposure (DESIGN.md §12.2, §7.7).
pub mod anim;

/// Inti stroke M2: bitmap RGBA8, kuas, penghapus, fill, undo/redo.
pub mod raster;

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
