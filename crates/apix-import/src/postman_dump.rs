//! Many exports at once: a folder that holds them, or the zip from Postman's
//! "Export data" (`collection/*.json`, `environment/*.json`, `archive.json`).
//! Files are recognised by their content, not their names — Postman,
//! Insomnia, OpenAPI/Swagger and HAR alike — and become one Apix project.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use crate::{ImportError, io_err};

/// Folders this deep below the one given are searched.
const MAX_DEPTH: usize = 3;
/// JSON files read at most, so opening a big folder by mistake stays quick.
const MAX_FILES: usize = 400;
/// Bigger files are not Postman exports anyone wants to import blind.
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

pub use crate::bundle::{Found, Written, write_project};

/// Reads every export in `path`: a folder (searched a few levels deep,
/// skipping hidden folders) or a `.zip`.
pub fn scan(path: &Path) -> Result<Found, ImportError> {
    let mut found = Found::default();
    if path.is_dir() {
        let mut files = Vec::new();
        json_files(path, 0, &mut files);
        files.sort();
        for file in files {
            if let Ok(json) = std::fs::read_to_string(&file) {
                let source = file
                    .strip_prefix(path)
                    .unwrap_or(&file)
                    .to_string_lossy()
                    .replace('\\', "/");
                found.add(source, &json);
            }
        }
    } else {
        let file = std::fs::File::open(path).map_err(io_err(path))?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| {
            ImportError::Unsupported(format!(
                "{} is not a readable zip file: {e}",
                path.display()
            ))
        })?;
        let mut names: Vec<String> = archive
            .file_names()
            .filter(|n| is_candidate(n))
            .map(str::to_owned)
            .collect();
        names.sort();
        for name in names.into_iter().take(MAX_FILES) {
            let Ok(mut entry) = archive.by_name(&name) else {
                continue;
            };
            if entry.size() > MAX_FILE_BYTES {
                continue;
            }
            let mut json = String::new();
            if entry.read_to_string(&mut json).is_ok() {
                found.add(name, &json);
            }
        }
    }
    Ok(found)
}

fn json_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= MAX_FILES {
            return;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.starts_with('.') || name == "node_modules" || name == "target" {
            continue;
        }
        if path.is_dir() {
            if depth < MAX_DEPTH {
                json_files(&path, depth + 1, out);
            }
        } else if is_candidate(&name) && entry.metadata().is_ok_and(|m| m.len() <= MAX_FILE_BYTES) {
            out.push(path);
        }
    }
}

/// File names worth reading: JSON exports, OpenAPI in YAML, HAR.
fn is_candidate(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [".json", ".yaml", ".yml", ".har"]
        .iter()
        .any(|ext| name.ends_with(ext))
}

/// The project name for a folder or zip: its file name, else "Postman".
pub fn project_name(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Postman".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use apix_core::Project;
    use std::io::Write as _;

    const SHOP: &str = include_str!("../tests/fixtures/shop.postman_collection.json");
    const STAGING: &str = include_str!("../tests/fixtures/staging.postman_environment.json");
    const GLOBALS: &str = r#"{
        "name": "Globals",
        "_postman_variable_scope": "globals",
        "values": [
            { "key": "region", "value": "eu", "enabled": true },
            { "key": "master", "value": "k", "type": "secret", "enabled": true }
        ]
    }"#;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("apix-dump-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn check(found: &Found) {
        assert_eq!(found.collections.len(), 1, "{:?}", found.errors);
        assert_eq!(found.environments.len(), 2, "the same env twice");
        assert_eq!(found.globals.len(), 1);
        assert!(found.errors.is_empty(), "{:?}", found.errors);
    }

    #[test]
    fn a_folder_of_exports_becomes_a_project() {
        let dir = temp("folder");
        std::fs::create_dir_all(dir.join("collection")).unwrap();
        std::fs::create_dir_all(dir.join("environment")).unwrap();
        std::fs::write(dir.join("collection/a1b2.json"), SHOP).unwrap();
        std::fs::write(dir.join("environment/c3.json"), STAGING).unwrap();
        std::fs::write(dir.join("environment/c4.json"), STAGING).unwrap();
        std::fs::write(dir.join("globals.json"), GLOBALS).unwrap();
        std::fs::write(dir.join("archive.json"), r#"{"collection":{"a1b2":true}}"#).unwrap();
        std::fs::write(dir.join("broken.json"), "{ not json").unwrap();

        let found = scan(&dir).unwrap();
        check(&found);
        assert_eq!(found.collections[0].0, "collection/a1b2.json");

        let out = temp("folder-out");
        let written = write_project(&found, &out, "Postman backup").unwrap();
        assert_eq!(written.root, out.join("postman-backup"));
        assert_eq!(written.collections, 1);
        assert!(written.requests > 5);
        assert_eq!(written.environments, 2);

        let project = Project::open(&written.root).unwrap();
        let (collections, errors) = project.collections();
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(collections[0].meta.name, "Shop API");
        let envs = project.environment_names(&collections);
        assert_eq!(envs, ["staging-eu", "staging-eu-2"]);
        assert_eq!(
            project.meta.vars.get("region").map(String::as_str),
            Some("eu")
        );
        assert!(
            !project.meta.vars.contains_key("master"),
            "secret globals stay out"
        );
        assert!(
            written.notes.iter().any(|n| n.contains("master")),
            "{:?}",
            written.notes
        );
        assert!(
            written.notes.iter().any(|n| n.contains("staging-eu-2")),
            "{:?}",
            written.notes
        );
        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&out).ok();
    }

    #[test]
    fn a_zip_from_export_data_is_read_too() {
        let dir = temp("zip");
        let zip_path = dir.join("backup.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, text) in [
            ("collection/a1b2.json", SHOP),
            ("environment/c3.json", STAGING),
            ("environment/c4.json", STAGING),
            ("globals/g.json", GLOBALS),
            ("archive.json", "{}"),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap();

        check(&scan(&zip_path).unwrap());
        assert_eq!(project_name(&zip_path), "backup");
        let not_zip = dir.join("x.zip");
        std::fs::write(&not_zip, "nope").unwrap();
        assert!(matches!(scan(&not_zip), Err(ImportError::Unsupported(_))));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_unrelated_folder_finds_nothing() {
        let dir = temp("empty");
        std::fs::write(dir.join("package.json"), r#"{"name":"x"}"#).unwrap();
        assert!(scan(&dir).unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
