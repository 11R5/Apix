//! The Messages tab of a WebSocket request (`ws://` or `wss://`): the
//! subprotocols offered in the handshake, a message to send (Ctrl+Enter) as
//! text or as bytes written in base64 or hex, and the request's saved
//! messages, kept in the `.apix` file as `[[message]]`.

use apix_core::{MessageFormat, WebSocketOptions, WsMessage};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Editor, EditorState, Input, InputEvent, InputState};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{Disableable as _, IconName, Sizable, h_flex, v_flex};
use gpui_kit::*;

use crate::bidi::{self, Bidi as _};
use crate::tr;

/// Send this text over the connection (connecting first when needed), read
/// as the format says.
pub(super) struct SendMessage(pub String, pub MessageFormat);

pub(super) struct WsEditor {
    composer: Entity<EditorState>,
    format: MessageFormat,
    subprotocols: Entity<InputState>,
    messages: Vec<WsMessage>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SendMessage> for WsEditor {}

fn format_label(format: MessageFormat) -> SharedString {
    match format {
        MessageFormat::Text => tr!("ws.format_text"),
        MessageFormat::Base64 => "Base64".into(),
        MessageFormat::Hex => "Hex".into(),
    }
}

impl WsEditor {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| EditorState::new(window, cx).language("json"));
        let subprotocols = cx
            .new(|cx| InputState::new(window, cx).placeholder(tr!("ws.subprotocols_placeholder")));
        let subscriptions = vec![
            cx.subscribe_in(
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
            ),
            cx.subscribe(&subprotocols, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        Self {
            composer,
            format: MessageFormat::Text,
            subprotocols,
            messages: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    /// Placeholders are kept inside the inputs; set them again after a language change.
    pub(super) fn retranslate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.subprotocols.update(cx, |state, cx| {
            state.set_placeholder(tr!("ws.subprotocols_placeholder"), window, cx)
        });
        cx.notify();
    }

    pub(super) fn set_messages(
        &mut self,
        messages: &[WsMessage],
        options: Option<&WebSocketOptions>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.messages = messages.to_vec();
        let protocols = options
            .map(|o| o.subprotocols.join(", "))
            .unwrap_or_default();
        self.subprotocols
            .update(cx, |state, cx| state.set_value(protocols, window, cx));
        cx.notify();
    }

    pub(super) fn messages(&self) -> Vec<WsMessage> {
        self.messages.clone()
    }

    /// The `[websocket]` options, or `None` when nothing is set.
    pub(super) fn options(&self, cx: &App) -> Option<WebSocketOptions> {
        let subprotocols: Vec<String> = self
            .subprotocols
            .read(cx)
            .value()
            .split(',')
            .map(|p| p.trim().to_owned())
            .filter(|p| !p.is_empty())
            .collect();
        (!subprotocols.is_empty()).then_some(WebSocketOptions { subprotocols })
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
            cx.emit(SendMessage(text, self.format));
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
            format: self.format,
        });
        cx.notify();
    }

    fn load(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(message) = self.messages.get(ix) {
            let body = message.body.clone();
            self.format = message.format;
            self.composer
                .update(cx, |state, cx| state.set_value(body, window, cx));
            cx.notify();
        }
    }
}

impl Render for WsEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let empty = self.composer_text(cx).trim().is_empty();
        // A base64 or hex message that does not read as bytes cannot be sent.
        let problem = match self.format.decode(&self.composer_text(cx)) {
            Some(Err(e)) if !empty => Some(e),
            _ => None,
        };
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
                .children((!message.format.is_text()).then(|| {
                    div()
                        .flex_none()
                        .px_1p5()
                        .rounded_sm()
                        .bg(p.accent.opacity(0.14))
                        .text_xs()
                        .text_color(p.accent)
                        .child(format_label(message.format))
                }))
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
                                cx.emit(SendMessage(message.body.clone(), message.format));
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
        let selected = MessageFormat::ALL.iter().position(|f| *f == self.format);
        v_flex()
            .size_full()
            .gap_2()
            .child(
                h_flex()
                    .dir_row()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_none()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.text_muted)
                            .child(tr!("ws.subprotocols")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .ltr()
                            .child(Input::new(&self.subprotocols).small()),
                    ),
            )
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
                    .items_center()
                    .child(
                        Button::new("ws-send")
                            .primary()
                            .small()
                            .icon(IconName::Play)
                            .label(tr!("ws.send"))
                            .disabled(empty || problem.is_some())
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
                    .child(bidi::fill_tabs(
                        TabBar::new("ws-format").segmented().xsmall(),
                        selected,
                        MessageFormat::ALL
                            .iter()
                            .map(|f| Tab::new().label(format_label(*f)))
                            .collect(),
                        cx.listener(|this, ix: &usize, _, cx| {
                            this.format = MessageFormat::ALL[*ix];
                            cx.notify();
                        }),
                    ))
                    .child(div().flex_1())
                    .child(match &problem {
                        Some(e) => div()
                            .text_xs()
                            .text_color(p.err)
                            .child(e.clone())
                            .into_any_element(),
                        None => div()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(tr!("ws.send_hint"))
                            .into_any_element(),
                    }),
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
