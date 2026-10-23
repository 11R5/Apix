//! What one or many imported files add up to: collections and environments,
//! ready to be written as one collection or as a project. Every importer
//! (Postman, Insomnia, OpenAPI, HAR) fills a [`Found`].

use std::path::{Path, PathBuf};

use apix_core::store;
use apix_core::{Project, project};

use crate::postman::EnvironmentPlan;
use crate::{ImportError, Plan};

/// What a file, folder or zip holds.
#[derive(Debug, Default)]
pub struct Found {
    /// `(where it came from, the converted collection)`.
    pub collections: Vec<(String, Plan)>,
    pub environments: Vec<EnvironmentPlan>,
    /// Postman globals: project variables.
    pub globals: Vec<EnvironmentPlan>,
    /// Files that look like exports but could not be converted: `(file, why)`.
    pub errors: Vec<(String, String)>,
}

impl Found {
    pub fn is_empty(&self) -> bool {
        self.collections.is_empty() && self.environments.is_empty() && self.globals.is_empty()
    }

    pub fn request_count(&self) -> usize {
        self.collections
            .iter()
            .map(|(_, p)| p.request_count())
            .sum()
    }

    pub fn environment_count(&self) -> usize {
        self.environments.len() + self.globals.len()
    }

    /// Adds whatever `text` (from `source`) holds. Files that are no known
    /// export format (archive.json, package.json…) are skipped silently.
    pub fn add(&mut self, source: String, text: &str) {
        let stem = Path::new(&source)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        match crate::convert_named(text, &stem) {
            Ok(Some(found)) => self.extend(found, &source),
            Ok(None) => {}
            Err(e) => self.errors.push((source, e.to_string())),
        }
    }

    fn extend(&mut self, other: Found, source: &str) {
        self.collections.extend(
            other
                .collections
                .into_iter()
                .map(|(_, plan)| (source.to_owned(), plan)),
        );
        self.environments.extend(other.environments);
        self.globals.extend(other.globals);
        self.errors.extend(other.errors);
    }

    /// Writes one collection (with its environments inside it) when that is
    /// all there is, else a project named `name`. Used for single files.
    pub fn write(&self, parent: &Path, name: &str) -> Result<Written, ImportError> {
        match &self.collections[..] {
            [(_, plan)] if self.globals.is_empty() => {
                let root =
                    store::unique_path(parent, &store::slug(&plan.meta.name, "collection"), "");
                plan.write(&root)?;
                let mut notes = Vec::new();
                notes_for(plan, &mut notes);
                let environments = write_environments(&self.environments, &root, &mut notes)?;
                for (source, error) in &self.errors {
                    notes.push(format!("{source}: {error}"));
                }
                Ok(Written {
                    root,
                    collections: 1,
                    requests: plan.request_count(),
                    environments,
                    global_vars: 0,
                    notes,
                })
            }
            _ => write_project(self, parent, name),
        }
    }
}

/// What a write made.
#[derive(Debug)]
pub struct Written {
    pub root: PathBuf,
    pub collections: usize,
    pub requests: usize,
    pub environments: usize,
    pub global_vars: usize,
    /// One line per warning, file error or thing worth knowing.
    pub notes: Vec<String>,
}

fn notes_for(plan: &Plan, notes: &mut Vec<String>) {
    for w in &plan.report.warnings {
        let at = if w.item.is_empty() {
            plan.meta.name.clone()
        } else {
            format!("{} / {}", plan.meta.name, w.item)
        };
        notes.push(format!("{at}: {}", w.message));
    }
    if plan.report.scripts > 0 {
        notes.push(format!(
            "{}: {} scripts kept as .js files.",
            plan.meta.name, plan.report.scripts
        ));
    }
}

/// Writes environments into `root`; a name already taken gets `-2`, `-3`…
fn write_environments(
    envs: &[EnvironmentPlan],
    root: &Path,
    notes: &mut Vec<String>,
) -> Result<usize, ImportError> {
    for env in envs {
        let mut env = env.clone();
        let taken = store::environment_names_at(root);
        let base = env.name.clone();
        let mut n = 2;
        while taken.contains(&env.name) {
            env.name = format!("{base}-{n}");
            n += 1;
        }
        if env.name != base {
            notes.push(format!(
                "Environment \"{}\" was saved as {} (the name was taken).",
                env.title, env.name
            ));
        }
        env.write(root)?;
    }
    Ok(envs.len())
}

/// Writes `found` as a new project `parent/<slug of name>/`: a collection
/// per collection, the environments shared, globals as project variables.
pub fn write_project(found: &Found, parent: &Path, name: &str) -> Result<Written, ImportError> {
    let root = project::create_project(parent, name, &[])?;
    let mut notes = Vec::new();
    for (_, plan) in &found.collections {
        let dir = store::unique_path(&root, &store::slug(&plan.meta.name, "collection"), "");
        plan.write(&dir)?;
        notes_for(plan, &mut notes);
    }
    let environments = write_environments(&found.environments, &root, &mut notes)?;

    let mut project = Project::open(&root)?;
    let mut global_vars = 0;
    for globals in &found.globals {
        global_vars += globals.vars.len();
        project.meta.vars.extend(globals.vars.clone());
        if !globals.secrets.is_empty() {
            notes.push(format!(
                "Secret globals were not imported (project variables are committed): {}.",
                globals
                    .secrets
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    if global_vars > 0 {
        project.save()?;
    }
    for (source, error) in &found.errors {
        notes.push(format!("{source}: {error}"));
    }
    Ok(Written {
        root,
        collections: found.collections.len(),
        requests: found.request_count(),
        environments,
        global_vars,
        notes,
    })
}
