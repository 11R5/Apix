//! "Obsidian & electric cyan": a dark, translucent palette. Surfaces are
//! partly transparent so the OS blur (Mica / vibrancy / compositor) shows through.

use apix_core::Method;
use gpui_kit::{Hsla, hsla};

fn hue(degrees: f32) -> f32 {
    degrees / 360.
}

pub fn background() -> Hsla {
    hsla(hue(225.), 0.25, 0.06, 0.78)
}

pub fn surface() -> Hsla {
    hsla(hue(225.), 0.20, 0.10, 0.70)
}

/// 1px panel separators.
pub fn hairline() -> Hsla {
    hsla(0., 0., 1., 0.08)
}

pub fn text() -> Hsla {
    hsla(0., 0., 0.92, 1.)
}

pub fn text_muted() -> Hsla {
    hsla(0., 0., 0.92, 0.45)
}

pub fn accent() -> Hsla {
    hsla(hue(185.), 1., 0.5, 1.)
}

pub fn method_color(method: Method) -> Hsla {
    match method {
        Method::Get => accent(),
        Method::Post => hsla(hue(142.), 0.70, 0.55, 1.),
        Method::Put | Method::Patch => hsla(hue(38.), 0.90, 0.60, 1.),
        Method::Delete => hsla(hue(0.), 0.80, 0.62, 1.),
        Method::Head | Method::Options => hsla(hue(265.), 0.60, 0.70, 1.),
    }
}
