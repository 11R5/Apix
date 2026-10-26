//! A gRPC request: loading the methods for the Message tab, and the call in
//! the response pane, as a log of what was sent (↑) and received (↓), then
//! the status, the response metadata and the trailers. Esc stops a call.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use apix_grpc::{CallEvent, CallOutcome};
use apix_http::human_duration;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{ResponseState, Workspace, undefined_message};
use crate::bidi::Bidi as _;
use crate::{net, tr};

pub(super) enum GrpcEntry {
    Sent(String),
    Received(String),
}

pub(super) struct GrpcSession {
    pub method: String,
    pub started: Instant,
    pub log: Vec<(Duration, GrpcEntry)>,
    pub headers: Vec<(String, String)>,
    /// How it ended, once it has.
    pub outcome: Option<CallOutcome>,
    /// Stopped with Esc before the server finished.
    pub stopped: bool,
    pub scroll: ScrollHandle,
}

impl GrpcSession {
    pub(super) fn is_live(&self) -> bool {
        self.outcome.is_none() && !self.stopped
    }
}

/// What the network side has seen, read by the UI every 60 ms.
#[derive(Default)]
struct Incoming {
    events: Vec<(Instant, CallEvent)>,
    result: Option<Result<CallOutcome, String>>,
}

/// gRPC errors name their cause already; transport ones get the chain.
fn message_of(err: &apix_grpc::GrpcError) -> String {
    net::error_chain(err)
}

impl Workspace {
    /// Whether the URL in the editor (variables filled in) is `grpc://` or `grpcs://`.
    pub(super) fn grpc_mode(&self, cx: &App) -> bool {
        let url = self.url.read(cx).value().trim().to_string();
        let mut missing = Default::default();
        apix_grpc::is_grpc_url(&self.vars.interpolate(&url, &mut missing))
    }

    /// The call of the request in front, while it runs.
    pub(super) fn live_grpc(&self) -> Option<Rc<RefCell<GrpcSession>>> {
        match &self.response {
            ResponseState::Grpc(session) if session.borrow().is_live() => Some(session.clone()),
            _ => None,
        }
    }

    /// The open request as it would be sent, for loading its methods:
    /// inherited auth and settings, variables, paths made absolute.
    fn grpc_request(&self, cx: &App) -> Result<apix_core::Request, SharedString> {
        let request = self.with_inherited_auth(self.build_request(cx), self.selected.as_deref());
        let request = self.with_inherited_settings(request, self.selected.as_deref());
        let resolved = request.resolve(&self.vars);
        if !resolved.missing.is_empty() {
            return Err(undefined_message(resolved.missing));
        }
        Ok(match self.body_location().0 {
            Some(dir) => resolved.request.with_files_relative_to(&dir),
            None => resolved.request,
        })
    }

    /// Asks the server for its methods (or reads the .proto files) and
    /// fills the method picker.
    pub(super) fn load_grpc_methods(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let request = match self.grpc_request(cx) {
            Ok(request) => request,
            Err(message) => {
                self.grpc_editor.update(cx, |editor, cx| {
                    editor.set_methods(Err(message.to_string()), window, cx)
                });
                return;
            }
        };
        self.grpc_editor
            .update(cx, |editor, cx| editor.set_loading(cx));
        let load = net::spawn(async move {
            let metadata = apix_grpc::metadata(&request).await?;
            let channel = apix_grpc::connect(&request).await?;
            apix_grpc::descriptors(&request, &channel, &metadata).await
        });
        let editor = self.grpc_editor.downgrade();
        self.grpc_task = Some(cx.spawn_in(window, async move |_, cx| {
            let result = match load.await {
                Ok(Ok(pool)) => Ok(pool),
                Ok(Err(e)) => Err(message_of(&e)),
                Err(e) => Err(e),
            };
            editor
                .update_in(cx, |editor, window, cx| {
                    editor.set_methods(result, window, cx)
                })
                .ok();
        }));
    }

    /// Calls a resolved gRPC request and follows the call.
    pub(super) fn call_grpc(
        &mut self,
        request: apix_core::Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let method = request
            .grpc
            .as_ref()
            .map(|g| g.method.clone())
            .unwrap_or_default();
        let session = Rc::new(RefCell::new(GrpcSession {
            method,
            started: Instant::now(),
            log: Vec::new(),
            headers: Vec::new(),
            outcome: None,
            stopped: false,
            scroll: ScrollHandle::new(),
        }));
        self.response = ResponseState::Grpc(session.clone());
        let incoming = Arc::new(Mutex::new(Incoming::default()));
        let shared = incoming.clone();
        let running = net::spawn(async move {
            let events = shared.clone();
            let result = apix_grpc::send(&request, move |event| {
                let mut seen = events.lock().unwrap_or_else(|e| e.into_inner());
                seen.events.push((Instant::now(), event));
            })
            .await
            .map_err(|e| message_of(&e));
            shared.lock().unwrap_or_else(|e| e.into_inner()).result = Some(result);
        });
        let tab = self.selected.clone();
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            // Dropping this (Esc, or another send) stops the call.
            let _running = running;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(60))
                    .await;
                let (events, result) = {
                    let mut incoming = incoming.lock().unwrap_or_else(|e| e.into_inner());
                    (std::mem::take(&mut incoming.events), incoming.result.take())
                };
                if let Some(Err(message)) = result {
                    let tab = tab.clone();
                    this.update(cx, |this, cx| this.set_grpc_failed(tab, message, cx))
                        .ok();
                    return;
                }
                let changed = !events.is_empty() || result.is_some();
                {
                    let mut s = session.borrow_mut();
                    let follow = super::socket_ui::at_bottom(&s.scroll);
                    for (at, event) in events {
                        let at = at.saturating_duration_since(s.started);
                        match event {
                            CallEvent::Sent(json) => s.log.push((at, GrpcEntry::Sent(json))),
                            CallEvent::Received(json) => {
                                s.log.push((at, GrpcEntry::Received(json)))
                            }
                            CallEvent::Headers(headers) => s.headers = headers,
                        }
                    }
                    if let Some(Ok(outcome)) = result {
                        s.outcome = Some(outcome);
                    }
                    if changed && follow {
                        s.scroll.scroll_to_bottom();
                    }
                }
                if changed {
                    this.update(cx, |_, cx| cx.notify()).ok();
                }
                if !session.borrow().is_live() {
                    return;
                }
            }
        }));
        cx.notify();
    }

    /// The call never got going (no connection, unknown method, bad JSON):
    /// show why, as for any failed request.
    fn set_grpc_failed(&mut self, tab: Option<PathBuf>, message: String, cx: &mut Context<Self>) {
        if tab == self.selected {
            self.task = None;
            self.response = ResponseState::Failed(message);
        } else {
            self.set_parked_response(tab, ResponseState::Failed(message));
        }
        cx.notify();
    }

    /// Esc during a call: stop it and keep what came; false when none runs.
    pub(super) fn stop_grpc(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(session) = self.live_grpc() else {
            return false;
        };
        session.borrow_mut().stopped = true;
        self.task = None;
        cx.notify();
        true
    }

    pub(super) fn render_grpc(
        &self,
        session: &Rc<RefCell<GrpcSession>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = crate::theme::current();
        let s = session.borrow();
        let mono = cx.theme().mono_font_family.clone();
        let (sent, received) = s.log.iter().fold((0, 0), |(up, down), (_, e)| match e {
            GrpcEntry::Sent(_) => (up + 1, down),
            GrpcEntry::Received(_) => (up, down + 1),
        });
        let elapsed = s
            .outcome
            .as_ref()
            .map_or_else(|| s.started.elapsed(), |o| o.elapsed);
        let status = match (&s.outcome, s.stopped) {
            (Some(outcome), _) => {
                let color = if outcome.is_ok() { p.ok } else { p.err };
                div()
                    .px_2()
                    .py_0p5()
                    .rounded_full()
                    .bg(color.opacity(0.15))
                    .text_color(color)
                    .font_weight(FontWeight::BOLD)
                    .child(format!("{} {}", outcome.code, outcome.code_name()))
                    .into_any_element()
            }
            (None, true) => div()
                .text_color(p.text_muted)
                .child(tr!("grpc.stopped"))
                .into_any_element(),
            (None, false) => h_flex()
                .gap_1()
                .items_center()
                .child(Spinner::new().xsmall().color(p.accent))
                .child(tr!("grpc.calling"))
                .into_any_element(),
        };
        let entries = s.log.iter().map(|(at, entry)| {
            let (arrow, color, text) = match entry {
                GrpcEntry::Sent(json) => ("↑", p.accent, json.clone()),
                GrpcEntry::Received(json) => ("↓", p.ok, json.clone()),
            };
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
                        .font_family(mono.clone())
                        .child(text),
                )
        });
        let detail = s
            .outcome
            .as_ref()
            .filter(|o| !o.status_message.is_empty())
            .map(|o| o.status_message.clone());
        let metadata = |title: SharedString, pairs: &[(String, String)]| {
            v_flex()
                .pt_2()
                .gap_0p5()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.text_muted)
                        .child(title),
                )
                .children(pairs.iter().map(|(k, v)| {
                    h_flex()
                        .gap_2()
                        .text_xs()
                        .child(div().text_color(p.text_muted).child(k.clone()))
                        .child(div().min_w_0().font_family(mono.clone()).child(v.clone()))
                }))
        };
        let trailers = s
            .outcome
            .as_ref()
            .map(|o| o.trailers.clone())
            .unwrap_or_default();
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
                    .child(status)
                    .child(format!("↑ {sent} · ↓ {received}"))
                    .child(human_duration(elapsed))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .ltr()
                            .truncate()
                            .child(s.method.clone()),
                    ),
            )
            .children(
                detail.map(|text| div().px_3().pb_2().text_sm().text_color(p.err).child(text)),
            )
            .child(
                div().flex_1().min_h_0().p_2().child(
                    v_flex()
                        .id("grpc-log")
                        .ltr()
                        .size_full()
                        .overflow_y_scroll()
                        .track_scroll(&s.scroll)
                        .children(entries)
                        .when(!s.headers.is_empty(), |d| {
                            d.child(metadata(tr!("grpc.headers"), &s.headers))
                        })
                        .when(!trailers.is_empty(), |d| {
                            d.child(metadata(tr!("grpc.trailers"), &trailers))
                        }),
                ),
            )
            .into_any_element()
    }
}
