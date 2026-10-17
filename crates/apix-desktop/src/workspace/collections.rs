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

use apix_core::store::{self, CollectionMeta, TreeNode};
use apix_core::{Collection, Method, Request, Variables};
use apix_import::postman::{self, Kind};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::AlertDialog;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Icon, IconName, Sizable, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{Workspace, describe, key_hint};
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

    pub(super) fn open_collection_dir(
        &mut self,
        dir: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(root) = Collection::find_root(dir) else {
            self.notify_error(
                format!("No apix.toml in {} or its parents.", dir.display()),
                window,
                cx,
            );
            return;
        };
        match Collection::open(&root) {
            Ok(collection) => {
                self.vars = Variables::new().with_layer(&collection.meta.vars);
                self.collection = Some(collection);
                self.watcher = watch(&root, window, cx);
                self.collapsed.clear();
                self.selected = None;
                self.reload(window, cx);
                match first_request(&self.tree) {
                    Some(path) => self.open_path(path, window, cx),
                    None => self.show_request(Request::default(), window, cx),
                }
            }
            Err(e) => self.notify_error(describe(&e), window, cx),
        }
    }

    /// Re-reads the collection from disk. If the open request changed on
    /// disk and has no unsaved edits, the editor follows the file.
    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(collection) = &mut self.collection else {
            return;
        };
        if let Ok(fresh) = Collection::open(&collection.root) {
            self.vars = Variables::new().with_layer(&fresh.meta.vars);
            *collection = fresh;
        }
        let root = collection.root.clone();
        self.tree = collection.tree();
        self.requests.clear();
        self.load_errors.clear();
        let mut paths = Vec::new();
        request_paths(&self.tree, &mut paths);
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

    /// Opens a request in the editor, asking first if the current one has unsaved edits.
    fn select(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
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
        self.selected = Some(path);
        self.show_request(request, window, cx);
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

    pub(super) fn new_request(
        &mut self,
        dir: Option<PathBuf>,
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
                let request = Request {
                    name: "New request".into(),
                    ..Request::new(Method::Get, "")
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

    fn new_folder(&mut self, parent: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
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
    fn open_or_create(&mut self, dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
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
        let Some(root) = self.collection.as_ref().map(|c| c.root.clone()) else {
            self.notify_error(
                "Open the collection this environment belongs to first, then import it.",
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

    fn notify_error(&self, message: impl Into<String>, window: &mut Window, cx: &mut App) {
        window.push_notification(Notification::error(message.into()), cx);
    }

    // -----------------------------------------------------------------------
    // Sidebar
    // -----------------------------------------------------------------------

    pub(super) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let has_collection = self.collection.is_some();
        let icon_button = |id: &'static str, icon: IconName, tooltip: String| {
            Button::new(id).ghost().xsmall().icon(icon).tooltip(tooltip)
        };

        let header = h_flex()
            .px_3()
            .pt_3()
            .pb_1()
            .gap_1()
            .items_center()
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child("COLLECTION"),
            )
            .when(has_collection, |bar| {
                bar.child(
                    icon_button(
                        "sidebar-new",
                        IconName::Plus,
                        format!("New request ({})", key_hint("N")),
                    )
                    .on_click(cx.listener(
                        |this, _: &ClickEvent, window, cx| this.new_request(None, window, cx),
                    )),
                )
                .child(
                    icon_button(
                        "sidebar-folder",
                        IconName::FolderClosed,
                        "New folder".into(),
                    )
                    .on_click(cx.listener(
                        |this, _: &ClickEvent, window, cx| this.new_folder(None, window, cx),
                    )),
                )
            })
            .child(
                icon_button(
                    "sidebar-import",
                    IconName::Inbox,
                    "Import from Postman".into(),
                )
                .on_click(
                    cx.listener(|this, _: &ClickEvent, window, cx| this.prompt_import(window, cx)),
                ),
            )
            .child(
                icon_button(
                    "sidebar-open",
                    IconName::FolderOpen,
                    format!("Open collection ({})", key_hint("O")),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.prompt_open_collection(window, cx)
                })),
            );

        let body = if has_collection {
            let mut rows = Vec::new();
            self.render_nodes(&self.tree, 0, &mut rows, cx);
            if rows.is_empty() {
                rows.push(
                    div()
                        .px_4()
                        .py_2()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(format!("No requests yet. {} creates one.", key_hint("N")))
                        .into_any_element(),
                );
            }
            v_flex()
                .gap_px()
                .px_2()
                .pb_2()
                .children(rows)
                .into_any_element()
        } else {
            self.render_empty_state(cx).into_any_element()
        };

        v_flex()
            .w(px(248.))
            .flex_none()
            .h_full()
            .border_r_1()
            .border_color(p.hairline)
            .child(header)
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
            .context_menu(move |menu: PopupMenu, _, _| {
                folder_menu(menu, view.clone(), menu_path.clone())
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
fn close_dialog(view: &WeakEntity<Workspace>, window: &mut Window, cx: &mut App) {
    window.close_dialog(cx);
    view.update(cx, |this, cx| this.focus.focus(window, cx))
        .ok();
}

/// For `on_close`: however a dialog is dismissed (Esc, ×, overlay), focus returns to the workspace.
fn refocus(view: WeakEntity<Workspace>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
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
    menu.item(item("Rename…", |this, path, window, cx| {
        this.rename_request(path, window, cx)
    }))
    .item(item("Duplicate", |this, path, window, cx| {
        this.duplicate(path, window, cx)
    }))
    .separator()
    .item(item("Move to trash", |this, path, window, cx| {
        this.confirm_delete(path, false, window, cx)
    }))
}

fn folder_menu(menu: PopupMenu, view: WeakEntity<Workspace>, path: PathBuf) -> PopupMenu {
    let item =
        |label: &'static str,
         f: fn(&mut Workspace, PathBuf, &mut Window, &mut Context<Workspace>)| {
            let (view, path) = (view.clone(), path.clone());
            PopupMenuItem::new(label).on_click(move |_, window, cx| {
                view.update(cx, |this, cx| f(this, path.clone(), window, cx))
                    .ok();
            })
        };
    menu.item(item("New request", |this, path, window, cx| {
        this.new_request(Some(path), window, cx)
    }))
    .item(item("New folder…", |this, path, window, cx| {
        this.new_folder(Some(path), window, cx)
    }))
    .item(item("Rename…", |this, path, window, cx| {
        this.rename_folder(path, window, cx)
    }))
    .separator()
    .item(item("Move to trash", |this, path, window, cx| {
        this.confirm_delete(path, true, window, cx)
    }))
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
