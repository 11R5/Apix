//! Running requests the way `apix send`, `apix run` and the desktop Runner
//! do: pre-request scripts, the request with variables filled in, then test
//! scripts; and for a run, the requests in order, as many iterations as
//! asked (one per data file row by default), with a pause between requests,
//! `setNextRequest`, stopping at the first failure (`bail`), and pausing or
//! stopping from outside.

pub mod data;
pub mod report;
pub mod vars;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use apix_core::scripts::{Event, ScopeValues, ScriptFile, ScriptVars, chain};
use apix_core::store::load_request;
use apix_core::{Collection, Project, Request, RequestSettings, Variables};
use apix_http::{HttpClient, Response};
use apix_script::{Info, Input, LogLine, Output, ResponseView, Sender};

pub use data::Row;
pub use report::{ItemResult, RunReport, Summary};

/// The collection, and the project around it, that a file or folder belongs to.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    pub collection: Option<Collection>,
    pub project: Option<Project>,
}

impl Scope {
    pub fn find(start: &Path) -> Result<Self> {
        let collection = Collection::find_root(start)
            .map(Collection::open)
            .transpose()?;
        let project = collection
            .as_ref()
            .and_then(|c| c.root.parent())
            .and_then(Project::find_root)
            .map(Project::open)
            .transpose()?;
        Ok(Self {
            collection,
            project,
        })
    }

    /// Loads a request and fills in the auth and transport settings it
    /// inherits from its folders, collection or project.
    pub fn load(&self, path: &Path) -> Result<Request> {
        let mut request = load_request(path)?;
        apix_core::project::apply_inherited_auth(
            &mut request,
            path,
            self.collection.as_ref(),
            self.project.as_ref(),
        )?;
        apix_core::project::apply_inherited_settings(
            &mut request,
            path,
            self.collection.as_ref(),
            self.project.as_ref(),
        )?;
        Ok(request)
    }
}

/// One request with its scripts: what the pre-request scripts did, the
/// request as sent and its response (unless a script stopped it), and what
/// the test scripts did.
pub struct Exchange {
    pub pre: Output,
    pub sent: Option<Request>,
    pub response: Option<Response>,
    pub tests: Output,
}

impl Exchange {
    /// Whether this request counts as passed: no script error, no failed
    /// test, and, for requests without tests, a successful status.
    pub fn passed(&self) -> bool {
        let scripts_ok = self.pre.error.is_none()
            && self.tests.error.is_none()
            && self.tests.failed_tests() == 0;
        let status_ok =
            !self.tests.tests.is_empty() || self.response.as_ref().is_none_or(Response::is_success);
        scripts_ok && status_ok && (self.response.is_some() || self.pre.skip_request)
    }

    /// The request for a report or a run's progress.
    pub fn to_item(&self, iteration: u32, path: String) -> ItemResult {
        let request = self.sent.as_ref().unwrap_or(&self.pre.request);
        let mut logs = self.pre.logs.clone();
        logs.extend(self.tests.logs.iter().cloned());
        let response = self.response.as_ref();
        ItemResult {
            iteration,
            name: request.name.clone(),
            path,
            method: request.method.to_string(),
            url: request.url.clone(),
            status: response.map(|r| r.status),
            reason: response
                .and_then(|r| r.reason)
                .unwrap_or_default()
                .to_owned(),
            time_ms: response.map_or(0.0, |r| r.timing.total.as_secs_f64() * 1000.0),
            size: response.map_or(0, |r| r.body.len() as u64),
            tests: self.tests.tests.clone(),
            logs,
            error: None,
            pre_error: self.pre.error.clone(),
            test_error: self.tests.error.clone(),
            skipped: self.pre.skip_request,
            passed: self.passed(),
        }
    }
}

/// Sends the request at `path`: pre-request scripts, then the request with
/// variables filled in, then test scripts. `state` carries what scripts set
/// from one request to the next; `progress` sees the response as it arrives
/// (for streams).
#[allow(clippy::too_many_arguments)]
pub async fn exchange(
    client: &HttpClient,
    scope: &Scope,
    path: &Path,
    vars: &Variables,
    scopes: &ScopeValues,
    state: &mut ScriptVars,
    overrides: &RequestSettings,
    info: Info,
    sender: Sender,
    progress: impl FnMut(apix_http::Progress<'_>) + Send,
) -> Result<Exchange> {
    let mut request = scope.load(path)?;
    request.settings = overrides.clone().or_inherit(&request.settings);
    let mut info = info;
    if info.request_name.is_empty() {
        info.request_name = request.name.clone();
    }
    let root = scope.collection.as_ref().map(|c| c.root.as_path());
    let pre = run_scripts(
        &chain(path, root, Event::PreRequest),
        &Input {
            event: Event::PreRequest,
            request: &request,
            response: None,
            scopes,
            vars: state,
            info: info.clone(),
            sender: Some(sender.clone()),
        },
    );
    *state = pre.vars.clone();
    let mut done = Exchange {
        pre,
        sent: None,
        response: None,
        tests: Output::default(),
    };
    if done.pre.error.is_some() || done.pre.skip_request {
        return Ok(done);
    }
    let resolved = done.pre.request.resolve(&state.apply(vars.clone()));
    if !resolved.missing.is_empty() {
        let names: Vec<_> = resolved.missing.into_iter().collect();
        bail!("undefined variable(s): {}", names.join(", "));
    }
    // A WebSocket, Socket.IO or MQTT conversation has no single response to
    // test; runs skip it.
    let kind = if resolved.request.is_mqtt() {
        Some("MQTT")
    } else if resolved.request.is_socketio() {
        Some("Socket.IO")
    } else if apix_http::ws::is_websocket_url(&resolved.request.url) {
        Some("WebSocket")
    } else {
        None
    };
    if let Some(kind) = kind {
        done.pre.skip_request = true;
        done.pre.logs.push(LogLine {
            level: "info".into(),
            text: format!("{kind} request: not part of runs (use apix send)"),
        });
        return Ok(done);
    }
    done.sent = Some(resolved.request.clone());
    // Body files (and .proto files) are relative to the request file.
    let sent = match path.parent() {
        Some(dir) => resolved.request.with_files_relative_to(dir),
        None => resolved.request,
    };
    // A gRPC call is tested like an HTTP response (`CallOutcome::to_response`).
    let response = if sent.is_grpc() {
        apix_grpc::send_for_tests(&sent).await?
    } else {
        client.send_with(&sent, progress).await?
    };
    let view = ResponseView::new(
        response.status,
        response.reason.unwrap_or(""),
        &response.headers,
        &response.body,
        response.timing.total,
    );
    let mut tests = chain(path, root, Event::Test);
    tests.extend(apix_script::assertions_script(&done.pre.request.asserts));
    done.tests = run_scripts(
        &tests,
        &Input {
            event: Event::Test,
            request: &done.pre.request,
            response: Some(&view),
            scopes,
            vars: state,
            info,
            sender: Some(sender),
        },
    );
    *state = done.tests.vars.clone();
    done.response = Some(response);
    Ok(done)
}

/// Runs scripts on their own thread: `pm.sendRequest` blocks until its
/// response is in, which must not happen on an async runtime's thread.
pub fn run_scripts(scripts: &[ScriptFile], input: &Input) -> Output {
    if scripts.is_empty() {
        return apix_script::run(scripts, input);
    }
    std::thread::scope(|scope| {
        scope
            .spawn(|| apix_script::run(scripts, input))
            .join()
            .unwrap_or_else(|_| Output {
                request: input.request.clone(),
                vars: input.vars.clone(),
                error: Some("the script engine stopped unexpectedly".into()),
                ..Default::default()
            })
    })
}

/// Pausing and stopping a run from another thread (the Runner's buttons).
#[derive(Debug, Default)]
pub struct Control {
    paused: AtomicBool,
    stopped: AtomicBool,
}

impl Control {
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
    }

    /// Ends the run after the request being sent.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }

    /// Waits while paused; false once stopped.
    async fn go_on(&self) -> bool {
        while self.is_paused() && !self.is_stopped() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        !self.is_stopped()
    }

    /// Waits `delay` (paused time not counted); false once stopped.
    async fn wait(&self, delay: Duration) -> bool {
        let mut left = delay;
        while !left.is_zero() {
            if !self.go_on().await {
                return false;
            }
            let step = left.min(Duration::from_millis(50));
            tokio::time::sleep(step).await;
            left -= step;
        }
        self.go_on().await
    }
}

/// What to run and how.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// The collection's root; reports name requests relative to it.
    pub root: PathBuf,
    /// The request files, in the order to send them.
    pub requests: Vec<PathBuf>,
    /// Every variable, the environment's included.
    pub vars: Variables,
    /// The same values by scope, for scripts.
    pub scopes: ScopeValues,
    pub environment: Option<String>,
    /// Values that win over a data row too (`apix run --var`).
    pub pinned: Vec<(String, String)>,
    /// One row per iteration.
    pub data: Vec<Row>,
    /// 0: one per data row, or one without data. With more iterations than
    /// rows, the rows start again from the first.
    pub iterations: u32,
    /// Between two requests.
    pub delay: Duration,
    /// Stop at the first request that fails.
    pub bail: bool,
    /// Transport settings that win over the requests' own.
    pub overrides: RequestSettings,
    /// What scripts set before the run (the desktop app's session); the
    /// run goes on from there.
    pub state: ScriptVars,
}

impl Plan {
    pub fn iteration_count(&self) -> u32 {
        match (self.iterations, self.data.len()) {
            (0, 0) => 1,
            (0, rows) => rows as u32,
            (n, _) => n,
        }
    }

    /// The variables and script scopes of iteration `ix`.
    fn iteration(&self, ix: u32) -> (Variables, ScopeValues, Option<&Row>) {
        let mut vars = self.vars.clone();
        let mut scopes = self.scopes.clone();
        let row = (!self.data.is_empty()).then(|| &self.data[ix as usize % self.data.len()]);
        for (k, v) in row
            .into_iter()
            .flatten()
            .chain(self.pinned.iter().map(|(k, v)| (k, v)))
        {
            vars.set(k, v);
            scopes.data.insert(k.clone(), v.clone());
        }
        (vars, scopes, row)
    }

    fn label(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }
}

/// What a run reports while it goes.
pub enum Progress<'a> {
    /// Iteration `n` (0-based) starts, with its data row.
    Iteration(u32, Option<&'a Row>),
    /// A request is about to be sent.
    Sending(&'a Path),
    /// A request is done.
    Item(&'a ItemResult),
}

/// Runs `plan`. Stopping (through `control` or `bail`) still returns what
/// was done so far.
pub async fn run(
    scope: &Scope,
    plan: &Plan,
    client: &HttpClient,
    sender: Sender,
    control: &Control,
    mut progress: impl FnMut(Progress),
) -> RunReport {
    let started = Instant::now();
    let iterations = plan.iteration_count();
    let mut report = RunReport {
        name: scope
            .collection
            .as_ref()
            .map(|c| c.meta.name.clone())
            .unwrap_or_default(),
        environment: plan.environment.clone(),
        started_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64),
        iterations,
        ..Default::default()
    };
    let names: Vec<String> = plan
        .requests
        .iter()
        .map(|p| load_request(p).map(|r| r.name).unwrap_or_default())
        .collect();
    let mut state = plan.state.clone();
    let mut first = true;
    'run: for iteration in 0..iterations {
        let (vars, scopes, row) = plan.iteration(iteration);
        progress(Progress::Iteration(iteration, row));
        let mut at = 0;
        // `setNextRequest` may loop (polling); a ceiling keeps a mistake from running forever.
        let mut steps = 0;
        while at < plan.requests.len() && steps < plan.requests.len() * 100 {
            steps += 1;
            let delay = if first { Duration::ZERO } else { plan.delay };
            first = false;
            if !control.wait(delay).await {
                report.stopped = true;
                break 'run;
            }
            let path = &plan.requests[at];
            let label = plan.label(path);
            progress(Progress::Sending(path));
            let info = Info {
                request_name: names[at].clone(),
                request_id: label.clone(),
                iteration,
                iteration_count: iterations,
            };
            let mut next = at + 1;
            let item = match exchange(
                client,
                scope,
                path,
                &vars,
                &scopes,
                &mut state,
                &plan.overrides,
                info,
                sender.clone(),
                |_| {},
            )
            .await
            {
                Ok(done) => {
                    let mut item = done.to_item(iteration, label);
                    let wanted = done
                        .tests
                        .next_request
                        .clone()
                        .or(done.pre.next_request.clone());
                    match wanted {
                        Some(None) => next = plan.requests.len(),
                        Some(Some(name)) => match names.iter().position(|n| *n == name) {
                            Some(ix) => next = ix,
                            None => item.logs.push(LogLine {
                                level: "warn".into(),
                                text: format!("setNextRequest: no request named \"{name}\""),
                            }),
                        },
                        None => {}
                    }
                    item
                }
                Err(err) => {
                    let request = load_request(path).ok();
                    ItemResult {
                        iteration,
                        name: names[at].clone(),
                        path: label,
                        method: request
                            .as_ref()
                            .map(|r| r.method.to_string())
                            .unwrap_or_default(),
                        url: request.map(|r| r.url).unwrap_or_default(),
                        error: Some(format!("{err:#}")),
                        ..Default::default()
                    }
                }
            };
            progress(Progress::Item(&item));
            let failed = !item.passed;
            report.items.push(item);
            if failed && plan.bail {
                report.stopped = true;
                break 'run;
            }
            at = next;
        }
    }
    report.duration_ms = started.elapsed().as_millis() as u64;
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterations_follow_the_data() {
        let row = |v: &str| Row::from([("id".to_string(), v.to_string())]);
        let mut plan = Plan::default();
        assert_eq!(plan.iteration_count(), 1);
        plan.data = vec![row("1"), row("2"), row("3")];
        assert_eq!(plan.iteration_count(), 3);
        plan.iterations = 5;
        assert_eq!(plan.iteration_count(), 5);
        // Rows start again; a pinned value wins over the row.
        let (vars, scopes, _) = plan.iteration(4);
        assert_eq!(vars.get("id"), Some("2"));
        plan.pinned = vec![("id".into(), "9".into())];
        let (vars, scopes2, _) = plan.iteration(4);
        assert_eq!(vars.get("id"), Some("9"));
        assert_eq!(scopes.data["id"], "2");
        assert_eq!(scopes2.data["id"], "9");
    }

    #[test]
    fn control_waits_and_stops() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let control = Control::default();
        rt.block_on(async {
            let t = Instant::now();
            assert!(control.wait(Duration::from_millis(120)).await);
            assert!(t.elapsed() >= Duration::from_millis(100));
            control.stop();
            assert!(!control.wait(Duration::from_secs(5)).await);
        });
    }
}
