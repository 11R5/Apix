//! The on-disk format. A collection is just a folder, so it can be committed,
//! diffed and reviewed like code:
//!
//! ```text
//! my-api/
//! ├── apix.toml              # collection name + collection variables
//! ├── environments/
//! │   ├── dev.toml           # [vars] base_url = "http://localhost:8080"
//! │   ├── dev.local.toml     # secrets layered over dev.toml, gitignored
//! │   └── prod.toml
//! ├── users/                 # folders are folders
//! │   ├── list-users.apix    # one request per file (TOML inside)
//! │   └── create-user.apix
//! └── health.apix
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::Request;

pub const COLLECTION_FILE: &str = "apix.toml";
pub const ENVIRONMENTS_DIR: &str = "environments";
/// Request files have their own extension so the OS can open them with Apix.
/// Their content is plain TOML.
pub const REQUEST_EXTENSION: &str = "apix";
/// `dev.local.toml` holds the secrets for `dev.toml` and is never committed.
const LOCAL_SUFFIX: &str = ".local";

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

pub fn is_request_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == REQUEST_EXTENSION)
}

pub fn load_request(path: &Path) -> Result<Request, StoreError> {
    read_toml(path)
}

pub fn save_request(path: &Path, request: &Request) -> Result<(), StoreError> {
    write_toml(path, request)
}

fn write_toml(path: &Path, value: &impl Serialize) -> Result<(), StoreError> {
    let text = toml::to_string_pretty(value)?;
    fs::write(path, text).map_err(|source| StoreError::Io {
        path: path.to_owned(),
        source,
    })
}

/// Variables are sorted on disk so saving never reorders a file and diffs stay small.
pub type VarMap = BTreeMap<String, String>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionMeta {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default)]
    pub vars: VarMap,
}

/// Writes `apix.toml` into `root`.
pub fn save_collection_meta(root: &Path, meta: &CollectionMeta) -> Result<(), StoreError> {
    write_toml(&root.join(COLLECTION_FILE), meta)
}

#[derive(Serialize)]
struct EnvironmentFile<'a> {
    vars: &'a VarMap,
}

/// Writes `environments/<name>.toml`, or `<name>.local.toml` when `local`
/// (secrets, gitignored). Creates the folder when needed.
pub fn save_environment(
    root: &Path,
    name: &str,
    vars: &VarMap,
    local: bool,
) -> Result<PathBuf, StoreError> {
    let dir = root.join(ENVIRONMENTS_DIR);
    fs::create_dir_all(&dir).map_err(|source| StoreError::Io {
        path: dir.clone(),
        source,
    })?;
    let suffix = if local { LOCAL_SUFFIX } else { "" };
    let path = dir.join(format!("{name}{suffix}.toml"));
    write_toml(&path, &EnvironmentFile { vars })?;
    Ok(path)
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
    pub vars: VarMap,
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
        // Absolute first, so a relative path like `health.apix` can still walk up past the cwd.
        let start = std::path::absolute(start).unwrap_or_else(|_| start.to_path_buf());
        start
            .ancestors()
            .find(|dir| dir.join(COLLECTION_FILE).is_file())
            .map(Path::to_path_buf)
    }

    /// Loads `environments/<name>.toml` with `<name>.local.toml` (if present) layered on top.
    pub fn environment(&self, name: &str) -> Result<Environment, StoreError> {
        let dir = self.root.join(ENVIRONMENTS_DIR);
        let mut env: Environment = read_toml(&dir.join(format!("{name}.toml")))?;
        let local = dir.join(format!("{name}{LOCAL_SUFFIX}.toml"));
        if local.is_file() {
            let secrets: Environment = read_toml(&local)?;
            env.vars.extend(secrets.vars);
        }
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
            .filter(|name| !name.ends_with(LOCAL_SUFFIX))
            .collect();
        names.sort();
        names
    }

    /// Every request file in the collection, in a stable (sorted) order.
    pub fn request_paths(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        collect_requests(&self.root, &mut out);
        out
    }
}

fn collect_requests(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        let hidden = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'));
        if hidden {
            continue;
        }
        if path.is_dir() {
            collect_requests(&path, out);
        } else if is_request_file(&path) {
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
            "[vars]\nhost = \"localhost\"\ntoken = \"placeholder\"\n",
        )
        .unwrap();
        fs::write(
            root.join("environments/dev.local.toml"),
            "[vars]\ntoken = \"secret\"\n",
        )
        .unwrap();
        fs::write(
            root.join("health.apix"),
            "name = \"Health\"\nurl = \"https://{{host}}/health\"\n",
        )
        .unwrap();
        // Not a request: other TOML files in the tree are ignored.
        fs::write(root.join("users/notes.toml"), "x = 1\n").unwrap();
        root
    }

    #[test]
    fn opens_collection_and_lists_requests() {
        let root = temp_collection("list");
        let path = root.join("users/create.apix");
        save_request(&path, &Request::new(Method::Post, "https://{{host}}/users")).unwrap();

        let collection = Collection::open(&root).unwrap();
        assert_eq!(collection.meta.name, "Demo");
        assert_eq!(
            collection.request_paths(),
            [root.join("health.apix"), path.clone()]
        );
        assert_eq!(
            Collection::find_root(&path).as_deref(),
            Some(root.as_path())
        );
        assert_eq!(load_request(&path).unwrap().method, Method::Post);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_environment_file_overrides_and_is_not_listed() {
        let root = temp_collection("local");
        let collection = Collection::open(&root).unwrap();

        assert_eq!(collection.environment_names(), ["dev"]);
        let dev = collection.environment("dev").unwrap();
        assert_eq!(dev.vars["host"], "localhost");
        assert_eq!(dev.vars["token"], "secret");

        fs::remove_dir_all(root).unwrap();
    }
}
