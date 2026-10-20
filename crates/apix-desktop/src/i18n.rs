//! Interface languages. Every text the app shows is looked up by key in
//! `locales/<code>.yml` (rust-i18n, the same system gpui-kit uses, so both
//! follow one locale). A missing text falls back to English.
//!
//! Adding a language: copy `locales/en.yml` to `locales/<code>.yml`,
//! translate the values, and add one line to [`LANGUAGES`].

use gpui_kit::SharedString;

pub struct Language {
    /// The locale code, also the file name in `locales/`.
    pub code: &'static str,
    /// The language's own name, so anyone can find theirs.
    pub name: &'static str,
    /// Written right to left: the layout mirrors (see `crate::bidi`).
    pub rtl: bool,
}

pub const LANGUAGES: &[Language] = &[
    Language {
        code: "en",
        name: "English",
        rtl: false,
    },
    Language {
        code: "tr",
        name: "Türkçe",
        rtl: false,
    },
    Language {
        code: "ar",
        name: "العربية",
        rtl: true,
    },
    Language {
        code: "de",
        name: "Deutsch",
        rtl: false,
    },
    Language {
        code: "fr",
        name: "Français",
        rtl: false,
    },
    Language {
        code: "ru",
        name: "Русский",
        rtl: false,
    },
    Language {
        code: "ja",
        name: "日本語",
        rtl: false,
    },
    // `zh-CN` is also the code gpui-kit's own texts use.
    Language {
        code: "zh-CN",
        name: "简体中文",
        rtl: false,
    },
    Language {
        code: "es",
        name: "Español",
        rtl: false,
    },
];

/// A text in the current language, to show.
/// `tr!("menu.file")`, `tr!("vault.locked", name = name)` for `%{name}`.
/// In a right-to-left language it reads right to left as a whole (see
/// [`display`]), so never save it or use it as data: use `tr_plain!` for that.
#[macro_export]
macro_rules! tr {
    ($($args:tt)*) => {
        $crate::i18n::display(rust_i18n::t!($($args)*))
    };
}

/// A text in the current language as plain text, for what is saved or used
/// as data (a default request name written to a file).
#[macro_export]
macro_rules! tr_plain {
    ($($args:tt)*) => {
        gpui_kit::SharedString::from(rust_i18n::t!($($args)*).into_owned())
    };
}

/// Right-to-left embedding (U+202B) and its end (U+202C). DirectWrite ignores
/// the newer isolates (U+2067…U+2069), so embeddings it is.
const RTL_EMBEDDING: char = '\u{202B}';
const POP_EMBEDDING: char = '\u{202C}';

/// `text` ready to show in the current language. gpui lays every line out
/// left to right, so in an RTL language a sentence that starts or ends with a
/// Latin word ("URL", "OAuth 2.0", "Ctrl+S") would come out with that word on
/// the wrong side. Wrapped in a right-to-left embedding, the text is ordered
/// right to left whatever it starts with.
pub fn display(text: impl AsRef<str>) -> SharedString {
    let text = text.as_ref();
    if current().rtl && !text.is_empty() {
        format!("{RTL_EMBEDDING}{text}{POP_EMBEDDING}").into()
    } else {
        text.to_owned().into()
    }
}

/// `text` without the direction marks [`display`] adds.
pub fn strip_marks(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(*c, RTL_EMBEDDING | POP_EMBEDDING))
        .collect()
}

pub fn language(code: &str) -> Option<&'static Language> {
    LANGUAGES.iter().find(|l| l.code.eq_ignore_ascii_case(code))
}

/// The language in use.
pub fn current() -> &'static Language {
    language(&rust_i18n::locale()).unwrap_or(&LANGUAGES[0])
}

/// Switches every text (Apix's and gpui-kit's) to `code`.
pub fn set(code: &str) {
    let code = language(code).map_or("en", |l| l.code);
    rust_i18n::set_locale(code);
}

/// The system's language if Apix has it, e.g. `tr-TR` → `tr`, `zh-Hans-CN` → `zh-CN`.
pub fn system_language() -> &'static str {
    let Some(locale) = sys_locale::get_locale() else {
        return "en";
    };
    let lower = locale.to_ascii_lowercase();
    if lower.starts_with("zh") {
        // Traditional Chinese is not translated yet; simplified is closer than English.
        return "zh-CN";
    }
    let base = lower.split(['-', '_']).next().unwrap_or_default();
    language(base).map_or("en", |l| l.code)
}

/// Upper case for small section titles. Turkish dotted i becomes İ (Rust's
/// `to_uppercase` makes it I); scripts without case stay as they are.
pub fn upper(text: &str) -> SharedString {
    if current().code == "tr" {
        text.chars()
            .flat_map(|c| match c {
                'i' => "İ".chars().collect::<Vec<_>>(),
                'ı' => vec!['I'],
                c => c.to_uppercase().collect(),
            })
            .collect::<String>()
            .into()
    } else {
        text.to_uppercase().into()
    }
}

/// The names to offer in a language picker, in [`LANGUAGES`] order.
pub fn names() -> Vec<SharedString> {
    LANGUAGES
        .iter()
        .map(|l| SharedString::from(l.name))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::LANGUAGES;

    /// `key: value` lines of a locale file (flat keys, one line each).
    fn entries(code: &str) -> BTreeMap<String, String> {
        let path = format!("{}/locales/{code}.yml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        // Some Windows editors add a byte-order mark.
        let text = text.trim_start_matches('\u{feff}');
        text.lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with('_') && !l.trim().is_empty())
            .map(|l| {
                let (k, v) = l
                    .split_once(": ")
                    .unwrap_or_else(|| panic!("{code}.yml: not `key: value`: {l}"));
                (k.trim().to_owned(), v.trim().to_owned())
            })
            .collect()
    }

    fn placeholders(text: &str) -> BTreeSet<&str> {
        text.split("%{")
            .skip(1)
            .filter_map(|rest| rest.split_once('}').map(|(name, _)| name))
            .collect()
    }

    #[test]
    fn every_language_has_every_text_with_the_same_placeholders() {
        let english = entries("en");
        assert!(english.len() > 100, "en.yml looks empty");
        for language in &LANGUAGES[1..] {
            let other = entries(language.code);
            let missing: Vec<_> = english.keys().filter(|k| !other.contains_key(*k)).collect();
            assert!(
                missing.is_empty(),
                "{}.yml lacks {missing:?}",
                language.code
            );
            let extra: Vec<_> = other.keys().filter(|k| !english.contains_key(*k)).collect();
            assert!(
                extra.is_empty(),
                "{}.yml has unknown {extra:?}",
                language.code
            );
            for (key, text) in &english {
                assert_eq!(
                    placeholders(text),
                    placeholders(&other[key]),
                    "{}.yml: {key} must use the same %{{…}} names as English",
                    language.code
                );
            }
        }
    }

    #[test]
    fn every_language_is_compiled_in_and_fills_placeholders() {
        let available = rust_i18n::available_locales!();
        for language in LANGUAGES {
            assert!(
                available.iter().any(|l| l == language.code),
                "{} not compiled in",
                language.code
            );
            let text = rust_i18n::t!(
                "settings.auto_lock_minutes",
                locale = language.code,
                count = 7
            );
            assert!(
                text.contains('7') && !text.contains("%{"),
                "{}: {text}",
                language.code
            );
        }
        assert_eq!(rust_i18n::t!("menu.file", locale = "tr"), "Dosya");
        assert_eq!(rust_i18n::t!("menu.file", locale = "de"), "Datei");
    }

    #[test]
    fn direction_marks_come_off_again() {
        let marked = format!("{}تُلحق URL.{}", super::RTL_EMBEDDING, super::POP_EMBEDDING);
        assert_eq!(super::strip_marks(&marked), "تُلحق URL.");
    }

    #[test]
    fn system_locales_map_to_supported_languages() {
        assert!(super::language("TR").is_some());
        assert!(super::language("xx").is_none());
    }
}
