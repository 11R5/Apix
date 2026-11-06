//! The capture proxy (View › Capture proxy…): requests that pass through a
//! local proxy are saved into the open collection's `captured` folder, each
//! with its answer as an example. The proxy is `apix-capture`'s, the same as
//! `apix capture`; it keeps running while the window is closed.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use apix_capture::{Authority, Event, Options};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{IconName, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use super::Workspace;
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::{net, tr};

const KEEP: usize = 200;

type Server = Pin<Box<dyn Future<Output = Result<std::io::Result<()>, String>> + Send>>;

/// One line of the log.
#[derive(Clone)]
struct Line {
    method: String,
    url: String,
    status: Option<u16>,
    note: String,
}

struct Running {
    port: u16,
    folder: PathBuf,
    https: bool,
    _server: Server,
}

#[derive(Default)]
pub(super) struct CaptureState {
    running: Option<Running>,
    lines: Arc<Mutex<Vec<Line>>>,
}

impl Workspace {
    pub(super) fn open_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(collection) = self.collection.clone() else {
            self.notify_error(
                tr!("collection.open_first", key = super::key_hint("O")),
                window,
                cx,
            );
            return;
        };
        let workspace = cx.entity().downgrade();
        let folder = collection.root.join("captured");
        let view = cx.new(|cx| CaptureView::new(workspace.clone(), folder, window, cx));
        super::tour::keep(&view);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .on_close(super::collections::refocus(workspace.clone()))
                .bidi_title(tr!("capture.title"))
                .width(px(760.))
                .child(div().pb_2().child(view.clone()))
        });
    }

    fn start_capture(
        &mut self,
        folder: PathBuf,
        port: u16,
        https: bool,
        hosts: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.capture.running = None;
        let authority = if https {
            match apix_capture::default_dir()
                .ok_or_else(|| std::io::Error::other("no configuration folder"))
                .and_then(|dir| Authority::load_or_create(&dir))
            {
                Ok(a) => Some(Arc::new(a)),
                Err(e) => return self.notify_error(e.to_string(), window, cx),
            }
        } else {
            None
        };
        let listener = match std::net::TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => listener,
            Err(e) => {
                return self.notify_error(
                    tr!("mock.cannot_listen", port = port, error = e.to_string()),
                    window,
                    cx,
                );
            }
        };
        let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
        let _ = listener.set_nonblocking(true);
        let lines = self.capture.lines.clone();
        let into = folder.clone();
        let server = net::spawn(async move {
            let listener = tokio::net::TcpListener::from_std(listener)?;
            let options = Options {
                hosts,
                open_https: authority,
                upstream_ca: None,
            };
            apix_capture::run(listener, options, move |event| {
                let line = match event {
                    Event::Exchange(exchange) => {
                        let note = match apix_capture::save(&exchange, &into) {
                            Ok(path) => path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                            Err(e) => e.to_string(),
                        };
                        Line {
                            method: exchange.request.method.as_str().to_owned(),
                            url: apix_core::display_url(&exchange.request.url),
                            status: Some(exchange.status),
                            note,
                        }
                    }
                    Event::Tunnel { host } => Line {
                        method: "CONNECT".into(),
                        url: host,
                        status: None,
                        note: String::new(),
                    },
                    Event::Failed { url, error } => Line {
                        method: String::new(),
                        url,
                        status: None,
                        note: error,
                    },
                };
                let mut lines = lines.lock().unwrap_or_else(|e| e.into_inner());
                lines.push(line);
                let extra = lines.len().saturating_sub(KEEP);
                lines.drain(..extra);
            })
            .await
        });
        self.capture.running = Some(Running {
            port,
            folder,
            https,
            _server: Box::pin(server),
        });
        cx.notify();
    }

    fn stop_capture(&mut self, cx: &mut Context<Self>) {
        self.capture.running = None;
        cx.notify();
    }
}

pub(super) struct CaptureView {
    workspace: WeakEntity<Workspace>,
    folder: PathBuf,
    port: Entity<InputState>,
    hosts: Entity<InputState>,
    https: bool,
    _clock: Task<()>,
}

impl CaptureView {
    fn new(
        workspace: WeakEntity<Workspace>,
        folder: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let port = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("8888");
            state.set_value("8888", window, cx);
            state
        });
        let hosts =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr!("capture.hosts_placeholder")));
        let clock = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        Self {
            workspace,
            folder,
            port,
            hosts,
            https: false,
            _clock: clock,
        }
    }
}

impl Render for CaptureView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = window;
        let p = crate::theme::current();
        let Some(workspace) = self.workspace.upgrade() else {
            return div().into_any_element();
        };
        let (running, lines) = {
            let ws = workspace.read(cx);
            let running = ws
                .capture
                .running
                .as_ref()
                .map(|r| (r.port, r.folder.clone(), r.https));
            let lines = ws
                .capture
                .lines
                .lock()
                .map(|l| l.iter().rev().take(60).cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            (running, lines)
        };
        let label = |text: SharedString| div().text_xs().text_color(p.text_muted).child(text);
        let mono = |text: String| div().text_xs().font_family(crate::fonts::MONO).child(text);

        let controls = match &running {
            Some((port, folder, https)) => {
                let ws = workspace.downgrade();
                let address = format!("http://127.0.0.1:{port}");
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .dir_row()
                            .gap_3()
                            .items_center()
                            .child(div().size(px(9.)).rounded_full().bg(p.ok))
                            .child(
                                div().flex_1().text_sm().child(bidi::para(tr!(
                                    "capture.running",
                                    url = address.clone()
                                ))),
                            )
                            .child(
                                Button::new("capture-stop")
                                    .outline()
                                    .small()
                                    .icon(IconName::Square)
                                    .label(tr!("mock.stop"))
                                    .on_click(move |_, _, cx| {
                                        ws.update(cx, |ws, cx| ws.stop_capture(cx)).ok();
                                    }),
                            ),
                    )
                    .child(label(tr!(
                        "capture.saving",
                        // Inside the open collection: its name is enough.
                        folder = format!(
                            "{}/",
                            folder
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        )
                    )))
                    .child(mono(format!("HTTP_PROXY={address}  HTTPS_PROXY={address}")))
                    .children(https.then(|| {
                        let hint = apix_capture::default_dir()
                            .map(|d| apix_capture::trust_hint(&d.join("capture-ca.pem")))
                            .unwrap_or_default();
                        v_flex()
                            .gap_1()
                            .child(label(tr!("capture.trust")))
                            .child(mono(hint))
                    }))
                    .into_any_element()
            }
            None => {
                let ws = workspace.downgrade();
                let (port_input, hosts_input, folder) =
                    (self.port.clone(), self.hosts.clone(), self.folder.clone());
                let https = self.https;
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .dir_row()
                            .gap_2()
                            .items_center()
                            .child(label(tr!("mock.port")))
                            .child(div().w(px(90.)).child(Input::new(&self.port).small()))
                            .child(label(tr!("capture.hosts")))
                            .child(div().flex_1().child(Input::new(&self.hosts).small()))
                            .child(
                                Button::new("capture-start")
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
                                            .unwrap_or(8888);
                                        let hosts: Vec<String> = hosts_input
                                            .read(cx)
                                            .value()
                                            .split([',', ' '])
                                            .map(str::trim)
                                            .filter(|h| !h.is_empty())
                                            .map(str::to_owned)
                                            .collect();
                                        let folder = folder.clone();
                                        ws.update(cx, |ws, cx| {
                                            ws.start_capture(folder, port, https, hosts, window, cx)
                                        })
                                        .ok();
                                    }),
                            ),
                    )
                    .child(
                        Checkbox::new("capture-https")
                            .dir_row()
                            .checked(self.https)
                            .label(tr!("capture.https"))
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.https = *checked;
                                cx.notify();
                            })),
                    )
                    .into_any_element()
            }
        };

        let log: Vec<AnyElement> = lines
            .iter()
            .map(|l| {
                let colour = match l.status {
                    Some(s) if s < 400 => p.ok,
                    Some(_) => p.err,
                    None => p.text_muted,
                };
                h_flex()
                    .dir_row()
                    .gap_3()
                    .py(px(1.))
                    .child(
                        div()
                            .w(px(64.))
                            .flex_none()
                            .text_xs()
                            .child(l.method.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(mono(l.url.clone())),
                    )
                    .child(
                        div()
                            .w(px(36.))
                            .flex_none()
                            .text_xs()
                            .text_color(colour)
                            .child(l.status.map(|s| s.to_string()).unwrap_or_default()),
                    )
                    .child(
                        div()
                            .w(px(220.))
                            .flex_none()
                            .truncate()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(l.note.clone()),
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
                    .child(bidi::para(tr!("capture.about"))),
            )
            .child(controls)
            .child(div().pt_1().child(label(tr!("mock.log"))))
            .child(if log.is_empty() {
                div()
                    .text_sm()
                    .text_color(p.text_muted)
                    .child(tr!("mock.log_empty"))
                    .into_any_element()
            } else {
                div()
                    .id("capture-log")
                    .max_h(px(240.))
                    .overflow_y_scroll()
                    .children(log)
                    .into_any_element()
            })
            .into_any_element()
    }
}

impl Workspace {
    /// The documentation's screenshot tour: starts the capture proxy on
    /// `port` and sends a few requests for `host` through it, as a browser
    /// set to use the proxy would.
    pub(super) fn tour_capture(
        &mut self,
        port: u16,
        host: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(collection) = self.collection.clone() else {
            return;
        };
        self.start_capture(
            collection.root.join("captured"),
            port,
            false,
            Vec::new(),
            window,
            cx,
        );
        std::thread::spawn(move || {
            use std::io::{Read as _, Write as _};
            std::thread::sleep(Duration::from_millis(400));
            for path in ["/users/1", "/users?_limit=3", "/orders/7"] {
                if let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) {
                    let _ = write!(
                        stream,
                        "GET http://{host}{path} HTTP/1.1\r\nHost: {host}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
                    );
                    let mut answer = Vec::new();
                    let _ = stream.read_to_end(&mut answer);
                }
            }
        });
    }
}
