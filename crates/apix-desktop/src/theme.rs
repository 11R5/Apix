//! Themes. A theme is a set of colours, dark or light; Apix has a few built
//! in and reads more from `<config>/Apix/themes/*.toml`. The settings pick a
//! dark and a light theme, which one to show (or follow the system), an
//! accent colour and how see-through the window is (the OS blurs what is
//! behind it).
//!
//! Views never keep a copy of the colours: they read [`current`] when they
//! paint, so [`set`] repaints the whole app in a new theme at once.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use apix_core::Method;
use apix_core::text::JsonToken;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{
    App, Background, Hsla, Rgba, SharedString, WindowAppearance, WindowBackgroundAppearance, hsla,
    linear_color_stop, linear_gradient, rgb, transparent_black,
};
use serde::Deserialize;

use crate::settings::Settings;
use crate::tr;

static CURRENT: std::sync::RwLock<Option<Palette>> = std::sync::RwLock::new(None);

/// Whether the embedded fonts loaded, for every later `set`.
static EMBEDDED_FONTS: AtomicBool = AtomicBool::new(false);

/// The palette now in use.
pub fn current() -> Palette {
    CURRENT
        .read()
        .ok()
        .and_then(|p| *p)
        .unwrap_or_else(|| Palette::build(&builtin()[0], "", 8))
}

/// The first palette, at start.
pub fn init(palette: Palette, embedded_fonts: bool, cx: &mut App) {
    EMBEDDED_FONTS.store(embedded_fonts, Ordering::Relaxed);
    set(palette, cx);
}

/// Makes `palette` the app's, for gpui-kit's components and the window
/// background too, and repaints every window.
pub fn set(palette: Palette, cx: &mut App) {
    apply(&palette, EMBEDDED_FONTS.load(Ordering::Relaxed), cx);
    if let Ok(mut current) = CURRENT.write() {
        *current = Some(palette);
    }
    let background = if palette.glass {
        WindowBackgroundAppearance::Blurred
    } else {
        WindowBackgroundAppearance::Opaque
    };
    for window in cx.windows() {
        window
            .update(cx, |_, window, _| {
                window.set_background_appearance(background)
            })
            .ok();
    }
    cx.refresh_windows();
}

/// The palette the settings ask for, now: with "follow the system", the
/// light or dark theme by the OS appearance.
pub fn from_settings(settings: &Settings, cx: &App) -> Palette {
    let themes = themes().0;
    let find = |id: &str| themes.iter().find(|t| t.id.eq_ignore_ascii_case(id));
    // `APIX_THEME=<id>` shows that theme whatever the settings say.
    let forced = std::env::var("APIX_THEME").ok();
    let def = forced.as_deref().and_then(find).or_else(|| {
        let light = match Appearance::from_id(&settings.appearance) {
            Appearance::Dark => false,
            Appearance::Light => true,
            Appearance::System => system_is_light(cx),
        };
        let (id, fallback) = if light {
            (&settings.light_theme, DAYLIGHT)
        } else {
            (&settings.theme, OBSIDIAN)
        };
        find(id)
            .filter(|t| t.light == light)
            .or_else(|| find(fallback))
    });
    let def = def.cloned().unwrap_or_else(|| builtin().remove(0));
    Palette::build(&def, &settings.accent, settings.transparency)
}

pub fn system_is_light(cx: &App) -> bool {
    matches!(
        cx.window_appearance(),
        WindowAppearance::Light | WindowAppearance::VibrantLight
    )
}

/// Points gpui-kit's component theme at our palette so built-in inputs,
/// tabs, menus and buttons match the rest of the window.
fn apply(palette: &Palette, embedded_fonts: bool, cx: &mut App) {
    // Switching mode reloads the stock colours (and the code colours), so it
    // must happen before our overrides.
    let mode = if palette.light {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    };
    Theme::change(mode, None, cx);
    let p = *palette;
    Theme::update(cx, |t| {
        if embedded_fonts {
            t.font_family = crate::fonts::UI.into();
            t.mono_font_family = crate::fonts::MONO.into();
        }
        // gpui-kit paints this under the whole window, so a see-through
        // window needs it transparent (dialogs set their own, see
        // `bidi::DialogBidi`). Our own panels use `surface` directly.
        t.background = if p.glass {
            transparent_black()
        } else {
            p.surface_raised
        };
        t.overlay = if p.light {
            hsla(0., 0., 0., 0.25)
        } else {
            hsla(0., 0., 0., 0.45)
        };
        t.foreground = p.text;
        t.muted_foreground = p.text_muted;
        t.border = p.hairline;
        t.input = p.input;
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
        t.slider_bar = p.accent;
        t.slider_thumb = p.surface_raised;
        t.popover = p.surface_raised;
        t.popover_foreground = p.text;
        t.tab_bar = transparent_black();
        t.tab = transparent_black();
        t.tab_active = transparent_black();
        t.tab_foreground = p.text_muted;
        t.tab_active_foreground = p.text;
        t.title_bar = transparent_black();
        t.title_bar_border = p.hairline;

        // Top-right toasts would cover the Send button; keep them above the status bar.
        t.notification.placement = gpui_kit::Anchor::BottomRight;
        t.notification.margins.bottom = gpui_kit::px(36.);

        // The code editor paints its own (opaque) background; let the panel show through.
        let mut highlight = (*t.highlight_theme).clone();
        highlight.style.editor_background = Some(transparent_black());
        highlight.style.editor_gutter_background = Some(transparent_black());
        highlight.style.editor_active_line = Some(p.hover.opacity(0.8));
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

/// `#RRGGBB` or `#RRGGBBAA` (the `#` is optional).
pub fn parse_color(text: &str) -> Option<Hsla> {
    let digits = text.trim().trim_start_matches('#');
    let value = u32::from_str_radix(digits, 16).ok()?;
    match digits.len() {
        6 => Some(hex(value, 1.0)),
        8 => Some(hex(value >> 8, (value & 0xFF) as f32 / 255.0)),
        _ => None,
    }
}

/// `#RRGGBB` or, when not opaque, `#RRGGBBAA`.
fn color_text(color: Hsla) -> String {
    let c = color.to_rgb();
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    let mut out = format!("#{:02X}{:02X}{:02X}", byte(c.r), byte(c.g), byte(c.b));
    if c.a < 0.999 {
        out += &format!("{:02X}", byte(c.a));
    }
    out
}

/// Dark, light, or whichever the OS uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Dark,
    Light,
    System,
}

impl Appearance {
    pub const ALL: [Appearance; 3] = [Appearance::Dark, Appearance::Light, Appearance::System];

    /// The name in `settings.toml`.
    pub fn id(self) -> &'static str {
        match self {
            Appearance::Dark => "dark",
            Appearance::Light => "light",
            Appearance::System => "system",
        }
    }

    pub fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|a| a.id() == id)
            .unwrap_or(Appearance::Dark)
    }

    pub fn label(self) -> SharedString {
        match self {
            Appearance::Dark => tr!("settings.appearance_dark"),
            Appearance::Light => tr!("settings.appearance_light"),
            Appearance::System => tr!("settings.appearance_system"),
        }
    }
}

/// Accent colours to pick from: the name in `settings.toml`, the shade for
/// dark themes and the (deeper) shade for light ones. An empty accent keeps
/// the theme's own; `#RRGGBB` in the file works too.
pub const ACCENTS: [(&str, u32, u32); 8] = [
    ("cyan", 0x22D3EE, 0x0891B2),
    ("blue", 0x60A5FA, 0x2563EB),
    ("purple", 0xA78BFA, 0x7C3AED),
    ("pink", 0xF472B6, 0xDB2777),
    ("red", 0xF87171, 0xDC2626),
    ("orange", 0xFB923C, 0xEA580C),
    ("yellow", 0xFACC15, 0xCA8A04),
    ("green", 0x34D399, 0x059669),
];

pub fn accent_label(id: &str) -> SharedString {
    match id {
        "" => tr!("settings.accent_theme"),
        "cyan" => tr!("settings.accent_cyan"),
        "blue" => tr!("settings.accent_blue"),
        "purple" => tr!("settings.accent_purple"),
        "pink" => tr!("settings.accent_pink"),
        "red" => tr!("settings.accent_red"),
        "orange" => tr!("settings.accent_orange"),
        "yellow" => tr!("settings.accent_yellow"),
        "green" => tr!("settings.accent_green"),
        other => other.to_owned().into(),
    }
}

/// The colours a theme sets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colors {
    /// The window background, a gradient from `background` to `background_end`.
    pub background: Hsla,
    pub background_end: Hsla,
    /// Panels, over the background.
    pub surface: Hsla,
    /// Menus, popovers and dialogs, which float over text.
    pub raised: Hsla,
    pub border: Hsla,
    pub accent: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub ok: Hsla,
    pub warning: Hsla,
    pub error: Hsla,
    /// JSON in the large-body viewer.
    pub string: Hsla,
    pub number: Hsla,
    pub keyword: Hsla,
}

impl Colors {
    /// The keys of a theme file's `[colors]`, with each colour.
    fn fields(&mut self) -> [(&'static str, &mut Hsla); 14] {
        [
            ("background", &mut self.background),
            ("background_end", &mut self.background_end),
            ("surface", &mut self.surface),
            ("raised", &mut self.raised),
            ("border", &mut self.border),
            ("accent", &mut self.accent),
            ("text", &mut self.text),
            ("muted", &mut self.muted),
            ("ok", &mut self.ok),
            ("warning", &mut self.warning),
            ("error", &mut self.error),
            ("string", &mut self.string),
            ("number", &mut self.number),
            ("keyword", &mut self.keyword),
        ]
    }
}

/// A theme: built in, or from a file in the themes folder.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeDef {
    /// The name in `settings.toml`: a built-in id or the file's name.
    pub id: String,
    /// What the settings show for a file theme (built-ins are translated).
    pub name: String,
    pub light: bool,
    pub colors: Colors,
    pub file: Option<PathBuf>,
}

impl ThemeDef {
    pub fn label(&self) -> SharedString {
        if self.file.is_some() {
            return self.name.clone().into();
        }
        match self.id.as_str() {
            "obsidian" => tr!("settings.theme_obsidian"),
            "cyber" => tr!("settings.theme_cyber"),
            "daylight" => tr!("settings.theme_daylight"),
            _ => self.name.clone().into(),
        }
    }

    /// The theme as a file to start your own from.
    pub fn to_toml(&self) -> String {
        let mut colors = self.colors;
        let mut out = format!(
            "# An Apix theme. Change the colours, save, and pick it in Settings.\n\
             # Colours are #RRGGBB or #RRGGBBAA (the last two digits are opacity).\n\
             name = \"{} (copy)\"\n\
             light = {}\n\n[colors]\n",
            self.name, self.light
        );
        for (key, color) in colors.fields() {
            out += &format!("{key} = \"{}\"\n", color_text(*color));
        }
        out
    }
}

const OBSIDIAN: &str = "obsidian";
const DAYLIGHT: &str = "daylight";

/// The themes that come with Apix: dark ones first, then light ones.
pub fn builtin() -> Vec<ThemeDef> {
    let dark = |id: &str, name: &str, bg: (u32, u32, f32), surface: u32, raised: u32| {
        let (from, to, alpha) = bg;
        ThemeDef {
            id: id.into(),
            name: name.into(),
            light: false,
            colors: Colors {
                background: hex(from, alpha),
                background_end: hex(to, alpha),
                surface: hex(surface, 0.70),
                raised: hex(raised, 0.97),
                border: hex(0xFFFFFF, 0.08),
                accent: hex(0x22D3EE, 1.0),
                text: hex(0xE6EDF3, 1.0),
                muted: hex(0x8B9BB0, 1.0),
                ok: hex(0x34D399, 1.0),
                warning: hex(0xFBBF24, 1.0),
                error: hex(0xF87171, 1.0),
                string: hex(0x86EFAC, 1.0),
                number: hex(0xFDBA74, 1.0),
                keyword: hex(0xC4B5FD, 1.0),
            },
            file: None,
        }
    };
    let light = |id: &str, name: &str, bg: (u32, u32), surface: u32, raised: u32| {
        let mut def = dark(id, name, (bg.0, bg.1, 0.94), surface, raised);
        def.light = true;
        def.colors = Colors {
            surface: hex(surface, 0.62),
            raised: hex(raised, 0.98),
            border: hex(0x0F172A, 0.12),
            accent: hex(0x0891B2, 1.0),
            text: hex(0x1E293B, 1.0),
            muted: hex(0x5B6B7D, 1.0),
            ok: hex(0x059669, 1.0),
            warning: hex(0xB45309, 1.0),
            error: hex(0xDC2626, 1.0),
            string: hex(0x15803D, 1.0),
            number: hex(0xC2410C, 1.0),
            keyword: hex(0x7C3AED, 1.0),
            ..def.colors
        };
        def
    };

    let obsidian = dark(
        OBSIDIAN,
        "Obsidian",
        (0x0A0E16, 0x111827, 0.92),
        0x0E1726,
        0x132033,
    );
    let cyber = dark(
        "cyber",
        "Cyber",
        (0x0F2A3D, 0x161E3A, 0.88),
        0x0E1726,
        0x132033,
    );
    let mut dracula = dark(
        "dracula",
        "Dracula",
        (0x282A36, 0x21222C, 0.94),
        0x21222C,
        0x343746,
    );
    dracula.colors = Colors {
        accent: hex(0xBD93F9, 1.0),
        text: hex(0xF8F8F2, 1.0),
        muted: hex(0x8C95C4, 1.0),
        ok: hex(0x50FA7B, 1.0),
        warning: hex(0xF1FA8C, 1.0),
        error: hex(0xFF5555, 1.0),
        string: hex(0xF1FA8C, 1.0),
        number: hex(0xBD93F9, 1.0),
        keyword: hex(0xFF79C6, 1.0),
        ..dracula.colors
    };
    let mut nord = dark(
        "nord",
        "Nord",
        (0x2E3440, 0x3B4252, 0.94),
        0x2E3440,
        0x3B4252,
    );
    nord.colors = Colors {
        accent: hex(0x88C0D0, 1.0),
        text: hex(0xECEFF4, 1.0),
        muted: hex(0x9AA5B8, 1.0),
        ok: hex(0xA3BE8C, 1.0),
        warning: hex(0xEBCB8B, 1.0),
        error: hex(0xBF616A, 1.0),
        string: hex(0xA3BE8C, 1.0),
        number: hex(0xB48EAD, 1.0),
        keyword: hex(0x81A1C1, 1.0),
        ..nord.colors
    };
    let solarized = |mut def: ThemeDef, text: u32, muted: u32| {
        def.colors = Colors {
            accent: hex(0x268BD2, 1.0),
            text: hex(text, 1.0),
            muted: hex(muted, 1.0),
            ok: hex(0x859900, 1.0),
            warning: hex(0xB58900, 1.0),
            error: hex(0xDC322F, 1.0),
            string: hex(0x2AA198, 1.0),
            number: hex(0xD33682, 1.0),
            keyword: hex(0x6C71C4, 1.0),
            ..def.colors
        };
        def
    };
    let solarized_dark = solarized(
        dark(
            "solarized-dark",
            "Solarized Dark",
            (0x002B36, 0x073642, 0.95),
            0x002B36,
            0x073642,
        ),
        0xEEE8D5,
        0x93A1A1,
    );
    let daylight = light(
        DAYLIGHT,
        "Daylight",
        (0xF7F9FB, 0xE8EEF5),
        0xFFFFFF,
        0xFFFFFF,
    );
    let mut solarized_light = solarized(
        light(
            "solarized-light",
            "Solarized Light",
            (0xFDF6E3, 0xEEE8D5),
            0xFFFDF6,
            0xFFFBEE,
        ),
        0x073642,
        0x657B83,
    );
    solarized_light.colors.border = hex(0x586E75, 0.18);
    vec![
        obsidian,
        cyber,
        dracula,
        nord,
        solarized_dark,
        daylight,
        solarized_light,
    ]
}

/// `<config>/Apix/themes`.
pub fn themes_dir() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("Apix").join("themes"))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    name: Option<String>,
    #[serde(default)]
    light: bool,
    #[serde(default)]
    colors: BTreeMap<String, String>,
}

/// Reads a theme file. Colours it leaves out come from Obsidian (or, for a
/// light theme, Daylight).
pub fn parse_theme(id: &str, text: &str) -> Result<ThemeDef, String> {
    let file: ThemeFile = toml::from_str(text).map_err(|e| e.message().to_owned())?;
    let base = if file.light { DAYLIGHT } else { OBSIDIAN };
    let mut def = builtin()
        .into_iter()
        .find(|t| t.id == base)
        .expect("built-in base theme");
    def.id = id.to_owned();
    def.name = file.name.unwrap_or_else(|| id.to_owned());
    for (key, value) in &file.colors {
        let mut found = false;
        for (name, slot) in def.colors.fields() {
            if name == key {
                *slot = parse_color(value)
                    .ok_or_else(|| format!("{key}: \"{value}\" is not a #RRGGBB colour"))?;
                found = true;
            }
        }
        if !found {
            return Err(format!("unknown colour \"{key}\""));
        }
    }
    Ok(def)
}

/// Every theme, and what went wrong reading theme files. A file theme with a
/// built-in's name replaces it.
pub fn themes() -> (Vec<ThemeDef>, Vec<String>) {
    let mut all = builtin();
    let mut errors = Vec::new();
    let Some(dir) = themes_dir() else {
        return (all, errors);
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return (all, errors);
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .collect();
    paths.sort();
    for path in paths {
        let id = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let parsed = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| parse_theme(&id, &text));
        match parsed {
            Ok(mut def) => {
                def.file = Some(path);
                match all.iter_mut().find(|t| t.id.eq_ignore_ascii_case(&def.id)) {
                    Some(slot) => *slot = def,
                    None => all.push(def),
                }
            }
            Err(e) => errors.push(format!("{}: {e}", file_name(&path))),
        }
    }
    (all, errors)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The most of the window that can be see-through, so text stays on something.
pub const MAX_TRANSPARENCY: u32 = 90;

/// The colours views paint with: a theme's, with the accent and
/// transparency settings applied and a few derived from them.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg_from: Hsla,
    pub bg_to: Hsla,
    pub surface: Hsla,
    pub surface_raised: Hsla,
    pub hairline: Hsla,
    pub border_active: Hsla,
    pub accent: Hsla,
    /// Text on the accent colour.
    pub accent_fg: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub ok: Hsla,
    pub warn: Hsla,
    pub err: Hsla,
    /// A row under the mouse.
    pub hover: Hsla,
    /// Input fields.
    pub input: Hsla,
    pub string: Hsla,
    pub number: Hsla,
    pub keyword: Hsla,
    pub light: bool,
    /// The window is see-through (with the OS blur behind it).
    pub glass: bool,
}

impl Palette {
    /// `transparency` is a percentage: 0 paints the window solid; more lets
    /// the blurred desktop through the background and, less so, the panels.
    pub fn build(def: &ThemeDef, accent: &str, transparency: u32) -> Self {
        let c = &def.colors;
        let accent = ACCENTS
            .iter()
            .find(|(id, _, _)| *id == accent)
            .map(|(_, dark, light)| hex(if def.light { *light } else { *dark }, 1.0))
            .or_else(|| parse_color(accent).filter(|_| accent.starts_with('#')))
            .unwrap_or(c.accent);
        let mut t = transparency.min(MAX_TRANSPARENCY) as f32 / 100.0;
        let glass = t > 0.0;
        // Dark text on a light window turns to mud over whatever dark is
        // behind it, so light themes keep more of their white.
        if def.light {
            t *= 0.7;
        }
        // The theme's colours, with the background as opaque as the slider says.
        let window = |color: Hsla| Hsla {
            a: 1.0 - t,
            ..color
        };
        // Panels thin out at half the rate, so text keeps a backing.
        let panel = Hsla {
            a: c.surface.a * (1.0 - t * 0.6),
            ..c.surface
        };
        // Glass catches light at its edges: borders brighten as it clears.
        let hairline = if glass {
            let rim = if def.light { 1.0 } else { 0.9 };
            let base = c.border;
            Hsla {
                l: base.l + (rim - base.l) * t * 0.6,
                a: base.a + t * 0.12,
                ..base
            }
        } else {
            c.border
        };
        let (hover, input) = if def.light {
            (hsla(0., 0., 0., 0.05), hsla(0., 0., 0., 0.06))
        } else {
            (hsla(0., 0., 1., 0.05), hsla(0., 0., 1., 0.12))
        };
        Self {
            bg_from: window(c.background),
            bg_to: window(c.background_end),
            surface: panel,
            surface_raised: c.raised,
            hairline,
            border_active: accent.opacity(0.6),
            accent,
            // Above ~0.18, dark text has more contrast than white (WCAG).
            accent_fg: if luminance(accent) > 0.18 {
                hex(0x04121A, 1.0)
            } else {
                hex(0xFFFFFF, 1.0)
            },
            text: c.text,
            text_muted: c.muted,
            ok: c.ok,
            warn: c.warning,
            err: c.error,
            hover,
            input,
            string: c.string,
            number: c.number,
            keyword: c.keyword,
            light: def.light,
            glass,
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

/// Relative luminance (0 black … 1 white), to pick text that reads on a colour.
fn luminance(color: Hsla) -> f32 {
    let c = color.to_rgb();
    let channel = |v: f32| {
        if v <= 0.039_28 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
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
        JsonToken::String => p.string,
        JsonToken::Number => p.number,
        JsonToken::Keyword => p.keyword,
        JsonToken::Punctuation => p.text_muted,
    }
}

/// Colours a collection can be given in a project, by the name saved in
/// `apix-project.toml`.
pub const COLLECTION_COLORS: [&str; 8] = [
    "red", "orange", "yellow", "green", "teal", "blue", "purple", "pink",
];

pub fn collection_color(name: &str) -> Option<Hsla> {
    let light = current().light;
    let pick = |dark: u32, deep: u32| hex(if light { deep } else { dark }, 1.0);
    Some(match name {
        "red" => pick(0xF87171, 0xDC2626),
        "orange" => pick(0xFB923C, 0xEA580C),
        "yellow" => pick(0xFACC15, 0xCA8A04),
        "green" => pick(0x4ADE80, 0x16A34A),
        "teal" => pick(0x2DD4BF, 0x0D9488),
        "blue" => pick(0x60A5FA, 0x2563EB),
        "purple" => pick(0xA78BFA, 0x7C3AED),
        "pink" => pick(0xF472B6, 0xDB2777),
        _ => return None,
    })
}

/// gRPC requests (indigo) in the sidebar and the URL bar.
pub fn grpc_color() -> Hsla {
    hex(if current().light { 0x4F46E5 } else { 0x818CF8 }, 1.0)
}

/// MQTT requests (pink) in the sidebar and the URL bar.
pub fn mqtt_color() -> Hsla {
    hex(if current().light { 0xDB2777 } else { 0xF472B6 }, 1.0)
}

/// Method colours; deeper shades on light themes so they stay readable.
pub fn method_color(method: Method) -> Hsla {
    let light = current().light;
    let pick = |dark: u32, deep: u32| hex(if light { deep } else { dark }, 1.0);
    match method {
        Method::Get => pick(0x34D399, 0x059669),
        Method::Post => pick(0x22D3EE, 0x0891B2),
        Method::Put => pick(0xFBBF24, 0xB45309),
        Method::Patch => pick(0xFB923C, 0xC2410C),
        Method::Delete => pick(0xF87171, 0xDC2626),
        Method::Head | Method::Options => pick(0xA78BFA, 0x7C3AED),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_parse_and_print() {
        let c = parse_color("#22D3EE").unwrap();
        assert_eq!(color_text(c), "#22D3EE");
        let c = parse_color("FFFFFF14").unwrap();
        assert_eq!(color_text(c), "#FFFFFF14");
        assert!(parse_color("#12345").is_none());
        assert!(parse_color("teal").is_none());
    }

    #[test]
    fn theme_files_fill_in_from_a_base() {
        let def = parse_theme(
            "ocean",
            "name = \"Ocean\"\n[colors]\naccent = \"#FF0000\"\nbackground = \"#000000CC\"\n",
        )
        .unwrap();
        assert_eq!(def.name, "Ocean");
        assert!(!def.light);
        assert_eq!(color_text(def.colors.accent), "#FF0000");
        assert_eq!(color_text(def.colors.background), "#000000CC");
        let obsidian = &builtin()[0];
        assert_eq!(def.colors.text, obsidian.colors.text);

        let light = parse_theme("paper", "light = true\n").unwrap();
        assert!(light.light);
        assert_eq!(light.name, "paper");

        assert!(parse_theme("x", "[colors]\naccent = \"red\"\n").is_err());
        assert!(parse_theme("x", "[colors]\nglow = \"#FFFFFF\"\n").is_err());
        assert!(parse_theme("x", "colour = 1\n").is_err());
    }

    #[test]
    fn a_theme_written_out_reads_back_the_same() {
        for def in builtin() {
            let back = parse_theme(&def.id, &def.to_toml()).unwrap();
            assert_eq!(back.light, def.light, "{}", def.id);
            let (mut a, mut b) = (back.colors, def.colors);
            for ((key, a), (_, b)) in a.fields().into_iter().zip(b.fields()) {
                assert_eq!(color_text(*a), color_text(*b), "{} {key}", def.id);
            }
        }
    }

    #[test]
    fn accent_and_transparency() {
        let themes = builtin();
        let obsidian = &themes[0];
        let daylight = themes.iter().find(|t| t.id == "daylight").unwrap();
        let p = Palette::build(obsidian, "purple", 20);
        assert_eq!(color_text(p.accent), "#A78BFA");
        assert_eq!(color_text(p.accent_fg), "#04121A");
        let p = Palette::build(daylight, "purple", 20);
        assert_eq!(color_text(p.accent), "#7C3AED");
        assert_eq!(color_text(p.accent_fg), "#FFFFFF");
        assert!(p.light);
        // Light windows clear more gently.
        assert!((p.bg_from.a - 0.86).abs() < 0.001);
        let p = Palette::build(obsidian, "#123456", 0);
        assert_eq!(color_text(p.accent), "#123456");
        assert!(!p.glass);
        assert_eq!(p.bg_from.a, 1.0);
        assert_eq!(p.surface.a, obsidian.colors.surface.a);
        let p = Palette::build(obsidian, "", 60);
        assert_eq!(color_text(p.accent), "#22D3EE");
        assert!(p.glass);
        assert!((p.bg_from.a - 0.4).abs() < 0.001);
        // Panels clear more slowly than the window, and edges brighten.
        assert!(p.surface.a > 0.4 && p.surface.a < obsidian.colors.surface.a);
        assert!(p.hairline.a > obsidian.colors.border.a);
        // Never fully see-through.
        let p = Palette::build(obsidian, "", 100);
        assert!((p.bg_from.a - 0.1).abs() < 0.001);
    }
}
