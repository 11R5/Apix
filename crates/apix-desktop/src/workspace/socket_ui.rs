//! A WebSocket or Socket.IO request in the response pane: connecting, then
//! a log of what was sent (↑) and received (↓) with times, until either
//! side closes. Socket.IO events show their name and acks. Esc or
//! "Disconnect" closes it; the log stays.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use apix_http::human_duration;
use apix_http::socketio::{SioCommand, SioEvent, SioMessage};
use apix_http::ws::{WsCommand, WsData, WsEvent};
use apix_proto::mqtt::{MqttCommand, MqttEvent, MqttMessage};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, IconName, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use tokio::sync::mpsc::UnboundedSender;

use super::{ResponseState, Workspace, undefined_message};
use crate::bidi::Bidi as _;
use crate::net::SocketIncoming;
use crate::{net, tr};

/// The log shows this many entries; older ones are dropped.
const MAX_LOG: usize = 5000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SocketState {
    Connecting,
    Open,
    Closed { code: Option<u16>, reason: String },
    Failed(String),
}

pub(super) enum Entry {
    Sent(WsData),
    Received(WsData),
    /// A Socket.IO event: sent or received, its name, its arguments for the
    /// eye, and its ack id (ours, or the one the server asks for).
    Event {
        sent: bool,
        name: String,
        args: SharedString,
        ack: Option<u64>,
    },
    /// The server acknowledged our event.
    Acked {
        id: u64,
        args: SharedString,
    },
    /// A MQTT message, published or received.
    Topic {
        sent: bool,
        topic: String,
        payload: SharedString,
        qos: u8,
        retain: bool,
    },
    /// The broker confirmed a MQTT message.
    Delivered {
        topic: String,
    },
    /// Connected, closed, or an error, in words.
    Note(SharedString),
}

enum Outgoing {
    Ws(WsCommand),
    Sio(SioCommand),
    Mqtt(MqttCommand),
}

/// Where the user's messages go.
enum Commands {
    Ws(UnboundedSender<WsCommand>),
    Sio(UnboundedSender<SioCommand>),
    Mqtt(UnboundedSender<MqttCommand>),
}

impl Commands {
    fn close(&self) {
        self.send(match self {
            Commands::Ws(_) => Outgoing::Ws(WsCommand::Close),
            Commands::Sio(_) => Outgoing::Sio(SioCommand::Close),
            Commands::Mqtt(_) => Outgoing::Mqtt(MqttCommand::Close),
        });
    }

    /// Sends a command; one for the other kind of connection is dropped.
    fn send(&self, command: Outgoing) {
        match (self, command) {
            (Commands::Ws(tx), Outgoing::Ws(command)) => {
                let _ = tx.send(command);
            }
            (Commands::Sio(tx), Outgoing::Sio(command)) => {
                let _ = tx.send(command);
            }
            (Commands::Mqtt(tx), Outgoing::Mqtt(command)) => {
                let _ = tx.send(command);
            }
            _ => {}
        }
    }
}

pub(super) struct SocketSession {
    pub url: String,
    pub state: SocketState,
    pub status: Option<u16>,
    pub started: Instant,
    pub log: Vec<(Duration, Entry)>,
    pub sent: usize,
    pub received: usize,
    commands: Commands,
    pub scroll: ScrollHandle,
}

impl SocketSession {
    fn is_live(&self) -> bool {
        matches!(self.state, SocketState::Connecting | SocketState::Open)
    }

    fn push(&mut self, at: Instant, entry: Entry) {
        match &entry {
            Entry::Sent(_) | Entry::Event { sent: true, .. } | Entry::Topic { sent: true, .. } => {
                self.sent += 1
            }
            Entry::Received(_)
            | Entry::Event { .. }
            | Entry::Topic { .. }
            | Entry::Acked { .. } => self.received += 1,
            Entry::Delivered { .. } | Entry::Note(_) => {}
        }
        self.log
            .push((at.saturating_duration_since(self.started), entry));
        if self.log.len() > MAX_LOG {
            self.log.drain(..self.log.len() - MAX_LOG);
        }
    }

    fn closed(&mut self, at: Instant, code: Option<u16>, reason: String) {
        self.push(at, Entry::Note(closed_note(code, &reason)));
        self.state = SocketState::Closed { code, reason };
    }

    fn failed(&mut self, at: Instant, message: String) {
        self.push(at, Entry::Note(tr!("ws.error", error = message.clone())));
        self.state = SocketState::Failed(message);
    }

    fn take_ws(&mut self, at: Instant, event: WsEvent) {
        match event {
            WsEvent::Sent(data) => self.push(at, Entry::Sent(data)),
            WsEvent::Received(data) => self.push(at, Entry::Received(data)),
            WsEvent::Closed { code, reason } => self.closed(at, code, reason),
            WsEvent::Error(message) => self.failed(at, message),
        }
    }

    fn take_sio(&mut self, at: Instant, event: SioEvent) {
        let entry = |sent: bool, m: SioMessage| Entry::Event {
            sent,
            name: m.event,
            args: apix_http::socketio::show_args(&m.args).into(),
            ack: m.ack,
        };
        match event {
            SioEvent::Sent(m) => self.push(at, entry(true, m)),
            SioEvent::Received(m) => self.push(at, entry(false, m)),
            SioEvent::Acked { id, args } => {
                let args = apix_http::socketio::show_args(&args).into();
                self.push(at, Entry::Acked { id, args })
            }
            SioEvent::Disconnected => self.push(at, Entry::Note(tr!("sio.left"))),
            SioEvent::Unreadable(text) => {
                self.push(at, Entry::Note(tr!("sio.unreadable", text = text)))
            }
            SioEvent::Closed { code, reason } => self.closed(at, code, reason),
            SioEvent::Error(message) => self.failed(at, message),
        }
    }

    fn take_mqtt(&mut self, at: Instant, event: MqttEvent) {
        let entry = |sent: bool, m: MqttMessage| Entry::Topic {
            sent,
            payload: payload_text(&m.payload),
            topic: m.topic,
            qos: m.qos,
            retain: m.retain,
        };
        match event {
            MqttEvent::Sent(m) => self.push(at, entry(true, m)),
            MqttEvent::Received(m) => self.push(at, entry(false, m)),
            MqttEvent::Delivered { topic } => self.push(at, Entry::Delivered { topic }),
            MqttEvent::Subscribed { topic, qos } => {
                let note = tr!("mqtt.subscribed", topic = topic, qos = qos);
                self.push(at, Entry::Note(note))
            }
            MqttEvent::SubscribeRefused { topic } => {
                self.push(at, Entry::Note(tr!("mqtt.sub_refused", topic = topic)))
            }
            MqttEvent::Unsubscribed { topic } => {
                self.push(at, Entry::Note(tr!("mqtt.unsubscribed", topic = topic)))
            }
            MqttEvent::Rejected(reason) => {
                self.push(at, Entry::Note(tr!("ws.bad_message", reason = reason)))
            }
            MqttEvent::Closed { by_broker } => {
                let note = if by_broker {
                    tr!("mqtt.closed_by_broker")
                } else {
                    tr!("ws.closed")
                };
                self.push(at, Entry::Note(note));
                self.state = SocketState::Closed {
                    code: None,
                    reason: String::new(),
                };
            }
            MqttEvent::Error(message) => self.failed(at, message),
        }
    }
}

impl Workspace {
    /// Whether the request in front is a connection: a `ws://`, `wss://`,
    /// `mqtt://` or `mqtts://` URL (variables filled in), or Socket.IO.
    pub(super) fn websocket_mode(&self, cx: &App) -> bool {
        if self.ws_editor.read(cx).is_socketio() || self.mqtt_mode(cx) {
            return true;
        }
        let url = self.url.read(cx).value().trim().to_string();
        let mut missing = Default::default();
        apix_http::ws::is_websocket_url(&self.vars.interpolate(&url, &mut missing))
    }

    /// Whether the URL in the editor (variables filled in) is `mqtt://` or `mqtts://`.
    pub(super) fn mqtt_mode(&self, cx: &App) -> bool {
        let url = self.url.read(cx).value().trim().to_string();
        let mut missing = Default::default();
        let url = self.vars.interpolate(&url, &mut missing);
        apix_core::Request::new(apix_core::Method::Get, url).is_mqtt()
    }

    /// The open (or opening) connection of the request in front.
    pub(super) fn live_socket(&self) -> Option<Rc<RefCell<SocketSession>>> {
        match &self.response {
            ResponseState::Socket(session) if session.borrow().is_live() => Some(session.clone()),
            _ => None,
        }
    }

    /// Connects a resolved WebSocket request and follows the connection.
    pub(super) fn connect_socket(
        &mut self,
        request: apix_core::Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let url = request.url.clone();
        let (incoming, commands, running) = net::ws_connect(request);
        self.follow_socket(
            url,
            Commands::Ws(commands),
            incoming,
            Box::new(running),
            SocketSession::take_ws,
            window,
            cx,
        );
    }

    /// Connects a resolved Socket.IO request, joins its namespace and
    /// follows the connection.
    pub(super) fn connect_socketio(
        &mut self,
        request: apix_core::Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let url = request.url.clone();
        let (incoming, commands, running) = net::sio_connect(request);
        self.follow_socket(
            url,
            Commands::Sio(commands),
            incoming,
            Box::new(running),
            SocketSession::take_sio,
            window,
            cx,
        );
    }

    /// Connects a resolved MQTT request and follows the connection.
    pub(super) fn connect_mqtt(
        &mut self,
        request: apix_core::Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let url = request.url.clone();
        let (incoming, commands, running) = net::mqtt_connect(request);
        self.follow_socket(
            url,
            Commands::Mqtt(commands),
            incoming,
            Box::new(running),
            SocketSession::take_mqtt,
            window,
            cx,
        );
    }

    /// Shows a new connection and takes what it reports every 60 ms until it is over.
    #[allow(clippy::too_many_arguments)]
    fn follow_socket<E: Send + 'static>(
        &mut self,
        url: String,
        commands: Commands,
        incoming: Arc<Mutex<SocketIncoming<E>>>,
        running: Box<dyn Send>,
        take: fn(&mut SocketSession, Instant, E),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session = Rc::new(RefCell::new(SocketSession {
            url,
            state: SocketState::Connecting,
            status: None,
            started: Instant::now(),
            log: Vec::new(),
            sent: 0,
            received: 0,
            commands,
            scroll: ScrollHandle::new(),
        }));
        self.response = ResponseState::Socket(session.clone());
        let tab = self.selected.clone();
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let _running = running;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(60))
                    .await;
                let (opened, joined, client, events, failed, done) = {
                    let mut incoming = incoming.lock().unwrap_or_else(|e| e.into_inner());
                    (
                        incoming.opened.take(),
                        incoming.joined.take(),
                        incoming.client.take(),
                        std::mem::take(&mut incoming.events),
                        incoming.failed.take(),
                        incoming.done,
                    )
                };
                if let Some(message) = failed {
                    let tab = tab.clone();
                    this.update(cx, |this, cx| {
                        this.set_socket_failed(tab, message, &session, cx)
                    })
                    .ok();
                    return;
                }
                let changed = opened.is_some() || !events.is_empty();
                {
                    let mut s = session.borrow_mut();
                    let follow = at_bottom(&s.scroll);
                    if let Some((at, status, headers)) = opened {
                        // MQTT has no HTTP status to show.
                        s.status = (status != 0).then_some(status);
                        s.state = SocketState::Open;
                        let note = tr!("ws.connected", url = s.url.clone());
                        s.push(at, Entry::Note(note));
                        // The subprotocol the server picked, when it picked one.
                        if let Some((_, protocol)) = headers
                            .iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case("sec-websocket-protocol"))
                        {
                            let note = tr!("ws.subprotocol", protocol = protocol.clone());
                            s.push(at, Entry::Note(note));
                        }
                        if let Some((namespace, sid)) = joined {
                            let note = tr!("sio.joined", namespace = namespace, sid = sid);
                            s.push(at, Entry::Note(note));
                        }
                        if let Some((id, session_present)) = client {
                            let note = if session_present {
                                tr!("mqtt.session_kept", id = id)
                            } else {
                                tr!("mqtt.session", id = id)
                            };
                            s.push(at, Entry::Note(note));
                        }
                    }
                    for (at, event) in events {
                        take(&mut s, at, event);
                    }
                    if done && s.is_live() {
                        s.state = SocketState::Closed {
                            code: None,
                            reason: String::new(),
                        };
                    }
                    if changed && follow {
                        s.scroll.scroll_to_bottom();
                    }
                }
                if changed || done {
                    this.update(cx, |_, cx| cx.notify()).ok();
                }
                if done {
                    return;
                }
            }
        }));
        cx.notify();
    }

    /// The handshake failed: show why, as for any failed request.
    fn set_socket_failed(
        &mut self,
        tab: Option<PathBuf>,
        message: String,
        session: &Rc<RefCell<SocketSession>>,
        cx: &mut Context<Self>,
    ) {
        session.borrow_mut().state = SocketState::Failed(message.clone());
        if tab == self.selected {
            self.task = None;
            self.response = ResponseState::Failed(message);
        } else {
            self.set_parked_response(tab, ResponseState::Failed(message));
        }
        cx.notify();
    }

    /// A message from the Messages tab: variables filled in, then sent (a
    /// Socket.IO event as its arguments); without a connection, connect
    /// first (it waits its turn).
    pub(super) fn send_socket_message(
        &mut self,
        message: apix_core::WsMessage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut missing = Default::default();
        let text = self.vars.interpolate(&message.body, &mut missing);
        let event = self.vars.interpolate(&message.event, &mut missing);
        let topic = self.vars.interpolate(&message.topic, &mut missing);
        if !missing.is_empty() {
            let message = undefined_message(missing);
            self.notify_error(message, window, cx);
            return;
        }
        // Base64 and hex become bytes here, after the variables.
        let command = if self.mqtt_mode(cx) {
            apix_proto::mqtt::payload_from_text(&text, message.format).map(|payload| {
                Outgoing::Mqtt(MqttCommand::Publish(MqttMessage {
                    topic,
                    payload,
                    qos: message.qos,
                    retain: message.retain,
                }))
            })
        } else if self.ws_editor.read(cx).is_socketio() {
            apix_http::socketio::args_from_text(&text, message.format).map(|args| {
                let event = match event.trim() {
                    "" => "message".to_owned(),
                    event => event.to_owned(),
                };
                Outgoing::Sio(SioCommand::Emit {
                    event,
                    args,
                    ack: message.ack,
                })
            })
        } else {
            WsData::from_text(text, message.format).map(|data| Outgoing::Ws(WsCommand::Send(data)))
        };
        let command = match command {
            Ok(command) => command,
            Err(e) => {
                self.notify_error(tr!("ws.bad_message", reason = e), window, cx);
                return;
            }
        };
        if self.live_socket().is_none() {
            self.send(window, cx);
        }
        if let Some(session) = self.live_socket() {
            session.borrow().commands.send(command);
        }
    }

    /// Subscribes to a topic now; without a connection, connecting subscribes
    /// to the request's list, which holds this one.
    pub(super) fn subscribe_topic(
        &mut self,
        topic: &str,
        qos: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut missing = Default::default();
        let topic = self.vars.interpolate(topic, &mut missing);
        if !missing.is_empty() {
            let message = undefined_message(missing);
            self.notify_error(message, window, cx);
            return;
        }
        match self.live_socket() {
            Some(session) => session
                .borrow()
                .commands
                .send(Outgoing::Mqtt(MqttCommand::Subscribe { topic, qos })),
            None => self.send(window, cx),
        }
    }

    /// Leaves a topic taken off the list, if connected.
    pub(super) fn unsubscribe_topic(&mut self, topic: &str) {
        if let Some(session) = self.live_socket() {
            let mut missing = Default::default();
            let topic = self.vars.interpolate(topic, &mut missing);
            session
                .borrow()
                .commands
                .send(Outgoing::Mqtt(MqttCommand::Unsubscribe { topic }));
        }
    }

    /// Closes the open connection (the log stays); false when there is none.
    pub(super) fn disconnect_socket(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(session) = self.live_socket() else {
            return false;
        };
        session.borrow().commands.close();
        cx.notify();
        true
    }

    pub(super) fn render_socket(
        &self,
        session: &Rc<RefCell<SocketSession>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = crate::theme::current();
        let s = session.borrow();
        let (state_label, state_color): (SharedString, Hsla) = match &s.state {
            SocketState::Connecting => (tr!("ws.connecting"), p.text_muted),
            SocketState::Open => (tr!("ws.open"), p.ok),
            SocketState::Closed { code, reason } => (closed_note(*code, reason), p.text_muted),
            SocketState::Failed(_) => (tr!("ws.failed"), p.err),
        };
        let live = s.is_live();
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
        let entries = s.log.iter().map(|(at, entry)| {
            let (arrow, color, text): (&str, Hsla, SharedString) = match entry {
                Entry::Sent(data) => ("↑", p.accent, show(data)),
                Entry::Received(data) => ("↓", p.ok, show(data)),
                Entry::Event { sent, args, .. } => {
                    if *sent {
                        ("↑", p.accent, args.clone())
                    } else {
                        ("↓", p.ok, args.clone())
                    }
                }
                Entry::Acked { args, .. } => ("✓", p.ok, args.clone()),
                Entry::Topic { sent, payload, .. } => {
                    if *sent {
                        ("↑", p.accent, payload.clone())
                    } else {
                        ("↓", p.ok, payload.clone())
                    }
                }
                Entry::Delivered { .. } => ("✓", p.ok, tr!("mqtt.delivered")),
                Entry::Note(note) => ("•", p.text_muted, note.clone()),
            };
            // A MQTT line's retain flag.
            let retained = matches!(entry, Entry::Topic { retain: true, .. });
            // The event name and ack, or the topic and QoS, in front of a line.
            let (name, ack): (Option<SharedString>, Option<SharedString>) = match entry {
                Entry::Topic { topic, qos, .. } => (
                    Some(topic.clone().into()),
                    (*qos > 0).then(|| format!("QoS {qos}").into()),
                ),
                Entry::Delivered { topic } => (Some(topic.clone().into()), None),
                Entry::Event {
                    sent, name, ack, ..
                } => (
                    Some(name.clone().into()),
                    ack.map(|id| {
                        if *sent {
                            tr!("sio.ack_id", id = id)
                        } else {
                            tr!("sio.wants_ack", id = id)
                        }
                    }),
                ),
                Entry::Acked { id, .. } => (None, Some(tr!("sio.ack_id", id = id))),
                _ => (None, None),
            };
            let note = matches!(entry, Entry::Note(_));
            h_flex()
                .gap_2()
                .items_start()
                .py_1()
                .border_b_1()
                .border_color(p.hairline)
                .child(
                    div()
                        .w(px(56.))
                        .flex_none()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(format!("+{:.2}s", at.as_secs_f64())),
                )
                .child(
                    div()
                        .w(px(14.))
                        .flex_none()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(color)
                        .child(arrow),
                )
                .children(name.map(|name| chip(name, color)))
                .children(ack.map(|ack| chip(ack, p.text_muted)))
                .when(retained, |d| d.child(chip("retain".into(), p.text_muted)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .text_color(if note { p.text_muted } else { p.text })
                        .when(!note, |d| {
                            d.font_family(cx.theme().mono_font_family.clone())
                        })
                        .child(text),
                )
        });
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .dir_row()
                    .gap_3()
                    .items_center()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(p.text_muted)
                    .children(s.status.map(|status| {
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_full()
                            .bg(p.ok.opacity(0.15))
                            .text_color(p.ok)
                            .font_weight(FontWeight::BOLD)
                            .child(status.to_string())
                    }))
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .when(s.state == SocketState::Connecting, |d| {
                                d.child(Spinner::new().xsmall().color(p.accent))
                            })
                            .when(s.state == SocketState::Open, |d| {
                                d.child(div().size(px(7.)).rounded_full().bg(p.ok))
                            })
                            .child(div().text_color(state_color).child(state_label)),
                    )
                    .child(format!("↑ {} · ↓ {}", s.sent, s.received))
                    .child(human_duration(s.started.elapsed()))
                    .child(div().flex_1())
                    .when(live, |d| {
                        d.child(
                            Button::new("ws-disconnect")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Close)
                                .label(tr!("ws.disconnect"))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.disconnect_socket(cx);
                                })),
                        )
                    }),
            )
            .child(
                div().flex_1().min_h_0().p_2().child(
                    v_flex()
                        .id("ws-log")
                        .ltr()
                        .size_full()
                        .overflow_y_scroll()
                        .track_scroll(&s.scroll)
                        .children(entries),
                ),
            )
            .into_any_element()
    }
}

fn show(data: &WsData) -> SharedString {
    match data {
        WsData::Text(text) => text.clone().into(),
        WsData::Binary(bytes) => tr!(
            "ws.bytes",
            count = bytes.len(),
            hex = apix_http::ws::hex_bytes(bytes)
        ),
    }
}

/// A MQTT payload: its text, or its size and first bytes.
fn payload_text(payload: &[u8]) -> SharedString {
    match std::str::from_utf8(payload) {
        Ok(text) => text.to_owned().into(),
        Err(_) => tr!(
            "ws.bytes",
            count = payload.len(),
            hex = apix_http::ws::hex_bytes(payload)
        ),
    }
}

fn closed_note(code: Option<u16>, reason: &str) -> SharedString {
    match (code, reason.is_empty()) {
        (Some(code), false) => tr!("ws.closed_reason", code = code, reason = reason),
        (Some(code), true) => tr!("ws.closed_code", code = code),
        (None, _) => tr!("ws.closed"),
    }
}

/// Whether the log shows its end (so new entries should keep it there).
pub(super) fn at_bottom(scroll: &ScrollHandle) -> bool {
    let max = scroll.max_offset().y;
    max <= px(1.) || -scroll.offset().y >= max - px(24.)
}
