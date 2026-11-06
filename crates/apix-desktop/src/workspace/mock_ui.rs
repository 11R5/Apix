//! The mock server (View › Mock server…): serves the open collection's saved
//! examples on a local port, lists what it serves and what it was asked. The
//! server is `apix-mock`'s, the same as `apix mock`; it keeps running while
//! the window is closed, until stopped or the app quits.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use apix_mock::{Hit, Route};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{IconName, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use super::Workspace;
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::{net, tr};

/// Requests kept in the log.
const KEEP: usize = 200;

type Server = Pin<Box<dyn Future<Output = Result<std::io::Result<()>, String>> + Send>>;

/// A running server.
struct Running {
    root: PathBuf,
    port: u16,
    /// Dropping it stops the server.
    _server: Server,
}

#[derive(Default)]
pub(super) struct MockState {
    running: Option<Running>,
    /// Newest last.
    hits: Arc<Mutex<Vec<Hit>>>,
}

impl Workspace {
    pub(super) fn open_mock(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(collection) = self.collection.clone() else {
            self.notify_error(
                tr!("collection.open_first", key = super::key_hint("O")),
                window,
                cx,
            );
            return;
        };
        let workspace = cx.entity().downgrade();
        let view = cx.new(|cx| MockView::new(workspace.clone(), collection.root, window, cx));
        super::tour::keep(&view);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .on_close(super::collections::refocus(workspace.clone()))
                .bidi_title(tr!("mock.title"))
                .width(px(760.))
                .child(div().pb_2().child(view.clone()))
        });
    }

    fn start_mock(
        &mut self,
        root: PathBuf,
        port: u16,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mock.running = None;
        let listener = match std::net::TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => listener,
            Err(e) => {
                self.notify_error(
                    tr!("mock.cannot_listen", port = port, error = e.to_string()),
                    window,
                    cx,
                );
                return;
            }
        };
        let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
        let _ = listener.set_nonblocking(true);
        let hits = self.mock.hits.clone();
        let serve_root = root.clone();
        let server = net::spawn(async move {
            let listener = tokio::net::TcpListener::from_std(listener)?;
            apix_mock::serve(listener, serve_root, Default::default(), move |hit| {
                let mut hits = hits.lock().unwrap_or_else(|e| e.into_inner());
                hits.push(hit);
                let extra = hits.len().saturating_sub(KEEP);
                hits.drain(..extra);
            })
            .await
        });
        self.mock.running = Some(Running {
            root,
            port,
            _server: Box::pin(server),
        });
        cx.notify();
    }

    fn stop_mock(&mut self, cx: &mut Context<Self>) {
        self.mock.running = None;
        cx.notify();
    }
}

pub(super) struct MockView {
    workspace: WeakEntity<Workspace>,
    root: PathBuf,
    port: Entity<InputState>,
    routes: Vec<Route>,
    _clock: Task<()>,
}

impl MockView {
    fn new(
        workspace: WeakEntity<Workspace>,
        root: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let port = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("4010");
            state.set_value("4010", window, cx);
            state
        });
        // The routes and the log are read again twice a second.
        let clock = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        this.routes = apix_mock::load(&this.root);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            routes: apix_mock::load(&root),
            workspace,
            root,
            port,
            _clock: clock,
        }
    }
}

impl Render for MockView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = window;
        let p = crate::theme::current();
        let Some(workspace) = self.workspace.upgrade() else {
            return div().into_any_element();
        };
        let (running, hits) = {
            let ws = workspace.read(cx);
            let running = ws.mock.running.as_ref().map(|r| (r.root.clone(), r.port));
            let hits = ws
                .mock
                .hits
                .lock()
                .map(|h| h.iter().rev().take(50).cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            (running, hits)
        };
        let label = |text: SharedString| div().text_xs().text_color(p.text_muted).child(text);
        let mono = |text: String| div().text_xs().font_family(crate::fonts::MONO).child(text);

        let control = match &running {
            Some((root, port)) => {
                let other = *root != self.root;
                let ws = workspace.downgrade();
                h_flex()
                    .dir_row()
                    .gap_3()
                    .items_center()
                    .child(div().size(px(9.)).rounded_full().bg(p.ok))
                    .child(div().flex_1().text_sm().child(bidi::para(if other {
                        tr!(
                            "mock.running_other",
                            url = format!("http://127.0.0.1:{port}")
                        )
                    } else {
                        tr!("mock.running", url = format!("http://127.0.0.1:{port}"))
                    })))
                    .child(
                        Button::new("mock-stop")
                            .outline()
                            .small()
                            .icon(IconName::Square)
                            .label(tr!("mock.stop"))
                            .on_click(move |_, _, cx| {
                                ws.update(cx, |ws, cx| ws.stop_mock(cx)).ok();
                            }),
                    )
            }
            None => {
                let ws = workspace.downgrade();
                let root = self.root.clone();
                let port_input = self.port.clone();
                h_flex()
                    .dir_row()
                    .gap_2()
                    .items_center()
                    .child(label(tr!("mock.port")))
                    .child(div().w(px(90.)).child(Input::new(&self.port).small()))
                    .child(
                        Button::new("mock-start")
                            .primary()
                            .small()
                            .icon(IconName::Play)
                            .label(tr!("mock.start"))
                            .on_click(move |_, window, cx| {
                                let port = port_input
                                    .read(cx)
                                    .value()
                                    .trim()
                                    .parse::<u16>()
                                    .unwrap_or(4010);
                                let root = root.clone();
                                ws.update(cx, |ws, cx| ws.start_mock(root, port, window, cx))
                                    .ok();
                            }),
                    )
            }
        };

        let routes: Vec<AnyElement> = self
            .routes
            .iter()
            .map(|r| {
                h_flex()
                    .dir_row()
                    .gap_3()
                    .py(px(2.))
                    .child(
                        div()
                            .w(px(60.))
                            .flex_none()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(crate::theme::method_color(r.method))
                            .child(r.method.as_str()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(mono(r.path.clone())),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(format!(
                                "{} · {} ({})",
                                r.request, r.example.name, r.example.status
                            )),
                    )
                    .into_any_element()
            })
            .collect();
        let log: Vec<AnyElement> = hits
            .iter()
            .map(|h| {
                let colour = if h.matched.is_some() { p.ok } else { p.err };
                h_flex()
                    .dir_row()
                    .gap_3()
                    .py(px(1.))
                    .child(
                        div()
                            .w(px(60.))
                            .flex_none()
                            .text_xs()
                            .child(h.method.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(mono(h.path.clone())),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_xs()
                            .text_color(colour)
                            .child(h.status.to_string()),
                    )
                    .child(
                        div()
                            .w(px(220.))
                            .flex_none()
                            .truncate()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(
                                h.matched
                                    .as_ref()
                                    .map(|(r, e)| format!("{r} · {e}"))
                                    .unwrap_or_else(|| tr!("mock.no_example").into()),
                            ),
                    )
                    .into_any_element()
            })
            .collect();

        v_flex()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(bidi::para(tr!("mock.about"))),
            )
            .child(control)
            .child(
                div()
                    .pt_1()
                    .child(label(tr!("mock.routes", count = self.routes.len()))),
            )
            .child(if routes.is_empty() {
                div()
                    .text_sm()
                    .text_color(p.text_muted)
                    .child(bidi::para(tr!("mock.no_routes")))
                    .into_any_element()
            } else {
                div()
                    .id("mock-routes")
                    .max_h(px(200.))
                    .overflow_y_scroll()
                    .children(routes)
                    .into_any_element()
            })
            .child(div().pt_1().child(label(tr!("mock.log"))))
            .child(if log.is_empty() {
                div()
                    .text_sm()
                    .text_color(p.text_muted)
                    .child(tr!("mock.log_empty"))
                    .into_any_element()
            } else {
                div()
                    .id("mock-log")
                    .max_h(px(200.))
                    .overflow_y_scroll()
                    .children(log)
                    .into_any_element()
            })
            .into_any_element()
    }
}

impl Workspace {
    /// The documentation's screenshot tour: starts the mock server of the
    /// open collection on `port`.
    pub(super) fn tour_mock(&mut self, port: u16, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(collection) = self.collection.clone() {
            self.start_mock(collection.root, port, window, cx);
        }
    }
}
