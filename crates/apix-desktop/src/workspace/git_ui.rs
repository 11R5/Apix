//! Git in the status bar and the sidebar: the branch the open collection (or
//! project) is on, how many of its files changed since the last commit, and
//! a mark on each request that changed. Read in the background after every
//! reload and when the window comes back to the front (a commit made in a
//! terminal changes nothing in the collection folder).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use apix_core::git::{self, ChangeKind, FileRole, Repo};
use gpui_kit::*;

use super::Workspace;
use crate::tr;

/// What git said about the open folder.
pub(super) struct GitStatus {
    /// The folder it is for: the project, or the single collection.
    root: PathBuf,
    branch: String,
    /// Changed files, by absolute path; a request whose scripts changed is listed too.
    changes: HashMap<PathBuf, ChangeKind>,
    /// Changed files in the folder, as git counts them.
    count: usize,
}

#[derive(Default)]
pub(super) struct GitState {
    status: Option<GitStatus>,
    _task: Option<Task<()>>,
}

impl Workspace {
    /// Reads the branch and the changed files again, without blocking.
    pub(super) fn refresh_git(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_git_now(cx);
    }

    /// The same, from places without a window (the Git window after a commit).
    pub(super) fn refresh_git_now(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.workspace_root() else {
            self.git = GitState::default();
            return;
        };
        let task = cx.spawn(async move |this, cx| {
            let found = cx.background_spawn(async move { status_of(&root) }).await;
            this.update(cx, |this, cx| {
                this.git.status = found;
                cx.notify();
            })
            .ok();
        });
        self.git._task = Some(task);
    }

    /// How `path` (a request file) differs from the last commit, if it does.
    pub(super) fn git_change(&self, path: &Path) -> Option<ChangeKind> {
        let status = self.git.status.as_ref()?;
        if !path.starts_with(&status.root) {
            return None;
        }
        status.changes.get(path).copied()
    }

    /// `⎇ main · 3 changed` for the status bar, when the folder is in a repository.
    pub(super) fn render_git_status(&self) -> Option<impl IntoElement + use<>> {
        let status = self.git.status.as_ref()?;
        let p = crate::theme::current();
        let changes = match status.count {
            0 => None,
            1 => Some(tr!("git.one_change")),
            n => Some(tr!("git.changes", count = n)),
        };
        Some(
            div()
                .id("git-status")
                .flex()
                .flex_none()
                .gap_1()
                .px_1()
                .rounded_sm()
                .cursor_pointer()
                .hover(|s| s.bg(p.hover))
                .on_click(|_, window, cx| window.dispatch_action(Box::new(super::OpenGit), cx))
                .child(
                    div()
                        .text_color(p.text)
                        .child(format!("\u{2387} {}", status.branch)),
                )
                .children(
                    changes.map(|text| div().text_color(p.warn).child(format!("\u{00b7} {text}"))),
                ),
        )
    }
}

/// The colour and letter a changed request gets in the sidebar.
pub(super) fn change_mark(kind: ChangeKind) -> (Hsla, char) {
    let p = crate::theme::current();
    let colour = match kind {
        ChangeKind::Added => p.ok,
        ChangeKind::Modified => p.warn,
        ChangeKind::Deleted => p.err,
    };
    (colour, kind.letter())
}

/// `root` joined with a path git gave (with `/`), in the form the sidebar's paths have.
fn native(root: &Path, path: &Path) -> PathBuf {
    let mut out = root.to_path_buf();
    out.extend(path.components());
    out
}

fn status_of(root: &Path) -> Option<GitStatus> {
    let repo = Repo::at(root)?;
    let branch = repo.branch().unwrap_or_else(|| "?".to_owned());
    let found = repo.working_changes().unwrap_or_default();
    let count = found.len();
    let mut changes = HashMap::new();
    for change in found {
        // A request's own scripts changing changes the request.
        if let FileRole::RequestScript(request) = git::role_of(&change.path) {
            changes
                .entry(native(root, &request))
                .or_insert(ChangeKind::Modified);
        }
        changes.insert(native(root, &change.path), change.kind);
    }
    Some(GitStatus {
        root: root.to_path_buf(),
        branch,
        changes,
        count,
    })
}
