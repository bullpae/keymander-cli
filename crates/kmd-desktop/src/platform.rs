//! Platform-specific window tweaks.

/// Force square (non-rounded) window corners on Windows 11.
///
/// Windows 11 DWM automatically applies rounded corners to all windows,
/// including borderless ones. This calls `DwmSetWindowAttribute` with
/// `DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_DONOTROUND` to override that.
#[cfg(target_os = "windows")]
pub fn force_square_corners() {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWM_WINDOW_CORNER_PREFERENCE,
        DWMWCP_DONOTROUND,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.0.is_null() {
            tracing::warn!("force_square_corners: no foreground window");
            return;
        }

        let preference = DWMWCP_DONOTROUND;
        let result = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const DWM_WINDOW_CORNER_PREFERENCE as *const _,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );

        match result {
            Ok(()) => tracing::info!("Windows 11 rounded corners disabled"),
            Err(e) => tracing::warn!("DwmSetWindowAttribute failed: {e}"),
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn force_square_corners() {
    // No-op on non-Windows platforms.
}

/// Switch the active keyboard layout to English (US) for the foreground window.
///
/// On Windows, after using Korean/Japanese/Chinese IME, the input language
/// persists across application focus changes. This forces English so the
/// launcher starts in a state ready for commands (@, :, !, etc.).
#[cfg(target_os = "windows")]
pub fn force_english_ime() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        ActivateKeyboardLayout, LoadKeyboardLayoutW, ACTIVATE_KEYBOARD_LAYOUT_FLAGS, KLF_ACTIVATE,
    };

    unsafe {
        // Load (or find) the English — United States keyboard layout.
        let hkl = match LoadKeyboardLayoutW(windows::core::w!("00000409"), KLF_ACTIVATE) {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!("force_english_ime: LoadKeyboardLayoutW failed: {e}");
                return;
            }
        };

        // Activate it for the current thread/process.
        match ActivateKeyboardLayout(hkl, ACTIVATE_KEYBOARD_LAYOUT_FLAGS(0)) {
            Ok(_) => tracing::debug!("IME reset to English (US)"),
            Err(e) => tracing::warn!("force_english_ime: ActivateKeyboardLayout failed: {e}"),
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn force_english_ime() {
    // No-op on non-Windows platforms.
}
