//! `apix monitor`: a collection's `[[monitor]]`s on their schedules, once
//! (for cron, the Windows Task Scheduler or a CI schedule), or listed.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use apix_core::Collection;
use apix_core::monitor::{Monitor, describe_every};
use apix_http::HttpClient;
use apix_runner::Scope;
use apix_runner::monitor::{self, Change, Log, Outcome, Scheduler};

use super::{Style, script_sender};

pub struct Options {
    pub dir: PathBuf,
    pub once: bool,
    pub list: bool,
    pub only: Vec<String>,
    pub pinned: Vec<(String, String)>,
    pub overrides: apix_core::RequestSettings,
    pub log: bool,
    /// Instead of `<config>/Apix/monitors`.
    pub log_dir: Option<PathBuf>,
}

/// The collection's monitors that `only` names (all when empty).
fn chosen(collection: &Collection, only: &[String]) -> Result<Vec<Monitor>> {
    let all = &collection.meta.monitors;
    if all.is_empty() {
        bail!(
            "{} has no monitors; add one to its apix.toml:\n\n[[monitor]]\nname = \"Health\"\nevery = \"5m\"",
            collection
                .root
                .join(apix_core::store::COLLECTION_FILE)
                .display()
        );
    }
    for name in only {
        if !all.iter().any(|m| m.name == *name) {
            let names: Vec<&str> = all.iter().map(|m| m.name.as_str()).collect();
            bail!(
                "no monitor named `{name}` (there are: {})",
                names.join(", ")
            );
        }
    }
    Ok(all
        .iter()
        .filter(|m| only.is_empty() || only.contains(&m.name))
        .cloned()
        .collect())
}

/// `09:30:05Z`, the time of day in UTC.
fn clock(ms: u64) -> String {
    apix_runner::report::iso8601(ms)[11..].to_owned()
}

pub async fn run(
    options: Options,
    client: &HttpClient,
    style: &Style,
    password_env: &str,
) -> Result<bool> {
    super::unlock_around(&options.dir, password_env)?;
    let root = Collection::find_root(&options.dir).with_context(|| {
        format!(
            "no apix.toml found in {} or its parents",
            options.dir.display()
        )
    })?;
    let dir = match &options.log_dir {
        // Relative to where the command runs, like any path given to it.
        Some(dir) => std::path::absolute(dir).unwrap_or_else(|_| dir.clone()),
        None => monitor::default_dir().context("no configuration folder for monitor logs")?,
    };
    let log_of = |m: &Monitor| Log::new(&dir, &root, &m.name);

    if options.list {
        let scope = Scope::find(&root)?;
        let collection = scope.collection.as_ref().context("no collection")?;
        for m in chosen(collection, &options.only)? {
            let every = match m.interval() {
                Ok(d) => describe_every(d),
                Err(e) => format!("{}{e}{}", style.red, style.reset),
            };
            let mut facts = vec![format!("every {every}")];
            if !m.run.is_empty() {
                facts.push(m.run.clone());
            }
            if let Some(env) = &m.environment {
                facts.push(format!("env {env}"));
            }
            if !m.webhook.is_empty() {
                facts.push("webhook".into());
            }
            if !m.enabled {
                facts.push("off".into());
            }
            println!(
                "{:<24} {}{}{}",
                m.name,
                style.dim,
                facts.join(" · "),
                style.reset
            );
            match log_of(&m).last() {
                Some(o) => println!(
                    "{:<24} {}last {} · {}{}",
                    "",
                    if o.ok() { style.green } else { style.red },
                    apix_runner::report::iso8601(o.at_ms),
                    o.summary(),
                    style.reset
                ),
                None => println!("{:<24} {}not run yet{}", "", style.dim, style.reset),
            }
        }
        return Ok(true);
    }

    let mut scheduler = Scheduler::default();
    let mut all_ok = true;
    let mut announced = false;
    loop {
        // Read again every time: monitors edited while this runs take effect.
        let scope = Scope::find(&root)?;
        let collection = scope.collection.clone().context("no collection")?;
        let monitors = chosen(&collection, &options.only)?;
        for m in &monitors {
            if let Err(e) = m.interval() {
                eprintln!("{}monitor `{}`: {e}{}", style.yellow, m.name, style.reset);
            }
        }
        let due: Vec<Monitor> = if options.once {
            // Named ones run even when switched off.
            monitors
                .iter()
                .filter(|m| m.enabled || options.only.contains(&m.name))
                .cloned()
                .collect()
        } else {
            if !announced {
                let on = monitors.iter().filter(|m| m.enabled).count();
                println!(
                    "{}Watching {on} monitor(s) of {}; logs in {} (Ctrl+C stops){}",
                    style.dim,
                    collection.meta.name,
                    dir.display(),
                    style.reset
                );
                announced = true;
            }
            scheduler.due(&monitors, Instant::now(), |m| log_of(m).last())
        };
        for m in &due {
            let started = Instant::now();
            let ok = run_one(&scope, &collection, m, client, &options, &log_of(m), style).await;
            all_ok &= ok;
            scheduler.ran(m, started);
            if !options.once
                && let Some(next) = scheduler.next(m)
            {
                let at = apix_core::history::now_ms()
                    + next.saturating_duration_since(Instant::now()).as_millis() as u64;
                println!(
                    "{}{:<24} next at {}{}",
                    style.dim,
                    "",
                    clock(at),
                    style.reset
                );
            }
        }
        if options.once {
            return Ok(all_ok);
        }
        // Wake for the soonest, and at least every minute to see edits.
        let wait = scheduler
            .soonest()
            .map(|t| t.saturating_duration_since(Instant::now()))
            .unwrap_or(Duration::from_secs(60))
            .clamp(Duration::from_millis(200), Duration::from_secs(60));
        tokio::time::sleep(wait).await;
    }
}

/// One run: printed, logged, and told to the webhook when it turned.
async fn run_one(
    scope: &Scope,
    collection: &Collection,
    m: &Monitor,
    client: &HttpClient,
    options: &Options,
    log: &Log,
    style: &Style,
) -> bool {
    let mut plan_scope = scope.clone();
    plan_scope.collection = Some(collection.clone());
    let (report, outcome) = run_monitor(&plan_scope, m, client, options).await;
    let (mark, colour) = if outcome.ok() {
        ("✓", style.green)
    } else {
        ("✗", style.red)
    };
    let took = report
        .as_ref()
        .map(|r| format!(" · {} ms", r.duration_ms))
        .unwrap_or_default();
    println!(
        "{}{}{} {colour}{mark} {:<22}{} {}{took}",
        style.dim,
        clock(outcome.at_ms),
        style.reset,
        m.name,
        style.reset,
        outcome.summary(),
    );
    for failure in &outcome.failures {
        println!("{}    {failure}{}", style.dim, style.reset);
    }
    let previous = log.last();
    if options.log
        && let Err(e) = log.record(&outcome)
    {
        eprintln!(
            "{}cannot write {}: {e}{}",
            style.yellow,
            log.path().display(),
            style.reset
        );
    }
    if let Some(change) = monitor::change(previous.as_ref(), &outcome)
        && !m.webhook.trim().is_empty()
    {
        let vars = monitor::plan(scope, m, &options.pinned)
            .map(|p| p.vars)
            .unwrap_or_default();
        let told = monitor::notify(client, &vars, &collection.meta.name, m, &outcome, change).await;
        let what = match change {
            Change::Failing => "failing",
            Change::Recovered => "recovered",
        };
        match told {
            Ok(()) => println!("{}    webhook told: {what}{}", style.dim, style.reset),
            Err(e) => eprintln!("{}    {e:#}{}", style.yellow, style.reset),
        }
    }
    outcome.ok()
}

async fn run_monitor(
    scope: &Scope,
    m: &Monitor,
    client: &HttpClient,
    options: &Options,
) -> (Option<apix_runner::RunReport>, Outcome) {
    monitor::run_once(
        scope,
        m,
        client,
        script_sender(),
        &options.pinned,
        &options.overrides,
    )
    .await
}
