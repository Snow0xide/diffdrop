//! Translucent window background: system vibrancy on macOS, Mica on Windows 11.

use tauri::WebviewWindow;

/// Returns whether the effect is active; if not, the frontend paints an
/// opaque background instead (for example on Windows 10).
pub fn apply(window: &WebviewWindow) -> bool {
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{NSVisualEffectMaterial, NSVisualEffectState, apply_vibrancy};
        apply_vibrancy(
            window,
            NSVisualEffectMaterial::UnderWindowBackground,
            Some(NSVisualEffectState::FollowsWindowActiveState),
            None,
        )
        .is_ok()
    }
    #[cfg(target_os = "windows")]
    {
        // `None` follows the system light or dark setting at the time of the call.
        window_vibrancy::apply_mica(window, None).is_ok()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = window;
        false
    }
}
