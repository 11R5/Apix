//! Server-sent events in the response pane: while the stream is open, its
//! events show up as they arrive (Esc stops it and keeps what came); once it
//! has ended, the Pretty tab lists them. Other responses are untouched.
//! Events can be narrowed to one type, and with "Reconnect" on, a stream
//! that ends or drops is picked up again after the server's retry time,
//! sending `Last-Event-ID`, as a browser's EventSource does.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use apix_http::sse::{SseEvent, SseParser};
use apix_http::{Response, Timing, human_duration};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{ActiveTheme as _, IconName, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::history_ui::PendingRecord;
use super::{Finished, ResponseState, Workspace};
use crate::bidi::Bidi as _;
use crate::{net, tr};

/// The live view keeps only the newest events; the finished response has all.
const MAX_SHOWN: usize = 2000;

/// An event and how long after the request it arrived (unknown for events
/// read back from a finished body).
pub(super) type TimedEvent = (Option<Duration>, SseEvent);

/// A line between events (reconnecting, reconnected), shown before the
/// event at that index.
pub(super) type Note = (usize, SharedString);

/// A stream that is still open.
pub(super) struct LiveStream {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub started: Instant,
    /// When the status and headers came in.
    pub head_at: Duration,
    pub events: Vec<TimedEvent>,
    pub notes: Vec<Note>,
    /// Everything received, for Raw and Hex once it is over.
    pub body: Vec<u8>,
    parser: SseParser,
    pub scroll: ScrollHandle,
}

impl LiveStream {
    fn new(status: u16, headers: Vec<(String, String)>, started: Instant) -> Self {
        Self {
            status,
            headers,
            head_at: started.elapsed(),
            started,
            events: Vec::new(),
            notes: Vec::new(),
            body: Vec::new(),
            parser: SseParser::new(),
            scroll: ScrollHandle::new(),
        }
    }

    fn feed(&mut self, bytes: &[u8]) {
        self.body.extend_from_slice(bytes);
        let at = self.started.elapsed();
        self.events
            .extend(self.parser.feed(bytes).into_iter().map(|e| (Some(at), e)));
    }

    fn note(&mut self, text: SharedString) {
        self.notes.push((self.events.len(), text));
    }

    /// What arrived so far, as a finished response (when the stream is stopped).
    fn to_response(&self) -> Response {
        let elapsed = self.started.elapsed();
        Response::from_parts(
            self.status,
            String::new(),
            self.headers.clone(),
            self.body.clone(),
            Timing {
                ttfb: self.head_at,
                total: elapsed,
            },
        )
    }

    /// The stream as it stands, as a stopped response.
    fn stopped(&mut self) -> Finished {
        let mut finished = Finished::prepare(self.to_response());
        finished.events = Some(std::mem::take(&mut self.events));
        finished.stopped = true;
        finished
    }
}

/// The events of a finished event-stream body.
pub(super) fn events_of(response: &Response) -> Option<Vec<TimedEvent>> {
    response.is_event_stream().then(|| {
        apix_http::sse::parse_all(&response.body)
            .into_iter()
            .map(|e| (None, e))
            .collect()
    })
}

/// What the network side came back with, taken in one go.
struct Taken {
    head: Option<(u16, Vec<(String, String)>)>,
    event_stream: bool,
    fresh: Vec<u8>,
    result: Option<Result<Response, String>>,
}

fn take(incoming: &std::sync::Mutex<net::Incoming>, want_head: bool) -> Taken {
    let mut incoming = incoming.lock().unwrap_or_else(|e| e.into_inner());
    Taken {
        head: want_head.then(|| incoming.head.take()).flatten(),
        event_stream: incoming.event_stream,
        fresh: std::mem::take(&mut incoming.fresh),
        result: incoming.result.take(),
    }
}

impl Workspace {
    /// Sends `request` and follows it: an event stream shows up live, any
    /// response ends in `finish_for` as usual.
    pub(super) fn send_following(
        &mut self,
        request: apix_core::Request,
        record: Option<PendingRecord>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.response = ResponseState::Loading;
        self.event_filter = None;
        let started = Instant::now();
        let (mut incoming, running) = net::send_streaming(request.clone());
        // The tab may be in the back by the time anything arrives.
        let tab = self.selected.clone();
        // Replacing an older task drops it, which cancels that request.
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let mut running = Box::new(running) as Box<dyn std::any::Any>;
            let mut live: Option<Rc<RefCell<LiveStream>>> = None;
            let mut record = record;
            // After a reconnection, until its status and headers are in.
            let mut reconnecting = false;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                let taken = take(&incoming, live.is_none() || reconnecting);
                if let Some((status, headers)) = taken.head {
                    match &live {
                        None if taken.event_stream => {
                            let stream =
                                Rc::new(RefCell::new(LiveStream::new(status, headers, started)));
                            live = Some(stream.clone());
                            let tab = tab.clone();
                            this.update(cx, |this, cx| {
                                this.set_response_for(tab, ResponseState::Streaming(stream), cx)
                            })
                            .ok();
                        }
                        Some(stream) if reconnecting => {
                            reconnecting = false;
                            if status == 200 && taken.event_stream {
                                stream.borrow_mut().note(tr!("stream.reconnected"));
                            } else {
                                // As a browser does: an answer that is not a
                                // stream ends it; keep what came.
                                let note = tr!("stream.not_a_stream", status = status);
                                stream.borrow_mut().note(note);
                                let finished = stream.borrow_mut().stopped();
                                let tab = tab.clone();
                                this.update_in(cx, |this, window, cx| {
                                    this.finish_for(tab, Ok(finished), window, cx)
                                })
                                .ok();
                                return;
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(stream) = &live
                    && !taken.fresh.is_empty()
                {
                    let follow = at_bottom(&stream.borrow().scroll);
                    stream.borrow_mut().feed(&taken.fresh);
                    if follow {
                        stream.borrow().scroll.scroll_to_bottom();
                    }
                    this.update(cx, |_, cx| cx.notify()).ok();
                }
                let Some(result) = taken.result else {
                    continue;
                };
                // A stream that ended or dropped, with Reconnect on: wait the
                // server's retry time and connect again.
                let again = this
                    .read_with(cx, |this, _| this.sse_reconnect)
                    .unwrap_or(false);
                if again && let Some(stream) = &live {
                    if let Some(record) = record.take() {
                        let saved = result.clone();
                        cx.background_spawn(async move { record.save(&saved) })
                            .detach();
                    }
                    let (wait, last_id) = {
                        let mut s = stream.borrow_mut();
                        let wait = s.parser.reconnection_time();
                        let last_id = s.parser.last_id().map(str::to_owned);
                        let seconds = format!("{:.1}", wait.as_secs_f64());
                        let note = match &result {
                            Ok(_) => tr!("stream.ended_reconnecting", seconds = seconds),
                            Err(e) => tr!(
                                "stream.dropped_reconnecting",
                                seconds = seconds,
                                error = e.clone()
                            ),
                        };
                        s.note(note);
                        s.parser = s.parser.reconnected();
                        (wait, last_id)
                    };
                    this.update(cx, |_, cx| cx.notify()).ok();
                    cx.background_executor().timer(wait).await;
                    let next = apix_http::sse::reconnect_request(&request, last_id.as_deref());
                    let (next_incoming, next_running) = net::send_streaming(next);
                    incoming = next_incoming;
                    running = Box::new(next_running);
                    reconnecting = true;
                    continue;
                }
                let timed = live.map(|s| std::mem::take(&mut s.borrow_mut().events));
                let prepared = cx
                    .background_spawn(async move {
                        if let Some(record) = record {
                            record.save(&result);
                        }
                        result.map(Finished::prepare)
                    })
                    .await
                    .map(|mut finished| {
                        // Keep when each event arrived.
                        if let Some(timed) = timed
                            && finished.events.is_some()
                        {
                            finished.events = Some(timed);
                        }
                        finished
                    });
                let tab = tab.clone();
                this.update_in(cx, |this, window, cx| {
                    this.finish_for(tab, prepared, window, cx)
                })
                .ok();
                drop(running);
                return;
            }
        }));
        cx.notify();
    }

    /// Esc on an open stream: keep what arrived as the response.
    pub(super) fn stop_stream(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let ResponseState::Streaming(stream) = &self.response else {
            return false;
        };
        let stream = stream.clone();
        self.task = None;
        let finished = stream.borrow_mut().stopped();
        self.finish(Ok(finished), window, cx);
        true
    }

    /// Puts `state` in the tab of `path`, in front or in the back.
    fn set_response_for(
        &mut self,
        path: Option<PathBuf>,
        state: ResponseState,
        cx: &mut Context<Self>,
    ) {
        if path == self.selected {
            self.response = state;
        } else {
            self.set_parked_response(path, state);
        }
        cx.notify();
    }

    pub(super) fn render_stream(&self, stream: &LiveStream, cx: &mut Context<Self>) -> AnyElement {
        let p = crate::theme::current();
        let status = p.status(stream.status);
        let elapsed = stream.started.elapsed();
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
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_full()
                            .bg(status.opacity(0.15))
                            .text_color(status)
                            .font_weight(FontWeight::BOLD)
                            .child(stream.status.to_string()),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(div().size(px(7.)).rounded_full().bg(p.ok))
                            .child(tr!("stream.live")),
                    )
                    .child(tr!("stream.count", count = stream.events.len()))
                    .child(human_duration(elapsed))
                    .child(div().flex_1())
                    .child(
                        Checkbox::new("stream-reconnect")
                            .checked(self.sse_reconnect)
                            .label(tr!("stream.reconnect"))
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.sse_reconnect = *checked;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("stream-stop")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Square)
                            .label(tr!("stream.stop"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.stop_stream(window, cx);
                            })),
                    ),
            )
            .child(div().flex_1().min_h_0().p_2().child(self.render_events(
                &stream.events,
                &stream.notes,
                &stream.scroll,
                cx,
            )))
            .into_any_element()
    }

    /// The events, newest last: when, type, id, then the data; above them,
    /// the event types to narrow the list to one.
    pub(super) fn render_events(
        &self,
        events: &[TimedEvent],
        notes: &[Note],
        scroll: &ScrollHandle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = crate::theme::current();
        if events.is_empty() && notes.is_empty() {
            return div()
                .p_4()
                .text_sm()
                .text_color(p.text_muted)
                .child(tr!("stream.waiting"))
                .into_any_element();
        }
        let filter = self.event_filter.as_deref();
        let skipped = events.len().saturating_sub(MAX_SHOWN);
        let mut rows: Vec<AnyElement> = Vec::new();
        let note_row = |text: &SharedString| {
            div()
                .py_1()
                .text_xs()
                .text_color(p.text_muted)
                .child(format!("• {text}"))
                .into_any_element()
        };
        for (ix, (at, event)) in events.iter().enumerate().skip(skipped) {
            for (_, text) in notes.iter().filter(|(at_ix, _)| *at_ix == ix) {
                rows.push(note_row(text));
            }
            if filter.is_some_and(|f| f != event.event) {
                continue;
            }
            let id = event.id.as_deref().map(|id| format!("#{id}"));
            rows.push(
                v_flex()
                    .py_1()
                    .border_b_1()
                    .border_color(p.hairline)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .text_xs()
                            .children(at.map(|at| {
                                div()
                                    .w(px(56.))
                                    .flex_none()
                                    .text_color(p.text_muted)
                                    .child(format!("+{:.2}s", at.as_secs_f64()))
                            }))
                            .child(
                                div()
                                    .px_1p5()
                                    .rounded_sm()
                                    .bg(p.accent.opacity(0.14))
                                    .text_color(p.accent)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(event.event.clone()),
                            )
                            .children(id.map(|id| div().text_color(p.text_muted).child(id))),
                    )
                    .child(
                        div()
                            .pt_0p5()
                            .text_sm()
                            .text_color(p.text)
                            .font_family(cx.theme().mono_font_family.clone())
                            .child(event.data.clone()),
                    )
                    .into_any_element(),
            );
        }
        for (_, text) in notes.iter().filter(|(at_ix, _)| *at_ix >= events.len()) {
            rows.push(note_row(text));
        }
        // The types in the order they first came, with how many of each.
        let mut counts: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
        for (ix, (_, event)) in events.iter().enumerate() {
            counts.entry(event.event.as_str()).or_insert((ix, 0)).1 += 1;
        }
        let mut types: Vec<(&str, usize, usize)> = counts
            .into_iter()
            .map(|(t, (first, n))| (t, first, n))
            .collect();
        types.sort_by_key(|(_, first, _)| *first);
        let chip = |id: SharedString, label: String, on: bool, pick: Option<String>| {
            div()
                .id(id)
                .px_2()
                .py_0p5()
                .rounded_full()
                .cursor_pointer()
                .text_xs()
                .when(on, |d| d.bg(p.accent.opacity(0.18)).text_color(p.accent))
                .when(!on, |d| d.text_color(p.text_muted).hover(|s| s.bg(p.hover)))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.event_filter = pick.clone();
                    cx.notify();
                }))
        };
        let chips = (types.len() > 1).then(|| {
            h_flex()
                .dir_row()
                .flex_wrap()
                .gap_1()
                .pb_1()
                .child(chip(
                    "sse-all".into(),
                    tr!("stream.all_types", count = events.len()).to_string(),
                    filter.is_none(),
                    None,
                ))
                .children(types.iter().map(|(t, _, n)| {
                    chip(
                        format!("sse-type-{t}").into(),
                        format!("{t} {n}"),
                        filter == Some(*t),
                        Some((*t).to_owned()),
                    )
                }))
        });
        v_flex()
            .size_full()
            .children(chips)
            .child(
                v_flex()
                    .id("sse-events")
                    .ltr()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(scroll)
                    .when(skipped > 0, |d| {
                        d.child(
                            div()
                                .py_1()
                                .text_xs()
                                .text_color(p.text_muted)
                                .child(tr!("stream.older_hidden", count = skipped)),
                        )
                    })
                    .children(rows),
            )
            .into_any_element()
    }
}

/// Whether the list shows its end (so new events should keep it there).
fn at_bottom(scroll: &ScrollHandle) -> bool {
    let max = scroll.max_offset().y;
    max <= px(1.) || -scroll.offset().y >= max - px(24.)
}
