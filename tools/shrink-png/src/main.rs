//! Rewrites PNG screenshots in place as 256-colour palette PNGs (NeuQuant),
//! which keeps the interface's text and colours as they look and saves about
//! a third of the size. `scripts/docs-tour.ps1` runs it on every picture:
//!
//! ```sh
//! cargo run --release -p shrink-png -- Docs/en/images/main.png …
//! ```
//!
//! Running it again on a palette PNG gives the same picture.

use std::fs::File;
use std::io::BufWriter;
use std::process::ExitCode;

fn shrink(path: &str) -> Result<(u64, u64), String> {
    let before = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    let decoded = image::open(path).map_err(|e| e.to_string())?;
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels = rgba.as_raw();
    let quant = color_quant::NeuQuant::new(3, 256, pixels);
    let indices: Vec<u8> = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| quant.index_of(p) as u8)
        .collect();
    let palette: Vec<u8> = quant
        .color_map_rgba()
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|c| [c[0], c[1], c[2]])
        .collect();
    let out = BufWriter::new(File::create(path).map_err(|e| e.to_string())?);
    let mut encoder = png::Encoder::new(out, width, height);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_palette(palette);
    encoder.set_compression(png::Compression::Best);
    encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(&indices))
        .map_err(|e| e.to_string())?;
    let after = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    Ok((before, after))
}

fn main() -> ExitCode {
    let (mut before, mut after, mut failed) = (0, 0, false);
    for path in std::env::args().skip(1) {
        match shrink(&path) {
            Ok((b, a)) => {
                before += b;
                after += a;
            }
            Err(e) => {
                eprintln!("{path}: {e}");
                failed = true;
            }
        }
    }
    println!("{} KB -> {} KB", before / 1024, after / 1024);
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
