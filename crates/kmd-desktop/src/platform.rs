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
    use windows::Win32::Foundation::{BOOL, HWND};
    use windows::Win32::UI::Input::Ime::{
        ImmGetContext, ImmReleaseContext, ImmSetOpenStatus,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.0.is_null() {
            tracing::warn!("force_english_ime: no foreground window");
            return;
        }

        // Safer than ActivateKeyboardLayout:
        // only close IME for this window/context without changing global layout.
        let himc = ImmGetContext(hwnd);
        if himc.0.is_null() {
            tracing::warn!("force_english_ime: ImmGetContext returned null");
            return;
        }

        let ok = ImmSetOpenStatus(himc, BOOL(0));
        let _ = ImmReleaseContext(hwnd, himc);

        if ok.as_bool() {
            tracing::debug!("IME closed for launcher window (English input mode)");
        } else {
            tracing::warn!("force_english_ime: ImmSetOpenStatus failed");
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn force_english_ime() {
    // No-op on non-Windows platforms.
}
