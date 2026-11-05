//! The Load test window (Request › Load test…): the open request sent by
//! many workers at once, with throughput, latency percentiles and a
//! second-by-second chart as it runs. The engine is `apix_runner::bench`,
//! the same as `apix bench`; scripts do not run in load tests.

use std::time::Duration;

use apix_core::Request;
use apix_runner::bench::{Bench, Plan, Summary};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{IconName, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::Workspace;
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::{net, tr};

/// Seconds shown in the chart.
const CHART_SECONDS: usize = 60;

impl Workspace {
    pub(super) fn open_load_test(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = self.build_request(cx);
        let request = self.with_inherited_auth(draft, self.selected.as_deref());
        let request = self.with_inherited_settings(request, self.selected.as_deref());
        let resolved = request.resolve(&self.vars);
        if resolved.request.url.trim().is_empty() {
            return self.notify_error(tr!("request.error.no_url"), window, cx);
        }
        if !resolved.missing.is_empty() {
            let names: Vec<_> = resolved.missing.into_iter().collect();
            return self.notify_error(tr!("load.undefined", names = names.join(", ")), window, cx);
        }
        let request = match self.body_location().0 {
            Some(dir) => resolved.request.with_files_relative_to(&dir),
            None => resolved.request,
        };
        let scheme = request.url.trim_start().to_ascii_lowercase();
        if request.is_grpc()
            || request.is_mqtt()
            || request.is_socketio()
            || scheme.starts_with("ws")
        {
            return self.notify_error(tr!("load.http_only"), window, cx);
        }
        let workspace = cx.entity().downgrade();
        let view = cx.new(|cx| LoadView::new(request, window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .on_close(super::collections::refocus(workspace.clone()))
                .bidi_title(tr!("load.title"))
                .width(px(860.))
                .child(div().pb_2().child(view.clone()))
        });
    }
}

pub(super) struct LoadView {
    request: Request,
    concurrency: Entity<InputState>,
    requests: Entity<InputState>,
    duration: Entity<InputState>,
    rate: Entity<InputState>,
    bench: Option<Bench>,
    summary: Option<Summary>,
    error: Option<String>,
    running: bool,
    _task: Option<Task<()>>,
    _clock: Option<Task<()>>,
}

impl Drop for LoadView {
    fn drop(&mut self) {
        // Closing the window ends the test.
        if let Some(bench) = &self.bench {
            bench.stop();
        }
    }
}

impl LoadView {
    fn new(request: Request, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = |value: &str,
                     placeholder: SharedString,
                     window: &mut Window,
                     cx: &mut Context<Self>| {
            let value = value.to_owned();
            cx.new(|cx| {
                let mut state = InputState::new(window, cx).placeholder(placeholder);
                state.set_value(value, window, cx);
                state
            })
        };
        Self {
            concurrency: field("10", "10".into(), window, cx),
            requests: field("", tr!("load.no_limit"), window, cx),
            duration: field("10s", "10s".into(), window, cx),
            rate: field("", tr!("load.no_limit"), window, cx),
            request,
            bench: None,
            summary: None,
            error: None,
            running: false,
            _task: None,
            _clock: None,
        }
    }

    fn plan(&self, cx: &App) -> Result<Plan, SharedString> {
        let text = |input: &Entity<InputState>| input.read(cx).value().trim().to_string();
        let concurrency = text(&self.concurrency)
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=10_000).contains(n))
            .ok_or_else(|| tr!("load.bad_concurrency"))?;
        let requests = match text(&self.requests) {
            t if t.is_empty() => None,
            t => Some(
                t.parse::<u64>()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| tr!("load.bad_requests"))?,
            ),
        };
        let duration = match text(&self.duration) {
            t if t.is_empty() => None,
            t => Some(
                apix_runner::bench::parse_duration(&t).ok_or_else(|| tr!("load.bad_duration"))?,
            ),
        };
        let rate = match text(&self.rate) {
            t if t.is_empty() => None,
            t => Some(
                t.parse::<f64>()
                    .ok()
                    .filter(|r| *r > 0.0)
                    .ok_or_else(|| tr!("load.bad_rate"))?,
            ),
        };
        Ok(Plan {
            concurrency,
            requests,
            duration: duration.or(if requests.is_none() {
                Some(Duration::from_secs(10))
            } else {
                None
            }),
            rate,
        })
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        let plan = match self.plan(cx) {
            Ok(plan) => plan,
            Err(e) => {
                self.error = Some(e.to_string());
                cx.notify();
                return;
            }
        };
        self.error = None;
        self.summary = None;
        let bench = Bench::new();
        self.bench = Some(bench.clone());
        self.running = true;
        let request = self.request.clone();
        let run = net::spawn(async move {
            let load = apix_http::load::Load::new(&request)
                .await
                .map_err(|e| e.to_string())?;
            Ok::<_, String>(bench.run(&load, &plan).await)
        });
        self._task = Some(cx.spawn(async move |this, cx| {
            let result = run.await.and_then(|r| r);
            this.update(cx, |this, cx| {
                this.running = false;
                match result {
                    Ok(summary) => this.summary = Some(summary),
                    Err(e) => this.error = Some(e),
                }
                cx.notify();
            })
            .ok();
        }));
        self._clock = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let more = this
                    .update(cx, |this, cx| {
                        if this.running
                            && let Some(bench) = &this.bench
                        {
                            this.summary = Some(bench.summary());
                        }
                        cx.notify();
                        this.running
                    })
                    .unwrap_or(false);
                if !more {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(bench) = &self.bench {
            bench.stop();
        }
        cx.notify();
    }

    fn render_chart(&self, summary: &Summary) -> impl IntoElement {
        let p = crate::theme::current();
        let seconds: Vec<_> = summary
            .seconds
            .iter()
            .rev()
            .take(CHART_SECONDS)
            .rev()
            .cloned()
            .collect();
        let most = seconds.iter().map(|s| s.requests).max().unwrap_or(1).max(1) as f32;
        let slowest = seconds
            .iter()
            .map(|s| s.mean_ms)
            .fold(0.0f64, f64::max)
            .max(0.001);
        let height = 120.0;
        let bars = seconds.iter().map(|s| {
            let total = s.requests as f32 / most;
            let failed = if s.requests == 0 {
                0.0
            } else {
                s.failed as f32 / s.requests as f32
            };
            let latency = (s.mean_ms / slowest) as f32;
            div()
                .flex_1()
                .min_w(px(2.))
                .h_full()
                .relative()
                .child(
                    v_flex()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right(px(1.))
                        .h(relative(total.max(0.01)))
                        .child(div().flex_1().bg(p.accent.opacity(0.75)))
                        .child(div().h(relative(failed)).bg(p.err)),
                )
                // The mean latency of that second, as a tick.
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right(px(1.))
                        .bottom(relative(latency.min(1.)))
                        .h(px(2.))
                        .bg(p.warn),
                )
        });
        v_flex()
            .gap_1()
            .child(
                h_flex()
                    .dir_row()
                    .gap_3()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(legend(
                        p.accent,
                        tr!("load.legend_requests", most = most as u64),
                    ))
                    .child(legend(p.err, tr!("load.legend_failed")))
                    .child(legend(
                        p.warn,
                        tr!("load.legend_latency", slowest = format!("{slowest:.1}")),
                    )),
            )
            .child(
                h_flex()
                    .h(px(height))
                    .items_end()
                    .p_1()
                    .rounded_md()
                    .border_1()
                    .border_color(p.hairline)
                    .children(bars),
            )
    }
}

fn legend(colour: Hsla, text: SharedString) -> impl IntoElement {
    h_flex()
        .gap_1()
        .items_center()
        .child(div().size(px(8.)).rounded_sm().bg(colour))
        .child(text)
}

fn tile(label: SharedString, value: String, colour: Option<Hsla>) -> impl IntoElement {
    let p = crate::theme::current();
    v_flex()
        .flex_1()
        .min_w(px(90.))
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(p.hairline)
        .child(div().text_xs().text_color(p.text_muted).child(label))
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colour.unwrap_or(p.text))
                .child(value),
        )
}

fn ms(value: f64) -> String {
    if value < 10.0 {
        format!("{value:.2} ms")
    } else {
        format!("{value:.0} ms")
    }
}

impl Render for LoadView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = window;
        let p = crate::theme::current();
        let label = |text: SharedString| div().text_xs().text_color(p.text_muted).child(text);
        let field = |input: &Entity<InputState>, width: f32| {
            div().w(px(width)).child(Input::new(input).small())
        };

        let controls = h_flex()
            .dir_row()
            .gap_2()
            .items_center()
            .flex_wrap()
            .child(label(tr!("load.concurrency")))
            .child(field(&self.concurrency, 70.))
            .child(label(tr!("load.requests")))
            .child(field(&self.requests, 90.))
            .child(label(tr!("load.duration")))
            .child(field(&self.duration, 70.))
            .child(label(tr!("load.rate")))
            .child(field(&self.rate, 90.))
            .child(div().flex_1())
            .child(if self.running {
                Button::new("load-stop")
                    .outline()
                    .small()
                    .icon(IconName::Square)
                    .label(tr!("mock.stop"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.stop(cx)))
            } else {
                Button::new("load-start")
                    .primary()
                    .small()
                    .icon(IconName::Play)
                    .label(tr!("load.start"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.start(cx)))
            });

        let results: AnyElement = match &self.summary {
            None => div()
                .text_sm()
                .text_color(p.text_muted)
                .child(if self.running {
                    tr!("load.starting")
                } else {
                    tr!("load.idle")
                })
                .into_any_element(),
            Some(s) => {
                let failed_colour = (s.failed > 0).then_some(p.err);
                let statuses: Vec<String> = s
                    .statuses
                    .iter()
                    .map(|(code, n)| format!("{code} × {n}"))
                    .collect();
                v_flex()
                    .gap_3()
                    .child(
                        h_flex()
                            .dir_row()
                            .gap_2()
                            .flex_wrap()
                            .child(tile(tr!("load.total"), s.requests.to_string(), None))
                            .child(tile(
                                tr!("load.per_second"),
                                format!("{:.1}", s.per_second),
                                None,
                            ))
                            .child(tile(
                                tr!("load.failed"),
                                s.failed.to_string(),
                                failed_colour,
                            ))
                            .child(tile("p50".into(), ms(s.p50_ms), None))
                            .child(tile("p95".into(), ms(s.p95_ms), None))
                            .child(tile("p99".into(), ms(s.p99_ms), None))
                            .child(tile(tr!("load.max"), ms(s.max_ms), None)),
                    )
                    .child(self.render_chart(s))
                    .child(div().text_xs().text_color(p.text_muted).child(tr!(
                        "load.details",
                        elapsed = format!("{:.1}", s.elapsed_ms / 1000.0),
                        min = ms(s.min_ms),
                        mean = ms(s.mean_ms),
                        p90 = ms(s.p90_ms),
                        statuses = if statuses.is_empty() {
                            "–".to_owned()
                        } else {
                            statuses.join(", ")
                        }
                    )))
                    .children(s.errors.iter().map(|(error, n)| {
                        div()
                            .text_xs()
                            .text_color(p.err)
                            .child(format!("{error} × {n}"))
                    }))
                    .into_any_element()
            }
        };

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .text_sm()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.request.method.as_str().to_owned()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(crate::fonts::MONO)
                            .child(self.request.url.clone()),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(bidi::para(tr!("load.about"))),
            )
            .child(controls)
            .children(
                self.error
                    .clone()
                    .map(|e| div().text_sm().text_color(p.err).child(e)),
            )
            .child(results)
            .when(self.running, |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(tr!("load.running")),
                )
            })
    }
}
