use apix_core::{Auth, Body, KeyValue, Method, Request};

/// A request read from a `curl` command, plus anything that was left out.
#[derive(Debug, Clone, PartialEq)]
pub struct CurlImport {
    pub request: Request,
    pub warnings: Vec<String>,
}

/// Whether `text` looks like a curl command worth parsing (for paste detection).
pub fn looks_like_curl(text: &str) -> bool {
    let first = text.split_whitespace().next().unwrap_or_default();
    let program = first.rsplit(['/', '\\']).next().unwrap_or(first);
    program.eq_ignore_ascii_case("curl") || program.eq_ignore_ascii_case("curl.exe")
}

/// Parses a `curl` command as bash/zsh or Windows `cmd` would pass it,
/// including browsers' "Copy as cURL" output in both styles.
pub fn from_curl(command: &str) -> Result<CurlImport, String> {
    let command = command.trim();
    if !looks_like_curl(command) {
        return Err("This is not a curl command.".into());
    }
    let cmd_style =
        command.contains("^\"") || command.contains(" ^\n") || command.contains(" ^\r\n");
    let words = if cmd_style {
        split_cmd(command)
    } else {
        split_posix(command)?
    };
    Ok(build(&words[1..]))
}

#[derive(Default)]
struct Parsed {
    url: Option<String>,
    method: Option<String>,
    headers: Vec<(String, String)>,
    data: Vec<String>,
    urlencoded: Vec<String>,
    form: Vec<String>,
    user: Option<String>,
    get: bool,
    head: bool,
    warnings: Vec<String>,
}

/// Flags that take no value and change nothing Apix needs to keep.
const IGNORED_FLAGS: &[&str] = &[
    "--compressed",
    "-L",
    "--location",
    "-k",
    "--insecure",
    "-s",
    "--silent",
    "-S",
    "--show-error",
    "-i",
    "--include",
    "-v",
    "--verbose",
    "-f",
    "--fail",
    "--http1.1",
    "--http2",
    "-g",
    "--globoff",
    "-#",
    "--progress-bar",
    "-N",
    "--no-buffer",
];
/// Options with a value that Apix does not import yet.
const SKIPPED_WITH_VALUE: &[&str] = &[
    "-o",
    "--output",
    "-m",
    "--max-time",
    "--connect-timeout",
    "-x",
    "--proxy",
    "--retry",
    "-w",
    "--write-out",
    "-c",
    "--cookie-jar",
    "--cacert",
    "--cert",
    "--key",
    "-E",
    "--resolve",
    "--limit-rate",
];

fn build(args: &[String]) -> CurlImport {
    let mut p = Parsed::default();
    let mut i = 0;
    let value = |i: &mut usize, flag: &str, warnings: &mut Vec<String>| -> Option<String> {
        *i += 1;
        let v = args.get(*i).cloned();
        if v.is_none() {
            warnings.push(format!("{flag} has no value."));
        }
        v
    };
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "-X" | "--request" => p.method = value(&mut i, arg, &mut p.warnings),
            "-H" | "--header" => {
                if let Some(h) = value(&mut i, arg, &mut p.warnings)
                    && let Some((k, v)) = h.split_once(':')
                    && !v.trim().is_empty()
                {
                    p.headers.push((k.trim().to_owned(), v.trim().to_owned()));
                }
            }
            "-d" | "--data" | "--data-ascii" | "--data-binary" | "--data-raw" => {
                if let Some(d) = value(&mut i, arg, &mut p.warnings) {
                    if d.starts_with('@') && arg != "--data-raw" {
                        p.warnings
                            .push(format!("Body from file ({d}) is not imported."));
                    } else {
                        p.data.push(d);
                    }
                }
            }
            "--data-urlencode" => p.urlencoded.extend(value(&mut i, arg, &mut p.warnings)),
            "-F" | "--form" => p.form.extend(value(&mut i, arg, &mut p.warnings)),
            "-u" | "--user" => p.user = value(&mut i, arg, &mut p.warnings),
            "-A" | "--user-agent" => {
                if let Some(v) = value(&mut i, arg, &mut p.warnings) {
                    p.headers.push(("User-Agent".into(), v));
                }
            }
            "-e" | "--referer" => {
                if let Some(v) = value(&mut i, arg, &mut p.warnings) {
                    p.headers.push(("Referer".into(), v));
                }
            }
            "-b" | "--cookie" => {
                if let Some(v) = value(&mut i, arg, &mut p.warnings) {
                    if v.contains('=') {
                        p.headers.push(("Cookie".into(), v));
                    } else {
                        p.warnings.push(format!("Cookie file {v} is not imported."));
                    }
                }
            }
            "--url" => p.url = value(&mut i, arg, &mut p.warnings),
            "-G" | "--get" => p.get = true,
            "-I" | "--head" => p.head = true,
            _ if IGNORED_FLAGS.contains(&arg) => {}
            _ if SKIPPED_WITH_VALUE.contains(&arg) => {
                value(&mut i, arg, &mut p.warnings);
                p.warnings.push(format!("Option {arg} is not imported."));
            }
            // -XPOST, -HAccept: x
            _ if arg.starts_with("-X") && arg.len() > 2 => p.method = Some(arg[2..].to_owned()),
            // Bundled short flags such as -sSL.
            _ if arg.starts_with('-')
                && !arg.starts_with("--")
                && arg.len() > 2
                && arg[1..].chars().all(|c| "sSLkivfgN#".contains(c)) => {}
            _ if arg.starts_with('-') && arg.len() > 1 => {
                p.warnings.push(format!("Option {arg} is not imported."));
            }
            _ => {
                if p.url.is_none() {
                    p.url = Some(arg.to_owned());
                } else {
                    p.warnings.push(format!("Extra URL {arg} is ignored."));
                }
            }
        }
        i += 1;
    }
    finish(p)
}

fn finish(mut p: Parsed) -> CurlImport {
    let mut url = p.url.take().unwrap_or_default();
    if !url.is_empty() && !url.contains("://") {
        url = format!("http://{url}");
    }

    // With -G, -d data becomes the query string.
    if p.get && !p.data.is_empty() {
        let extra = p.data.join("&");
        url.push(if url.contains('?') { '&' } else { '?' });
        url.push_str(&extra);
        p.data.clear();
    }

    let mut request = Request::default();
    let (base, query) = match url.split_once('?') {
        Some((base, query)) => (base.to_owned(), Some(query.to_owned())),
        None => (url.clone(), None),
    };
    request.url = base;
    if let Some(query) = query {
        request.query = url::form_urlencoded::parse(query.as_bytes())
            .map(|(k, v)| KeyValue::new(k, v))
            .collect();
    }

    // Authorization headers become auth settings, so the Auth tab shows them.
    let mut headers = Vec::new();
    for (key, value) in p.headers {
        if key.eq_ignore_ascii_case("authorization")
            && let Some(token) = value
                .strip_prefix("Bearer ")
                .or_else(|| value.strip_prefix("bearer "))
        {
            request.auth = Auth::Bearer {
                token: token.trim().to_owned(),
            };
            continue;
        }
        headers.push(KeyValue::new(key, value));
    }
    if let Some(user) = p.user {
        let (username, password) = user.split_once(':').unwrap_or((&user, ""));
        request.auth = Auth::Basic {
            username: username.to_owned(),
            password: password.to_owned(),
        };
    }

    let content_type = headers
        .iter()
        .find(|h| h.key.eq_ignore_ascii_case("content-type"))
        .map(|h| h.value.to_ascii_lowercase());

    if !p.form.is_empty() {
        p.warnings.push(
            "multipart/form-data (-F) is not supported yet; text fields were imported as a URL-encoded form and files were skipped."
                .into(),
        );
        request.body = Body::Form(
            p.form
                .iter()
                .filter(|f| !f.contains("=@") && !f.contains("=<"))
                .filter_map(|f| f.split_once('='))
                .map(|(k, v)| KeyValue::new(k, v.trim_matches('"')))
                .collect(),
        );
        headers.retain(|h| !h.key.eq_ignore_ascii_case("content-type"));
    } else if !p.urlencoded.is_empty() || (is_form(content_type.as_deref()) && !p.data.is_empty()) {
        let mut rows: Vec<KeyValue> = p
            .data
            .iter()
            .flat_map(|d| {
                url::form_urlencoded::parse(d.as_bytes()).map(|(k, v)| KeyValue::new(k, v))
            })
            .collect();
        rows.extend(p.urlencoded.iter().map(|d| match d.split_once('=') {
            Some((k, v)) => KeyValue::new(k, v),
            None => KeyValue::new(d.trim_start_matches('='), ""),
        }));
        request.body = Body::Form(rows);
    } else if !p.data.is_empty() {
        let text = p.data.join("&");
        request.body = match content_type.as_deref() {
            Some(ct) if ct.contains("json") => Body::Json(text),
            Some(ct) if ct.contains("xml") => Body::Xml(text),
            Some(_) => Body::Text(text),
            // curl's default is a form; JSON-looking data was almost certainly meant as JSON.
            None if looks_like_json(&text) => Body::Json(text),
            None if text.contains('=') => Body::Form(
                url::form_urlencoded::parse(text.as_bytes())
                    .map(|(k, v)| KeyValue::new(k, v))
                    .collect(),
            ),
            None => Body::Text(text),
        };
    }
    request.headers = headers;

    let method = match (&p.method, p.head) {
        (Some(m), _) => m.to_ascii_uppercase(),
        (None, true) => "HEAD".into(),
        (None, false) if !request.body.is_none() => "POST".into(),
        _ => "GET".into(),
    };
    request.method = method.parse().unwrap_or_else(|_| {
        p.warnings.push(format!(
            "Method {method} is not supported yet; imported as GET."
        ));
        Method::Get
    });

    CurlImport {
        request,
        warnings: p.warnings,
    }
}

fn is_form(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|ct| ct.contains("x-www-form-urlencoded"))
}

fn looks_like_json(text: &str) -> bool {
    let t = text.trim();
    (t.starts_with('{') && t.ends_with('}')) || (t.starts_with('[') && t.ends_with(']'))
}

/// Splits like bash: '...' literal, "..." with \" \\ \$ \` escapes,
/// $'...' with C escapes, \ escapes and \-newline continuations.
fn split_posix(input: &str) -> Result<Vec<String>, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            '\\' => {
                in_word = true;
                match chars.get(i + 1) {
                    Some('\n') => in_word = !word.is_empty(),
                    Some('\r') if chars.get(i + 2) == Some(&'\n') => {
                        in_word = !word.is_empty();
                        i += 1;
                    }
                    Some(&n) => word.push(n),
                    None => {}
                }
                i += 1;
            }
            '\'' => {
                in_word = true;
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    word.push(chars[i]);
                    i += 1;
                }
                if i >= chars.len() {
                    return Err("Unterminated ' quote.".into());
                }
            }
            '"' => {
                in_word = true;
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\'
                        && matches!(chars.get(i + 1), Some('"' | '\\' | '$' | '`' | '\n'))
                    {
                        i += 1;
                        if chars[i] != '\n' {
                            word.push(chars[i]);
                        }
                    } else {
                        word.push(chars[i]);
                    }
                    i += 1;
                }
                if i >= chars.len() {
                    return Err("Unterminated \" quote.".into());
                }
            }
            '$' if chars.get(i + 1) == Some(&'\'') => {
                in_word = true;
                i += 2;
                while i < chars.len() && chars[i] != '\'' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                        match chars[i] {
                            'n' => word.push('\n'),
                            't' => word.push('\t'),
                            'r' => word.push('\r'),
                            '0' => word.push('\0'),
                            'x' => {
                                let hex: String = chars[i + 1..]
                                    .iter()
                                    .take(2)
                                    .take_while(|c| c.is_ascii_hexdigit())
                                    .collect();
                                if let Ok(v) = u8::from_str_radix(&hex, 16) {
                                    word.push(v as char);
                                    i += hex.len();
                                }
                            }
                            'u' => {
                                let hex: String = chars[i + 1..]
                                    .iter()
                                    .take(4)
                                    .take_while(|c| c.is_ascii_hexdigit())
                                    .collect();
                                if let Some(ch) =
                                    u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
                                {
                                    word.push(ch);
                                    i += hex.len();
                                }
                            }
                            other => word.push(other),
                        }
                    } else {
                        word.push(chars[i]);
                    }
                    i += 1;
                }
                if i >= chars.len() {
                    return Err("Unterminated $' quote.".into());
                }
            }
            _ => {
                in_word = true;
                word.push(c);
            }
        }
        i += 1;
    }
    if in_word {
        words.push(word);
    }
    Ok(words)
}

/// Splits like `cmd` then `curl.exe` would: `^` escapes the next character
/// (`^` + newline continues the line, `^` + two newlines is a newline), then
/// Windows argv rules apply to quotes and backslashes.
fn split_cmd(input: &str) -> Vec<String> {
    // Pass 1: undo cmd's caret escaping.
    let chars: Vec<char> = input.chars().filter(|&c| c != '\r').collect();
    let mut unescaped = String::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '^' && i + 1 < chars.len() {
            if chars[i + 1] == '\n' {
                if chars.get(i + 2) == Some(&'\n') {
                    unescaped.push('\n');
                    i += 3;
                } else {
                    unescaped.push(' ');
                    i += 2;
                }
                continue;
            }
            unescaped.push(chars[i + 1]);
            i += 2;
            continue;
        }
        unescaped.push(chars[i]);
        i += 1;
    }

    // Pass 2: Windows argv parsing.
    let chars: Vec<char> = unescaped.chars().collect();
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut quoted = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            let start = i;
            while i < chars.len() && chars[i] == '\\' {
                i += 1;
            }
            let count = i - start;
            in_word = true;
            if chars.get(i) == Some(&'"') {
                word.extend(std::iter::repeat_n('\\', count / 2));
                if count % 2 == 1 {
                    word.push('"');
                    i += 1;
                }
            } else {
                word.extend(std::iter::repeat_n('\\', count));
            }
            continue;
        }
        match c {
            '"' => {
                in_word = true;
                quoted = !quoted;
            }
            c if (c == ' ' || c == '\t' || c == '\n') && !quoted => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            _ => {
                in_word = true;
                word.push(c);
            }
        }
        i += 1;
    }
    if in_word {
        words.push(word);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Shell, to_shell};
    use apix_core::ApiKeyLocation;

    #[test]
    fn reads_a_browser_style_bash_command() {
        let cmd = r#"curl 'https://api.example.com/v1/orders?status=open&page=2' \
  -H 'accept: application/json' \
  -H 'authorization: Bearer abc.def' \
  -H 'content-type: application/json' \
  --data-raw '{"sku":"A-1","note":"it'\''s fine"}' \
  --compressed"#;
        let imported = from_curl(cmd).unwrap();
        let r = &imported.request;
        assert_eq!(r.method, Method::Post);
        assert_eq!(r.url, "https://api.example.com/v1/orders");
        assert_eq!(
            r.query,
            [KeyValue::new("status", "open"), KeyValue::new("page", "2")]
        );
        assert_eq!(
            r.headers,
            [
                KeyValue::new("accept", "application/json"),
                KeyValue::new("content-type", "application/json")
            ]
        );
        assert_eq!(
            r.auth,
            Auth::Bearer {
                token: "abc.def".into()
            }
        );
        assert_eq!(
            r.body,
            Body::Json(r#"{"sku":"A-1","note":"it's fine"}"#.into())
        );
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    }

    #[test]
    fn reads_a_browser_style_cmd_command() {
        let cmd = "curl ^\"https://api.example.com/search?q=100^%^%^\" ^\n  -H ^\"accept: */*^\" ^\n  --data-raw ^\"^{^\\^\"q^\\^\":^\\^\"a ^& b^\\^\"^}^\"";
        let r = from_curl(cmd).unwrap().request;
        assert_eq!(r.url, "https://api.example.com/search");
        assert_eq!(r.query, [KeyValue::new("q", "100%%")]);
        assert_eq!(r.headers, [KeyValue::new("accept", "*/*")]);
        assert_eq!(r.body, Body::Json(r#"{"q":"a & b"}"#.into()));
    }

    #[test]
    fn understands_common_flags() {
        let r = from_curl("curl -sSL -XPUT -u ada:pw -A 'apix-test' -b 'sid=1' example.com/x")
            .unwrap()
            .request;
        assert_eq!(r.method, Method::Put);
        assert_eq!(r.url, "http://example.com/x");
        assert_eq!(
            r.auth,
            Auth::Basic {
                username: "ada".into(),
                password: "pw".into()
            }
        );
        assert_eq!(
            r.headers,
            [
                KeyValue::new("User-Agent", "apix-test"),
                KeyValue::new("Cookie", "sid=1")
            ]
        );

        let get = from_curl("curl -G https://x.dev/s -d 'q=rust lang' -d page=2")
            .unwrap()
            .request;
        assert_eq!(get.method, Method::Get);
        assert_eq!(
            get.query,
            [KeyValue::new("q", "rust lang"), KeyValue::new("page", "2")]
        );
        assert!(get.body.is_none());

        let form = from_curl(
            "curl https://x.dev/login -d 'user=ada&pass=p%26w' --data-urlencode 'note=a b'",
        )
        .unwrap()
        .request;
        assert_eq!(form.method, Method::Post);
        assert_eq!(
            form.body,
            Body::Form(vec![
                KeyValue::new("user", "ada"),
                KeyValue::new("pass", "p&w"),
                KeyValue::new("note", "a b")
            ])
        );
    }

    #[test]
    fn reports_what_it_skips() {
        let imported = from_curl(
            "curl -o out.json --proxy http://p:8080 -F 'f=@a.png' -F 'k=v' https://x.dev/up",
        )
        .unwrap();
        assert_eq!(
            imported.request.body,
            Body::Form(vec![KeyValue::new("k", "v")])
        );
        let all = imported.warnings.join("\n");
        assert!(
            all.contains("-o is not imported") && all.contains("--proxy is not imported"),
            "{all}"
        );
        assert!(all.contains("multipart/form-data"), "{all}");
        assert!(from_curl("wget https://x.dev").is_err());
        assert!(from_curl("curl 'unterminated").is_err());
    }

    #[test]
    fn round_trips_through_every_shell_that_curl_reads() {
        let mut original = Request::new(Method::Patch, "https://api.example.com/items/7");
        original.query.push(KeyValue::new("dry run", "a&b=c"));
        original
            .headers
            .push(KeyValue::new("X-Note", "say \"hi\" 100% ^_^ C:\\path\\"));
        original
            .headers
            .push(KeyValue::new("Content-Type", "application/json"));
        original.auth = Auth::Bearer { token: "t".into() };
        original.body = Body::Json("{\n  \"q\": \"it's %PATH% & <b>\"\n}".into());

        for shell in [Shell::Bash, Shell::Cmd] {
            let command = to_shell(&original, shell);
            let back = from_curl(&command).unwrap_or_else(|e| panic!("{shell:?}: {e}\n{command}"));
            assert!(back.warnings.is_empty(), "{shell:?}: {:?}", back.warnings);
            assert_eq!(back.request, original, "{shell:?}\n{command}");
        }

        let mut keyed = Request::new(Method::Get, "https://x.dev/a");
        keyed.auth = Auth::ApiKey {
            key: "k".into(),
            value: "v".into(),
            location: ApiKeyLocation::Query,
        };
        let back = from_curl(&to_shell(&keyed, Shell::Bash)).unwrap().request;
        assert_eq!(
            back.query,
            [KeyValue::new("k", "v")],
            "query keys come back as params"
        );
    }
}
