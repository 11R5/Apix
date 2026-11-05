//! An importer: a CSV list of endpoints becomes requests.
//!
//! ```text
//! method,url,name,folder
//! GET,{{base_url}}/users,List users,users
//! POST,{{base_url}}/users,Create user,users
//! ```
//!
//! The header line and `#` comments are optional; name defaults to
//! `METHOD path`, folder to none.

use apix_plugin_sdk::serde_json::{Value, json};
use apix_plugin_sdk::{Plugin, export};

struct CsvEndpoints;

/// Fields of one line; double quotes group, `""` is a quote.
fn fields(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                out.last_mut().unwrap().push('"');
            }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(String::new()),
            c => out.last_mut().unwrap().push(c),
        }
    }
    out.into_iter().map(|f| f.trim().to_owned()).collect()
}

impl Plugin for CsvEndpoints {
    fn manifest() -> Value {
        json!({
            "name": "csv-endpoints",
            "version": "1.0.0",
            "description": "Imports a CSV list of endpoints: method, url, name, folder.",
            "import": { "label": "CSV list of endpoints", "extensions": ["csv"] }
        })
    }

    fn call(hook: &str, input: Value) -> Result<Value, String> {
        if hook != "import" {
            return Err(format!("csv-endpoints has no {hook} hook"));
        }
        let content = input["content"].as_str().unwrap_or_default();
        let mut requests = Vec::new();
        for (n, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let f = fields(line);
            if n == 0 && f[0].eq_ignore_ascii_case("method") {
                continue;
            }
            let method = f[0].to_ascii_uppercase();
            let url = f.get(1).cloned().unwrap_or_default();
            if method.is_empty() || url.is_empty() {
                return Err(format!("line {}: a method and a URL are needed", n + 1));
            }
            let path = url
                .split_once("}}")
                .map_or(url.as_str(), |(_, p)| p)
                .to_owned();
            let name = f
                .get(2)
                .filter(|s| !s.is_empty())
                .cloned()
                .unwrap_or(format!("{method} {path}"));
            requests.push(json!({
                "folder": f.get(3).cloned().unwrap_or_default(),
                "name": name,
                "method": method,
                "url": url,
            }));
        }
        if requests.is_empty() {
            return Err("no endpoints in the file".into());
        }
        let name = input["file_name"]
            .as_str()
            .unwrap_or("Imported")
            .trim_end_matches(".csv")
            .to_owned();
        Ok(json!({ "name": name, "requests": requests }))
    }
}

export!(CsvEndpoints);
