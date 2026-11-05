//! `apix flow run` and `apix flow list`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use apix_core::flow::{self, NodeKind};
use apix_http::HttpClient;
use apix_runner::Scope;
use apix_runner::flow::{FlowPlan, FlowProgress};

use super::{NetArgs, Style, VarArgs, note_line, print_scripts, print_status, unlock_around};

/// Where to write reports of the flow's requests.
pub struct Reports {
    pub junit: Option<PathBuf>,
    pub json: Option<PathBuf>,
    pub html: Option<PathBuf>,
    pub markdown: Option<PathBuf>,
}

/// Runs a flow file; true when it passed.
pub async fn run(
    client: &HttpClient,
    style: &Style,
    password_var: &str,
    file: &Path,
    vars: &VarArgs,
    net: &NetArgs,
    reports: Reports,
) -> Result<bool> {
    unlock_around(file, password_var)?;
    let flow = flow::load_flow(file)?;
    let problems = flow.problems();
    if !problems.is_empty() {
        bail!("{} cannot run: {}", file.display(), problems.join("; "));
    }
    let scope = Scope::find(file)?;
    let resolved = vars.resolve(&scope)?;
    let plan = FlowPlan {
        base: FlowPlan::base_for(&scope, file),
        flow,
        vars: resolved.vars,
        scopes: resolved.scopes,
        environment: vars.env.clone(),
        overrides: net.overrides(),
        state: Default::default(),
    };
    let name = if plan.flow.name.is_empty() {
        file.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else {
        plan.flow.name.clone()
    };
    println!("{}Flow {name}{}", style.dim, style.reset);
    let control = apix_runner::Control::default();
    let mut report = apix_runner::flow::run(
        &scope,
        &plan,
        client,
        super::script_sender(),
        &control,
        |progress| {
            let FlowProgress::Step(step) = progress else {
                return;
            };
            let port = step.port.as_deref().unwrap_or("stopped");
            let color = match port {
                "fail" => style.red,
                "no" => style.yellow,
                _ => style.green,
            };
            print!("{:<20} {color}{port:<5}{} ", step.node, style.reset);
            match &step.item {
                Some(item) => {
                    print!("{:<32} ", item.path);
                    match item.status {
                        Some(status) => print_status(
                            style,
                            status,
                            &item.reason,
                            item.time_ms,
                            item.size as usize,
                        ),
                        None => println!(
                            "{}{}{}",
                            style.red,
                            item.error.as_deref().unwrap_or_default(),
                            style.reset
                        ),
                    }
                    if !step.note.is_empty() && item.status.is_some() {
                        println!("{:27}{}{}{}", "", style.dim, step.note, style.reset);
                    }
                    print_scripts(style, item);
                }
                None => println!("{}{}{}", style.dim, step.note, style.reset),
            }
        },
    )
    .await;
    if report.name.is_empty() {
        report.name = name;
    }
    if let Some(why) = &report.ended {
        note_line(style, why);
    }
    let requests = report.steps.iter().filter(|s| s.item.is_some()).count();
    println!(
        "\n{}{}{} after {} steps ({requests} requests, {} ms)",
        if report.passed {
            style.green
        } else {
            style.red
        },
        if report.passed { "Passed" } else { "Failed" },
        style.reset,
        report.steps.len(),
        report.duration_ms
    );
    let run_report = report.to_run_report(vars.env.clone());
    for (path, text) in [
        (reports.junit, run_report.to_junit()),
        (
            reports.json,
            serde_json::to_string_pretty(&report).unwrap_or_default(),
        ),
        (reports.html, run_report.to_html()),
        (reports.markdown, run_report.to_markdown()),
    ]
    .into_iter()
    .filter_map(|(path, text)| Some((path?, text)))
    {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, text).with_context(|| path.display().to_string())?;
    }
    Ok(report.passed)
}

/// Lists the flows under `dir` with their nodes.
pub fn list(dir: &Path) -> Result<()> {
    let flows = flow::list_flows(dir);
    if flows.is_empty() {
        println!("No .flow files in {}", dir.display());
        return Ok(());
    }
    for path in flows {
        let shown = path.strip_prefix(dir).unwrap_or(&path).display();
        match flow::load_flow(&path) {
            Ok(f) => {
                let requests = f
                    .nodes
                    .iter()
                    .filter(|n| matches!(n.kind, NodeKind::Request { .. }))
                    .count();
                let name = if f.name.is_empty() {
                    String::new()
                } else {
                    format!("  {}", f.name)
                };
                println!(
                    "{shown}{name}  ({} nodes, {requests} requests)",
                    f.nodes.len()
                );
                for problem in f.problems() {
                    println!("  ! {problem}");
                }
            }
            Err(e) => println!("{shown}  ! {e}"),
        }
    }
    Ok(())
}
