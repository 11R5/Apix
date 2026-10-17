//! Importers that turn other tools' exports into Apix collection folders.
//!
//! Every importer works in two steps: `convert_*` builds an in-memory
//! [`Plan`] (pure, easy to test), and [`Plan::write`] puts it on disk.
//! Nothing is dropped silently: whatever Apix cannot represent yet is listed
//! in the [`Report`].

pub mod postman;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_names_get_a_counter() {
        let mut used = HashSet::new();
        assert_eq!(unique("a".into(), &mut used), "a");
        assert_eq!(unique("a".into(), &mut used), "a-2");
        assert_eq!(unique("a".into(), &mut used), "a-3");
    }
}
