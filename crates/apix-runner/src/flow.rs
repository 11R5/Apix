//! Running a flow (`apix_core::flow`): from the start node, each node does
//! its work and its outcome picks the edge to the next one. Requests go
//! through [`exchange`](crate::exchange) like in a run, so their scripts and
//! tests work, and what a node extracts or sets is a variable for the nodes
//! after it.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use apix_core::flow::{Flow, NodeKind, Seen};
use apix_core::scripts::{ScopeValues, ScriptVars};
use apix_core::{RequestSettings, Variables};
use apix_http::HttpClient;
use apix_script::{Info, Sender};
use serde::Serialize;

use crate::{Control, ItemResult, RunReport, Scope, exchange};

/// A loop without an end (a check that never turns) stops here.
pub const MOST_STEPS: usize = 1000;

/// What to run and with which values.
#[derive(Debug, Clone, Default)]
pub struct FlowPlan {
    pub flow: Flow,
    /// Request paths in the flow are relative to this folder (the
    /// collection's root, else the flow file's folder).
    pub base: PathBuf,
    pub vars: Variables,
    pub scopes: ScopeValues,
    pub environment: Option<String>,
    pub overrides: RequestSettings,
    /// What scripts set before (the desktop app's session).
    pub state: ScriptVars,
}

impl FlowPlan {
    /// The folder request paths are relative to, for a flow file.
    pub fn base_for(scope: &Scope, flow_file: &Path) -> PathBuf {
        scope
            .collection
            .as_ref()
            .map(|c| c.root.clone())
            .or_else(|| flow_file.parent().map(Path::to_path_buf))
            .unwrap_or_default()
    }
}

/// One node done.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Step {
    pub node: String,
    pub kind: String,
    /// The outcome (`ok`, `fail`, `yes`, `no`, `next`); none when stopped.
    pub port: Option<String>,
    /// What happened, in words: a check's values, what was set or extracted, an error.
    pub note: String,
    /// The request, for request nodes.
    pub item: Option<ItemResult>,
    pub time_ms: u64,
}

impl Step {
    pub fn failed(&self) -> bool {
        self.port.as_deref() == Some("fail")
    }
}

/// What a flow run did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FlowReport {
    pub name: String,
    pub steps: Vec<Step>,
    /// It ended at a node with no edge for its outcome, without a request
    /// failing there, and was not stopped.
    pub passed: bool,
    pub stopped: bool,
    /// Why it ended, when not at a node without an edge.
    pub ended: Option<String>,
    pub duration_ms: u64,
}

impl FlowReport {
    /// The requests as a run report, for JUnit, HTML or Markdown output.
    pub fn to_run_report(&self, environment: Option<String>) -> RunReport {
        RunReport {
            name: self.name.clone(),
            environment,
            iterations: 1,
            items: self.steps.iter().filter_map(|s| s.item.clone()).collect(),
            stopped: self.stopped,
            duration_ms: self.duration_ms,
            ..Default::default()
        }
    }
}

/// What a flow run reports while it goes.
pub enum FlowProgress<'a> {
    /// A node starts.
    Node(&'a str),
    /// A node is done.
    Step(&'a Step),
}

/// Runs `plan.flow`. Stopping through `control` still returns the steps done.
pub async fn run(
    scope: &Scope,
    plan: &FlowPlan,
    client: &HttpClient,
    sender: Sender,
    control: &Control,
    mut progress: impl FnMut(FlowProgress) + Send,
) -> FlowReport {
    let started = Instant::now();
    let flow = &plan.flow;
    let mut report = FlowReport {
        name: flow.name.clone(),
        ..Default::default()
    };
    let problems = flow.problems();
    if !problems.is_empty() {
        report.ended = Some(problems.join("; "));
        return report;
    }
    let mut state = plan.state.clone();
    let mut last: Option<Seen> = None;
    let mut at = flow.start_node().map(str::to_owned);
    let mut passed = true;
    while let Some(id) = at.take() {
        if report.steps.len() >= MOST_STEPS {
            report.ended = Some(format!("stopped after {MOST_STEPS} steps"));
            passed = false;
            break;
        }
        if !control.go_on().await {
            report.stopped = true;
            break;
        }
        let Some(node) = flow.node(&id) else { break };
        progress(FlowProgress::Node(&id));
        let began = Instant::now();
        let mut step = Step {
            node: id.clone(),
            kind: node.kind.name().into(),
            ..Default::default()
        };
        let vars = state.apply(plan.vars.clone());
        match &node.kind {
            NodeKind::Request { request, extract } => {
                let path = plan.base.join(request);
                let info = Info {
                    request_name: String::new(),
                    request_id: request.clone(),
                    iteration: 0,
                    iteration_count: 1,
                };
                match exchange(
                    client,
                    scope,
                    &path,
                    &plan.vars,
                    &plan.scopes,
                    &mut state,
                    &plan.overrides,
                    info,
                    sender.clone(),
                    |_| {},
                )
                .await
                {
                    Ok(done) => {
                        let item = done.to_item(0, request.replace('\\', "/"));
                        let mut ok = item.passed;
                        if let Some(r) = &done.response {
                            last = Some(Seen {
                                status: r.status,
                                headers: r.headers.clone(),
                                body: String::from_utf8_lossy(&r.body).into_owned(),
                                time_ms: r.timing.total.as_secs_f64() * 1000.0,
                            });
                        }
                        let mut notes = Vec::new();
                        for (name, subject) in extract {
                            match Seen::subject(last.as_ref(), subject) {
                                Ok(Some(value)) => {
                                    notes.push(format!("{name} = {}", short(&value)));
                                    state.local.insert(name.clone(), Some(value));
                                }
                                Ok(None) => {
                                    ok = false;
                                    notes.push(format!("{name}: nothing at {subject}"));
                                }
                                Err(e) => {
                                    ok = false;
                                    notes.push(format!("{name}: {e}"));
                                }
                            }
                        }
                        step.note = notes.join(", ");
                        step.port = Some(if ok { "ok" } else { "fail" }.into());
                        step.item = Some(item);
                    }
                    Err(err) => {
                        step.note = format!("{err:#}");
                        step.port = Some("fail".into());
                        step.item = Some(ItemResult {
                            name: request.clone(),
                            path: request.replace('\\', "/"),
                            error: Some(format!("{err:#}")),
                            ..Default::default()
                        });
                    }
                }
            }
            NodeKind::Check { subject, op, value } => {
                let want = interpolate(&vars, value);
                let got = if Seen::is_response_subject(subject) {
                    Seen::subject(last.as_ref(), subject)
                } else {
                    {
                        // A text with an undefined variable has no value.
                        let mut missing = Default::default();
                        let text = vars.interpolate(subject, &mut missing);
                        Ok((missing.is_empty() && !text.is_empty()).then_some(text))
                    }
                };
                match got {
                    Ok(got) => {
                        let yes = op.test(got.as_deref(), &want);
                        let shown = got.as_deref().map_or("nothing".into(), short);
                        step.note = if op.takes_value() {
                            format!("{subject} is {shown}; {} {}", op.name(), short(&want))
                        } else {
                            format!("{subject} is {shown}; {}", op.name())
                        };
                        step.port = Some(if yes { "yes" } else { "no" }.into());
                    }
                    Err(e) => {
                        step.note = format!("{subject}: {e}");
                        step.port = Some("no".into());
                    }
                }
            }
            NodeKind::Set { values } => {
                let mut notes = Vec::new();
                for (name, value) in values {
                    let value = interpolate(&vars, value);
                    notes.push(format!("{name} = {}", short(&value)));
                    state.local.insert(name.clone(), Some(value));
                }
                step.note = notes.join(", ");
                step.port = Some("next".into());
            }
            NodeKind::Delay { ms } => {
                step.note = format!("{ms} ms");
                if control.wait(Duration::from_millis(*ms)).await {
                    step.port = Some("next".into());
                } else {
                    report.stopped = true;
                }
            }
        }
        step.time_ms = began.elapsed().as_millis() as u64;
        progress(FlowProgress::Step(&step));
        let port = step.port.clone();
        report.steps.push(step);
        let Some(port) = port else { break };
        match flow.next(&id, &port) {
            Some(next) => at = Some(next.to_owned()),
            None if port == "fail" => passed = false,
            None => {}
        }
    }
    report.passed = passed && !report.stopped && report.ended.is_none();
    report.duration_ms = started.elapsed().as_millis() as u64;
    report
}

fn interpolate(vars: &Variables, text: &str) -> String {
    vars.interpolate(text, &mut Default::default())
}

/// A value for a note: one line, at most 80 characters.
fn short(value: &str) -> String {
    let line = value.lines().next().unwrap_or_default();
    let mut out: String = line.chars().take(80).collect();
    if out.len() < value.len() {
        out.push('…');
    }
    out
}
