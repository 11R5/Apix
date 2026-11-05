//! Two responses side by side: the status, the headers by name, the body
//! (JSON field by field, text line by line), the time and the size. Used to
//! compare a response with an earlier one from the history ("time travel")
//! and the same request in two environments.

use serde_json::Value;

use crate::diff::{self, FieldChange, Line};

/// One response to compare.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// What it is: "now", "staging", "3 h ago".
    pub label: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub total_ms: f64,
}

/// A header that differs (`None`: not sent on that side).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderChange {
    pub name: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BodyDiff {
    /// The same bytes, or the same JSON written differently.
    Same,
    /// Both are JSON: the fields that differ.
    Json(Vec<FieldChange>),
    /// Text: every line, the same ones too (for context).
    Text(Vec<Line>),
    /// Not text: only the sizes.
    Binary,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    pub old_label: String,
    pub new_label: String,
    pub status: (u16, u16),
    pub headers: Vec<HeaderChange>,
    /// Headers left out because they change on every response.
    pub ignored: Vec<String>,
    pub body: BodyDiff,
    pub time_ms: (f64, f64),
    pub size: (usize, usize),
}

/// Headers that differ between any two responses (dates, request ids, how
/// the body was sent,
/// cache and tracing details), so they are not reported as changes.
pub const VOLATILE: &[&str] = &[
    "age",
    "alt-svc",
    "connection",
    "content-encoding",
    "content-length",
    "keep-alive",
    "transfer-encoding",
    "cf-cache-status",
    "cf-ray",
    "date",
    "etag",
    "expires",
    "last-modified",
    "nel",
    "report-to",
    "server-timing",
    "via",
    "x-amz-cf-id",
    "x-amz-cf-pop",
    "x-amzn-requestid",
    "x-amzn-trace-id",
    "x-cache",
    "x-request-id",
    "x-runtime",
    "x-served-by",
    "x-timer",
    "x-trace-id",
];

fn volatile(name: &str) -> bool {
    VOLATILE.contains(&name) || name.starts_with("x-ratelimit") || name.starts_with("ratelimit")
}

/// Fields that identify a record, in the order they are tried.
const IDS: &[&str] = &["id", "_id", "uuid", "key"];

/// Lists of records that each carry a distinct id become tables keyed
/// `[id=7]`, so a record is matched by its id rather than its place in the
/// list; other lists are compared place by place.
fn by_id(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            Value::Object(map.iter().map(|(k, v)| (k.clone(), by_id(v))).collect())
        }
        Value::Array(items) => {
            let keyed = IDS.iter().find_map(|id| {
                let keys: Vec<String> = items
                    .iter()
                    .map(|item| match item.get(id)? {
                        Value::String(s) => Some(s.clone()),
                        Value::Number(n) => Some(n.to_string()),
                        _ => None,
                    })
                    .collect::<Option<_>>()?;
                let mut unique = keys.clone();
                unique.sort();
                unique.dedup();
                (!keys.is_empty() && unique.len() == keys.len()).then_some((id, keys))
            });
            match keyed {
                Some((id, keys)) => Value::Object(
                    keys.into_iter()
                        .zip(items)
                        .map(|(key, item)| (format!("[{id}={key}]"), by_id(item)))
                        .collect(),
                ),
                None => Value::Array(items.iter().map(by_id).collect()),
            }
        }
        other => other.clone(),
    }
}

/// Compares `old` with `new`.
pub fn compare(old: &Snapshot, new: &Snapshot) -> Comparison {
    let (headers, ignored) = headers(&old.headers, &new.headers);
    Comparison {
        old_label: old.label.clone(),
        new_label: new.label.clone(),
        status: (old.status, new.status),
        headers,
        ignored,
        body: body(&old.body, &new.body),
        time_ms: (old.total_ms, new.total_ms),
        size: (old.body.len(), new.body.len()),
    }
}

/// Headers by lower-case name; several of a name are joined with `, `.
fn headers(old: &[(String, String)], new: &[(String, String)]) -> (Vec<HeaderChange>, Vec<String>) {
    use std::collections::BTreeMap;
    let table = |list: &[(String, String)]| {
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (name, value) in list {
            map.entry(name.to_ascii_lowercase())
                .or_default()
                .push(value.clone());
        }
        map.into_iter()
            .map(|(k, v)| (k, v.join(", ")))
            .collect::<BTreeMap<_, _>>()
    };
    let (a, b) = (table(old), table(new));
    let mut names: Vec<&String> = a.keys().chain(b.keys()).collect();
    names.sort();
    names.dedup();
    let mut changes = Vec::new();
    let mut ignored = Vec::new();
    for name in names {
        let (x, y) = (a.get(name), b.get(name));
        if x == y {
            continue;
        }
        if volatile(name) {
            ignored.push(name.clone());
            continue;
        }
        changes.push(HeaderChange {
            name: name.clone(),
            old: x.cloned(),
            new: y.cloned(),
        });
    }
    (changes, ignored)
}

fn body(old: &[u8], new: &[u8]) -> BodyDiff {
    if old == new {
        return BodyDiff::Same;
    }
    let json = |b: &[u8]| serde_json::from_slice::<Value>(b).ok();
    if let (Some(a), Some(b)) = (json(old), json(new)) {
        let fields: Vec<FieldChange> = diff::fields_as_they_are(&by_id(&a), &by_id(&b))
            .into_iter()
            .map(|mut f| {
                f.field = f.field.replace(".[", "[");
                f
            })
            .collect();
        return if fields.is_empty() {
            BodyDiff::Same
        } else {
            BodyDiff::Json(fields)
        };
    }
    match (std::str::from_utf8(old), std::str::from_utf8(new)) {
        (Ok(a), Ok(b)) => BodyDiff::Text(diff::lines(a, b)),
        _ => BodyDiff::Binary,
    }
}

impl Comparison {
    /// Nothing that matters differs (times and volatile headers aside).
    pub fn is_same(&self) -> bool {
        self.status.0 == self.status.1 && self.headers.is_empty() && self.body == BodyDiff::Same
    }

    /// How many things differ: the status, each header, each field or line.
    pub fn count(&self) -> usize {
        usize::from(self.status.0 != self.status.1)
            + self.headers.len()
            + match &self.body {
                BodyDiff::Same => 0,
                BodyDiff::Json(fields) => fields.len(),
                BodyDiff::Text(lines) => {
                    lines.iter().filter(|l| !matches!(l, Line::Same(_))).count()
                }
                BodyDiff::Binary => 1,
            }
    }

    /// For a terminal or a pull request: what differs, old then new.
    pub fn to_text(&self) -> String {
        let mut out = format!("--- {}\n+++ {}\n", self.old_label, self.new_label);
        if self.status.0 != self.status.1 {
            out += &format!("status: {} → {}\n", self.status.0, self.status.1);
        }
        let show = |v: &Option<String>| v.clone().unwrap_or_else(|| "(none)".into());
        for h in &self.headers {
            out += &format!("header {}: {} → {}\n", h.name, show(&h.old), show(&h.new));
        }
        match &self.body {
            BodyDiff::Same => {}
            BodyDiff::Json(fields) => {
                for f in fields {
                    let show = |v: &Option<Value>| {
                        v.as_ref()
                            .map(diff::show)
                            .unwrap_or_else(|| "(none)".into())
                    };
                    out += &format!(
                        "body {}: {} → {}\n",
                        field(&f.field),
                        show(&f.old),
                        show(&f.new)
                    );
                }
            }
            BodyDiff::Text(lines) => {
                for line in lines {
                    match line {
                        Line::Same(_) => {}
                        Line::Removed(t) => out += &format!("- {t}\n"),
                        Line::Added(t) => out += &format!("+ {t}\n"),
                    }
                }
            }
            BodyDiff::Binary => {
                out += &format!("body: {} bytes → {} bytes\n", self.size.0, self.size.1);
            }
        }
        if self.is_same() {
            out += "the same";
            if !self.ignored.is_empty() {
                out += &format!(" (left out: {})", self.ignored.join(", "));
            }
            out += "\n";
        }
        out += &format!(
            "time: {:.0} ms → {:.0} ms · size: {} → {} bytes\n",
            self.time_ms.0, self.time_ms.1, self.size.0, self.size.1
        );
        out
    }
}

/// `$` for the whole body, the path otherwise.
pub fn field(path: &str) -> &str {
    if path.is_empty() { "$" } else { path }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(label: &str, status: u16, headers: &[(&str, &str)], body: &str) -> Snapshot {
        Snapshot {
            label: label.into(),
            status,
            headers: headers
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
            body: body.as_bytes().to_vec(),
            total_ms: 10.0,
        }
    }

    #[test]
    fn json_bodies_field_by_field() {
        let old = snap(
            "then",
            200,
            &[
                ("Content-Type", "application/json"),
                ("Date", "Mon"),
                ("X-Version", "1"),
            ],
            r#"{"id":1,"name":"Ada","tags":["a"],"address":{"city":"London"}}"#,
        );
        let new = snap(
            "now",
            201,
            &[
                ("content-type", "application/json"),
                ("date", "Tue"),
                ("X-Version", "2"),
                ("X-New", "yes"),
            ],
            r#"{"address":{"city":"Paris"},"id":1,"name":"Ada","tags":["a","b"],"email":"a@x"}"#,
        );
        let c = compare(&old, &new);
        assert_eq!(c.status, (200, 201));
        assert_eq!(c.ignored, vec!["date"]);
        assert_eq!(
            c.headers,
            vec![
                HeaderChange {
                    name: "x-new".into(),
                    old: None,
                    new: Some("yes".into())
                },
                HeaderChange {
                    name: "x-version".into(),
                    old: Some("1".into()),
                    new: Some("2".into())
                },
            ]
        );
        let BodyDiff::Json(fields) = &c.body else {
            panic!("{:?}", c.body)
        };
        let names: Vec<&str> = fields.iter().map(|f| f.field.as_str()).collect();
        assert_eq!(names, ["tags", "address.city", "email"]);
        assert!(!c.is_same());
        assert_eq!(c.count(), 1 + 2 + 3);
        let text = c.to_text();
        assert!(text.contains("status: 200 → 201"), "{text}");
        assert!(text.contains("body address.city: London → Paris"), "{text}");
        assert!(text.contains("header x-new: (none) → yes"), "{text}");
    }

    #[test]
    fn records_matched_by_id() {
        let old = snap(
            "a",
            200,
            &[],
            r#"[{"id":1,"name":"Ada"},{"id":2,"name":"Bob","tags":[{"name":"x"}]}]"#,
        );
        let new = snap(
            "b",
            200,
            &[],
            r#"[{"id":2,"name":"Bob","tags":[{"name":"y"}]},{"id":1,"name":"Ada L."},{"id":3,"name":"Cy"}]"#,
        );
        let c = compare(&old, &new);
        let BodyDiff::Json(fields) = &c.body else {
            panic!("{:?}", c.body)
        };
        let names: Vec<&str> = fields.iter().map(|f| f.field.as_str()).collect();
        // Moved records are found by id, a renamed one is one change, a new
        // one is listed field by field.
        assert_eq!(
            names,
            [
                "[id=1].name",
                "[id=2].tags[0].name",
                "[id=3].id",
                "[id=3].name"
            ]
        );
        let nested = snap("a", 200, &[], r#"{"users":[{"id":"u1","email":"a"}]}"#);
        let changed = snap("b", 200, &[], r#"{"users":[{"id":"u1","email":"b"}]}"#);
        let BodyDiff::Json(fields) = compare(&nested, &changed).body else {
            panic!()
        };
        assert_eq!(fields[0].field, "users[id=u1].email");
    }

    #[test]
    fn the_same_apart_from_formatting_and_volatile_headers() {
        let old = snap(
            "a",
            200,
            &[("Date", "Mon"), ("CF-Ray", "1")],
            "{\"a\": 1, \"b\": [1, 2]}",
        );
        let new = snap(
            "b",
            200,
            &[("Date", "Tue"), ("CF-Ray", "2")],
            "{\"b\":[1,2],\"a\":1}",
        );
        let c = compare(&old, &new);
        assert!(c.is_same(), "{c:?}");
        assert_eq!(c.count(), 0);
        assert!(c.to_text().contains("the same (left out: cf-ray, date)"));
    }

    #[test]
    fn text_and_binary_bodies() {
        let c = compare(
            &snap("a", 200, &[], "one\ntwo\nthree"),
            &snap("b", 200, &[], "one\n2\nthree"),
        );
        let BodyDiff::Text(lines) = &c.body else {
            panic!()
        };
        assert_eq!(
            lines.iter().filter(|l| !matches!(l, Line::Same(_))).count(),
            2
        );
        let text = c.to_text();
        assert!(text.contains("- two\n") && text.contains("+ 2\n"), "{text}");

        let mut a = snap("a", 200, &[], "");
        a.body = vec![0xff, 0xfe, 0x00];
        let mut b = a.clone();
        b.body.push(1);
        let c = compare(&a, &b);
        assert_eq!(c.body, BodyDiff::Binary);
        assert!(c.to_text().contains("body: 3 bytes → 4 bytes"));
        // A whole JSON value replaced (an array by an object): the body itself.
        let c = compare(
            &snap("a", 200, &[], "[1,2]"),
            &snap("b", 200, &[], "{\"a\":1}"),
        );
        let BodyDiff::Json(fields) = &c.body else {
            panic!()
        };
        assert_eq!(field(&fields[0].field), "$");
    }
}
