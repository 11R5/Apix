//! The Messages tab of a WebSocket request (`ws://` or `wss://`) or a
//! Socket.IO one: the subprotocols offered in the handshake (WebSocket) or
//! the namespace, path and auth (Socket.IO), a message to send (Ctrl+Enter)
//! as text or as bytes written in base64 or hex, with its event name and
//! whether to ask for an ack (Socket.IO), and the request's saved messages,
//! kept in the `.apix` file as `[[message]]`.

use apix_core::{MessageFormat, SocketIoOptions, WebSocketOptions, WsMessage};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Editor, EditorState, Input, InputEvent, InputState};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{Disableable as _, IconName, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::bidi::{self, Bidi as _};
use crate::tr;

/// Send this message over the connection (connecting first when needed):
/// its text read as the format says, and for Socket.IO its event and ack.
pub(super) struct SendMessage(pub WsMessage);

pub(super) struct WsEditor {
    composer: Entity<EditorState>,
    format: MessageFormat,
    subprotocols: Entity<InputState>,
    /// Socket.IO instead of plain WebSocket messages.
    socketio: bool,
    namespace: Entity<InputState>,
    path: Entity<InputState>,
    auth: Entity<InputState>,
    event: Entity<InputState>,
    ack: bool,
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

fn input(
    placeholder: SharedString,
    window: &mut Window,
    cx: &mut Context<WsEditor>,
) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
}

impl WsEditor {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| EditorState::new(window, cx).language("json"));
        let subprotocols = input(tr!("ws.subprotocols_placeholder"), window, cx);
        let namespace = input(tr!("sio.namespace_placeholder"), window, cx);
        let path = input("/socket.io/".into(), window, cx);
        let auth = input(tr!("sio.auth_placeholder"), window, cx);
        let event = input("message".into(), window, cx);
        let mut subscriptions = vec![cx.subscribe_in(
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
        for state in [&subprotocols, &namespace, &path, &auth, &event] {
            subscriptions.push(cx.subscribe(state, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }));
        }
        Self {
            composer,
            format: MessageFormat::Text,
            subprotocols,
            socketio: false,
            namespace,
            path,
            auth,
            event,
            ack: false,
            messages: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    /// Placeholders are kept inside the inputs; set them again after a language change.
    pub(super) fn retranslate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (state, text) in [
            (&self.subprotocols, tr!("ws.subprotocols_placeholder")),
            (&self.namespace, tr!("sio.namespace_placeholder")),
            (&self.auth, tr!("sio.auth_placeholder")),
        ] {
            state.update(cx, |state, cx| state.set_placeholder(text, window, cx));
        }
        cx.notify();
    }

    pub(super) fn set_messages(
        &mut self,
        messages: &[WsMessage],
        options: Option<&WebSocketOptions>,
        socketio: Option<&SocketIoOptions>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.messages = messages.to_vec();
        let protocols = options
            .map(|o| o.subprotocols.join(", "))
            .unwrap_or_default();
        self.subprotocols
            .update(cx, |state, cx| state.set_value(protocols, window, cx));
        self.socketio = socketio.is_some();
        let sio = socketio.cloned().unwrap_or_default();
        for (state, value) in [
            (&self.namespace, sio.namespace),
            (&self.path, sio.path),
            (&self.auth, sio.auth),
            (&self.event, String::new()),
        ] {
            state.update(cx, |state, cx| state.set_value(value, window, cx));
        }
        self.ack = false;
        cx.notify();
    }

    pub(super) fn messages(&self) -> Vec<WsMessage> {
        self.messages.clone()
    }

    /// Whether messages are Socket.IO events.
    pub(super) fn is_socketio(&self) -> bool {
        self.socketio
    }

    /// The `[websocket]` options, or `None` when nothing is set.
    pub(super) fn options(&self, cx: &App) -> Option<WebSocketOptions> {
        if self.socketio {
            return None;
        }
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

    /// The `[socketio]` options of a Socket.IO request.
    pub(super) fn socketio_options(&self, cx: &App) -> Option<SocketIoOptions> {
        let value = |state: &Entity<InputState>| state.read(cx).value().trim().to_owned();
        self.socketio.then(|| SocketIoOptions {
            namespace: value(&self.namespace),
            path: value(&self.path),
            auth: value(&self.auth),
        })
    }

    fn composer_text(&self, cx: &App) -> String {
        self.composer.read(cx).value().to_string()
    }

    /// The composer as a message (event and ack only for Socket.IO).
    fn composed(&self, cx: &App) -> WsMessage {
        WsMessage {
            body: self.composer_text(cx),
            format: self.format,
            event: if self.socketio {
                self.event.read(cx).value().trim().to_owned()
            } else {
                String::new()
            },
            ack: self.socketio && self.ack,
            ..Default::default()
        }
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

    /// An empty text is a message only for Socket.IO (an event without arguments).
    fn can_send(&self, cx: &App) -> bool {
        self.socketio || !self.composer_text(cx).trim().is_empty()
    }

    fn send_composer(&mut self, cx: &mut Context<Self>) {
        if self.can_send(cx) {
            cx.emit(SendMessage(self.composed(cx)));
        }
    }

    fn save_composer(&mut self, cx: &mut Context<Self>) {
        if !self.can_send(cx) {
            return;
        }
        self.messages.push(self.composed(cx));
        cx.notify();
    }

    fn load(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(message) = self.messages.get(ix) {
            let (body, event) = (message.body.clone(), message.event.clone());
            self.format = message.format;
            self.ack = message.ack;
            self.composer
                .update(cx, |state, cx| state.set_value(body, window, cx));
            self.event
                .update(cx, |state, cx| state.set_value(event, window, cx));
            cx.notify();
        }
    }

    /// A label and its input, side by side.
    fn field(&self, label: SharedString, state: &Entity<InputState>) -> impl IntoElement + use<> {
        let p = crate::theme::current();
        h_flex()
            .dir_row()
            .flex_1()
            .min_w_0()
            .gap_2()
            .items_center()
            .child(
                div()
                    .flex_none()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.text_muted)
                    .child(label),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .ltr()
                    .child(Input::new(state).small()),
            )
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
        let chip = |text: SharedString, color: Hsla| {
            div()
                .flex_none()
                .px_1p5()
                .rounded_sm()
                .bg(color.opacity(0.14))
                .text_xs()
                .text_color(color)
                .child(text)
        };
        let socketio = self.socketio;
        let rows = self.messages.iter().enumerate().map(|(ix, message)| {
            let event = match message.event.as_str() {
                "" => "message".to_owned(),
                event => event.to_owned(),
            };
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
                .when(socketio, |row| row.child(chip(event.into(), p.ok)))
                .when(socketio && message.ack, |row| {
                    row.child(chip("ack".into(), p.text_muted))
                })
                .children(
                    (!message.format.is_text())
                        .then(|| chip(format_label(message.format), p.accent)),
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
                                cx.emit(SendMessage(message.clone()));
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
        let protocol = bidi::fill_tabs(
            TabBar::new("ws-protocol").segmented().xsmall(),
            Some(usize::from(socketio)),
            vec![Tab::new().label("WebSocket"), Tab::new().label("Socket.IO")],
            cx.listener(|this, ix: &usize, _, cx| {
                this.socketio = *ix == 1;
                cx.notify();
            }),
        );
        // What the connection is set up with: subprotocols, or Socket.IO's options.
        let setup = if socketio {
            v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .dir_row()
                        .gap_3()
                        .child(protocol)
                        .child(self.field(tr!("sio.namespace"), &self.namespace))
                        .child(self.field(tr!("sio.path"), &self.path)),
                )
                .child(self.field(tr!("sio.auth"), &self.auth))
                .into_any_element()
        } else {
            h_flex()
                .dir_row()
                .gap_3()
                .child(protocol)
                .child(self.field(tr!("ws.subprotocols"), &self.subprotocols))
                .into_any_element()
        };
        let hint = if socketio {
            tr!("sio.send_hint")
        } else {
            tr!("ws.send_hint")
        };
        v_flex()
            .size_full()
            .gap_2()
            .child(setup)
            .when(socketio, |d| {
                d.child(
                    h_flex()
                        .dir_row()
                        .gap_3()
                        .items_center()
                        .child(self.field(tr!("sio.event"), &self.event))
                        .child(
                            Checkbox::new("sio-ack")
                                .dir_row()
                                .checked(self.ack)
                                .label(tr!("sio.ack"))
                                .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                    this.ack = *checked;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(
                div()
                    .h(px(if socketio { 120. } else { 160. }))
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
                            .disabled(!self.can_send(cx) || problem.is_some())
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
                            .disabled(!self.can_send(cx))
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
                    )),
            )
            // On its own line, so it has the tab's width.
            .child(div().text_xs().map(|d| match &problem {
                Some(e) => d.text_color(p.err).child(e.clone()),
                None => d.text_color(p.text_muted).child(hint),
            }))
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
