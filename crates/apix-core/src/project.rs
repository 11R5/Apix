//! Projects group collections and share environments between them:
//!
//! ```text
//! shop/
//! ├── apix-project.toml   name, collection order and colours, sidebar grouping, pins
//! ├── environments/       test.toml, prod.toml (+ *.local.toml secrets)
//! ├── users/              a collection (apix.toml + .apix files)
//! └── payments/           a collection
//! ```
//!
//! Variables are layered, later wins: project → collection → the project's
//! environment → the collection's environment of the same name.

use crate::vfs;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{Auth, Request, RequestSettings};
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
    /// A colour name per collection folder (`red`, `blue`, …), to tell
    /// collections apart in the sidebar and in tabs.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub colors: BTreeMap<String, String>,
    /// Auth for every request in the project that does not set its own.
    #[serde(default, skip_serializing_if = "Auth::is_inherit")]
    pub auth: Auth,
    /// Transport settings for requests that do not set their own.
    #[serde(default, skip_serializing_if = "RequestSettings::is_inherit")]
    pub settings: RequestSettings,
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
            .find(|dir| vfs::is_file(&dir.join(PROJECT_FILE)))
            .map(Path::to_path_buf)
    }

    pub fn save(&self) -> Result<(), StoreError> {
        write_toml(&self.root.join(PROJECT_FILE), &self.meta)
    }

    /// Encrypted collections directly inside the project that are still locked.
    pub fn locked_collections(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = vfs::read_dir(&self.root)
            .unwrap_or_default()
            .into_iter()
            .filter(|dir| vfs::is_encrypted(dir) && !vfs::is_unlocked(dir))
            .collect();
        out.sort();
        out
    }

    /// The collections directly inside the project, in sidebar order.
    /// Folders whose `apix.toml` cannot be read are returned as errors.
    pub fn collections(&self) -> (Vec<Collection>, Vec<String>) {
        let Ok(entries) = vfs::read_dir(&self.root) else {
            return (Vec::new(), Vec::new());
        };
        let mut dirs: Vec<PathBuf> = entries
            .into_iter()
            .filter(|p| vfs::is_file(&p.join(COLLECTION_FILE)))
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

    /// Puts the collection at `item` just before `anchor`, or after it, and
    /// saves that order.
    pub fn place_collection(
        &mut self,
        item: &Path,
        anchor: &Path,
        after: bool,
    ) -> Result<(), StoreError> {
        let (item, anchor) = (self.key(item), self.key(anchor));
        if item == anchor {
            return Ok(());
        }
        let mut keys: Vec<String> = self
            .collections()
            .0
            .iter()
            .map(|c| self.key(&c.root))
            .filter(|k| *k != item)
            .collect();
        let Some(at) = keys.iter().position(|k| *k == anchor) else {
            return Ok(());
        };
        keys.insert(if after { at + 1 } else { at }, item);
        self.meta.order = keys;
        self.save()
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

    /// The colour name given to a collection, if any.
    pub fn color_of(&self, collection_root: &Path) -> Option<&str> {
        self.meta
            .colors
            .get(&self.key(collection_root))
            .map(String::as_str)
    }

    /// Gives a collection a colour name, or takes it away.
    pub fn set_color(&mut self, collection_root: &Path, color: Option<&str>) {
        let key = self.key(collection_root);
        match color {
            Some(color) => self.meta.colors.insert(key, color.to_owned()),
            None => self.meta.colors.remove(&key),
        };
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
    vfs::create_dir_all(&root).map_err(io_error(&root))?;
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
    vfs::create_dir_all(&root).map_err(io_error(&root))?;
    store::save_collection_meta(
        &root,
        &CollectionMeta {
            name: name.trim().to_owned(),
            ..CollectionMeta::default()
        },
    )?;
    Ok(root)
}

/// Moves the collection folder at `collection` into the project at `project`
/// (keeping its folder name unless that is taken there). Returns where it went.
/// Works across drives by copying and then removing the original.
pub fn move_collection_into(project: &Path, collection: &Path) -> Result<PathBuf, StoreError> {
    let refuse = |why: &str| StoreError::Io {
        path: collection.to_owned(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidInput, why.to_owned()),
    };
    if !vfs::is_file(&project.join(PROJECT_FILE)) {
        return Err(refuse("not a project folder"));
    }
    if !collection.join(COLLECTION_FILE).is_file() && !vfs::is_encrypted(collection) {
        return Err(refuse("not a collection folder"));
    }
    if collection.parent() == Some(project) {
        return Ok(collection.to_owned());
    }
    if project.starts_with(collection) || collection.starts_with(project) {
        return Err(refuse(
            "the collection is already inside the project, or holds it",
        ));
    }
    let name = collection
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "collection".into());
    let target = unique_path(project, &name, "");
    if std::fs::rename(collection, &target).is_err() {
        // Another drive: copy, and remove the original only once the copy is whole.
        if let Err(e) = copy_tree(collection, &target) {
            let _ = std::fs::remove_dir_all(&target);
            return Err(StoreError::Io {
                path: collection.to_owned(),
                source: e,
            });
        }
        std::fs::remove_dir_all(collection).map_err(io_error(collection))?;
    }
    Ok(target)
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// What a request in `dir` gets when its auth is [`Auth::Inherit`]: the auth
/// of the nearest folder between `dir` and the collection root that sets one
/// (in its `folder.toml`), else the collection's, else the project's. Returns
/// `Auth::None` and no source when nothing sets one. The source reads like
/// `folder "users"` or `collection "Shop API"`.
pub fn inherited_auth(
    dir: &Path,
    collection: Option<&Collection>,
    project: Option<&Project>,
) -> Result<(Auth, Option<String>), StoreError> {
    if let Some(c) = collection {
        let folders = dir
            .ancestors()
            .take_while(|d| d.starts_with(&c.root) && *d != c.root.as_path());
        for folder in folders {
            let meta = store::load_folder_meta(folder)?;
            if !meta.auth.is_inherit() {
                let name = folder.file_name().unwrap_or_default().to_string_lossy();
                return Ok((meta.auth, Some(format!("folder \"{name}\""))));
            }
        }
        if !c.meta.auth.is_inherit() {
            return Ok((
                c.meta.auth.clone(),
                Some(format!("collection \"{}\"", c.meta.name)),
            ));
        }
    }
    if let Some(p) = project.filter(|p| !p.meta.auth.is_inherit()) {
        return Ok((
            p.meta.auth.clone(),
            Some(format!("project \"{}\"", p.meta.name)),
        ));
    }
    Ok((Auth::None, None))
}

/// Replaces [`Auth::Inherit`] in `request` (saved at `path`) with what it inherits.
pub fn apply_inherited_auth(
    request: &mut Request,
    path: &Path,
    collection: Option<&Collection>,
    project: Option<&Project>,
) -> Result<(), StoreError> {
    if request.auth.is_inherit() {
        let dir = path.parent().unwrap_or(Path::new("."));
        request.auth = inherited_auth(dir, collection, project)?.0;
    }
    Ok(())
}

/// What a request in `dir` inherits for the `None` fields of its
/// `[settings]`: folder settings between `dir` and the collection root
/// (nearest folder first), then the collection's, then the project's.
/// Relative certificate paths are made absolute against the folder of
/// the file that defined them.
pub fn inherited_settings(
    dir: &Path,
    collection: Option<&Collection>,
    project: Option<&Project>,
) -> Result<RequestSettings, StoreError> {
    let mut merged = RequestSettings::default();
    if let Some(c) = collection {
        let folders = dir
            .ancestors()
            .take_while(|d| d.starts_with(&c.root) && *d != c.root.as_path());
        for folder in folders {
            let meta = store::load_folder_meta(folder)?;
            merged = merged.or_inherit(&meta.settings.with_files_relative_to(folder));
        }
        merged = merged.or_inherit(&c.meta.settings.clone().with_files_relative_to(&c.root));
    }
    if let Some(p) = project {
        merged = merged.or_inherit(&p.meta.settings.clone().with_files_relative_to(&p.root));
    }
    Ok(merged)
}

/// Fills the `None` fields of `request.settings` (saved at `path`) with
/// what they inherit.
pub fn apply_inherited_settings(
    request: &mut Request,
    path: &Path,
    collection: Option<&Collection>,
    project: Option<&Project>,
) -> Result<(), StoreError> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let inherited = inherited_settings(dir, collection, project)?;
    request.settings = request.settings.clone().or_inherit(&inherited);
    Ok(())
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
    use std::fs;

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
        let mut project = Project::open(&root).unwrap();
        assert_eq!(names(&project), ["Users", "Orders"]);
        project.set_color(&users, Some("blue"));
        assert_eq!(project.color_of(&users), Some("blue"));
        assert_eq!(project.meta.colors["users"], "blue");
        project.set_color(&users, None);
        assert!(project.meta.colors.is_empty());
        // Dragging Users after Orders.
        project.place_collection(&users, &orders, true).unwrap();
        let project = Project::open(&root).unwrap();
        assert_eq!(names(&project), ["Orders", "Users"]);
        assert_eq!(project.meta.order, ["orders", "users"]);
        assert_eq!(
            project.environment_names(&project.collections().0),
            ["prod", "test"]
        );
        assert_eq!(
            Project::find_root(&users.join("x.apix")).as_deref(),
            Some(root.as_path())
        );
        assert!(orders.join(COLLECTION_FILE).is_file());

        // A collection from elsewhere moves in; a second one with the same
        // folder name gets its own.
        let outside = temp("outside");
        let stray = create_collection(&outside, "Users").unwrap();
        fs::write(stray.join("ping.apix"), "url = \"x\"\n").unwrap();
        let moved = move_collection_into(&root, &stray).unwrap();
        assert_eq!(moved, root.join("users-2"));
        assert!(moved.join("ping.apix").is_file() && !stray.exists());
        assert_eq!(Project::open(&root).unwrap().collections().0.len(), 3);
        assert!(move_collection_into(&root, &users).is_ok(), "already there");
        assert!(
            move_collection_into(&outside, &users).is_err(),
            "not a project"
        );
        fs::remove_dir_all(outside).unwrap();
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn auth_comes_from_the_nearest_parent_that_sets_one() {
        let parent = temp("auth");
        let root = create_project(&parent, "Shop", &[]).unwrap();
        let croot = create_collection(&root, "Users").unwrap();
        let deep = croot.join("admin").join("audit");
        fs::create_dir_all(&deep).unwrap();
        let bearer = |t: &str| Auth::Bearer { token: t.into() };
        let load = || {
            (
                Project::open(&root).unwrap(),
                Collection::open(&croot).unwrap(),
            )
        };

        let (p, c) = load();
        assert_eq!(
            inherited_auth(&deep, Some(&c), Some(&p)).unwrap(),
            (Auth::None, None)
        );

        let mut p = p;
        p.meta.auth = bearer("project");
        p.save().unwrap();
        let (p, c) = load();
        let (auth, from) = inherited_auth(&deep, Some(&c), Some(&p)).unwrap();
        assert_eq!(
            (auth, from.as_deref()),
            (bearer("project"), Some("project \"Shop\""))
        );

        let mut meta = c.meta.clone();
        meta.auth = bearer("collection");
        store::save_collection_meta(&croot, &meta).unwrap();
        let admin = store::FolderMeta {
            auth: bearer("admin"),
            ..Default::default()
        };
        store::save_folder_meta(&croot.join("admin"), &admin).unwrap();
        let (p, c) = load();
        let (auth, from) = inherited_auth(&deep, Some(&c), Some(&p)).unwrap();
        assert_eq!(
            (auth, from.as_deref()),
            (bearer("admin"), Some("folder \"admin\""))
        );
        let (auth, _) = inherited_auth(&croot, Some(&c), Some(&p)).unwrap();
        assert_eq!(
            auth,
            bearer("collection"),
            "the collection root is not a folder"
        );

        // A request's own auth, including an explicit "none", is kept.
        let mut request = Request::new(Method::Get, "x");
        apply_inherited_auth(&mut request, &deep.join("r.apix"), Some(&c), Some(&p)).unwrap();
        assert_eq!(request.auth, bearer("admin"));
        request.auth = Auth::None;
        apply_inherited_auth(&mut request, &deep.join("r.apix"), Some(&c), Some(&p)).unwrap();
        assert_eq!(request.auth, Auth::None);

        // Back to the default: folder.toml goes away.
        store::save_folder_meta(&croot.join("admin"), &store::FolderMeta::default()).unwrap();
        assert!(!croot.join("admin").join(store::FOLDER_FILE).exists());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn settings_merge_per_field_from_the_nearest_parent() {
        let parent = temp("settings");
        let root = create_project(&parent, "Shop", &[]).unwrap();
        let croot = create_collection(&root, "Users").unwrap();
        let deep = croot.join("admin");
        fs::create_dir_all(&deep).unwrap();

        let mut p = Project::open(&root).unwrap();
        p.meta.settings.timeout_ms = Some(30_000);
        p.meta.settings.tls_verify = Some(false);
        p.meta.settings.client_cert = Some("certs/project.pem".into());
        p.save().unwrap();

        let c = Collection::open(&croot).unwrap();
        let mut cmeta = c.meta.clone();
        cmeta.settings.tls_verify = Some(true);
        store::save_collection_meta(&croot, &cmeta).unwrap();

        let folder = store::FolderMeta {
            settings: RequestSettings {
                proxy: Some("http://127.0.0.1:8888".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        store::save_folder_meta(&deep, &folder).unwrap();

        let p = Project::open(&root).unwrap();
        let c = Collection::open(&croot).unwrap();
        let merged = inherited_settings(&deep, Some(&c), Some(&p)).unwrap();
        assert_eq!(merged.timeout_ms, Some(30_000), "from the project");
        assert_eq!(merged.tls_verify, Some(true), "the collection wins");
        assert_eq!(merged.proxy.as_deref(), Some("http://127.0.0.1:8888"));
        // A relative cert path is absolute against its defining folder.
        assert_eq!(
            merged.client_cert.as_deref(),
            Some(root.join("certs/project.pem").to_string_lossy().as_ref())
        );

        // The request's own value still wins over everything.
        let mut request = Request::new(Method::Get, "x");
        request.settings.timeout_ms = Some(5);
        apply_inherited_settings(&mut request, &deep.join("r.apix"), Some(&c), Some(&p)).unwrap();
        assert_eq!(request.settings.timeout_ms, Some(5));
        assert_eq!(request.settings.tls_verify, Some(true));
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
