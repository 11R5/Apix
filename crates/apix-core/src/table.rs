//! A JSON response as a table, for the response's Table and Chart views and
//! `apix send --table`: the list of records in it (an array of objects, or
//! the largest such array inside an object), one column per key.

use serde_json::Value;

/// Most columns kept; the rest are counted.
pub const MAX_COLUMNS: usize = 40;

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    /// Where the records were found, as a JSONPath (`$`, `$.items`).
    pub source: String,
    /// Columns left out past [`MAX_COLUMNS`].
    pub more_columns: usize,
}

/// The records in `value`, or `None` when there is no list to show.
pub fn from_json(value: &Value) -> Option<Table> {
    let (path, list) = find_list(value, "$".to_owned(), 0)?;
    Some(table_of(list, path))
}

/// The array to show: `value` itself, or the longest array of objects inside
/// it (two levels down at most), else an array of plain values.
fn find_list(value: &Value, path: String, depth: usize) -> Option<(String, &Vec<Value>)> {
    match value {
        Value::Array(items) if !items.is_empty() => Some((path, items)),
        Value::Object(map) if depth < 2 => {
            let mut best: Option<(String, &Vec<Value>)> = None;
            for (key, inner) in map {
                let inner_path = if key.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    format!("{path}.{key}")
                } else {
                    format!("{path}['{key}']")
                };
                if let Some((p, list)) = find_list(inner, inner_path, depth + 1) {
                    let objects = list.iter().filter(|v| v.is_object()).count();
                    let better = best.as_ref().is_none_or(|(_, b)| {
                        let b_objects = b.iter().filter(|v| v.is_object()).count();
                        (objects, list.len()) > (b_objects, b.len())
                    });
                    if better {
                        best = Some((p, list));
                    }
                }
            }
            best
        }
        _ => None,
    }
}

fn table_of(items: &[Value], source: String) -> Table {
    let objects = items.iter().any(Value::is_object);
    if !objects {
        return Table {
            columns: vec!["value".into()],
            rows: items.iter().map(|v| vec![v.clone()]).collect(),
            source,
            more_columns: 0,
        };
    }
    // Columns in the order keys first appear.
    let mut columns: Vec<String> = Vec::new();
    for item in items {
        if let Value::Object(map) = item {
            for key in map.keys() {
                if !columns.iter().any(|c| c == key) {
                    columns.push(key.clone());
                }
            }
        }
    }
    let more_columns = columns.len().saturating_sub(MAX_COLUMNS);
    columns.truncate(MAX_COLUMNS);
    let rows = items
        .iter()
        .map(|item| {
            columns
                .iter()
                .map(|c| match item {
                    Value::Object(map) => map.get(c).cloned().unwrap_or(Value::Null),
                    other if c == "value" => other.clone(),
                    _ => Value::Null,
                })
                .collect()
        })
        .collect();
    Table {
        columns,
        rows,
        source,
        more_columns,
    }
}

/// A cell as one line: text as it is, `—` for nothing, anything else as JSON.
pub fn cell_text(value: &Value) -> String {
    match value {
        Value::Null => "—".into(),
        Value::String(s) => s.replace(['\r', '\n'], " "),
        other => other.to_string(),
    }
}

/// A cell as a number, when it is one (or text that reads as one).
pub fn cell_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok().filter(|n| n.is_finite()),
        _ => None,
    }
}

impl Table {
    /// Columns where every non-empty cell is a number.
    pub fn numeric_columns(&self) -> Vec<usize> {
        (0..self.columns.len())
            .filter(|&c| {
                let mut any = false;
                for row in &self.rows {
                    match &row[c] {
                        Value::Null => {}
                        v if cell_number(v).is_some() => any = true,
                        _ => return false,
                    }
                }
                any
            })
            .collect()
    }

    /// The column that best names a row: `name`, `title`, `label`, … else the
    /// first column of text.
    pub fn label_column(&self) -> Option<usize> {
        const NAMES: [&str; 8] = [
            "name", "title", "label", "username", "email", "key", "id", "date",
        ];
        NAMES
            .iter()
            .find_map(|n| self.columns.iter().position(|c| c.eq_ignore_ascii_case(n)))
            .or_else(|| {
                (0..self.columns.len()).find(|&c| self.rows.iter().any(|r| r[c].is_string()))
            })
            .or((!self.columns.is_empty()).then_some(0))
    }

    /// The table as aligned text, cells cut at `width` characters.
    pub fn to_text(&self, width: usize) -> String {
        let cut = |s: String| -> String {
            if s.chars().count() > width {
                let mut t: String = s.chars().take(width.saturating_sub(1)).collect();
                t.push('…');
                t
            } else {
                s
            }
        };
        let cells: Vec<Vec<String>> = self
            .rows
            .iter()
            .map(|r| r.iter().map(|v| cut(cell_text(v))).collect())
            .collect();
        let widths: Vec<usize> = (0..self.columns.len())
            .map(|c| {
                cells
                    .iter()
                    .map(|r| r[c].chars().count())
                    .chain([self.columns[c].chars().count()])
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        let line = |values: &[String]| -> String {
            values
                .iter()
                .zip(&widths)
                .map(|(v, w)| format!("{v:<w$}", w = *w))
                .collect::<Vec<_>>()
                .join("  ")
                .trim_end()
                .to_owned()
        };
        let mut out = line(
            &self
                .columns
                .iter()
                .map(|c| cut(c.clone()))
                .collect::<Vec<_>>(),
        );
        out.push('\n');
        out += &widths
            .iter()
            .map(|w| "─".repeat(*w))
            .collect::<Vec<_>>()
            .join("  ");
        out.push('\n');
        for row in &cells {
            out += &line(row);
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finds_records_and_columns() {
        let t = from_json(&json!([
            {"id": 1, "name": "Ada", "age": 36},
            {"id": 2, "name": "Linus", "city": "Helsinki"}
        ]))
        .unwrap();
        assert_eq!(t.source, "$");
        assert_eq!(t.columns, ["id", "name", "age", "city"]);
        assert_eq!(t.rows[1][2], Value::Null);
        assert_eq!(t.numeric_columns(), [0, 2]);
        assert_eq!(t.label_column(), Some(1));

        // The list inside an envelope.
        let t = from_json(
            &json!({"page": 1, "meta": {"tags": ["a"]}, "items": [{"v": "3"}, {"v": "4.5"}]}),
        )
        .unwrap();
        assert_eq!(t.source, "$.items");
        assert_eq!(t.numeric_columns(), [0]);

        // Plain values.
        let t = from_json(&json!([1, 2, 3])).unwrap();
        assert_eq!(t.columns, ["value"]);
        assert!(from_json(&json!({"a": 1})).is_none());
        assert!(from_json(&json!([])).is_none());
    }

    #[test]
    fn text_tables_line_up() {
        let t = from_json(&json!([{"name": "Ada Lovelace", "n": 1}, {"name": "Bo", "n": null}]))
            .unwrap();
        let text = t.to_text(8);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "name      n");
        assert_eq!(lines[2], "Ada Lov…  1");
        assert_eq!(lines[3], "Bo        —");
    }
}
