//! Drag and drop in the sidebar: drag a request or folder onto a folder or a
//! collection to move it there (files, scripts and `folder.toml` move along).
//! Dropping onto a request moves into that request's folder.

use std::path::{Path, PathBuf};

use apix_core::store;
use gpui_kit::component::h_flex;
use gpui_kit::*;

use super::{Workspace, describe};
use crate::theme::Palette;

/// A request or folder being dragged.
#[derive(Clone)]
pub(super) struct DraggedNode {
    pub(super) path: PathBuf,
    pub(super) folder: bool,
    pub(super) label: String,
    colors: Palette,
}

impl DraggedNode {
    pub(super) fn new(path: &Path, folder: bool, label: String, colors: Palette) -> Self {
        Self {
            path: path.to_owned(),
            folder,
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
            .child(if self.folder { "📁" } else { "↳" })
            .child(self.label.clone())
    }
}

/// Lets `row` be dragged as `node`.
pub(super) fn draggable(row: Stateful<Div>, node: DraggedNode) -> Stateful<Div> {
    row.on_drag(node, |node, _, _, cx| cx.new(|_| node.clone()))
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
        row.drag_over::<DraggedNode>(move |style, _, _, _| style.bg(accent.opacity(0.18)))
            .on_drop(cx.listener(move |this, node: &DraggedNode, window, cx| {
                cx.stop_propagation();
                this.move_node(node, &dir, window, cx);
            }))
    }

    fn move_node(
        &mut self,
        node: &DraggedNode,
        dir: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if node.path.parent() == Some(dir) || node.path == dir {
            return;
        }
        let moved = if node.folder {
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
            }
            Err(e) => self.notify_error(describe(&e), window, cx),
        }
    }
}
