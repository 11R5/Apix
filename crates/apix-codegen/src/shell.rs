use std::iter::repeat_n;

use apix_core::{Auth, Method, Request};

use crate::{effective_headers, form_fields, full_url, text_body};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    /// `curl` for bash, zsh and sh (also macOS and Linux terminals).
    Bash,
    /// `curl.exe` for the Windows command prompt.
    Cmd,
    /// `Invoke-RestMethod` for Windows PowerShell 5.1 and PowerShell 7.
    PowerShell,
}

impl Shell {
    pub const ALL: [Shell; 3] = [Shell::Bash, Shell::Cmd, Shell::PowerShell];

    pub fn label(self) -> &'static str {
        match self {
            Shell::Bash => "cURL (bash)",
            Shell::Cmd => "cURL (Windows cmd)",
            Shell::PowerShell => "PowerShell",
        }
    }
}

/// Writes `request` as a command for `shell`. Pass a resolved request to
/// get concrete values, or the saved one to keep `{{variables}}`.
pub fn to_shell(request: &Request, shell: Shell) -> String {
    match shell {
        Shell::Bash => curl(request, "curl", bash_quote, " \\\n  "),
        Shell::Cmd => curl(request, "curl.exe", cmd_quote, " ^\n  "),
        Shell::PowerShell => powershell(request),
    }
}

fn curl(request: &Request, program: &str, quote: fn(&str) -> String, newline: &str) -> String {
    let mut first = program.to_owned();
    match request.method {
        Method::Get => {}
        Method::Head => first.push_str(" --head"),
        other => {
            first.push_str(" -X ");
            first.push_str(other.as_str());
        }
    }
    first.push(' ');
    first.push_str(&quote(&full_url(request)));

    let mut parts = vec![first];
    for (key, value) in effective_headers(request) {
        parts.push(format!("-H {}", quote(&format!("{key}: {value}"))));
    }
    if let Auth::Basic { username, password } = &request.auth {
        parts.push(format!("-u {}", quote(&format!("{username}:{password}"))));
    }
    if let Some(text) = text_body(&request.body) {
        parts.push(format!("--data-raw {}", quote(text)));
    } else if let Some(fields) = form_fields(&request.body) {
        for field in fields {
            parts.push(format!(
                "--data-urlencode {}",
                quote(&format!("{}={}", field.key, field.value))
            ));
        }
    }
    parts.join(newline)
}

/// POSIX single quotes: nothing inside is special; `'` becomes `'\''`.
fn bash_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Quotes for `curl.exe`'s own argument parser, then escapes every
/// character `cmd` would interpret, so the text reaches curl unchanged.
fn cmd_quote(s: &str) -> String {
    cmd_escape(&argv_quote(s))
}

/// Windows argv rules: backslashes are literal unless they precede a quote.
fn argv_quote(s: &str) -> String {
    let mut out = String::from('"');
    let mut backslashes = 0;
    for c in s.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.extend(repeat_n('\\', backslashes));
                backslashes = 0;
                out.push(c);
            }
        }
    }
    out.extend(repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

/// Caret-escapes cmd metacharacters (quotes included, so cmd never enters a
/// quoted region and every caret applies). `%NAME` gets a caret after the
/// `%`, which stops variable expansion; `^` + two line feeds is a newline.
fn cmd_escape(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + s.len() / 4);
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '^' | '&' | '|' | '<' | '>' | '(' | ')' | '"' | '!' => {
                out.push('^');
                out.push(c);
            }
            '%' => {
                out.push('%');
                if chars
                    .get(i + 1)
                    .is_some_and(|n| n.is_ascii_alphanumeric() || *n == '_')
                {
                    out.push('^');
                }
            }
            '\r' => {}
            '\n' => out.push_str("^\n\n"),
            _ => out.push(c),
        }
    }
    out
}

/// PowerShell single quotes: only quote characters are special, doubled.
/// PowerShell treats typographic single quotes as quotes too.
fn ps_quote(s: &str) -> String {
    let mut out = String::from('\'');
    for c in s.chars() {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}') {
            out.push(c);
        }
        out.push(c);
    }
    out.push('\'');
    out
}

fn powershell(request: &Request) -> String {
    let mut headers = effective_headers(request);
    // Windows PowerShell 5.1 refuses Content-Type in -Headers; it has its own parameter.
    let content_type = headers
        .iter()
        .position(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|ix| headers.remove(ix).1);
    let basic = match &request.auth {
        Auth::Basic { username, password } => Some(format!("{username}:{password}")),
        _ => None,
    };

    let mut out = String::new();
    let has_headers = !headers.is_empty() || basic.is_some();
    if has_headers {
        out.push_str("$headers = @{\n");
        for (key, value) in &headers {
            out.push_str(&format!("    {} = {}\n", ps_quote(key), ps_quote(value)));
        }
        if let Some(credentials) = &basic {
            out.push_str(&format!(
                "    'Authorization' = 'Basic ' + [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes({}))\n",
                ps_quote(credentials)
            ));
        }
        out.push_str("}\n");
    }

    let mut has_body = true;
    if let Some(text) = text_body(&request.body) {
        let here_string_safe = !text.lines().any(|l| l.starts_with("'@"));
        if text.contains('\n') && here_string_safe {
            out.push_str(&format!(
                "$body = @'\n{}\n'@\n",
                text.trim_end_matches(['\r', '\n'])
            ));
        } else {
            out.push_str(&format!("$body = {}\n", ps_quote(text)));
        }
    } else if let Some(fields) = form_fields(&request.body) {
        out.push_str("$body = @{\n");
        for field in fields {
            out.push_str(&format!(
                "    {} = {}\n",
                ps_quote(&field.key),
                ps_quote(&field.value)
            ));
        }
        out.push_str("}\n");
    } else {
        has_body = false;
    }

    let method = match request.method {
        Method::Get => "Get",
        Method::Post => "Post",
        Method::Put => "Put",
        Method::Patch => "Patch",
        Method::Delete => "Delete",
        Method::Head => "Head",
        Method::Options => "Options",
    };
    out.push_str(&format!(
        "Invoke-RestMethod -Method {method} -Uri {}",
        ps_quote(&full_url(request))
    ));
    if has_headers {
        out.push_str(" -Headers $headers");
    }
    if let Some(content_type) = content_type {
        out.push_str(&format!(" -ContentType {}", ps_quote(&content_type)));
    }
    if has_body {
        out.push_str(" -Body $body");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use apix_core::{ApiKeyLocation, Body, KeyValue};

    fn post_json() -> Request {
        let mut r = Request::new(Method::Post, "https://api.example.com/users");
        r.query.push(KeyValue::new("dry run", "yes & no"));
        r.query.push(KeyValue {
            enabled: false,
            ..KeyValue::new("debug", "1")
        });
        r.headers.push(KeyValue::new("Accept", "application/json"));
        r.auth = Auth::Bearer {
            token: "t0k3n".into(),
        };
        r.body = Body::Json("{\"name\": \"O'Brien\", \"path\": \"C:\\\\tmp\"}".into());
        r
    }

    #[test]
    fn bash_quotes_and_orders_like_curl_users_expect() {
        assert_eq!(
            to_shell(&post_json(), Shell::Bash),
            "curl -X POST 'https://api.example.com/users?dry+run=yes+%26+no' \\\n  \
             -H 'Accept: application/json' \\\n  \
             -H 'Authorization: Bearer t0k3n' \\\n  \
             -H 'Content-Type: application/json' \\\n  \
             --data-raw '{\"name\": \"O'\\''Brien\", \"path\": \"C:\\\\tmp\"}'"
        );
    }

    #[test]
    fn get_and_head_need_no_method_flag() {
        let get = Request::new(Method::Get, "https://x.dev/a");
        assert_eq!(to_shell(&get, Shell::Bash), "curl 'https://x.dev/a'");
        let head = Request::new(Method::Head, "https://x.dev/a");
        assert_eq!(
            to_shell(&head, Shell::Bash),
            "curl --head 'https://x.dev/a'"
        );
    }

    #[test]
    fn cmd_escapes_quotes_carets_and_percent_variables() {
        assert_eq!(
            argv_quote(r#"say "hi" \ C:\dir\"#),
            r#""say \"hi\" \ C:\dir\\""#
        );
        assert_eq!(
            cmd_quote(r#"{"a":"%PATH% & 100%"}"#),
            r#"^"{\^"a\^":\^"%^PATH% ^& 100%\^"}^""#
        );
        let cmd = to_shell(&post_json(), Shell::Cmd);
        // `%26` gets a caret too: cmd must not read `%26+no...%` as a variable.
        assert!(
            cmd.starts_with("curl.exe -X POST ^\"https://api.example.com/users?dry+run=yes+%^26+no^\" ^\n  -H ^\"Accept: application/json^\""),
            "{cmd}"
        );
    }

    #[test]
    fn powershell_moves_content_type_and_uses_here_strings() {
        let mut r = post_json();
        r.body = Body::Json("{\n  \"name\": \"Ada\"\n}".into());
        assert_eq!(
            to_shell(&r, Shell::PowerShell),
            "$headers = @{\n    'Accept' = 'application/json'\n    'Authorization' = 'Bearer t0k3n'\n}\n\
             $body = @'\n{\n  \"name\": \"Ada\"\n}\n'@\n\
             Invoke-RestMethod -Method Post -Uri 'https://api.example.com/users?dry+run=yes+%26+no' -Headers $headers -ContentType 'application/json' -Body $body"
        );
    }

    #[test]
    fn powershell_handles_forms_basic_auth_and_quotes() {
        let mut r = Request::new(Method::Put, "https://x.dev/it's");
        r.auth = Auth::Basic {
            username: "ada".into(),
            password: "p'w".into(),
        };
        r.body = Body::Form(vec![KeyValue::new("sku", "A-1")]);
        let ps = to_shell(&r, Shell::PowerShell);
        assert!(
            ps.contains("[Text.Encoding]::UTF8.GetBytes('ada:p''w'))"),
            "{ps}"
        );
        assert!(ps.contains("$body = @{\n    'sku' = 'A-1'\n}"), "{ps}");
        assert!(ps.ends_with("-Method Put -Uri 'https://x.dev/it''s' -Headers $headers -ContentType 'application/x-www-form-urlencoded' -Body $body"), "{ps}");
    }

    #[test]
    fn query_api_keys_go_into_the_url() {
        let mut r = Request::new(Method::Get, "https://x.dev/a?page=1");
        r.auth = Auth::ApiKey {
            key: "api_key".into(),
            value: "s3cr3t".into(),
            location: ApiKeyLocation::Query,
        };
        assert_eq!(
            to_shell(&r, Shell::Bash),
            "curl 'https://x.dev/a?page=1&api_key=s3cr3t'"
        );
    }
}
