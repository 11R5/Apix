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
use apix_core::vfs;
use apix_core::{Body, Collection, Method, Request};
use apix_import::Format;
use apix_import::postman::{self, Kind};
use apix_import::{postman_dump, postman_export};
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
use super::drag::{DraggedNode, Kind as DragKind, draggable};
use super::{Workspace, describe, key_hint};
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::theme;
use crate::{tr, tr_plain};

/// Keeps the folder watcher alive; dropping it stops watching.
pub(super) struct Watcher {
    _watcher: notify::RecommendedWatcher,
    _poll: Task<()>,
}

/// Something to do once unsaved changes are saved or discarded.
pub(super) type Then = Rc<dyn Fn(&mut Workspace, &mut Window, &mut Context<Workspace>)>;

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
                self.notify_error(tr!("collection.no_root", path = dir.display()), window, cx);
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
        self.tabs.clear();
        self.script_vars = Default::default();
        self.reload(window, cx);

        // Inside a project, start in the collection that was opened, if any.
        let chosen = collection.and_then(|c| self.collections.iter().position(|x| x.root == c));
        self.collection = self.collections.get(chosen.unwrap_or(0)).cloned();
        let first_of_collection = self
            .collection
            .as_ref()
            .and_then(|c| self.trees.iter().find(|(root, _)| *root == c.root))
            .and_then(|(_, tree)| first_request(tree));
        let restored = self.restore_tabs(&root, window, cx);
        // A collection opened by name inside a project comes to the front.
        let first = if self.project.is_some() && chosen.is_some() {
            first_of_collection.or(restored)
        } else {
            restored.or(first_of_collection)
        };
        match first {
            Some(path) => self.open_path(path, window, cx),
            None => {
                self.rebuild_vars(window, cx);
                self.show_request(Request::default(), window, cx);
            }
        }
    }

    /// Closes whatever is open (after its encrypted collection was locked).
    pub(super) fn close_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.project = None;
        self.collections.clear();
        self.collection = None;
        self.trees.clear();
        self.requests.clear();
        self.tabs.clear();
        self.selected = None;
        self.watcher = None;
        self.load_errors.clear();
        self.show_request(Request::default(), window, cx);
        self.refresh_environments(window, cx);
        cx.notify();
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
            paths.extend(TreeNode::request_paths(tree));
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

        self.sync_parked_tabs();
        if let Some(path) = self.selected.clone() {
            let dirty = self.is_dirty(cx);
            match self.requests.get(&path).cloned() {
                // Deleted or moved outside Apix: keep unsaved edits, otherwise close the tab.
                None if !dirty => self.forget_tabs(&path, window, cx),
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
    pub(super) fn refresh_environments(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        let items: Vec<SharedString> = std::iter::once(tr!("env.none"))
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
        // `pm.environment.set` belonged to the environment that was active.
        self.script_vars.environment.clear();
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
    pub(super) fn rebuild_vars(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        // What scripts set wins, as it would when sending.
        if !self.script_vars.is_empty() {
            self.vars = self.script_vars.apply(std::mem::take(&mut self.vars));
        }
    }

    /// Production environments get a red line under the title bar.
    pub(super) fn is_prod(&self) -> bool {
        self.environment
            .as_ref()
            .is_some_and(|name| name.to_lowercase().contains("prod"))
    }

    /// Opens a request in a tab (or brings its tab to the front). Unsaved
    /// edits stay in their own tab, so there is nothing to ask.
    pub(super) fn select(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.open_tab(path, window, cx);
    }

    fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.open_tab(path, window, cx);
    }

    /// Makes the collection that holds `path` the one whose variables apply.
    pub(super) fn set_active_collection(
        &mut self,
        path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        self.selected.is_some()
            && (self.build_request(cx) != self.base
                || self.script_editor.read(cx).scripts(cx) != self.base_scripts)
    }

    /// Runs `then` now, or after the user saves or discards unsaved edits.
    pub(super) fn guard_unsaved(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: Then,
    ) {
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
                .bidi_title(tr!("collection.unsaved_title"))
                .child(
                    div()
                        .text_sm()
                        .child(bidi::para(tr!("collection.unsaved_text", name = name))),
                )
                .footer(
                    h_flex()
                        .dir_row()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("unsaved-cancel")
                                .ghost()
                                .label(tr!("dialog.cancel"))
                                .on_click(move |_, window, cx| {
                                    close_dialog(&cancel_view, window, cx)
                                }),
                        )
                        .child(
                            Button::new("unsaved-discard")
                                .label(tr!("collection.dont_save"))
                                .on_click(move |_, window, cx| {
                                    close_dialog(&discard_view, window, cx);
                                    discard_view
                                        .update(cx, |this, cx| discard_then(this, window, cx))
                                        .ok();
                                }),
                        )
                        .child(
                            Button::new("unsaved-save")
                                .primary()
                                .label(tr!("dialog.save"))
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
                        tr!("collection.open_first", key = key_hint("O")),
                        window,
                        cx,
                    );
                    return false;
                };
                if request.name.is_empty() {
                    request.name = tr!("collection.new_request_name").into();
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
        let scripts = self.script_editor.read(cx).scripts(cx);
        if let Err(e) = scripts.write(&path) {
            self.notify_error(e.to_string(), window, cx);
            return false;
        }
        self.base_scripts = scripts;
        let created = self.selected.is_none();
        self.base = request.clone();
        self.requests.insert(path.clone(), request);
        if created {
            self.tabs.push(super::tabs::Tab::new(path.clone()));
        }
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
                tr!("collection.open_first", key = key_hint("O")),
                window,
                cx,
            );
            return;
        };
        // A new tab: whatever is open keeps its edits in its own tab.
        let body = match method {
            Method::Post | Method::Put | Method::Patch => Body::Json("{\n  \n}".into()),
            _ => Body::None,
        };
        let request = Request {
            name: tr_plain!("collection.new_request_name").into(),
            body,
            ..Request::new(method, "")
        };
        match store::create_request(&dir, &request) {
            Ok(path) => {
                self.collapsed.remove(&dir);
                self.reload(window, cx);
                self.open_path(path, window, cx);
            }
            Err(e) => self.notify_error(describe(&e), window, cx),
        }
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
            tr!("collection.new_folder_title"),
            String::new(),
            tr!("collection.create"),
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
            tr!("collection.rename_request_title"),
            current,
            tr!("collection.rename"),
            window,
            cx,
            move |this, name, window, cx| match store::rename_request(&path, &name) {
                Ok(new_path) => {
                    let front = this.selected.as_ref() == Some(&path);
                    this.retarget_tabs(&path, &new_path);
                    if front {
                        this.base.name = name;
                    } else {
                        this.rename_parked(&new_path, &name);
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
            tr!("collection.rename_folder_title"),
            current,
            tr!("collection.rename"),
            window,
            cx,
            move |this, name, window, cx| match store::rename_folder(&path, &name) {
                Ok(new_path) => {
                    this.retarget_tabs(&path, &new_path);
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
        // Encrypted collections have no trash: nothing decrypted may land on disk.
        let permanent = vfs::is_virtual(&path) && !vfs::is_encrypted(&path);
        window.open_alert_dialog(cx, move |alert: AlertDialog, _, _| {
            let (view, path) = (view.clone(), path.clone());
            let what = if folder {
                tr!("collection.delete_folder_title")
            } else {
                tr!("collection.delete_request_title")
            };
            let ok = if permanent {
                tr!("collection.delete")
            } else {
                tr!("sidebar.move_to_trash")
            };
            let detail = match (folder, permanent) {
                (true, true) => tr!("collection.delete_folder_permanent", name = name),
                (true, false) => tr!("collection.delete_folder_trash", name = name),
                (false, true) => tr!("collection.delete_request_permanent", name = name),
                (false, false) => tr!("collection.delete_request_trash", name = name),
            };
            alert
                .on_close(refocus(view.clone()))
                .confirm()
                .cancel_text(tr!("dialog.cancel"))
                .text_start()
                .title(what)
                .description(bidi::para(detail))
                .ok_text(ok)
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
        if let Err(message) = super::vault_ui::discard(&files) {
            self.notify_error(message, window, cx);
            return;
        }
        self.forget_tabs(&path, window, cx);
        self.reload(window, cx);
    }

    /// A small dialog with one text field, used for names.
    fn prompt_name(
        &mut self,
        title: SharedString,
        initial: String,
        ok_label: SharedString,
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
                .bidi_title(title.clone())
                .width(px(420.))
                .child(Input::new(&input))
                .on_ok(move |_, window, cx| on_enter(window, cx))
                .footer(
                    h_flex()
                        .dir_row()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("name-cancel")
                                .ghost()
                                .label(tr!("dialog.cancel"))
                                .on_click(move |_, window, cx| {
                                    close_dialog(&cancel_view, window, cx)
                                }),
                        )
                        .child(
                            Button::new("name-ok")
                                .primary()
                                .label(ok_label.clone())
                                .on_click(move |_, window, cx| {
                                    if confirm(window, cx) {
                                        window.close_dialog(cx);
                                    }
                                }),
                        ),
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
            prompt: Some(tr!("collection.open_prompt")),
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
        // A locked encrypted collection: ask for its password first.
        if let Some(root) = vfs::encrypted_root(&dir)
            && !vfs::is_unlocked(&root)
        {
            let open = root.clone();
            self.prompt_unlock(
                root,
                Rc::new(move |this, window, cx| this.open_or_create(open.clone(), window, cx)),
                window,
                cx,
            );
            return;
        }
        if Collection::find_root(&dir).is_some() || Project::find_root(&dir).is_some() {
            self.guard_all_tabs(
                window,
                cx,
                Rc::new(move |this, window, cx| this.open_collection_dir(&dir, window, cx)),
            );
            return;
        }
        // Not Apix yet: Postman exports in it are offered for import,
        // otherwise a new collection can start here.
        let scan_dir = dir.clone();
        let scan = cx.background_spawn(async move { postman_dump::scan(&scan_dir) });
        cx.spawn_in(window, async move |this, cx| {
            let found = scan.await.ok().filter(|f| !f.is_empty());
            this.update_in(cx, |this, window, cx| match found {
                Some(found) => this.offer_dump(dir, found, window, cx),
                None => this.offer_start_here(dir, window, cx),
            })
            .ok();
        })
        .detach();
    }

    /// Offers to make `dir` a new collection.
    fn offer_start_here(&mut self, dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert: AlertDialog, _, _| {
            let (view, dir) = (view.clone(), dir.clone());
            alert
                .on_close(refocus(view.clone()))
                .confirm()
                .cancel_text(tr!("dialog.cancel"))
                .text_start()
                .title(tr!("collection.start_here_title"))
                .description(bidi::para(tr!(
                    "collection.start_here_text",
                    path = dir.display()
                )))
                .ok_text(tr!("collection.create_collection"))
                .on_ok(move |_, window, cx| {
                    let meta = CollectionMeta {
                        name: dir
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| tr_plain!("collection.default_name").into()),
                        ..CollectionMeta::default()
                    };
                    view.update(cx, |this, cx| {
                        match store::save_collection_meta(&dir, &meta) {
                            Ok(()) => {
                                let dir = dir.clone();
                                this.guard_all_tabs(
                                    window,
                                    cx,
                                    Rc::new(move |this, window, cx| {
                                        this.open_collection_dir(&dir, window, cx)
                                    }),
                                );
                            }
                            Err(e) => this.notify_error(describe(&e), window, cx),
                        }
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
            prompt: Some(tr!("import.prompt")),
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
        let outside_project = self
            .project
            .as_ref()
            .is_some_and(|p| !path.starts_with(&p.root) && !p.root.starts_with(&path));
        let is_collection = path.join(store::COLLECTION_FILE).is_file() || vfs::is_encrypted(&path);
        if path.is_dir() && outside_project && is_collection {
            self.offer_move_into_project(path, window, cx);
        } else if path.is_dir() {
            self.open_or_create(path, window, cx);
        } else {
            self.import_file(path, window, cx);
        }
    }

    /// Reads every Postman export in a folder or "Export data" zip, then
    /// offers to import them as a project.
    fn import_dump(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let scan_path = path.clone();
        let scan = cx.background_spawn(async move { postman_dump::scan(&scan_path) });
        cx.spawn_in(window, async move |this, cx| {
            let result = scan.await;
            this.update_in(cx, |this, window, cx| match result {
                Ok(found) if found.is_empty() => this.notify_error(
                    tr!("import.nothing_found", path = path.display()),
                    window,
                    cx,
                ),
                Ok(found) => this.offer_dump(path, found, window, cx),
                Err(e) => this.notify_error(e.to_string(), window, cx),
            })
            .ok();
        })
        .detach();
    }

    /// "Postman export found: import it as a project?", then where to put it.
    fn offer_dump(
        &mut self,
        source: PathBuf,
        found: postman_dump::Found,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let found = Rc::new(found);
        let view = cx.entity().downgrade();
        let folder = source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| source.display().to_string());
        let text = tr!(
            "import.found_text",
            path = folder,
            collections = found.collections.len(),
            requests = found.request_count(),
            environments = found.environments.len() + found.globals.len()
        );
        window.open_alert_dialog(cx, move |alert: AlertDialog, _, _| {
            let (view, found, source) = (view.clone(), found.clone(), source.clone());
            alert
                .on_close(refocus(view.clone()))
                .confirm()
                .cancel_text(tr!("dialog.cancel"))
                .text_start()
                .title(tr!("import.found_title"))
                .description(bidi::para(text.clone()))
                .ok_text(tr!("import.found_ok"))
                .on_ok(move |_, window, cx| {
                    let (found, source) = (found.clone(), source.clone());
                    view.update(cx, |this, cx| {
                        this.write_dump(source, found, false, window, cx)
                    })
                    .ok();
                    true
                })
        });
    }

    /// Asks where to put `found`, writes it and opens it. `single` (one
    /// imported file) writes a lone collection as just that collection;
    /// otherwise everything becomes a project.
    fn write_dump(
        &mut self,
        source: PathBuf,
        found: Rc<postman_dump::Found>,
        single: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr!("import.project_in")),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(parent) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |this, window, cx| {
                let name = postman_dump::project_name(&source);
                let result = if single {
                    found.write(&parent, &name)
                } else {
                    postman_dump::write_project(&found, &parent, &name)
                };
                let written = match result {
                    Ok(written) => written,
                    Err(e) => return this.notify_error(e.to_string(), window, cx),
                };
                let mut message = tr!(
                    "import.project_summary",
                    collections = written.collections,
                    requests = written.requests,
                    environments = written.environments,
                    name = written.root.display()
                )
                .to_string();
                for note in &written.notes {
                    message.push_str("\n• ");
                    message.push_str(note);
                }
                let root = written.root.clone();
                this.guard_all_tabs(
                    window,
                    cx,
                    Rc::new(move |this, window, cx| this.open_collection_dir(&root, window, cx)),
                );
                window.push_notification(
                    Notification::success(message)
                        .title(tr!("import.project_done"))
                        .autohide(written.notes.is_empty()),
                    cx,
                );
            })
            .ok();
        })
        .detach();
    }

    /// Writes the collection at `root` as Postman files into a folder the
    /// user picks.
    pub(super) fn export_to_postman(
        &mut self,
        root: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let export = match postman_export::export_collection(&root, self.project.as_ref()) {
            Ok(export) => export,
            Err(e) => return self.notify_error(e.to_string(), window, cx),
        };
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr!("export.prompt")),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(dir) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |this, window, cx| match export.write(&dir) {
                Ok(written) => {
                    let mut message =
                        tr!("export.done", count = written.len(), path = dir.display()).to_string();
                    for note in &export.notes {
                        message.push_str("\n• ");
                        message.push_str(note);
                    }
                    window.push_notification(
                        Notification::success(message)
                            .title(tr!("export.done_title"))
                            .autohide(export.notes.is_empty()),
                        cx,
                    );
                }
                Err(e) => this.notify_error(e.to_string(), window, cx),
            })
            .ok();
        })
        .detach();
    }

    /// Export for the open collection (File menu, palette).
    pub(super) fn export_active_to_postman(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.collection.as_ref().map(|c| c.root.clone()) {
            Some(root) => self.export_to_postman(root, window, cx),
            None => self.notify_error(
                tr!("collection.open_first", key = key_hint("O")),
                window,
                cx,
            ),
        }
    }

    fn import_file(&mut self, file: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if file
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
        {
            return self.import_dump(file, window, cx);
        }
        let json = match std::fs::read_to_string(&file) {
            Ok(json) => json,
            Err(e) => {
                self.notify_error(format!("{}: {e}", file.display()), window, cx);
                return;
            }
        };
        match apix_import::detect(&json) {
            Ok(Some(Format::Postman(Kind::Collection))) => {
                self.import_collection(&json, window, cx)
            }
            Ok(Some(Format::Postman(Kind::Environment | Kind::Globals))) => {
                self.import_environment(&json, window, cx)
            }
            // Insomnia, OpenAPI, HAR: a collection, or a project when there are several.
            Ok(Some(_)) => {
                let name = postman_dump::project_name(&file);
                match apix_import::convert_named(&json, &name) {
                    Ok(Some(found)) => self.write_dump(file, Rc::new(found), true, window, cx),
                    Ok(None) => {}
                    Err(e) => self.notify_error(e.to_string(), window, cx),
                }
            }
            Ok(None) => self.notify_error(
                tr!("import.unknown_format", path = file.display()),
                window,
                cx,
            ),
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
            prompt: Some(tr!("collection.create_collection_in")),
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
                this.guard_all_tabs(
                    window,
                    cx,
                    Rc::new(move |this, window, cx| this.open_collection_dir(&root, window, cx)),
                );
                window.push_notification(
                    Notification::success(report.trim_end().to_owned())
                        .title(tr!("import.collection_done"))
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
            self.notify_error(tr!("import.environment_needs_open"), window, cx);
            return;
        };
        let result = postman::convert_environment(json).and_then(|env| {
            env.write(&root)?;
            Ok(env)
        });
        match result {
            Ok(env) => {
                let message = if env.secrets.is_empty() {
                    tr!(
                        "import.environment_saved",
                        title = env.title,
                        name = env.name,
                        count = env.vars.len()
                    )
                } else {
                    tr!(
                        "import.environment_saved_with_secrets",
                        title = env.title,
                        name = env.name,
                        count = env.vars.len(),
                        secrets = env.secrets.len()
                    )
                };
                window.push_notification(
                    Notification::success(message).title(tr!("import.environment_done")),
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
        let p = crate::theme::current();
        let open = !self.collections.is_empty() || self.project.is_some();
        let by_environment = self
            .project
            .as_ref()
            .is_some_and(|pr| pr.meta.group_by == GroupBy::Environment);
        let icon_button = |id: &'static str, icon: IconName, tooltip: SharedString| {
            Button::new(id).ghost().xsmall().icon(icon).tooltip(tooltip)
        };

        let (kind, title) = match (&self.project, self.collections.first()) {
            (Some(project), _) => (
                tr!("sidebar.kind_project"),
                SharedString::from(project.meta.name.clone()),
            ),
            (None, Some(c)) => (
                tr!("sidebar.kind_collection"),
                SharedString::from(c.meta.name.clone()),
            ),
            (None, None) => (SharedString::from("APIX"), tr!("sidebar.nothing_open")),
        };
        // Opening, creating and project settings live in the menu bar above.
        let header = v_flex()
            .px_3()
            .pt_2()
            .pb_1()
            .child(div().px_1().text_xs().text_color(p.text_muted).child(kind))
            .child(
                h_flex()
                    .dir_row()
                    .gap_1()
                    .items_center()
                    .h(px(28.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .px_1()
                            .truncate()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .when(open, |bar| {
                        bar.child(
                            icon_button(
                                "sidebar-new",
                                IconName::Plus,
                                tr!("sidebar.new_request_tooltip", key = key_hint("N")),
                            )
                            .dropdown_menu_with_anchor(
                                bidi::menu_anchor(),
                                {
                                    let view = cx.entity().downgrade();
                                    move |menu, _, _| method_items(menu, view.clone(), None)
                                },
                            ),
                        )
                        .child(
                            icon_button(
                                "sidebar-folder",
                                IconName::FolderClosed,
                                tr!("sidebar.new_folder"),
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
                .dir_row()
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
                        tr!("sidebar.edit_variables_tooltip", key = key_hint("E")),
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
            if let Some(project) = &self.project {
                for (ix, root) in project.locked_collections().into_iter().enumerate() {
                    rows.push(self.render_locked(ix, root, cx));
                }
            }
            if rows.is_empty() {
                let hint = if self.project.is_some() && self.collections.is_empty() {
                    tr!("sidebar.no_collections", item = tr!("menu.new_collection"))
                } else {
                    tr!("sidebar.no_requests", key = key_hint("N"))
                };
                rows.push(
                    div()
                        .px_4()
                        .py_2()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(bidi::para(hint))
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
            .border_e_1()
            .border_color(p.hairline)
            .child(header)
            .children(env_row)
            .child({
                let list = div()
                    .id("sidebar-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(body);
                // A single collection has no header row: its empty space is the root.
                match (&self.project, self.collections.first()) {
                    (None, Some(c)) => self.drop_target(list, c.root.clone(), cx),
                    _ => list,
                }
            })
            .children(self.load_errors.iter().map(|e| {
                div()
                    .px_4()
                    .py_1()
                    .text_xs()
                    .text_color(p.err)
                    .child(bidi::para(e.clone()))
            }))
    }

    /// Green for an active environment, red for production, grey for none.
    fn environment_dot(&self, name: Option<&str>) -> Hsla {
        let p = crate::theme::current();
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
        let p = crate::theme::current();
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
        let color_name = self
            .project
            .as_ref()
            .map(|project| project.color_of(&collection.root).map(str::to_owned));
        let color = color_name
            .clone()
            .flatten()
            .and_then(|name| theme::collection_color(&name));
        let header = row_base(&collection.root, depth);
        // Inside a project, collections can be dragged to reorder them.
        let header = if self.project.is_some() {
            draggable(
                header,
                DraggedNode::new(
                    &collection.root,
                    DragKind::Collection,
                    collection.meta.name.clone(),
                    p,
                ),
            )
        } else {
            header
        };
        out.push(
            self.drop_zones(header, &collection.root, DragKind::Collection, cx)
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
                        bidi::chevron_collapsed()
                    } else {
                        IconName::ChevronDown
                    })
                    .xsmall(),
                )
                .when_some(color, |row, color| {
                    row.child(
                        div()
                            .w(px(8.))
                            .h(px(8.))
                            .flex_none()
                            .rounded_full()
                            .bg(color),
                    )
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(collection.meta.name.clone()),
                )
                // Unlocked and encrypted: decrypted only in memory.
                .when(vfs::is_virtual(&collection.root), |row| {
                    row.child(
                        Icon::new(gpui_kit::assets::IconName::LockOpen)
                            .xsmall()
                            .text_color(p.ok),
                    )
                })
                .context_menu(move |menu: PopupMenu, window, cx| {
                    collection_menu(
                        menu,
                        view.clone(),
                        menu_root.clone(),
                        pins.clone(),
                        color_name.clone(),
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
                        .ps(px(8. + 14. * (depth + 1) as f32))
                        .py_1()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(tr!("sidebar.empty"))
                        .into_any_element(),
                );
            }
        }
    }

    /// Project → Environment → Collections. Clicking an environment makes it
    /// active and lists the collections shown in it.
    fn render_environment_groups(&self, out: &mut Vec<AnyElement>, cx: &mut Context<Self>) {
        let p = crate::theme::current();
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
                    .child(bidi::para(tr!("sidebar.no_environments")))
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
                    .dir_row()
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
                            menu.item(PopupMenuItem::new(tr!("sidebar.edit_variables")).on_click(
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
        let p = crate::theme::current();
        v_flex()
            .px_4()
            .pt_4()
            .gap_2()
            .child(
                Button::new("empty-open")
                    .small()
                    .icon(IconName::FolderOpen)
                    .label(tr!("sidebar.open_folder"))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.prompt_open_collection(window, cx)
                    })),
            )
            .child(
                Button::new("empty-import")
                    .small()
                    .icon(IconName::Inbox)
                    .label(tr!("sidebar.import_postman"))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.prompt_import(window, cx)
                    })),
            )
            .child(
                div()
                    .pt_1()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(bidi::para(tr!("sidebar.drop_hint"))),
            )
            .children(self.render_recent(cx))
    }

    /// Recently opened collections, shown when nothing is open.
    fn render_recent(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let p = crate::theme::current();
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
                        .child(tr!("sidebar.recent")),
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
        let p = crate::theme::current();
        let view = cx.entity().downgrade();
        let toggle_path = path.to_path_buf();
        let menu_path = path.to_path_buf();
        let row = draggable(
            row_base(path, depth),
            DraggedNode::new(path, DragKind::Folder, name.to_owned(), p),
        );
        self.drop_zones(row, path, DragKind::Folder, cx)
            .text_color(p.text_muted)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if !this.collapsed.remove(&toggle_path) {
                    this.collapsed.insert(toggle_path.clone());
                }
                cx.notify();
            }))
            .child(
                Icon::new(if collapsed {
                    bidi::chevron_collapsed()
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
        let p = crate::theme::current();
        let selected = self.selected.as_deref() == Some(path);
        let dirty = self.path_dirty(path, cx);
        // The open request shows the editor's method, so the badge follows edits.
        let method = if selected {
            self.current_method(cx)
        } else {
            request.method
        };
        let view = cx.entity().downgrade();
        let open_path = path.to_path_buf();
        let menu_path = path.to_path_buf();
        let row = draggable(
            row_base(path, depth),
            DraggedNode::new(path, DragKind::Request, request.name.clone(), p),
        );
        self.drop_zones(row, path, DragKind::Request, cx)
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
            .context_menu(move |menu: PopupMenu, window, cx| {
                request_menu(menu, view.clone(), menu_path.clone(), window, cx)
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
        .dir_row()
        .items_center()
        .gap_2()
        .ps(px(8. + 14. * depth as f32))
        .pe(px(8.))
        .py_1()
        .rounded_md()
        .text_sm()
        .cursor_pointer()
        .hover(|s| s.bg(hsla(0., 0., 1., 0.05)))
}

fn request_menu(
    menu: PopupMenu,
    view: WeakEntity<Workspace>,
    path: PathBuf,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let item =
        |label: SharedString,
         f: fn(&mut Workspace, PathBuf, &mut Window, &mut Context<Workspace>)| {
            let (view, path) = (view.clone(), path.clone());
            PopupMenuItem::new(label).on_click(move |_, window, cx| {
                view.update(cx, |this, cx| f(this, path.clone(), window, cx))
                    .ok();
            })
        };
    let menu = menu
        .item(item(tr!("sidebar.rename"), |this, path, window, cx| {
            this.rename_request(path, window, cx)
        }))
        .item(item(tr!("sidebar.duplicate"), |this, path, window, cx| {
            this.duplicate(path, window, cx)
        }))
        .separator();
    let (copy_view, copy_path) = (view.clone(), path.clone());
    let copy = move |format: apix_codegen::Format, label: SharedString| {
        let (view, path) = (copy_view.clone(), copy_path.clone());
        PopupMenuItem::new(label).on_click(move |_, window, cx| {
            view.update(cx, |this, cx| {
                this.copy_saved_as(path.clone(), format, window, cx)
            })
            .ok();
        })
    };
    let menu = apix_codegen::Shell::ALL
        .into_iter()
        .map(apix_codegen::Format::Shell)
        .fold(menu, |menu, format| {
            menu.item(copy(
                format,
                tr!("sidebar.copy_as_shell", shell = format.label()),
            ))
        });
    let menu = menu.submenu(
        tr!("sidebar.copy_as_code"),
        window,
        cx,
        move |menu, _, _| {
            apix_codegen::Lang::ALL
                .into_iter()
                .map(apix_codegen::Format::Code)
                .fold(menu, |menu, format| {
                    menu.item(copy(format, format.label().into()))
                })
        },
    );
    menu.separator().item(item(
        tr!("sidebar.move_to_trash"),
        |this, path, window, cx| this.confirm_delete(path, false, window, cx),
    ))
}

/// One item per HTTP method; picking one creates a request with it in `dir`.
pub(super) fn method_items(
    menu: PopupMenu,
    view: WeakEntity<Workspace>,
    dir: Option<PathBuf>,
) -> PopupMenu {
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
    menu.submenu(tr!("menu.new_request"), window, cx, move |menu, _, _| {
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
        |label: SharedString,
         f: fn(&mut Workspace, PathBuf, &mut Window, &mut Context<Workspace>)| {
            let (view, path) = (view.clone(), path.clone());
            PopupMenuItem::new(label).on_click(move |_, window, cx| {
                view.update(cx, |this, cx| f(this, path.clone(), window, cx))
                    .ok();
            })
        };
    new_request_submenu(menu, view.clone(), path.clone(), window, cx)
        .item(item(tr!("menu.new_folder"), |this, path, window, cx| {
            this.new_folder(Some(path), window, cx)
        }))
        .item(item(tr!("sidebar.rename"), |this, path, window, cx| {
            this.rename_folder(path, window, cx)
        }))
        .item(item(tr!("sidebar.auth"), |this, path, window, cx| {
            this.open_auth_dialog(AuthOwner::Folder(path), window, cx)
        }))
        .item(item(tr!("sidebar.settings"), |this, path, window, cx| {
            this.open_settings_dialog(AuthOwner::Folder(path), window, cx)
        }))
        .item(item(tr!("sidebar.run"), |this, path, window, cx| {
            this.run_folder(path, window, cx)
        }))
        .separator()
        .item(item(
            tr!("sidebar.move_to_trash"),
            |this, path, window, cx| this.confirm_delete(path, true, window, cx),
        ))
}

/// The context menu of a collection header. `pins` lists the project's
/// environments and whether the collection is shown in each.
fn collection_menu(
    menu: PopupMenu,
    view: WeakEntity<Workspace>,
    root: PathBuf,
    pins: Option<(Vec<String>, Vec<bool>)>,
    color: Option<Option<String>>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let item =
        |label: SharedString,
         f: fn(&mut Workspace, PathBuf, &mut Window, &mut Context<Workspace>)| {
            let (view, root) = (view.clone(), root.clone());
            PopupMenuItem::new(label).on_click(move |_, window, cx| {
                view.update(cx, |this, cx| f(this, root.clone(), window, cx))
                    .ok();
            })
        };
    let mut menu = new_request_submenu(menu, view.clone(), root.clone(), window, cx)
        .item(item(tr!("menu.new_folder"), |this, root, window, cx| {
            this.new_folder(Some(root), window, cx)
        }))
        .item(item(tr!("sidebar.rename"), |this, root, window, cx| {
            this.rename_collection(root, window, cx)
        }))
        .item(item(tr!("sidebar.auth"), |this, root, window, cx| {
            this.open_auth_dialog(AuthOwner::Collection(root), window, cx)
        }))
        .item(item(tr!("sidebar.settings"), |this, root, window, cx| {
            this.open_settings_dialog(AuthOwner::Collection(root), window, cx)
        }))
        .item(item(tr!("sidebar.run"), |this, root, window, cx| {
            this.run_folder(root, window, cx)
        }))
        .item(item(
            tr!("menu.export_postman"),
            |this, root, window, cx| this.export_to_postman(root, window, cx),
        ))
        .separator();
    menu = if vfs::is_virtual(&root) {
        menu.item(item(tr!("sidebar.lock"), |this, _, window, cx| {
            this.lock_collections(window, cx)
        }))
        .item(item(
            tr!("sidebar.change_password"),
            |this, root, window, cx| this.prompt_change_password(root, window, cx),
        ))
        .item(item(
            tr!("sidebar.remove_encryption"),
            |this, root, window, cx| this.confirm_decrypt(root, window, cx),
        ))
        .when(vfs::keychain::load(&root).is_some(), |menu| {
            menu.item(item(
                tr!("sidebar.forget_password"),
                |this, root, window, cx| this.forget_saved_password(root, window, cx),
            ))
        })
    } else {
        menu.item(item(tr!("sidebar.encrypt"), |this, root, window, cx| {
            this.prompt_encrypt(root, window, cx)
        }))
    };
    if let Some((envs, shown)) = pins.filter(|(envs, _)| !envs.is_empty()) {
        menu = menu.separator().label(tr!("sidebar.show_in_environment"));
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
    if let Some(current) = color {
        let (view, root) = (view.clone(), root.clone());
        menu = menu
            .separator()
            .submenu(tr!("sidebar.color"), window, cx, move |menu, _, _| {
                let pick = |label: SharedString, name: Option<&'static str>| {
                    let (view, root) = (view.clone(), root.clone());
                    PopupMenuItem::new(label)
                        .checked(current.as_deref() == name)
                        .on_click(move |_, window, cx| {
                            view.update(cx, |this, cx| {
                                this.set_collection_color(&root, name, window, cx)
                            })
                            .ok();
                        })
                };
                let mut menu = menu.item(pick(tr!("color.none"), None));
                for name in theme::COLLECTION_COLORS {
                    menu = menu.item(pick(color_label(name), Some(name)));
                }
                menu
            });
    }
    menu.separator().item(item(
        tr!("sidebar.move_to_trash"),
        |this, root, window, cx| this.confirm_delete(root, true, window, cx),
    ))
}

fn color_label(name: &str) -> SharedString {
    match name {
        "red" => tr!("color.red"),
        "orange" => tr!("color.orange"),
        "yellow" => tr!("color.yellow"),
        "green" => tr!("color.green"),
        "teal" => tr!("color.teal"),
        "blue" => tr!("color.blue"),
        "purple" => tr!("color.purple"),
        _ => tr!("color.pink"),
    }
}

impl Workspace {
    /// Asks for a name and a parent folder, then creates and opens a project
    /// with Test and Prod environments.
    pub(super) fn prompt_new_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt_name(
            tr!("collection.new_project_title"),
            String::new(),
            tr!("collection.choose_folder"),
            window,
            cx,
            |_, name, window, cx| {
                let picked = cx.prompt_for_paths(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some(tr!("collection.create_project_in")),
                });
                cx.spawn_in(window, async move |this, cx| {
                    if let Ok(Ok(Some(paths))) = picked.await
                        && let Some(parent) = paths.into_iter().next()
                    {
                        this.update_in(cx, |this, window, cx| {
                            match project::create_project(&parent, &name, &["test", "prod"]) {
                                Ok(root) => this.guard_all_tabs(
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

    /// File › Move into a project…: picks a project folder and moves the open
    /// collection there, then opens the project on it.
    pub(super) fn prompt_move_into_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(collection) = self.collections.first().map(|c| c.root.clone()) else {
            return;
        };
        if self.project.is_some() {
            return;
        }
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr!("project.move_in")),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await
                && let Some(picked) = paths.into_iter().next()
            {
                this.update_in(cx, |this, window, cx| {
                    let Some(project_root) = Project::find_root(&picked) else {
                        this.notify_error(
                            tr!("project.not_a_project", path = picked.display()),
                            window,
                            cx,
                        );
                        return;
                    };
                    let collection = collection.clone();
                    this.guard_all_tabs(
                        window,
                        cx,
                        Rc::new(move |this, window, cx| {
                            match project::move_collection_into(&project_root, &collection) {
                                Ok(moved) => this.open_collection_dir(&moved, window, cx),
                                Err(e) => this.notify_error(describe(&e), window, cx),
                            }
                        }),
                    );
                })
                .ok();
            }
        })
        .detach();
    }

    /// A collection folder dropped onto an open project: offers to move it in.
    fn offer_move_into_project(
        &mut self,
        collection: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.project.as_ref() else {
            return;
        };
        let (project_root, project_name) = (project.root.clone(), project.meta.name.clone());
        let name = Collection::open(&collection)
            .map(|c| c.meta.name)
            .unwrap_or_else(|_| {
                collection
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert: AlertDialog, _, _| {
            let (view, project_root, collection) =
                (view.clone(), project_root.clone(), collection.clone());
            alert
                .on_close(refocus(view.clone()))
                .confirm()
                .cancel_text(tr!("dialog.cancel"))
                .text_start()
                .title(tr!("project.move_in_title"))
                .description(bidi::para(tr!(
                    "project.move_in_detail",
                    collection = name,
                    project = project_name,
                    path = collection.display()
                )))
                .ok_text(tr!("project.move_in"))
                .on_ok(move |_, window, cx| {
                    view.update(cx, |this, cx| {
                        match project::move_collection_into(&project_root, &collection) {
                            Ok(_) => this.reload(window, cx),
                            Err(e) => this.notify_error(describe(&e), window, cx),
                        }
                    })
                    .ok();
                    true
                })
        });
    }

    /// In a project, adds a collection to it; otherwise asks where to create one.
    pub(super) fn prompt_new_collection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let project_root = self.project.as_ref().map(|p| p.root.clone());
        let label = if project_root.is_some() {
            tr!("collection.create")
        } else {
            tr!("collection.choose_folder")
        };
        self.prompt_name(
            tr!("collection.new_collection_title"),
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
                    prompt: Some(tr!("collection.create_collection_in")),
                });
                cx.spawn_in(window, async move |this, cx| {
                    if let Ok(Ok(Some(paths))) = picked.await
                        && let Some(parent) = paths.into_iter().next()
                    {
                        this.update_in(cx, |this, window, cx| {
                            match project::create_collection(&parent, &name) {
                                Ok(root) => this.guard_all_tabs(
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
            tr!("collection.rename_collection_title"),
            current,
            tr!("collection.rename"),
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

    fn set_collection_color(
        &mut self,
        root: &Path,
        color: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = &mut self.project else {
            return;
        };
        project.set_color(root, color);
        if let Err(e) = project.save() {
            self.notify_error(describe(&e), window, cx);
        }
        cx.notify();
    }

    /// The colour of the project collection that holds `path`, if it has one.
    pub(super) fn collection_color(&self, path: &Path) -> Option<Hsla> {
        let project = self.project.as_ref()?;
        let collection = self.collection_for(path)?;
        theme::collection_color(project.color_of(&collection.root)?)
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
                    .update_in(cx, |this, window, cx| {
                        // An encrypted collection changed on disk (a git pull): re-read its index.
                        for root in vfs::unlocked_roots() {
                            vfs::refresh(&root).ok();
                        }
                        this.reload(window, cx)
                    })
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

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}
