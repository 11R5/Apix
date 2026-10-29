//! A gRPC request: loading the methods for the Message tab, and the call in
//! the response pane, as a log of what was sent (↑) and received (↓), then
//! the status, the response metadata and the trailers. Esc stops a call.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use apix_core::history::{Entry, StoredResponse};
use apix_grpc::{CallEvent, CallOutcome};
use apix_http::human_duration;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::history_ui::PendingRecord;
use super::{ResponseState, Workspace, undefined_message};
use crate::bidi::Bidi as _;
use crate::{net, tr};

#[derive(Clone)]
pub(super) enum GrpcEntry {
    Sent(String),
    Received(String),
}

/// A call's log as the history keeps it:
/// `[{"at_ms": 12, "sent": true, "message": {…}}, …]`.
pub(super) fn log_to_json(log: &[(Duration, GrpcEntry)]) -> Vec<u8> {
    let items: Vec<serde_json::Value> = log
        .iter()
        .map(|(at, entry)| {
            let (sent, text) = match entry {
                GrpcEntry::Sent(t) => (true, t),
                GrpcEntry::Received(t) => (false, t),
            };
            let message = serde_json::from_str(text)
                .unwrap_or_else(|_| serde_json::Value::String(text.clone()));
            serde_json::json!({ "at_ms": at.as_millis() as u64, "sent": sent, "message": message })
        })
        .collect();
    serde_json::to_vec(&items).unwrap_or_default()
}

/// The log back from the history (empty when it was cut short).
pub(super) fn log_from_json(bytes: &[u8]) -> Vec<(Duration, GrpcEntry)> {
    let Ok(serde_json::Value::Array(items)) = serde_json::from_slice(bytes) else {
        return Vec::new();
    };
    items
        .iter()
        .map(|item| {
            let at = Duration::from_millis(item["at_ms"].as_u64().unwrap_or(0));
            let text = serde_json::to_string_pretty(&item["message"]).unwrap_or_default();
            let entry = if item["sent"].as_bool().unwrap_or(false) {
                GrpcEntry::Sent(text)
            } else {
                GrpcEntry::Received(text)
            };
            (at, entry)
        })
        .collect()
}

/// A call from the history, shown as it ended.
pub(super) fn restored(entry: &Entry, stored: &StoredResponse) -> Rc<RefCell<GrpcSession>> {
    let log = log_from_json(&entry.body);
    let received = log
        .iter()
        .filter_map(|(_, e)| match e {
            GrpcEntry::Received(text) => Some(text.clone()),
            GrpcEntry::Sent(_) => None,
        })
        .collect();
    let sent = log
        .iter()
        .filter(|(_, e)| matches!(e, GrpcEntry::Sent(_)))
        .count();
    Rc::new(RefCell::new(GrpcSession {
        method: entry
            .request
            .grpc
            .as_ref()
            .map(|g| g.method.clone())
            .unwrap_or_default(),
        started: Instant::now(),
        log,
        headers: stored.headers.clone(),
        outcome: Some(CallOutcome {
            code: i32::from(stored.status),
            status_message: stored.message.clone(),
            headers: stored.headers.clone(),
            trailers: stored.trailers.clone(),
            received,
            sent,
            elapsed: Duration::from_micros(stored.total_us),
        }),
        stopped: false,
        scroll: ScrollHandle::new(),
    }))
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
        record: Option<PendingRecord>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut record = record;
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
                // Into the history once it is over, on a background thread.
                if let Some(result) = &result
                    && let Some(record) = record.take()
                {
                    let mut log = session.borrow().log.clone();
                    log.extend(events.iter().filter_map(|(at, event)| {
                        let at = at.saturating_duration_since(session.borrow().started);
                        match event {
                            CallEvent::Sent(json) => Some((at, GrpcEntry::Sent(json.clone()))),
                            CallEvent::Received(json) => {
                                Some((at, GrpcEntry::Received(json.clone())))
                            }
                            CallEvent::Headers(_) => None,
                        }
                    }));
                    let result = result.clone();
                    cx.background_executor()
                        .spawn(async move { record.save_grpc(log_to_json(&log), &result) })
                        .detach();
                }
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{GrpcEntry, log_from_json, log_to_json};

    #[test]
    fn the_log_survives_the_history() {
        let log = vec![
            (
                Duration::from_millis(0),
                GrpcEntry::Sent("{\n  \"from\": 3\n}".into()),
            ),
            (
                Duration::from_millis(1010),
                GrpcEntry::Received("{\n  \"n\": 2\n}".into()),
            ),
        ];
        let back = log_from_json(&log_to_json(&log));
        assert_eq!(back.len(), 2);
        assert_eq!(back[1].0, Duration::from_millis(1010));
        assert!(matches!(&back[0].1, GrpcEntry::Sent(t) if t == "{\n  \"from\": 3\n}"));
        assert!(matches!(&back[1].1, GrpcEntry::Received(t) if t == "{\n  \"n\": 2\n}"));
        // A log cut at the history's size limit is not JSON any more.
        assert!(log_from_json(b"[{\"at_ms\": 0, \"sent\"").is_empty());
    }
}
