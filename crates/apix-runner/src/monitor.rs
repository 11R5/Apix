//! Running monitors (`[[monitor]]` in `apix.toml`, see `apix_core::monitor`):
//! one run of a monitor, what it came to ([`Outcome`]), a log of those per
//! monitor, when each is due ([`Scheduler`]) and telling a webhook when a
//! monitor starts failing or passes again. `apix monitor` and the desktop
//! app share all of it, and the same log, so the app shows what a monitor
//! running in a terminal on the same computer found.

use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use apix_core::monitor::Monitor;
use apix_core::{Body, KeyValue, Method, Request, RequestSettings, Variables};
use apix_http::HttpClient;
use apix_script::Sender;
use serde::{Deserialize, Serialize};

use crate::{Control, Plan, RunReport, Scope, vars};

/// What one run of a monitor came to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    /// When it started, in milliseconds since 1970 (UTC).
    pub at_ms: u64,
    pub passed: usize,
    pub failed: usize,
    #[serde(default)]
    pub tests_failed: usize,
    pub duration_ms: u64,
    /// The first few failures, `users/get.apix: got 500`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<String>,
    /// The run could not start (an unknown environment, a missing folder).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Outcome {
    pub fn ok(&self) -> bool {
        self.failed == 0 && self.error.is_none()
    }

    fn from_report(report: &RunReport) -> Self {
        let summary = report.summary();
        let failures = report
            .items
            .iter()
            .filter(|i| !i.passed && !i.skipped)
            .take(5)
            .map(|i| {
                let why = i
                    .errors()
                    .into_iter()
                    .chain(i.tests.iter().filter(|t| !t.passed && !t.skipped).map(
                        |t| match &t.error {
                            Some(e) => format!("{}: {e}", t.name),
                            None => t.name.clone(),
                        },
                    ))
                    .next()
                    .unwrap_or_else(|| match i.status {
                        Some(code) => format!("got {code} {}", i.reason).trim_end().to_owned(),
                        None => "failed".to_owned(),
                    });
                format!("{}: {why}", i.path)
            })
            .collect();
        Self {
            at_ms: report.started_ms,
            passed: summary.passed,
            failed: summary.failed,
            tests_failed: summary.tests_failed,
            duration_ms: report.duration_ms,
            failures,
            error: None,
        }
    }

    fn from_error(at_ms: u64, error: String) -> Self {
        Self {
            at_ms,
            error: Some(error),
            ..Self::default()
        }
    }

    /// `3 passed` or `1 of 4 failed`, or the error.
    pub fn summary(&self) -> String {
        match &self.error {
            Some(e) => e.clone(),
            None if self.failed == 0 => format!("{} passed", self.passed),
            None => format!("{} of {} failed", self.failed, self.passed + self.failed),
        }
    }
}

/// The requests a monitor sends: the collection's, or those under its `run`.
pub fn requests(scope: &Scope, monitor: &Monitor) -> Result<Vec<PathBuf>> {
    let collection = scope.collection.as_ref().context("no collection")?;
    let mut requests = collection.request_paths();
    let run = monitor.run.trim().trim_matches('/');
    if !run.is_empty() {
        let start = collection.root.join(run);
        requests.retain(|p| p.starts_with(&start));
        if requests.is_empty() {
            bail!("`{run}` holds no requests");
        }
    }
    Ok(requests)
}

/// The run of `monitor`: its environment, `APIX_VAR_*` variables from the
/// process, then `pinned` (an env file, `--var`) on top.
pub fn plan(scope: &Scope, monitor: &Monitor, pinned: &[(String, String)]) -> Result<Plan> {
    let collection = scope.collection.as_ref().context("no collection")?;
    let project = scope.project.as_ref();
    let env = monitor.environment.as_deref().filter(|e| !e.is_empty());
    if let Some(name) = env {
        let names = match project {
            Some(p) => p.environment_names(std::slice::from_ref(collection)),
            None => collection.environment_names(),
        };
        if !names.iter().any(|n| n == name) {
            bail!("unknown environment `{name}`");
        }
    }
    let requests = requests(scope, monitor)?;
    let mut vars = apix_core::project::variables(project, Some(collection), env)?;
    let mut scopes = apix_core::project::scope_values(project, Some(collection), env)?;
    // Names the collection defines or its requests use, to match `APIX_VAR_TOKEN` to `token`.
    let mut known: Vec<String> = vars.iter().map(|(k, _)| k.to_owned()).collect();
    for path in &requests {
        if let Ok(request) = apix_core::store::load_request(path) {
            known.extend(request.resolve(&Variables::new()).missing);
        }
    }
    known.extend(braced_names(&monitor.webhook));
    let mut overrides = vars::match_known(
        vars::from_environment(std::env::vars()),
        known.iter().map(String::as_str),
    );
    overrides.extend(pinned.iter().cloned());
    for (k, v) in &overrides {
        vars.set(k, v);
    }
    scopes.data.extend(overrides.iter().cloned());
    Ok(Plan {
        root: collection.root.clone(),
        requests,
        vars,
        scopes,
        environment: env.map(str::to_owned),
        pinned: overrides,
        ..Plan::default()
    })
}

/// `{{name}}`s in a text.
fn braced_names(text: &str) -> Vec<String> {
    text.split("{{")
        .skip(1)
        .filter_map(|rest| {
            rest.split_once("}}")
                .map(|(name, _)| name.trim().to_owned())
        })
        .collect()
}

/// Runs `monitor` once, with `overrides` winning over the requests' transport
/// settings. The report is `None` when the run could not start.
pub async fn run_once(
    scope: &Scope,
    monitor: &Monitor,
    client: &HttpClient,
    sender: Sender,
    pinned: &[(String, String)],
    overrides: &RequestSettings,
) -> (Option<RunReport>, Outcome) {
    let at_ms = apix_core::history::now_ms();
    let mut plan = match plan(scope, monitor, pinned) {
        Ok(plan) => plan,
        Err(e) => return (None, Outcome::from_error(at_ms, format!("{e:#}"))),
    };
    plan.overrides = overrides.clone();
    let report = crate::run(scope, &plan, client, sender, &Control::default(), |_| {}).await;
    let outcome = Outcome::from_report(&report);
    (Some(report), outcome)
}

/// Where monitor logs go by default: `<config>/Apix/monitors`.
pub fn default_dir() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("Apix").join("monitors"))
}

/// The outcomes of one monitor, newest last, as JSON lines.
#[derive(Debug, Clone)]
pub struct Log {
    path: PathBuf,
}

/// Lines kept per monitor.
const KEEP: usize = 500;

impl Log {
    /// The log of the monitor `name` of the collection at `root`, in `dir`.
    pub fn new(dir: &Path, root: &Path, name: &str) -> Self {
        // FNV-1a of the collection's folder: the same file on every run.
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in root.to_string_lossy().to_lowercase().bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        let file = format!(
            "{:08x}-{}.jsonl",
            hash as u32,
            apix_core::store::slug(name, "monitor")
        );
        Self {
            path: dir.join(file),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn record(&self, outcome: &Outcome) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{}", serde_json::to_string(outcome)?)?;
        drop(file);
        // Trimmed now and then rather than on every line.
        let lines = self.recent(usize::MAX);
        if lines.len() > KEEP + KEEP / 5 {
            let kept: Vec<String> = lines[lines.len() - KEEP..]
                .iter()
                .filter_map(|o| serde_json::to_string(o).ok())
                .collect();
            std::fs::write(&self.path, kept.join("\n") + "\n")?;
        }
        Ok(())
    }

    /// The last `n` outcomes, oldest first.
    pub fn recent(&self, n: usize) -> Vec<Outcome> {
        let Ok(text) = std::fs::read_to_string(&self.path) else {
            return Vec::new();
        };
        let all: Vec<Outcome> = text
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        let skip = all.len().saturating_sub(n);
        all.into_iter().skip(skip).collect()
    }

    pub fn last(&self) -> Option<Outcome> {
        self.recent(1).pop()
    }
}

/// A monitor's state turning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// It failed, after passing (or on its first run).
    Failing,
    /// It passed again after failing.
    Recovered,
}

/// What changed between the previous outcome and this one, if anything.
pub fn change(previous: Option<&Outcome>, now: &Outcome) -> Option<Change> {
    match (previous.map(Outcome::ok), now.ok()) {
        (Some(false), true) => Some(Change::Recovered),
        (Some(true) | None, false) => Some(Change::Failing),
        _ => None,
    }
}

/// The line a person reads in a chat: `❌ Apix monitor "Health" (Shop API) is failing: …`.
pub fn message(collection: &str, monitor: &Monitor, outcome: &Outcome, change: Change) -> String {
    match change {
        Change::Failing => {
            let mut text = format!(
                "❌ Apix monitor \"{}\" ({collection}) is failing: {}",
                monitor.name,
                outcome.summary()
            );
            for failure in &outcome.failures {
                text += &format!("\n• {failure}");
            }
            text
        }
        Change::Recovered => format!(
            "✅ Apix monitor \"{}\" ({collection}) passes again: {}",
            monitor.name,
            outcome.summary()
        ),
    }
}

/// Posts the change to the monitor's webhook (its `{{variables}}` filled from
/// `vars`): JSON with `text` (Slack, Mattermost, Google Chat) and `content`
/// (Discord) holding [`message`], and the details as fields.
pub async fn notify(
    client: &HttpClient,
    vars: &Variables,
    collection: &str,
    monitor: &Monitor,
    outcome: &Outcome,
    change: Change,
) -> Result<()> {
    let mut missing = Default::default();
    let url = vars.interpolate(monitor.webhook.trim(), &mut missing);
    if !missing.is_empty() {
        let names: Vec<String> = missing.into_iter().collect();
        bail!("the webhook needs {}", names.join(", "));
    }
    let text = message(collection, monitor, outcome, change);
    let body = serde_json::json!({
        "text": text,
        "content": text,
        "monitor": monitor.name,
        "collection": collection,
        "status": match change { Change::Failing => "failing", Change::Recovered => "recovered" },
        "passed": outcome.passed,
        "failed": outcome.failed,
        "failures": outcome.failures,
        "error": outcome.error,
        "at": crate::report::iso8601(outcome.at_ms),
    });
    let mut request = Request::new(Method::Post, url);
    request
        .headers
        .push(KeyValue::new("Content-Type", "application/json"));
    request.body = Body::Json(body.to_string());
    request.settings.timeout_ms = Some(15_000);
    let response = client
        .send(&request)
        .await
        .map_err(|e| anyhow::anyhow!("webhook: {}", apix_core::display_url(&e.to_string())))?;
    if response.status >= 300 {
        bail!("webhook answered {}", response.status);
    }
    Ok(())
}

/// When each monitor is due next, by name.
#[derive(Debug, Default)]
pub struct Scheduler {
    next: HashMap<String, Instant>,
}

impl Scheduler {
    /// The enabled monitors due at `now`. One seen for the first time is due
    /// once its interval has passed since its last logged run (`last`), or
    /// at once.
    pub fn due(
        &mut self,
        monitors: &[Monitor],
        now: Instant,
        last: impl Fn(&Monitor) -> Option<Outcome>,
    ) -> Vec<Monitor> {
        self.next
            .retain(|name, _| monitors.iter().any(|m| m.name == *name && m.enabled));
        let wall = apix_core::history::now_ms();
        let mut out = Vec::new();
        for monitor in monitors.iter().filter(|m| m.enabled) {
            let Ok(every) = monitor.interval() else {
                continue;
            };
            let next = *self.next.entry(monitor.name.clone()).or_insert_with(|| {
                let since = last(monitor)
                    .map(|o| Duration::from_millis(wall.saturating_sub(o.at_ms)))
                    .unwrap_or(every);
                now + every.saturating_sub(since)
            });
            if next <= now {
                out.push(monitor.clone());
            }
        }
        out
    }

    /// `monitor` ran from `started`; it is due again one interval later.
    pub fn ran(&mut self, monitor: &Monitor, started: Instant) {
        let every = monitor.interval().unwrap_or(apix_core::monitor::MIN_EVERY);
        self.next.insert(monitor.name.clone(), started + every);
    }

    /// When `monitor` is due next, once it has been seen.
    pub fn next(&self, monitor: &Monitor) -> Option<Instant> {
        self.next.get(&monitor.name).copied()
    }

    /// The soonest a seen monitor is due.
    pub fn soonest(&self) -> Option<Instant> {
        self.next.values().min().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(failed: usize) -> Outcome {
        Outcome {
            at_ms: 1,
            passed: 3 - failed,
            failed,
            ..Outcome::default()
        }
    }

    #[test]
    fn changes_are_only_turns() {
        assert_eq!(change(None, &outcome(0)), None);
        assert_eq!(change(None, &outcome(1)), Some(Change::Failing));
        assert_eq!(
            change(Some(&outcome(0)), &outcome(1)),
            Some(Change::Failing)
        );
        assert_eq!(change(Some(&outcome(1)), &outcome(2)), None);
        assert_eq!(
            change(Some(&outcome(1)), &outcome(0)),
            Some(Change::Recovered)
        );
        assert_eq!(change(Some(&outcome(0)), &outcome(0)), None);
    }

    #[test]
    fn logs_keep_the_last_outcomes() {
        let dir = std::env::temp_dir().join(format!("apix-monitor-log-{}", std::process::id()));
        let log = Log::new(&dir, Path::new("C:/api"), "Health check");
        assert!(
            log.path()
                .to_string_lossy()
                .ends_with("-health-check.jsonl")
        );
        assert_eq!(log.last(), None);
        for n in 0..(KEEP + KEEP / 5 + 3) {
            let mut o = outcome(n % 2);
            o.at_ms = n as u64;
            log.record(&o).unwrap();
        }
        let recent = log.recent(usize::MAX);
        assert!(recent.len() <= KEEP + KEEP / 5);
        assert_eq!(log.last().unwrap().at_ms, (KEEP + KEEP / 5 + 2) as u64);
        assert_eq!(log.recent(2).len(), 2);
        // Another collection with a monitor of the same name has its own log.
        assert_ne!(
            Log::new(&dir, Path::new("C:/other"), "Health check").path(),
            log.path()
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn schedules_follow_the_log() {
        let mut every_minute = Monitor::new("A", "1m");
        let off = Monitor {
            enabled: false,
            ..Monitor::new("Off", "1m")
        };
        let bad = Monitor::new("Bad", "soon");
        let now = Instant::now();
        let mut s = Scheduler::default();
        let monitors = vec![every_minute.clone(), off, bad];
        // Never ran: due at once.
        assert_eq!(s.due(&monitors, now, |_| None).len(), 1);
        s.ran(&every_minute, now);
        assert!(
            s.due(&monitors, now + Duration::from_secs(30), |_| None)
                .is_empty()
        );
        assert_eq!(
            s.due(&monitors, now + Duration::from_secs(61), |_| None)
                .len(),
            1
        );
        assert_eq!(s.soonest(), Some(now + Duration::from_secs(60)));

        // Ran 20 s ago (by the log): due in 40 s.
        let mut fresh = Scheduler::default();
        let recent = Outcome {
            at_ms: apix_core::history::now_ms() - 20_000,
            ..Outcome::default()
        };
        assert!(
            fresh
                .due(&monitors, now, |_| Some(recent.clone()))
                .is_empty()
        );
        let next = fresh.next(&every_minute).unwrap();
        assert!(next > now + Duration::from_secs(35) && next <= now + Duration::from_secs(40));

        // Renamed or removed: forgotten.
        every_minute.name = "B".into();
        s.due(&[every_minute], now, |_| None);
        assert_eq!(s.next.len(), 1);
    }

    #[test]
    fn messages_and_summaries() {
        let monitor = Monitor::new("Health", "5m");
        let mut o = outcome(1);
        o.failures = vec!["users/get.apix: got 500 Internal Server Error".into()];
        let text = message("Shop", &monitor, &o, Change::Failing);
        assert!(text.starts_with("❌ Apix monitor \"Health\" (Shop) is failing: 1 of 3 failed"));
        assert!(text.contains("\n• users/get.apix: got 500"));
        assert!(
            message("Shop", &monitor, &outcome(0), Change::Recovered)
                .contains("passes again: 3 passed")
        );
        assert_eq!(braced_names("https://x/{{ hook }}/{{id}}"), ["hook", "id"]);
    }
}
