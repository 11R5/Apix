//! The screenshot tour behind the documentation's pictures.
//!
//! `APIX_TOUR=<scenes.toml>` makes the app walk through the scenes in that
//! file: each one opens requests, sends them, opens a window and so on, then
//! the app writes the scene's name into `<APIX_TOUR_OUT>/ready` and waits
//! until that file is gone. `scripts/docs-tour.ps1` takes the picture of the
//! window in between, once for every language. The tour quits at the end.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui_kit::component::WindowExt as _;
use gpui_kit::*;
use serde::Deserialize;

use super::{ResponseState, Workspace};

#[derive(Deserialize)]
struct Tour {
    scene: Vec<Scene>,
}

/// One picture, and what to do before taking it, in this order.
#[derive(Deserialize, Default, Clone)]
#[serde(default, deny_unknown_fields)]
struct Scene {
    /// The picture's file name, without `.png`.
    name: String,
    /// A collection or project folder: relative to the scenes file, or
    /// starting with `$WORK/` (the tour script's working folder).
    open: Option<String>,
    /// Request files (relative to the opened folder) open as tabs; the last
    /// one is in front.
    tabs: Vec<String>,
    /// Folders (relative to the opened folder) shown closed in the sidebar;
    /// every other folder is open.
    closed: Vec<String>,
    /// The active environment, by name; `""` for none.
    env: Option<String>,
    /// The request pane's tab: 0 Params, 1 Headers, 2 Auth, 3 Body,
    /// 4 Scripts, 5 Settings, 6 Examples.
    request_tab: Option<usize>,
    /// Send the front request and wait for its answer.
    send: bool,
    /// The response pane's tab: 0 Pretty, 1 Raw, 2 Headers, 3 Network, 4 Hex,
    /// 5 Tests, 6 Console.
    response_tab: Option<usize>,
    /// A JSONPath or jq filter over the response.
    filter: Option<String>,
    /// "text", "table" or "chart".
    visual: Option<String>,
    /// A window to open (see `Workspace::tour_dialog`).
    dialog: Option<String>,
    /// Steps of their own (see `Workspace::tour_step`).
    steps: Vec<String>,
    /// Milliseconds to wait before the picture (default 800).
    wait: Option<u64>,
}

thread_local! {
    /// The window opened last, for steps that act inside it.
    static VIEW: RefCell<Option<AnyEntity>> = const { RefCell::new(None) };
    /// The folder of the scenes file.
    static BASE: RefCell<PathBuf> = const { RefCell::new(PathBuf::new()) };
}

pub(super) fn active() -> Option<PathBuf> {
    std::env::var_os("APIX_TOUR").map(PathBuf::from)
}

/// Remembers a window's view, during a tour, for the steps that act in it.
pub(super) fn keep<T: 'static>(view: &Entity<T>) {
    if active().is_some() {
        VIEW.with(|v| *v.borrow_mut() = Some(view.clone().into_any()));
    }
}

fn view<T: 'static>() -> Option<Entity<T>> {
    VIEW.with(|v| v.borrow().clone()?.downcast::<T>().ok())
}

/// A path written in the scenes file.
pub(super) fn path(text: &str) -> PathBuf {
    match text.strip_prefix("$WORK/") {
        Some(rest) => std::env::var_os("APIX_TOUR_WORK")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(rest),
        None => BASE.with(|b| b.borrow().join(text)),
    }
}

fn out_dir() -> PathBuf {
    std::env::var_os("APIX_TOUR_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("apix-tour"))
}

impl Workspace {
    /// Starts the tour in `file` (see the module's documentation).
    pub(super) fn start_tour(
        &mut self,
        file: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tour: Tour = match std::fs::read_to_string(&file)
            .map_err(|e| e.to_string())
            .and_then(|text| toml::from_str(&text).map_err(|e| e.to_string()))
        {
            Ok(tour) => tour,
            Err(error) => {
                eprintln!("tour: {}: {error}", file.display());
                cx.quit();
                return;
            }
        };
        let base = file.parent().map(Path::to_path_buf).unwrap_or_default();
        BASE.with(|b| *b.borrow_mut() = base);
        let only: Option<Vec<String>> = std::env::var("APIX_TOUR_ONLY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.split(',').map(|n| n.trim().to_string()).collect());
        let scenes: Vec<Scene> = tour
            .scene
            .into_iter()
            .filter(|s| only.as_ref().is_none_or(|o| o.contains(&s.name)))
            .collect();
        let out = out_dir();
        let _ = std::fs::create_dir_all(&out);
        cx.spawn_in(window, async move |this, cx| {
            for scene in scenes {
                let set_up = this.update_in(cx, |this, window, cx| {
                    window.close_all_dialogs(cx);
                    window.clear_notifications(cx);
                    this.tour_setup(&scene, window, cx);
                });
                if set_up.is_err() {
                    return;
                }
                pause(cx, 500).await;
                if scene.send {
                    let _ = this.update_in(cx, |this, window, cx| this.send(window, cx));
                    wait_for_answer(&this, cx).await;
                }
                let _ = this.update_in(cx, |this, window, cx| {
                    this.tour_after_send(&scene, window, cx)
                });
                pause(cx, 300).await;
                for step in &scene.steps {
                    if step == "send" {
                        let _ = this.update_in(cx, |this, window, cx| this.send(window, cx));
                        wait_for_answer(&this, cx).await;
                        continue;
                    }
                    let ms = this
                        .update_in(cx, |this, window, cx| this.tour_step(step, window, cx))
                        .unwrap_or(0);
                    pause(cx, ms.max(150)).await;
                }
                pause(cx, scene.wait.unwrap_or(800)).await;
                let ready = out.join("ready");
                let _ = std::fs::write(&ready, &scene.name);
                let asked = Instant::now();
                while ready.exists() && asked.elapsed() < Duration::from_secs(30) {
                    pause(cx, 100).await;
                }
            }
            let _ = std::fs::write(out.join("done"), "");
            let _ = this.update(cx, |_, cx| cx.quit());
        })
        .detach();
    }

    /// The parts of a scene that come before sending.
    fn tour_setup(&mut self, scene: &Scene, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = &scene.open {
            let dir = path(open);
            let dir = dir.canonicalize().unwrap_or(dir);
            let dir = PathBuf::from(dir.to_string_lossy().trim_start_matches(r"\\?\"));
            if self.workspace_root().as_deref() != Some(dir.as_path()) {
                // Nothing of the folder before stays behind an unlock prompt.
                self.close_workspace(window, cx);
                if open != "-" {
                    self.open_or_create(dir, window, cx);
                }
            }
        }
        let Some(root) = self.workspace_root() else {
            return;
        };
        self.collapsed = scene.closed.iter().map(|f| root.join(f)).collect();
        let wanted: Vec<PathBuf> = scene
            .tabs
            .iter()
            .filter_map(|t| {
                let tail = Path::new(t);
                self.requests.keys().find(|k| k.ends_with(tail)).cloned()
            })
            .collect();
        // Tabs not in this scene are closed, the others opened in order.
        let open: Vec<PathBuf> = self.tabs.iter().map(|t| t.path.clone()).collect();
        for path in open {
            if !wanted.contains(&path) {
                self.forget_tabs(&path, window, cx);
            }
        }
        for path in &wanted {
            self.open_tab(path.clone(), window, cx);
        }
        if let Some(env) = &scene.env {
            let env = (!env.is_empty()).then(|| env.clone());
            self.set_environment(env, window, cx);
            self.refresh_environments(window, cx);
        }
        if let Some(tab) = scene.request_tab {
            self.request_tab = tab;
        }
        cx.notify();
    }

    /// The parts of a scene that look at the response.
    fn tour_after_send(&mut self, scene: &Scene, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab) = scene.response_tab {
            self.set_response_tab(tab, window, cx);
        }
        if let Some(filter) = &scene.filter {
            self.filter
                .update(cx, |state, cx| state.set_value(filter.clone(), window, cx));
            self.run_filter(window, cx);
        }
        let mode = match scene.visual.as_deref() {
            Some("table") => super::visual_ui::VisualMode::Table,
            Some("chart") => super::visual_ui::VisualMode::Chart,
            Some("plugin") => super::visual_ui::VisualMode::Plugin(0),
            _ => super::visual_ui::VisualMode::Text,
        };
        self.set_visual(mode, cx);
        if let Some(dialog) = &scene.dialog {
            self.tour_dialog(dialog, window, cx);
        }
        cx.notify();
    }

    /// Opens a window by name; `runner <folder>` runs a folder.
    fn tour_dialog(&mut self, dialog: &str, window: &mut Window, cx: &mut Context<Self>) {
        let (name, arg) = dialog.split_once(' ').unwrap_or((dialog, ""));
        match name {
            "runner" if !arg.is_empty() => {
                if let Some(root) = self.workspace_root() {
                    self.run_folder(root.join(arg), window, cx);
                }
                if let Some(runner) = &self.runner {
                    keep(runner);
                }
            }
            "runner" => {
                self.open_runner(window, cx);
                if let Some(runner) = &self.runner {
                    keep(runner);
                }
            }
            "flows" => self.open_flows(window, cx),
            "load" => self.open_load_test(window, cx),
            "monitors" => self.open_monitors(window, cx),
            "mock" => self.open_mock(window, cx),
            "capture" => self.open_capture(window, cx),
            "git" => self.open_git(window, cx),
            "route" => self.open_route(window, cx),
            "plugins" => self.open_plugins(window, cx),
            "palette" => self.open_palette(window, cx),
            "settings" => self.open_settings(window, cx),
            "setup" => self.open_setup(window, cx),
            "claude" => self.open_connect_claude(window, cx),
            "history" => self.open_history(window, cx),
            "compare" => self.open_compare(window, cx),
            "variables" => self.open_env_editor(window, cx),
            "cookies" => self.open_cookies(window, cx),
            "import" => self.tour_import_url(arg.to_string(), window, cx),
            "update" => self.tour_update(window, cx),
            "explain" => {
                let code = crate::i18n::current().code;
                let file = path(&arg.replace("{lang}", code));
                let answer = std::fs::read_to_string(&file)
                    .or_else(|_| std::fs::read_to_string(path(&arg.replace("{lang}", "en"))))
                    .unwrap_or_default();
                self.tour_explain(answer, window, cx);
            }
            other => eprintln!("tour: no window called {other}"),
        }
    }

    /// One named step of a scene; returns how long to wait after it, in ms.
    fn tour_step(&mut self, step: &str, window: &mut Window, cx: &mut Context<Self>) -> u64 {
        let (name, arg) = step.split_once(' ').unwrap_or((step, ""));
        // `window.step arg`: a step inside the window opened last.
        if let Some((part, inner)) = name.split_once('.') {
            return match part {
                "runner" => view::<super::runner_ui::RunnerView>()
                    .map(|v| v.update(cx, |v, cx| v.tour(inner, arg, window, cx))),
                "flows" => view::<super::flows_ui::FlowsView>()
                    .map(|v| v.update(cx, |v, cx| v.tour(inner, arg, window, cx))),
                "load" => view::<super::load_ui::LoadView>()
                    .map(|v| v.update(cx, |v, cx| v.tour(inner, arg, window, cx))),
                "compare" => view::<super::compare_ui::CompareView>()
                    .map(|v| v.update(cx, |v, cx| v.tour(inner, arg, cx))),
                "route" => view::<super::route_ui::RouteView>()
                    .map(|v| v.update(cx, |v, cx| v.tour(inner, arg, cx))),
                "settings" => view::<super::settings_ui::SettingsView>()
                    .map(|v| v.update(cx, |v, cx| v.tour(inner, arg, cx))),
                _ => None,
            }
            .unwrap_or_else(|| {
                eprintln!("tour: no window for {step}");
                0
            });
        }
        match name {
            "wait" => arg.parse().unwrap_or(500),
            // A window opened after other steps (`dialog` opens it before them).
            "dialog" => {
                self.tour_dialog(arg, window, cx);
                600
            }
            "mock" => {
                self.tour_mock(arg.parse().unwrap_or(4011), window, cx);
                300
            }
            "capture" => {
                let (port, host) = arg.split_once(' ').unwrap_or((arg, "127.0.0.1:4011"));
                self.tour_capture(port.parse().unwrap_or(8888), host.to_string(), window, cx);
                2500
            }
            "monitor" => {
                self.tour_monitor(arg, window, cx);
                4000
            }
            "message" => {
                let messages = self.ws_editor.read(cx).messages().to_vec();
                if let Some(message) = arg.parse().ok().and_then(|i: usize| messages.get(i)) {
                    self.send_socket_message(message.clone(), window, cx);
                }
                900
            }
            "header" => {
                let row = self.new_header_row(
                    &apix_core::KeyValue {
                        key: arg.to_string(),
                        value: String::new(),
                        enabled: true,
                    },
                    window,
                    cx,
                );
                let key = row.key.clone();
                self.headers.push(row);
                self.request_tab = 1;
                key.update(cx, |state, cx| state.focus(window, cx));
                300
            }
            "example" => {
                self.open_example(arg.parse().unwrap_or(0), window, cx);
                300
            }
            // Opens or closes a group of the Network tab.
            "network_group" => {
                const GROUPS: [&str; 7] = [
                    "timing",
                    "connection",
                    "dns",
                    "redirects",
                    "body",
                    "encryption",
                    "certificates",
                ];
                if let Some(key) = GROUPS.iter().find(|g| **g == arg)
                    && !self.network_toggled.remove(key)
                {
                    self.network_toggled.insert(key);
                }
                cx.notify();
                200
            }
            "scroll_network" => {
                let y = arg.parse::<f32>().unwrap_or(400.);
                self.network_scroll.set_offset(point(px(0.), px(-y)));
                cx.notify();
                300
            }
            "schema" => {
                self.fetch_schema(window, cx);
                2000
            }
            "complete" => {
                let (line, column) = arg.split_once(' ').unwrap_or((arg, "0"));
                let (line, column) = (line.parse().unwrap_or(0), column.parse().unwrap_or(0));
                self.body_editor.update(cx, |editor, cx| {
                    editor.tour_complete(line, column, window, cx)
                });
                800
            }
            "grpc_convert" => {
                let source = arg.to_string();
                self.grpc_editor
                    .update(cx, |editor, cx| editor.tour_convert(source, window, cx));
                2000
            }
            "url" => {
                let url = arg.to_string();
                self.url
                    .update(cx, |state, cx| state.set_value(url, window, cx));
                300
            }
            _ => {
                eprintln!("tour: unknown step {step}");
                0
            }
        }
    }
}

async fn pause(cx: &mut AsyncWindowContext, ms: u64) {
    cx.background_executor()
        .timer(Duration::from_millis(ms))
        .await;
}

/// Until the front request is answered (20 s at most).
async fn wait_for_answer(this: &WeakEntity<Workspace>, cx: &mut AsyncWindowContext) {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(20) {
        pause(cx, 100).await;
        let busy = this
            .update(cx, |this, _| {
                matches!(this.response, ResponseState::Loading)
            })
            .unwrap_or(false);
        if !busy {
            break;
        }
    }
}
