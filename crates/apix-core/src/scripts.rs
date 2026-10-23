//! Pre-request and test scripts, kept as JavaScript files next to what they
//! belong to (the layout Postman imports already use):
//!
//! ```text
//! my-api/
//! ├── _collection.prerequest.js   runs before every request in the collection
//! ├── users/
//! │   ├── _folder.test.js         runs after every request in the folder
//! │   ├── get-user.apix
//! │   └── get-user.test.js        runs after this request
//! ```
//!
//! Scripts run from the outside in: collection, then each folder down to the
//! request's own. Running them is `apix-script`'s job; this module only finds
//! them and keeps what they change.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::vars::Variables;
use crate::vfs;

/// When a script runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Before the request is sent; it can change the request and variables.
    PreRequest,
    /// After the response arrives; it checks it with `pm.test`.
    Test,
}

impl Event {
    /// The part of the file name: `prerequest` or `test`.
    pub fn suffix(self) -> &'static str {
        match self {
            Event::PreRequest => "prerequest",
            Event::Test => "test",
        }
    }
}

/// One script file and its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptFile {
    pub path: PathBuf,
    pub source: String,
}

/// `<stem>.prerequest.js` or `<stem>.test.js` next to a request file.
pub fn request_script(request: &Path, event: Event) -> PathBuf {
    let stem = request
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    request.with_file_name(format!("{stem}.{}.js", event.suffix()))
}

/// `_folder.<event>.js` in a folder.
pub fn folder_script(dir: &Path, event: Event) -> PathBuf {
    dir.join(format!("_folder.{}.js", event.suffix()))
}

/// `_collection.<event>.js` in a collection's root.
pub fn collection_script(root: &Path, event: Event) -> PathBuf {
    root.join(format!("_collection.{}.js", event.suffix()))
}

/// The scripts that run for the request at `request`, outermost first:
/// the collection's, each folder's down to the request's folder, then the
/// request's own. Missing and blank files are left out.
pub fn chain(request: &Path, collection_root: Option<&Path>, event: Event) -> Vec<ScriptFile> {
    let mut paths = Vec::new();
    if let Some(root) = collection_root {
        paths.push(collection_script(root, event));
        let mut folders: Vec<&Path> = request
            .parent()
            .into_iter()
            .flat_map(Path::ancestors)
            .take_while(|dir| dir.starts_with(root) && *dir != root)
            .collect();
        folders.reverse();
        paths.extend(folders.into_iter().map(|dir| folder_script(dir, event)));
    }
    paths.push(request_script(request, event));
    paths
        .into_iter()
        .filter_map(|path| {
            let source = vfs::read_to_string(&path).ok()?;
            (!source.trim().is_empty()).then_some(ScriptFile { path, source })
        })
        .collect()
}

/// The text of the script at `path`, or empty when there is none.
pub fn read_script(path: &Path) -> String {
    vfs::read_to_string(path).unwrap_or_default()
}

/// Saves a script, or removes its file when `source` is blank.
pub fn write_script(path: &Path, source: &str) -> std::io::Result<()> {
    if source.trim().is_empty() {
        if vfs::is_file(path) {
            vfs::remove_file(path)?;
        }
        Ok(())
    } else {
        vfs::write(path, source)
    }
}

/// Variable values as scripts see them before any change, per scope.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeValues {
    /// `pm.globals`: the project's variables.
    pub globals: BTreeMap<String, String>,
    /// `pm.collectionVariables`: the collection's variables.
    pub collection: BTreeMap<String, String>,
    /// `pm.environment`: the active environment, secrets included.
    pub environment: BTreeMap<String, String>,
    /// The active environment's name.
    pub environment_name: Option<String>,
    /// Values set for this run only (`apix run --var`, a data file row),
    /// which win over every scope but `pm.variables.set`.
    pub data: BTreeMap<String, String>,
}

/// What scripts set (`Some`) or unset (`None`) during a session: one
/// `apix run`, or the desktop app until it quits. Nothing here is written to
/// disk; a script that should change a file does not exist.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptVars {
    #[serde(default)]
    pub globals: BTreeMap<String, Option<String>>,
    #[serde(default)]
    pub collection: BTreeMap<String, Option<String>>,
    #[serde(default)]
    pub environment: BTreeMap<String, Option<String>>,
    /// `pm.variables.set`: the narrowest scope.
    #[serde(default)]
    pub local: BTreeMap<String, Option<String>>,
}

/// How the variable card names values set by a script.
pub const SCRIPT_SCOPE: &str = "Script";

impl ScriptVars {
    pub fn is_empty(&self) -> bool {
        self.globals.is_empty()
            && self.collection.is_empty()
            && self.environment.is_empty()
            && self.local.is_empty()
    }

    /// `vars` with these changes on top, broadest scope first.
    pub fn apply(&self, mut vars: Variables) -> Variables {
        for scope in [
            &self.globals,
            &self.collection,
            &self.environment,
            &self.local,
        ] {
            for (key, value) in scope {
                match value {
                    Some(value) => vars.set_with_scope(key, value, SCRIPT_SCOPE),
                    None => vars.unset(key),
                }
            }
        }
        vars
    }

    /// Forgets `pm.variables.set` values, which last for one request (or
    /// one iteration of a run).
    pub fn clear_local(&mut self) {
        self.local.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_scripts_from_the_collection_down() {
        let root = std::env::temp_dir().join(format!("apix-scripts-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let deep = root.join("users").join("admin");
        fs::create_dir_all(&deep).unwrap();
        let request = deep.join("get-user.apix");
        fs::write(&request, "url = \"x\"").unwrap();
        fs::write(root.join("_collection.prerequest.js"), "a()").unwrap();
        fs::write(root.join("users").join("_folder.prerequest.js"), "b()").unwrap();
        fs::write(deep.join("_folder.prerequest.js"), "  \n").unwrap();
        fs::write(deep.join("get-user.prerequest.js"), "c()").unwrap();
        fs::write(deep.join("get-user.test.js"), "t()").unwrap();

        let pre = chain(&request, Some(&root), Event::PreRequest);
        let sources: Vec<&str> = pre.iter().map(|s| s.source.as_str()).collect();
        assert_eq!(sources, ["a()", "b()", "c()"], "blank files are skipped");
        assert_eq!(chain(&request, Some(&root), Event::Test).len(), 1);
        assert_eq!(chain(&request, None, Event::PreRequest).len(), 1);
        assert_eq!(
            request_script(&request, Event::Test),
            deep.join("get-user.test.js")
        );

        write_script(&deep.join("get-user.test.js"), " ").unwrap();
        assert!(
            !deep.join("get-user.test.js").exists(),
            "blank removes the file"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn script_changes_go_on_top_of_the_variables() {
        let base = Variables::new().with_layer(&BTreeMap::from([
            ("host".to_string(), "a".to_string()),
            ("token".to_string(), "old".to_string()),
        ]));
        let mut changes = ScriptVars::default();
        changes
            .environment
            .insert("token".into(), Some("new".into()));
        changes.local.insert("host".into(), None);
        let vars = changes.apply(base);
        assert_eq!(vars.get("token"), Some("new"));
        assert_eq!(vars.origin("token").unwrap().scope, SCRIPT_SCOPE);
        assert_eq!(vars.get("host"), None);
    }
}
