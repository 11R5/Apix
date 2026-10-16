//! The on-disk format. A collection is just a folder, so it can be committed,
//! diffed and reviewed like code:
//!
//! ```text
//! my-api/
//! ├── apix.toml              # collection name + collection variables
//! ├── environments/
//! │   ├── dev.toml           # [vars] base_url = "http://localhost:8080"
//! │   └── prod.toml
//! ├── users/                 # folders are folders
//! │   ├── list-users.toml    # one request per file
//! │   └── create-user.toml
//! └── health.toml
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::Request;

pub const COLLECTION_FILE: &str = "apix.toml";
pub const ENVIRONMENTS_DIR: &str = "environments";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("{path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}")]
    Parse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },
    #[error("could not serialize: {0}")]
    Serialize(#[from] toml::ser::Error),
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, StoreError> {
    let text = fs::read_to_string(path).map_err(|source| StoreError::Io {
        path: path.to_owned(),
        source,
    })?;
    toml::from_str(&text).map_err(|source| StoreError::Parse {
        path: path.to_owned(),
        source: Box::new(source),
    })
}

pub fn load_request(path: &Path) -> Result<Request, StoreError> {
    read_toml(path)
}

pub fn save_request(path: &Path, request: &Request) -> Result<(), StoreError> {
    let text = toml::to_string_pretty(request)?;
    fs::write(path, text).map_err(|source| StoreError::Io {
        path: path.to_owned(),
        source,
    })
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionMeta {
    pub name: String,
    #[serde(default)]
    pub vars: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Collection {
    pub root: PathBuf,
    pub meta: CollectionMeta,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Environment {
    #[serde(skip)]
    pub name: String,
    #[serde(default)]
    pub vars: HashMap<String, String>,
}

impl Collection {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root = root.into();
        let meta = read_toml(&root.join(COLLECTION_FILE))?;
        Ok(Self { root, meta })
    }

    /// Walks up from `start` (a request file or any folder inside a
    /// collection) to the nearest folder that contains `apix.toml`.
    pub fn find_root(start: &Path) -> Option<PathBuf> {
        // Absolute first, so a relative path like `health.toml` can still walk up past the cwd.
        let start = std::path::absolute(start).unwrap_or_else(|_| start.to_path_buf());
        start
            .ancestors()
            .find(|dir| dir.join(COLLECTION_FILE).is_file())
            .map(Path::to_path_buf)
    }

    pub fn environment(&self, name: &str) -> Result<Environment, StoreError> {
        let path = self
            .root
            .join(ENVIRONMENTS_DIR)
            .join(format!("{name}.toml"));
        let mut env: Environment = read_toml(&path)?;
        env.name = name.to_owned();
        Ok(env)
    }

    pub fn environment_names(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(self.root.join(ENVIRONMENTS_DIR)) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "toml"))
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect();
        names.sort();
        names
    }

    /// Every request file in the collection, in a stable (sorted) order.
    pub fn request_paths(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        collect_requests(&self.root, true, &mut out);
        out
    }
}

fn collect_requests(dir: &Path, is_root: bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            if !(is_root && name == ENVIRONMENTS_DIR) {
                collect_requests(&path, false, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "toml")
            && !(is_root && name == COLLECTION_FILE)
        {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Method;

    fn temp_collection(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("apix-store-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("environments")).unwrap();
        fs::create_dir_all(root.join("users")).unwrap();
        fs::write(
            root.join(COLLECTION_FILE),
            "name = \"Demo\"\n[vars]\nhost = \"example.com\"\n",
        )
        .unwrap();
        fs::write(
            root.join("environments/dev.toml"),
            "[vars]\nhost = \"localhost\"\n",
        )
        .unwrap();
        fs::write(
            root.join("health.toml"),
            "name = \"Health\"\nurl = \"https://{{host}}/health\"\n",
        )
        .unwrap();
        root
    }

    #[test]
    fn opens_collection_and_lists_requests() {
        let root = temp_collection("list");
        let path = root.join("users/create.toml");
        save_request(&path, &Request::new(Method::Post, "https://{{host}}/users")).unwrap();

        let collection = Collection::open(&root).unwrap();
        assert_eq!(collection.meta.name, "Demo");
        assert_eq!(collection.environment_names(), ["dev"]);
        assert_eq!(
            collection.environment("dev").unwrap().vars["host"],
            "localhost"
        );
        assert_eq!(
            collection.request_paths(),
            [root.join("health.toml"), path.clone()]
        );
        assert_eq!(
            Collection::find_root(&path).as_deref(),
            Some(root.as_path())
        );
        assert_eq!(load_request(&path).unwrap().method, Method::Post);

        fs::remove_dir_all(root).unwrap();
    }
}
