//! User settings, in `settings.toml` next to the app state in the OS config
//! folder. Unknown or missing values fall back to the defaults.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Interface language code; empty follows the system language.
    pub language: String,
    /// `obsidian` or `cyber`. Read at start.
    pub theme: String,
    /// Encrypted collections lock after this many idle minutes; 0 never.
    pub auto_lock_minutes: u32,
    /// Keep sent requests and their responses in the history.
    pub history: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: String::new(),
            theme: "obsidian".into(),
            auto_lock_minutes: 15,
            history: true,
        }
    }
}

impl Settings {
    fn path() -> Option<PathBuf> {
        Some(dirs::config_dir()?.join("Apix").join("settings.toml"))
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = Self::path() else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }

    /// The language to show: the chosen one, else the system's.
    pub fn language_code(&self) -> &'static str {
        crate::i18n::language(&self.language)
            .map(|l| l.code)
            .unwrap_or_else(crate::i18n::system_language)
    }
}
