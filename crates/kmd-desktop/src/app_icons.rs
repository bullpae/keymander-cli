//! 네이티브 앱 아이콘 추출 — OS Shell API로 실행 파일/바로가기에서 아이콘 추출
//!
//! Windows: SHGetFileInfoW → HICON → BGRA → RGBA pixel data
//! macOS/Linux: 향후 구현 (현재 None 반환)
//!
//! 아이콘은 경로 기준으로 캐시되며, iced의 `Handle::from_bytes`를 사용하여
//! 텍스처 캐시가 정상 작동한다.

use iced::widget::image::Handle;
use kmd_core::ItemKind;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

const FALLBACK_ICON_SIZE: u32 = 32;

static ICON_CACHE: LazyLock<Mutex<HashMap<String, Option<Handle>>>> =
    LazyLock::new(|| Mutex::new(HashMap::with_capacity(64)));

/// App/Executable 아이템의 네이티브 아이콘 반환 (캐시됨).
/// 지원하지 않는 종류이거나 추출 실패 시 None.
pub fn app_icon_for_item(kind: ItemKind, path: &str) -> Option<Handle> {
    if !matches!(kind, ItemKind::App | ItemKind::Executable) {
        return None;
    }
    if path.is_empty() || path.starts_with("http") || path.starts_with("kmd:") {
        return None;
    }

    let mut cache = ICON_CACHE.lock().ok()?;
    if let Some(cached) = cache.get(path) {
        return cached.clone();
    }
    let handle = platform::extract_icon(path);
    cache.insert(path.to_string(), handle.clone());
    handle
}

// ── Windows 구현 ────────────────────────────────────────────────────────────

#[cfg(windows)]
mod platform {
    use super::FALLBACK_ICON_SIZE;
    use iced::widget::image::Handle;

    pub fn extract_icon(path: &str) -> Option<Handle> {
        use windows::Win32::UI::Shell::{
            SHGetFileInfoW, SHFILEINFOW, SHGFI_FLAGS, SHGFI_ICON, SHGFI_LARGEICON,
        };
        use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;

        unsafe {
            let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let mut info = SHFILEINFOW::default();

            let result = SHGetFileInfoW(
                windows::core::PCWSTR(wide.as_ptr()),
                windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
                Some(&mut info),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_FLAGS(SHGFI_ICON.0 | SHGFI_LARGEICON.0),
            );

            if result == 0 || info.hIcon.0.is_null() {
                return None;
            }

            let result = hicon_to_rgba(info.hIcon);
            let _ = DestroyIcon(info.hIcon);

            result.and_then(|(pixels, w, h)| encode_png(w, h, &pixels))
        }
    }

    fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Handle> {
        use image::codecs::png::PngEncoder;
        use image::ImageEncoder;

        let mut buf = Vec::new();
        let encoder = PngEncoder::new(&mut buf);
        encoder
            .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
            .ok()?;
        Some(Handle::from_bytes(buf))
    }

    /// HICON → (RGBA pixel data, width, height)
    unsafe fn hicon_to_rgba(
        hicon: windows::Win32::UI::WindowsAndMessaging::HICON,
    ) -> Option<(Vec<u8>, u32, u32)> {
        use windows::Win32::Graphics::Gdi::*;
        use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

        let mut icon_info = ICONINFO::default();
        GetIconInfo(hicon, &mut icon_info).ok()?;

        // 실제 비트맵 크기 조회
        let (width, height) = if !icon_info.hbmColor.0.is_null() {
            bitmap_dimensions(icon_info.hbmColor)
        } else {
            (0, 0)
        };
        let (width, height) = if width > 0 && height > 0 {
            (width, height)
        } else {
            (FALLBACK_ICON_SIZE, FALLBACK_ICON_SIZE)
        };

        let hdc_screen = GetDC(None);
        let hdc = CreateCompatibleDC(hdc_screen);

        if hdc.0.is_null() {
            ReleaseDC(None, hdc_screen);
            cleanup_bitmaps(&icon_info);
            return None;
        }

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: height as i32, // bottom-up
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0 as u32,
                ..Default::default()
            },
            ..Default::default()
        };

        let pixel_count = (width * height * 4) as usize;
        let mut pixels = vec![0u8; pixel_count];

        let old = SelectObject(hdc, icon_info.hbmColor);
        let lines = GetDIBits(
            hdc,
            icon_info.hbmColor,
            0,
            height,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        SelectObject(hdc, old);

        let _ = DeleteDC(hdc);
        ReleaseDC(None, hdc_screen);
        cleanup_bitmaps(&icon_info);

        if lines == 0 {
            return None;
        }

        // bottom-up → top-down 변환
        let row_bytes = (width * 4) as usize;
        let mut top_down = Vec::with_capacity(pixel_count);
        for row in pixels.chunks_exact(row_bytes).rev() {
            top_down.extend_from_slice(row);
        }

        // BGRA → RGBA + 알파 처리
        let has_alpha = top_down.chunks_exact(4).any(|c| c[3] != 0);
        for chunk in top_down.chunks_exact_mut(4) {
            chunk.swap(0, 2);
            if !has_alpha {
                chunk[3] = 255;
            }
        }

        Some((top_down, width, height))
    }

    /// HBITMAP의 실제 픽셀 크기 조회
    unsafe fn bitmap_dimensions(hbm: windows::Win32::Graphics::Gdi::HBITMAP) -> (u32, u32) {
        use windows::Win32::Graphics::Gdi::{GetObjectW, BITMAP};

        let mut bm = BITMAP::default();
        let ret = GetObjectW(
            hbm,
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut _ as *mut _),
        );
        if ret > 0 && bm.bmWidth > 0 && bm.bmHeight > 0 {
            (bm.bmWidth as u32, bm.bmHeight as u32)
        } else {
            (0, 0)
        }
    }

    unsafe fn cleanup_bitmaps(info: &windows::Win32::UI::WindowsAndMessaging::ICONINFO) {
        use windows::Win32::Graphics::Gdi::DeleteObject;

        if !info.hbmColor.0.is_null() {
            let _ = DeleteObject(info.hbmColor);
        }
        if !info.hbmMask.0.is_null() {
            let _ = DeleteObject(info.hbmMask);
        }
    }
}

// ── 비-Windows 스텁 ─────────────────────────────────────────────────────────

#[cfg(not(windows))]
mod platform {
    use iced::widget::image::Handle;

    pub fn extract_icon(_path: &str) -> Option<Handle> {
        None
    }
}
