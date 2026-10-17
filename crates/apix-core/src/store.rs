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
    #[error("no environment named `{0}`")]
    UnknownEnvironment(String),
}

pub(crate) fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, StoreError> {
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

pub(crate) fn write_toml(path: &Path, value: &impl Serialize) -> Result<(), StoreError> {
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
    #[serde(default, skip_serializing_if = "VarMap::is_empty")]
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
    if local {
        ensure_local_ignored(root)?;
    }
    Ok(path)
}

const LOCAL_PATTERN: &str = "*.local.toml";

/// Makes sure `<root>/.gitignore` ignores `*.local.toml`, so secrets are
/// never committed. Returns whether the file had to be changed.
pub fn ensure_local_ignored(root: &Path) -> Result<bool, StoreError> {
    let path = root.join(".gitignore");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(source) => return Err(StoreError::Io { path, source }),
    };
    let ignored = text
        .lines()
        .map(str::trim)
        .any(|line| line == LOCAL_PATTERN || line == format!("**/{LOCAL_PATTERN}"));
    if ignored {
        return Ok(false);
    }
    let mut text = text;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("# Apix secrets\n");
    text.push_str(LOCAL_PATTERN);
    text.push('\n');
    fs::write(&path, text).map_err(io_error(&path))?;
    Ok(true)
}

/// One variable of an environment as an editor shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
    /// Kept in `<name>.local.toml` instead of the shared file.
    pub secret: bool,
}

/// An environment's two files: the shared values and the secrets layered over them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvironmentParts {
    pub vars: VarMap,
    pub secrets: VarMap,
}

impl EnvironmentParts {
    /// Reads `environments/<name>.toml` and `<name>.local.toml`; missing files are empty.
    pub fn read(root: &Path, name: &str) -> Result<Self, StoreError> {
        let dir = root.join(ENVIRONMENTS_DIR);
        let part = |file: String| -> Result<VarMap, StoreError> {
            let path = dir.join(file);
            if path.is_file() {
                Ok(read_toml::<Environment>(&path)?.vars)
            } else {
                Ok(VarMap::new())
            }
        };
        Ok(Self {
            vars: part(format!("{name}.toml"))?,
            secrets: part(format!("{name}{LOCAL_SUFFIX}.toml"))?,
        })
    }

    /// One row per key, sorted. A key with a secret shows the secret.
    pub fn rows(&self) -> Vec<EnvVar> {
        let mut rows: Vec<EnvVar> = self
            .vars
            .iter()
            .filter(|(key, _)| !self.secrets.contains_key(*key))
            .map(|(key, value)| EnvVar {
                key: key.clone(),
                value: value.clone(),
                secret: false,
            })
            .chain(self.secrets.iter().map(|(key, value)| EnvVar {
                key: key.clone(),
                value: value.clone(),
                secret: true,
            }))
            .collect();
        rows.sort_by(|a, b| a.key.cmp(&b.key));
        rows
    }

    /// The files for edited `rows`. Rows without a key are dropped. A secret
    /// keeps the placeholder it had in the shared file (so teammates still
    /// see the name), but its value only goes to the secrets file.
    pub fn with_rows(&self, rows: &[EnvVar]) -> Self {
        let mut out = Self::default();
        for row in rows {
            let key = row.key.trim();
            if key.is_empty() {
                continue;
            }
            if row.secret {
                out.secrets.insert(key.to_owned(), row.value.clone());
                if let Some(placeholder) = self.vars.get(key) {
                    out.vars.insert(key.to_owned(), placeholder.clone());
                }
            } else {
                out.vars.insert(key.to_owned(), row.value.clone());
            }
        }
        out
    }

    /// Writes both files. The secrets file is removed when there are no secrets.
    pub fn write(&self, root: &Path, name: &str) -> Result<(), StoreError> {
        save_environment(root, name, &self.vars, false)?;
        if self.secrets.is_empty() {
            let local = local_environment_path(root, name);
            if local.is_file() {
                fs::remove_file(&local).map_err(io_error(&local))?;
            }
        } else {
            save_environment(root, name, &self.secrets, true)?;
        }
        Ok(())
    }
}

fn local_environment_path(root: &Path, name: &str) -> PathBuf {
    root.join(ENVIRONMENTS_DIR)
        .join(format!("{name}{LOCAL_SUFFIX}.toml"))
}

/// The files of an environment that exist, shared file first.
pub fn environment_files(root: &Path, name: &str) -> Vec<PathBuf> {
    [
        root.join(ENVIRONMENTS_DIR).join(format!("{name}.toml")),
        local_environment_path(root, name),
    ]
    .into_iter()
    .filter(|p| p.is_file())
    .collect()
}

/// Creates an empty environment named after `title`. Returns its file name,
/// or an `AlreadyExists` error if that environment is already there.
pub fn create_environment(root: &Path, title: &str) -> Result<String, StoreError> {
    let name = slug(title, "environment");
    let path = root.join(ENVIRONMENTS_DIR).join(format!("{name}.toml"));
    if !environment_files(root, &name).is_empty() {
        return Err(already_exists(path));
    }
    save_environment(root, &name, &VarMap::new(), false)?;
    Ok(name)
}

/// Renames both files of an environment. Returns the new name.
pub fn rename_environment(root: &Path, old: &str, title: &str) -> Result<String, StoreError> {
    let name = slug(title, "environment");
    if name == old {
        return Ok(name);
    }
    let dir = root.join(ENVIRONMENTS_DIR);
    if !environment_files(root, &name).is_empty() {
        return Err(already_exists(dir.join(format!("{name}.toml"))));
    }
    for suffix in ["", LOCAL_SUFFIX] {
        let from = dir.join(format!("{old}{suffix}.toml"));
        if from.is_file() {
            let to = dir.join(format!("{name}{suffix}.toml"));
            fs::rename(&from, &to).map_err(io_error(&from))?;
        }
    }
    Ok(name)
}

fn already_exists(path: PathBuf) -> StoreError {
    StoreError::Io {
        path,
        source: std::io::Error::new(std::io::ErrorKind::AlreadyExists, "already exists"),
    }
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
        let path = self
            .root
            .join(ENVIRONMENTS_DIR)
            .join(format!("{name}.toml"));
        let vars = environment_vars_at(&self.root, name)?.ok_or_else(|| StoreError::Io {
            path,
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "no such environment"),
        })?;
        Ok(Environment {
            name: name.to_owned(),
            vars,
        })
    }

    pub fn environment_names(&self) -> Vec<String> {
        environment_names_at(&self.root)
    }

    /// Every request file in the collection, in a stable (sorted) order.
    pub fn request_paths(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        collect_requests(&self.root, &mut out);
        out
    }
}

/// Environment names in `<root>/environments/`, without the `.local` secret files.
pub fn environment_names_at(root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root.join(ENVIRONMENTS_DIR)) else {
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

/// `<root>/environments/<name>.toml` with its `.local.toml` secrets on top,
/// or `None` when neither file exists.
pub fn environment_vars_at(root: &Path, name: &str) -> Result<Option<VarMap>, StoreError> {
    let dir = root.join(ENVIRONMENTS_DIR);
    let base = dir.join(format!("{name}.toml"));
    let local = dir.join(format!("{name}{LOCAL_SUFFIX}.toml"));
    if !base.is_file() && !local.is_file() {
        return Ok(None);
    }
    let mut vars = VarMap::new();
    for path in [base, local] {
        if path.is_file() {
            let env: Environment = read_toml(&path)?;
            vars.extend(env.vars);
        }
    }
    Ok(Some(vars))
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

/// The collection as the sidebar shows it: folders first, then requests,
/// each sorted by name. Paths are absolute. Empty folders are included so a
/// folder you just created shows up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeNode {
    Folder {
        name: String,
        path: PathBuf,
        children: Vec<TreeNode>,
    },
    Request {
        path: PathBuf,
    },
}

impl Collection {
    pub fn tree(&self) -> Vec<TreeNode> {
        tree_of(&self.root, true)
    }
}

fn tree_of(dir: &Path, is_root: bool) -> Vec<TreeNode> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut folders = Vec::new();
    let mut requests = Vec::new();
    for path in entries.filter_map(Result::ok).map(|e| e.path()) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.starts_with('.') || (is_root && name == ENVIRONMENTS_DIR) {
            continue;
        }
        if path.is_dir() {
            folders.push((name, path));
        } else if is_request_file(&path) {
            requests.push(path);
        }
    }
    folders.sort_by_key(|(name, _)| name.to_lowercase());
    requests.sort_by_key(|p| p.to_string_lossy().to_lowercase());

    let mut out: Vec<TreeNode> = folders
        .into_iter()
        .map(|(name, path)| TreeNode::Folder {
            children: tree_of(&path, false),
            name,
            path,
        })
        .collect();
    out.extend(requests.into_iter().map(|path| TreeNode::Request { path }));
    out
}

/// A file or folder name made from a free-form title: lowercase, words
/// joined by `-`, safe on every OS. `Create User (v2)` → `create-user-v2`.
pub fn slug(name: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.chars().flat_map(char::to_lowercase) {
        // `İ` lowercases to `i` + U+0307 (combining dot); drop the mark.
        if c == '\u{0307}' {
            continue;
        }
        if c.is_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !out.is_empty() && !dash {
            out.push('-');
            dash = true;
        }
    }
    let mut out: String = out.chars().take(80).collect();
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out = fallback.to_owned();
    }
    const RESERVED: [&str; 4] = ["con", "prn", "aux", "nul"];
    let reserved_numbered = (out.starts_with("com") || out.starts_with("lpt"))
        && out.len() == 4
        && out.as_bytes()[3].is_ascii_digit();
    if RESERVED.contains(&out.as_str()) || reserved_numbered {
        out.push_str("-1");
    }
    out
}

/// `dir/stem.ext`, or `dir/stem-2.ext`, `-3`, ... if that is taken.
pub fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let with_ext = |s: &str| {
        if ext.is_empty() {
            dir.join(s)
        } else {
            dir.join(format!("{s}.{ext}"))
        }
    };
    let first = with_ext(stem);
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| with_ext(&format!("{stem}-{n}")))
        .find(|p| !p.exists())
        .expect("an unused name always exists")
}

pub(crate) fn io_error(path: &Path) -> impl FnOnce(std::io::Error) -> StoreError + '_ {
    move |source| StoreError::Io {
        path: path.to_owned(),
        source,
    }
}

fn stem_of(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Script files that belong to a request: `<stem>.prerequest.js`, `<stem>.test.js`.
fn sidecars(path: &Path) -> Vec<PathBuf> {
    let (Some(dir), stem) = (path.parent(), stem_of(path)) else {
        return Vec::new();
    };
    let prefix = format!("{stem}.");
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            name.starts_with(&prefix) && name.ends_with(".js")
        })
        .collect();
    out.sort();
    out
}

/// The request file and its scripts, e.g. for moving them to the trash together.
pub fn request_files(path: &Path) -> Vec<PathBuf> {
    let mut files = vec![path.to_owned()];
    files.extend(sidecars(path));
    files
}

/// Saves a new request in `dir`, named after `request.name`. Returns its path.
pub fn create_request(dir: &Path, request: &Request) -> Result<PathBuf, StoreError> {
    let path = unique_path(dir, &slug(&request.name, "request"), REQUEST_EXTENSION);
    save_request(&path, request)?;
    Ok(path)
}

pub fn create_folder(parent: &Path, name: &str) -> Result<PathBuf, StoreError> {
    let path = unique_path(parent, &slug(name, "folder"), "");
    fs::create_dir_all(&path).map_err(io_error(&path))?;
    Ok(path)
}

/// Renames a folder to the slug of `name`. Returns the new path.
pub fn rename_folder(path: &Path, name: &str) -> Result<PathBuf, StoreError> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = slug(name, "folder");
    if path
        .file_name()
        .is_some_and(|n| n.to_string_lossy() == stem)
    {
        return Ok(path.to_owned());
    }
    let target = unique_path(parent, &stem, "");
    fs::rename(path, &target).map_err(io_error(path))?;
    Ok(target)
}

/// Renames a request: its `name`, and its file (plus scripts) to match.
pub fn rename_request(path: &Path, new_name: &str) -> Result<PathBuf, StoreError> {
    let mut request = load_request(path)?;
    request.name = new_name.trim().to_owned();
    let dir = path.parent().unwrap_or(Path::new("."));
    let old_stem = stem_of(path);
    let new_stem = slug(new_name, "request");
    if new_stem == old_stem {
        save_request(path, &request)?;
        return Ok(path.to_owned());
    }

    let new_path = unique_path(dir, &new_stem, REQUEST_EXTENSION);
    let final_stem = stem_of(&new_path);
    save_request(&new_path, &request)?;
    for script in sidecars(path) {
        let name = script.file_name().unwrap_or_default().to_string_lossy();
        let target = dir.join(format!("{final_stem}{}", &name[old_stem.len()..]));
        if !target.exists() {
            fs::rename(&script, &target).map_err(io_error(&script))?;
        }
    }
    fs::remove_file(path).map_err(io_error(path))?;
    Ok(new_path)
}

/// Copies a request (and its scripts) next to it as "<name> copy".
pub fn duplicate_request(path: &Path) -> Result<PathBuf, StoreError> {
    let mut request = load_request(path)?;
    request.name = format!("{} copy", request.name);
    let dir = path.parent().unwrap_or(Path::new("."));
    let old_stem = stem_of(path);
    let new_path = unique_path(dir, &format!("{old_stem}-copy"), REQUEST_EXTENSION);
    let new_stem = stem_of(&new_path);
    save_request(&new_path, &request)?;
    for script in sidecars(path) {
        let name = script.file_name().unwrap_or_default().to_string_lossy();
        let target = dir.join(format!("{new_stem}{}", &name[old_stem.len()..]));
        fs::copy(&script, &target).map_err(io_error(&script))?;
    }
    Ok(new_path)
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
    fn tree_lists_folders_first_and_skips_environments() {
        let root = temp_collection("tree");
        fs::create_dir_all(root.join("empty")).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        save_request(&root.join("users/b.apix"), &Request::new(Method::Get, "x")).unwrap();
        save_request(&root.join("users/a.apix"), &Request::new(Method::Get, "x")).unwrap();

        let tree = Collection::open(&root).unwrap().tree();
        let shape: Vec<String> = tree
            .iter()
            .map(|n| match n {
                TreeNode::Folder { name, children, .. } => format!("{name}/{}", children.len()),
                TreeNode::Request { path } => stem_of(path),
            })
            .collect();
        assert_eq!(shape, ["empty/0", "users/2", "health"]);
        let TreeNode::Folder { children, .. } = &tree[1] else {
            panic!("users is a folder")
        };
        assert_eq!(
            children[0],
            TreeNode::Request {
                path: root.join("users/a.apix")
            }
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn creates_renames_and_duplicates_with_scripts() {
        let root = temp_collection("ops");
        let mut request = Request::new(Method::Post, "https://x/login");
        request.name = "Log in".into();
        let path = create_request(&root, &request).unwrap();
        assert_eq!(path, root.join("log-in.apix"));
        assert_eq!(
            create_request(&root, &request).unwrap(),
            root.join("log-in-2.apix")
        );
        fs::write(root.join("log-in.test.js"), "pm.test()").unwrap();

        let renamed = rename_request(&path, "Sign in").unwrap();
        assert_eq!(renamed, root.join("sign-in.apix"));
        assert!(!path.exists());
        assert_eq!(load_request(&renamed).unwrap().name, "Sign in");
        assert!(
            root.join("sign-in.test.js").is_file(),
            "scripts follow the request"
        );

        let copy = duplicate_request(&renamed).unwrap();
        assert_eq!(copy, root.join("sign-in-copy.apix"));
        assert_eq!(load_request(&copy).unwrap().name, "Sign in copy");
        assert!(root.join("sign-in-copy.test.js").is_file());
        assert_eq!(
            request_files(&renamed),
            [renamed.clone(), root.join("sign-in.test.js")]
        );

        // Renaming to the same slug only updates the name.
        assert_eq!(rename_request(&renamed, "sign IN").unwrap(), renamed);
        let folder = create_folder(&root, "Admin Tools").unwrap();
        assert_eq!(folder, root.join("admin-tools"));
        assert_eq!(rename_folder(&folder, "Admin").unwrap(), root.join("admin"));
        assert!(root.join("admin").is_dir() && !folder.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn slugs_are_readable_and_safe() {
        assert_eq!(slug("Create User (v2)", "x"), "create-user-v2");
        assert_eq!(slug("  ** ", "request"), "request");
        assert_eq!(slug("İstanbul Şube/Çıkış", "x"), "istanbul-şube-çıkış");
        assert_eq!(slug("CON", "x"), "con-1");
        assert_eq!(slug("com1", "x"), "com1-1");
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

    #[test]
    fn edits_environment_parts_and_keeps_secrets_out_of_git() {
        let root = temp_collection("parts");
        let parts = EnvironmentParts::read(&root, "dev").unwrap();
        let mut rows = parts.rows();
        assert_eq!(
            rows,
            [
                EnvVar {
                    key: "host".into(),
                    value: "localhost".into(),
                    secret: false
                },
                EnvVar {
                    key: "token".into(),
                    value: "secret".into(),
                    secret: true
                },
            ]
        );

        // Make `host` secret, add a key, drop an empty row, rename the token.
        rows[0].secret = true;
        rows[1].value = "rotated".into();
        rows.push(EnvVar {
            key: " user ".into(),
            value: "ada".into(),
            secret: false,
        });
        rows.push(EnvVar {
            key: "".into(),
            value: "ignored".into(),
            secret: false,
        });
        let edited = parts.with_rows(&rows);
        assert_eq!(edited.vars["token"], "placeholder", "placeholder survives");
        assert_eq!(edited.vars["host"], "localhost");
        assert_eq!(edited.secrets["host"], "localhost");
        assert_eq!(edited.vars["user"], "ada");
        assert_eq!(edited.vars.len(), 3);
        edited.write(&root, "dev").unwrap();
        assert_eq!(EnvironmentParts::read(&root, "dev").unwrap(), edited);
        let ignore = fs::read_to_string(root.join(".gitignore")).unwrap();
        assert!(ignore.contains("*.local.toml"));
        assert!(!ensure_local_ignored(&root).unwrap(), "added only once");

        // No secrets left: the local file goes away.
        let plain: Vec<EnvVar> = rows
            .into_iter()
            .map(|r| EnvVar { secret: false, ..r })
            .collect();
        parts.with_rows(&plain).write(&root, "dev").unwrap();
        assert_eq!(
            environment_files(&root, "dev"),
            [root.join("environments/dev.toml")]
        );

        // Create, rename (with secrets), and refuse duplicates.
        assert_eq!(
            create_environment(&root, "Staging EU").unwrap(),
            "staging-eu"
        );
        assert!(create_environment(&root, "staging eu").is_err());
        save_environment(
            &root,
            "staging-eu",
            &VarMap::from([("k".into(), "v".into())]),
            true,
        )
        .unwrap();
        assert_eq!(
            rename_environment(&root, "staging-eu", "Stage").unwrap(),
            "stage"
        );
        assert_eq!(environment_names_at(&root), ["dev", "stage"]);
        assert_eq!(environment_files(&root, "stage").len(), 2);
        assert!(rename_environment(&root, "stage", "dev").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn gitignore_is_appended_not_replaced() {
        let root = temp_collection("ignore");
        fs::write(root.join(".gitignore"), "target/").unwrap();
        assert!(ensure_local_ignored(&root).unwrap());
        assert_eq!(
            fs::read_to_string(root.join(".gitignore")).unwrap(),
            "target/\n# Apix secrets\n*.local.toml\n"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
