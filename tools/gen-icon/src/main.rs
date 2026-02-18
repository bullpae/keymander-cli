//! PNG → ICO 변환 도구.
//!
//! assets/icon.png (원본)을 읽어 256/48/32/16 리사이즈 후
//! assets/icon.ico (multi-resolution) 생성.
//!
//! 실행: cargo run --manifest-path tools/gen-icon/Cargo.toml

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::{ExtendedColorType, GenericImageView, RgbaImage};
use std::path::Path;

fn main() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("assets");

    let png_path = base.join("icon.png");
    let ico_path = base.join("icon.ico");

    println!("원본 PNG: {}", png_path.display());

    let src = image::open(&png_path).expect("assets/icon.png 을 열 수 없습니다");
    let (sw, sh) = src.dimensions();
    println!("원본 크기: {sw}x{sh}");

    let sizes: &[u32] = &[256, 48, 32, 16];

    let resized: Vec<RgbaImage> = sizes
        .iter()
        .map(|&sz| {
            if sw == sz && sh == sz {
                src.to_rgba8()
            } else {
                image::imageops::resize(
                    &src.to_rgba8(),
                    sz,
                    sz,
                    image::imageops::FilterType::Lanczos3,
                )
            }
        })
        .collect();

    let frames: Vec<IcoFrame<'_>> = resized
        .iter()
        .map(|img| {
            IcoFrame::as_png(img.as_raw(), img.width(), img.height(), ExtendedColorType::Rgba8)
                .expect("ICO 프레임 생성 실패")
        })
        .collect();

    let file = std::fs::File::create(&ico_path).expect("ICO 파일 생성 실패");
    let encoder = IcoEncoder::new(file);
    encoder.encode_images(&frames).expect("ICO 인코딩 실패");

    println!("ICO 생성 완료: {}", ico_path.display());
    for sz in sizes {
        println!("  - {sz}x{sz}");
    }
}
