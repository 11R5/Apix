//! Data files for data-driven runs: every row is one iteration, its columns
//! become variables (`{{email}}`, `pm.iterationData.get("email")`).
//!
//! - CSV: the first line names the columns. Quotes, doubled quotes inside
//!   them, line breaks inside them and a byte order mark are understood; a
//!   header with semicolons and no commas (what Excel writes in many
//!   locales) is read with `;`.
//! - JSON: an array of objects. Strings are used as they are, `null` as an
//!   empty value, anything else as its JSON text (`42`, `true`, `{"a":1}`).

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};

/// One iteration's values.
pub type Row = BTreeMap<String, String>;

/// Reads a `.csv` or `.json` data file (by extension; other names are read
/// as JSON when they start with `[`, else as CSV).
pub fn load(path: &Path) -> Result<Vec<Row>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    let json = match ext.as_deref() {
        Some("json") => true,
        Some("csv") => false,
        _ => text
            .trim_start_matches('\u{feff}')
            .trim_start()
            .starts_with('['),
    };
    let rows = if json {
        parse_json(&text)
    } else {
        parse_csv(&text)
    };
    rows.with_context(|| format!("reading the data file {}", path.display()))
}

pub fn parse_json(text: &str) -> Result<Vec<Row>> {
    let value: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
    let serde_json::Value::Array(items) = value else {
        bail!("expected an array of objects, like [{{\"id\": 1}}, {{\"id\": 2}}]");
    };
    items
        .into_iter()
        .enumerate()
        .map(|(ix, item)| match item {
            serde_json::Value::Object(fields) => Ok(fields
                .into_iter()
                .map(|(k, v)| {
                    let v = match v {
                        serde_json::Value::String(s) => s,
                        serde_json::Value::Null => String::new(),
                        other => other.to_string(),
                    };
                    (k, v)
                })
                .collect()),
            _ => bail!("item {} is not an object", ix + 1),
        })
        .collect()
}

pub fn parse_csv(text: &str) -> Result<Vec<Row>> {
    let text = text.trim_start_matches('\u{feff}');
    let first_line = text.lines().next().unwrap_or_default();
    let delimiter = if !first_line.contains(',') && first_line.contains(';') {
        ';'
    } else {
        ','
    };
    let mut records = records(text, delimiter)?.into_iter();
    let Some(header) = records.next() else {
        return Ok(Vec::new());
    };
    let header: Vec<String> = header.into_iter().map(|h| h.trim().to_owned()).collect();
    Ok(records
        .filter(|record| record.iter().any(|field| !field.is_empty()))
        .map(|record| {
            header
                .iter()
                .enumerate()
                .filter(|(_, name)| !name.is_empty())
                .map(|(ix, name)| (name.clone(), record.get(ix).cloned().unwrap_or_default()))
                .collect()
        })
        .collect())
}

/// Splits CSV text into records of fields (RFC 4180, either line ending).
fn records(text: &str, delimiter: char) -> Result<Vec<Vec<String>>> {
    let mut out = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut line = 1;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => quoted = false,
                '\n' => {
                    line += 1;
                    field.push(c);
                }
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => quoted = true,
            c if c == delimiter => record.push(std::mem::take(&mut field)),
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' => {
                line += 1;
                record.push(std::mem::take(&mut field));
                out.push(std::mem::take(&mut record));
            }
            _ => field.push(c),
        }
    }
    if quoted {
        bail!("a quoted value that starts before line {line} is never closed");
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        out.push(record);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pairs: &[(&str, &str)]) -> Row {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn reads_csv_with_quotes_and_line_breaks() {
        let text = "\u{feff}id,name,note\r\n1,Ada,\"says \"\"hi\"\"\"\r\n\r\n2,\"Lin, B\",\"two\nlines\"\n3,Kim\n";
        assert_eq!(
            parse_csv(text).unwrap(),
            [
                row(&[("id", "1"), ("name", "Ada"), ("note", "says \"hi\"")]),
                row(&[("id", "2"), ("name", "Lin, B"), ("note", "two\nlines")]),
                row(&[("id", "3"), ("name", "Kim"), ("note", "")]),
            ]
        );
    }

    #[test]
    fn reads_semicolon_csv() {
        assert_eq!(
            parse_csv("id;city\n7;İzmir").unwrap(),
            [row(&[("id", "7"), ("city", "İzmir")])]
        );
    }

    #[test]
    fn unclosed_quote_is_an_error() {
        assert!(parse_csv("a\n\"open\n").is_err());
    }

    #[test]
    fn reads_json_rows() {
        let text = r#"[{"id": 1, "name": "Ada", "admin": true, "tags": ["x"], "gone": null}]"#;
        assert_eq!(
            parse_json(text).unwrap(),
            [row(&[
                ("id", "1"),
                ("name", "Ada"),
                ("admin", "true"),
                ("tags", "[\"x\"]"),
                ("gone", "")
            ])]
        );
        assert!(parse_json(r#"{"id": 1}"#).is_err());
        assert!(parse_json("[1]").is_err());
    }
}
