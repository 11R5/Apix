//! Examples: responses kept with their request (`[[example]]`). One is
//! saved from the response pane, listed on the Examples tab with its name to
//! edit, and shown again in the response pane. They are part of the request,
//! so Ctrl+S writes them like any other change.

use apix_core::Example;
use apix_core::model::MAX_EXAMPLE_BODY;
use apix_http::{Response, Timing, human_size};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{IconName, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use super::{Finished, ResponseState, Workspace};
use crate::bidi::{self, Bidi as _};
use crate::tr;

/// The Examples tab's place among the request tabs.
pub(super) const EXAMPLES_TAB: usize = 6;

pub(super) struct ExampleRow {
    pub example: Example,
    pub name: Entity<InputState>,
    _change: Subscription,
}

impl Workspace {
    fn example_row(
        &mut self,
        example: Example,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ExampleRow {
        let name = cx.new(|cx| InputState::new(window, cx).default_value(example.name.clone()));
        let change = cx.subscribe(&name, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        ExampleRow {
            example,
            name,
            _change: change,
        }
    }

    /// The request's examples, into the Examples tab.
    pub(super) fn show_examples(
        &mut self,
        examples: &[Example],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.examples = examples
            .iter()
            .cloned()
            .map(|e| self.example_row(e, window, cx))
            .collect();
    }

    /// The examples as the request keeps them, with the names as edited.
    pub(super) fn example_values(&self, cx: &App) -> Vec<Example> {
        self.examples
            .iter()
            .map(|row| Example {
                name: row.name.read(cx).value().trim().to_string(),
                ..row.example.clone()
            })
            .collect()
    }

    /// Keeps the response in front as an example of the request.
    pub(super) fn save_example(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ResponseState::Done(done) = &self.response else {
            return;
        };
        let response = &done.response;
        if response.body.len() > MAX_EXAMPLE_BODY {
            window.push_notification(
                Notification::error(tr!(
                    "example.too_big",
                    size = human_size(response.body.len()),
                    most = human_size(MAX_EXAMPLE_BODY)
                )),
                cx,
            );
            return;
        }
        let name = format!("{} {}", response.status, response.reason.unwrap_or(""))
            .trim()
            .to_owned();
        let example = Example::new(
            name.clone(),
            response.status,
            &response.headers,
            &response.body,
            Some(&self.build_request(cx)),
        );
        let row = self.example_row(example, window, cx);
        self.examples.push(row);
        window.push_notification(Notification::success(tr!("example.saved", name = name)), cx);
        cx.notify();
    }

    /// An example in the response pane, as if it had just come back.
    fn open_example(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.examples.get(ix) else {
            return;
        };
        let example = &row.example;
        let response = Response::from_parts(
            example.status,
            "HTTP/1.1".into(),
            example.header_pairs(),
            example.body_bytes(),
            Timing {
                ttfb: std::time::Duration::ZERO,
                total: std::time::Duration::ZERO,
            },
        );
        let mut finished = Finished::prepare(response);
        finished.example = Some(row.name.read(cx).value().to_string().into());
        self.finish(Ok(finished), window, cx);
    }

    pub(super) fn render_examples(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = crate::theme::current();
        if self.examples.is_empty() {
            return div()
                .p_3()
                .text_xs()
                .text_color(p.text_muted)
                .child(bidi::para(tr!("example.none")))
                .into_any_element();
        }
        let rows = self.examples.iter().enumerate().map(|(ix, row)| {
            let status = p.status(row.example.status);
            let size = human_size(row.example.body_bytes().len());
            h_flex()
                .dir_row()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .flex_none()
                        .px_2()
                        .py_0p5()
                        .rounded_full()
                        .bg(status.opacity(0.15))
                        .text_xs()
                        .text_color(status)
                        .font_weight(FontWeight::BOLD)
                        .child(row.example.status.to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(Input::new(&row.name).small()),
                )
                .child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(size),
                )
                .child(
                    Button::new(("example-open", ix))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Play)
                        .tooltip(tr!("example.open"))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.open_example(ix, window, cx)
                        })),
                )
                .child(
                    Button::new(("example-delete", ix))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Delete)
                        .tooltip(tr!("example.delete"))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if ix < this.examples.len() {
                                this.examples.remove(ix);
                            }
                            cx.notify();
                        })),
                )
        });
        v_flex()
            .id("examples")
            .size_full()
            .p_2()
            .gap_1()
            .overflow_y_scroll()
            .children(rows)
            .child(
                div()
                    .pt_2()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(bidi::para(tr!("example.hint"))),
            )
            .into_any_element()
    }
}
