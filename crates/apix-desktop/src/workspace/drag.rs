//! Drag and drop in the sidebar. Where a row is dropped on decides what
//! happens: the top or bottom edge of a request or folder puts the dragged
//! item before or after it (moving it into that folder first if needed, and
//! saving the order); the middle of a folder or a collection moves the item
//! into it (files, scripts and `folder.toml` move along). Inside a project,
//! collections can be dragged among themselves to change their order.

use std::path::{Path, PathBuf};

use apix_core::store;
use gpui_kit::component::h_flex;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{Workspace, describe};
use crate::theme::Palette;

/// What is being dragged.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    Request,
    Folder,
    Collection,
}

/// A request, folder or collection being dragged.
#[derive(Clone)]
pub(super) struct DraggedNode {
    pub(super) path: PathBuf,
    pub(super) kind: Kind,
    pub(super) label: String,
    colors: Palette,
}

impl DraggedNode {
    pub(super) fn new(path: &Path, kind: Kind, label: String, colors: Palette) -> Self {
        Self {
            path: path.to_owned(),
            kind,
            label,
            colors,
        }
    }
}

impl Render for DraggedNode {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let p = self.colors;
        h_flex()
            .gap_2()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(p.accent.opacity(0.6))
            .bg(p.surface_raised)
            .text_sm()
            .text_color(p.text)
            .shadow_lg()
            .child(if self.kind == Kind::Request {
                "↳"
            } else {
                "📁"
            })
            .child(self.label.clone())
    }
}

/// Lets `row` be dragged as `node`.
pub(super) fn draggable(row: Stateful<Div>, node: DraggedNode) -> Stateful<Div> {
    row.on_drag(node, |node, _, _, cx| cx.new(|_| node.clone()))
}

/// Where on a row the pointer is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Zone {
    Before,
    Into,
    After,
}

/// The part of a row under the pointer, or `None` when `dragged` cannot go there.
fn zone(row: Kind, dragged: Kind, bounds: Bounds<Pixels>, y: Pixels) -> Option<Zone> {
    let at = ((y - bounds.top()) / bounds.size.height).clamp(0., 1.);
    let half = if at < 0.5 { Zone::Before } else { Zone::After };
    match (row, dragged) {
        (Kind::Collection, Kind::Collection) => Some(half),
        (_, Kind::Collection) => None,
        (Kind::Collection, _) => Some(Zone::Into),
        (Kind::Folder, _) if (0.25..0.75).contains(&at) => Some(Zone::Into),
        _ => Some(half),
    }
}

impl Workspace {
    /// Makes `row` accept dropped requests and folders, moving them into `dir`.
    pub(super) fn drop_target(
        &self,
        row: Stateful<Div>,
        dir: PathBuf,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let accent = self.palette.accent;
        let view = cx.entity().downgrade();
        let hovered = cx.has_active_drag()
            && self
                .drop_hint
                .as_ref()
                .is_some_and(|(p, zone)| *p == dir && *zone == Zone::Into);
        let target = dir.clone();
        // Parents hear the move first, so a row under the pointer overrides this.
        row.on_drag_move::<DraggedNode>(move |event, _, cx| {
            if event.bounds.contains(&event.event.position) {
                let hint =
                    (event.drag(cx).kind != Kind::Collection).then(|| (target.clone(), Zone::Into));
                view.update(cx, |this, cx| this.set_drop_hint(hint, cx))
                    .ok();
            }
        })
        .when(hovered, |row| row.bg(accent.opacity(0.08)))
        .on_drop(cx.listener(move |this, node: &DraggedNode, window, cx| {
            cx.stop_propagation();
            this.drop_hint = None;
            if node.kind != Kind::Collection {
                this.move_node(node, &dir, window, cx);
            }
        }))
    }

    /// Makes the sidebar row of `path` (a `row` kind of item) a drop target
    /// with before, into and after zones, and draws the marker for the zone
    /// under the pointer.
    pub(super) fn drop_zones(
        &self,
        row: Stateful<Div>,
        path: &Path,
        row_kind: Kind,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let accent = self.palette.accent;
        let view = cx.entity().downgrade();
        let hint = self
            .drop_hint
            .as_ref()
            .filter(|(p, _)| p == path && cx.has_active_drag())
            .map(|(_, zone)| *zone);
        let (target, drop_path) = (path.to_path_buf(), path.to_path_buf());
        let line = |top: bool| {
            div()
                .absolute()
                .left_0()
                .right_0()
                .h(px(2.))
                .rounded_full()
                .bg(accent)
                .when(top, |d| d.top_0())
                .when(!top, |d| d.bottom_0())
        };
        row.relative()
            .on_drag_move::<DraggedNode>(move |event, _, cx| {
                if !event.bounds.contains(&event.event.position) {
                    return;
                }
                let dragged = event.drag(cx).kind;
                let zone = zone(row_kind, dragged, event.bounds, event.event.position.y);
                let hint = zone.map(|z| (target.clone(), z));
                view.update(cx, |this, cx| this.set_drop_hint(hint, cx))
                    .ok();
            })
            .on_drop(cx.listener(move |this, node: &DraggedNode, window, cx| {
                cx.stop_propagation();
                let zone = this
                    .drop_hint
                    .take()
                    .filter(|(p, _)| *p == drop_path)
                    .map(|(_, zone)| zone);
                if let Some(zone) = zone {
                    this.drop_node(node, &drop_path, zone, window, cx);
                }
                cx.notify();
            }))
            .when(hint == Some(Zone::Into), |row| row.bg(accent.opacity(0.18)))
            .when(hint == Some(Zone::Before), |row| row.child(line(true)))
            .when(hint == Some(Zone::After), |row| row.child(line(false)))
    }

    fn set_drop_hint(&mut self, hint: Option<(PathBuf, Zone)>, cx: &mut Context<Self>) {
        if self.drop_hint != hint {
            self.drop_hint = hint;
            cx.notify();
        }
    }

    fn drop_node(
        &mut self,
        node: &DraggedNode,
        target: &Path,
        zone: Zone,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if node.path == target {
            return;
        }
        let after = zone == Zone::After;
        if node.kind == Kind::Collection {
            let Some(project) = self.project.as_mut() else {
                return;
            };
            match project.place_collection(&node.path, target, after) {
                Ok(()) => self.reload(window, cx),
                Err(e) => self.notify_error(describe(&e), window, cx),
            }
            return;
        }
        if zone == Zone::Into {
            self.move_node(node, target, window, cx);
            return;
        }
        let Some(dir) = target.parent() else {
            return;
        };
        let moved = if node.path.parent() == Some(dir) {
            Some(node.path.clone())
        } else {
            self.move_node(node, dir, window, cx)
        };
        if let Some(path) = moved {
            match store::place(&path, target, after) {
                Ok(()) => self.reload(window, cx),
                Err(e) => self.notify_error(describe(&e), window, cx),
            }
        }
    }

    /// Moves a dragged request or folder into `dir`. Returns its new path.
    fn move_node(
        &mut self,
        node: &DraggedNode,
        dir: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<PathBuf> {
        if node.path.parent() == Some(dir) || node.path == dir {
            return None;
        }
        let moved = if node.kind == Kind::Folder {
            store::move_folder(&node.path, dir)
        } else {
            store::move_request(&node.path, dir)
        };
        match moved {
            Ok(new) => {
                self.retarget_tabs(&node.path, &new);
                // Show where it went.
                for open in dir.ancestors() {
                    self.collapsed.remove(open);
                }
                self.reload(window, cx);
                // The active collection follows a request that changed collections.
                if let Some(selected) = self.selected.clone() {
                    self.set_active_collection(&selected, window, cx);
                }
                Some(new)
            }
            Err(e) => {
                self.notify_error(describe(&e), window, cx);
                None
            }
        }
    }
}
