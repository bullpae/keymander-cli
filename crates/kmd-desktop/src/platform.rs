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
