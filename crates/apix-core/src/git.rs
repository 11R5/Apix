//! What git knows about a collection: the branch, the files changed since a
//! commit, and a file as it was at a commit. It runs the `git` program that
//! is already installed rather than linking a git library, so Apix stays
//! small and behaves exactly like the user's own `git`.
//!
//! Every path here is relative to the folder the [`Repo`] was opened at (a
//! collection or a project), never to the top of the work tree, so a
//! collection inside a bigger repository only sees its own files.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::scripts::Event;
use crate::store::{self, COLLECTION_FILE, ENVIRONMENTS_DIR, FOLDER_FILE};

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("git could not be started (is it installed and on PATH?): {0}")]
    Missing(std::io::Error),
    #[error("git {command}: {message}")]
    Failed { command: String, message: String },
}

/// A folder inside a git work tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    dir: PathBuf,
}

/// How a file differs from the commit it is compared with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

impl ChangeKind {
    /// The letter `git status --short` uses.
    pub fn letter(self) -> char {
        match self {
            ChangeKind::Added => 'A',
            ChangeKind::Modified => 'M',
            ChangeKind::Deleted => 'D',
        }
    }
}

/// One commit in [`Repo::log`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// The short hash.
    pub hash: String,
    pub subject: String,
    /// When it was committed, in seconds since 1970.
    pub time: u64,
}

/// One changed file, relative to the [`Repo`]'s folder.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct Change {
    pub path: PathBuf,
    pub kind: ChangeKind,
}

/// The files changed since a commit, and how they were found.
#[derive(Debug, Clone, Default)]
pub struct Since {
    pub changes: Vec<Change>,
    /// Set when the branch point with the commit could not be found (a
    /// shallow clone), so the files were compared with the commit itself.
    pub warning: Option<String>,
}

impl Repo {
    /// The repository around `dir`, or `None` when `dir` is not in a git
    /// work tree (or git is not installed).
    pub fn at(dir: &Path) -> Option<Self> {
        let repo = Self {
            dir: dir.to_path_buf(),
        };
        let inside = repo.git(&["rev-parse", "--is-inside-work-tree"]).ok()?;
        (inside.trim() == "true").then_some(repo)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The checked-out branch, or the short commit when HEAD is detached.
    /// `None` in a repository without commits.
    pub fn branch(&self) -> Option<String> {
        // Works before the first commit too; fails when HEAD is detached.
        if let Ok(name) = self.git(&["symbolic-ref", "--short", "-q", "HEAD"])
            && !name.trim().is_empty()
        {
            return Some(name.trim().to_owned());
        }
        let short = self.git(&["rev-parse", "--short", "HEAD"]).ok()?;
        Some(short.trim().to_owned())
    }

    /// Whether `rev` names a commit here.
    pub fn has_commit(&self, rev: &str) -> bool {
        self.git(&[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ])
        .is_ok()
    }

    /// Files that differ between `since` and the working tree: committed
    /// since, changed and not committed, and new files git does not ignore.
    /// Like a pull request, it compares with the point where the current
    /// branch left `since`, so changes made on `since` meanwhile do not count.
    pub fn changes_since(&self, since: &str) -> Result<Since, GitError> {
        self.ensure_commit(since)?;
        let mut warning = None;
        let mut changes = match self.diff(&["--merge-base", since]) {
            Ok(changes) => changes,
            // A shallow clone has no history to find the branch point in.
            Err(_) if since != "HEAD" => {
                warning = Some(format!(
                    "no common ancestor with {since} (a shallow clone?), so this compares with {since} itself; fetch with full history (fetch-depth: 0) to compare with where the branch started"
                ));
                self.diff(&[since])?
            }
            Err(e) => return Err(e),
        };
        changes.extend(self.untracked()?);
        changes.sort();
        changes.dedup();
        Ok(Since { changes, warning })
    }

    /// Files that differ between two commits; with `to` = `None`, between
    /// `from` and the working tree (new untracked files included).
    pub fn changes_between(&self, from: &str, to: Option<&str>) -> Result<Vec<Change>, GitError> {
        self.ensure_commit(from)?;
        let mut changes = match to {
            Some(to) => {
                self.ensure_commit(to)?;
                self.diff(&[from, to])?
            }
            None => {
                let mut changes = self.diff(&[from])?;
                changes.extend(self.untracked()?);
                changes
            }
        };
        changes.sort();
        changes.dedup();
        Ok(changes)
    }

    /// Changes not committed yet (staged or not), with new files: what
    /// `git status` lists.
    pub fn working_changes(&self) -> Result<Vec<Change>, GitError> {
        if !self.has_commit("HEAD") {
            // Nothing committed yet: every file is new.
            return self.untracked();
        }
        self.changes_between("HEAD", None)
    }

    /// The bytes of `path` (relative to the repo's folder) at commit `rev`,
    /// or `None` when it did not exist there.
    pub fn show(&self, rev: &str, path: &Path) -> Result<Option<Vec<u8>>, GitError> {
        let spec = format!("{rev}:./{}", slashes(path));
        match self.run(&["cat-file", "blob", &spec]) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(GitError::Failed { .. }) if self.has_commit(rev) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The local branches, by name.
    pub fn branches(&self) -> Result<Vec<String>, GitError> {
        let out = self.git(&["for-each-ref", "--format=%(refname:short)", "refs/heads"])?;
        Ok(out
            .lines()
            .map(str::to_owned)
            .filter(|l| !l.is_empty())
            .collect())
    }

    /// Checks out another branch. Git refuses when changes not committed
    /// would be lost, and says so.
    pub fn switch(&self, branch: &str) -> Result<(), GitError> {
        self.git(&["switch", branch]).map(|_| ())
    }

    /// Makes a branch at the current commit and checks it out.
    pub fn create_branch(&self, name: &str) -> Result<(), GitError> {
        self.git(&["switch", "-c", name]).map(|_| ())
    }

    /// Commits every change inside the folder (new and deleted files too),
    /// leaving changes elsewhere in the repository alone. Returns the new
    /// commit's short hash.
    pub fn commit_folder(&self, message: &str) -> Result<String, GitError> {
        if message.trim().is_empty() {
            return Err(GitError::Failed {
                command: "commit".into(),
                message: "the message is empty".into(),
            });
        }
        self.git(&["add", "-A", "--", "."])?;
        self.git(&["commit", "-m", message, "--", "."])?;
        Ok(self
            .git(&["rev-parse", "--short", "HEAD"])?
            .trim()
            .to_owned())
    }

    /// `git pull --ff-only`: brings in the remote branch when that needs no
    /// merge. Returns what git said.
    pub fn pull(&self) -> Result<String, GitError> {
        self.git(&["pull", "--ff-only"])
            .map(|s| s.trim().to_owned())
    }

    /// `git push`, setting the upstream to `origin` the first time.
    pub fn push(&self) -> Result<String, GitError> {
        let has_upstream = self
            .git(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
            .is_ok();
        let out = if has_upstream {
            self.run_text(&["push"])?
        } else {
            let branch = self.branch().unwrap_or_else(|| "HEAD".into());
            self.run_text(&["push", "-u", "origin", &branch])?
        };
        Ok(out)
    }

    /// The last `n` commits that touched the folder: short hash, subject and
    /// how long ago.
    pub fn log(&self, n: usize) -> Result<Vec<Commit>, GitError> {
        if !self.has_commit("HEAD") {
            return Ok(Vec::new());
        }
        let count = format!("-{n}");
        let out = self.git(&["log", &count, "--format=%h%x09%ct%x09%s", "--", "."])?;
        Ok(out
            .lines()
            .filter_map(|l| {
                let mut parts = l.splitn(3, '\t');
                Some(Commit {
                    hash: parts.next()?.to_owned(),
                    time: parts.next()?.parse().ok()?,
                    subject: parts.next()?.to_owned(),
                })
            })
            .collect())
    }

    /// Like `git`, but with what git printed on stderr (push reports there).
    fn run_text(&self, args: &[&str]) -> Result<String, GitError> {
        let mut command = self.command(args);
        let output = command.output().map_err(GitError::Missing)?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .trim()
        .to_owned();
        if output.status.success() {
            Ok(text)
        } else {
            Err(GitError::Failed {
                command: args.first().copied().unwrap_or_default().to_owned(),
                message: text,
            })
        }
    }

    fn ensure_commit(&self, rev: &str) -> Result<(), GitError> {
        if self.has_commit(rev) {
            Ok(())
        } else {
            Err(GitError::Failed {
                command: format!("rev-parse {rev}"),
                message: format!("`{rev}` is not a commit, branch or tag here"),
            })
        }
    }

    fn diff(&self, revs: &[&str]) -> Result<Vec<Change>, GitError> {
        let mut args = vec![
            "diff",
            "--name-status",
            "-z",
            "--no-renames",
            "--relative",
            "--no-ext-diff",
        ];
        args.extend_from_slice(revs);
        args.push("--");
        let out = self.git(&args)?;
        let mut fields = out.split('\0').filter(|f| !f.is_empty());
        let mut changes = Vec::new();
        while let (Some(status), Some(path)) = (fields.next(), fields.next()) {
            let kind = match status.chars().next() {
                Some('A') => ChangeKind::Added,
                Some('D') => ChangeKind::Deleted,
                _ => ChangeKind::Modified,
            };
            changes.push(Change {
                path: PathBuf::from(path),
                kind,
            });
        }
        Ok(changes)
    }

    fn untracked(&self) -> Result<Vec<Change>, GitError> {
        let out = self.git(&["ls-files", "--others", "--exclude-standard", "-z"])?;
        Ok(out
            .split('\0')
            .filter(|f| !f.is_empty())
            .map(|path| Change {
                path: PathBuf::from(path),
                kind: ChangeKind::Added,
            })
            .collect())
    }

    fn git(&self, args: &[&str]) -> Result<String, GitError> {
        self.run(args)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&self.dir)
            // Paths come back as they are, not quoted, whatever the user's config.
            .args(["-c", "core.quotepath=off"])
            .args(args)
            // Never wait for a password typed into a terminal nobody sees;
            // the system's credential helper still answers.
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // No console window flashing up from the desktop app.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command
    }

    fn run(&self, args: &[&str]) -> Result<Vec<u8>, GitError> {
        let output = self.command(args).output().map_err(GitError::Missing)?;
        if output.status.success() {
            Ok(output.stdout)
        } else {
            let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            Err(GitError::Failed {
                command: args.first().copied().unwrap_or_default().to_owned(),
                message: if message.is_empty() {
                    format!("exited with {}", output.status)
                } else {
                    message
                },
            })
        }
    }
}

/// `a/b/c` on every OS: git paths always use forward slashes.
pub fn slashes(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// What a changed file is to a collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileRole {
    /// A request file.
    Request,
    /// `<stem>.prerequest.js` or `<stem>.test.js`: belongs to the request `<stem>.apix` next to it.
    RequestScript(PathBuf),
    /// `folder.toml` or `_folder.*.js`: every request in this folder (relative to the collection).
    Folder(PathBuf),
    /// `apix.toml`, `_collection.*.js`, an environment or the project file: every request.
    Everything,
    /// Anything else, e.g. a body file or a `.proto`.
    Other,
}

/// What `path` (relative to a collection root) is to the collection.
pub fn role_of(path: &Path) -> FileRole {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();
    let top = dir.as_os_str().is_empty();
    if store::is_request_file(path) {
        return FileRole::Request;
    }
    if (top && name == COLLECTION_FILE) || name == crate::project::PROJECT_FILE {
        return FileRole::Everything;
    }
    if dir == Path::new(ENVIRONMENTS_DIR) && name.ends_with(".toml") {
        return FileRole::Everything;
    }
    for event in [Event::PreRequest, Event::Test] {
        let suffix = format!(".{}.js", event.suffix());
        if let Some(stem) = name.strip_suffix(&suffix) {
            return match stem {
                "_collection" if top => FileRole::Everything,
                "_folder" => FileRole::Folder(dir),
                _ => FileRole::RequestScript(
                    dir.join(format!("{stem}.{}", store::REQUEST_EXTENSION)),
                ),
            };
        }
    }
    if name == FOLDER_FILE {
        return FileRole::Folder(dir);
    }
    FileRole::Other
}

/// A request that has to run again, and the changed file that says so.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Affected {
    /// Absolute, as in [`crate::Collection::request_paths`].
    pub request: PathBuf,
    /// The changed file, relative to the folder the changes were listed in.
    pub because: PathBuf,
}

/// The requests (out of `requests`, all under `root`) that `changes` touch,
/// in the order of `requests`. `changes` are relative to `base`, which is
/// `root` or a folder above it (the project). A request is touched when its
/// file or its own scripts changed, when settings or scripts of a folder
/// around it or of the collection changed, when an environment or the
/// project file changed, or when it names another changed file (a body
/// file, a `.proto`).
pub fn affected(
    root: &Path,
    base: &Path,
    requests: &[PathBuf],
    changes: &[Change],
) -> Vec<Affected> {
    let rel_root = root.strip_prefix(base).unwrap_or(Path::new(""));
    let mut why: Vec<Option<PathBuf>> = vec![None; requests.len()];
    let mut mark = |pick: &dyn Fn(&Path) -> bool, because: &Path| {
        for (request, slot) in requests.iter().zip(why.iter_mut()) {
            if slot.is_none() && pick(request) {
                *slot = Some(because.to_path_buf());
            }
        }
    };
    for change in changes {
        // Outside the collection, only the project's own files matter.
        let Ok(inside) = change.path.strip_prefix(rel_root) else {
            let name = change.path.file_name().unwrap_or_default();
            let is_project_file =
                name == crate::project::PROJECT_FILE && change.path.parent() == Some(Path::new(""));
            let is_project_env = change.path.parent() == Some(Path::new(ENVIRONMENTS_DIR));
            if is_project_file || is_project_env {
                mark(&|_| true, &change.path);
            }
            continue;
        };
        match role_of(inside) {
            FileRole::Request => {
                let full = root.join(inside);
                mark(&|r| r == full, &change.path);
            }
            FileRole::RequestScript(request) => {
                let full = root.join(request);
                mark(&|r| r == full, &change.path);
            }
            FileRole::Folder(dir) => {
                let full = root.join(dir);
                mark(&|r| r.starts_with(&full), &change.path);
            }
            FileRole::Everything => mark(&|_| true, &change.path),
            FileRole::Other => {
                let Some(name) = inside.file_name().map(|n| n.to_string_lossy().into_owned())
                else {
                    continue;
                };
                mark(
                    &|r| {
                        crate::vfs::read_to_string(r)
                            .map(|text| text.contains(&name))
                            .unwrap_or(false)
                    },
                    &change.path,
                );
            }
        }
    }
    requests
        .iter()
        .zip(why)
        .filter_map(|(request, because)| {
            Some(Affected {
                request: request.clone(),
                because: because?,
            })
        })
        .collect()
}

/// Request files among `changes` that were deleted (relative to `base`).
pub fn removed_requests(changes: &[Change]) -> Vec<PathBuf> {
    changes
        .iter()
        .filter(|c| c.kind == ChangeKind::Deleted && store::is_request_file(&c.path))
        .map(|c| c.path.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(path: &str, kind: ChangeKind) -> Change {
        Change {
            path: PathBuf::from(path),
            kind,
        }
    }

    #[test]
    fn roles() {
        assert_eq!(role_of(Path::new("users/get.apix")), FileRole::Request);
        assert_eq!(
            role_of(Path::new("users/get.test.js")),
            FileRole::RequestScript(PathBuf::from("users/get.apix"))
        );
        assert_eq!(
            role_of(Path::new("users/_folder.prerequest.js")),
            FileRole::Folder(PathBuf::from("users"))
        );
        assert_eq!(
            role_of(Path::new("users/folder.toml")),
            FileRole::Folder(PathBuf::from("users"))
        );
        assert_eq!(role_of(Path::new("apix.toml")), FileRole::Everything);
        assert_eq!(
            role_of(Path::new("_collection.test.js")),
            FileRole::Everything
        );
        assert_eq!(
            role_of(Path::new("environments/dev.toml")),
            FileRole::Everything
        );
        assert_eq!(role_of(Path::new("users/apix.toml")), FileRole::Other);
        assert_eq!(role_of(Path::new("README.md")), FileRole::Other);
    }

    #[test]
    fn affected_follows_folders_scripts_and_names() {
        let dir = std::env::temp_dir().join(format!("apix-git-affected-{}", std::process::id()));
        let root = dir.join("api");
        std::fs::create_dir_all(root.join("users")).unwrap();
        std::fs::create_dir_all(root.join("orders")).unwrap();
        let get = root.join("users/get.apix");
        let list = root.join("users/list.apix");
        let order = root.join("orders/new.apix");
        std::fs::write(&get, "name = \"get\"\nurl = \"x\"\n").unwrap();
        std::fs::write(&list, "name = \"list\"\nurl = \"x\"\n").unwrap();
        std::fs::write(
            &order,
            "name = \"new\"\nurl = \"x\"\n[body.binary]\nfile = \"order.json\"\n",
        )
        .unwrap();
        let requests = vec![get.clone(), list.clone(), order.clone()];

        let pick = |changes: &[Change]| -> Vec<PathBuf> {
            affected(&root, &dir, &requests, changes)
                .into_iter()
                .map(|a| a.request)
                .collect()
        };
        assert_eq!(
            pick(&[change("api/users/get.test.js", ChangeKind::Modified)]),
            vec![get.clone()]
        );
        assert_eq!(
            pick(&[change("api/users/folder.toml", ChangeKind::Added)]),
            vec![get.clone(), list.clone()]
        );
        assert_eq!(
            pick(&[change("api/orders/order.json", ChangeKind::Modified)]),
            vec![order.clone()]
        );
        assert_eq!(
            pick(&[change("api/environments/dev.toml", ChangeKind::Modified)]).len(),
            3
        );
        // The project around the collection.
        assert_eq!(
            pick(&[change("apix-project.toml", ChangeKind::Modified)]).len(),
            3
        );
        assert_eq!(
            pick(&[change("environments/prod.toml", ChangeKind::Added)]).len(),
            3
        );
        // Another collection of the project.
        assert!(pick(&[change("other/users/get.apix", ChangeKind::Modified)]).is_empty());
        // The first reason is kept.
        let both = affected(
            &root,
            &dir,
            &requests,
            &[
                change("api/users/get.apix", ChangeKind::Modified),
                change("api/apix.toml", ChangeKind::Modified),
            ],
        );
        assert_eq!(both[0].because, PathBuf::from("api/users/get.apix"));
        assert_eq!(both[1].because, PathBuf::from("api/apix.toml"));
        assert_eq!(
            removed_requests(&[
                change("api/users/old.apix", ChangeKind::Deleted),
                change("api/users/old.test.js", ChangeKind::Deleted),
            ]),
            vec![PathBuf::from("api/users/old.apix")]
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
