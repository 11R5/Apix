//! Monitors (View › Monitors…): the `[[monitor]]`s of the open collections,
//! their last results and when they run next; adding, editing, removing and
//! running one now. While the setting is on, the app runs them on their
//! schedules in the background, one at a time, and says so when one starts
//! failing or passes again (and tells its webhook). The work, the log and the
//! schedule are `apix-runner`'s, shared with `apix monitor`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use apix_core::Collection;
use apix_core::monitor::{Monitor, describe_every, parse_every};
use apix_runner::Scope;
use apix_runner::monitor::{self, Change, Log, Outcome, Scheduler};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{
    Disableable as _, IconName, Sizable as _, WindowExt as _, h_flex, v_flex,
};

use gpui_kit::*;

use super::Workspace;
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::{net, tr};

/// What the workspace keeps about monitors between openings of the window.
#[derive(Default)]
pub(super) struct MonitorState {
    /// When each monitor is due, per collection root.
    schedulers: HashMap<PathBuf, Scheduler>,
    /// The last outcome of each (collection root, monitor name), read from its log.
    last: HashMap<(PathBuf, String), Option<Outcome>>,
    /// The monitor being run now.
    running: Option<(PathBuf, String)>,
    run: Option<Task<()>>,
    ticker: Option<Task<()>>,
}

type Done = (Outcome, Option<Change>, Option<Result<(), String>>);

fn log_of(root: &Path, name: &str) -> Option<Log> {
    Some(Log::new(&monitor::default_dir()?, root, name))
}

impl Workspace {
    /// Starts looking every second for monitors that are due.
    pub(super) fn start_monitors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.monitors.ticker = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this
                    .update_in(cx, |this, window, cx| this.tick_monitors(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    fn tick_monitors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.settings.monitors_in_app || self.monitors.running.is_some() {
            return;
        }
        let now = Instant::now();
        for collection in self.collections.clone() {
            let root = collection.root.clone();
            let due = self
                .monitors
                .schedulers
                .entry(root.clone())
                .or_default()
                .due(&collection.meta.monitors, now, |m| {
                    log_of(&root, &m.name).and_then(|log| log.last())
                });
            if let Some(m) = due.into_iter().next() {
                self.run_monitor(collection, m, window, cx);
                return;
            }
        }
    }

    /// Runs `m` of `collection` now, in the background.
    pub(super) fn run_monitor(
        &mut self,
        collection: Collection,
        m: Monitor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.monitors.running.is_some() {
            window.push_notification(Notification::warning(tr!("monitors.busy")), cx);
            return;
        }
        let root = collection.root.clone();
        let scope = Scope {
            project: self.project.clone().filter(|p| root.starts_with(&p.root)),
            collection: Some(collection),
        };
        let client = net::client().clone();
        let started = Instant::now();
        let (work_root, work_monitor) = (root.clone(), m.clone());
        let work = async move {
            let (_, outcome) = monitor::run_once(
                &scope,
                &work_monitor,
                &client,
                net::script_sender(),
                &[],
                &Default::default(),
            )
            .await;
            let log = log_of(&work_root, &work_monitor.name);
            let previous = log.as_ref().and_then(Log::last);
            if let Some(log) = &log {
                let _ = log.record(&outcome);
            }
            let change = monitor::change(previous.as_ref(), &outcome);
            let mut told = None;
            if let Some(change) = change
                && !work_monitor.webhook.trim().is_empty()
            {
                let vars = monitor::plan(&scope, &work_monitor, &[])
                    .map(|p| p.vars)
                    .unwrap_or_default();
                let name = scope
                    .collection
                    .as_ref()
                    .map(|c| c.meta.name.clone())
                    .unwrap_or_default();
                told = Some(
                    monitor::notify(&client, &vars, &name, &work_monitor, &outcome, change)
                        .await
                        .map_err(|e| format!("{e:#}")),
                );
            }
            (outcome, change, told)
        };
        let running = net::spawn(work);
        self.monitors.running = Some((root.clone(), m.name.clone()));
        self.monitors.run = Some(cx.spawn_in(window, async move |this, cx| {
            let done = running.await;
            this.update_in(cx, |this, window, cx| {
                this.monitor_done(root, m, started, done, window, cx)
            })
            .ok();
        }));
        cx.notify();
    }

    fn monitor_done(
        &mut self,
        root: PathBuf,
        m: Monitor,
        started: Instant,
        done: Result<Done, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.monitors.running = None;
        self.monitors
            .schedulers
            .entry(root.clone())
            .or_default()
            .ran(&m, started);
        match done {
            Ok((outcome, change, told)) => {
                match change {
                    Some(Change::Failing) => window.push_notification(
                        Notification::error(tr!(
                            "monitors.toast_failing",
                            name = m.name,
                            summary = outcome.summary()
                        )),
                        cx,
                    ),
                    Some(Change::Recovered) => window.push_notification(
                        Notification::success(tr!("monitors.toast_recovered", name = m.name)),
                        cx,
                    ),
                    None => {}
                }
                if let Some(Err(e)) = told {
                    window.push_notification(
                        Notification::warning(tr!(
                            "monitors.webhook_failed",
                            name = m.name,
                            error = e
                        )),
                        cx,
                    );
                }
                self.monitors.last.insert((root, m.name), Some(outcome));
            }
            Err(e) => self.notify_error(e, window, cx),
        }
        cx.notify();
    }

    fn last_outcome(&mut self, root: &Path, name: &str) -> Option<Outcome> {
        self.monitors
            .last
            .entry((root.to_path_buf(), name.to_owned()))
            .or_insert_with(|| log_of(root, name).and_then(|log| log.last()))
            .clone()
    }

    /// Writes `monitors` into the `apix.toml` of the collection at `root`.
    fn save_monitors(
        &mut self,
        root: &Path,
        monitors: Vec<Monitor>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let saved = Collection::open(root).and_then(|mut c| {
            c.meta.monitors = monitors;
            apix_core::store::save_collection_meta(root, &c.meta)
        });
        match saved {
            Ok(()) => {
                self.reload(window, cx);
                true
            }
            Err(e) => {
                self.notify_error(
                    tr!("monitors.save_error", error = super::describe(&e)),
                    window,
                    cx,
                );
                false
            }
        }
    }

    pub(super) fn open_monitors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.collections.is_empty() {
            self.notify_error(
                tr!("collection.open_first", key = super::key_hint("O")),
                window,
                cx,
            );
            return;
        }
        // Read the logs again: `apix monitor` may have run meanwhile.
        self.monitors.last.clear();
        let workspace = cx.entity().downgrade();
        let view = cx.new(|cx| MonitorsView::new(workspace.clone(), cx));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .on_close(super::collections::refocus(workspace.clone()))
                .bidi_title(tr!("monitors.title"))
                .width(px(760.))
                .child(div().pb_2().child(view.clone()))
        });
    }
}

/// The add or edit form.
struct Form {
    root: PathBuf,
    /// The name of the monitor being edited; `None` for a new one.
    editing: Option<String>,
    name: Entity<InputState>,
    every: Entity<InputState>,
    run: Entity<InputState>,
    webhook: Entity<InputState>,
    environments: Vec<String>,
    environment: Option<String>,
    enabled: bool,
    error: Option<SharedString>,
}

pub(super) struct MonitorsView {
    workspace: WeakEntity<Workspace>,
    form: Option<Form>,
    _observe: Option<Subscription>,
    /// Redraws every second, so "next in" and "min ago" stay true.
    _clock: Task<()>,
}

impl MonitorsView {
    fn new(workspace: WeakEntity<Workspace>, cx: &mut Context<Self>) -> Self {
        let observe = workspace
            .upgrade()
            .map(|ws| cx.observe(&ws, |_, _, cx| cx.notify()));
        let clock = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        Self {
            workspace,
            form: None,
            _observe: observe,
            _clock: clock,
        }
    }

    fn open_form(
        &mut self,
        collection: &Collection,
        environments: Vec<String>,
        m: Option<&Monitor>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = |value: &str,
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
        let blank = Monitor::new("", "5m");
        let m = m.unwrap_or(&blank);
        self.form = Some(Form {
            root: collection.root.clone(),
            editing: (!m.name.is_empty()).then(|| m.name.clone()),
            name: input(&m.name, tr!("monitors.name_placeholder"), window, cx),
            every: input(&m.every, "5m".into(), window, cx),
            run: input(&m.run, tr!("monitors.run_placeholder"), window, cx),
            webhook: input(&m.webhook, "https://hooks.slack.com/…".into(), window, cx),
            environment: m.environment.clone().filter(|e| environments.contains(e)),
            environments,
            enabled: m.enabled,
            error: None,
        });
        cx.notify();
    }

    fn save_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &mut self.form else {
            return;
        };
        let value = |input: &Entity<InputState>| input.read(cx).value().trim().to_string();
        let monitor = Monitor {
            name: value(&form.name),
            every: value(&form.every),
            run: value(&form.run)
                .replace('\\', "/")
                .trim_matches('/')
                .to_owned(),
            environment: form.environment.clone(),
            webhook: value(&form.webhook),
            enabled: form.enabled,
        };
        let Some(workspace) = self.workspace.upgrade() else {
            return;
        };
        let Some(collection) = workspace
            .read(cx)
            .collections
            .iter()
            .find(|c| c.root == form.root)
            .cloned()
        else {
            return;
        };
        let mut monitors = collection.meta.monitors.clone();
        let error = if monitor.name.is_empty() {
            Some(tr!("monitors.name_needed"))
        } else if monitors
            .iter()
            .any(|m| m.name == monitor.name && form.editing.as_ref() != Some(&m.name))
        {
            Some(tr!("monitors.name_taken", name = monitor.name))
        } else if let Err(e) = parse_every(&monitor.every) {
            Some(e.into())
        } else if let Err(e) = monitor::requests(
            &Scope {
                collection: Some(collection.clone()),
                project: None,
            },
            &monitor,
        ) {
            Some(format!("{e:#}").into())
        } else {
            None
        };
        if error.is_some() {
            form.error = error;
            cx.notify();
            return;
        }
        match &form.editing {
            Some(old) => {
                if let Some(slot) = monitors.iter_mut().find(|m| m.name == *old) {
                    *slot = monitor;
                }
            }
            None => monitors.push(monitor),
        }
        let root = form.root.clone();
        let saved = workspace.update(cx, |ws, cx| ws.save_monitors(&root, monitors, window, cx));
        if saved {
            self.form = None;
            cx.notify();
        }
    }

    fn render_form(&self, form: &Form, cx: &mut Context<Self>) -> AnyElement {
        let p = crate::theme::current();
        let label =
            |text: SharedString| div().pt_1().text_xs().text_color(p.text_muted).child(text);
        let hint = |text: SharedString| {
            div()
                .text_xs()
                .text_color(p.text_muted)
                .child(bidi::para(text))
        };
        let environment = Button::new("monitor-env")
            .small()
            .outline()
            .w_full()
            .label(
                form.environment
                    .clone()
                    .map(SharedString::from)
                    .unwrap_or_else(|| tr!("env.none")),
            )
            .dropdown_menu({
                let view = cx.entity().downgrade();
                let (names, current) = (form.environments.clone(), form.environment.clone());
                move |menu, _, _| {
                    let pick = |label: String, env: Option<String>| {
                        let view = view.clone();
                        PopupMenuItem::new(label).checked(current == env).on_click(
                            move |_, _, cx| {
                                view.update(cx, |this, cx| {
                                    if let Some(form) = &mut this.form {
                                        form.environment = env.clone();
                                    }
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
        v_flex()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(match &form.editing {
                        Some(name) => tr!("monitors.editing", name = name),
                        None => tr!("monitors.new"),
                    }),
            )
            .child(label(tr!("monitors.name")))
            .child(Input::new(&form.name).small())
            .child(
                h_flex()
                    .dir_row()
                    .gap_2()
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(label(tr!("monitors.every_label")))
                            .child(Input::new(&form.every).small()),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(label(tr!("monitors.environment")))
                            .child(environment),
                    ),
            )
            .child(hint(tr!("monitors.every_hint")))
            .child(label(tr!("monitors.run")))
            .child(Input::new(&form.run).small())
            .child(label(tr!("monitors.webhook")))
            .child(Input::new(&form.webhook).small())
            .child(hint(tr!("monitors.webhook_hint")))
            .child(
                div().pt_1().child(
                    Checkbox::new("monitor-enabled")
                        .dir_row()
                        .checked(form.enabled)
                        .label(tr!("monitors.enabled"))
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            if let Some(form) = &mut this.form {
                                form.enabled = *checked;
                            }
                            cx.notify();
                        })),
                ),
            )
            .children(form.error.clone().map(|e| {
                div()
                    .pt_1()
                    .text_sm()
                    .text_color(p.err)
                    .child(bidi::para(e))
            }))
            .child(
                h_flex()
                    .dir_row()
                    .gap_2()
                    .pt_2()
                    .justify_end()
                    .child(
                        Button::new("monitor-cancel")
                            .outline()
                            .label(tr!("monitors.cancel"))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.form = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("monitor-save")
                            .primary()
                            .label(tr!("monitors.save"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.save_form(window, cx)
                            })),
                    ),
            )
            .into_any_element()
    }
}

/// `just now`, `5 min ago` …
fn ago(at_ms: u64) -> SharedString {
    let secs = apix_core::history::now_ms().saturating_sub(at_ms) / 1000;
    match secs {
        0..60 => tr!("monitors.ago_now"),
        60..3600 => tr!("monitors.ago_minutes", count = secs / 60),
        3600..86_400 => tr!("monitors.ago_hours", count = secs / 3600),
        _ => tr!("monitors.ago_days", count = secs / 86_400),
    }
}

/// `4m`, `40s`: how long until `at`.
fn until(at: Instant) -> String {
    let left = at.saturating_duration_since(Instant::now());
    let rounded = if left >= Duration::from_secs(60) {
        Duration::from_secs(left.as_secs().div_ceil(60) * 60)
    } else {
        Duration::from_secs(left.as_secs().max(1))
    };
    describe_every(rounded)
}

impl Render for MonitorsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = window;
        let p = crate::theme::current();
        if let Some(form) = &self.form {
            return self.render_form(form, cx);
        }
        let Some(workspace) = self.workspace.upgrade() else {
            return div().into_any_element();
        };
        let in_app = workspace.read(cx).settings.monitors_in_app;
        let collections = workspace.read(cx).collections.clone();
        let running = workspace.read(cx).monitors.running.clone();
        let several = collections.len() > 1;

        let mut body = v_flex().gap_2().child(
            v_flex()
                .gap_1()
                .child(
                    Checkbox::new("monitors-in-app")
                        .dir_row()
                        .checked(in_app)
                        .label(tr!("monitors.in_app"))
                        .on_click({
                            let workspace = workspace.downgrade();
                            move |checked: &bool, _, cx| {
                                let on = *checked;
                                workspace
                                    .update(cx, |ws, cx| {
                                        ws.settings.monitors_in_app = on;
                                        let _ = ws.settings.save();
                                        cx.notify();
                                    })
                                    .ok();
                            }
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(bidi::para(tr!("monitors.in_app_hint"))),
                ),
        );

        for collection in collections {
            let root = collection.root.clone();
            let environments = match workspace.read(cx).project.as_ref() {
                Some(project) if root.starts_with(&project.root) => {
                    project.environment_names(std::slice::from_ref(&collection))
                }
                _ => collection.environment_names(),
            };
            if several {
                body = body.child(
                    div()
                        .pt_2()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.text_muted)
                        .child(crate::i18n::upper(&collection.meta.name)),
                );
            }
            if collection.meta.monitors.is_empty() {
                body = body.child(
                    div()
                        .text_sm()
                        .text_color(p.text_muted)
                        .child(bidi::para(tr!("monitors.empty"))),
                );
            }
            for (i, m) in collection.meta.monitors.iter().enumerate() {
                let last = workspace.update(cx, |ws, _| ws.last_outcome(&root, &m.name));
                let next = workspace
                    .read(cx)
                    .monitors
                    .schedulers
                    .get(&root)
                    .and_then(|s| s.next(m));
                let is_running = running
                    .as_ref()
                    .is_some_and(|(r, n)| *r == root && *n == m.name);
                let (dot, line): (Hsla, SharedString) = if is_running {
                    (p.accent, tr!("monitors.running"))
                } else {
                    match &last {
                        None => (p.text_muted, tr!("monitors.not_run")),
                        Some(o) if o.error.is_some() => (
                            p.err,
                            tr!(
                                "monitors.error",
                                error = o.error.clone().unwrap_or_default(),
                                ago = ago(o.at_ms)
                            ),
                        ),
                        Some(o) if o.ok() => (
                            p.ok,
                            tr!("monitors.passed", count = o.passed, ago = ago(o.at_ms)),
                        ),
                        Some(o) => (
                            p.err,
                            tr!(
                                "monitors.failed",
                                failed = o.failed,
                                total = o.passed + o.failed,
                                ago = ago(o.at_ms)
                            ),
                        ),
                    }
                };
                let mut facts: Vec<String> = vec![match m.interval() {
                    Ok(d) => tr!("monitors.every", every = describe_every(d)).into(),
                    Err(e) => e,
                }];
                facts.push(if m.run.is_empty() {
                    tr!("monitors.whole").into()
                } else {
                    m.run.clone()
                });
                if let Some(env) = &m.environment {
                    facts.push(env.clone());
                }
                if !m.webhook.is_empty() {
                    facts.push(tr!("monitors.webhook_set").into());
                }
                if !m.enabled {
                    facts.push(tr!("monitors.off").into());
                } else if in_app
                    && !is_running
                    && let Some(next) = next
                {
                    facts.push(tr!("monitors.next", time = until(next)).into());
                }
                let failures: Vec<String> = last
                    .as_ref()
                    .filter(|o| !o.ok() && !is_running)
                    .map(|o| o.failures.iter().take(2).cloned().collect())
                    .unwrap_or_default();
                let (run_c, run_m) = (collection.clone(), m.clone());
                let (edit_c, edit_m, edit_envs) =
                    (collection.clone(), m.clone(), environments.clone());
                let (del_root, del_name) = (root.clone(), m.name.clone());
                let ws_run = workspace.downgrade();
                let ws_del = workspace.downgrade();
                body = body.child(
                    h_flex()
                        .dir_row()
                        .gap_3()
                        .items_start()
                        .p_2()
                        .rounded_md()
                        .border_1()
                        .border_color(p.hairline)
                        .child(
                            div()
                                .mt(px(5.))
                                .size(px(9.))
                                .flex_none()
                                .rounded_full()
                                .bg(dot),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(m.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(p.text_muted)
                                        .child(bidi::para(facts.join("  ·  "))),
                                )
                                .child(div().text_xs().text_color(dot).child(bidi::para(line)))
                                .children(failures.into_iter().map(|f| {
                                    div().text_xs().text_color(p.text_muted).truncate().child(f)
                                })),
                        )
                        .child(
                            h_flex()
                                .dir_row()
                                .gap_1()
                                .flex_none()
                                .child(
                                    Button::new(("monitor-run", i))
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::Play)
                                        .tooltip(tr!("monitors.run_now"))
                                        .disabled(running.is_some())
                                        .on_click(move |_, window, cx| {
                                            let (c, m) = (run_c.clone(), run_m.clone());
                                            ws_run
                                                .update(cx, |ws, cx| {
                                                    ws.run_monitor(c, m, window, cx)
                                                })
                                                .ok();
                                        }),
                                )
                                .child(
                                    Button::new(("monitor-edit", i))
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::Settings)
                                        .tooltip(tr!("monitors.edit"))
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, window, cx| {
                                                this.open_form(
                                                    &edit_c,
                                                    edit_envs.clone(),
                                                    Some(&edit_m),
                                                    window,
                                                    cx,
                                                )
                                            },
                                        )),
                                )
                                .child(
                                    Button::new(("monitor-delete", i))
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::Delete)
                                        .tooltip(tr!("monitors.delete"))
                                        .on_click(move |_, window, cx| {
                                            let (root, name) = (del_root.clone(), del_name.clone());
                                            ws_del
                                                .update(cx, |ws, cx| {
                                                    let kept: Vec<Monitor> = ws
                                                        .collections
                                                        .iter()
                                                        .find(|c| c.root == root)
                                                        .map(|c| {
                                                            c.meta
                                                                .monitors
                                                                .iter()
                                                                .filter(|m| m.name != name)
                                                                .cloned()
                                                                .collect()
                                                        })
                                                        .unwrap_or_default();
                                                    ws.save_monitors(&root, kept, window, cx);
                                                })
                                                .ok();
                                        }),
                                ),
                        ),
                );
            }
            let (add_c, add_envs) = (collection.clone(), environments.clone());
            body = body.child(
                h_flex().dir_row().child(
                    Button::new(SharedString::from(format!(
                        "monitor-add-{}",
                        root.display()
                    )))
                    .small()
                    .outline()
                    .icon(IconName::Plus)
                    .label(tr!("monitors.add"))
                    .on_click(cx.listener(
                        move |this, _: &ClickEvent, window, cx| {
                            this.open_form(&add_c, add_envs.clone(), None, window, cx)
                        },
                    )),
                ),
            );
        }
        div()
            .id("monitors")
            .max_h(px(560.))
            .overflow_y_scroll()
            .child(body)
            .into_any_element()
    }
}
