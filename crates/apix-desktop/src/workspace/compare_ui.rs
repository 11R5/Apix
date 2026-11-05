//! The Compare window (⇄ next to the response, or Request › Compare
//! responses…): the response now against an earlier one of the same request
//! from the history ("time travel"), or against the same request sent in
//! another environment. `apix_core::compare` does the comparing.

use apix_core::Request;
use apix_core::compare::{self, BodyDiff, Comparison, Snapshot};
use apix_core::diff::Line;
use apix_core::history::{History, Summary};
use apix_core::project;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{ResponseState, Workspace};
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::{net, tr};

impl Workspace {
    pub(super) fn open_compare(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ResponseState::Done(done) = &self.response else {
            return self.notify_error(tr!("compare.send_first"), window, cx);
        };
        let response = &done.response;
        let now = Snapshot {
            label: tr!("compare.now").to_string(),
            status: response.status,
            headers: response.headers.clone(),
            body: response.body.to_vec(),
            total_ms: response.timing.total.as_secs_f64() * 1000.0,
        };
        let draft = self.build_request(cx);
        let (method, url) = (draft.method, draft.url.clone());
        // The same request in every other environment, ready to send.
        let mut others = Vec::new();
        for env in self.environment_names() {
            if Some(&env) == self.environment.as_ref() {
                continue;
            }
            let request =
                project::variables(self.project.as_ref(), self.collection.as_ref(), Some(&env))
                    .map_err(|e| e.to_string())
                    .and_then(|vars| {
                        let request =
                            self.with_inherited_auth(draft.clone(), self.selected.as_deref());
                        let request =
                            self.with_inherited_settings(request, self.selected.as_deref());
                        let resolved = request.resolve(&vars);
                        if resolved.missing.is_empty() {
                            Ok(match self.body_location().0 {
                                Some(dir) => resolved.request.with_files_relative_to(&dir),
                                None => resolved.request,
                            })
                        } else {
                            let names: Vec<_> = resolved.missing.into_iter().collect();
                            Err(tr!("load.undefined", names = names.join(", ")).to_string())
                        }
                    });
            others.push((env, request));
        }
        let path = self.selected.clone();
        let earlier: Vec<Summary> = history_store()
            .map(|h| {
                h.list()
                    .into_iter()
                    .filter(|s| match &path {
                        Some(path) => s.path.as_ref() == Some(path),
                        None => s.method == method && s.url == url,
                    })
                    .filter(|s| s.status.is_some())
                    .take(30)
                    .collect()
            })
            .unwrap_or_default();
        let workspace = cx.entity().downgrade();
        let view = cx.new(|_| CompareView {
            now,
            earlier,
            others,
            picked: None,
            comparison: None,
            busy: false,
            _task: None,
        });
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .on_close(super::collections::refocus(workspace.clone()))
                .bidi_title(tr!("compare.title"))
                .width(px(1000.))
                .child(div().pb_2().child(view.clone()))
        });
    }
}

fn history_store() -> Option<History> {
    Some(History::new(
        dirs::config_dir()?.join("Apix").join("history"),
    ))
}

pub(super) struct CompareView {
    now: Snapshot,
    earlier: Vec<Summary>,
    others: Vec<(String, Result<Request, String>)>,
    /// Which choice is shown: `h<id>` or `e<environment>`.
    picked: Option<String>,
    comparison: Option<Result<Comparison, String>>,
    busy: bool,
    _task: Option<Task<()>>,
}

impl CompareView {
    fn pick_history(&mut self, id: String, label: String, cx: &mut Context<Self>) {
        self.picked = Some(format!("h{id}"));
        let now = self.now.clone();
        let entry = history_store().and_then(|h| h.load(&id));
        self.comparison = Some(match entry.and_then(|e| Some((e.response?, e.body))) {
            Some((response, body)) => {
                let then = Snapshot {
                    label,
                    status: response.status,
                    headers: response.headers,
                    body,
                    total_ms: response.total_us as f64 / 1000.0,
                };
                Ok(compare::compare(&then, &now))
            }
            None => Err(tr!("compare.gone").to_string()),
        });
        cx.notify();
    }

    fn pick_environment(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some((env, request)) = self.others.get(ix).cloned() else {
            return;
        };
        self.picked = Some(format!("e{env}"));
        let request = match request {
            Ok(r) => r,
            Err(e) => {
                self.comparison = Some(Err(e));
                cx.notify();
                return;
            }
        };
        self.busy = true;
        self.comparison = None;
        let now = self.now.clone();
        let sent = net::send(request);
        self._task = Some(cx.spawn(async move |this, cx| {
            let result = sent.await.map(|r| {
                let other = Snapshot {
                    label: env.clone(),
                    status: r.status,
                    headers: r.headers.clone(),
                    body: r.body.to_vec(),
                    total_ms: r.timing.total.as_secs_f64() * 1000.0,
                };
                compare::compare(&other, &now)
            });
            this.update(cx, |this, cx| {
                this.busy = false;
                this.comparison = Some(result);
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn render_comparison(&self) -> AnyElement {
        let p = crate::theme::current();
        let muted = |text: SharedString| div().text_sm().text_color(p.text_muted).child(text);
        if self.busy {
            return muted(tr!("compare.sending")).into_any_element();
        }
        let Some(result) = &self.comparison else {
            return muted(tr!("compare.pick")).into_any_element();
        };
        let c = match result {
            Ok(c) => c,
            Err(e) => {
                return div()
                    .text_sm()
                    .text_color(p.err)
                    .child(e.clone())
                    .into_any_element();
            }
        };
        let mono = |text: String, colour: Hsla| {
            div()
                .text_xs()
                .font_family(crate::fonts::MONO)
                .text_color(colour)
                .child(text)
        };
        let heading = |text: SharedString| {
            div()
                .pt_2()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.text_muted)
                .child(crate::i18n::upper(&text))
        };
        let summary = if c.is_same() {
            tr!("compare.same")
        } else {
            tr!("compare.differences", count = c.count())
        };
        let mut body = v_flex()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if c.is_same() { p.ok } else { p.warn })
                    .child(summary),
            )
            .child(div().text_xs().text_color(p.text_muted).child(tr!(
                "compare.sides",
                old = c.old_label.clone(),
                new = c.new_label.clone(),
                old_ms = format!("{:.0}", c.time_ms.0),
                new_ms = format!("{:.0}", c.time_ms.1),
                old_size = c.size.0,
                new_size = c.size.1
            )));
        if c.status.0 != c.status.1 {
            body = body.child(heading(tr!("compare.status"))).child(
                h_flex()
                    .gap_2()
                    .pl_2()
                    .child(mono(c.status.0.to_string(), p.err))
                    .child(mono("→".into(), p.text_muted))
                    .child(mono(c.status.1.to_string(), p.ok)),
            );
        }
        if !c.headers.is_empty() {
            body = body.child(heading(tr!("compare.headers")));
            let none = tr!("git.none").to_string();
            for h in &c.headers {
                body = body.child(
                    h_flex()
                        .pl_2()
                        .gap_2()
                        .flex_wrap()
                        .child(mono(format!("{}:", h.name), p.text))
                        .child(mono(h.old.clone().unwrap_or_else(|| none.clone()), p.err))
                        .child(mono("→".into(), p.text_muted))
                        .child(mono(h.new.clone().unwrap_or_else(|| none.clone()), p.ok)),
                );
            }
        }
        match &c.body {
            BodyDiff::Same => {}
            BodyDiff::Json(fields) => {
                body = body.child(heading(tr!("compare.body")));
                for field in fields.iter().take(400) {
                    let mut shown = field.clone();
                    shown.field = compare::field(&field.field).to_owned();
                    body = body.child(super::git_panel::field_row(&shown));
                }
                if fields.len() > 400 {
                    body = body.child(muted(tr!("compare.more", count = fields.len() - 400)));
                }
            }
            BodyDiff::Text(lines) => {
                body = body.child(heading(tr!("compare.body")));
                for line in super::git_panel::context(lines).into_iter().take(600) {
                    body = body.child(match line {
                        None => mono("…".into(), p.text_muted),
                        Some(Line::Same(t)) => mono(format!("  {t}"), p.text_muted),
                        Some(Line::Removed(t)) => mono(format!("- {t}"), p.err),
                        Some(Line::Added(t)) => mono(format!("+ {t}"), p.ok),
                    });
                }
            }
            BodyDiff::Binary => {
                body = body
                    .child(heading(tr!("compare.body")))
                    .child(muted(tr!("compare.binary")));
            }
        }
        if !c.ignored.is_empty() {
            body = body.child(
                div()
                    .pt_2()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(tr!("compare.ignored", names = c.ignored.join(", "))),
            );
        }
        body.into_any_element()
    }
}

impl Render for CompareView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = window;
        let p = crate::theme::current();
        let picked = self.picked.clone();
        let label = |text: SharedString| {
            div()
                .pt_2()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.text_muted)
                .child(crate::i18n::upper(&text))
        };
        let row = |id: ElementId, selected: bool| {
            h_flex()
                .id(id)
                .gap_2()
                .px_2()
                .py(px(3.))
                .rounded_md()
                .cursor_pointer()
                .text_xs()
                .when(selected, |d| d.bg(p.accent.opacity(0.14)))
                .hover(|s| s.bg(p.hover))
        };
        let history = self.earlier.iter().enumerate().map(|(i, s)| {
            let when = super::history_ui::ago(s.at).to_string();
            let key = format!("h{}", s.id);
            let (id, label) = (s.id.clone(), when.clone());
            let colour = match s.status {
                Some(code) if code < 400 => p.ok,
                _ => p.err,
            };
            row(
                ("compare-h", i).into(),
                picked.as_deref() == Some(key.as_str()),
            )
            .child(div().flex_1().child(when))
            .child(
                div()
                    .text_color(colour)
                    .child(s.status.map(|c| c.to_string()).unwrap_or_default()),
            )
            .child(
                div()
                    .w(px(56.))
                    .text_end()
                    .text_color(p.text_muted)
                    .child(format!("{} ms", s.duration_ms)),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.pick_history(id.clone(), label.clone(), cx)
            }))
        });
        let environments = self.others.iter().enumerate().map(|(i, (env, _))| {
            let key = format!("e{env}");
            row(
                ("compare-e", i).into(),
                picked.as_deref() == Some(key.as_str()),
            )
            .child(div().flex_1().child(env.clone()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_environment(i, cx)))
        });
        let left = v_flex()
            .id("compare-choices")
            .w(px(260.))
            .flex_none()
            .h(px(460.))
            .overflow_y_scroll()
            .child(label(tr!("compare.earlier")))
            .children(history)
            .when(self.earlier.is_empty(), |d| {
                d.child(
                    div()
                        .px_2()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(tr!("compare.no_history")),
                )
            })
            .child(label(tr!("compare.environments")))
            .children(environments)
            .when(self.others.is_empty(), |d| {
                d.child(
                    div()
                        .px_2()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(tr!("compare.no_environments")),
                )
            });
        let right = div()
            .id("compare-result")
            .flex_1()
            .min_w_0()
            .h(px(460.))
            .overflow_y_scroll()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(p.hairline)
            .child(self.render_comparison());
        v_flex()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(bidi::para(tr!("compare.about"))),
            )
            .child(
                h_flex()
                    .dir_row()
                    .gap_3()
                    .items_start()
                    .child(left)
                    .child(right),
            )
            .child(
                h_flex().dir_row().justify_end().child(
                    Button::new("compare-copy")
                        .small()
                        .ghost()
                        .label(tr!("compare.copy"))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(Ok(c)) = &this.comparison {
                                cx.write_to_clipboard(ClipboardItem::new_string(c.to_text()));
                            }
                        })),
                ),
            )
    }
}
