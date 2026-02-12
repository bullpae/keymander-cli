//! One-off tool to generate keymander's application icon (ICO).
//!
//! Design: keycap shape with » (guillemet) + cyan→green accent bar
//!         on a dark rounded-square background.
//! Outputs: ../../assets/icon.ico (multi-resolution: 256, 48, 32, 16)

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut};
use imageproc::rect::Rect;
use std::path::Path;

// ── Brand colors ──────────────────────────────────────────────────────────
const BG: Rgba<u8> = Rgba([0x18, 0x18, 0x28, 0xFF]);
const KEYCAP: Rgba<u8> = Rgba([0x2A, 0x2A, 0x40, 0xFF]);
const KEYCAP_HL: Rgba<u8> = Rgba([0x3A, 0x3A, 0x58, 0xFF]);
const PEACH: Rgba<u8> = Rgba([0xFF, 0xA5, 0x60, 0xFF]);
const CYAN: Rgba<u8> = Rgba([0x56, 0xD2, 0xFF, 0xFF]);
const GREEN: Rgba<u8> = Rgba([0x50, 0xFA, 0x7B, 0xFF]);

fn main() {
    let out_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets")
        .join("icon.ico");

    // Load system font (Arial Bold on Windows)
    let font_path = r"C:\Windows\Fonts\arialbd.ttf";
    let font_data = std::fs::read(font_path).expect("Cannot read Arial Bold font");
    let font = FontVec::try_from_vec(font_data).expect("Cannot parse font");

    // Generate 256px master
    let master = render_icon(256, &font);

    // Save PNG for reference
    let png_path = out_path.with_extension("png");
    master.save(&png_path).expect("Cannot save PNG");
    println!("PNG saved to {}", png_path.display());

    // Resize for smaller resolutions
    let img48 = image::imageops::resize(&master, 48, 48, image::imageops::FilterType::Lanczos3);
    let img32 = image::imageops::resize(&master, 32, 32, image::imageops::FilterType::Lanczos3);
    let img16 = image::imageops::resize(&master, 16, 16, image::imageops::FilterType::Lanczos3);

    // Write ICO
    let file = std::fs::File::create(&out_path).expect("Cannot create ICO file");
    let encoder = image::codecs::ico::IcoEncoder::new(file);

    let frames = vec![
        make_frame(&master),
        make_frame_buf(&img48),
        make_frame_buf(&img32),
        make_frame_buf(&img16),
    ];

    encoder.encode_images(&frames).expect("Failed to encode ICO");
    println!("ICO written to {}", out_path.display());
}

fn make_frame(img: &RgbaImage) -> image::codecs::ico::IcoFrame<'_> {
    image::codecs::ico::IcoFrame::as_png(
        img.as_raw(),
        img.width(),
        img.height(),
        image::ExtendedColorType::Rgba8,
    )
    .unwrap()
}

fn make_frame_buf(img: &image::ImageBuffer<Rgba<u8>, Vec<u8>>) -> image::codecs::ico::IcoFrame<'_> {
    image::codecs::ico::IcoFrame::as_png(
        img.as_raw(),
        img.width(),
        img.height(),
        image::ExtendedColorType::Rgba8,
    )
    .unwrap()
}

fn render_icon(size: u32, font: &FontVec) -> RgbaImage {
    let s = size as i32;
    let mut img = RgbaImage::from_pixel(size, size, BG);

    // ── Keycap ──
    let margin = s * 11 / 100;
    let kw = s - 2 * margin;
    let kh = s * 70 / 100;
    let ky = s * 9 / 100;

    // Keycap highlight strip (top edge for 3D feel)
    let hl_h = std::cmp::max(s * 3 / 100, 1);
    draw_filled_rect_mut(
        &mut img,
        Rect::at(margin, ky).of_size(kw as u32, hl_h as u32),
        KEYCAP_HL,
    );

    // Keycap face
    draw_filled_rect_mut(
        &mut img,
        Rect::at(margin, ky + hl_h).of_size(kw as u32, (kh - hl_h) as u32),
        KEYCAP,
    );

    // ── Guillemet » ──
    let font_size = s as f32 * 0.52;
    let px_scale = PxScale::from(font_size);
    let scaled = font.as_scaled(px_scale);

    // Measure the glyph
    let glyph_id = font.glyph_id('»');
    let h_advance = scaled.h_advance(glyph_id);
    let ascent = scaled.ascent();
    let descent = scaled.descent();
    let glyph_height = ascent - descent;

    // Center in keycap area
    let keycap_cx = margin as f32 + kw as f32 / 2.0;
    let keycap_cy = ky as f32 + kh as f32 / 2.0;

    let text_x = keycap_cx - h_advance / 2.0;
    let text_y = keycap_cy - glyph_height / 2.0;

    draw_text_mut(
        &mut img,
        PEACH,
        text_x as i32,
        text_y as i32,
        px_scale,
        font,
        "\u{00BB}",
    );

    // ── Accent gradient bar ──
    let bar_h = std::cmp::max(s * 3 / 100, 2) as u32;
    let bar_y = (ky + kh + s * 4 / 100) as u32;
    let bar_x = (margin + s * 3 / 100) as u32;
    let bar_w = (kw - s * 6 / 100) as u32;

    for x in 0..bar_w {
        let t = x as f32 / bar_w as f32;
        let r = lerp_u8(CYAN.0[0], GREEN.0[0], t);
        let g = lerp_u8(CYAN.0[1], GREEN.0[1], t);
        let b = lerp_u8(CYAN.0[2], GREEN.0[2], t);
        let color = Rgba([r, g, b, 0xFF]);
        for dy in 0..bar_h {
            let px = bar_x + x;
            let py = bar_y + dy;
            if px < size && py < size {
                img.put_pixel(px, py, color);
            }
        }
    }

    // ── Round corners ──
    let radius = (s as f32 * 0.19) as u32;
    apply_rounded_corners(&mut img, radius);

    img
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

fn apply_rounded_corners(img: &mut RgbaImage, radius: u32) {
    let (w, h) = img.dimensions();
    let r = radius as f32;

    for y in 0..h {
        for x in 0..w {
            let corner = if x < radius && y < radius {
                Some((radius as f32, radius as f32))
            } else if x >= w - radius && y < radius {
                Some(((w - radius) as f32, radius as f32))
            } else if x < radius && y >= h - radius {
                Some((radius as f32, (h - radius) as f32))
            } else if x >= w - radius && y >= h - radius {
                Some(((w - radius) as f32, (h - radius) as f32))
            } else {
                None
            };

            if let Some((cx, cy)) = corner {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > r {
                    img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                } else if dist > r - 1.5 {
                    let alpha = ((r - dist) / 1.5 * 255.0).clamp(0.0, 255.0) as u8;
                    let p = *img.get_pixel(x, y);
                    let blended = (p.0[3] as u16 * alpha as u16 / 255) as u8;
                    img.put_pixel(x, y, Rgba([p.0[0], p.0[1], p.0[2], blended]));
                }
            }
        }
    }
}
