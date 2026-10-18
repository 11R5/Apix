//! Per-user app state that is not part of any collection: recently opened
//! collections and the environment chosen for each. Stored as TOML in the OS
//! config folder (`%APPDATA%\Apix`, `~/Library/Application Support/Apix`,
//! `~/.config/Apix`). Losing it is harmless, so errors are ignored.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_RECENT: usize = 8;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppState {
    /// Most recently opened collection roots, newest first.
    #[serde(default)]
    pub recent: Vec<PathBuf>,
    /// Chosen environment per collection root.
    #[serde(default)]
    pub environments: BTreeMap<String, String>,
}

impl AppState {
    fn path() -> Option<PathBuf> {
        Some(dirs::config_dir()?.join("Apix").join("state.toml"))
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = toml::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }

    pub fn remember_collection(&mut self, root: &Path) {
        self.recent.retain(|p| p != root);
        self.recent.insert(0, root.to_owned());
        self.recent.truncate(MAX_RECENT);
        self.save();
    }

    /// Recent collections that still exist on disk.
    pub fn recent_collections(&self) -> impl Iterator<Item = &PathBuf> {
        self.recent.iter().filter(|p| {
            p.join(apix_core::store::COLLECTION_FILE).is_file() || apix_core::vfs::is_encrypted(p)
        })
    }

    pub fn environment_for(&self, root: &Path) -> Option<String> {
        self.environments.get(&key(root)).cloned()
    }

    pub fn set_environment(&mut self, root: &Path, name: Option<&str>) {
        match name {
            Some(name) => self.environments.insert(key(root), name.to_owned()),
            None => self.environments.remove(&key(root)),
        };
        self.save();
    }
}

fn key(root: &Path) -> String {
    root.to_string_lossy().into_owned()
}
