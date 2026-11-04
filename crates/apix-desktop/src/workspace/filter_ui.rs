//! Above a JSON response's Pretty tab: a filter that shows only what a
//! JSONPath or jq expression picks out, and Ctrl+F for every text tab.

use std::sync::Arc;
use std::time::Duration;

use apix_core::filter::Filtered;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState, Search};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{IconName, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{Finished, ResponseState, Workspace};
use crate::bidi::Bidi as _;
use crate::large_view::Document;
use crate::tr;

/// Where the response filter stands.
pub(super) enum FilterState {
    Off,
    Running,
    /// What the filter picked out, ready to show, and how many values.
    Shown {
        doc: Arc<Document>,
        count: usize,
        language: &'static str,
    },
    Failed(SharedString),
}

impl Workspace {
    pub(super) fn new_filter_input(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        cx.new(|cx| InputState::new(window, cx).placeholder(tr!("response.filter_placeholder")))
    }

    /// What the Pretty tab shows instead of the whole body, when filtering.
    pub(super) fn filtered_doc(&self) -> Option<Arc<Document>> {
        match &self.filter_state {
            FilterState::Shown { doc, .. } => Some(doc.clone()),
            _ => None,
        }
    }

    /// Runs the filter on the response in front, shortly after the last
    /// keystroke, off the UI thread.
    pub(super) fn run_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let expression = self.filter.read(cx).value().trim().to_string();
        let ResponseState::Done(done) = &self.response else {
            return;
        };
        if expression.is_empty() || !done.json {
            self.filter_task = None;
            if !matches!(self.filter_state, FilterState::Off) {
                self.filter_state = FilterState::Off;
                if self.response_tab == 0 {
                    self.set_response_tab(0, window, cx);
                }
            }
            return;
        }
        let body = done.response.body.clone();
        self.filter_owner = self.selected.clone();
        self.filter_state = FilterState::Running;
        self.filter_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let result = cx
                .background_spawn(async move {
                    let text = String::from_utf8_lossy(&body);
                    apix_core::filter::filter(&text, &expression).map(|f: Filtered| {
                        let count = f.values.len();
                        let language = f.language.name();
                        (
                            Arc::new(Document::new(f.to_pretty(), true)),
                            count,
                            language,
                        )
                    })
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.filter_task = None;
                this.filter_state = match result {
                    Ok((doc, count, language)) => FilterState::Shown {
                        doc,
                        count,
                        language,
                    },
                    Err(message) => FilterState::Failed(message.into()),
                };
                if this.response_tab == 0 {
                    this.set_response_tab(0, window, cx);
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn clear_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filter
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.run_filter(window, cx);
    }

    /// A filter belongs to the request it was written for: another request
    /// shows its whole body. Called when the request in front changes.
    pub(super) fn forget_filter_of_other(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.filter_owner == self.selected || self.filter.read(cx).value().is_empty() {
            return;
        }
        self.filter
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.filter_task = None;
        self.filter_state = FilterState::Off;
        self.filter_owner = None;
    }

    /// Ctrl+F in whichever viewer the tab shows.
    pub(super) fn search_response(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = &self.large_view {
            view.update(cx, |view, cx| view.open_search(window, cx));
            return;
        }
        self.response_body
            .update(cx, |state, cx| state.focus(window, cx));
        window.dispatch_action(Box::new(Search), cx);
    }

    /// The filter row, on a JSON response's Pretty tab.
    pub(super) fn render_filter(
        &self,
        done: &Finished,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !done.json || self.response_tab != 0 || done.events.is_some() {
            return None;
        }
        let p = crate::theme::current();
        let (note, color): (Option<SharedString>, Hsla) = match &self.filter_state {
            FilterState::Off => (None, p.text_muted),
            FilterState::Running => (None, p.text_muted),
            FilterState::Shown {
                count: 1, language, ..
            } => (
                Some(tr!("response.filter_one", language = language)),
                p.text_muted,
            ),
            FilterState::Shown {
                count, language, ..
            } => (
                Some(tr!(
                    "response.filter_count",
                    count = count,
                    language = language
                )),
                p.text_muted,
            ),
            FilterState::Failed(message) => (Some(message.clone()), p.err),
        };
        let has_text = !self.filter.read(cx).value().is_empty();
        Some(
            h_flex()
                .dir_row()
                .gap_2()
                .items_center()
                .px_3()
                .pt_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .ltr()
                        .child(Input::new(&self.filter).small()),
                )
                .when(matches!(self.filter_state, FilterState::Running), |d| {
                    d.child(Spinner::new().xsmall().color(p.accent))
                })
                .children(note.map(|note| {
                    div()
                        .min_w_0()
                        .max_w(px(260.))
                        .truncate()
                        .text_xs()
                        .text_color(color)
                        .child(note)
                }))
                .when(has_text, |d| {
                    d.child(
                        Button::new("filter-clear")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .tooltip(tr!("response.filter_clear"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.clear_filter(window, cx)
                            })),
                    )
                })
                .into_any_element(),
        )
    }
}
