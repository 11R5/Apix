use std::iter::repeat_n;

use apix_core::{Auth, Body, Method, Request};

use crate::{effective_headers, form_fields, full_url, multipart_parts, text_body};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    /// `curl` for bash, zsh and sh (also macOS and Linux terminals).
    Bash,
    /// `curl.exe` for the Windows command prompt.
    Cmd,
    /// `Invoke-RestMethod` for Windows PowerShell 5.1 and PowerShell 7.
    PowerShell,
    /// `curl` for the fish shell, whose single quotes treat `\` specially.
    Fish,
    /// HTTPie's `http` command (3.0 or later, for `--raw`), quoted for bash.
    Httpie,
    /// GNU `wget`, quoted for bash.
    Wget,
}

impl Shell {
    pub const ALL: [Shell; 6] = [
        Shell::Bash,
        Shell::Cmd,
        Shell::PowerShell,
        Shell::Fish,
        Shell::Httpie,
        Shell::Wget,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Shell::Bash => "cURL (bash)",
            Shell::Cmd => "cURL (Windows cmd)",
            Shell::PowerShell => "PowerShell",
            Shell::Fish => "cURL (fish)",
            Shell::Httpie => "HTTPie",
            Shell::Wget => "wget",
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
        Shell::Fish => curl(request, "curl", fish_quote, " \\\n  "),
        Shell::Httpie => httpie(request),
        Shell::Wget => wget(request),
    }
}

/// fish single quotes: `\'` and `\\` are the only escapes.
fn fish_quote(s: &str) -> String {
    format!("'{}'", s.replace('\\', r"\\").replace('\'', r"\'"))
}

fn httpie(request: &Request) -> String {
    let q = bash_quote;
    let mut notes = Vec::new();
    let mut first = format!("http {} {}", request.method.as_str(), q(&full_url(request)));
    let mut parts = Vec::new();
    match &request.auth {
        Auth::Basic { username, password } => {
            parts.push(format!("-a {}", q(&format!("{username}:{password}"))));
        }
        Auth::Digest { username, password } => {
            parts.push("-A digest".into());
            parts.push(format!("-a {}", q(&format!("{username}:{password}"))));
        }
        _ => {}
    }
    let settings = &request.settings;
    if let Some(ms) = settings.timeout_ms.filter(|ms| *ms > 0) {
        parts.push(format!("--timeout={}", ms as f64 / 1000.0));
    }
    // HTTPie does not follow redirects unless asked.
    if settings.follow_redirects == Some(true)
        || (settings.follow_redirects.is_none() && settings.max_redirects.is_some())
    {
        parts.push("--follow".into());
        if let Some(n) = settings.max_redirects {
            parts.push(format!("--max-redirects={n}"));
        }
    }
    if settings.tls_verify == Some(false) {
        parts.push("--verify=no".into());
    }
    match settings.proxy.as_deref() {
        None => {}
        Some("") => notes.push("# The proxy is off for this request; unset HTTP(S)_PROXY first"),
        Some(url) => {
            parts.push(format!("--proxy={}", q(&format!("http:{url}"))));
            parts.push(format!("--proxy={}", q(&format!("https:{url}"))));
        }
    }
    if let Some(cert) = &settings.client_cert {
        parts.push(format!("--cert={}", q(cert)));
    }
    if let Some(key) = &settings.client_key {
        parts.push(format!("--cert-key={}", q(key)));
    }
    let multipart = multipart_parts(&request.body);
    let form = form_fields(&request.body);
    if multipart.is_some() {
        parts.push("--multipart".into());
    } else if form.is_some() {
        parts.push("--form".into());
    }
    let mut redirect = None;
    if let Some(text) = text_body(&request.body) {
        parts.push(format!("--raw {}", q(&text)));
    } else if let Body::Binary { file } = &request.body {
        redirect = Some(format!("< {}", q(file)));
    }
    for (key, value) in effective_headers(request, false) {
        // `Name:` with nothing after it removes a header; `Name;` sends it empty.
        let item = if value.is_empty() {
            format!("{key};")
        } else {
            format!("{key}:{value}")
        };
        parts.push(q(&item));
    }
    if let Some(fields) = form {
        for field in fields {
            parts.push(q(&format!("{}={}", field.key, field.value)));
        }
    }
    if let Some(fields) = multipart {
        for part in fields {
            if part.file {
                let mime = part
                    .content_type
                    .as_deref()
                    .filter(|m| !m.is_empty())
                    .map(|m| format!(";type={m}"))
                    .unwrap_or_default();
                parts.push(q(&format!("{}@{}{mime}", part.key, part.value)));
            } else {
                parts.push(q(&format!("{}={}", part.key, part.value)));
            }
        }
    }
    parts.extend(redirect);
    for part in parts {
        first.push_str(" \\\n  ");
        first.push_str(&part);
    }
    notes.push("");
    notes.join("\n") + &first
}

fn wget(request: &Request) -> String {
    let q = bash_quote;
    let mut notes: Vec<&str> = Vec::new();
    let mut parts = vec!["wget -qO- --content-on-error".to_owned()];
    let multipart = multipart_parts(&request.body);
    let has_body = !request.body.is_none() && multipart.is_none();
    // --body-data and --body-file only go with --method.
    if request.method != Method::Get || has_body {
        parts.push(format!("--method={}", request.method.as_str()));
    }
    for (key, value) in effective_headers(request, false) {
        parts.push(format!("--header={}", q(&format!("{key}: {value}"))));
    }
    match &request.auth {
        Auth::Basic { username, password } => {
            parts.push(format!("--user={}", q(username)));
            parts.push(format!("--password={}", q(password)));
            // Without it wget waits for a 401 before sending Basic credentials.
            parts.push("--auth-no-challenge".into());
        }
        Auth::Digest { username, password } => {
            parts.push(format!("--user={}", q(username)));
            parts.push(format!("--password={}", q(password)));
        }
        _ => {}
    }
    let settings = &request.settings;
    if let Some(ms) = settings.timeout_ms.filter(|ms| *ms > 0) {
        parts.push(format!("--timeout={}", ms as f64 / 1000.0));
    }
    // wget follows up to 20 redirects unless told otherwise.
    if settings.follow_redirects == Some(false) {
        parts.push("--max-redirect=0".into());
    } else if let Some(n) = settings.max_redirects {
        parts.push(format!("--max-redirect={n}"));
    }
    if settings.tls_verify == Some(false) {
        parts.push("--no-check-certificate".into());
    }
    match settings.proxy.as_deref() {
        None => {}
        Some("") => parts.push("--no-proxy".into()),
        Some(url) => {
            parts.push(format!("-e {}", q(&format!("http_proxy={url}"))));
            parts.push(format!("-e {}", q(&format!("https_proxy={url}"))));
        }
    }
    if let Some(cert) = &settings.client_cert {
        parts.push(format!("--certificate={}", q(cert)));
    }
    if let Some(key) = &settings.client_key {
        parts.push(format!("--private-key={}", q(key)));
    }
    if let Some(text) = text_body(&request.body) {
        parts.push(format!("--body-data={}", q(&text)));
    } else if let Some(fields) = form_fields(&request.body) {
        let data = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(fields.iter().map(|f| (&f.key, &f.value)))
            .finish();
        parts.push(format!("--body-data={}", q(&data)));
    } else if let Body::Binary { file } = &request.body {
        parts.push(format!("--body-file={}", q(file)));
    } else if multipart.is_some() {
        notes.push("# wget cannot send multipart forms; copy as cURL instead");
    }
    parts.push(q(&full_url(request)));
    notes.push("");
    notes.join("\n") + &parts.join(" \\\n  ")
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
    for (key, value) in effective_headers(request, true) {
        parts.push(format!("-H {}", quote(&format!("{key}: {value}"))));
    }
    match &request.auth {
        Auth::Basic { username, password } => {
            parts.push(format!("-u {}", quote(&format!("{username}:{password}"))));
        }
        Auth::Digest { username, password } => {
            parts.push("--digest".into());
            parts.push(format!("-u {}", quote(&format!("{username}:{password}"))));
        }
        // curl 7.75 and later sign for AWS themselves.
        Auth::AwsSigV4(aws) => {
            let provider = format!("aws:amz:{}:{}", aws.region, aws.service);
            parts.push(format!("--aws-sigv4 {}", quote(&provider)));
            let user = format!("{}:{}", aws.access_key, aws.secret_key);
            parts.push(format!("-u {}", quote(&user)));
        }
        _ => {}
    }
    let settings = &request.settings;
    if let Some(ms) = settings.timeout_ms.filter(|ms| *ms > 0) {
        parts.push(format!("-m {}", ms as f64 / 1000.0));
    }
    if settings.follow_redirects == Some(true)
        || (settings.follow_redirects.is_none() && settings.max_redirects.is_some())
    {
        parts.push("-L".into());
    }
    if let Some(n) = settings.max_redirects {
        parts.push(format!("--max-redirs {n}"));
    }
    if settings.tls_verify == Some(false) {
        parts.push("-k".into());
    }
    match settings.proxy.as_deref() {
        None => {}
        Some("") => parts.push(format!("--noproxy {}", quote("*"))),
        Some(url) => parts.push(format!("-x {}", quote(url))),
    }
    if let Some(cert) = &settings.client_cert {
        parts.push(format!("--cert {}", quote(cert)));
    }
    if let Some(key) = &settings.client_key {
        parts.push(format!("--key {}", quote(key)));
    }
    if let Some(text) = text_body(&request.body) {
        parts.push(format!("--data-raw {}", quote(&text)));
    } else if let Some(fields) = form_fields(&request.body) {
        for field in fields {
            parts.push(format!(
                "--data-urlencode {}",
                quote(&format!("{}={}", field.key, field.value))
            ));
        }
    } else if let Some(fields) = multipart_parts(&request.body) {
        for part in fields {
            let mime = part
                .content_type
                .as_deref()
                .filter(|m| !m.is_empty())
                .map(|m| format!(";type={m}"))
                .unwrap_or_default();
            if part.file {
                parts.push(format!(
                    "-F {}",
                    quote(&format!("{}=@{}{mime}", part.key, form_value(&part.value)))
                ));
            } else if mime.is_empty() {
                // --form-string: a value starting with @ or < stays text.
                parts.push(format!(
                    "--form-string {}",
                    quote(&format!("{}={}", part.key, part.value))
                ));
            } else {
                // Quoted, so neither ';' nor a leading @ or < means anything.
                let value = format!(
                    "\"{}\"",
                    part.value.replace('\\', "\\\\").replace('"', "\\\"")
                );
                parts.push(format!(
                    "-F {}",
                    quote(&format!("{}={value}{mime}", part.key))
                ));
            }
        }
    } else if let Body::Binary { file } = &request.body {
        parts.push(format!("--data-binary {}", quote(&format!("@{file}"))));
    }
    parts.join(newline)
}

/// A file path for `-F name=@path`, quoted in curl's own syntax when `;`,
/// `,` or `"` would otherwise be read as part of the field syntax.
fn form_value(path: &str) -> String {
    if path.contains([';', ',', '"']) {
        format!("\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        path.to_owned()
    }
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
    let mut headers = effective_headers(request, false);
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
    let mut extra = String::new();
    match &request.auth {
        Auth::OAuth1(_) | Auth::AwsSigV4(_) => {
            out.push_str("# Signed when copied: copy again for a fresh signature\n");
        }
        // .NET answers the Digest challenge for a credential.
        Auth::Digest { username, password } => {
            if request.url.starts_with("http:") {
                out.push_str(
                    "# Over plain http, PowerShell 7 also needs -AllowUnencryptedAuthentication\n",
                );
            }
            out.push_str(&format!(
                "$credential = [pscredential]::new({}, (ConvertTo-SecureString {} -AsPlainText -Force))\n",
                ps_quote(username),
                ps_quote(password)
            ));
            extra.push_str(" -Credential $credential");
        }
        _ => {}
    }
    let settings = &request.settings;
    if let Some(ms) = settings.timeout_ms.filter(|ms| *ms > 0) {
        extra.push_str(&format!(" -TimeoutSec {}", ms.div_ceil(1000)));
    }
    if settings.follow_redirects == Some(false) {
        out.push_str("# PowerShell treats a stopped redirect as an error\n");
        extra.push_str(" -MaximumRedirection 0");
    } else if let Some(n) = settings.max_redirects {
        extra.push_str(&format!(" -MaximumRedirection {n}"));
    }
    if settings.tls_verify == Some(false) {
        out.push_str("# -SkipCertificateCheck needs PowerShell 7 or later\n");
        extra.push_str(" -SkipCertificateCheck");
    }
    match settings.proxy.as_deref() {
        None => {}
        Some("") => out.push_str(
            "# The proxy is off for this request; Invoke-RestMethod has no flag for that\n",
        ),
        Some(url) => extra.push_str(&format!(" -Proxy {}", ps_quote(url))),
    }
    if let Some(cert) = &settings.client_cert {
        out.push_str("# CreateFromPemFile needs PowerShell 7.1 or later\n");
        let args = match settings
            .client_key
            .as_deref()
            .filter(|k| *k != cert.as_str())
        {
            Some(key) => format!("{}, {}", ps_quote(cert), ps_quote(key)),
            None => ps_quote(cert),
        };
        out.push_str(&format!(
            "$certificate = [System.Security.Cryptography.X509Certificates.X509Certificate2]::CreateFromPemFile({args})\n"
        ));
        extra.push_str(" -Certificate $certificate");
    }
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
    if let Some(parts) = multipart_parts(&request.body) {
        // Invoke-RestMethod's -Form arrived in PowerShell 7.
        out.insert_str(0, "# -Form needs PowerShell 7 or later\n");
        out.push_str("$form = @{\n");
        for part in parts {
            let value = if part.file {
                format!("Get-Item -LiteralPath {}", ps_quote(&part.value))
            } else {
                ps_quote(&part.value)
            };
            out.push_str(&format!("    {} = {value}\n", ps_quote(&part.key)));
        }
        out.push_str("}\n");
        extra.push_str(" -Form $form");
        has_body = false;
    } else if let Body::Binary { file } = &request.body {
        extra.push_str(&format!(" -InFile {}", ps_quote(file)));
        has_body = false;
    } else if let Some(text) = text_body(&request.body) {
        let text = text.as_str();
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
    out.push_str(&extra);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use apix_core::{ApiKeyLocation, Body, FormPart, GraphQl, KeyValue, OAuth1};

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
    fn writes_multipart_binary_and_graphql_bodies() {
        let mut r = Request::new(Method::Post, "https://x.dev/up");
        r.body = Body::Multipart(vec![
            FormPart::text("note", "@not a file"),
            FormPart {
                content_type: Some("application/json".into()),
                ..FormPart::text("meta", "{\"a\";1}")
            },
            FormPart::file("photo", "C:\\pics\\a;b.png"),
            FormPart {
                enabled: false,
                ..FormPart::text("off", "x")
            },
        ]);
        assert_eq!(
            to_shell(&r, Shell::Bash),
            "curl -X POST 'https://x.dev/up' \\\n  \
             --form-string 'note=@not a file' \\\n  \
             -F 'meta=\"{\\\"a\\\";1}\";type=application/json' \\\n  \
             -F 'photo=@\"C:\\\\pics\\\\a;b.png\"'"
        );
        let ps = to_shell(&r, Shell::PowerShell);
        assert!(ps.starts_with("# -Form needs PowerShell 7"), "{ps}");
        assert!(
            ps.contains("    'photo' = Get-Item -LiteralPath 'C:\\pics\\a;b.png'\n"),
            "{ps}"
        );
        assert!(ps.ends_with("-Uri 'https://x.dev/up' -Form $form"), "{ps}");

        r.body = Body::Binary {
            file: "data.bin".into(),
        };
        assert!(to_shell(&r, Shell::Bash).ends_with(
            "-H 'Content-Type: application/octet-stream' \\\n  --data-binary '@data.bin'"
        ));
        assert!(
            to_shell(&r, Shell::PowerShell)
                .ends_with("-ContentType 'application/octet-stream' -InFile 'data.bin'")
        );

        r.body = Body::GraphQl(GraphQl {
            query: "{ me { id } }".into(),
            variables: String::new(),
        });
        assert!(
            to_shell(&r, Shell::Bash)
                .ends_with(r#"--data-raw '{"query":"{ me { id } }","variables":null}'"#),
            "{}",
            to_shell(&r, Shell::Bash)
        );
    }

    #[test]
    fn signing_auth_types_are_signed_when_copied() {
        let mut r = Request::new(Method::Post, "https://api.x.dev/v1/items?a=1");
        r.body = Body::Form(vec![KeyValue::new("q", "x y")]);
        r.auth = Auth::OAuth1(OAuth1 {
            consumer_key: "ck".into(),
            consumer_secret: "cs".into(),
            ..OAuth1::default()
        });
        let bash = to_shell(&r, Shell::Bash);
        assert!(
            bash.contains("-H 'Authorization: OAuth oauth_consumer_key=\"ck\", oauth_nonce=\""),
            "{bash}"
        );
        let ps = to_shell(&r, Shell::PowerShell);
        assert!(ps.starts_with("# Signed when copied"), "{ps}");

        r.auth = Auth::OAuth1(OAuth1 {
            consumer_key: "ck".into(),
            location: ApiKeyLocation::Query,
            ..OAuth1::default()
        });
        assert!(
            to_shell(&r, Shell::Bash)
                .starts_with("curl -X POST 'https://api.x.dev/v1/items?a=1&oauth_consumer_key=ck&")
        );

        // Unresolved variables cannot be signed.
        r.auth = Auth::OAuth1(OAuth1 {
            consumer_key: "{{ck}}".into(),
            ..OAuth1::default()
        });
        assert!(to_shell(&r, Shell::Bash).contains("copy with variables resolved"));

        r.auth = Auth::AwsSigV4(apix_core::AwsSigV4 {
            access_key: "AK".into(),
            secret_key: "SK".into(),
            session_token: String::new(),
            region: "us-east-1".into(),
            service: "execute-api".into(),
        });
        let ps = to_shell(&r, Shell::PowerShell);
        assert!(ps.contains("'X-Amz-Date' = '"), "{ps}");
        assert!(
            ps.contains("'Authorization' = 'AWS4-HMAC-SHA256 Credential=AK/"),
            "{ps}"
        );
        assert!(
            ps.contains("SignedHeaders=content-type;host;x-amz-date"),
            "{ps}"
        );

        r.auth = Auth::OAuth2(apix_core::OAuth2::default());
        assert!(to_shell(&r, Shell::Bash).contains("-H 'Authorization: Bearer <access token>'"));

        r.auth = Auth::Digest {
            username: "u".into(),
            password: "p".into(),
        };
        let ps = to_shell(&r, Shell::PowerShell);
        assert!(
            ps.contains("$credential = [pscredential]::new('u', (ConvertTo-SecureString 'p' -AsPlainText -Force))"),
            "{ps}"
        );
        assert!(ps.ends_with(" -Credential $credential"), "{ps}");
    }

    #[test]
    fn fish_escapes_backslashes_and_quotes() {
        assert_eq!(fish_quote(r"it's C:\tmp"), r"'it\'s C:\\tmp'");
        let fish = to_shell(&post_json(), Shell::Fish);
        assert!(
            fish.ends_with(r#"--data-raw '{"name": "O\'Brien", "path": "C:\\\\tmp"}'"#),
            "{fish}"
        );
    }

    #[test]
    fn httpie_uses_raw_bodies_items_and_flags() {
        let mut r = post_json();
        r.headers.push(KeyValue::new("X-Empty", ""));
        r.settings.follow_redirects = Some(true);
        r.settings.tls_verify = Some(false);
        assert_eq!(
            to_shell(&r, Shell::Httpie),
            "http POST 'https://api.example.com/users?dry+run=yes+%26+no' \\\n  \
             --follow \\\n  \
             --verify=no \\\n  \
             --raw '{\"name\": \"O'\\''Brien\", \"path\": \"C:\\\\tmp\"}' \\\n  \
             'Accept:application/json' \\\n  \
             'X-Empty;' \\\n  \
             'Authorization:Bearer t0k3n' \\\n  \
             'Content-Type:application/json'"
        );

        let mut r = Request::new(Method::Post, "https://x.dev/up");
        r.auth = Auth::Digest {
            username: "u".into(),
            password: "p".into(),
        };
        r.body = Body::Multipart(vec![
            FormPart::text("note", "hi"),
            FormPart::file("photo", "a.png"),
        ]);
        assert_eq!(
            to_shell(&r, Shell::Httpie),
            "http POST 'https://x.dev/up' \\\n  -A digest \\\n  -a 'u:p' \\\n  --multipart \\\n  \
             'note=hi' \\\n  'photo@a.png'"
        );
        r.body = Body::Binary {
            file: "data.bin".into(),
        };
        assert!(to_shell(&r, Shell::Httpie).ends_with("\\\n  < 'data.bin'"));
    }

    #[test]
    fn wget_sends_bodies_with_a_method() {
        let mut r = Request::new(Method::Post, "https://x.dev/form");
        r.body = Body::Form(vec![KeyValue::new("q", "a b&c")]);
        r.auth = Auth::Basic {
            username: "ada".into(),
            password: "pw".into(),
        };
        r.settings.follow_redirects = Some(false);
        assert_eq!(
            to_shell(&r, Shell::Wget),
            "wget -qO- --content-on-error \\\n  \
             --method=POST \\\n  \
             --header='Content-Type: application/x-www-form-urlencoded' \\\n  \
             --user='ada' \\\n  \
             --password='pw' \\\n  \
             --auth-no-challenge \\\n  \
             --max-redirect=0 \\\n  \
             --body-data='q=a+b%26c' \\\n  \
             'https://x.dev/form'"
        );
        let get = Request::new(Method::Get, "https://x.dev/a");
        assert_eq!(
            to_shell(&get, Shell::Wget),
            "wget -qO- --content-on-error \\\n  'https://x.dev/a'"
        );
        r.body = Body::Multipart(vec![FormPart::text("a", "b")]);
        assert!(to_shell(&r, Shell::Wget).starts_with("# wget cannot send multipart"));
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
