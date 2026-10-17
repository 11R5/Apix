//! The collection side of the window: the sidebar tree, opening and
//! importing collections, saving, and file operations (new, rename,
//! duplicate, delete to trash). The folder is watched, so changes made
//! outside Apix (a `git pull`, another editor) show up on their own.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use apix_core::project::{self, GroupBy, Project};
use apix_core::store::{self, CollectionMeta, StoreError, TreeNode};
use apix_core::{Body, Collection, Method, Request};
use apix_import::postman::{self, Kind};
use gpui_kit::component::IndexPath;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::AlertDialog;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::select::Select;
use gpui_kit::component::{Icon, IconName, Sizable, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::auth_editor::AuthOwner;
use super::{NO_ENVIRONMENT, Workspace, describe, key_hint};
use crate::theme;

/// Keeps the folder watcher alive; dropping it stops watching.
pub(super) struct Watcher {
    _watcher: notify::RecommendedWatcher,
    _poll: Task<()>,
}

/// Something to do once unsaved changes are saved or discarded.
type Then = Rc<dyn Fn(&mut Workspace, &mut Window, &mut Context<Workspace>)>;

impl Workspace {
    // -----------------------------------------------------------------------
    // Opening and reloading
    // -----------------------------------------------------------------------

    /// Opens what `dir` belongs to: the whole project if it is inside one,
    /// otherwise the collection around it.
    pub(super) fn open_collection_dir(
        &mut self,
        dir: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let project = match Project::find_root(dir).map(Project::open).transpose() {
            Ok(project) => project,
            Err(e) => {
                self.notify_error(describe(&e), window, cx);
                return;
            }
        };
        let collection = Collection::find_root(dir);
        let root = match (&project, &collection) {
            (Some(p), _) => p.root.clone(),
            (None, Some(c)) => c.clone(),
            (None, None) => {
                self.notify_error(
                    format!(
                        "No apix.toml or apix-project.toml in {} or its parents.",
                        dir.display()
                    ),
                    window,
                    cx,
                );
                return;
            }
        };

        self.project = project;
        self.collections.clear();
        self.collection = None;
        if self.project.is_none() {
            match Collection::open(&root) {
                Ok(c) => self.collections.push(c),
                Err(e) => {
                    self.notify_error(describe(&e), window, cx);
                    return;
                }
            }
        }
        self.environment = self.app_state.environment_for(&root);
        self.app_state.remember_collection(&root);
        self.watcher = watch(&root, window, cx);
        self.collapsed.clear();
        self.selected = None;
        self.reload(window, cx);

        // Inside a project, start in the collection that was opened, if any.
        let start = collection
            .and_then(|c| self.collections.iter().position(|x| x.root == c))
            .unwrap_or(0);
        self.collection = self.collections.get(start).cloned();
        let first = self
            .collection
            .as_ref()
            .and_then(|c| self.trees.iter().find(|(root, _)| *root == c.root))
            .and_then(|(_, tree)| first_request(tree));
        match first {
            Some(path) => self.open_path(path, window, cx),
            None => {
                self.rebuild_vars(window, cx);
                self.show_request(Request::default(), window, cx);
            }
        }
    }

    /// The project root, or the collection root when no project is open.
    pub(super) fn workspace_root(&self) -> Option<PathBuf> {
        self.project
            .as_ref()
            .map(|p| p.root.clone())
            .or_else(|| self.collections.first().map(|c| c.root.clone()))
    }

    pub(super) fn collection_for(&self, path: &Path) -> Option<&Collection> {
        self.collections
            .iter()
            .filter(|c| path.starts_with(&c.root))
            .max_by_key(|c| c.root.components().count())
    }

    /// Re-reads the project and its collections from disk. If the open
    /// request changed on disk and has no unsaved edits, the editor follows.
    pub(super) fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.load_errors.clear();
        if let Some(project) = &self.project {
            match Project::open(&project.root) {
                Ok(fresh) => {
                    let (collections, errors) = fresh.collections();
                    self.collections = collections;
                    self.load_errors.extend(errors);
                    self.project = Some(fresh);
                }
                Err(e) => self.load_errors.push(describe(&e)),
            }
        } else if let Some(root) = self.collections.first().map(|c| c.root.clone()) {
            match Collection::open(&root) {
                Ok(fresh) => self.collections = vec![fresh],
                Err(e) => self.load_errors.push(describe(&e)),
            }
        }
        // Keep the active collection if it still exists.
        self.collection = self
            .collection
            .as_ref()
            .and_then(|active| self.collections.iter().find(|c| c.root == active.root))
            .or_else(|| self.collections.first())
            .cloned();

        self.trees = self
            .collections
            .iter()
            .map(|c| (c.root.clone(), c.tree()))
            .collect();
        self.requests.clear();
        let mut paths = Vec::new();
        for (_, tree) in &self.trees {
            request_paths(tree, &mut paths);
        }
        let root = self.workspace_root().unwrap_or_default();
        for path in paths {
            match store::load_request(&path) {
                Ok(mut request) => {
                    if request.name.is_empty() {
                        request.name = file_stem(&path);
                    }
                    self.requests.insert(path, request);
                }
                Err(e) => {
                    let rel = path
                        .strip_prefix(&root)
                        .unwrap_or(&path)
                        .display()
                        .to_string();
                    let cause = std::error::Error::source(&e)
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| e.to_string());
                    self.load_errors.push(format!("{rel}: {cause}"));
                }
            }
        }
        self.refresh_environments(window, cx);
        // A folder.toml or apix.toml may have changed what the open request inherits.
        self.refresh_inherited_auth(cx);

        if let Some(path) = self.selected.clone() {
            let dirty = self.is_dirty(cx);
            match self.requests.get(&path).cloned() {
                // Deleted or moved outside Apix: keep unsaved edits, otherwise clear.
                None if !dirty => {
                    self.selected = None;
                    self.show_request(Request::default(), window, cx);
                }
                Some(on_disk) if on_disk != self.base && !dirty => {
                    self.show_request(on_disk, window, cx);
                }
                _ => {}
            }
        }
        cx.notify();
    }

    // -----------------------------------------------------------------------
    // Environments
    // -----------------------------------------------------------------------

    /// Re-reads `environments/`, keeps the choice if it still exists, and
    /// rebuilds the variables used for sending.
    fn refresh_environments(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let names = self.environment_names();
        if self
            .environment
            .as_ref()
            .is_some_and(|current| !names.contains(current))
        {
            self.environment = None;
        }
        let selected = self
            .environment
            .as_ref()
            .and_then(|current| names.iter().position(|n| n == current))
            .map_or(0, |ix| ix + 1);
        let items: Vec<SharedString> = std::iter::once(SharedString::from(NO_ENVIRONMENT))
            .chain(names.into_iter().map(SharedString::from))
            .collect();
        self.env_select.update(cx, |state, cx| {
            state.set_items(items, window, cx);
            state.set_selected_index(Some(IndexPath::new(selected)), window, cx);
        });
        self.rebuild_vars(window, cx);
    }

    pub(super) fn set_environment(
        &mut self,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.environment == name {
            return;
        }
        self.environment = name;
        if let Some(root) = self.workspace_root() {
            self.app_state
                .set_environment(&root, self.environment.as_deref());
        }
        // The env picker and the "group by environment" headers both set this.
        let names = self.environment_names();
        let selected = self
            .environment
            .as_ref()
            .and_then(|current| names.iter().position(|n| n == current))
            .map_or(0, |ix| ix + 1);
        self.env_select.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(selected)), window, cx)
        });
        self.rebuild_vars(window, cx);
        cx.notify();
    }

    /// Environments of the project and all its collections, or of the open collection.
    pub(super) fn environment_names(&self) -> Vec<String> {
        match &self.project {
            Some(project) => project.environment_names(&self.collections),
            None => self
                .collections
                .first()
                .map(Collection::environment_names)
                .unwrap_or_default(),
        }
    }

    /// Project → collection → project environment → collection environment,
    /// for the collection of the open request.
    fn rebuild_vars(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match project::variables(
            self.project.as_ref(),
            self.collection.as_ref(),
            self.environment.as_deref(),
        ) {
            Ok(vars) => self.vars = vars,
            // The environment exists elsewhere in the project, just not here: use the shared layers.
            Err(StoreError::UnknownEnvironment(_)) => {
                self.vars =
                    project::variables(self.project.as_ref(), self.collection.as_ref(), None)
                        .unwrap_or_default();
            }
            Err(e) => self.notify_error(describe(&e), window, cx),
        }
    }

    /// Production environments get a red line under the title bar.
    pub(super) fn is_prod(&self) -> bool {
        self.environment
            .as_ref()
            .is_some_and(|name| name.to_lowercase().contains("prod"))
    }

    /// Opens a request in the editor, asking first if the current one has unsaved edits.
    pub(super) fn select(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected.as_ref() == Some(&path) {
            return;
        }
        self.guard_unsaved(
            window,
            cx,
            Rc::new(move |this, window, cx| this.open_path(path.clone(), window, cx)),
        );
    }

    fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let Some(request) = self.requests.get(&path).cloned() else {
            return;
        };
        self.set_active_collection(&path, window, cx);
        self.selected = Some(path);
        self.show_request(request, window, cx);
    }

    /// Makes the collection that holds `path` the one whose variables apply.
    fn set_active_collection(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let Some(found) = self.collection_for(path).cloned() else {
            return;
        };
        if self
            .collection
            .as_ref()
            .is_some_and(|c| c.root == found.root)
        {
            return;
        }
        self.collection = Some(found);
        self.rebuild_vars(window, cx);
    }

    pub(super) fn is_dirty(&self, cx: &App) -> bool {
        self.selected.is_some() && self.build_request(cx) != self.base
    }

    /// Runs `then` now, or after the user saves or discards unsaved edits.
    fn guard_unsaved(&mut self, window: &mut Window, cx: &mut Context<Self>, then: Then) {
        if !self.is_dirty(cx) {
            then(self, window, cx);
            return;
        }
        let view = cx.entity().downgrade();
        let name = self.base.name.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let (save_view, discard_view, cancel_view) = (view.clone(), view.clone(), view.clone());
            let (save_then, discard_then) = (then.clone(), then.clone());
            dialog
                .on_close(refocus(view.clone()))
                .title("Unsaved changes")
                .child(div().text_sm().child(format!(
                    "Save your changes to \u{201c}{name}\u{201d} first?"
                )))
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("unsaved-cancel")
                                .ghost()
                                .label("Cancel")
                                .on_click(move |_, window, cx| {
                                    close_dialog(&cancel_view, window, cx)
                                }),
                        )
                        .child(Button::new("unsaved-discard").label("Don't save").on_click(
                            move |_, window, cx| {
                                close_dialog(&discard_view, window, cx);
                                discard_view
                                    .update(cx, |this, cx| discard_then(this, window, cx))
                                    .ok();
                            },
                        ))
                        .child(
                            Button::new("unsaved-save")
                                .primary()
                                .label("Save")
                                .on_click(move |_, window, cx| {
                                    close_dialog(&save_view, window, cx);
                                    save_view
                                        .update(cx, |this, cx| {
                                            if this.save(window, cx) {
                                                save_then(this, window, cx);
                                            }
                                        })
                                        .ok();
                                }),
                        ),
                )
        });
    }

    // -----------------------------------------------------------------------
    // Saving and file operations
    // -----------------------------------------------------------------------

    /// Saves the open request; a request that has no file yet is created in
    /// the collection root. Returns whether it was saved.
    pub(super) fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let mut request = self.build_request(cx);
        let path = match &self.selected {
            Some(path) => path.clone(),
            None => {
                let Some(collection) = &self.collection else {
                    self.notify_error(
                        format!("Open or create a collection first ({}).", key_hint("O")),
                        window,
                        cx,
                    );
                    return false;
                };
                if request.name.is_empty() {
                    request.name = "New request".into();
                }
                match store::create_request(&collection.root, &request) {
                    Ok(path) => path,
                    Err(e) => {
                        self.notify_error(describe(&e), window, cx);
                        return false;
                    }
                }
            }
        };
        if let Err(e) = store::save_request(&path, &request) {
            self.notify_error(describe(&e), window, cx);
            return false;
        }
        let created = self.selected.is_none();
        self.base = request.clone();
        self.requests.insert(path.clone(), request);
        self.selected = Some(path);
        if created {
            self.reload(window, cx);
        }
        cx.notify();
        true
    }

    /// The folder new items go into: the given one, else the open request's folder, else the root.
    fn target_dir(&self, dir: Option<PathBuf>) -> Option<PathBuf> {
        let root = self.collection.as_ref()?.root.clone();
        Some(
            dir.or_else(|| {
                self.selected
                    .as_ref()
                    .and_then(|p| p.parent())
                    .map(Path::to_path_buf)
            })
            .unwrap_or(root),
        )
    }

    /// Creates a request in `dir` (or next to the open one) and opens it.
    /// POST, PUT and PATCH start with an empty JSON body.
    pub(super) fn new_request(
        &mut self,
        dir: Option<PathBuf>,
        method: Method,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dir) = self.target_dir(dir) else {
            self.notify_error(
                format!("Open or create a collection first ({}).", key_hint("O")),
                window,
                cx,
            );
            return;
        };
        self.guard_unsaved(
            window,
            cx,
            Rc::new(move |this, window, cx| {
                let body = match method {
                    Method::Post | Method::Put | Method::Patch => Body::Json("{\n  \n}".into()),
                    _ => Body::None,
                };
                let request = Request {
                    name: "New request".into(),
                    body,
                    ..Request::new(method, "")
                };
                match store::create_request(&dir, &request) {
                    Ok(path) => {
                        this.collapsed.remove(&dir);
                        this.reload(window, cx);
                        this.open_path(path, window, cx);
                        this.url.update(cx, |state, cx| state.focus(window, cx));
                    }
                    Err(e) => this.notify_error(describe(&e), window, cx),
                }
            }),
        );
    }

    pub(super) fn new_folder(
        &mut self,
        parent: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(parent) = parent.or_else(|| self.collection.as_ref().map(|c| c.root.clone()))
        else {
            return;
        };
        self.prompt_name(
            "New folder",
            String::new(),
            "Create",
            window,
            cx,
            move |this, name, window, cx| match store::create_folder(&parent, &name) {
                Ok(_) => {
                    this.collapsed.remove(&parent);
                    this.reload(window, cx);
                }
                Err(e) => this.notify_error(describe(&e), window, cx),
            },
        );
    }

    fn rename_request(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let current = self
            .requests
            .get(&path)
            .map(|r| r.name.clone())
            .unwrap_or_default();
        self.prompt_name(
            "Rename request",
            current,
            "Rename",
            window,
            cx,
            move |this, name, window, cx| match store::rename_request(&path, &name) {
                Ok(new_path) => {
                    if this.selected.as_ref() == Some(&path) {
                        this.selected = Some(new_path);
                        this.base.name = name;
                    }
                    this.reload(window, cx);
                }
                Err(e) => this.notify_error(describe(&e), window, cx),
            },
        );
    }

    fn rename_folder(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let current = file_stem(&path);
        self.prompt_name(
            "Rename folder",
            current,
            "Rename",
            window,
            cx,
            move |this, name, window, cx| match store::rename_folder(&path, &name) {
                Ok(new_path) => {
                    if let Some(rest) = this
                        .selected
                        .as_ref()
                        .and_then(|s| s.strip_prefix(&path).ok())
                    {
                        this.selected = Some(new_path.join(rest));
                    }
                    this.reload(window, cx);
                }
                Err(e) => this.notify_error(describe(&e), window, cx),
            },
        );
    }

    fn duplicate(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        match store::duplicate_request(&path) {
            Ok(copy) => {
                self.reload(window, cx);
                self.select(copy, window, cx);
            }
            Err(e) => self.notify_error(describe(&e), window, cx),
        }
    }

    /// Asks, then moves a request (with its scripts) or a whole folder to the OS trash.
    fn confirm_delete(
        &mut self,
        path: PathBuf,
        folder: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = if folder {
            file_stem(&path)
        } else {
            self.requests
                .get(&path)
                .map(|r| r.name.clone())
                .unwrap_or_else(|| file_stem(&path))
        };
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert: AlertDialog, _, _| {
            let (view, path) = (view.clone(), path.clone());
            let what = if folder {
                "Delete folder?"
            } else {
                "Delete request?"
            };
            let detail = if folder {
                format!("\u{201c}{name}\u{201d} and everything in it will be moved to the trash.")
            } else {
                format!("\u{201c}{name}\u{201d} will be moved to the trash.")
            };
            alert
                .on_close(refocus(view.clone()))
                .confirm()
                .title(what)
                .description(SharedString::from(detail))
                .ok_text("Move to trash")
                .on_ok(move |_, window, cx| {
                    view.update(cx, |this, cx| this.delete(path.clone(), folder, window, cx))
                        .ok();
                    true
                })
        });
    }

    fn delete(&mut self, path: PathBuf, folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        let files = if folder {
            vec![path.clone()]
        } else {
            store::request_files(&path)
        };
        if let Err(e) = trash::delete_all(&files) {
            self.notify_error(format!("Could not move to the trash: {e}"), window, cx);
            return;
        }
        if self.selected.as_ref().is_some_and(|s| s.starts_with(&path)) {
            self.selected = None;
            self.show_request(Request::default(), window, cx);
        }
        self.reload(window, cx);
    }

    /// A small dialog with one text field, used for names.
    fn prompt_name(
        &mut self,
        title: &'static str,
        initial: String,
        ok_label: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_ok: impl Fn(&mut Workspace, String, &mut Window, &mut Context<Workspace>) + 'static,
    ) {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(initial));
        let view = cx.entity().downgrade();
        let on_ok = Rc::new(on_ok);
        let first = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first.replace(false) {
                input.update(cx, |state, cx| state.focus(window, cx));
            }
            let confirm = {
                let (input, view, on_ok) = (input.clone(), view.clone(), on_ok.clone());
                Rc::new(move |window: &mut Window, cx: &mut App| -> bool {
                    let name = input.read(cx).value().trim().to_string();
                    if name.is_empty() {
                        return false;
                    }
                    view.update(cx, |this, cx| {
                        on_ok(this, name, window, cx);
                        this.focus.focus(window, cx);
                    })
                    .ok();
                    true
                })
            };
            let on_enter = confirm.clone();
            let cancel_view = view.clone();
            dialog
                .on_close(refocus(view.clone()))
                .title(title)
                .width(px(420.))
                .child(Input::new(&input))
                .on_ok(move |_, window, cx| on_enter(window, cx))
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("name-cancel").ghost().label("Cancel").on_click(
                                move |_, window, cx| close_dialog(&cancel_view, window, cx),
                            ),
                        )
                        .child(Button::new("name-ok").primary().label(ok_label).on_click(
                            move |_, window, cx| {
                                if confirm(window, cx) {
                                    window.close_dialog(cx);
                                }
                            },
                        )),
                )
        });
    }

    // -----------------------------------------------------------------------
    // Open, import, drop
    // -----------------------------------------------------------------------

    pub(super) fn prompt_open_collection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open collection".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await
                && let Some(dir) = paths.into_iter().next()
            {
                this.update_in(cx, |this, window, cx| this.open_or_create(dir, window, cx))
                    .ok();
            }
        })
        .detach();
    }

    /// Opens the collection around `dir`, or offers to start one there.
    pub(super) fn open_or_create(
        &mut self,
        dir: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if Collection::find_root(&dir).is_some() {
            self.guard_unsaved(
                window,
                cx,
                Rc::new(move |this, window, cx| this.open_collection_dir(&dir, window, cx)),
            );
            return;
        }
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert: AlertDialog, _, _| {
            let (view, dir) = (view.clone(), dir.clone());
            alert
                .on_close(refocus(view.clone()))
                .confirm()
                .title("Start a new collection here?")
                .description(SharedString::from(format!(
                    "{} has no apix.toml yet. Apix will create one; requests are saved as .apix files in this folder.",
                    dir.display()
                )))
                .ok_text("Create collection")
                .on_ok(move |_, window, cx| {
                    let meta = CollectionMeta {
                        name: dir
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "Collection".into()),
                        ..CollectionMeta::default()
                    };
                    view.update(cx, |this, cx| match store::save_collection_meta(&dir, &meta) {
                        Ok(()) => {
                            let dir = dir.clone();
                            this.guard_unsaved(window, cx, Rc::new(move |this, window, cx| {
                                this.open_collection_dir(&dir, window, cx)
                            }));
                        }
                        Err(e) => this.notify_error(describe(&e), window, cx),
                    })
                    .ok();
                    true
                })
        });
    }

    pub(super) fn prompt_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import Postman export".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await
                && let Some(file) = paths.into_iter().next()
            {
                this.update_in(cx, |this, window, cx| this.import_file(file, window, cx))
                    .ok();
            }
        })
        .detach();
    }

    pub(super) fn handle_dropped(
        &mut self,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = paths.into_iter().next() else {
            return;
        };
        if path.is_dir() {
            self.open_or_create(path, window, cx);
        } else {
            self.import_file(path, window, cx);
        }
    }

    fn import_file(&mut self, file: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let json = match std::fs::read_to_string(&file) {
            Ok(json) => json,
            Err(e) => {
                self.notify_error(format!("{}: {e}", file.display()), window, cx);
                return;
            }
        };
        match postman::detect(&json) {
            Ok(Kind::Collection) => self.import_collection(&json, window, cx),
            Ok(Kind::Environment | Kind::Globals) => self.import_environment(&json, window, cx),
            Err(e) => self.notify_error(e.to_string(), window, cx),
        }
    }

    /// Converts a Postman collection, asks where to put it, writes it and opens it.
    fn import_collection(&mut self, json: &str, window: &mut Window, cx: &mut Context<Self>) {
        let plan = match postman::convert_collection(json) {
            Ok(plan) => plan,
            Err(e) => {
                self.notify_error(e.to_string(), window, cx);
                return;
            }
        };
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Create the collection in".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(parent) = paths.into_iter().next() else {
                return;
            };
            let root = store::unique_path(&parent, &store::slug(&plan.meta.name, "collection"), "");
            this.update_in(cx, |this, window, cx| {
                if let Err(e) = plan.write(&root) {
                    this.notify_error(e.to_string(), window, cx);
                    return;
                }
                let report = plan.report.to_string();
                let has_warnings = !plan.report.warnings.is_empty();
                let root = root.clone();
                this.guard_unsaved(
                    window,
                    cx,
                    Rc::new(move |this, window, cx| this.open_collection_dir(&root, window, cx)),
                );
                window.push_notification(
                    Notification::success(report.trim_end().to_owned())
                        .title("Postman collection imported")
                        .autohide(!has_warnings),
                    cx,
                );
            })
            .ok();
        })
        .detach();
    }

    fn import_environment(&mut self, json: &str, window: &mut Window, cx: &mut Context<Self>) {
        // In a project the environment is shared by all its collections.
        let Some(root) = self.workspace_root() else {
            self.notify_error(
                "Open the collection or project this environment belongs to first, then import it.",
                window,
                cx,
            );
            return;
        };
        let result = postman::convert_environment(json).and_then(|env| {
            env.write(&root)?;
            Ok(env)
        });
        match result {
            Ok(env) => {
                let mut message = format!(
                    "\u{201c}{}\u{201d} saved as environments/{}.toml with {} variables.",
                    env.title,
                    env.name,
                    env.vars.len()
                );
                if !env.secrets.is_empty() {
                    message.push_str(&format!(
                        " {} secrets went to {}.local.toml, which git ignores.",
                        env.secrets.len(),
                        env.name
                    ));
                }
                window.push_notification(
                    Notification::success(message).title("Environment imported"),
                    cx,
                );
            }
            Err(e) => self.notify_error(e.to_string(), window, cx),
        }
    }

    pub(super) fn notify_error(
        &self,
        message: impl Into<String>,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.push_notification(Notification::error(message.into()), cx);
    }

    // -----------------------------------------------------------------------
    // Sidebar
    // -----------------------------------------------------------------------

    pub(super) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let open = !self.collections.is_empty() || self.project.is_some();
        let by_environment = self
            .project
            .as_ref()
            .is_some_and(|pr| pr.meta.group_by == GroupBy::Environment);
        let icon_button = |id: &'static str, icon: IconName, tooltip: String| {
            Button::new(id).ghost().xsmall().icon(icon).tooltip(tooltip)
        };

        let (kind, title) = match (&self.project, self.collections.first()) {
            (Some(project), _) => ("PROJECT", project.meta.name.clone()),
            (None, Some(c)) => ("COLLECTION", c.meta.name.clone()),
            (None, None) => ("APIX", "Open or create…".to_owned()),
        };
        let menu_state = WorkspaceMenu {
            view: cx.entity().downgrade(),
            has_project: self.project.is_some(),
            group_by: self.project.as_ref().map(|p| p.meta.group_by),
            recent: self
                .app_state
                .recent_collections()
                .filter(|r| Some(*r) != self.workspace_root().as_ref())
                .take(6)
                .cloned()
                .collect(),
        };
        let header = v_flex()
            .px_3()
            .pt_2()
            .pb_1()
            .child(div().px_1().text_xs().text_color(p.text_muted).child(kind))
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        div().flex_1().min_w_0().child(
                            Button::new("workspace-menu")
                                .ghost()
                                .small()
                                .label(title)
                                .dropdown_caret(true)
                                .dropdown_menu(move |menu, _, _| menu_state.build(menu)),
                        ),
                    )
                    .when(open, |bar| {
                        bar.child(
                            icon_button(
                                "sidebar-new",
                                IconName::Plus,
                                format!("New request ({} for GET)", key_hint("N")),
                            )
                            .dropdown_menu({
                                let view = cx.entity().downgrade();
                                move |menu, _, _| method_items(menu, view.clone(), None)
                            }),
                        )
                        .child(
                            icon_button(
                                "sidebar-folder",
                                IconName::FolderClosed,
                                "New folder".into(),
                            )
                            .on_click(cx.listener(
                                |this, _: &ClickEvent, window, cx| {
                                    this.new_folder(None, window, cx)
                                },
                            )),
                        )
                    }),
            );

        let env_row = (open && !by_environment).then(|| {
            h_flex()
                .px_3()
                .pb_2()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .w(px(8.))
                        .h(px(8.))
                        .flex_none()
                        .rounded_full()
                        .bg(self.environment_dot(self.environment.as_deref())),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(Select::new(&self.env_select).small()),
                )
                .child(
                    icon_button(
                        "sidebar-vars",
                        IconName::Settings,
                        format!("Edit variables ({})", key_hint("E")),
                    )
                    .on_click(cx.listener(
                        |this, _: &ClickEvent, window, cx| this.open_env_editor(window, cx),
                    )),
                )
        });

        let body = if !open {
            self.render_empty_state(cx).into_any_element()
        } else {
            let mut rows = Vec::new();
            if by_environment {
                self.render_environment_groups(&mut rows, cx);
            } else if self.project.is_some() || self.collections.len() > 1 {
                for collection in &self.collections {
                    self.render_collection(collection, 0, &mut rows, cx);
                }
            } else if let Some((_, tree)) = self.trees.first() {
                self.render_nodes(tree, 0, &mut rows, cx);
            }
            if rows.is_empty() {
                let hint = if self.project.is_some() && self.collections.is_empty() {
                    "No collections yet. Use the menu above: New collection…".to_owned()
                } else {
                    format!("No requests yet. {} creates one.", key_hint("N"))
                };
                rows.push(
                    div()
                        .px_4()
                        .py_2()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(hint)
                        .into_any_element(),
                );
            }
            v_flex()
                .gap_px()
                .px_2()
                .pb_2()
                .children(rows)
                .into_any_element()
        };

        v_flex()
            .w(px(248.))
            .flex_none()
            .h_full()
            .border_r_1()
            .border_color(p.hairline)
            .child(header)
            .children(env_row)
            .child(
                div()
                    .id("sidebar-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(body),
            )
            .children(self.load_errors.iter().map(|e| {
                div()
                    .px_4()
                    .py_1()
                    .text_xs()
                    .text_color(p.err)
                    .child(e.clone())
            }))
    }

    /// Green for an active environment, red for production, grey for none.
    fn environment_dot(&self, name: Option<&str>) -> Hsla {
        let p = self.palette;
        match name {
            Some(n) if n.to_lowercase().contains("prod") => p.err,
            Some(_) => p.ok,
            None => p.text_muted.opacity(0.4),
        }
    }

    /// One collection as a collapsible header with its tree below.
    fn render_collection(
        &self,
        collection: &Collection,
        depth: usize,
        out: &mut Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) {
        let p = self.palette;
        let collapsed = self.collapsed.contains(&collection.root);
        let active = self
            .collection
            .as_ref()
            .is_some_and(|c| c.root == collection.root);
        let view = cx.entity().downgrade();
        let toggle = collection.root.clone();
        let menu_root = collection.root.clone();
        let pins = self.project.as_ref().map(|project| {
            let all = self.environment_names();
            let shown: Vec<bool> = all
                .iter()
                .map(|env| project.shows(&collection.root, env))
                .collect();
            (all, shown)
        });
        out.push(
            row_base(&collection.root, depth)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if active { p.text } else { p.text_muted })
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if !this.collapsed.remove(&toggle) {
                        this.collapsed.insert(toggle.clone());
                    }
                    cx.notify();
                }))
                .child(
                    Icon::new(if collapsed {
                        IconName::ChevronRight
                    } else {
                        IconName::ChevronDown
                    })
                    .xsmall(),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(collection.meta.name.clone()),
                )
                .context_menu(move |menu: PopupMenu, window, cx| {
                    collection_menu(
                        menu,
                        view.clone(),
                        menu_root.clone(),
                        pins.clone(),
                        window,
                        cx,
                    )
                })
                .into_any_element(),
        );
        if !collapsed
            && let Some((_, tree)) = self.trees.iter().find(|(root, _)| *root == collection.root)
        {
            let before = out.len();
            self.render_nodes(tree, depth + 1, out, cx);
            if out.len() == before {
                out.push(
                    div()
                        .pl(px(8. + 14. * (depth + 1) as f32))
                        .py_1()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child("Empty")
                        .into_any_element(),
                );
            }
        }
    }

    /// Project → Environment → Collections. Clicking an environment makes it
    /// active and lists the collections shown in it.
    fn render_environment_groups(&self, out: &mut Vec<AnyElement>, cx: &mut Context<Self>) {
        let p = self.palette;
        let Some(project) = &self.project else {
            return;
        };
        let names = self.environment_names();
        if names.is_empty() {
            out.push(
                div()
                    .px_2()
                    .py_2()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child("This project has no environments yet. Add test.toml or prod.toml to its environments/ folder.")
                    .into_any_element(),
            );
            for collection in &self.collections {
                self.render_collection(collection, 0, out, cx);
            }
            return;
        }
        for (ix, name) in names.iter().enumerate() {
            let active = self.environment.as_deref() == Some(name.as_str());
            let shown: Vec<&Collection> = self
                .collections
                .iter()
                .filter(|c| project.shows(&c.root, name))
                .collect();
            let pick = name.clone();
            out.push(
                div()
                    .id(("env-group", ix))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .mt_1()
                    .rounded_md()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .hover(|s| s.bg(hsla(0., 0., 1., 0.05)))
                    .when(active, |row| row.bg(p.accent.opacity(0.10)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.set_environment(Some(pick.clone()), window, cx)
                    }))
                    .context_menu({
                        let view = cx.entity().downgrade();
                        move |menu, _, _| {
                            let view = view.clone();
                            menu.item(PopupMenuItem::new("Edit variables…").on_click(
                                move |_, window, cx| {
                                    view.update(cx, |this, cx| this.open_env_editor(window, cx))
                                        .ok();
                                },
                            ))
                        }
                    })
                    .child(
                        div()
                            .w(px(8.))
                            .h(px(8.))
                            .flex_none()
                            .rounded_full()
                            .bg(self.environment_dot(Some(name))),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(name.clone()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(shown.len().to_string()),
                    )
                    .into_any_element(),
            );
            if active {
                for collection in shown {
                    self.render_collection(collection, 1, out, cx);
                }
            }
        }
    }
    fn render_empty_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        v_flex()
            .px_4()
            .pt_4()
            .gap_2()
            .child(
                Button::new("empty-open")
                    .small()
                    .icon(IconName::FolderOpen)
                    .label("Open a folder")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.prompt_open_collection(window, cx)
                    })),
            )
            .child(
                Button::new("empty-import")
                    .small()
                    .icon(IconName::Inbox)
                    .label("Import from Postman")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.prompt_import(window, cx)
                    })),
            )
            .child(
                div()
                    .pt_1()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child("Or drop a Postman export or a folder onto the window."),
            )
            .children(self.render_recent(cx))
    }

    /// Recently opened collections, shown when nothing is open.
    fn render_recent(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let p = self.palette;
        let recent: Vec<PathBuf> = self.app_state.recent_collections().cloned().collect();
        if recent.is_empty() {
            return None;
        }
        let rows = recent.into_iter().enumerate().map(|(ix, root)| {
            let name = root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| root.display().to_string());
            let parent = root
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let open = root.clone();
            div()
                .id(("recent", ix))
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(hsla(0., 0., 1., 0.05)))
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.open_or_create(open.clone(), window, cx)
                }))
                .child(div().text_sm().truncate().child(name))
                .child(
                    div()
                        .text_xs()
                        .text_color(p.text_muted)
                        .truncate()
                        .child(parent),
                )
        });
        Some(
            v_flex()
                .pt_4()
                .gap_px()
                .child(
                    div()
                        .px_2()
                        .pb_1()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child("RECENT"),
                )
                .children(rows),
        )
    }

    fn render_nodes(
        &self,
        nodes: &[TreeNode],
        depth: usize,
        out: &mut Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) {
        for node in nodes {
            match node {
                TreeNode::Folder {
                    name,
                    path,
                    children,
                } => {
                    let collapsed = self.collapsed.contains(path);
                    out.push(
                        self.render_folder_row(name, path, depth, collapsed, cx)
                            .into_any_element(),
                    );
                    if !collapsed {
                        self.render_nodes(children, depth + 1, out, cx);
                    }
                }
                TreeNode::Request { path } => {
                    if let Some(request) = self.requests.get(path) {
                        out.push(
                            self.render_request_row(path, request, depth, cx)
                                .into_any_element(),
                        );
                    }
                }
            }
        }
    }

    fn render_folder_row(
        &self,
        name: &str,
        path: &Path,
        depth: usize,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = self.palette;
        let view = cx.entity().downgrade();
        let toggle_path = path.to_path_buf();
        let menu_path = path.to_path_buf();
        row_base(path, depth)
            .text_color(p.text_muted)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if !this.collapsed.remove(&toggle_path) {
                    this.collapsed.insert(toggle_path.clone());
                }
                cx.notify();
            }))
            .child(
                Icon::new(if collapsed {
                    IconName::ChevronRight
                } else {
                    IconName::ChevronDown
                })
                .xsmall(),
            )
            .child(div().truncate().child(name.to_owned()))
            .context_menu(move |menu: PopupMenu, window, cx| {
                folder_menu(menu, view.clone(), menu_path.clone(), window, cx)
            })
    }

    fn render_request_row(
        &self,
        path: &Path,
        request: &Request,
        depth: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = self.palette;
        let selected = self.selected.as_deref() == Some(path);
        let dirty = selected && self.is_dirty(cx);
        // The open request shows the editor's method, so the badge follows edits.
        let method = if selected {
            self.current_method(cx)
        } else {
            request.method
        };
        let view = cx.entity().downgrade();
        let open_path = path.to_path_buf();
        let menu_path = path.to_path_buf();
        row_base(path, depth)
            .when(selected, |row| row.bg(p.accent.opacity(0.14)))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.select(open_path.clone(), window, cx)
            }))
            .child(
                div()
                    .w(px(46.))
                    .flex_none()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::method_color(method))
                    .child(method.as_str()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(request.name.clone()),
            )
            .when(dirty, |row| {
                row.child(div().flex_none().text_color(p.warn).child("\u{25cf}"))
            })
            .context_menu(move |menu: PopupMenu, _, _| {
                request_menu(menu, view.clone(), menu_path.clone())
            })
    }
}

/// Closes the top dialog and gives focus back to the workspace, so keyboard
/// shortcuts work right away.
pub(super) fn close_dialog(view: &WeakEntity<Workspace>, window: &mut Window, cx: &mut App) {
    window.close_dialog(cx);
    view.update(cx, |this, cx| this.focus.focus(window, cx))
        .ok();
}

/// For `on_close`: however a dialog is dismissed (Esc, ×, overlay), focus returns to the workspace.
pub(super) fn refocus(
    view: WeakEntity<Workspace>,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    move |_, window, cx| {
        view.update(cx, |this, cx| this.focus.focus(window, cx))
            .ok();
    }
}

fn row_base(path: &Path, depth: usize) -> Stateful<Div> {
    div()
        .id(SharedString::from(path.to_string_lossy().into_owned()))
        .flex()
        .items_center()
        .gap_2()
        .pl(px(8. + 14. * depth as f32))
        .pr_2()
        .py_1()
        .rounded_md()
        .text_sm()
        .cursor_pointer()
        .hover(|s| s.bg(hsla(0., 0., 1., 0.05)))
}

fn request_menu(menu: PopupMenu, view: WeakEntity<Workspace>, path: PathBuf) -> PopupMenu {
    let item =
        |label: &'static str,
         f: fn(&mut Workspace, PathBuf, &mut Window, &mut Context<Workspace>)| {
            let (view, path) = (view.clone(), path.clone());
            PopupMenuItem::new(label).on_click(move |_, window, cx| {
                view.update(cx, |this, cx| f(this, path.clone(), window, cx))
                    .ok();
            })
        };
    let menu = menu
        .item(item("Rename…", |this, path, window, cx| {
            this.rename_request(path, window, cx)
        }))
        .item(item("Duplicate", |this, path, window, cx| {
            this.duplicate(path, window, cx)
        }))
        .separator();
    let menu = apix_codegen::Shell::ALL
        .into_iter()
        .fold(menu, |menu, shell| {
            let (view, path) = (view.clone(), path.clone());
            menu.item(
                PopupMenuItem::new(format!("Copy as {}", shell.label())).on_click(
                    move |_, window, cx| {
                        view.update(cx, |this, cx| {
                            this.copy_saved_as(path.clone(), shell, window, cx)
                        })
                        .ok();
                    },
                ),
            )
        });
    menu.separator()
        .item(item("Move to trash", |this, path, window, cx| {
            this.confirm_delete(path, false, window, cx)
        }))
}

/// One item per HTTP method; picking one creates a request with it in `dir`.
fn method_items(menu: PopupMenu, view: WeakEntity<Workspace>, dir: Option<PathBuf>) -> PopupMenu {
    Method::ALL.into_iter().fold(menu, |menu, method| {
        let (view, dir) = (view.clone(), dir.clone());
        menu.item(
            PopupMenuItem::new(method.as_str()).on_click(move |_, window, cx| {
                view.update(cx, |this, cx| {
                    this.new_request(dir.clone(), method, window, cx)
                })
                .ok();
            }),
        )
    })
}

/// "New request ▸" with a submenu of methods, for the folder and collection menus.
fn new_request_submenu(
    menu: PopupMenu,
    view: WeakEntity<Workspace>,
    dir: PathBuf,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    menu.submenu("New request", window, cx, move |menu, _, _| {
        method_items(menu, view.clone(), Some(dir.clone()))
    })
}

fn folder_menu(
    menu: PopupMenu,
    view: WeakEntity<Workspace>,
    path: PathBuf,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let item =
        |label: &'static str,
         f: fn(&mut Workspace, PathBuf, &mut Window, &mut Context<Workspace>)| {
            let (view, path) = (view.clone(), path.clone());
            PopupMenuItem::new(label).on_click(move |_, window, cx| {
                view.update(cx, |this, cx| f(this, path.clone(), window, cx))
                    .ok();
            })
        };
    new_request_submenu(menu, view.clone(), path.clone(), window, cx)
        .item(item("New folder…", |this, path, window, cx| {
            this.new_folder(Some(path), window, cx)
        }))
        .item(item("Rename…", |this, path, window, cx| {
            this.rename_folder(path, window, cx)
        }))
        .item(item("Auth…", |this, path, window, cx| {
            this.open_auth_dialog(AuthOwner::Folder(path), window, cx)
        }))
        .separator()
        .item(item("Move to trash", |this, path, window, cx| {
            this.confirm_delete(path, true, window, cx)
        }))
}

/// What the workspace menu (the project or collection name at the top of
/// the sidebar) needs; built fresh each time the menu opens.
#[derive(Clone)]
struct WorkspaceMenu {
    view: WeakEntity<Workspace>,
    has_project: bool,
    group_by: Option<GroupBy>,
    recent: Vec<PathBuf>,
}

impl WorkspaceMenu {
    fn build(&self, menu: PopupMenu) -> PopupMenu {
        let item = |label: &str, f: fn(&mut Workspace, &mut Window, &mut Context<Workspace>)| {
            let view = self.view.clone();
            PopupMenuItem::new(label.to_owned()).on_click(move |_, window, cx| {
                view.update(cx, |this, cx| f(this, window, cx)).ok();
            })
        };
        let mut menu = menu
            .item(item("New collection…", Workspace::prompt_new_collection))
            .item(item("New project…", Workspace::prompt_new_project))
            .separator()
            .item(item(
                &format!("Open… ({})", key_hint("O")),
                Workspace::prompt_open_collection,
            ))
            .item(item("Import from Postman…", Workspace::prompt_import))
            .separator()
            .item(item(
                &format!("Variables and environments… ({})", key_hint("E")),
                Workspace::open_env_editor,
            ))
            .item(item(
                if self.has_project {
                    "Project auth…"
                } else {
                    "Collection auth…"
                },
                |this, window, cx| {
                    let owner = match (&this.project, this.collections.first()) {
                        (Some(p), _) => AuthOwner::Project(p.root.clone()),
                        (None, Some(c)) => AuthOwner::Collection(c.root.clone()),
                        (None, None) => return,
                    };
                    this.open_auth_dialog(owner, window, cx)
                },
            ));

        if self.has_project {
            let group = |label: &str, by: GroupBy, current: Option<GroupBy>| {
                let view = self.view.clone();
                PopupMenuItem::new(label.to_owned())
                    .checked(current == Some(by))
                    .on_click(move |_, window, cx| {
                        view.update(cx, |this, cx| this.set_group_by(by, window, cx))
                            .ok();
                    })
            };
            menu = menu
                .separator()
                .label("Group the sidebar by")
                .item(group("Collection", GroupBy::Collection, self.group_by))
                .item(group("Environment", GroupBy::Environment, self.group_by));
        }

        if !self.recent.is_empty() {
            menu = menu.separator().label("Recent");
            for root in &self.recent {
                let (view, dir) = (self.view.clone(), root.clone());
                let name = root
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| root.display().to_string());
                menu = menu.item(PopupMenuItem::new(name).on_click(move |_, window, cx| {
                    view.update(cx, |this, cx| this.open_or_create(dir.clone(), window, cx))
                        .ok();
                }));
            }
        }
        menu
    }
}

/// The context menu of a collection header. `pins` lists the project's
/// environments and whether the collection is shown in each.
fn collection_menu(
    menu: PopupMenu,
    view: WeakEntity<Workspace>,
    root: PathBuf,
    pins: Option<(Vec<String>, Vec<bool>)>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let item =
        |label: &'static str,
         f: fn(&mut Workspace, PathBuf, &mut Window, &mut Context<Workspace>)| {
            let (view, root) = (view.clone(), root.clone());
            PopupMenuItem::new(label).on_click(move |_, window, cx| {
                view.update(cx, |this, cx| f(this, root.clone(), window, cx))
                    .ok();
            })
        };
    let mut menu = new_request_submenu(menu, view.clone(), root.clone(), window, cx)
        .item(item("New folder…", |this, root, window, cx| {
            this.new_folder(Some(root), window, cx)
        }))
        .item(item("Rename…", |this, root, window, cx| {
            this.rename_collection(root, window, cx)
        }))
        .item(item("Auth…", |this, root, window, cx| {
            this.open_auth_dialog(AuthOwner::Collection(root), window, cx)
        }));
    if let Some((envs, shown)) = pins.filter(|(envs, _)| !envs.is_empty()) {
        menu = menu.separator().label("Show in environment");
        for (env, is_shown) in envs.into_iter().zip(shown) {
            let (view, root) = (view.clone(), root.clone());
            menu = menu.item(PopupMenuItem::new(env.clone()).checked(is_shown).on_click(
                move |_, window, cx| {
                    view.update(cx, |this, cx| {
                        this.set_collection_shown(&root, &env, !is_shown, window, cx)
                    })
                    .ok();
                },
            ));
        }
    }
    menu.separator()
        .item(item("Move to trash", |this, root, window, cx| {
            this.confirm_delete(root, true, window, cx)
        }))
}

impl Workspace {
    /// Asks for a name and a parent folder, then creates and opens a project
    /// with Test and Prod environments.
    pub(super) fn prompt_new_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt_name(
            "New project",
            String::new(),
            "Choose folder…",
            window,
            cx,
            |_, name, window, cx| {
                let picked = cx.prompt_for_paths(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Create the project in".into()),
                });
                cx.spawn_in(window, async move |this, cx| {
                    if let Ok(Ok(Some(paths))) = picked.await
                        && let Some(parent) = paths.into_iter().next()
                    {
                        this.update_in(cx, |this, window, cx| {
                            match project::create_project(&parent, &name, &["test", "prod"]) {
                                Ok(root) => this.guard_unsaved(
                                    window,
                                    cx,
                                    Rc::new(move |this, window, cx| {
                                        this.open_collection_dir(&root, window, cx)
                                    }),
                                ),
                                Err(e) => this.notify_error(describe(&e), window, cx),
                            }
                        })
                        .ok();
                    }
                })
                .detach();
            },
        );
    }

    /// In a project, adds a collection to it; otherwise asks where to create one.
    pub(super) fn prompt_new_collection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let project_root = self.project.as_ref().map(|p| p.root.clone());
        let label = if project_root.is_some() {
            "Create"
        } else {
            "Choose folder…"
        };
        self.prompt_name(
            "New collection",
            String::new(),
            label,
            window,
            cx,
            move |this, name, window, cx| {
                if let Some(root) = &project_root {
                    match project::create_collection(root, &name) {
                        Ok(created) => {
                            this.reload(window, cx);
                            this.collection =
                                this.collections.iter().find(|c| c.root == created).cloned();
                            this.rebuild_vars(window, cx);
                            cx.notify();
                        }
                        Err(e) => this.notify_error(describe(&e), window, cx),
                    }
                    return;
                }
                let picked = cx.prompt_for_paths(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Create the collection in".into()),
                });
                cx.spawn_in(window, async move |this, cx| {
                    if let Ok(Ok(Some(paths))) = picked.await
                        && let Some(parent) = paths.into_iter().next()
                    {
                        this.update_in(cx, |this, window, cx| {
                            match project::create_collection(&parent, &name) {
                                Ok(root) => this.guard_unsaved(
                                    window,
                                    cx,
                                    Rc::new(move |this, window, cx| {
                                        this.open_collection_dir(&root, window, cx)
                                    }),
                                ),
                                Err(e) => this.notify_error(describe(&e), window, cx),
                            }
                        })
                        .ok();
                    }
                })
                .detach();
            },
        );
    }

    fn rename_collection(&mut self, root: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let current = self
            .collections
            .iter()
            .find(|c| c.root == root)
            .map(|c| c.meta.name.clone())
            .unwrap_or_default();
        self.prompt_name(
            "Rename collection",
            current,
            "Rename",
            window,
            cx,
            move |this, name, window, cx| {
                let result = Collection::open(&root).and_then(|mut c| {
                    c.meta.name = name.clone();
                    store::save_collection_meta(&root, &c.meta)
                });
                match result {
                    Ok(()) => this.reload(window, cx),
                    Err(e) => this.notify_error(describe(&e), window, cx),
                }
            },
        );
    }

    pub(super) fn set_group_by(
        &mut self,
        group_by: GroupBy,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = &mut self.project else {
            return;
        };
        project.meta.group_by = group_by;
        if let Err(e) = project.save() {
            self.notify_error(describe(&e), window, cx);
        }
        cx.notify();
    }

    fn set_collection_shown(
        &mut self,
        root: &Path,
        environment: &str,
        shown: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let all = self.environment_names();
        let Some(project) = &mut self.project else {
            return;
        };
        project.set_shown(root, environment, shown, &all);
        if let Err(e) = project.save() {
            self.notify_error(describe(&e), window, cx);
        }
        cx.notify();
    }
}

/// Watches the collection folder and reloads shortly after anything changes.
fn watch(root: &Path, window: &mut Window, cx: &mut Context<Workspace>) -> Option<Watcher> {
    use notify::Watcher as _;

    let changed = Arc::new(AtomicBool::new(false));
    let flag = changed.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if event.is_ok_and(|e| !matches!(e.kind, notify::EventKind::Access(_))) {
            flag.store(true, Ordering::Relaxed);
        }
    })
    .ok()?;
    watcher.watch(root, notify::RecursiveMode::Recursive).ok()?;

    // Changes are batched: one reload at most every 400 ms, however many files changed.
    let poll = cx.spawn_in(window, async move |this, cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(400))
                .await;
            if changed.swap(false, Ordering::Relaxed)
                && this
                    .update_in(cx, |this, window, cx| this.reload(window, cx))
                    .is_err()
            {
                break;
            }
        }
    });
    Some(Watcher {
        _watcher: watcher,
        _poll: poll,
    })
}

fn first_request(nodes: &[TreeNode]) -> Option<PathBuf> {
    nodes.iter().find_map(|node| match node {
        TreeNode::Request { path } => Some(path.clone()),
        TreeNode::Folder { children, .. } => first_request(children),
    })
}

fn request_paths(nodes: &[TreeNode], out: &mut Vec<PathBuf>) {
    for node in nodes {
        match node {
            TreeNode::Request { path } => out.push(path.clone()),
            TreeNode::Folder { children, .. } => request_paths(children, out),
        }
    }
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}
