//! Model domain Flipbrush: proyek animasi, frame, layer, dan cel.
//!
//! Crate ini murni domain logic — tanpa UI (egui) dan tanpa I/O platform.
//! Milestone pengisian: M3 (model animasi, lihat DESIGN.md §12.2).

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
