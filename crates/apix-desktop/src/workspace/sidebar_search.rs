//! The sidebar's search box: only the requests whose name, method or URL
//! hold every word typed, with the folders that lead to them opened.

use apix_core::store::TreeNode;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{IconName, Sizable as _, h_flex};
use gpui_kit::*;

use super::Workspace;
use crate::tr;

impl Workspace {
    pub(super) fn new_sidebar_search(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<InputState>, Subscription) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(tr!("sidebar.search_placeholder"))
                .clean_on_escape()
        });
        let subscription = cx.subscribe(&input, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        (input, subscription)
    }

    /// What is searched for, as lowercase words; empty when not searching.
    pub(super) fn sidebar_words(&self, cx: &App) -> Vec<String> {
        self.sidebar_search
            .read(cx)
            .value()
            .split_whitespace()
            .map(str::to_lowercase)
            .collect()
    }

    fn has_words(haystack: &str, words: &[String]) -> bool {
        let haystack = haystack.to_lowercase();
        words.iter().all(|w| haystack.contains(w.as_str()))
    }

    pub(super) fn request_matches(&self, path: &std::path::Path, words: &[String]) -> bool {
        let Some(request) = self.requests.get(path) else {
            return false;
        };
        let text = format!(
            "{} {} {} {}",
            request.name,
            request.method.as_str(),
            request.url,
            path.file_stem()
                .map(|s| s.to_string_lossy())
                .unwrap_or_default()
        );
        Self::has_words(&text, words)
    }

    /// Whether a folder's name holds every word, so all of it shows.
    pub(super) fn folder_matches(name: &str, words: &[String]) -> bool {
        Self::has_words(name, words)
    }

    /// Whether anything under `nodes` is shown for `words`.
    pub(super) fn nodes_match(&self, nodes: &[TreeNode], words: &[String]) -> bool {
        nodes.iter().any(|node| match node {
            TreeNode::Folder { name, children, .. } => {
                Self::folder_matches(name, words) || self.nodes_match(children, words)
            }
            TreeNode::Request { path } => self.request_matches(path, words),
        })
    }

    pub(super) fn render_sidebar_search(&self) -> impl IntoElement {
        h_flex().px_3().pb_2().child(
            Input::new(&self.sidebar_search)
                .small()
                .prefix(gpui_kit::component::Icon::new(IconName::Search).xsmall())
                .cleanable(true),
        )
    }
}
