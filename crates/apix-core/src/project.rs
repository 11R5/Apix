//! Projects group collections and share environments between them:
//!
//! ```text
//! shop/
//! ├── apix-project.toml   name, collection order, sidebar grouping, pins
//! ├── environments/       test.toml, prod.toml (+ *.local.toml secrets)
//! ├── users/              a collection (apix.toml + .apix files)
//! └── payments/           a collection
//! ```
//!
//! Variables are layered, later wins: project → collection → the project's
//! environment → the collection's environment of the same name.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::store::{
    self, COLLECTION_FILE, Collection, CollectionMeta, StoreError, VarMap, environment_names_at,
    io_error, read_toml, slug, unique_path, write_toml,
};
use crate::vars::Variables;

pub const PROJECT_FILE: &str = "apix-project.toml";

/// How the sidebar lays out a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    /// Project → Collections; one environment picker for all.
    #[default]
    Collection,
    /// Project → Environment → Collections; each environment lists the
    /// collections shown in it.
    Environment,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Variables shared by every collection in the project.
    #[serde(default, skip_serializing_if = "VarMap::is_empty")]
    pub vars: VarMap,
    /// Collection folders in the order the sidebar shows them; others follow by name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
    #[serde(default)]
    pub group_by: GroupBy,
    /// Collections shown only in some environments, by collection folder.
    /// A collection without an entry is shown in every environment.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pins: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub meta: ProjectMeta,
}

impl Project {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root = root.into();
        let meta = read_toml(&root.join(PROJECT_FILE))?;
        Ok(Self { root, meta })
    }

    /// The nearest folder at or above `start` that holds `apix-project.toml`.
    pub fn find_root(start: &Path) -> Option<PathBuf> {
        let start = std::path::absolute(start).unwrap_or_else(|_| start.to_path_buf());
        start
            .ancestors()
            .find(|dir| dir.join(PROJECT_FILE).is_file())
            .map(Path::to_path_buf)
    }

    pub fn save(&self) -> Result<(), StoreError> {
        write_toml(&self.root.join(PROJECT_FILE), &self.meta)
    }

    /// The collections directly inside the project, in sidebar order.
    /// Folders whose `apix.toml` cannot be read are returned as errors.
    pub fn collections(&self) -> (Vec<Collection>, Vec<String>) {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return (Vec::new(), Vec::new());
        };
        let mut dirs: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.join(COLLECTION_FILE).is_file())
            .collect();
        let rank = |dir: &Path| {
            let key = self.key(dir);
            self.meta
                .order
                .iter()
                .position(|o| *o == key)
                .unwrap_or(usize::MAX)
        };
        dirs.sort_by(|a, b| {
            rank(a).cmp(&rank(b)).then_with(|| {
                a.to_string_lossy()
                    .to_lowercase()
                    .cmp(&b.to_string_lossy().to_lowercase())
            })
        });

        let mut collections = Vec::new();
        let mut errors = Vec::new();
        for dir in dirs {
            match Collection::open(&dir) {
                Ok(c) => collections.push(c),
                Err(e) => errors.push(format!("{}: {e}", self.key(&dir))),
            }
        }
        (collections, errors)
    }

    /// How a collection folder is named in `order` and `pins`.
    pub fn key(&self, collection_root: &Path) -> String {
        collection_root
            .strip_prefix(&self.root)
            .unwrap_or(collection_root)
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// Project environments plus those defined by any of `collections`, sorted.
    pub fn environment_names(&self, collections: &[Collection]) -> Vec<String> {
        let mut names = environment_names_at(&self.root);
        for collection in collections {
            names.extend(collection.environment_names());
        }
        names.sort();
        names.dedup();
        names
    }

    /// Whether `collection` is shown when `environment` is active.
    pub fn shows(&self, collection_root: &Path, environment: &str) -> bool {
        self.meta
            .pins
            .get(&self.key(collection_root))
            .is_none_or(|envs| envs.iter().any(|e| e == environment))
    }

    /// Shows or hides a collection in one environment.
    pub fn set_shown(
        &mut self,
        collection_root: &Path,
        environment: &str,
        shown: bool,
        all: &[String],
    ) {
        let key = self.key(collection_root);
        let mut envs = self.meta.pins.remove(&key).unwrap_or_else(|| all.to_vec());
        envs.retain(|e| e != environment);
        if shown {
            envs.push(environment.to_owned());
            envs.sort();
        }
        // Shown everywhere is the default, so it needs no entry.
        let everywhere = all.iter().all(|e| envs.contains(e));
        if !everywhere {
            self.meta.pins.insert(key, envs);
        }
    }
}

/// Creates `parent/<slug of name>/` with `apix-project.toml` and empty
/// environment files for each of `environments`. Returns the new folder.
pub fn create_project(
    parent: &Path,
    name: &str,
    environments: &[&str],
) -> Result<PathBuf, StoreError> {
    let root = unique_path(parent, &slug(name, "project"), "");
    fs::create_dir_all(&root).map_err(io_error(&root))?;
    let project = Project {
        root: root.clone(),
        meta: ProjectMeta {
            name: name.trim().to_owned(),
            ..ProjectMeta::default()
        },
    };
    project.save()?;
    for env in environments {
        store::save_environment(&root, &slug(env, "environment"), &VarMap::new(), false)?;
    }
    Ok(root)
}

/// Creates `parent/<slug of name>/apix.toml`. Returns the collection folder.
pub fn create_collection(parent: &Path, name: &str) -> Result<PathBuf, StoreError> {
    let root = unique_path(parent, &slug(name, "collection"), "");
    fs::create_dir_all(&root).map_err(io_error(&root))?;
    store::save_collection_meta(
        &root,
        &CollectionMeta {
            name: name.trim().to_owned(),
            ..CollectionMeta::default()
        },
    )?;
    Ok(root)
}

/// Every variable layer that applies to a request in `collection` under
/// `environment`, in order. Fails if the environment is defined nowhere.
pub fn variables(
    project: Option<&Project>,
    collection: Option<&Collection>,
    environment: Option<&str>,
) -> Result<Variables, StoreError> {
    let mut vars = Variables::new();
    if let Some(p) = project {
        vars = vars.with_scope(&p.meta.vars, &format!("Project \"{}\"", p.meta.name), false);
    }
    if let Some(c) = collection {
        vars = vars.with_scope(
            &c.meta.vars,
            &format!("Collection \"{}\"", c.meta.name),
            false,
        );
    }
    let Some(name) = environment else {
        return Ok(vars);
    };
    let mut found = false;
    let layers = [
        project.map(|p| (&p.root, format!("Environment \"{name}\""))),
        collection.map(|c| {
            let scope = match project {
                Some(_) => format!("Environment \"{name}\" of \"{}\"", c.meta.name),
                None => format!("Environment \"{name}\""),
            };
            (&c.root, scope)
        }),
    ];
    for (root, scope) in layers.into_iter().flatten() {
        if store::environment_files(root, name).is_empty() {
            continue;
        }
        let parts = store::EnvironmentParts::read(root, name)?;
        vars = vars
            .with_scope(&parts.vars, &scope, false)
            .with_scope(&parts.secrets, &scope, true);
        found = true;
    }
    if found {
        Ok(vars)
    } else {
        Err(StoreError::UnknownEnvironment(name.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{save_environment, save_request};
    use crate::{Method, Request};

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("apix-project-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn vars(pairs: &[(&str, &str)]) -> VarMap {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn creates_projects_and_lists_collections_in_order() {
        let parent = temp("create");
        let root = create_project(&parent, "Shop API", &["Test", "Prod"]).unwrap();
        assert_eq!(root, parent.join("shop-api"));
        let users = create_collection(&root, "Users").unwrap();
        let orders = create_collection(&root, "Orders").unwrap();
        fs::create_dir_all(root.join("not-a-collection")).unwrap();

        let mut project = Project::open(&root).unwrap();
        assert_eq!(project.meta.name, "Shop API");
        let names = |p: &Project| -> Vec<String> {
            p.collections().0.into_iter().map(|c| c.meta.name).collect()
        };
        assert_eq!(names(&project), ["Orders", "Users"]);

        project.meta.order = vec!["users".into()];
        project.save().unwrap();
        let project = Project::open(&root).unwrap();
        assert_eq!(names(&project), ["Users", "Orders"]);
        assert_eq!(
            project.environment_names(&project.collections().0),
            ["prod", "test"]
        );
        assert_eq!(
            Project::find_root(&users.join("x.apix")).as_deref(),
            Some(root.as_path())
        );
        assert!(orders.join(COLLECTION_FILE).is_file());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn layers_project_collection_and_environments() {
        let parent = temp("layers");
        let root = create_project(&parent, "P", &["test"]).unwrap();
        let mut project = Project::open(&root).unwrap();
        project.meta.vars = vars(&[("base", "https://project"), ("team", "core")]);
        project.save().unwrap();
        save_environment(
            &root,
            "test",
            &vars(&[("base", "https://test"), ("token", "t1")]),
            false,
        )
        .unwrap();
        save_environment(&root, "test", &vars(&[("token", "secret")]), true).unwrap();

        let croot = create_collection(&root, "Users").unwrap();
        let mut meta = Collection::open(&croot).unwrap().meta;
        meta.vars = vars(&[("team", "users"), ("path", "/users")]);
        store::save_collection_meta(&croot, &meta).unwrap();
        save_environment(&croot, "test", &vars(&[("path", "/v2/users")]), false).unwrap();
        save_environment(&croot, "only-here", &vars(&[("x", "1")]), false).unwrap();
        save_request(
            &croot.join("a.apix"),
            &Request::new(Method::Get, "{{base}}{{path}}"),
        )
        .unwrap();

        let project = Project::open(&root).unwrap();
        let collection = Collection::open(&croot).unwrap();
        let v = variables(Some(&project), Some(&collection), None).unwrap();
        assert_eq!(
            (v.get("base"), v.get("team"), v.get("path")),
            (Some("https://project"), Some("users"), Some("/users"))
        );

        let v = variables(Some(&project), Some(&collection), Some("test")).unwrap();
        assert_eq!(v.get("base"), Some("https://test"));
        assert_eq!(v.get("token"), Some("secret"), "local secrets win");
        assert_eq!(
            v.get("path"),
            Some("/v2/users"),
            "the collection's environment wins over the project's"
        );
        let token = v.origin("token").unwrap();
        assert!(token.secret);
        assert_eq!(token.scope, "Environment \"test\"");
        assert!(v.origin("path").unwrap().scope.ends_with("of \"Users\""));
        assert!(!v.origin("team").unwrap().secret);

        assert!(variables(Some(&project), Some(&collection), Some("only-here")).is_ok());
        assert!(matches!(
            variables(Some(&project), Some(&collection), Some("nope")),
            Err(StoreError::UnknownEnvironment(_))
        ));
        assert_eq!(
            project.environment_names(&[collection]),
            ["only-here", "test"]
        );
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn pins_hide_collections_in_other_environments() {
        let root = temp("pins");
        let mut project = Project {
            root: root.clone(),
            meta: ProjectMeta::default(),
        };
        let reports = root.join("reports");
        let all = vec!["prod".to_string(), "test".to_string()];
        assert!(
            project.shows(&reports, "prod"),
            "shown everywhere by default"
        );

        project.set_shown(&reports, "prod", false, &all);
        assert!(!project.shows(&reports, "prod"));
        assert!(project.shows(&reports, "test"));
        assert_eq!(project.meta.pins["reports"], ["test"]);

        project.set_shown(&reports, "prod", true, &all);
        assert!(
            project.meta.pins.is_empty(),
            "back to everywhere needs no entry"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
