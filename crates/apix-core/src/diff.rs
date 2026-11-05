//! What changed in a collection's files between two versions, the way a
//! reviewer wants to read it: a request's fields (headers by name, the body
//! line by line) rather than lines of TOML, so reformatting a file is not a
//! change and a moved request is one move, not a delete and an add.

use std::path::Path;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::git::{FileRole, role_of, slashes};
use crate::model::Request;

/// One line of a line-by-line comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "op", content = "text", rename_all = "lowercase")]
pub enum Line {
    Same(String),
    Removed(String),
    Added(String),
}

/// Compares two texts line by line (longest common subsequence). Very large
/// texts are shown as all removed, then all added, rather than spending
/// seconds on them.
pub fn lines(old: &str, new: &str) -> Vec<Line> {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    // Common head and tail first: most edits touch a few lines in the middle.
    let head = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let tail = a[head..]
        .iter()
        .rev()
        .zip(b[head..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (mid_a, mid_b) = (&a[head..a.len() - tail], &b[head..b.len() - tail]);
    let mut out: Vec<Line> = a[..head]
        .iter()
        .map(|l| Line::Same(l.to_string()))
        .collect();
    if mid_a.len().saturating_mul(mid_b.len()) > 4_000_000 {
        out.extend(mid_a.iter().map(|l| Line::Removed(l.to_string())));
        out.extend(mid_b.iter().map(|l| Line::Added(l.to_string())));
    } else {
        let (n, m) = (mid_a.len(), mid_b.len());
        // lcs[i][j]: longest common subsequence of mid_a[i..] and mid_b[j..].
        let mut lcs = vec![vec![0u32; m + 1]; n + 1];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                lcs[i][j] = if mid_a[i] == mid_b[j] {
                    lcs[i + 1][j + 1] + 1
                } else {
                    lcs[i + 1][j].max(lcs[i][j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && mid_a[i] == mid_b[j] {
                out.push(Line::Same(mid_a[i].to_string()));
                i += 1;
                j += 1;
            } else if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
                out.push(Line::Added(mid_b[j].to_string()));
                j += 1;
            } else {
                out.push(Line::Removed(mid_a[i].to_string()));
                i += 1;
            }
        }
    }
    out.extend(
        a[a.len() - tail..]
            .iter()
            .map(|l| Line::Same(l.to_string())),
    );
    out
}

/// One field that differs, named by its path in the file (`headers.Accept`,
/// `body.json`, `settings.timeout_ms`). `None` means the field is not there.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldChange {
    pub field: String,
    pub old: Option<Value>,
    pub new: Option<Value>,
}

impl FieldChange {
    /// Both sides are text and one of them has several lines: worth showing line by line.
    pub fn multiline(&self) -> Option<(&str, &str)> {
        fn text(v: &Option<Value>) -> Option<&str> {
            match v {
                Some(Value::String(s)) => Some(s.as_str()),
                None => Some(""),
                _ => None,
            }
        }
        let (old, new) = (text(&self.old)?, text(&self.new)?);
        (old.contains('\n') || new.contains('\n')).then_some((old, new))
    }
}

/// A value as one short line: text as it is, anything else as JSON.
pub fn show(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The fields that differ between two values of the same file.
pub fn fields(old: &Value, new: &Value) -> Vec<FieldChange> {
    let mut out = Vec::new();
    walk(
        String::new(),
        Some(&normalize(old)),
        Some(&normalize(new)),
        &mut out,
    );
    out
}

/// The fields that differ, without matching named rows first (for values
/// that are not request files, such as response bodies).
pub fn fields_as_they_are(old: &Value, new: &Value) -> Vec<FieldChange> {
    let mut out = Vec::new();
    walk(String::new(), Some(old), Some(new), &mut out);
    out
}

/// Lists of named rows (`headers`, `query`, `[[example]]` …) become tables
/// keyed by their name, so rows are matched by name rather than position,
/// and a row that is only a value becomes that value.
fn normalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            Value::Object(map.iter().map(|(k, v)| (k.clone(), normalize(v))).collect())
        }
        Value::Array(items) => match keyed(items) {
            Some((id, names)) => {
                let mut map = Map::new();
                for (name, item) in names.into_iter().zip(items) {
                    let mut row = item.as_object().cloned().unwrap_or_default();
                    row.remove(id);
                    let enabled = row.get("enabled").and_then(Value::as_bool);
                    let row = match (row.len(), row.get("value"), enabled) {
                        (1, Some(v), _) => normalize(v),
                        (2, Some(Value::String(v)), Some(false)) => {
                            Value::String(format!("{v} (off)"))
                        }
                        _ => normalize(&Value::Object(row)),
                    };
                    map.insert(name, row);
                }
                Value::Object(map)
            }
            None => Value::Array(items.iter().map(normalize).collect()),
        },
        other => other.clone(),
    }
}

/// When every item is a table with a distinct `key` (or else `name`), that
/// field's name and the items' names.
fn keyed(items: &[Value]) -> Option<(&'static str, Vec<String>)> {
    if items.is_empty() {
        return None;
    }
    ["key", "name"].into_iter().find_map(|id| {
        let names: Vec<String> = items
            .iter()
            .map(|item| item.get(id)?.as_str().map(str::to_owned))
            .collect::<Option<_>>()?;
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        (sorted.len() == names.len()).then_some((id, names))
    })
}

fn walk(path: String, old: Option<&Value>, new: Option<&Value>, out: &mut Vec<FieldChange>) {
    if old == new {
        return;
    }
    let join = |key: &str| {
        if path.is_empty() {
            key.to_owned()
        } else {
            format!("{path}.{key}")
        }
    };
    match (old, new) {
        (Some(Value::Object(a)), Some(Value::Object(b))) => {
            for (key, value) in a {
                walk(join(key), Some(value), b.get(key), out);
            }
            for (key, value) in b.iter().filter(|(k, _)| !a.contains_key(*k)) {
                walk(join(key), None, Some(value), out);
            }
        }
        // A table added or removed whole: each of its fields.
        (Some(Value::Object(a)), None) if !a.is_empty() => {
            for (key, value) in a {
                walk(join(key), Some(value), None, out);
            }
        }
        (None, Some(Value::Object(b))) if !b.is_empty() => {
            for (key, value) in b {
                walk(join(key), None, Some(value), out);
            }
        }
        (Some(Value::Array(a)), Some(Value::Array(b)))
            if a.iter().chain(b).any(|v| v.is_object()) =>
        {
            for i in 0..a.len().max(b.len()) {
                walk(format!("{path}[{i}]"), a.get(i), b.get(i), out);
            }
        }
        _ => out.push(FieldChange {
            field: path,
            old: old.cloned(),
            new: new.cloned(),
        }),
    }
}

/// What kind of file of a collection changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Request,
    Script,
    Folder,
    Collection,
    Environment,
    Project,
    Other,
}

/// How the file changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Added,
    Removed,
    Changed,
    /// A request file that moved or was renamed without other changes.
    Moved,
}

/// One changed file.
#[derive(Debug, Clone, Serialize)]
pub struct FileDiff {
    /// Relative to the compared folder, with `/`.
    pub path: String,
    pub kind: FileKind,
    pub status: Status,
    /// Where a moved request was before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moved_from: Option<String>,
    /// A request's method and name (of its newer side).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// For TOML files: the fields that differ (for an added or removed
    /// file, empty).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<FieldChange>,
    /// For scripts and other text: the lines.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<Line>,
    /// A side could not be read as its format (it is shown as text instead).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The parsed request, to recognise moves.
    #[serde(skip)]
    value: Option<Value>,
}

/// Compares two versions of `path` (relative to a collection or project
/// folder). `None` for a side the file is missing from. Returns `None` when
/// they mean the same (only formatting differs).
pub fn file(path: &Path, old: Option<&[u8]>, new: Option<&[u8]>) -> Option<FileDiff> {
    let status = match (old, new) {
        (None, None) => return None,
        (None, Some(_)) => Status::Added,
        (Some(_), None) => Status::Removed,
        (Some(a), Some(b)) if a == b => return None,
        _ => Status::Changed,
    };
    let kind = kind_of(path);
    let mut diff = FileDiff {
        path: slashes(path),
        kind,
        status,
        moved_from: None,
        method: None,
        name: None,
        fields: Vec::new(),
        lines: Vec::new(),
        note: None,
        value: None,
    };
    let text = |bytes: Option<&[u8]>| bytes.map(|b| String::from_utf8_lossy(b).into_owned());
    let (old_text, new_text) = (text(old), text(new));
    let is_toml = path
        .extension()
        .is_some_and(|e| e == "toml" || e == crate::store::REQUEST_EXTENSION);
    if is_toml {
        let parse = |text: &Option<String>| -> Result<Option<Value>, String> {
            let Some(text) = text else { return Ok(None) };
            let value = if kind == FileKind::Request {
                toml::from_str::<Request>(text)
                    .map_err(|e| e.to_string())
                    .and_then(|r| serde_json::to_value(r).map_err(|e| e.to_string()))
            } else {
                toml::from_str::<toml::Value>(text)
                    .map_err(|e| e.to_string())
                    .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
            };
            value.map(Some)
        };
        match (parse(&old_text), parse(&new_text)) {
            (Ok(a), Ok(b)) => {
                let shown = b.as_ref().or(a.as_ref());
                if kind == FileKind::Request
                    && let Some(request) = shown
                {
                    diff.method = request.get("method").map(show);
                    diff.name = request.get("name").map(show);
                }
                if let (Some(a), Some(b)) = (&a, &b) {
                    diff.fields = fields(a, b);
                    if diff.fields.is_empty() {
                        return None;
                    }
                }
                diff.value = b.or(a);
                return Some(diff);
            }
            (Err(e), _) | (_, Err(e)) => {
                diff.note = Some(format!("not valid TOML: {e}"));
            }
        }
    }
    let is_text = |bytes: Option<&[u8]>| bytes.is_none_or(|b| std::str::from_utf8(b).is_ok());
    if !(is_text(old) && is_text(new)) {
        diff.note.get_or_insert_with(|| "binary file".to_owned());
        return Some(diff);
    }
    // A new or deleted script is shown whole, unless it is long.
    let whole = kind == FileKind::Script
        && old_text
            .as_deref()
            .or(new_text.as_deref())
            .unwrap_or_default()
            .lines()
            .count()
            <= 200;
    if status == Status::Changed || whole {
        diff.lines = lines(
            old_text.as_deref().unwrap_or_default(),
            new_text.as_deref().unwrap_or_default(),
        );
    }
    Some(diff)
}

fn kind_of(path: &Path) -> FileKind {
    let name = path.file_name().unwrap_or_default();
    if name == crate::project::PROJECT_FILE {
        return FileKind::Project;
    }
    if path
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|d| d == crate::store::ENVIRONMENTS_DIR)
        && path.extension().is_some_and(|e| e == "toml")
    {
        return FileKind::Environment;
    }
    match role_of(path) {
        FileRole::Request => FileKind::Request,
        FileRole::RequestScript(_) => FileKind::Script,
        FileRole::Folder(_) if name == crate::store::FOLDER_FILE => FileKind::Folder,
        FileRole::Folder(_) => FileKind::Script,
        FileRole::Everything if name == crate::store::COLLECTION_FILE => FileKind::Collection,
        FileRole::Everything => FileKind::Script,
        // `apix.toml` of a collection inside a project.
        FileRole::Other if name == crate::store::COLLECTION_FILE => FileKind::Collection,
        FileRole::Other => FileKind::Other,
    }
}

/// Turns a removed and an added request with the same content into one move.
pub fn pair_moves(diffs: &mut Vec<FileDiff>) {
    let mut i = 0;
    while i < diffs.len() {
        let removed = &diffs[i];
        let twin = (removed.kind == FileKind::Request
            && removed.status == Status::Removed
            && removed.value.is_some())
        .then(|| {
            diffs.iter().position(|d| {
                d.kind == FileKind::Request && d.status == Status::Added && d.value == removed.value
            })
        })
        .flatten();
        match twin {
            Some(j) => {
                let from = diffs.remove(i).path;
                let j = if j > i { j - 1 } else { j };
                diffs[j].status = Status::Moved;
                diffs[j].moved_from = Some(from);
            }
            None => i += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn line_diff_keeps_order() {
        let got = lines("a\nb\nc\nd", "a\nc\nx\nd");
        assert_eq!(
            got,
            vec![
                Line::Same("a".into()),
                Line::Removed("b".into()),
                Line::Same("c".into()),
                Line::Added("x".into()),
                Line::Same("d".into()),
            ]
        );
        assert!(
            lines("same", "same")
                .iter()
                .all(|l| matches!(l, Line::Same(_)))
        );
    }

    #[test]
    fn rows_are_matched_by_name() {
        let old = json!({
            "method": "GET",
            "headers": [{"key": "Accept", "value": "a"}, {"key": "X-Old", "value": "1"}],
        });
        let new = json!({
            "method": "POST",
            "headers": [{"key": "X-New", "value": "2", "enabled": false}, {"key": "Accept", "value": "b"}],
        });
        let got = fields(&old, &new);
        let names: Vec<&str> = got.iter().map(|c| c.field.as_str()).collect();
        assert_eq!(
            names,
            ["method", "headers.Accept", "headers.X-Old", "headers.X-New"]
        );
        assert_eq!(got[1].old, Some(json!("a")));
        assert_eq!(got[1].new, Some(json!("b")));
        assert_eq!(got[2].new, None);
        assert_eq!(got[3].new, Some(json!("2 (off)")));
    }

    #[test]
    fn request_files_compare_by_meaning() {
        let a = b"name = \"Get\"\nurl = \"https://x/users\"\nmethod = \"GET\"\n";
        let b = b"method = \"GET\"\nname = \"Get\"\n\nurl = \"https://x/users\"\n";
        assert!(file(Path::new("users/get.apix"), Some(a), Some(b)).is_none());

        let c = b"name = \"Get\"\nurl = \"https://x/users/2\"\n[body]\njson = \"{\\n  \\\"a\\\": 1\\n}\"\n";
        let d = file(Path::new("users/get.apix"), Some(a), Some(c)).unwrap();
        assert_eq!(d.kind, FileKind::Request);
        assert_eq!(d.status, Status::Changed);
        assert_eq!(d.name.as_deref(), Some("Get"));
        let names: Vec<&str> = d.fields.iter().map(|c| c.field.as_str()).collect();
        assert_eq!(names, ["url", "body.json"]);
        assert!(d.fields[1].multiline().is_some());
    }

    #[test]
    fn scripts_environments_and_moves() {
        let js = file(
            Path::new("users/get.test.js"),
            Some(b"pm.test('a', () => {});\n"),
            Some(b"pm.test('b', () => {});\n"),
        )
        .unwrap();
        assert_eq!(js.kind, FileKind::Script);
        assert_eq!(js.lines.len(), 2);

        let env = file(
            Path::new("environments/dev.toml"),
            Some(b"[vars]\nbase = \"a\"\n"),
            Some(b"[vars]\nbase = \"b\"\ntoken = \"t\"\n"),
        )
        .unwrap();
        assert_eq!(env.kind, FileKind::Environment);
        let names: Vec<&str> = env.fields.iter().map(|c| c.field.as_str()).collect();
        assert_eq!(names, ["vars.base", "vars.token"]);

        let request = b"name = \"Get\"\nurl = \"https://x\"\n";
        let mut diffs = vec![
            file(Path::new("old/get.apix"), Some(request), None).unwrap(),
            file(
                Path::new("apix.toml"),
                Some(b"name = \"a\""),
                Some(b"name = \"b\""),
            )
            .unwrap(),
            file(Path::new("new/get.apix"), None, Some(request)).unwrap(),
        ];
        pair_moves(&mut diffs);
        assert_eq!(diffs.len(), 2);
        assert_eq!(diffs[0].kind, FileKind::Collection);
        assert_eq!(diffs[1].status, Status::Moved);
        assert_eq!(diffs[1].moved_from.as_deref(), Some("old/get.apix"));
    }
}
