//! Sending a request that has scripts: pre-request scripts in the
//! background, then the request with what they set, then the test scripts.
//! The response pane gets a Tests and a Console tab with the results.

use apix_core::scripts::{self, Event, ScriptFile, ScriptVars};
use apix_core::{Request, project};
use apix_script::{Info, Input, LogLine, Output, ResponseView, TestResult};
use gpui_kit::component::tab::Tab;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::history_ui::PendingRecord;
use super::{Finished, ResponseState, Workspace, script_editor::Scripts, undefined_message};
use crate::bidi::{self, Bidi as _};
use crate::{net, tr};

/// What the scripts of one send reported.
pub(super) struct ScriptReport {
    pub tests: Vec<TestResult>,
    pub logs: Vec<LogLine>,
    /// Script errors, already naming the file.
    pub errors: Vec<String>,
}

impl ScriptReport {
    fn new(pre: &Output, tests: Option<&Output>) -> Self {
        let mut logs = pre.logs.clone();
        let mut errors: Vec<String> = pre.error.iter().cloned().collect();
        if let Some(tests) = tests {
            logs.extend(tests.logs.iter().cloned());
            errors.extend(tests.error.iter().cloned());
        }
        Self {
            tests: tests.map(|t| t.tests.clone()).unwrap_or_default(),
            logs,
            errors,
        }
    }

    fn failed(&self) -> usize {
        self.tests.iter().filter(|t| !t.passed).count()
    }
}

/// The Tests and Console tabs of a response whose request has scripts.
pub(super) fn tests_tab(report: &ScriptReport) -> [Tab; 2] {
    let run = report.tests.iter().filter(|t| !t.skipped).count();
    let passed = report
        .tests
        .iter()
        .filter(|t| t.passed && !t.skipped)
        .count();
    let tests = if report.tests.is_empty() {
        tr!("response.tab.tests")
    } else {
        tr!("response.tab.tests_count", passed = passed, total = run)
    };
    let console = if report.logs.is_empty() {
        tr!("response.tab.console")
    } else {
        tr!("response.tab.console_count", count = report.logs.len())
    };
    [Tab::new().label(tests), Tab::new().label(console)]
}

impl Workspace {
    /// Shows `scripts` in the Scripts tab, with how many folder and
    /// collection scripts run around them.
    pub(super) fn show_scripts(
        &mut self,
        scripts: &Scripts,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let around = |event| {
            let Some(path) = self.selected.as_deref() else {
                return 0;
            };
            let own = scripts::request_script(path, event);
            let root = self.collection_for(path).map(|c| c.root.clone());
            scripts::chain(path, root.as_deref(), event)
                .iter()
                .filter(|s| s.path != own)
                .count()
        };
        let counts = (around(Event::PreRequest), around(Event::Test));
        self.script_editor.update(cx, |editor, cx| {
            editor.set_scripts(scripts, counts, window, cx)
        });
    }

    /// The scripts that run for the open request: the collection's and
    /// folders' from disk, and its own as they are in the editor (saved or not).
    pub(super) fn script_chains(&self, cx: &App) -> (Vec<ScriptFile>, Vec<ScriptFile>) {
        let own = self.script_editor.read(cx).scripts(cx);
        let chain = |event: Event, source: &str| {
            let (mut chain, own_path) = match self.selected.as_deref() {
                Some(path) => {
                    let root = self.collection_for(path).map(|c| c.root.clone());
                    (
                        scripts::chain(path, root.as_deref(), event),
                        scripts::request_script(path, event),
                    )
                }
                None => (Vec::new(), format!("request.{}.js", event.suffix()).into()),
            };
            chain.retain(|s| s.path != own_path);
            if !source.trim().is_empty() {
                chain.push(ScriptFile {
                    path: own_path,
                    source: source.to_owned(),
                });
            }
            chain
        };
        (
            chain(Event::PreRequest, &own.pre),
            chain(Event::Test, &own.test),
        )
    }

    /// Sends `request` (unresolved, with inherited auth and settings) with
    /// its scripts. Scripts and formatting run off the UI thread.
    pub(super) fn send_scripted(
        &mut self,
        request: Request,
        record: Option<PendingRecord>,
        pre_scripts: Vec<ScriptFile>,
        test_scripts: Vec<ScriptFile>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let scopes = project::scope_values(
            self.project.as_ref(),
            self.collection.as_ref(),
            self.environment.as_deref(),
        )
        .unwrap_or_default();
        let state = self.script_vars.clone();
        let base_vars = self.vars.clone();
        let info = Info {
            request_name: request.name.clone(),
            request_id: self
                .selected
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            iteration: 0,
            iteration_count: 1,
        };
        let body_dir = self.body_location().0;
        let tab = self.selected.clone();
        self.response = ResponseState::Loading;
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let pre = {
                let (request, scopes, state, info) =
                    (request.clone(), scopes.clone(), state.clone(), info.clone());
                cx.background_spawn(async move {
                    apix_script::run(
                        &pre_scripts,
                        &Input {
                            event: Event::PreRequest,
                            request: &request,
                            response: None,
                            scopes: &scopes,
                            vars: &state,
                            info,
                        },
                    )
                })
                .await
            };
            if pre.error.is_some() || pre.skip_request {
                let message = match &pre.error {
                    Some(error) => tr!("scripts.pre_error", error = error),
                    None => tr!("scripts.skipped"),
                };
                let logs: Vec<String> = pre.logs.iter().map(|l| l.text.clone()).collect();
                let message = if logs.is_empty() {
                    message.to_string()
                } else {
                    format!("{message}\n\n{}", logs.join("\n"))
                };
                this.update_in(cx, |this, window, cx| {
                    this.keep_script_vars(pre.vars.clone(), window, cx);
                    this.finish_for(tab, Err(message), window, cx);
                })
                .ok();
                return;
            }
            let resolved = pre.request.resolve(&pre.vars.apply(base_vars));
            if !resolved.missing.is_empty() {
                this.update_in(cx, |this, window, cx| {
                    this.keep_script_vars(pre.vars.clone(), window, cx);
                    let message = undefined_message(resolved.missing).to_string();
                    this.finish_for(tab, Err(message), window, cx);
                })
                .ok();
                return;
            }
            let sent = match &body_dir {
                Some(dir) => resolved.request.with_files_relative_to(dir),
                None => resolved.request,
            };
            let result = net::send(sent).await;
            let (prepared, vars) = cx
                .background_spawn(async move {
                    if let Some(record) = record {
                        record.save(&result);
                    }
                    match result {
                        Ok(response) => {
                            let view = ResponseView::new(
                                response.status,
                                response.reason.unwrap_or(""),
                                &response.headers,
                                &response.body,
                                response.timing.total,
                            );
                            let tests = apix_script::run(
                                &test_scripts,
                                &Input {
                                    event: Event::Test,
                                    request: &pre.request,
                                    response: Some(&view),
                                    scopes: &scopes,
                                    vars: &pre.vars,
                                    info,
                                },
                            );
                            let mut finished = Finished::prepare(response);
                            finished.scripts = Some(ScriptReport::new(&pre, Some(&tests)));
                            (Ok(finished), tests.vars)
                        }
                        Err(error) => (Err(error), pre.vars.clone()),
                    }
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.keep_script_vars(vars, window, cx);
                this.finish_for(tab, prepared, window, cx);
            })
            .ok();
        }));
        cx.notify();
    }

    /// Keeps what scripts set for the next sends (`pm.variables` only lasts
    /// one request) and shows it in the variables.
    fn keep_script_vars(
        &mut self,
        mut vars: ScriptVars,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        vars.clear_local();
        if vars != self.script_vars {
            self.script_vars = vars;
            self.rebuild_vars(window, cx);
        }
    }

    pub(super) fn render_tests(&self, done: &Finished) -> impl IntoElement {
        let p = self.palette;
        let Some(report) = &done.scripts else {
            return v_flex().into_any_element();
        };
        let failed = report.failed();
        let total = report.tests.iter().filter(|t| !t.skipped).count();
        let summary = if report.tests.is_empty() {
            tr!("tests.none")
        } else {
            tr!("tests.summary", passed = total - failed, failed = failed)
        };
        let rows = report.tests.iter().map(|test| {
            let (mark, color) = if test.skipped {
                ("–", p.text_muted)
            } else if test.passed {
                ("✓", p.ok)
            } else {
                ("✗", p.err)
            };
            v_flex()
                .gap_0p5()
                .py_1()
                .border_b_1()
                .border_color(p.hairline)
                .child(
                    h_flex()
                        .dir_row()
                        .gap_2()
                        .child(div().w(px(14.)).flex_none().text_color(color).child(mark))
                        .child(div().text_sm().text_color(p.text).child(test.name.clone()))
                        .when(test.skipped, |row| {
                            row.child(
                                div()
                                    .text_xs()
                                    .text_color(p.text_muted)
                                    .child(tr!("tests.skipped")),
                            )
                        }),
                )
                .children(test.error.clone().map(|error| {
                    div()
                        .ps(px(22.))
                        .ltr()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(error)
                }))
        });
        v_flex()
            .id("test-results")
            .size_full()
            .overflow_y_scroll()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(if failed > 0 { p.err } else { p.text_muted })
                    .child(summary),
            )
            .children(report.errors.iter().map(|error| {
                div()
                    .text_xs()
                    .ltr()
                    .text_color(p.err)
                    .child(tr!("scripts.error", error = error))
            }))
            .children(rows)
            .into_any_element()
    }

    pub(super) fn render_console(&self, done: &Finished, cx: &App) -> impl IntoElement {
        use gpui_kit::component::ActiveTheme as _;
        let p = self.palette;
        let Some(report) = &done.scripts else {
            return v_flex().into_any_element();
        };
        if report.logs.is_empty() {
            return div()
                .text_xs()
                .text_color(p.text_muted)
                .child(bidi::para(tr!("console.empty")))
                .into_any_element();
        }
        let lines = report.logs.iter().map(|line| {
            let color = match line.level.as_str() {
                "error" => p.err,
                "warn" => p.warn,
                "info" => p.accent,
                "debug" => p.text_muted,
                _ => p.text,
            };
            div()
                .py_0p5()
                .border_b_1()
                .border_color(p.hairline)
                .text_xs()
                .text_color(color)
                .child(line.text.clone())
        });
        v_flex()
            .id("script-console")
            .ltr()
            .size_full()
            .overflow_y_scroll()
            .font_family(cx.theme().mono_font_family.clone())
            .children(lines)
            .into_any_element()
    }
}
