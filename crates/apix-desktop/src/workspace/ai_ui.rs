//! The optional AI assist next to the response (Settings › AI assist):
//! **AI tests** asks Claude for a Tests script that fits the response and
//! adds it to the request's Tests; **Explain** asks why a failed response
//! failed. Uses the user's own Anthropic API key from the keychain;
//! `apix_core::ai` builds the request (credentials masked) and reads the
//! answer.

use apix_core::Body;
use apix_core::ai::{self, Task};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{WindowExt as _, v_flex};
use gpui_kit::*;

use super::{ResponseState, Workspace};
use crate::bidi::{self, DialogBidi as _};
use crate::{net, tr};

impl Workspace {
    /// The exchange on screen, for the prompt.
    fn ai_context(&self, cx: &App) -> Option<ai::Context> {
        let ResponseState::Done(done) = &self.response else {
            return None;
        };
        let request = self.build_request(cx).resolve(&self.vars).request;
        let request_body = match &request.body {
            Body::Json(s) | Body::Text(s) | Body::Xml(s) | Body::Html(s) => s.clone(),
            Body::GraphQl(g) => g.query.clone(),
            _ => String::new(),
        };
        let r = &done.response;
        Some(ai::Context {
            method: request.method.as_str().to_owned(),
            url: request.url.clone(),
            request_headers: request
                .headers
                .iter()
                .filter(|h| h.enabled)
                .map(|h| (h.key.clone(), h.value.clone()))
                .collect(),
            request_body,
            status: r.status,
            response_headers: r.headers.clone(),
            response_body: String::from_utf8_lossy(&r.body).into_owned(),
            time_ms: r.timing.total.as_secs_f64() * 1000.0,
        })
    }

    /// The question and the key, or a notice saying what is missing.
    fn ai_request(
        &self,
        task: Task,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<apix_core::Request> {
        let Some(context) = self.ai_context(cx) else {
            self.notify_error(tr!("compare.send_first"), window, cx);
            return None;
        };
        let Some(key) = ai::load_key() else {
            self.notify_error(tr!("ai.no_key"), window, cx);
            return None;
        };
        Some(ai::request(
            task,
            &context,
            &key,
            &self.settings.ai_model,
            crate::i18n::current().code,
        ))
    }

    pub(super) fn ai_write_tests(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(request) = self.ai_request(Task::WriteTests, window, cx) else {
            return;
        };
        window.push_notification(Notification::info(tr!("ai.asking")), cx);
        let sent = net::send(request);
        cx.spawn_in(window, async move |this, cx| {
            let result = sent.await.and_then(|r| ai::answer(r.status, &r.body));
            this.update_in(cx, |this, window, cx| match result {
                Ok(text) => {
                    let script = ai::script(&text);
                    this.script_editor.update(cx, |editor, cx| {
                        editor.append_test(
                            &format!(
                                "// Written by Claude; check it before you keep it.\n{script}"
                            ),
                            window,
                            cx,
                        )
                    });
                    window.push_notification(Notification::success(tr!("ai.tests_added")), cx);
                    cx.notify();
                }
                Err(e) => this.notify_error(e, window, cx),
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn ai_explain(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(request) = self.ai_request(Task::Explain, window, cx) else {
            return;
        };
        let view = cx.new(|_| ExplainView { answer: None });
        let sent = net::send(request);
        let shown = view.clone();
        cx.spawn(async move |_, cx| {
            let result = sent.await.and_then(|r| ai::answer(r.status, &r.body));
            shown.update(cx, |view, cx| {
                view.answer = Some(result);
                cx.notify();
            });
        })
        .detach();
        let workspace = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .on_close(super::collections::refocus(workspace.clone()))
                .bidi_title(tr!("ai.explain_title"))
                .width(px(720.))
                .child(div().pb_2().child(view.clone()))
        });
    }
}

struct ExplainView {
    answer: Option<Result<String, String>>,
}

impl Render for ExplainView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let body: AnyElement = match &self.answer {
            None => div()
                .text_sm()
                .text_color(p.text_muted)
                .child(tr!("ai.asking"))
                .into_any_element(),
            Some(Err(e)) => div()
                .text_sm()
                .text_color(p.err)
                .child(e.clone())
                .into_any_element(),
            Some(Ok(text)) => v_flex()
                .gap_2()
                .children(
                    text.split("\n\n")
                        .map(|para| div().text_sm().child(bidi::para(para.trim().to_owned()))),
                )
                .into_any_element(),
        };
        v_flex()
            .gap_3()
            .child(
                div()
                    .id("ai-explain")
                    .max_h(px(480.))
                    .overflow_y_scroll()
                    .child(body),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(tr!("ai.note")),
            )
    }
}
