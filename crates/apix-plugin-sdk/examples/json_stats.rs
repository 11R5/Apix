//! A visualizer: for a JSON list of records (or the first list in the
//! body), one row per field with its types, how many records have it, how
//! many distinct values, and the smallest and largest number or an example.

use std::collections::{BTreeMap, BTreeSet};

use apix_plugin_sdk::serde_json::{Value, json};
use apix_plugin_sdk::{Plugin, export};

struct JsonStats;

#[derive(Default)]
struct Stat {
    types: BTreeSet<&'static str>,
    present: usize,
    values: BTreeSet<String>,
    min: Option<f64>,
    max: Option<f64>,
    example: Option<String>,
}

fn kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// The records: the body if it is a list, else its first list field.
fn records(body: &Value) -> Option<&Vec<Value>> {
    match body {
        Value::Array(items) => Some(items),
        Value::Object(map) => map.values().find_map(|v| v.as_array()),
        _ => None,
    }
}

impl Plugin for JsonStats {
    fn manifest() -> Value {
        json!({
            "name": "json-stats",
            "version": "1.0.0",
            "description": "Field statistics of a JSON list: types, presence, distinct values, ranges.",
            "visualize": { "label": "Field stats" }
        })
    }

    fn call(hook: &str, input: Value) -> Result<Value, String> {
        if hook != "visualize" {
            return Err(format!("json-stats has no {hook} hook"));
        }
        let body: Value =
            apix_plugin_sdk::serde_json::from_str(input["body"].as_str().unwrap_or_default())
                .map_err(|_| "the body is not JSON".to_owned())?;
        let items = records(&body).ok_or("there is no list of records in the body")?;
        let mut stats: BTreeMap<String, Stat> = BTreeMap::new();
        let mut order: Vec<String> = Vec::new();
        for item in items {
            let Some(map) = item.as_object() else {
                continue;
            };
            for (key, value) in map {
                if !stats.contains_key(key) {
                    order.push(key.clone());
                }
                let s = stats.entry(key.clone()).or_default();
                s.types.insert(kind(value));
                if value.is_null() {
                    continue;
                }
                s.present += 1;
                let shown = match value {
                    Value::String(t) => t.clone(),
                    other => other.to_string(),
                };
                if s.values.len() < 10_000 {
                    s.values.insert(shown.clone());
                }
                if let Some(n) = value.as_f64() {
                    s.min = Some(s.min.map_or(n, |m| m.min(n)));
                    s.max = Some(s.max.map_or(n, |m| m.max(n)));
                }
                if s.example.is_none() {
                    let short: String = shown.chars().take(40).collect();
                    s.example = Some(short);
                }
            }
        }
        let rows: Vec<Vec<String>> = order
            .iter()
            .map(|key| {
                let s = &stats[key];
                let range = match (s.min, s.max) {
                    (Some(a), Some(b)) => format!("{a} … {b}"),
                    _ => s.example.clone().unwrap_or_default(),
                };
                vec![
                    key.clone(),
                    s.types.iter().copied().collect::<Vec<_>>().join(" | "),
                    format!("{}/{}", s.present, items.len()),
                    s.values.len().to_string(),
                    range,
                ]
            })
            .collect();
        Ok(json!({
            "title": format!("{} records, {} fields", items.len(), rows.len()),
            "columns": ["field", "type", "present", "distinct", "range or example"],
            "rows": rows,
        }))
    }
}

export!(JsonStats);
