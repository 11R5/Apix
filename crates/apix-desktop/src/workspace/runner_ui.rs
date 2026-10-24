//! The Runner (Ctrl+Shift+R, or "Run…" on a collection or folder): sends a
//! collection's saved requests in order, as many iterations as asked or one
//! per row of a CSV/JSON data file, with a pause between requests. It can be
//! paused, resumed and stopped, shows each result with its tests, and saves
//! JUnit XML, JSON or HTML reports. The work is `apix-runner`'s, the same
//! code `apix run` uses.
//!
//! The view lives on in the workspace, so closing the dialog does not stop a
//! run and reopening it shows the results.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use apix_core::scripts::ScriptVars;
use apix_core::store::StoreError;
use apix_core::{Collection, Method, Project, project};
use apix_runner::{Control, ItemResult, Plan, Progress, Row, RunReport, Scope, Summary};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Disableable as _, IconName, Sizable, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::Workspace;
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::theme;
use crate::{net, tr};

/// One request in the run's list.
struct RunItem {
    path: PathBuf,
    name: String,
    method: Method,
    on: bool,
}

/// What the run reports back; the UI copies it out a few times a second.
#[derive(Default)]
struct Live {
    items: Vec<ItemResult>,
    iteration: u32,
    sending: Option<String>,
    report: Option<RunReport>,
}

#[derive(Clone, Copy)]
enum ReportKind {
    Junit,
    Json,
    Html,
}

impl ReportKind {
    fn label(self) -> &'static str {
        match self {
            ReportKind::Junit => "JUnit XML",
            ReportKind::Json => "JSON",
            ReportKind::Html => "HTML",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            ReportKind::Junit => "xml",
            ReportKind::Json => "json",
            ReportKind::Html => "html",
        }
    }

    fn render(self, report: &RunReport) -> String {
        match self {
            ReportKind::Junit => report.to_junit(),
            ReportKind::Json => report.to_json(),
            ReportKind::Html => report.to_html(),
        }
    }
}

pub(super) struct RunnerView {
    collection: Collection,
    project: Option<Project>,
    /// The folder the run was opened on, when not the whole collection.
    folder: Option<PathBuf>,
    items: Vec<RunItem>,
    environments: Vec<String>,
    environment: Option<String>,
    /// What scripts set in the app so far; the run starts from it.
    script_vars: ScriptVars,
    iterations: Entity<InputState>,
    delay: Entity<InputState>,
    bail: bool,
    data: Option<(PathBuf, Vec<Row>)>,

    control: Option<Arc<Control>>,
    live: Arc<Mutex<Live>>,
    results: Vec<ItemResult>,
    sending: Option<String>,
    iteration: u32,
    total_iterations: u32,
    planned: usize,
    report: Option<RunReport>,
    expanded: Option<usize>,
    task: Option<Task<()>>,
}

impl RunnerView {
    #[allow(clippy::too_many_arguments)]
    fn new(
        collection: Collection,
        project: Option<Project>,
        folder: Option<PathBuf>,
        environment: Option<String>,
        script_vars: ScriptVars,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let items = collection
            .request_paths()
            .into_iter()
            .filter(|p| folder.as_ref().is_none_or(|f| p.starts_with(f)))
            .map(|path| {
                let request = apix_core::store::load_request(&path).ok();
                let name = request
                    .as_ref()
                    .map(|r| r.name.clone())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| {
                        path.file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    });
                RunItem {
                    method: request.map_or(Method::Get, |r| r.method),
                    name,
                    path,
                    on: true,
                }
            })
            .collect();
        let environments = match &project {
            Some(p) => p.environment_names(std::slice::from_ref(&collection)),
            None => collection.environment_names(),
        };
        let environment = environment.filter(|e| environments.contains(e));
        // No placeholder: what empty means depends on the data file, and the hint says it.
        let iterations = cx.new(|cx| InputState::new(window, cx));
        let delay = cx.new(|cx| InputState::new(window, cx).placeholder("0"));
        Self {
            collection,
            project,
            folder,
            items,
            environments,
            environment,
            script_vars,
            iterations,
            delay,
            bail: false,
            data: None,
            control: None,
            live: Arc::default(),
            results: Vec::new(),
            sending: None,
            iteration: 0,
            total_iterations: 0,
            planned: 0,
            report: None,
            expanded: None,
            task: None,
        }
    }

    pub(super) fn is_running(&self) -> bool {
        self.control.is_some()
    }

    /// The title: the collection, or "collection › folder".
    fn title(&self) -> String {
        match &self.folder {
            Some(folder) => {
                let rel = folder
                    .strip_prefix(&self.collection.root)
                    .unwrap_or(folder)
                    .to_string_lossy()
                    .replace('\\', " › ");
                format!("{} › {rel}", self.collection.meta.name)
            }
            None => self.collection.meta.name.clone(),
        }
    }

    fn plan(&self, cx: &App) -> Result<Plan, SharedString> {
        let number = |input: &Entity<InputState>| {
            let text = input.read(cx).value().trim().to_string();
            if text.is_empty() {
                Ok(0)
            } else {
                text.parse::<u64>().map_err(|_| tr!("runner.bad_number"))
            }
        };
        let iterations = number(&self.iterations)?.min(u32::MAX as u64) as u32;
        let delay = number(&self.delay)?;
        let requests: Vec<PathBuf> = self
            .items
            .iter()
            .filter(|i| i.on)
            .map(|i| i.path.clone())
            .collect();
        if requests.is_empty() {
            return Err(tr!("runner.nothing_selected"));
        }
        let (project, collection, env) = (
            self.project.as_ref(),
            Some(&self.collection),
            self.environment.as_deref(),
        );
        let vars = match project::variables(project, collection, env) {
            Err(StoreError::UnknownEnvironment(_)) => project::variables(project, collection, None),
            other => other,
        }
        .map_err(|e| tr!("runner.error", error = super::describe(&e)))?;
        let scopes = project::scope_values(project, collection, env).unwrap_or_default();
        Ok(Plan {
            root: self.collection.root.clone(),
            requests,
            vars,
            scopes,
            environment: self.environment.clone(),
            data: self
                .data
                .as_ref()
                .map(|(_, rows)| rows.clone())
                .unwrap_or_default(),
            iterations,
            delay: Duration::from_millis(delay),
            bail: self.bail,
            state: self.script_vars.clone(),
            ..Default::default()
        })
    }

    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_running() {
            return;
        }
        let plan = match self.plan(cx) {
            Ok(plan) => plan,
            Err(message) => {
                window.push_notification(Notification::error(message), cx);
                return;
            }
        };
        let scope = Scope {
            collection: Some(self.collection.clone()),
            project: self.project.clone(),
        };
        self.total_iterations = plan.iteration_count();
        self.planned = plan.requests.len() * self.total_iterations as usize;
        self.results.clear();
        self.report = None;
        self.expanded = None;
        self.iteration = 0;
        self.sending = None;
        let control = Arc::new(Control::default());
        let live = Arc::new(Mutex::new(Live::default()));
        self.control = Some(control.clone());
        self.live = live.clone();

        let names: Vec<(PathBuf, String)> = self
            .items
            .iter()
            .map(|i| (i.path.clone(), i.name.clone()))
            .collect();
        let client = net::client().clone();
        let work = async move {
            let report = apix_runner::run(
                &scope,
                &plan,
                &client,
                net::script_sender(),
                &control,
                |progress| {
                    let mut live = live.lock().unwrap_or_else(|e| e.into_inner());
                    match progress {
                        Progress::Iteration(n, _) => live.iteration = n,
                        Progress::Sending(path) => {
                            live.sending = names
                                .iter()
                                .find(|(p, _)| p == path)
                                .map(|(_, name)| name.clone());
                        }
                        Progress::Item(item) => {
                            live.sending = None;
                            live.items.push(item.clone());
                        }
                    }
                },
            )
            .await;
            live.lock().unwrap_or_else(|e| e.into_inner()).report = Some(report);
        };
        let running = net::spawn(work);
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            // Dropping this task (the view going away) drops `running`, which aborts the run.
            let _running = running;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                let done = this.update(cx, |this, cx| this.sync(cx)).unwrap_or(true);
                if done {
                    break;
                }
            }
        }));
        cx.notify();
    }

    /// Copies what the run reported; true once it ended.
    fn sync(&mut self, cx: &mut Context<Self>) -> bool {
        let mut live = self.live.lock().unwrap_or_else(|e| e.into_inner());
        let fresh = live.items.len() > self.results.len();
        if fresh {
            self.results
                .extend(live.items[self.results.len()..].iter().cloned());
        }
        self.iteration = live.iteration;
        self.sending = live.sending.clone();
        let report = live.report.take();
        drop(live);
        let done = report.is_some();
        if let Some(report) = report {
            self.report = Some(report);
            self.control = None;
            self.sending = None;
        }
        cx.notify();
        done
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(control) = &self.control {
            control.resume();
            control.stop();
        }
        cx.notify();
    }

    fn toggle_pause(&mut self, cx: &mut Context<Self>) {
        if let Some(control) = &self.control {
            if control.is_paused() {
                control.resume();
            } else {
                control.pause();
            }
        }
        cx.notify();
    }

    fn choose_data(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("runner.choose_data")),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let rows = apix_runner::data::load(&path);
            this.update_in(cx, |this, window, cx| {
                match rows {
                    Ok(rows) if rows.is_empty() => {
                        window.push_notification(Notification::error(tr!("runner.data_empty")), cx)
                    }
                    Ok(rows) => this.data = Some((path, rows)),
                    Err(e) => window.push_notification(
                        Notification::error(tr!("runner.data_error", error = format!("{e:#}"))),
                        cx,
                    ),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn save_report(&mut self, kind: ReportKind, window: &mut Window, cx: &mut Context<Self>) {
        let Some(report) = &self.report else {
            return;
        };
        let text = kind.render(report);
        let name = format!(
            "{}-report.{}",
            apix_import::slug(&self.collection.meta.name, "run"),
            kind.extension()
        );
        let picked = cx.prompt_for_new_path(&self.collection.root, Some(&name));
        cx.spawn_in(window, async move |_, cx| {
            let Ok(Ok(Some(path))) = picked.await else {
                return;
            };
            let result = std::fs::write(&path, text);
            cx.update(|window, cx| {
                let note = match result {
                    Ok(()) => Notification::success(tr!(
                        "runner.saved",
                        path = path.display().to_string()
                    )),
                    Err(e) => Notification::error(format!("{}: {e}", path.display())),
                };
                window.push_notification(note, cx);
            })
            .ok();
        })
        .detach();
    }

    fn move_item(&mut self, ix: usize, up: bool, cx: &mut Context<Self>) {
        let to = if up { ix.checked_sub(1) } else { Some(ix + 1) };
        if let Some(to) = to.filter(|to| *to < self.items.len()) {
            self.items.swap(ix, to);
            cx.notify();
        }
    }

    fn render_requests(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let running = self.is_running();
        let all_on = self.items.iter().all(|i| i.on);
        let rows = self.items.iter().enumerate().map(|(ix, item)| {
            let last = ix + 1 == self.items.len();
            let rel = item
                .path
                .strip_prefix(&self.collection.root)
                .unwrap_or(&item.path)
                .parent()
                .map(|d| d.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            h_flex()
                .id(("run-item", ix))
                .dir_row()
                .gap_1p5()
                .items_center()
                .px_1()
                .py_0p5()
                .rounded_md()
                .hover(|s| s.bg(p.surface))
                .child(
                    Checkbox::new(("run-on", ix))
                        .checked(item.on)
                        .disabled(running)
                        .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                            if let Some(item) = this.items.get_mut(ix) {
                                item.on = *checked;
                            }
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .w(px(48.))
                        .flex_none()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::method_color(item.method))
                        .child(item.method.as_str()),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_sm().truncate().child(item.name.clone()))
                        .when(!rel.is_empty(), |d| {
                            d.child(
                                div()
                                    .text_xs()
                                    .text_color(p.text_muted)
                                    .truncate()
                                    .child(rel),
                            )
                        }),
                )
                .when(!running, |row| {
                    row.child(
                        Button::new(("run-up", ix))
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowUp)
                            .disabled(ix == 0)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.move_item(ix, true, cx)
                            })),
                    )
                    .child(
                        Button::new(("run-down", ix))
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowDown)
                            .disabled(last)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.move_item(ix, false, cx)
                            })),
                    )
                })
        });
        v_flex()
            .gap_1()
            .flex_1()
            .min_h_0()
            .child(
                h_flex()
                    .dir_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(tr!("runner.requests", count = self.items.len())),
                    )
                    .child(
                        Button::new("run-all")
                            .ghost()
                            .xsmall()
                            .disabled(running)
                            .label(if all_on {
                                tr!("runner.none")
                            } else {
                                tr!("runner.all")
                            })
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                for item in &mut this.items {
                                    item.on = !all_on;
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex()
                    .id("run-items")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(rows),
            )
    }

    fn render_options(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let running = self.is_running();
        let label = |text: SharedString| div().text_xs().text_color(p.text_muted).child(text);
        let env_label = self
            .environment
            .clone()
            .map(SharedString::from)
            .unwrap_or_else(|| tr!("env.none"));
        let environment = Button::new("run-env")
            .small()
            .outline()
            .w_full()
            .disabled(running)
            .label(env_label)
            .dropdown_menu({
                let view = cx.entity().downgrade();
                let (names, current) = (self.environments.clone(), self.environment.clone());
                move |menu, _, _| {
                    let pick = |label: String, env: Option<String>| {
                        let view = view.clone();
                        PopupMenuItem::new(label).checked(current == env).on_click(
                            move |_, _, cx| {
                                view.update(cx, |this, cx| {
                                    this.environment = env.clone();
                                    cx.notify();
                                })
                                .ok();
                            },
                        )
                    };
                    names.iter().fold(
                        menu.item(pick(tr!("env.none").to_string(), None))
                            .separator(),
                        |menu, name| menu.item(pick(name.clone(), Some(name.clone()))),
                    )
                }
            });
        let data: AnyElement =
            match &self.data {
                Some((path, rows)) => {
                    let file = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let columns: Vec<&str> = rows
                        .first()
                        .map(|r| r.keys().map(String::as_str).collect())
                        .unwrap_or_default();
                    h_flex()
                        .dir_row()
                        .gap_1()
                        .items_center()
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_sm().truncate().child(file))
                                .child(div().text_xs().text_color(p.text_muted).truncate().child(
                                    tr!(
                                        "runner.data_rows",
                                        count = rows.len(),
                                        columns = columns.join(", ")
                                    ),
                                )),
                        )
                        .child(
                            Button::new("run-data-clear")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Close)
                                .disabled(running)
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.data = None;
                                    cx.notify();
                                })),
                        )
                        .into_any_element()
                }
                None => {
                    Button::new("run-data")
                        .small()
                        .outline()
                        .w_full()
                        .disabled(running)
                        .label(tr!("runner.choose_data"))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.choose_data(window, cx)
                        }))
                        .into_any_element()
                }
            };
        let iterations_hint = match &self.data {
            Some((_, rows)) => tr!("runner.iterations_rows", count = rows.len()),
            None => tr!("runner.iterations_hint"),
        };
        v_flex()
            .gap_1()
            .child(label(tr!("runner.environment")))
            .child(environment)
            .child(
                h_flex()
                    .dir_row()
                    .gap_2()
                    .pt_1()
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(label(tr!("runner.iterations")))
                            .child(Input::new(&self.iterations).small().disabled(running)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(label(tr!("runner.delay")))
                            .child(Input::new(&self.delay).small().disabled(running)),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(bidi::para(iterations_hint)),
            )
            .child(div().pt_1().child(label(tr!("runner.data"))))
            .child(data)
            .child(
                div().pt_1().child(
                    Checkbox::new("run-bail")
                        .dir_row()
                        .checked(self.bail)
                        .disabled(running)
                        .label(tr!("runner.bail"))
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            this.bail = *checked;
                            cx.notify();
                        })),
                ),
            )
    }

    fn render_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let row = h_flex().dir_row().gap_2().pt_1();
        match &self.control {
            None => row.child(
                Button::new("run-start")
                    .primary()
                    .flex_1()
                    .icon(IconName::Play)
                    .label(tr!("runner.run"))
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| this.start(window, cx)),
                    ),
            ),
            Some(control) => {
                let paused = control.is_paused();
                row.child(
                    Button::new("run-pause")
                        .outline()
                        .flex_1()
                        .icon(if paused {
                            IconName::Play
                        } else {
                            IconName::Pause
                        })
                        .label(if paused {
                            tr!("runner.resume")
                        } else {
                            tr!("runner.pause")
                        })
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_pause(cx))),
                )
                .child(
                    Button::new("run-stop")
                        .outline()
                        .flex_1()
                        .icon(IconName::Square)
                        .label(tr!("runner.stop"))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.stop(cx))),
                )
            }
        }
    }

    fn render_results(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        if self.results.is_empty() && !self.is_running() && self.report.is_none() {
            return v_flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .w(px(420.))
                        .text_sm()
                        .text_color(p.text_muted)
                        .child(bidi::para(tr!("runner.idle"))),
                )
                .into_any_element();
        }
        let summary = Summary::of(&self.results);
        let state = if let Some(control) = &self.control {
            if control.is_stopped() {
                tr!("runner.stopping")
            } else if control.is_paused() {
                tr!("runner.paused")
            } else {
                match &self.sending {
                    Some(name) => tr!("runner.sending", name = name),
                    None => tr!("runner.waiting"),
                }
            }
        } else if let Some(report) = &self.report {
            let seconds = format!("{:.1}", report.duration_ms as f64 / 1000.0);
            if report.stopped {
                tr!("runner.stopped", seconds = seconds)
            } else {
                tr!("runner.finished", seconds = seconds)
            }
        } else {
            SharedString::default()
        };
        let progress = if self.planned == 0 {
            0.0
        } else {
            (self.results.len() as f32 / self.planned as f32).min(1.0)
        };
        let several = self.total_iterations > 1;
        let rows = self.results.iter().enumerate().map(|(ix, item)| {
            let (mark, color) = if item.skipped {
                ("–", p.text_muted)
            } else if item.passed {
                ("✓", p.ok)
            } else {
                ("✗", p.err)
            };
            let status = match item.status {
                Some(code) => format!("{code} {}", item.reason),
                None if item.skipped => tr!("runner.skipped").to_string(),
                None => tr!("runner.not_sent").to_string(),
            };
            let tests = if item.tests.is_empty() {
                String::new()
            } else {
                let run = item.tests.iter().filter(|t| !t.skipped).count();
                format!("{}/{run}", run - item.failed_tests())
            };
            let open = self.expanded == Some(ix);
            // A request that was not sent says why under its name.
            let (line, line_is_error) = match (&item.error, item.status) {
                (Some(error), None) => (error.clone(), true),
                _ => (item.url.clone(), false),
            };
            let has_more =
                !item.tests.is_empty() || !item.logs.is_empty() || !item.errors().is_empty();
            v_flex()
                .border_b_1()
                .border_color(p.hairline)
                .child(
                    h_flex()
                        .id(("run-result", ix))
                        .dir_row()
                        .gap_2()
                        .items_center()
                        .px_1()
                        .py_1()
                        .when(has_more, |row| {
                            row.cursor_pointer()
                                .hover(|s| s.bg(p.surface))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.expanded = if open { None } else { Some(ix) };
                                    cx.notify();
                                }))
                        })
                        .child(div().w(px(14.)).flex_none().text_color(color).child(mark))
                        .when(several, |row| {
                            row.child(
                                div()
                                    .w(px(28.))
                                    .flex_none()
                                    .text_xs()
                                    .text_color(p.text_muted)
                                    .child(format!("#{}", item.iteration + 1)),
                            )
                        })
                        .child(
                            div()
                                .w(px(52.))
                                .flex_none()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::method_color(
                                    item.method.parse().unwrap_or(Method::Get),
                                ))
                                .child(item.method.clone()),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_sm().truncate().child(item.name.clone()))
                                .child(
                                    div()
                                        .ltr()
                                        .text_xs()
                                        .text_color(if line_is_error {
                                            p.err
                                        } else {
                                            p.text_muted
                                        })
                                        .truncate()
                                        .child(line),
                                ),
                        )
                        .child(
                            div()
                                .w(px(120.))
                                .flex_none()
                                .text_xs()
                                .text_color(item.status.map_or(p.err, |s| p.status(s)))
                                .truncate()
                                .child(status),
                        )
                        .child(
                            div()
                                .w(px(64.))
                                .flex_none()
                                .text_xs()
                                .text_color(p.text_muted)
                                .child(if item.status.is_some() {
                                    format!("{:.0} ms", item.time_ms)
                                } else {
                                    String::new()
                                }),
                        )
                        .child(
                            div()
                                .w(px(40.))
                                .flex_none()
                                .text_xs()
                                .text_color(if item.failed_tests() > 0 {
                                    p.err
                                } else {
                                    p.text_muted
                                })
                                .child(tests),
                        ),
                )
                .when(open, |d| d.child(self.render_details(item)))
        });
        v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .gap_2()
            .child(
                h_flex()
                    .dir_row()
                    .gap_3()
                    .items_center()
                    .child(div().flex_1().min_w_0().text_sm().truncate().child(state))
                    .when(several, |d| {
                        d.child(div().text_xs().text_color(p.text_muted).child(tr!(
                            "runner.iteration",
                            iteration = (self.iteration + 1).min(self.total_iterations),
                            total = self.total_iterations
                        )))
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(if summary.failed > 0 {
                                p.err
                            } else {
                                p.text_muted
                            })
                            .child(tr!(
                                "runner.summary",
                                passed = summary.passed,
                                failed = summary.failed,
                                tests_passed = summary.tests - summary.tests_failed,
                                tests = summary.tests
                            )),
                    ),
            )
            .child(
                div().h(px(4.)).w_full().rounded_full().bg(p.surface).child(
                    div()
                        .h_full()
                        .rounded_full()
                        .w(relative(progress))
                        .bg(if summary.failed > 0 { p.err } else { p.accent }),
                ),
            )
            .child(
                v_flex()
                    .id("run-results")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(rows),
            )
            .when_some(self.report.as_ref(), |d, _| {
                d.child(
                    h_flex()
                        .dir_row()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .text_color(p.text_muted)
                                .child(tr!("runner.save_report")),
                        )
                        .children([ReportKind::Junit, ReportKind::Json, ReportKind::Html].map(
                            |kind| {
                                Button::new(kind.extension())
                                    .small()
                                    .outline()
                                    .label(kind.label())
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, window, cx| {
                                            this.save_report(kind, window, cx)
                                        },
                                    ))
                            },
                        )),
                )
            })
            .into_any_element()
    }

    fn render_details(&self, item: &ItemResult) -> impl IntoElement {
        let p = crate::theme::current();
        let errors = item
            .errors()
            .into_iter()
            .map(move |error| div().ltr().text_xs().text_color(p.err).child(error));
        let tests = item.tests.iter().map(move |test| {
            let (mark, color) = if test.skipped {
                ("–", p.text_muted)
            } else if test.passed {
                ("✓", p.ok)
            } else {
                ("✗", p.err)
            };
            v_flex()
                .child(
                    h_flex()
                        .dir_row()
                        .gap_2()
                        .child(div().w(px(14.)).flex_none().text_color(color).child(mark))
                        .child(div().text_xs().child(test.name.clone())),
                )
                .children(test.error.clone().map(|e| {
                    div()
                        .ps(px(22.))
                        .ltr()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(e)
                }))
        });
        let logs = item.logs.iter().map(move |line| {
            let color = match line.level.as_str() {
                "error" => p.err,
                "warn" => p.warn,
                _ => p.text_muted,
            };
            div()
                .ltr()
                .text_xs()
                .text_color(color)
                .child(line.text.clone())
        });
        v_flex()
            .gap_0p5()
            .ps(px(22.))
            .pb_2()
            .children(errors)
            .children(tests)
            .when(!item.logs.is_empty(), |d| {
                d.child(
                    v_flex()
                        .mt_1()
                        .p_1()
                        .rounded_md()
                        .bg(p.surface)
                        .children(logs),
                )
            })
    }
}

impl Render for RunnerView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        h_flex()
            .dir_row()
            .h(px(540.))
            .gap_4()
            .items_stretch()
            .child(
                v_flex()
                    .w(px(330.))
                    .flex_none()
                    .gap_2()
                    .child(self.render_requests(cx))
                    .child(div().h(px(1.)).bg(p.hairline))
                    .child(self.render_options(cx))
                    .child(self.render_controls(cx)),
            )
            .child(div().w(px(1.)).bg(p.hairline))
            .child(self.render_results(cx))
    }
}

impl Workspace {
    /// The Runner for the active collection (or reopened, as it was).
    pub(super) fn open_runner(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(runner) = &self.runner
            && (runner.read(cx).is_running()
                || self
                    .collection
                    .as_ref()
                    .is_some_and(|c| runner.read(cx).collection.root == c.root))
        {
            let runner = runner.clone();
            self.show_runner(runner, window, cx);
            return;
        }
        match self.collection.as_ref().map(|c| c.root.clone()) {
            Some(root) => self.run_folder(root, window, cx),
            None => self.notify_error(
                tr!("collection.open_first", key = super::key_hint("O")),
                window,
                cx,
            ),
        }
    }

    /// The Runner for the collection or folder at `path`.
    pub(super) fn run_folder(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(runner) = &self.runner
            && runner.read(cx).is_running()
        {
            window.push_notification(Notification::warning(tr!("runner.busy")), cx);
            let runner = runner.clone();
            self.show_runner(runner, window, cx);
            return;
        }
        let Some(collection) = self.collection_for(&path).cloned() else {
            return;
        };
        let folder = (path != collection.root).then_some(path);
        // The project's environments apply only to its own collections.
        let project = self
            .project
            .clone()
            .filter(|p| collection.root.starts_with(&p.root));
        let environment = self.environment.clone();
        let script_vars = self.script_vars.clone();
        let runner = cx.new(|cx| {
            RunnerView::new(
                collection,
                project,
                folder,
                environment,
                script_vars,
                window,
                cx,
            )
        });
        self.runner = Some(runner.clone());
        self.show_runner(runner, window, cx);
    }

    fn show_runner(
        &mut self,
        runner: Entity<RunnerView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = cx.entity().downgrade();
        let title = tr!("runner.title", name = runner.read(cx).title());
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .on_close(super::collections::refocus(view.clone()))
                .bidi_title(title.clone())
                .width(px(1000.))
                .child(div().pb_2().child(runner.clone()))
        });
    }
}
