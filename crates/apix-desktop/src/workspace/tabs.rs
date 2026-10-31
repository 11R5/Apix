//! Tabs: several requests open at once. The front tab lives in the editor
//! widgets; a tab in the back keeps its unsaved draft, its last response and
//! even a request still in flight, so switching never loses anything.

use std::mem;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use apix_core::{Method, Request};
use gpui_kit::component::h_flex;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::script_editor::Scripts;
use super::{Finished, ResponseState, Workspace};
use crate::bidi::{self, Bidi as _};
use crate::large_view::Document;
use crate::state::OpenTabs;
use crate::theme;

/// One open request.
pub(super) struct Tab {
    pub(super) path: PathBuf,
    /// `None` while the tab is in front.
    parked: Option<Parked>,
}

/// What a tab in the back remembers.
struct Parked {
    base: Request,
    draft: Request,
    base_scripts: Scripts,
    scripts: Scripts,
    request_tab: usize,
    response_tab: usize,
    response: ResponseState,
    raw_doc: Option<Arc<Document>>,
    /// A request still in flight; it finishes into this tab.
    task: Option<Task<()>>,
}

impl Tab {
    pub(super) fn new(path: PathBuf) -> Self {
        Self { path, parked: None }
    }
}

impl Workspace {
    /// Writes the open tabs to the app state when they changed. Tabs of
    /// encrypted collections are left out: their file names are secret too.
    pub(super) fn remember_tabs(&mut self) {
        let Some(root) = self.workspace_root() else {
            return;
        };
        // A request of an encrypted collection is open only while it is unlocked.
        let plain = |p: &&PathBuf| !apix_core::vfs::is_virtual(p);
        let open = OpenTabs {
            paths: self
                .tabs
                .iter()
                .map(|t| &t.path)
                .filter(plain)
                .cloned()
                .collect(),
            front: self.selected.as_ref().filter(plain).cloned(),
        };
        let current = (root, open);
        if self.remembered_tabs.as_ref() != Some(&current) {
            self.app_state.set_tabs(&current.0, current.1.clone());
            self.remembered_tabs = Some(current);
        }
    }

    /// Reopens the tabs remembered for `root`. Returns the one put in front,
    /// or `None` when there was nothing left to reopen.
    pub(super) fn restore_tabs(
        &mut self,
        root: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<PathBuf> {
        let saved = self.app_state.tabs_for(root)?.clone();
        // The loaded request's own path: `Path` equality ignores a trailing
        // separator, but the file system does not.
        let paths: Vec<PathBuf> = saved
            .paths
            .iter()
            .filter_map(|p| self.requests.get_key_value(p).map(|(k, _)| k.clone()))
            .collect();
        let front = saved
            .front
            .and_then(|f| paths.iter().find(|p| **p == f).cloned())
            .or_else(|| paths.last().cloned())?;
        for path in paths {
            self.open_tab(path, window, cx);
        }
        Some(front)
    }

    fn tab_index(&self, path: &Path) -> Option<usize> {
        self.tabs.iter().position(|t| t.path == path)
    }

    /// Brings `path` to the front, opening a tab for it if needed.
    pub(super) fn open_tab(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected.as_ref() == Some(&path) {
            return;
        }
        let Some(saved) = self.requests.get(&path).cloned() else {
            return;
        };
        let at = self.selected.as_deref().and_then(|p| self.tab_index(p));
        self.park_front(cx);
        let parked = match self.tab_index(&path) {
            Some(ix) => self.tabs[ix].parked.take(),
            None => {
                let ix = at.map_or(self.tabs.len(), |ix| ix + 1);
                self.tabs.insert(ix, Tab::new(path.clone()));
                None
            }
        };
        self.set_active_collection(&path, window, cx);
        self.selected = Some(path);
        match parked {
            Some(parked) => self.unpark(parked, window, cx),
            None => self.show_request(saved, window, cx),
        }
        self.url.update(cx, |state, cx| state.focus(window, cx));
    }

    /// Moves the front tab's state into its `Tab`.
    pub(super) fn park_front(&mut self, cx: &mut Context<Self>) {
        let Some(ix) = self.selected.as_deref().and_then(|p| self.tab_index(p)) else {
            return;
        };
        let parked = Parked {
            base: self.base.clone(),
            draft: self.build_request(cx),
            base_scripts: self.base_scripts.clone(),
            scripts: self.script_editor.read(cx).scripts(cx),
            request_tab: self.request_tab,
            response_tab: self.response_tab,
            response: mem::replace(&mut self.response, ResponseState::Idle),
            raw_doc: self.raw_doc.take(),
            task: self.task.take(),
        };
        self.tabs[ix].parked = Some(parked);
        self.large_view = None;
        self.raw_task = None;
        self.hex_doc = None;
        self.hex_task = None;
        self.hovered_var = None;
    }

    fn unpark(&mut self, parked: Parked, window: &mut Window, cx: &mut Context<Self>) {
        self.fill_editor(&parked.draft, window, cx);
        self.base = parked.base;
        self.show_scripts(&parked.scripts, window, cx);
        self.base_scripts = parked.base_scripts;
        self.request_tab = parked.request_tab;
        self.response = parked.response;
        self.raw_doc = parked.raw_doc;
        self.task = parked.task;
        self.set_response_tab(parked.response_tab, window, cx);
        cx.notify();
    }

    /// A response that arrived for `path`: into the editor if it is in
    /// front, otherwise into its tab.
    pub(super) fn finish_for(
        &mut self,
        path: Option<PathBuf>,
        result: Result<Finished, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if path == self.selected {
            return self.finish(result, window, cx);
        }
        let parked = path
            .and_then(|p| self.tab_index(&p))
            .and_then(|ix| self.tabs[ix].parked.as_mut());
        if let Some(parked) = parked {
            parked.task = None;
            parked.raw_doc = None;
            parked.response_tab = 0;
            parked.response = match result {
                Ok(finished) => ResponseState::Done(Rc::new(finished)),
                Err(message) => ResponseState::Failed(message),
            };
            cx.notify();
        }
    }

    /// Whether the request at `path` has unsaved edits in its tab.
    pub(super) fn path_dirty(&self, path: &Path, cx: &App) -> bool {
        self.tab_index(path)
            .is_some_and(|ix| self.tab_dirty(&self.tabs[ix], cx))
    }

    fn tab_dirty(&self, tab: &Tab, cx: &App) -> bool {
        match &tab.parked {
            Some(parked) => parked.draft != parked.base || parked.scripts != parked.base_scripts,
            None => self.is_dirty(cx),
        }
    }

    /// Closes the tab of `path`, asking first if it has unsaved changes.
    pub(super) fn close_tab(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.tab_index(&path) else {
            return;
        };
        if self.selected.as_ref() != Some(&path) {
            if !self.tab_dirty(&self.tabs[ix], cx) {
                self.tabs.remove(ix);
                cx.notify();
                return;
            }
            // Show it, so the question is about what is on screen.
            self.open_tab(path.clone(), window, cx);
        }
        self.guard_unsaved(
            window,
            cx,
            Rc::new(move |this, window, cx| this.drop_front_tab(window, cx)),
        );
    }

    pub(super) fn close_front_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self.selected.clone() {
            self.close_tab(path, window, cx);
        }
    }

    /// Removes the front tab without asking and shows its neighbour.
    fn drop_front_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.selected.as_deref().and_then(|p| self.tab_index(p)) else {
            return;
        };
        self.tabs.remove(ix);
        self.selected = None;
        self.show_request(Request::default(), window, cx);
        if let Some(next) = self.tabs.get(ix.min(self.tabs.len().saturating_sub(1))) {
            let next = next.path.clone();
            self.open_tab(next, window, cx);
        }
        cx.notify();
    }

    /// Ctrl+Tab / Ctrl+Shift+Tab.
    pub(super) fn cycle_tab(&mut self, step: isize, window: &mut Window, cx: &mut Context<Self>) {
        let len = self.tabs.len() as isize;
        let Some(ix) = self.selected.as_deref().and_then(|p| self.tab_index(p)) else {
            return;
        };
        if len < 2 {
            return;
        }
        let next = (ix as isize + step).rem_euclid(len) as usize;
        let path = self.tabs[next].path.clone();
        self.open_tab(path, window, cx);
    }

    /// After a rename or move on disk: tabs under `old` follow to `new`.
    pub(super) fn retarget_tabs(&mut self, old: &Path, new: &Path) {
        // `join("")` would add a trailing separator to the moved file itself.
        let follow = |p: &Path| {
            p.strip_prefix(old).ok().map(|rest| {
                if rest.as_os_str().is_empty() {
                    new.to_path_buf()
                } else {
                    new.join(rest)
                }
            })
        };
        for tab in &mut self.tabs {
            if let Some(moved) = follow(&tab.path) {
                tab.path = moved;
            }
        }
        if let Some(moved) = self.selected.as_deref().and_then(follow) {
            self.selected = Some(moved);
        }
    }

    /// A request in the back was renamed: its tab shows the new name.
    pub(super) fn rename_parked(&mut self, path: &Path, name: &str) {
        if let Some(parked) = self
            .tab_index(path)
            .and_then(|ix| self.tabs[ix].parked.as_mut())
        {
            parked.base.name = name.to_owned();
            parked.draft.name = name.to_owned();
        }
    }

    /// After a delete: tabs under `path` close without asking.
    pub(super) fn forget_tabs(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let front = self.selected.clone();
        self.tabs
            .retain(|t| !t.path.starts_with(path) || Some(&t.path) == front.as_ref());
        if front.is_some_and(|s| s.starts_with(path)) {
            self.drop_front_tab(window, cx);
        }
    }

    /// After a reload: tabs whose file is gone close unless they hold edits,
    /// and clean tabs in the back pick up changes made outside Apix.
    pub(super) fn sync_parked_tabs(&mut self) {
        let requests = &self.requests;
        self.tabs.retain_mut(|tab| {
            let Some(parked) = tab.parked.as_mut() else {
                return true;
            };
            let clean = parked.draft == parked.base;
            match requests.get(&tab.path) {
                None => !clean,
                Some(on_disk) if clean && *on_disk != parked.base => {
                    parked.base = on_disk.clone();
                    parked.draft = on_disk.clone();
                    true
                }
                Some(_) => true,
            }
        });
    }

    /// Asks about every tab with unsaved changes, one by one, then runs `then`
    /// (used before opening another project or collection).
    pub(super) fn guard_all_tabs(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: super::collections::Then,
    ) {
        let dirty = self
            .tabs
            .iter()
            .find(|t| self.tab_dirty(t, cx))
            .map(|t| t.path.clone());
        let Some(path) = dirty else {
            return then(self, window, cx);
        };
        self.open_tab(path, window, cx);
        self.guard_unsaved(
            window,
            cx,
            Rc::new(move |this, window, cx| {
                // Saved or discarded: either way this tab is settled now.
                let base = this.base.clone();
                this.show_request(base, window, cx);
                this.guard_all_tabs(window, cx, then.clone());
            }),
        );
    }

    /// Like `guard_all_tabs`, for the tabs inside `roots` only (before locking them).
    pub(super) fn guard_tabs_under(
        &mut self,
        roots: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: super::collections::Then,
    ) {
        let dirty = self
            .tabs
            .iter()
            .find(|t| roots.iter().any(|r| t.path.starts_with(r)) && self.tab_dirty(t, cx))
            .map(|t| t.path.clone());
        let Some(path) = dirty else {
            return then(self, window, cx);
        };
        self.open_tab(path, window, cx);
        self.guard_unsaved(
            window,
            cx,
            Rc::new(move |this, window, cx| {
                let base = this.base.clone();
                this.show_request(base, window, cx);
                this.guard_tabs_under(roots.clone(), window, cx, then.clone());
            }),
        );
    }

    /// Whether a tab inside `root` has unsaved edits.
    pub(super) fn has_dirty_tab_under(&self, root: &Path, cx: &App) -> bool {
        self.tabs
            .iter()
            .any(|t| t.path.starts_with(root) && self.tab_dirty(t, cx))
    }

    pub(super) fn render_tabs(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if self.tabs.is_empty() {
            return None;
        }
        let p = crate::theme::current();
        let front_method = self.current_method(cx);
        let front_grpc = self.grpc_mode(cx);
        let front_mqtt = self.mqtt_mode(cx);
        let tabs = self.tabs.iter().enumerate().map(|(ix, tab)| {
            let front = tab.parked.is_none();
            let (name, method, grpc, mqtt) = match &tab.parked {
                Some(parked) => (
                    parked.draft.name.clone(),
                    parked.draft.method,
                    parked.draft.is_grpc(),
                    parked.draft.is_mqtt(),
                ),
                None => (self.base.name.clone(), front_method, front_grpc, front_mqtt),
            };
            let name = if name.is_empty() {
                tab.path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default()
            } else {
                name
            };
            let dirty = self.tab_dirty(tab, cx);
            let color = self.collection_color(&tab.path);
            let (open, close, middle) = (tab.path.clone(), tab.path.clone(), tab.path.clone());
            h_flex()
                .id(("tab", ix))
                .dir_row()
                .flex_none()
                .gap_2()
                .items_center()
                .ps(px(12.))
                .pe(px(4.))
                .h(px(30.))
                .rounded_t_md()
                .border_b_2()
                .border_color(if front {
                    p.accent
                } else {
                    gpui_kit::transparent_black()
                })
                .when(front, |t| t.bg(p.surface))
                .hover(|t| t.bg(crate::theme::current().hover))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.open_tab(open.clone(), window, cx)
                }))
                .on_mouse_up(
                    MouseButton::Middle,
                    cx.listener(move |this, _: &MouseUpEvent, window, cx| {
                        this.close_tab(middle.clone(), window, cx)
                    }),
                )
                // The colour of the request's collection, in a project.
                .when_some(color, |t, color| {
                    t.child(
                        div()
                            .w(px(6.))
                            .h(px(6.))
                            .flex_none()
                            .rounded_full()
                            .bg(color),
                    )
                })
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if grpc {
                            theme::grpc_color()
                        } else if mqtt {
                            theme::mqtt_color()
                        } else {
                            theme::method_color(method)
                        })
                        .child(if grpc {
                            "gRPC"
                        } else if mqtt {
                            "MQTT"
                        } else {
                            short(method)
                        }),
                )
                .child(
                    div()
                        .max_w(px(180.))
                        .truncate()
                        .text_sm()
                        .text_color(if front { p.text } else { p.text_muted })
                        .child(name),
                )
                .child(
                    div()
                        .id(("tab-close", ix))
                        .w(px(18.))
                        .h(px(18.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_sm()
                        .text_xs()
                        .text_color(if dirty { p.warn } else { p.text_muted })
                        .hover(|c| c.bg(crate::theme::current().hover).text_color(p.text))
                        .child(if dirty { "●" } else { "✕" })
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            this.close_tab(close.clone(), window, cx)
                        })),
                )
        });
        let strip = h_flex()
            .id("tabs")
            .w_full()
            .flex_none()
            .gap_1()
            .overflow_x_scroll()
            .border_b_1()
            .border_color(p.hairline);
        // A reversed row would overflow to the left, out of scroll reach, so in
        // RTL the tabs sit in a row at least as wide as the strip that grows rightwards.
        Some(if bidi::rtl() {
            strip.child(
                h_flex()
                    .flex_none()
                    .min_w_full()
                    .gap_1()
                    .flex_row_reverse()
                    .children(tabs),
            )
        } else {
            strip.children(tabs)
        })
    }
}

/// Short method names keep tabs narrow.
fn short(method: Method) -> &'static str {
    match method {
        Method::Delete => "DEL",
        Method::Options => "OPT",
        Method::Patch => "PATCH",
        other => other.as_str(),
    }
}

impl Workspace {
    /// Puts `state` in the parked tab of `path` (a stream that started
    /// while its tab was in the back).
    pub(super) fn set_parked_response(&mut self, path: Option<PathBuf>, state: ResponseState) {
        if let Some(parked) = path
            .and_then(|p| self.tab_index(&p))
            .and_then(|ix| self.tabs[ix].parked.as_mut())
        {
            parked.response = state;
        }
    }
}
