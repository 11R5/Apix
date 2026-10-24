//! The Messages tab of a WebSocket request (`ws://` or `wss://`): a message
//! to send (Ctrl+Enter) and the request's saved messages, kept in the
//! `.apix` file as `[[message]]`.

use apix_core::WsMessage;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Editor, EditorState, InputEvent};
use gpui_kit::component::{Disableable as _, IconName, Sizable, h_flex, v_flex};
use gpui_kit::*;

use crate::bidi::{self, Bidi as _};
use crate::tr;

/// Send this text over the connection (connecting first when needed).
pub(super) struct SendMessage(pub String);

pub(super) struct WsEditor {
    composer: Entity<EditorState>,
    messages: Vec<WsMessage>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SendMessage> for WsEditor {}

impl WsEditor {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| EditorState::new(window, cx).language("json"));
        let subscriptions = vec![cx.subscribe_in(
            &composer,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter {
                    secondary: true, ..
                } => {
                    this.take_back_newline(window, cx);
                    this.send_composer(cx);
                }
                InputEvent::Change => cx.notify(),
                _ => {}
            },
        )];
        Self {
            composer,
            messages: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    pub(super) fn set_messages(&mut self, messages: &[WsMessage], cx: &mut Context<Self>) {
        self.messages = messages.to_vec();
        cx.notify();
    }

    pub(super) fn messages(&self) -> Vec<WsMessage> {
        self.messages.clone()
    }

    fn composer_text(&self, cx: &App) -> String {
        self.composer.read(cx).value().to_string()
    }

    /// The editor has already put a line break (and indent) at the cursor
    /// for the Enter of Ctrl+Enter; the message should not get it.
    fn take_back_newline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (text, cursor) = {
            let state = self.composer.read(cx);
            (state.value().to_string(), state.cursor())
        };
        let Some(newline) = text.get(..cursor).and_then(|before| before.rfind('\n')) else {
            return;
        };
        if text[newline + 1..cursor]
            .chars()
            .all(|c| c == ' ' || c == '\t')
        {
            let fixed = format!("{}{}", &text[..newline], &text[cursor..]);
            self.composer
                .update(cx, |state, cx| state.set_value(fixed, window, cx));
        }
    }

    fn send_composer(&mut self, cx: &mut Context<Self>) {
        let text = self.composer_text(cx);
        if !text.trim().is_empty() {
            cx.emit(SendMessage(text));
        }
    }

    fn save_composer(&mut self, cx: &mut Context<Self>) {
        let body = self.composer_text(cx);
        if body.trim().is_empty() {
            return;
        }
        self.messages.push(WsMessage {
            name: String::new(),
            body,
        });
        cx.notify();
    }

    fn load(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(message) = self.messages.get(ix) {
            let body = message.body.clone();
            self.composer
                .update(cx, |state, cx| state.set_value(body, window, cx));
        }
    }
}

impl Render for WsEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let empty = self.composer_text(cx).trim().is_empty();
        let rows = self.messages.iter().enumerate().map(|(ix, message)| {
            h_flex()
                .id(("ws-saved", ix))
                .dir_row()
                .gap_2()
                .items_center()
                .px_1()
                .py_0p5()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(p.hover))
                .on_click(
                    cx.listener(move |this, _: &ClickEvent, window, cx| this.load(ix, window, cx)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .ltr()
                        .text_sm()
                        .truncate()
                        .child(message.label()),
                )
                .child(
                    Button::new(("ws-saved-send", ix))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Play)
                        .tooltip(tr!("ws.send"))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(message) = this.messages.get(ix) {
                                cx.emit(SendMessage(message.body.clone()));
                            }
                            cx.stop_propagation();
                        })),
                )
                .child(
                    Button::new(("ws-saved-del", ix))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if ix < this.messages.len() {
                                this.messages.remove(ix);
                            }
                            cx.stop_propagation();
                            cx.notify();
                        })),
                )
        });
        v_flex()
            .size_full()
            .gap_2()
            .child(
                div()
                    .h(px(160.))
                    .flex_none()
                    .child(Editor::new(&self.composer).h_full()),
            )
            .child(
                h_flex()
                    .dir_row()
                    .gap_2()
                    .child(
                        Button::new("ws-send")
                            .primary()
                            .small()
                            .icon(IconName::Play)
                            .label(tr!("ws.send"))
                            .disabled(empty)
                            .on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| this.send_composer(cx)),
                            ),
                    )
                    .child(
                        Button::new("ws-save")
                            .ghost()
                            .small()
                            .icon(IconName::Plus)
                            .label(tr!("ws.save"))
                            .disabled(empty)
                            .on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| this.save_composer(cx)),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(tr!("ws.send_hint")),
                    ),
            )
            .child(
                div()
                    .pt_1()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(tr!("ws.saved", count = self.messages.len())),
            )
            .child(if self.messages.is_empty() {
                div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(bidi::para(tr!("ws.saved_empty")))
                    .into_any_element()
            } else {
                v_flex()
                    .id("ws-saved-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(rows)
                    .into_any_element()
            })
    }
}
