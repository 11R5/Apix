//! The parts of a URL the request editor keeps apart: the query, which is
//! also the Params table, and `:name` path variables, which are filled in
//! from the request's own values when it is sent.

use crate::KeyValue;

/// A URL in three parts: up to the query, the query (without `?`), and the
/// fragment (with its `#`). `{{variables}}` are left as they are.
pub fn split(url: &str) -> (&str, Option<&str>, &str) {
    let (rest, fragment) = match url.find('#') {
        Some(at) => (&url[..at], &url[at..]),
        None => (url, ""),
    };
    match rest.find('?') {
        Some(at) => (&rest[..at], Some(&rest[at + 1..]), fragment),
        None => (rest, None, fragment),
    }
}

/// The rows of a query, decoded, in order. A part without `=` is a key with
/// an empty value.
pub fn query_rows(query: &str) -> Vec<KeyValue> {
    url::form_urlencoded::parse(query.as_bytes())
        .map(|(key, value)| KeyValue::new(key.into_owned(), value.into_owned()))
        .collect()
}

/// What a row's text needs escaped to read back as the same row: the
/// characters that would split it, and `%` and `+`, which would decode.
fn escape(text: &str, key: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '%' => out.push_str("%25"),
            '&' => out.push_str("%26"),
            '#' => out.push_str("%23"),
            '+' => out.push_str("%2B"),
            '=' if key => out.push_str("%3D"),
            c => out.push(c),
        }
    }
    out
}

/// The enabled rows as a query to show in a URL: as written, except for
/// what has to be escaped; a row with no value is just its key.
pub fn query_text(rows: &[KeyValue]) -> String {
    rows.iter()
        .filter(|r| r.enabled && !r.key.is_empty())
        .map(|r| {
            if r.value.is_empty() {
                escape(&r.key, true)
            } else {
                format!("{}={}", escape(&r.key, true), escape(&r.value, false))
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// `url` with its query replaced by the enabled rows (none: no `?`).
pub fn with_query(url: &str, rows: &[KeyValue]) -> String {
    let (base, _, fragment) = split(url);
    let query = query_text(rows);
    if query.is_empty() {
        format!("{base}{fragment}")
    } else {
        format!("{base}?{query}{fragment}")
    }
}

/// The name of a path variable at the start of a segment, and where it ends.
fn variable_at(segment: &str) -> Option<usize> {
    let rest = segment.strip_prefix(':')?;
    let len = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .unwrap_or(rest.len());
    (len > 0).then_some(len + 1)
}

/// The path, as segments of the part before the query; the scheme and host
/// are not path, so `http://host:8080` has none.
fn path_segments(url: &str) -> impl Iterator<Item = &str> {
    let (before, _, _) = split(url);
    let path = match before.find("://") {
        Some(at) => {
            let after = &before[at + 3..];
            after.find('/').map_or("", |slash| &after[slash..])
        }
        None => before,
    };
    path.split('/')
}

/// The `:name` path variables of a URL, in order, each once.
pub fn path_variables(url: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for segment in path_segments(url) {
        if let Some(end) = variable_at(segment) {
            let name = &segment[1..end];
            if !names.iter().any(|n| n == name) {
                names.push(name.to_owned());
            }
        }
    }
    names
}

/// A value as one path segment: what would end or split it is encoded,
/// `{{variables}}` stay readable.
fn encode_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        let keep = b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@{}".contains(&b);
        if keep {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `url` with each `:name` segment replaced by its enabled value, encoded.
/// A variable without a value stays as written.
pub fn fill_path_variables(url: &str, vars: &[KeyValue]) -> String {
    if vars.iter().all(|v| !v.enabled || v.value.is_empty()) {
        return url.to_owned();
    }
    let (before, _, _) = split(url);
    let after_before = &url[before.len()..];
    let (prefix, path) = match before.find("://") {
        Some(at) => {
            let after = &before[at + 3..];
            match after.find('/') {
                Some(slash) => before.split_at(at + 3 + slash),
                None => (before, ""),
            }
        }
        None => ("", before),
    };
    let filled: Vec<String> = path
        .split('/')
        .map(|segment| {
            let Some(end) = variable_at(segment) else {
                return segment.to_owned();
            };
            let name = &segment[1..end];
            match vars
                .iter()
                .find(|v| v.enabled && v.key == name && !v.value.is_empty())
            {
                Some(v) => format!("{}{}", encode_segment(&v.value), &segment[end..]),
                None => segment.to_owned(),
            }
        })
        .collect();
    format!("{prefix}{}{after_before}", filled.join("/"))
}

/// A request as the editor keeps it: a query written into the URL moves to
/// the rows (before the ones already there), and the path variables follow
/// the URL's `:name` segments in order, keeping their values.
pub fn tidy(request: &mut crate::Request) {
    let (base, query, fragment) = split(&request.url);
    if let Some(query) = query {
        let mut rows = query_rows(query);
        rows.append(&mut request.query);
        request.query = rows;
        request.url = format!("{base}{fragment}");
    }
    request.path_vars = path_variables(&request.url)
        .into_iter()
        .map(|name| {
            request
                .path_vars
                .iter()
                .find(|v| v.key == name)
                .cloned()
                .unwrap_or_else(|| KeyValue::new(name, ""))
        })
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tidying_moves_the_query_and_follows_the_path() {
        let mut request = crate::Request::new(crate::Method::Get, "{{base}}/users/:id?page=2#top");
        request.query.push(KeyValue::new("sort", "name"));
        request.path_vars = vec![KeyValue::new("gone", "1"), KeyValue::new("id", "7")];
        tidy(&mut request);
        assert_eq!(request.url, "{{base}}/users/:id#top");
        assert_eq!(
            request.query,
            [KeyValue::new("page", "2"), KeyValue::new("sort", "name")]
        );
        assert_eq!(request.path_vars, [KeyValue::new("id", "7")]);
        let again = request.clone();
        tidy(&mut request);
        assert_eq!(request, again);
    }

    #[test]
    fn splits_query_and_fragment() {
        assert_eq!(
            split("https://x.io/a?b=1&c#top"),
            ("https://x.io/a", Some("b=1&c"), "#top")
        );
        assert_eq!(split("{{base}}/a"), ("{{base}}/a", None, ""));
        assert_eq!(split("/a?"), ("/a", Some(""), ""));
    }

    #[test]
    fn query_rows_round_trip() {
        let rows = query_rows("q=a%20b&tag=x+y&flag&path=%2F%26&v={{v}}");
        assert_eq!(
            rows,
            [
                KeyValue::new("q", "a b"),
                KeyValue::new("tag", "x y"),
                KeyValue::new("flag", ""),
                KeyValue::new("path", "/&"),
                KeyValue::new("v", "{{v}}"),
            ]
        );
        let text = query_text(&rows);
        assert_eq!(text, "q=a b&tag=x y&flag&path=/%26&v={{v}}");
        assert_eq!(query_rows(&text), rows);
        // What would decode or split is escaped, and comes back the same.
        let odd = vec![KeyValue::new("a=b", "100%+1#2")];
        assert_eq!(query_rows(&query_text(&odd)), odd);
    }

    #[test]
    fn disabled_rows_stay_out_of_the_url() {
        let mut off = KeyValue::new("debug", "1");
        off.enabled = false;
        let rows = vec![KeyValue::new("page", "2"), off];
        assert_eq!(
            with_query("https://x.io/a?old=1#f", &rows),
            "https://x.io/a?page=2#f"
        );
        assert_eq!(with_query("https://x.io/a?old=1", &[]), "https://x.io/a");
    }

    #[test]
    fn finds_path_variables_in_the_path_only() {
        assert_eq!(
            path_variables("https://api.io:8443/users/:id/posts/:post_id.json?x=:no"),
            ["id", "post_id"]
        );
        assert_eq!(path_variables("{{base}}/orgs/:org/users/:org"), ["org"]);
        assert!(path_variables("http://localhost:8080/").is_empty());
        assert!(path_variables("https://x.io/v1/files/a:copy").is_empty());
        assert!(path_variables("https://x.io/:").is_empty());
    }

    #[test]
    fn fills_path_variables() {
        let vars = vec![
            KeyValue::new("id", "7"),
            KeyValue::new("post_id", "a b/c"),
            KeyValue::new("empty", ""),
        ];
        assert_eq!(
            fill_path_variables(
                "https://api.io:8443/users/:id/posts/:post_id.json/:empty?x=:id",
                &vars
            ),
            "https://api.io:8443/users/7/posts/a%20b%2Fc.json/:empty?x=:id"
        );
        assert_eq!(
            fill_path_variables("{{base}}/users/:id", &vars),
            "{{base}}/users/7"
        );
        // Filling twice changes nothing more.
        let once = fill_path_variables("https://x.io/:id", &vars);
        assert_eq!(fill_path_variables(&once, &vars), once);
        let mut off = KeyValue::new("id", "9");
        off.enabled = false;
        assert_eq!(
            fill_path_variables("https://x.io/:id", &[off]),
            "https://x.io/:id"
        );
    }
}
