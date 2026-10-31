//! Importers that turn other tools' exports into Apix collection folders.
//!
//! Every importer works in two steps: `convert_*` builds an in-memory
//! [`Plan`] (pure, easy to test), and [`Plan::write`] puts it on disk.
//! Nothing is dropped silently: whatever Apix cannot represent yet is listed
//! in the [`Report`].

pub mod bundle;
pub mod har;
pub mod insomnia;
pub mod openapi;
pub mod postman;
pub mod postman_dump;
pub mod postman_export;
pub mod wsdl;

pub use bundle::{Found, Written};

/// The export formats Apix reads, told apart by content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Postman(postman::Kind),
    Insomnia,
    OpenApi,
    Har,
    Wsdl,
}

impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::Postman(postman::Kind::Collection) => "Postman collection",
            Format::Postman(postman::Kind::Environment) => "Postman environment",
            Format::Postman(postman::Kind::Globals) => "Postman globals",
            Format::Insomnia => "Insomnia",
            Format::OpenApi => "OpenAPI",
            Format::Har => "HAR",
            Format::Wsdl => "WSDL",
        }
    }
}

/// Which export `text` is, or `None` for anything else. A Postman v1
/// collection is an error with advice, not `None`.
pub fn detect(text: &str) -> Result<Option<Format>, ImportError> {
    if wsdl::is_wsdl(text) {
        return Ok(Some(Format::Wsdl));
    }
    let Some(value) = openapi::parse(text) else {
        return Ok(None);
    };
    if openapi::is_openapi(&value) {
        return Ok(Some(Format::OpenApi));
    }
    if insomnia::is_insomnia(&value) {
        return Ok(Some(Format::Insomnia));
    }
    if har::is_har(&value) {
        return Ok(Some(Format::Har));
    }
    if !value.is_object() {
        return Ok(None);
    }
    match postman::detect(text) {
        Ok(kind) => Ok(Some(Format::Postman(kind))),
        Err(ImportError::Unsupported(message)) if message.contains("v1") => {
            Err(ImportError::Unsupported(message))
        }
        Err(_) => Ok(None),
    }
}

/// Converts any export Apix reads; `None` when `text` is none of them.
/// `name` titles what has no name of its own (a HAR file).
pub fn convert_named(text: &str, name: &str) -> Result<Option<Found>, ImportError> {
    let Some(format) = detect(text)? else {
        return Ok(None);
    };
    let found = match format {
        Format::Postman(postman::Kind::Collection) => Found {
            collections: vec![(String::new(), postman::convert_collection(text)?)],
            ..Found::default()
        },
        Format::Postman(postman::Kind::Environment) => Found {
            environments: vec![postman::convert_environment(text)?],
            ..Found::default()
        },
        Format::Postman(postman::Kind::Globals) => Found {
            globals: vec![postman::convert_environment(text)?],
            ..Found::default()
        },
        Format::Insomnia => insomnia::convert(text)?,
        Format::OpenApi => openapi::convert(text)?,
        Format::Har => har::convert(text, name)?,
        // Without its imports; see [`wsdl::Loader`] to follow them.
        Format::Wsdl => wsdl::convert(text, &wsdl::Loader::default())?,
    };
    Ok(Some(found))
}

/// A name for something imported from this address: its last path segment
/// without an extension, or the host.
pub fn location_name(location: &str) -> String {
    let address = location.split(['?', '#']).next().unwrap_or(location);
    let rest = address.split_once("://").map_or(address, |(_, rest)| rest);
    let name = match rest.trim_end_matches('/').rsplit_once('/') {
        Some((_, last)) => last.rsplit_once('.').map_or(last, |(stem, _)| stem),
        None => {
            let host = rest.rsplit('@').next().unwrap_or(rest);
            host.split(':').next().unwrap_or(host).trim_end_matches('/')
        }
    };
    if name.is_empty() {
        "Import".to_owned()
    } else {
        name.to_owned()
    }
}

/// [`convert_named`] without a name.
pub fn convert(text: &str) -> Result<Option<Found>, ImportError> {
    convert_named(text, "HAR import")
}

use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use apix_core::Request;
use apix_core::store::{self, CollectionMeta, FolderMeta, REQUEST_EXTENSION, StoreError};

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Unsupported(String),
    #[error("{} already exists and is not empty; choose a new folder", .0.display())]
    NotEmpty(PathBuf),
    #[error("{} already exists", .0.display())]
    Exists(PathBuf),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> ImportError + '_ {
    move |source| ImportError::Io {
        path: path.to_owned(),
        source,
    }
}

/// Something the importer could not carry over exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// Where it happened, e.g. `Users / Create user`. Empty for the whole collection.
    pub item: String,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub collection: String,
    pub requests: usize,
    pub folders: usize,
    /// Pre-request and test scripts saved as `.js` files next to their requests.
    pub scripts: usize,
    /// Saved example responses, which Apix does not import yet.
    pub examples_skipped: usize,
    pub variables: usize,
    pub warnings: Vec<Warning>,
}

impl Report {
    fn warn(&mut self, item: &str, message: impl Into<String>) {
        self.warnings.push(Warning {
            item: item.to_owned(),
            message: message.into(),
        });
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Imported \"{}\": {} in {}, {}.",
            self.collection,
            count(self.requests, "request", "requests"),
            count(self.folders, "folder", "folders"),
            count(
                self.variables,
                "collection variable",
                "collection variables"
            ),
        )?;
        if self.scripts > 0 {
            writeln!(
                f,
                "Scripts kept as .js files next to their requests: {}. Apix will run them once scripting lands (M5).",
                self.scripts
            )?;
        }
        if self.examples_skipped > 0 {
            writeln!(
                f,
                "Saved example responses not imported yet: {}.",
                self.examples_skipped
            )?;
        }
        for w in &self.warnings {
            if w.item.is_empty() {
                writeln!(f, "  ! {}", w.message)?;
            } else {
                writeln!(f, "  ! {}: {}", w.item, w.message)?;
            }
        }
        Ok(())
    }
}

/// `1 request`, `3 requests`.
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A file the importer wants to create, relative to the collection root.
// Nearly every planned file is a request, so boxing it would gain nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum PlannedFile {
    Request {
        path: PathBuf,
        request: Request,
    },
    Script {
        path: PathBuf,
        text: String,
    },
    /// `folder.toml` for the folder at `path` (folder-level auth).
    Folder {
        path: PathBuf,
        meta: FolderMeta,
    },
}

/// A converted collection, ready to be written.
#[derive(Debug, Clone)]
pub struct Plan {
    pub meta: CollectionMeta,
    pub files: Vec<PlannedFile>,
    pub report: Report,
}

impl Plan {
    /// Writes the collection into `root`, which must not exist yet or be an
    /// empty folder, so an import can never overwrite anyone's files.
    pub fn write(&self, root: &Path) -> Result<(), ImportError> {
        if root.exists() {
            let mut entries = fs::read_dir(root).map_err(io_err(root))?;
            if entries.next().is_some() {
                return Err(ImportError::NotEmpty(root.to_owned()));
            }
        }
        fs::create_dir_all(root).map_err(io_err(root))?;
        store::save_collection_meta(root, &self.meta)?;
        for file in &self.files {
            let (PlannedFile::Request { path, .. }
            | PlannedFile::Script { path, .. }
            | PlannedFile::Folder { path, .. }) = file;
            let target = root.join(path);
            let dir = match file {
                PlannedFile::Folder { .. } => Some(target.as_path()),
                _ => target.parent(),
            };
            if let Some(dir) = dir {
                fs::create_dir_all(dir).map_err(io_err(dir))?;
            }
            match file {
                PlannedFile::Request { request, .. } => store::save_request(&target, request)?,
                PlannedFile::Script { text, .. } => {
                    fs::write(&target, text).map_err(io_err(&target))?
                }
                PlannedFile::Folder { meta, .. } => store::save_folder_meta(&target, meta)?,
            }
        }
        Ok(())
    }

    pub fn request_count(&self) -> usize {
        self.files
            .iter()
            .filter(|f| matches!(f, PlannedFile::Request { .. }))
            .count()
    }
}

pub use apix_core::store::slug;

/// Makes `name` unique among the names already handed out in one folder.
fn unique(name: String, used: &mut HashSet<String>) -> String {
    if used.insert(name.clone()) {
        return name;
    }
    (2..)
        .map(|n| format!("{name}-{n}"))
        .find(|candidate| used.insert(candidate.clone()))
        .expect("an unused name always exists")
}

fn request_file(stem: &str) -> String {
    format!("{stem}.{REQUEST_EXTENSION}")
}

/// The `order` to save for children written in this order (names, each with
/// whether it is a folder): empty when Apix would show them so anyway
/// (folders first, then requests, each by name), so no file is needed.
fn manual_order(written: Vec<(String, bool)>) -> Vec<String> {
    let mut default = written.clone();
    default.sort_by_key(|(name, folder)| (!*folder, name.to_lowercase()));
    if default == written {
        Vec::new()
    } else {
        written.into_iter().map(|(name, _)| name).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_every_format_apart() {
        let postman = include_str!("../tests/fixtures/shop.postman_collection.json");
        let env = include_str!("../tests/fixtures/staging.postman_environment.json");
        assert_eq!(
            detect(postman).unwrap(),
            Some(Format::Postman(postman::Kind::Collection))
        );
        assert_eq!(
            detect(env).unwrap(),
            Some(Format::Postman(postman::Kind::Environment))
        );
        assert_eq!(
            detect(r#"{"_type":"export","__export_format":4,"resources":[]}"#).unwrap(),
            Some(Format::Insomnia)
        );
        assert_eq!(
            detect("openapi: 3.1.0\ninfo: { title: x }\npaths: {}\n").unwrap(),
            Some(Format::OpenApi)
        );
        assert_eq!(
            detect(r#"{"swagger":"2.0","paths":{}}"#).unwrap(),
            Some(Format::OpenApi)
        );
        assert_eq!(
            detect(r#"{"log":{"entries":[]}}"#).unwrap(),
            Some(Format::Har)
        );
        assert_eq!(detect(r#"{"name":"package"}"#).unwrap(), None);
        assert_eq!(detect("just: yaml").unwrap(), None);
        assert_eq!(detect("not json at all {").unwrap(), None);
        assert!(
            detect(r#"{"id":"x","order":[],"requests":[]}"#).is_err(),
            "Postman v1 gets advice"
        );
    }

    #[test]
    fn a_folder_with_mixed_exports_is_read_whole() {
        let dir = std::env::temp_dir().join(format!("apix-mixed-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("specs")).unwrap();
        fs::write(
            dir.join("specs/api.yaml"),
            "openapi: 3.0.0\ninfo: { title: Spec }\npaths:\n  /a:\n    get: { summary: A }\n",
        )
        .unwrap();
        fs::write(
            dir.join("session.har"),
            r#"{"log":{"entries":[{"request":{"method":"GET","url":"https://x.dev/api/b","headers":[]}}]}}"#,
        )
        .unwrap();
        fs::write(dir.join("docker-compose.yml"), "services: {}\n").unwrap();
        let found = postman_dump::scan(&dir).unwrap();
        assert_eq!(found.collections.len(), 2, "{:?}", found.errors);
        assert_eq!(found.request_count(), 2);
        let out = dir.join("out");
        let written = found.write(&out, "Mixed").unwrap();
        assert!(
            apix_core::Project::open(&written.root).is_ok(),
            "two collections make a project"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn names_of_addresses() {
        for (location, name) in [
            ("http://h:1/soap/calc.asmx?WSDL", "calc"),
            ("https://h/api/spec.yaml", "spec"),
            ("https://h/service/", "service"),
            ("https://ann:pw@api.example.com:8443", "api.example.com"),
            ("https://h/", "h"),
        ] {
            assert_eq!(location_name(location), name);
        }
    }

    #[test]
    fn unique_names_get_a_counter() {
        let mut used = HashSet::new();
        assert_eq!(unique("a".into(), &mut used), "a");
        assert_eq!(unique("a".into(), &mut used), "a-2");
        assert_eq!(unique("a".into(), &mut used), "a-3");
    }
}
