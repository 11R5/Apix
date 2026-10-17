//! Design tokens from Docs/IS-PLANI.md §1.3. Surfaces are partly transparent
//! so the OS blur (Mica / vibrancy / compositor) shows through.

use std::sync::Arc;

use apix_core::Method;
use apix_core::text::JsonToken;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{
    App, Background, Hsla, Rgba, hsla, linear_color_stop, linear_gradient, rgb, rgba,
    transparent_black,
};

/// Points gpui-kit's component theme at our palette so built-in inputs,
/// tabs, menus and buttons match the rest of the window.
pub fn apply(palette: &Palette, embedded_fonts: bool, cx: &mut App) {
    // Switching mode reloads the stock colours, so it must happen before our overrides.
    Theme::change(ThemeMode::Dark, None, cx);
    let p = *palette;
    Theme::update(cx, |t| {
        if embedded_fonts {
            t.font_family = crate::fonts::UI.into();
            t.mono_font_family = crate::fonts::MONO.into();
        }
        // Dialogs and inputs paint with this, so it stays nearly opaque;
        // our own panels use the translucent `surface` directly.
        t.background = p.surface_raised;
        t.overlay = hsla(0., 0., 0., 0.45);
        t.foreground = p.text;
        t.muted_foreground = p.text_muted;
        t.border = p.hairline;
        t.input = hex(0xFFFFFF, 0.12);
        t.ring = p.border_active;
        t.caret = p.accent;
        t.selection = p.accent.opacity(0.28);
        t.primary = p.accent;
        t.primary_hover = p.accent.opacity(0.88);
        t.primary_active = p.accent.opacity(0.76);
        t.primary_foreground = p.accent_fg;
        // Buttons have their own colour slots that do not follow `primary`.
        t.button_primary = p.accent;
        t.button_primary_hover = p.accent.opacity(0.88);
        t.button_primary_active = p.accent.opacity(0.76);
        t.button_primary_foreground = p.accent_fg;
        t.popover = p.surface_raised;
        t.popover_foreground = p.text;
        t.tab_bar = transparent_black();
        t.tab = transparent_black();
        t.tab_active = transparent_black();
        t.tab_foreground = p.text_muted;
        t.tab_active_foreground = p.text;
        t.title_bar = transparent_black();
        t.title_bar_border = p.hairline;

        // The code editor paints its own (opaque) background; let the panel show through.
        let mut highlight = (*t.highlight_theme).clone();
        highlight.style.editor_background = Some(transparent_black());
        highlight.style.editor_gutter_background = Some(transparent_black());
        highlight.style.editor_active_line = Some(hex(0xFFFFFF, 0.04));
        highlight.style.editor_line_number = Some(p.text_muted.opacity(0.6));
        t.highlight_theme = Arc::new(highlight);
    });
}

/// `0xRRGGBB` + opacity (0.0–1.0).
fn hex(color: u32, alpha: f32) -> Hsla {
    let mut c: Rgba = rgb(color);
    c.a = alpha;
    c.into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    /// Dark and quiet. The default.
    #[default]
    Obsidian,
    /// The cyan glass gradient from the mockup.
    Cyber,
}

impl Variant {
    /// `APIX_THEME=cyber` picks the mockup gradient until the settings screen exists.
    pub fn from_env() -> Self {
        match std::env::var("APIX_THEME").as_deref() {
            Ok(v) if v.eq_ignore_ascii_case("cyber") => Variant::Cyber,
            _ => Variant::Obsidian,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg_from: Hsla,
    pub bg_to: Hsla,
    pub surface: Hsla,
    pub surface_raised: Hsla,
    pub hairline: Hsla,
    pub border_active: Hsla,
    pub accent: Hsla,
    pub accent_fg: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub ok: Hsla,
    pub warn: Hsla,
    pub err: Hsla,
}

impl Palette {
    pub fn new(variant: Variant) -> Self {
        let (bg_from, bg_to) = match variant {
            Variant::Obsidian => (hex(0x0A0E16, 0.92), hex(0x111827, 0.92)),
            Variant::Cyber => (hex(0x0F2A3D, 0.88), hex(0x161E3A, 0.88)),
        };
        Self {
            bg_from,
            bg_to,
            surface: hex(0x0E1726, 0.70),
            // Menus and popovers float over text, so they stay nearly opaque.
            surface_raised: hex(0x132033, 0.97),
            hairline: rgba(0xFFFFFF14).into(),
            border_active: hex(0x22D3EE, 0.60),
            accent: hex(0x22D3EE, 1.0),
            accent_fg: hex(0x04121A, 1.0),
            text: hex(0xE6EDF3, 1.0),
            text_muted: hex(0x8B9BB0, 1.0),
            ok: hex(0x34D399, 1.0),
            warn: hex(0xFBBF24, 1.0),
            err: hex(0xF87171, 1.0),
        }
    }

    pub fn background(&self) -> Background {
        linear_gradient(
            135.,
            linear_color_stop(self.bg_from, 0.),
            linear_color_stop(self.bg_to, 1.),
        )
    }

    pub fn status(&self, code: u16) -> Hsla {
        match code {
            200..300 => self.ok,
            300..400 => self.warn,
            _ => self.err,
        }
    }
}

/// The app icon for the title bar (32 px, drawn at 16 pt for sharp HiDPI).
/// Generated by `cargo run -p icongen --release`.
pub fn app_icon() -> Arc<gpui_kit::Image> {
    static ICON: std::sync::OnceLock<Arc<gpui_kit::Image>> = std::sync::OnceLock::new();
    ICON.get_or_init(|| {
        Arc::new(gpui_kit::Image::from_bytes(
            gpui_kit::ImageFormat::Png,
            include_bytes!("../assets/icon-32.png").to_vec(),
        ))
    })
    .clone()
}

/// Colours for the large-body viewer, close to the code editor's JSON theme.
pub fn json_token_color(p: &Palette, token: JsonToken) -> Hsla {
    match token {
        JsonToken::Key => p.text,
        JsonToken::String => hex(0x86EFAC, 1.0),
        JsonToken::Number => hex(0xFDBA74, 1.0),
        JsonToken::Keyword => hex(0xC4B5FD, 1.0),
        JsonToken::Punctuation => p.text_muted,
    }
}

pub fn method_color(method: Method) -> Hsla {
    match method {
        Method::Get => hex(0x34D399, 1.0),
        Method::Post => hex(0x22D3EE, 1.0),
        Method::Put => hex(0xFBBF24, 1.0),
        Method::Patch => hex(0xFB923C, 1.0),
        Method::Delete => hex(0xF87171, 1.0),
        Method::Head | Method::Options => hex(0xA78BFA, 1.0),
    }
}
