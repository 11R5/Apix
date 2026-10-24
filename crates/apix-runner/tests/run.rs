//! A whole run against a small local server: data rows, iterations,
//! scripts, `setNextRequest` and `bail`.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

use apix_core::Variables;
use apix_http::HttpClient;
use apix_runner::{Control, Plan, Progress, Scope, data};

/// Answers every request with its path as JSON; `/fail` gets a 500.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line.trim().is_empty() {
                    break;
                }
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let path = first.split_whitespace().nth(1).unwrap_or("/").to_owned();
            let status = if path == "/fail" {
                "500 Internal Server Error"
            } else {
                "200 OK"
            };
            let json = format!("{{\"path\":\"{path}\"}}");
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
    let root = std::env::temp_dir().join(format!("apix-runner-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("users")).unwrap();
    fs::write(
        root.join("apix.toml"),
        format!("name = \"Run test\"\n\n[vars]\nbase = \"{base}\"\n"),
    )
    .unwrap();
    fs::write(
        root.join("users/get.apix"),
        "name = \"Get user\"\nurl = \"{{base}}/users/{{id}}\"\n",
    )
    .unwrap();
    fs::write(
        root.join("users/get.test.js"),
        r#"pm.test("path has the row's id", () => {
  pm.expect(pm.response.json().path).to.eql("/users/" + pm.iterationData.get("id"));
});
pm.collectionVariables.set("seen", (Number(pm.collectionVariables.get("seen") || 0) + 1).toString());
"#,
    )
    .unwrap();
    fs::write(
        root.join("z-fail.apix"),
        "name = \"Fail\"\nurl = \"{{base}}/fail\"\n",
    )
    .unwrap();
    root
}

fn plan(root: &Path, scope: &Scope) -> Plan {
    let collection = scope.collection.as_ref().unwrap();
    let vars: Variables = apix_core::project::variables(None, Some(collection), None).unwrap();
    Plan {
        root: root.to_path_buf(),
        requests: collection.request_paths(),
        vars,
        scopes: apix_core::project::scope_values(None, Some(collection), None).unwrap(),
        data: data::parse_csv("id\n7\n8\n").unwrap(),
        ..Default::default()
    }
}

fn sender() -> apix_script::Sender {
    apix_script::Sender::new(|_| Err("not used".into()))
}

#[test]
fn runs_every_row_and_reports() {
    let base = serve();
    let root = collection("rows", &base);
    let scope = Scope::find(&root).unwrap();
    let mut plan = plan(&root, &scope);
    plan.delay = Duration::from_millis(20);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let client = HttpClient::new().unwrap();
    let mut rows_seen = Vec::new();
    let report = rt.block_on(apix_runner::run(
        &scope,
        &plan,
        &client,
        sender(),
        &Control::default(),
        |p| {
            if let Progress::Iteration(n, Some(row)) = p {
                rows_seen.push((n, row["id"].clone()));
            }
        },
    ));
    assert_eq!(rows_seen, [(0, "7".to_string()), (1, "8".to_string())]);
    // Folders first, as in the sidebar: users/get, then z-fail; twice.
    let order: Vec<(u32, &str)> = report
        .items
        .iter()
        .map(|i| (i.iteration, i.path.as_str()))
        .collect();
    assert_eq!(
        order,
        [
            (0, "users/get.apix"),
            (0, "z-fail.apix"),
            (1, "users/get.apix"),
            (1, "z-fail.apix")
        ]
    );
    assert!(report.items[0].passed, "{:?}", report.items[0]);
    assert_eq!(report.items[2].url, format!("{base}/users/8"));
    assert_eq!(report.items[1].status, Some(500));
    assert!(!report.items[1].passed);
    let summary = report.summary();
    assert_eq!((summary.passed, summary.failed, summary.tests), (2, 2, 2));
    assert!(!report.stopped);
    assert!(report.to_junit().contains("got 500 Internal Server Error"));
}

#[test]
fn bail_stops_at_the_first_failure() {
    let base = serve();
    let root = collection("bail", &base);
    let scope = Scope::find(&root).unwrap();
    let mut plan = plan(&root, &scope);
    plan.bail = true;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let client = HttpClient::new().unwrap();
    let report = rt.block_on(apix_runner::run(
        &scope,
        &plan,
        &client,
        sender(),
        &Control::default(),
        |_| {},
    ));
    assert_eq!(report.items.len(), 2);
    assert!(report.stopped);
}

#[test]
fn set_next_request_ends_the_iteration() {
    let base = serve();
    let root = collection("next", &base);
    fs::write(
        root.join("users/get.prerequest.js"),
        "pm.execution.setNextRequest(null);",
    )
    .unwrap();
    let scope = Scope::find(&root).unwrap();
    let mut plan = plan(&root, &scope);
    plan.data.clear();
    plan.iterations = 2;
    plan.pinned = vec![("id".into(), "1".into())];
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let client = HttpClient::new().unwrap();
    let report = rt.block_on(apix_runner::run(
        &scope,
        &plan,
        &client,
        sender(),
        &Control::default(),
        |_| {},
    ));
    let paths: Vec<&str> = report.items.iter().map(|i| i.path.as_str()).collect();
    assert_eq!(paths, ["users/get.apix", "users/get.apix"]);
    assert_eq!(report.items[1].url, format!("{base}/users/1"));
}
