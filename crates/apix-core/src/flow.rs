//! Flows: requests joined into a graph. A flow is a `.flow` file (TOML) in a
//! collection; its nodes send a request, check a value, set variables or
//! wait, and its edges say which node comes next for each outcome (a
//! request's `ok` or `fail`, a check's `yes` or `no`).
//!
//! ```toml
//! name = "Checkout"
//!
//! [[node]]
//! id = "login"
//! kind = "request"
//! request = "auth/login.apix"
//! x = 40.0
//! y = 60.0
//! extract = { token = "$.token" }
//!
//! [[node]]
//! id = "has-items"
//! kind = "check"
//! subject = "$.items.length()"
//! op = "greater"
//! value = "0"
//!
//! [[edge]]
//! from = "login"
//! port = "ok"
//! to = "has-items"
//! ```
//!
//! Running a flow is `apix_runner::flow`; this module is the file, its
//! checks and the values nodes read from a response.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::store::StoreError;
use crate::vfs;

/// The extension of flow files.
pub const FLOW_EXTENSION: &str = "flow";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Flow {
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The first node; without it, the first node nothing leads to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(default, rename = "node", skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<Node>,
    #[serde(default, rename = "edge", skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(flatten)]
    pub kind: NodeKind,
    /// Where the editor draws it.
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeKind {
    /// Sends a request file (relative to the collection), with its scripts
    /// and tests, then keeps values of its response as variables.
    Request {
        request: String,
        /// Variable name → subject (see [`Seen::subject`]).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        extract: BTreeMap<String, String>,
    },
    /// Compares a subject of the last response (or a text with
    /// `{{variables}}`) with a value.
    Check {
        subject: String,
        #[serde(default)]
        op: Op,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        value: String,
    },
    /// Sets variables; values may use `{{variables}}`.
    Set {
        #[serde(default)]
        values: BTreeMap<String, String>,
    },
    /// Waits, for polling loops.
    Delay {
        #[serde(default)]
        ms: u64,
    },
}

impl NodeKind {
    /// The outcomes this kind of node can have, each a place for an edge.
    pub fn ports(&self) -> &'static [&'static str] {
        match self {
            NodeKind::Request { .. } => &["ok", "fail"],
            NodeKind::Check { .. } => &["yes", "no"],
            NodeKind::Set { .. } | NodeKind::Delay { .. } => &["next"],
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            NodeKind::Request { .. } => "request",
            NodeKind::Check { .. } => "check",
            NodeKind::Set { .. } => "set",
            NodeKind::Delay { .. } => "delay",
        }
    }
}

/// How a check compares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    #[default]
    Equals,
    NotEquals,
    Contains,
    Greater,
    Less,
    /// The subject has a value (the JSONPath matched, the header is there).
    Exists,
    Missing,
}

impl Op {
    pub const ALL: [Op; 7] = [
        Op::Equals,
        Op::NotEquals,
        Op::Contains,
        Op::Greater,
        Op::Less,
        Op::Exists,
        Op::Missing,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Op::Equals => "equals",
            Op::NotEquals => "not_equals",
            Op::Contains => "contains",
            Op::Greater => "greater",
            Op::Less => "less",
            Op::Exists => "exists",
            Op::Missing => "missing",
        }
    }

    /// Whether the comparison needs a value.
    pub fn takes_value(self) -> bool {
        !matches!(self, Op::Exists | Op::Missing)
    }

    /// `subject` (None when it has no value) against `value`. Numbers
    /// compare as numbers when both sides are numbers.
    pub fn test(self, subject: Option<&str>, value: &str) -> bool {
        let number = |s: &str| s.trim().parse::<f64>().ok();
        match (self, subject) {
            (Op::Exists, s) => s.is_some(),
            (Op::Missing, s) => s.is_none(),
            (_, None) => self == Op::NotEquals,
            (Op::Equals | Op::NotEquals, Some(s)) => {
                let same = match (number(s), number(value)) {
                    (Some(a), Some(b)) => a == b,
                    _ => s == value,
                };
                same == (self == Op::Equals)
            }
            (Op::Contains, Some(s)) => s.contains(value),
            (Op::Greater | Op::Less, Some(s)) => match (number(s), number(value)) {
                (Some(a), Some(b)) if self == Op::Greater => a > b,
                (Some(a), Some(b)) => a < b,
                _ if self == Op::Greater => s > value,
                _ => s < value,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    #[serde(default = "next_port")]
    pub port: String,
    pub to: String,
}

fn next_port() -> String {
    "next".into()
}

impl Flow {
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Where a run begins.
    pub fn start_node(&self) -> Option<&str> {
        if let Some(start) = &self.start
            && self.node(start).is_some()
        {
            return Some(start);
        }
        let targets: BTreeSet<&str> = self.edges.iter().map(|e| e.to.as_str()).collect();
        self.nodes
            .iter()
            .find(|n| !targets.contains(n.id.as_str()))
            .or(self.nodes.first())
            .map(|n| n.id.as_str())
    }

    /// The node after `from` when it ended with `port`.
    pub fn next(&self, from: &str, port: &str) -> Option<&str> {
        self.edges
            .iter()
            .find(|e| e.from == from && e.port == port)
            .map(|e| e.to.as_str())
    }

    /// Connects `from`'s `port` to `to`, replacing what that port led to.
    pub fn connect(&mut self, from: &str, port: &str, to: &str) {
        self.edges.retain(|e| !(e.from == from && e.port == port));
        if from != to {
            self.edges.push(Edge {
                from: from.into(),
                port: port.into(),
                to: to.into(),
            });
        }
    }

    /// Removes a node and every edge touching it.
    pub fn remove(&mut self, id: &str) {
        self.nodes.retain(|n| n.id != id);
        self.edges.retain(|e| e.from != id && e.to != id);
        if self.start.as_deref() == Some(id) {
            self.start = None;
        }
    }

    /// An id for a new node: `base`, else `base-2`, `base-3`, …
    pub fn new_id(&self, base: &str) -> String {
        let base = crate::store::slug(base, "node");
        (1..)
            .map(|n| {
                if n == 1 {
                    base.clone()
                } else {
                    format!("{base}-{n}")
                }
            })
            .find(|id| self.node(id).is_none())
            .unwrap_or(base)
    }

    /// What is wrong with the flow, in sentences; empty when it can run.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.nodes.is_empty() {
            out.push("the flow has no nodes".into());
        }
        let mut seen = BTreeSet::new();
        for node in &self.nodes {
            if node.id.trim().is_empty() {
                out.push("a node has no id".into());
            } else if !seen.insert(node.id.as_str()) {
                out.push(format!("two nodes are called {}", node.id));
            }
            match &node.kind {
                NodeKind::Request { request, .. } if request.trim().is_empty() => {
                    out.push(format!("{}: no request chosen", node.id));
                }
                NodeKind::Check { subject, .. } if subject.trim().is_empty() => {
                    out.push(format!("{}: nothing to check", node.id));
                }
                _ => {}
            }
        }
        for edge in &self.edges {
            match self.node(&edge.from) {
                None => out.push(format!(
                    "an edge starts at {}, which is not a node",
                    edge.from
                )),
                Some(n) if !n.kind.ports().contains(&edge.port.as_str()) => out.push(format!(
                    "{} has no outcome {} (it has {})",
                    n.id,
                    edge.port,
                    n.kind.ports().join(", ")
                )),
                _ => {}
            }
            if self.node(&edge.to).is_none() {
                out.push(format!("an edge leads to {}, which is not a node", edge.to));
            }
        }
        if let Some(start) = &self.start
            && self.node(start).is_none()
        {
            out.push(format!("the start node {start} does not exist"));
        }
        out
    }
}

/// What a check or an extraction reads: the last response.
#[derive(Debug, Clone, Default)]
pub struct Seen {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub time_ms: f64,
}

impl Seen {
    /// The value of `subject`:
    /// - `status`, `time` (ms), `body`;
    /// - `header:Name` (any case);
    /// - `$…` (JSONPath) or `.…` (jq) on the JSON body: one value as itself
    ///   (a string without quotes), several as a JSON list.
    ///
    /// None when there is nothing there (no response, no such header, the
    /// path matched nothing or `null`).
    pub fn subject(this: Option<&Seen>, subject: &str) -> Result<Option<String>, String> {
        let subject = subject.trim();
        let Some(seen) = this else {
            return Ok(None);
        };
        if let Some(name) = subject.strip_prefix("header:") {
            let name = name.trim();
            return Ok(seen
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.clone()));
        }
        match subject {
            "status" => return Ok(Some(seen.status.to_string())),
            "time" => return Ok(Some(format!("{:.0}", seen.time_ms))),
            "body" => return Ok(Some(seen.body.clone())),
            _ => {}
        }
        let filtered = crate::filter::filter(&seen.body, subject)?;
        let values: Vec<_> = filtered
            .values
            .into_iter()
            .filter(|v| !v.is_null())
            .collect();
        Ok(match values.as_slice() {
            [] => None,
            [serde_json::Value::String(s)] => Some(s.clone()),
            [one] => Some(one.to_string()),
            many => Some(serde_json::Value::Array(many.to_vec()).to_string()),
        })
    }

    /// Whether `subject` is read from the response rather than being text.
    pub fn is_response_subject(subject: &str) -> bool {
        let s = subject.trim();
        matches!(s, "status" | "time" | "body")
            || s.starts_with("header:")
            || s.starts_with('$')
            || s.starts_with('.')
    }
}

/// A flow from its text, as in a `.flow` file.
pub fn parse_flow(text: &str) -> Result<Flow, String> {
    toml::from_str(text).map_err(|e| e.message().to_owned())
}

pub fn load_flow(path: &Path) -> Result<Flow, StoreError> {
    crate::store::read_toml(path)
}

pub fn save_flow(path: &Path, flow: &Flow) -> Result<(), StoreError> {
    crate::store::write_toml(path, flow)
}

/// The `.flow` files under `root` (hidden folders skipped), sorted.
pub fn list_flows(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
        let Ok(entries) = vfs::read_dir(dir) else {
            return;
        };
        for path in entries {
            let hidden = path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'));
            if hidden {
                continue;
            }
            if vfs::is_dir(&path) {
                if depth < 8 {
                    walk(&path, out, depth + 1);
                }
            } else if path.extension().is_some_and(|e| e == FLOW_EXTENSION) {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out, 0);
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
name = "Checkout"

[[node]]
id = "login"
kind = "request"
request = "auth/login.apix"
x = 40.0
y = 60.0
extract = { token = "$.token" }

[[node]]
id = "has-items"
kind = "check"
subject = "$.items.length()"
op = "greater"
value = "0"

[[node]]
id = "wait"
kind = "delay"
ms = 500

[[edge]]
from = "login"
port = "ok"
to = "has-items"

[[edge]]
from = "has-items"
port = "no"
to = "wait"

[[edge]]
from = "wait"
to = "login"
"#;

    #[test]
    fn reads_and_writes_a_flow() {
        let flow: Flow = toml::from_str(SAMPLE).unwrap();
        assert_eq!(flow.nodes.len(), 3);
        assert_eq!(
            flow.nodes[0].kind,
            NodeKind::Request {
                request: "auth/login.apix".into(),
                extract: [("token".to_owned(), "$.token".to_owned())].into(),
            }
        );
        assert_eq!(flow.edges[2].port, "next");
        assert!(flow.problems().is_empty(), "{:?}", flow.problems());
        let again: Flow = toml::from_str(&toml::to_string(&flow).unwrap()).unwrap();
        assert_eq!(again, flow);
    }

    #[test]
    fn start_and_next() {
        let mut flow: Flow = toml::from_str(SAMPLE).unwrap();
        // Everything has an incoming edge: the first node starts.
        assert_eq!(flow.start_node(), Some("login"));
        assert_eq!(flow.next("login", "ok"), Some("has-items"));
        assert_eq!(flow.next("login", "fail"), None);
        flow.start = Some("wait".into());
        assert_eq!(flow.start_node(), Some("wait"));
        flow.connect("login", "ok", "wait");
        assert_eq!(flow.next("login", "ok"), Some("wait"));
        assert_eq!(flow.edges.iter().filter(|e| e.from == "login").count(), 1);
        flow.remove("wait");
        assert!(
            flow.edges
                .iter()
                .all(|e| e.to != "wait" && e.from != "wait")
        );
        assert_eq!(flow.start, None);
        assert_eq!(flow.new_id("login"), "login-2");
        assert_eq!(flow.new_id("Get user"), "get-user");
    }

    #[test]
    fn problems_are_named() {
        let mut flow: Flow = toml::from_str(SAMPLE).unwrap();
        flow.edges.push(Edge {
            from: "has-items".into(),
            port: "ok".into(),
            to: "nowhere".into(),
        });
        flow.nodes.push(flow.nodes[2].clone());
        let problems = flow.problems().join("\n");
        assert!(
            problems.contains("has-items has no outcome ok"),
            "{problems}"
        );
        assert!(problems.contains("leads to nowhere"), "{problems}");
        assert!(problems.contains("two nodes are called wait"), "{problems}");
        assert_eq!(Flow::default().problems(), ["the flow has no nodes"]);
    }

    #[test]
    fn subjects_of_a_response() {
        let seen = Seen {
            status: 201,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: r#"{"token":"abc","n":3,"items":[{"id":1},{"id":2}],"none":null}"#.into(),
            time_ms: 12.4,
        };
        let s = |subject: &str| Seen::subject(Some(&seen), subject).unwrap();
        assert_eq!(s("status").as_deref(), Some("201"));
        assert_eq!(s("time").as_deref(), Some("12"));
        assert_eq!(
            s("header:content-type").as_deref(),
            Some("application/json")
        );
        assert_eq!(s("header:x-missing"), None);
        assert_eq!(s("$.token").as_deref(), Some("abc"));
        assert_eq!(s("$.n").as_deref(), Some("3"));
        assert_eq!(s("$.items[*].id").as_deref(), Some("[1,2]"));
        assert_eq!(s(".items | length").as_deref(), Some("2"));
        assert_eq!(s("$.none"), None);
        assert_eq!(s("$.nothing"), None);
        assert!(Seen::subject(Some(&seen), "$[").is_err());
        assert_eq!(Seen::subject(None, "status").unwrap(), None);
        assert!(Seen::is_response_subject(" $.a"));
        assert!(!Seen::is_response_subject("{{token}}"));
    }

    #[test]
    fn comparisons() {
        assert!(Op::Equals.test(Some("200"), "200.0"));
        assert!(Op::Equals.test(Some("ok"), "ok"));
        assert!(!Op::Equals.test(None, ""));
        assert!(Op::NotEquals.test(None, "x"));
        assert!(Op::NotEquals.test(Some("a"), "b"));
        assert!(Op::Contains.test(Some("hello world"), "lo w"));
        assert!(Op::Greater.test(Some("10"), "9"));
        assert!(!Op::Greater.test(Some("10"), "10"));
        assert!(Op::Less.test(Some("b"), "c"));
        assert!(Op::Exists.test(Some(""), ""));
        assert!(Op::Missing.test(None, ""));
        assert!(!Op::Less.test(None, "1"));
    }

    #[test]
    fn lists_flow_files() {
        let dir = std::env::temp_dir().join(format!("apix-flows-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        std::fs::write(dir.join("b.flow"), SAMPLE).unwrap();
        std::fs::write(dir.join("sub/a.flow"), "").unwrap();
        std::fs::write(dir.join(".git/c.flow"), "").unwrap();
        std::fs::write(dir.join("x.apix"), "").unwrap();
        let found = list_flows(&dir);
        assert_eq!(found, [dir.join("b.flow"), dir.join("sub").join("a.flow")]);
        let flow = load_flow(&dir.join("b.flow")).unwrap();
        save_flow(&dir.join("c.flow"), &flow).unwrap();
        assert_eq!(load_flow(&dir.join("c.flow")).unwrap(), flow);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
