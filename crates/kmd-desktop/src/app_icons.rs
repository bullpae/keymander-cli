//! 네이티브 아이콘 추출 — OS Shell API로 파일/앱/디렉터리 아이콘 추출
//!
//! Windows: SHGetFileInfoW → HICON → BGRA → RGBA pixel data
//! macOS/Linux: 향후 구현 (현재 None 반환)
//!
//! App/Executable: 경로 기준 캐시
//! File: 확장자 기준 캐시 (같은 확장자는 같은 아이콘)
//! Directory: 고정 "dir" 키로 캐시

use iced::widget::image::Handle;
use kmd_core::ItemKind;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

const FALLBACK_ICON_SIZE: u32 = 32;
const TARGET_ICON_SIZE: u32 = 32;

static ICON_CACHE: LazyLock<Mutex<HashMap<String, Option<Handle>>>> =
    LazyLock::new(|| Mutex::new(HashMap::with_capacity(128)));

/// 아이템의 네이티브 아이콘 반환 (캐시됨).
/// App/Executable: 실행 파일 아이콘, File: 연결 프로그램 아이콘, Directory: 폴더 아이콘
pub fn app_icon_for_item(kind: ItemKind, path: &str) -> Option<Handle> {
    if path.is_empty() || path.starts_with("http") || path.starts_with("kmd:") {
        return None;
    }

    match kind {
        ItemKind::App | ItemKind::Executable => {
            let mut cache = ICON_CACHE.lock().ok()?;
            if let Some(cached) = cache.get(path) {
                return cached.clone();
            }
            let handle = platform::extract_icon(path);
            cache.insert(path.to_string(), handle.clone());
            handle
        }
        ItemKind::File => {
            let ext = std::path::Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext.is_empty() {
                return None;
            }
            let cache_key = format!("ext:{ext}");
            let mut cache = ICON_CACHE.lock().ok()?;
            if let Some(cached) = cache.get(&cache_key) {
                return cached.clone();
            }
            let dummy = format!("file.{ext}");
            let handle = platform::extract_icon_by_ext(&dummy);
            cache.insert(cache_key, handle.clone());
            handle
        }
        ItemKind::Directory => {
            let cache_key = "type:dir".to_string();
            let mut cache = ICON_CACHE.lock().ok()?;
            if let Some(cached) = cache.get(&cache_key) {
                return cached.clone();
            }
            let handle = platform::extract_folder_icon();
            cache.insert(cache_key, handle.clone());
            handle
        }
        _ => None,
    }
}

// ── Windows 구현 ────────────────────────────────────────────────────────────

#[cfg(windows)]
mod platform {
    use super::FALLBACK_ICON_SIZE;
    use iced::widget::image::Handle;
    use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
    use windows::Win32::UI::Shell::{
        SHGetFileInfoW, SHFILEINFOW, SHGFI_FLAGS, SHGFI_ICON, SHGFI_LARGEICON,
        SHGFI_USEFILEATTRIBUTES,
    };
    use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;

    /// 실제 파일/바로가기 경로에서 아이콘 추출 (App/Executable용)
    pub fn extract_icon(path: &str) -> Option<Handle> {
        extract_shell_icon(path, FILE_FLAGS_AND_ATTRIBUTES(0), false)
    }

    /// 확장자 기반 연결 프로그램 아이콘 추출 (File용, 파일 존재 불필요)
    pub fn extract_icon_by_ext(dummy_name: &str) -> Option<Handle> {
        extract_shell_icon(
            dummy_name,
            FILE_FLAGS_AND_ATTRIBUTES(0x80), // FILE_ATTRIBUTE_NORMAL
            true,
        )
    }

    /// 폴더 아이콘 추출
    pub fn extract_folder_icon() -> Option<Handle> {
        extract_shell_icon(
            "directory",
            FILE_FLAGS_AND_ATTRIBUTES(0x10), // FILE_ATTRIBUTE_DIRECTORY
            true,
        )
    }

    fn extract_shell_icon(
        path: &str,
        file_attrs: FILE_FLAGS_AND_ATTRIBUTES,
        use_file_attrs: bool,
    ) -> Option<Handle> {
        unsafe {
            let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let mut info = SHFILEINFOW::default();

            let mut flags = SHGFI_ICON.0 | SHGFI_LARGEICON.0;
            if use_file_attrs {
                flags |= SHGFI_USEFILEATTRIBUTES.0;
            }

            let result = SHGetFileInfoW(
                windows::core::PCWSTR(wide.as_ptr()),
                file_attrs,
                Some(&mut info),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_FLAGS(flags),
            );

            if result == 0 || info.hIcon.0.is_null() {
                return None;
            }

            let rgba = hicon_to_rgba(info.hIcon);
            let _ = DestroyIcon(info.hIcon);

            rgba.and_then(|(pixels, w, h)| normalize_and_encode(w, h, &pixels))
        }
    }

    /// 원본 RGBA를 TARGET_ICON_SIZE 정사각형으로 리사이즈 후 PNG 인코딩
    fn normalize_and_encode(width: u32, height: u32, rgba: &[u8]) -> Option<Handle> {
        use image::codecs::png::PngEncoder;
        use image::imageops::FilterType;
        use image::{ImageEncoder, RgbaImage};

        let target = super::TARGET_ICON_SIZE;
        let img = RgbaImage::from_raw(width, height, rgba.to_vec())?;

        let resized = if width == target && height == target {
            img
        } else {
            image::imageops::resize(&img, target, target, FilterType::Lanczos3)
        };

        let mut buf = Vec::new();
        PngEncoder::new(&mut buf)
            .write_image(resized.as_raw(), target, target, image::ExtendedColorType::Rgba8)
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

    pub fn extract_icon_by_ext(_dummy_name: &str) -> Option<Handle> {
        None
    }

    pub fn extract_folder_icon() -> Option<Handle> {
        None
    }
}
