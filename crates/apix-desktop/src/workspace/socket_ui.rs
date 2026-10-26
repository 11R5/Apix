//! A WebSocket request in the response pane: connecting, then a log of
//! what was sent (↑) and received (↓) with times, until either side closes.
//! Esc or "Disconnect" closes it; the log stays.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use apix_http::human_duration;
use apix_http::ws::{WsCommand, WsData, WsEvent};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, IconName, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use tokio::sync::mpsc::UnboundedSender;

use super::{ResponseState, Workspace, undefined_message};
use crate::bidi::Bidi as _;
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
    /// Connected, closed, or an error, in words.
    Note(SharedString),
}

pub(super) struct SocketSession {
    pub url: String,
    pub state: SocketState,
    pub status: Option<u16>,
    pub started: Instant,
    pub log: Vec<(Duration, Entry)>,
    pub sent: usize,
    pub received: usize,
    commands: UnboundedSender<WsCommand>,
    pub scroll: ScrollHandle,
}

impl SocketSession {
    fn is_live(&self) -> bool {
        matches!(self.state, SocketState::Connecting | SocketState::Open)
    }

    fn push(&mut self, at: Instant, entry: Entry) {
        match &entry {
            Entry::Sent(_) => self.sent += 1,
            Entry::Received(_) => self.received += 1,
            Entry::Note(_) => {}
        }
        self.log
            .push((at.saturating_duration_since(self.started), entry));
        if self.log.len() > MAX_LOG {
            self.log.drain(..self.log.len() - MAX_LOG);
        }
    }
}

impl Workspace {
    /// Whether the URL in the editor (variables filled in) is `ws://` or `wss://`.
    pub(super) fn websocket_mode(&self, cx: &App) -> bool {
        let url = self.url.read(cx).value().trim().to_string();
        let mut missing = Default::default();
        apix_http::ws::is_websocket_url(&self.vars.interpolate(&url, &mut missing))
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
                let (opened, events, failed, done) = {
                    let mut incoming = incoming.lock().unwrap_or_else(|e| e.into_inner());
                    (
                        incoming.opened.take(),
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
                    if let Some((at, status, _)) = opened {
                        s.status = Some(status);
                        s.state = SocketState::Open;
                        let note = tr!("ws.connected", url = s.url.clone());
                        s.push(at, Entry::Note(note));
                    }
                    for (at, event) in events {
                        match event {
                            WsEvent::Sent(data) => s.push(at, Entry::Sent(data)),
                            WsEvent::Received(data) => s.push(at, Entry::Received(data)),
                            WsEvent::Closed { code, reason } => {
                                let note = closed_note(code, &reason);
                                s.push(at, Entry::Note(note));
                                s.state = SocketState::Closed { code, reason };
                            }
                            WsEvent::Error(message) => {
                                s.push(at, Entry::Note(tr!("ws.error", error = message.clone())));
                                s.state = SocketState::Failed(message);
                            }
                        }
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

    /// A message from the Messages tab: variables filled in, then sent;
    /// without a connection, connect first (it waits its turn).
    pub(super) fn send_socket_message(
        &mut self,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut missing = Default::default();
        let text = self.vars.interpolate(&text, &mut missing);
        if !missing.is_empty() {
            let message = undefined_message(missing);
            self.notify_error(message, window, cx);
            return;
        }
        if self.live_socket().is_none() {
            self.send(window, cx);
        }
        if let Some(session) = self.live_socket() {
            let _ = session
                .borrow()
                .commands
                .send(WsCommand::Send(WsData::Text(text)));
        }
    }

    /// Closes the open connection (the log stays); false when there is none.
    pub(super) fn disconnect_socket(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(session) = self.live_socket() else {
            return false;
        };
        let _ = session.borrow().commands.send(WsCommand::Close);
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
        let entries = s.log.iter().map(|(at, entry)| {
            let (arrow, color, text): (&str, Hsla, SharedString) = match entry {
                Entry::Sent(data) => ("↑", p.accent, show(data)),
                Entry::Received(data) => ("↓", p.ok, show(data)),
                Entry::Note(note) => ("•", p.text_muted, note.clone()),
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
        WsData::Binary(bytes) => tr!("ws.binary", count = bytes.len()),
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
