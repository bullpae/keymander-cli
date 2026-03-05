//! Platform-specific window tweaks.

/// Force square (non-rounded) corners on target window (Windows 11).
///
/// Windows 11 DWM automatically applies rounded corners to all windows,
/// including borderless ones. This calls `DwmSetWindowAttribute` with
/// `DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_DONOTROUND` to override that.
#[cfg(target_os = "windows")]
pub fn force_square_corners(raw_id: u64) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
        DWM_WINDOW_CORNER_PREFERENCE,
    };

    unsafe {
        let hwnd = HWND(raw_id as usize as *mut core::ffi::c_void);
        if hwnd.0.is_null() {
            tracing::warn!("force_square_corners: invalid window id");
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
pub fn force_square_corners(_raw_id: u64) {
    // No-op on non-Windows platforms.
}

/// Switch the active keyboard layout to English (US) for the foreground window.
///
/// On Windows, after using Korean/Japanese/Chinese IME, the input language
/// persists across application focus changes. This forces English so the
/// launcher starts in a state ready for commands (@, :, !, etc.).
#[cfg(target_os = "windows")]
pub fn force_english_ime(raw_id: u64) {
    use windows::Win32::Foundation::{BOOL, HWND};
    use windows::Win32::UI::Input::Ime::{ImmGetContext, ImmReleaseContext, ImmSetOpenStatus};

    unsafe {
        let hwnd = HWND(raw_id as usize as *mut core::ffi::c_void);
        if hwnd.0.is_null() {
            tracing::warn!("force_english_ime: invalid window id");
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
pub fn force_english_ime(_raw_id: u64) {
    // No-op on non-Windows platforms.
}

/// Bring the window to the foreground and give it keyboard focus.
///
/// Windows restricts `SetForegroundWindow` to the current foreground
/// process. We work around this by temporarily attaching our thread's
/// input queue to the foreground thread via `AttachThreadInput`, then
/// calling `ShowWindow(SW_SHOW)` + `SetForegroundWindow` + `SetFocus`.
///
/// This avoids the older `SendInput(ALT)` trick, which generated
/// spurious keyboard events that could cause UI flickering.
#[cfg(target_os = "windows")]
pub fn force_foreground(raw_id: u64) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
        ShowWindow, SW_SHOW,
    };

    unsafe {
        let hwnd = HWND(raw_id as usize as *mut core::ffi::c_void);
        if hwnd.0.is_null() {
            return;
        }

        let fg_hwnd = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg_hwnd, None);
        let my_thread = GetCurrentThreadId();

        if fg_thread != 0 && fg_thread != my_thread {
            let _ = AttachThreadInput(my_thread, fg_thread, true);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(hwnd);
            let _ = AttachThreadInput(my_thread, fg_thread, false);
        } else {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(hwnd);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn force_foreground(_raw_id: u64) {
    // No-op on non-Windows platforms.
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 플랫폼 함수 시그니처 존재 검증 (컴파일 가드).
    /// 이 함수 중 하나라도 삭제/이름 변경되면 컴파일 에러로 즉시 감지.
    /// v0.3.6 회귀 방지: force_foreground 누락 시 키보드 포커스 미동작.
    #[test]
    fn test_platform_functions_compile() {
        let _: fn(u64) = force_square_corners;
        let _: fn(u64) = force_english_ime;
        let _: fn(u64) = force_foreground;
    }
}
