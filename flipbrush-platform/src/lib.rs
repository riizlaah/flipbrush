//! Abstraksi platform Flipbrush.
//!
//! Berisi jembatan ke API spesifik platform yang tidak boleh bocor ke
//! [`flipbrush_core`]: haptics & inset Android (via JNI, M4/M5),
//! path settings desktop, dsb. Milestone pengisian: M2+ (DESIGN.md §12.2).

/// Nama platform yang sedang berjalan.
pub fn current_platform() -> &'static str {
    if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_is_recognized() {
        assert_ne!(current_platform(), "unknown");
    }
}
