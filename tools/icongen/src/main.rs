//! Builds every app icon from the logo artwork:
//!
//! ```sh
//! cargo run -p icongen --release
//! ```
//!
//! The source (`Logo/LogoFirst.jpg`, 2048×2048) is a glowing squircle on a dark
//! background with a reflection underneath. We cut the squircle out, make
//! everything outside it transparent, and resample it to each size. The
//! smallest sizes (16–32 px) use a tighter crop of just the bird and the X,
//! because the frame ring turns to mush at that scale.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::imageops::{self, FilterType};
use image::{ExtendedColorType, ImageEncoder, Rgba, RgbaImage};

const SOURCE: &str = "Logo/LogoFirst.jpg";

/// The squircle in the source, measured from its edges: x 216–1832, y 236–1836.
const TILE: Crop = Crop {
    cx: 1024,
    cy: 1036,
    side: 1606,
};
/// The bird and the X only, for 16–32 px.
const GLYPH: Crop = Crop {
    cx: 975,
    cy: 925,
    side: 1180,
};

/// Below this size the tight glyph crop is used.
const SMALL_MAX: u32 = 32;
/// Superellipse exponent for the mask; ~5 matches the source's corners.
const SQUIRCLE_N: f32 = 5.0;

struct Crop {
    cx: u32,
    cy: u32,
    side: u32,
}

fn main() -> Result<()> {
    let source = image::open(SOURCE)
        .with_context(|| format!("opening {SOURCE} (run from the repository root)"))?
        .to_rgba8();
    let tile = master(&source, &TILE);
    let glyph = master(&source, &GLYPH);
    let icon = |size: u32| -> RgbaImage {
        let base = if size <= SMALL_MAX { &glyph } else { &tile };
        imageops::resize(base, size, size, FilterType::Lanczos3)
    };

    // Shared PNGs (also used by the app's title bar and the README).
    let out = Path::new("packaging/icons");
    fs::create_dir_all(out)?;
    for size in [16, 24, 32, 48, 64, 128, 256, 512, 1024] {
        save_png(&icon(size), &out.join(format!("apix-{size}.png")))?;
    }
    save_png(
        &icon(32),
        Path::new("crates/apix-desktop/assets/icon-32.png"),
    )?;

    // Windows: one .ico with every size Explorer and the taskbar ask for.
    fs::create_dir_all("packaging/windows")?;
    let frames = [16, 24, 32, 48, 64, 128, 256]
        .into_iter()
        .map(|size| {
            let img = icon(size);
            IcoFrame::as_png(img.as_raw(), size, size, ExtendedColorType::Rgba8)
                .context("encoding an .ico frame")
        })
        .collect::<Result<Vec<_>>>()?;
    let file = fs::File::create("packaging/windows/apix.ico")?;
    IcoEncoder::new(file).encode_images(&frames)?;

    // macOS: Apple's grid puts the squircle in 824 of 1024 px, with room for the shadow.
    fs::create_dir_all("packaging/macos")?;
    let mut family = icns::IconFamily::new();
    for size in [16, 32, 64, 128, 256, 512, 1024] {
        let img = padded(&icon(size * 824 / 1024), size);
        let image = icns::Image::from_data(icns::PixelFormat::RGBA, size, size, img.into_raw())?;
        family.add_icon(&image)?;
    }
    family.write(fs::File::create("packaging/macos/Apix.icns")?)?;

    // Linux: the freedesktop hicolor theme layout.
    for size in [16, 24, 32, 48, 64, 128, 256, 512] {
        let dir = Path::new("packaging/linux/icons/hicolor").join(format!("{size}x{size}/apps"));
        fs::create_dir_all(&dir)?;
        save_png(&icon(size), &dir.join("apix.png"))?;
    }

    println!("icons written to packaging/ and crates/apix-desktop/assets/icon-32.png");
    Ok(())
}

/// Crops a square and masks it to a squircle at 1024 px, with soft edges.
fn master(source: &RgbaImage, crop: &Crop) -> RgbaImage {
    let half = crop.side / 2;
    let square =
        imageops::crop_imm(source, crop.cx - half, crop.cy - half, crop.side, crop.side).to_image();
    let mut img = imageops::resize(&square, 1024, 1024, FilterType::Lanczos3);
    apply_squircle_mask(&mut img);
    img
}

/// Multiplies alpha by squircle coverage, supersampled 4×4 per pixel.
fn apply_squircle_mask(img: &mut RgbaImage) {
    let size = img.width() as f32;
    let samples = 4;
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let mut inside = 0;
        for sy in 0..samples {
            for sx in 0..samples {
                let u = (x as f32 + (sx as f32 + 0.5) / samples as f32) / size * 2.0 - 1.0;
                let v = (y as f32 + (sy as f32 + 0.5) / samples as f32) / size * 2.0 - 1.0;
                if u.abs().powf(SQUIRCLE_N) + v.abs().powf(SQUIRCLE_N) <= 1.0 {
                    inside += 1;
                }
            }
        }
        let coverage = inside as f32 / (samples * samples) as f32;
        pixel.0[3] = (pixel.0[3] as f32 * coverage).round() as u8;
    }
}

/// Centres `img` on a transparent canvas of `size`.
fn padded(img: &RgbaImage, size: u32) -> RgbaImage {
    let mut canvas = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));
    let offset = ((size - img.width()) / 2) as i64;
    imageops::overlay(&mut canvas, img, offset, offset);
    canvas
}

fn save_png(img: &RgbaImage, path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let file = fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    image::codecs::png::PngEncoder::new(file).write_image(
        img.as_raw(),
        img.width(),
        img.height(),
        ExtendedColorType::Rgba8,
    )?;
    Ok(())
}
