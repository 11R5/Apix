//! `apix mcp`: a Model Context Protocol server over standard input and
//! output, so an AI agent (Claude, an IDE assistant …) can list a
//! collection's requests and environments, send a request and run a folder
//! with its tests. One JSON-RPC 2.0 message per line, as MCP's stdio
//! transport says; logs go to standard error.
//!
//! The server only reaches requests inside the collection it was started
//! for, never prints variable values, and with `--list-only` sends nothing.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use apix_core::Collection;
use apix_http::HttpClient;
use apix_runner::Scope;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

use super::{Resolved, VarArgs, script_sender};

/// The protocol versions this server speaks, newest first.
const VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];
/// Bodies longer than this are cut in tool results.
const MAX_BODY: usize = 64 * 1024;

pub struct Server {
    root: PathBuf,
    scope: Scope,
    collection: Collection,
    list_only: bool,
    client: HttpClient,
}

impl Server {
    pub fn new(dir: &Path, list_only: bool) -> Result<Self> {
        let root = Collection::find_root(dir)
            .with_context(|| format!("no apix.toml found in {} or its parents", dir.display()))?;
        let root = std::path::absolute(&root).unwrap_or(root);
        let scope = Scope::find(&root)?;
        let collection = scope.collection.clone().context("no collection")?;
        Ok(Self {
            root,
            scope,
            collection,
            list_only,
            client: HttpClient::new()?,
        })
    }

    /// Reads messages from standard input until it closes.
    pub async fn serve(self) -> Result<()> {
        let mut lines = BufReader::new(tokio::io::stdin()).lines();
        let mut stdout = tokio::io::stdout();
        eprintln!(
            "apix mcp: serving {} ({})",
            self.collection.meta.name,
            self.root.display()
        );
        while let Some(line) = lines.next_line().await? {
            // Some shells put a byte order mark before the first line.
            let line = line.trim_start_matches('\u{feff}');
            if line.trim().is_empty() {
                continue;
            }
            let reply = match serde_json::from_str::<Value>(line) {
                Ok(message) => self.handle(message).await,
                Err(e) => Some(error(Value::Null, -32700, &format!("not JSON: {e}"))),
            };
            if let Some(reply) = reply {
                stdout.write_all(reply.to_string().as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
        }
        Ok(())
    }

    /// The answer to one message; `None` for notifications.
    pub async fn handle(&self, message: Value) -> Option<Value> {
        let id = message.get("id").cloned();
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        // Notifications (no id) get no answer.
        let id = id?;
        let result = match method {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": self.tools() })),
            "tools/call" => Ok(self.call(&params).await),
            "resources/list" => Ok(json!({ "resources": [] })),
            "prompts/list" => Ok(json!({ "prompts": [] })),
            other => Err((-32601, format!("unknown method: {other}"))),
        };
        Some(match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => error(id, code, &message),
        })
    }

    fn initialize(&self, params: &Value) -> Value {
        let asked = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let version = VERSIONS
            .iter()
            .find(|v| **v == asked)
            .unwrap_or(&VERSIONS[0]);
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "apix", "version": env!("CARGO_PKG_VERSION") },
            "instructions": format!(
                "API requests of the Apix collection \"{}\". Use list_requests to see them, \
                 list_environments for the environments, send_request to send one (its \
                 scripts and tests run) and run_requests to run a folder with its tests. \
                 Paths are relative to the collection.",
                self.collection.meta.name
            ),
        })
    }

    fn tools(&self) -> Vec<Value> {
        let mut tools = vec![
            json!({
                "name": "list_requests",
                "title": "List requests",
                "description": "The requests of the collection (or of one folder): path, method, name and URL as written, with {{variables}} unresolved.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "folder": { "type": "string", "description": "A folder inside the collection, relative to it. Leave out for all." }
                    }
                },
                "annotations": { "readOnlyHint": true }
            }),
            json!({
                "name": "list_environments",
                "title": "List environments",
                "description": "The collection's environments and the names of the variables each sets (never their values).",
                "inputSchema": { "type": "object", "properties": {} },
                "annotations": { "readOnlyHint": true }
            }),
        ];
        if !self.list_only {
            let variables = json!({
                "type": "object",
                "description": "Variables that win over the environment's, e.g. {\"user_id\": \"42\"}.",
                "additionalProperties": { "type": "string" }
            });
            tools.push(json!({
                "name": "send_request",
                "title": "Send a request",
                "description": "Sends one request of the collection: its pre-request scripts, the request with variables filled in, then its tests. Returns the status, time, headers, body (cut at 64 KB) and test results.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "The request file, relative to the collection (from list_requests)." },
                        "environment": { "type": "string", "description": "An environment name (from list_environments)." },
                        "variables": variables
                    },
                    "required": ["path"]
                },
                "annotations": { "readOnlyHint": false, "openWorldHint": true }
            }));
            tools.push(json!({
                "name": "run_requests",
                "title": "Run requests with their tests",
                "description": "Runs every request of the collection or of one folder in order, with scripts and tests, and reports which passed.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "folder": { "type": "string", "description": "A folder inside the collection, relative to it. Leave out for all." },
                        "environment": { "type": "string" },
                        "variables": variables
                    }
                },
                "annotations": { "readOnlyHint": false, "openWorldHint": true }
            }));
        }
        tools
    }

    async fn call(&self, params: &Value) -> Value {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let args = params.get("arguments").cloned().unwrap_or(json!({}));
        let result = match name {
            "list_requests" => self.list_requests(&args),
            "list_environments" => Ok(self.list_environments()),
            "send_request" if !self.list_only => self.send_request(&args).await,
            "run_requests" if !self.list_only => self.run_requests(&args).await,
            other => Err(anyhow::anyhow!("unknown tool: {other}")),
        };
        match result {
            Ok(value) => json!({
                "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }],
                "structuredContent": value,
                "isError": false
            }),
            // Tool failures are results the agent can read, not protocol errors.
            Err(e) => json!({
                "content": [{ "type": "text", "text": format!("{e:#}") }],
                "isError": true
            }),
        }
    }

    /// A path the agent gave, inside the collection, or an error.
    fn inside(&self, relative: &str) -> Result<PathBuf> {
        let joined = self.root.join(relative.trim_start_matches(['/', '\\']));
        let full = std::path::absolute(&joined).unwrap_or(joined);
        let mut clean = PathBuf::new();
        for part in full.components() {
            match part {
                std::path::Component::ParentDir => {
                    clean.pop();
                }
                std::path::Component::CurDir => {}
                other => clean.push(other),
            }
        }
        anyhow::ensure!(
            clean.starts_with(&self.root),
            "{relative} is outside the collection"
        );
        Ok(clean)
    }

    fn relative(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn requests_in(&self, folder: Option<&str>) -> Result<Vec<PathBuf>> {
        let mut requests = self.collection.request_paths();
        if let Some(folder) = folder.filter(|f| !f.trim().is_empty()) {
            let start = self.inside(folder)?;
            requests.retain(|p| p.starts_with(&start));
            anyhow::ensure!(!requests.is_empty(), "no requests in {folder}");
        }
        Ok(requests)
    }

    fn list_requests(&self, args: &Value) -> Result<Value> {
        let folder = args.get("folder").and_then(Value::as_str);
        let list: Vec<Value> = self
            .requests_in(folder)?
            .iter()
            .filter_map(|path| {
                let request = self.scope.load(path).ok()?;
                Some(json!({
                    "path": self.relative(path),
                    "method": request.method.as_str(),
                    "name": request.name,
                    "url": request.url,
                }))
            })
            .collect();
        Ok(json!({ "collection": self.collection.meta.name, "requests": list }))
    }

    fn list_environments(&self) -> Value {
        let list: Vec<Value> = self
            .collection
            .environment_names()
            .into_iter()
            .map(|name| {
                let mut variables: Vec<String> = self
                    .collection
                    .environment(&name)
                    .map(|env| env.vars.keys().cloned().collect())
                    .unwrap_or_default();
                variables.sort();
                json!({ "name": name, "variables": variables })
            })
            .collect();
        json!({ "environments": list })
    }

    fn variables(&self, args: &Value) -> Result<(Resolved, Option<String>)> {
        let environment = args
            .get("environment")
            .and_then(Value::as_str)
            .filter(|e| !e.is_empty())
            .map(str::to_owned);
        let vars: Vec<(String, String)> = args
            .get("variables")
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .map(|(k, v)| {
                        let value = v
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| v.to_string());
                        (k.clone(), value)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let resolved = VarArgs {
            env: environment.clone(),
            vars,
            env_file: None,
        }
        .resolve(&self.scope)?;
        Ok((resolved, environment))
    }

    async fn send_request(&self, args: &Value) -> Result<Value> {
        let relative = args
            .get("path")
            .and_then(Value::as_str)
            .context("`path` is required")?;
        let path = self.inside(relative)?;
        anyhow::ensure!(path.is_file(), "no request at {relative}");
        let (resolved, _) = self.variables(args)?;
        let mut state = Default::default();
        let exchange = apix_runner::exchange(
            &self.client,
            &self.scope,
            &path,
            &resolved.vars,
            &resolved.scopes,
            &mut state,
            &Default::default(),
            Default::default(),
            script_sender(),
            |_| {},
        )
        .await?;
        let item = exchange.to_item(0, self.relative(&path));
        let mut out = json!({
            "path": item.path,
            "method": item.method,
            "url": item.url,
            "passed": item.passed,
            "tests": item.tests.iter().map(|t| json!({
                "name": t.name, "passed": t.passed, "error": t.error
            })).collect::<Vec<_>>(),
        });
        if let Some(e) = &item.pre_error {
            out["pre_request_error"] = json!(e);
        }
        if let Some(e) = &item.test_error {
            out["test_error"] = json!(e);
        }
        if let Some(response) = &exchange.response {
            let body = String::from_utf8_lossy(&response.body);
            let cut = body.len() > MAX_BODY;
            let mut end = body.len().min(MAX_BODY);
            while !body.is_char_boundary(end) {
                end -= 1;
            }
            out["status"] = json!(response.status);
            out["time_ms"] = json!((response.timing.total.as_secs_f64() * 1000.0).round());
            out["headers"] = json!(
                response
                    .headers
                    .iter()
                    .map(|(k, v)| json!({ "name": k, "value": v }))
                    .collect::<Vec<_>>()
            );
            out["body"] = json!(&body[..end]);
            if cut {
                out["body_truncated_at"] = json!(MAX_BODY);
                out["body_size"] = json!(response.body.len());
            }
        }
        Ok(out)
    }

    async fn run_requests(&self, args: &Value) -> Result<Value> {
        let folder = args.get("folder").and_then(Value::as_str);
        let requests = self.requests_in(folder)?;
        let (resolved, environment) = self.variables(args)?;
        let plan = apix_runner::Plan {
            root: self.root.clone(),
            requests,
            vars: resolved.vars,
            scopes: resolved.scopes,
            environment,
            pinned: resolved.overrides,
            ..Default::default()
        };
        let report = apix_runner::run(
            &self.scope,
            &plan,
            &self.client,
            script_sender(),
            &apix_runner::Control::default(),
            |_| {},
        )
        .await;
        let items: Vec<Value> = report
            .items
            .iter()
            .map(|item| {
                let failed: Vec<Value> = item
                    .tests
                    .iter()
                    .filter(|t| !t.passed)
                    .map(|t| json!({ "name": t.name, "error": t.error }))
                    .collect();
                json!({
                    "path": item.path,
                    "method": item.method,
                    "status": item.status,
                    "time_ms": item.time_ms.round(),
                    "passed": item.passed,
                    "failed_tests": failed,
                    "error": item.error.clone().or(item.pre_error.clone()).or(item.test_error.clone()),
                })
            })
            .collect();
        let passed = report.items.iter().filter(|i| i.passed).count();
        Ok(json!({
            "collection": report.name,
            "environment": report.environment,
            "passed": passed,
            "failed": report.items.len() - passed,
            "requests": items,
        }))
    }
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead as _, Read as _, Write as _};

    /// A collection with two requests against a local server that answers
    /// `{"ok":true}` with the path in a header.
    fn collection(tag: &str) -> PathBuf {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                let _ = reader.read_line(&mut first);
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0; length];
                let _ = reader.read_exact(&mut body);
                let path = first.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let mut stream = stream;
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-Path: {path}\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{{\"ok\":true}}"
                );
            }
        });
        let root = std::env::temp_dir().join(format!("apix-mcp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("users")).unwrap();
        std::fs::create_dir_all(root.join("environments")).unwrap();
        std::fs::write(
            root.join("apix.toml"),
            format!("name = \"Shop\"\n\n[vars]\nbase_url = \"http://{addr}\"\n"),
        )
        .unwrap();
        std::fs::write(
            root.join("environments/staging.toml"),
            "[vars]\ntoken = \"s3cret\"\nuser_id = \"7\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("users/get-user.apix"),
            "name = \"Get user\"\nmethod = \"GET\"\nurl = \"{{base_url}}/users/{{user_id}}\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("users/get-user.test.js"),
            "pm.test(\"ok\", () => pm.response.to.have.status(200));\n",
        )
        .unwrap();
        std::fs::write(
            root.join("health.apix"),
            "name = \"Health\"\nmethod = \"GET\"\nurl = \"{{base_url}}/health\"\n",
        )
        .unwrap();
        root
    }

    async fn ask(server: &Server, method: &str, params: Value) -> Value {
        server
            .handle(json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
            .await
            .unwrap()
    }

    async fn tool(server: &Server, name: &str, arguments: Value) -> Value {
        ask(
            server,
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )
        .await["result"]
            .clone()
    }

    #[tokio::test]
    async fn the_protocol() {
        let root = collection("protocol");
        let server = Server::new(&root, false).unwrap();
        let init = ask(
            &server,
            "initialize",
            json!({ "protocolVersion": "2024-11-05", "capabilities": {} }),
        )
        .await;
        assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(init["result"]["serverInfo"]["name"], "apix");
        let newer = ask(
            &server,
            "initialize",
            json!({ "protocolVersion": "2099-01-01" }),
        )
        .await;
        assert_eq!(newer["result"]["protocolVersion"], VERSIONS[0]);
        // Notifications get no answer.
        assert!(
            server
                .handle(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
                .await
                .is_none()
        );
        assert_eq!(ask(&server, "ping", json!({})).await["result"], json!({}));
        let unknown = ask(&server, "sampling/createMessage", json!({})).await;
        assert_eq!(unknown["error"]["code"], -32601);
        let tools = ask(&server, "tools/list", json!({})).await;
        let names: Vec<&str> = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "list_requests",
                "list_environments",
                "send_request",
                "run_requests"
            ]
        );
        let listing = Server::new(&root, true).unwrap();
        let tools = ask(&listing, "tools/list", json!({})).await;
        assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 2);
        let refused = tool(&listing, "send_request", json!({ "path": "health.apix" })).await;
        assert_eq!(refused["isError"], true);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn listing_sending_and_running() {
        let root = collection("tools");
        let server = Server::new(&root, false).unwrap();
        let list = tool(&server, "list_requests", json!({})).await;
        let requests = list["structuredContent"]["requests"].as_array().unwrap();
        let paths: Vec<&str> = requests
            .iter()
            .map(|r| r["path"].as_str().unwrap())
            .collect();
        assert!(
            paths.contains(&"users/get-user.apix") && paths.contains(&"health.apix"),
            "{paths:?}"
        );
        let folder = tool(&server, "list_requests", json!({ "folder": "users" })).await;
        assert_eq!(
            folder["structuredContent"]["requests"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        // Names only, never values.
        let envs = tool(&server, "list_environments", json!({})).await;
        assert_eq!(
            envs["structuredContent"]["environments"][0]["name"],
            "staging"
        );
        assert_eq!(
            envs["structuredContent"]["environments"][0]["variables"],
            json!(["token", "user_id"])
        );
        assert!(
            !envs["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("s3cret")
        );

        let sent = tool(
            &server,
            "send_request",
            json!({ "path": "users/get-user.apix", "environment": "staging" }),
        )
        .await;
        let out = &sent["structuredContent"];
        assert_eq!(sent["isError"], false, "{sent}");
        assert_eq!(out["status"], 200);
        assert_eq!(out["body"], "{\"ok\":true}");
        assert_eq!(out["passed"], true);
        assert_eq!(out["tests"][0]["name"], "ok");
        assert!(
            out["headers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|h| h["value"] == "/users/7"),
            "{out}"
        );
        // Variables from the agent win.
        let other = tool(&server, "send_request", json!({ "path": "users/get-user.apix", "environment": "staging", "variables": { "user_id": 9 } })).await;
        assert!(
            other["structuredContent"]["headers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|h| h["value"] == "/users/9")
        );

        let run = tool(&server, "run_requests", json!({ "environment": "staging" })).await;
        assert_eq!(run["structuredContent"]["passed"], 2, "{run}");
        assert_eq!(run["structuredContent"]["failed"], 0);

        // Nothing outside the collection.
        let outside = tool(
            &server,
            "send_request",
            json!({ "path": "../../etc/passwd" }),
        )
        .await;
        assert_eq!(outside["isError"], true);
        assert!(
            outside["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("outside the collection")
        );
        let missing = tool(
            &server,
            "send_request",
            json!({ "path": "users/nope.apix" }),
        )
        .await;
        assert_eq!(missing["isError"], true);
        let _ = std::fs::remove_dir_all(&root);
    }
}
