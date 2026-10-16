//! Fonts embedded in the binary so Apix looks the same on every OS.
//! Both families are SIL OFL 1.1; the licenses sit next to the files in assets/fonts.

use std::borrow::Cow;

use gpui_kit::App;

pub const UI: &str = "Inter";
pub const MONO: &str = "JetBrains Mono";

const FILES: [&[u8]; 6] = [
    include_bytes!("../assets/fonts/inter/Inter-Regular.ttf"),
    include_bytes!("../assets/fonts/inter/Inter-Medium.ttf"),
    include_bytes!("../assets/fonts/inter/Inter-SemiBold.ttf"),
    include_bytes!("../assets/fonts/inter/Inter-Bold.ttf"),
    include_bytes!("../assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf"),
    include_bytes!("../assets/fonts/jetbrains-mono/JetBrainsMono-Bold.ttf"),
];

/// Registers the embedded fonts. Returns false if the platform rejected
/// them, in which case the theme keeps its system fonts.
pub fn register(cx: &mut App) -> bool {
    let fonts = FILES.iter().map(|bytes| Cow::Borrowed(*bytes)).collect();
    cx.text_system().add_fonts(fonts).is_ok()
}
