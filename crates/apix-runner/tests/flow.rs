//! Flows against a small local server: values extracted and used by later
//! nodes, a polling loop with a check and a delay, a failing request, an
//! endless loop stopped, and a flow that cannot run.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use apix_core::flow::Flow;
use apix_http::HttpClient;
use apix_runner::flow::{FlowPlan, FlowProgress, FlowReport};
use apix_runner::{Control, Scope};

/// `/login` gives a token, `/count/<token>` counts its calls, `/fail` is a 500.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let calls = Arc::new(AtomicU32::new(0));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line.trim().is_empty() {
                    break;
                }
            }
            let path = first.split_whitespace().nth(1).unwrap_or("/").to_owned();
            let (status, json) = if path == "/login" {
                ("200 OK", r#"{"token":"t-1"}"#.to_owned())
            } else if let Some(token) = path.strip_prefix("/count/") {
                let n = calls.fetch_add(1, Ordering::SeqCst) + 1;
                ("200 OK", format!(r#"{{"n":{n},"token":"{token}"}}"#))
            } else {
                ("500 Internal Server Error", "{}".to_owned())
            };
            let mut stream = stream;
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
                json.len()
            )
            .unwrap();
        }
    });
    format!("http://{addr}")
}

fn collection(tag: &str, base: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("apix-flow-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("auth")).unwrap();
    fs::write(
        root.join("apix.toml"),
        format!("name = \"Flow test\"\n\n[vars]\nbase = \"{base}\"\n"),
    )
    .unwrap();
    fs::write(
        root.join("auth/login.apix"),
        "name = \"Login\"\nmethod = \"POST\"\nurl = \"{{base}}/login\"\n",
    )
    .unwrap();
    fs::write(
        root.join("count.apix"),
        "name = \"Count\"\nurl = \"{{base}}/count/{{token}}\"\n",
    )
    .unwrap();
    fs::write(
        root.join("fail.apix"),
        "name = \"Fail\"\nurl = \"{{base}}/fail\"\n",
    )
    .unwrap();
    root
}

fn run(root: &Path, flow: &str) -> (FlowReport, Vec<String>) {
    let scope = Scope::find(root).unwrap();
    let collection = scope.collection.as_ref().unwrap();
    let flow: Flow = apix_core::flow::parse_flow(flow).unwrap();
    let plan = FlowPlan {
        flow,
        base: FlowPlan::base_for(&scope, &root.join("x.flow")),
        vars: apix_core::project::variables(None, Some(collection), None).unwrap(),
        scopes: apix_core::project::scope_values(None, Some(collection), None).unwrap(),
        ..Default::default()
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let client = HttpClient::new().unwrap();
    let mut started = Vec::new();
    let report = rt.block_on(apix_runner::flow::run(
        &scope,
        &plan,
        &client,
        apix_script::Sender::new(|_| Err("not used".into())),
        &Control::default(),
        |p| {
            if let FlowProgress::Node(id) = p {
                started.push(id.to_owned());
            }
        },
    ));
    (report, started)
}

const POLL: &str = r#"
name = "Poll"

[[node]]
id = "login"
kind = "request"
request = "auth/login.apix"
extract = { token = "$.token" }

[[node]]
id = "count"
kind = "request"
request = "count.apix"
extract = { n = "$.n" }

[[node]]
id = "enough"
kind = "check"
subject = "$.n"
op = "greater"
value = "{{limit}}"

[[node]]
id = "limit"
kind = "set"
values = { limit = "2" }

[[node]]
id = "wait"
kind = "delay"
ms = 10

[[node]]
id = "done"
kind = "set"
values = { result = "{{token}}-{{n}}" }

[[edge]]
from = "limit"
to = "login"

[[edge]]
from = "login"
port = "ok"
to = "count"

[[edge]]
from = "count"
port = "ok"
to = "enough"

[[edge]]
from = "enough"
port = "no"
to = "wait"

[[edge]]
from = "wait"
to = "count"

[[edge]]
from = "enough"
port = "yes"
to = "done"
"#;

#[test]
fn values_flow_from_node_to_node_and_loops_end() {
    let root = collection("poll", &serve());
    let (report, started) = run(&root, POLL);
    // `limit` starts: nothing leads to it.
    assert_eq!(
        started,
        [
            "limit", "login", "count", "enough", "wait", "count", "enough", "wait", "count",
            "enough", "done"
        ]
    );
    assert!(report.passed, "{report:#?}");
    let login = &report.steps[1];
    assert_eq!(login.port.as_deref(), Some("ok"));
    assert_eq!(login.note, "token = t-1");
    // The token went into the next request's URL.
    let count = report.steps[2].item.as_ref().unwrap();
    assert!(count.url.ends_with("/count/t-1"), "{}", count.url);
    assert_eq!(report.steps[3].note, "$.n is 1; greater 2");
    assert_eq!(report.steps[3].port.as_deref(), Some("no"));
    assert_eq!(report.steps.last().unwrap().note, "result = t-1-3");
    let run = report.to_run_report(None);
    assert_eq!(run.items.len(), 4);
    assert!(run.items.iter().all(|i| i.passed));
}

#[test]
fn a_failing_request_without_a_fail_edge_fails_the_flow() {
    let root = collection("fail", &serve());
    let flow = r#"
[[node]]
id = "fail"
kind = "request"
request = "fail.apix"

[[node]]
id = "after"
kind = "set"
values = { x = "1" }

[[edge]]
from = "fail"
port = "ok"
to = "after"
"#;
    let (report, started) = run(&root, flow);
    assert_eq!(started, ["fail"]);
    assert!(!report.passed);
    assert!(report.steps[0].failed());
    assert_eq!(report.steps[0].item.as_ref().unwrap().status, Some(500));

    // With a fail edge, the failure is handled and the flow goes on.
    let handled = format!("{flow}\n[[edge]]\nfrom = \"fail\"\nport = \"fail\"\nto = \"after\"\n");
    let (report, started) = run(&root, &handled);
    assert_eq!(started, ["fail", "after"]);
    assert!(report.passed, "{report:#?}");

    // A value that is not there fails the request node.
    let missing = r#"
[[node]]
id = "login"
kind = "request"
request = "auth/login.apix"
extract = { id = "$.id" }
"#;
    let (report, _) = run(&root, missing);
    assert!(!report.passed);
    assert_eq!(report.steps[0].note, "id: nothing at $.id");

    // An unknown request file is a failed step, not a panic.
    let (report, _) = run(
        &root,
        "[[node]]\nid = \"x\"\nkind = \"request\"\nrequest = \"nope.apix\"\n",
    );
    assert!(!report.passed);
    assert!(report.steps[0].item.as_ref().unwrap().error.is_some());
}

#[test]
fn endless_loops_stop_and_broken_flows_do_not_start() {
    let root = collection("loop", "http://127.0.0.1:9");
    let endless = r#"
[[node]]
id = "again"
kind = "check"
subject = "{{nothing}}"
op = "exists"

[[node]]
id = "wait"
kind = "delay"
ms = 0

[[edge]]
from = "again"
port = "no"
to = "wait"

[[edge]]
from = "wait"
to = "again"
"#;
    let (report, _) = run(&root, endless);
    assert_eq!(report.steps.len(), apix_runner::flow::MOST_STEPS);
    assert!(!report.passed);
    assert!(report.ended.as_deref().unwrap().contains("stopped after"));

    let (report, started) = run(&root, "[[edge]]\nfrom = \"a\"\nto = \"b\"\n");
    assert!(started.is_empty());
    assert!(!report.passed);
    assert!(report.ended.as_deref().unwrap().contains("no nodes"));
}
