//! Server-sent events in the response pane: while the stream is open, its
//! events show up as they arrive (Esc stops it and keeps what came); once it
//! has ended, the Pretty tab lists them. Other responses are untouched.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use apix_http::sse::{SseEvent, SseParser};
use apix_http::{Response, Timing, human_duration};
use gpui_kit::component::button::{Button, ButtonVariants as _};
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

/// A stream that is still open.
pub(super) struct LiveStream {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub started: Instant,
    /// When the status and headers came in.
    pub head_at: Duration,
    pub events: Vec<TimedEvent>,
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
        let started = Instant::now();
        let (incoming, running) = net::send_streaming(request);
        // The tab may be in the back by the time anything arrives.
        let tab = self.selected.clone();
        // Replacing an older task drops it, which cancels that request.
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let _running = running;
            let mut live: Option<Rc<RefCell<LiveStream>>> = None;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                let (head, fresh, result) = {
                    let mut incoming = incoming.lock().unwrap_or_else(|e| e.into_inner());
                    let head = (live.is_none() && incoming.event_stream)
                        .then(|| incoming.head.clone())
                        .flatten();
                    (
                        head,
                        std::mem::take(&mut incoming.fresh),
                        incoming.result.take(),
                    )
                };
                if let Some((status, headers)) = head {
                    let stream = Rc::new(RefCell::new(LiveStream::new(status, headers, started)));
                    live = Some(stream.clone());
                    let tab = tab.clone();
                    this.update(cx, |this, cx| {
                        this.set_response_for(tab, ResponseState::Streaming(stream), cx)
                    })
                    .ok();
                }
                if let Some(stream) = &live
                    && !fresh.is_empty()
                {
                    let follow = at_bottom(&stream.borrow().scroll);
                    stream.borrow_mut().feed(&fresh);
                    if follow {
                        stream.borrow().scroll.scroll_to_bottom();
                    }
                    this.update(cx, |_, cx| cx.notify()).ok();
                }
                let Some(result) = result else {
                    continue;
                };
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
        let mut finished = Finished::prepare(stream.borrow().to_response());
        finished.events = Some(std::mem::take(&mut stream.borrow_mut().events));
        finished.stopped = true;
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
            .child(div().flex_1().min_h_0().p_2().child(render_events(
                &stream.events,
                &stream.scroll,
                cx,
            )))
            .into_any_element()
    }
}

/// Whether the list shows its end (so new events should keep it there).
fn at_bottom(scroll: &ScrollHandle) -> bool {
    let max = scroll.max_offset().y;
    max <= px(1.) || -scroll.offset().y >= max - px(24.)
}

/// The events, newest last: when, type, id, then the data.
pub(super) fn render_events(events: &[TimedEvent], scroll: &ScrollHandle, cx: &App) -> AnyElement {
    let p = crate::theme::current();
    if events.is_empty() {
        return div()
            .p_4()
            .text_sm()
            .text_color(p.text_muted)
            .child(tr!("stream.waiting"))
            .into_any_element();
    }
    let skipped = events.len().saturating_sub(MAX_SHOWN);
    let rows = events[skipped..].iter().map(|(at, event)| {
        let id = event.id.as_deref().map(|id| format!("#{id}"));
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
    });
    v_flex()
        .id("sse-events")
        .ltr()
        .size_full()
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
        .children(rows)
        .into_any_element()
}
