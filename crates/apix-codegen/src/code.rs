//! Requests as code in other languages: fetch, axios, Python requests, Go,
//! Rust reqwest, C#, Java, PHP, Swift and Kotlin.
//!
//! Every snippet sends what the Apix engine would send: the same URL and
//! query, the same headers (OAuth 1.0 and AWS signatures computed now), the
//! same body. Request settings a language has no simple switch for are not
//! silently dropped: they are listed in a "Not in this snippet" comment.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use apix_core::{Auth, Body, FormPart, KeyValue, Method, Request, RequestSettings};

use crate::{effective_headers, form_fields, full_url, multipart_parts, text_body};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// `fetch`, in browsers and Node.js 18+.
    Fetch,
    /// axios on Node.js.
    Axios,
    /// Python with `requests`.
    Python,
    /// Go with `net/http`.
    Go,
    /// Rust with `reqwest` and tokio.
    Rust,
    /// C# with `HttpClient` (.NET 6+ top-level statements).
    CSharp,
    /// Java 11+ with `java.net.http.HttpClient`.
    Java,
    /// PHP with the cURL extension.
    Php,
    /// Swift with `URLSession` (async/await).
    Swift,
    /// Kotlin with OkHttp.
    Kotlin,
}

impl Lang {
    pub const ALL: [Lang; 10] = [
        Lang::Fetch,
        Lang::Axios,
        Lang::Python,
        Lang::Go,
        Lang::Rust,
        Lang::CSharp,
        Lang::Java,
        Lang::Php,
        Lang::Swift,
        Lang::Kotlin,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Lang::Fetch => "JavaScript (fetch)",
            Lang::Axios => "Node.js (axios)",
            Lang::Python => "Python (requests)",
            Lang::Go => "Go (net/http)",
            Lang::Rust => "Rust (reqwest)",
            Lang::CSharp => "C# (HttpClient)",
            Lang::Java => "Java (HttpClient)",
            Lang::Php => "PHP (cURL)",
            Lang::Swift => "Swift (URLSession)",
            Lang::Kotlin => "Kotlin (OkHttp)",
        }
    }
}

/// Writes `request` as a program in `lang`. Pass a resolved request to get
/// concrete values, or the saved one to keep `{{variables}}`.
pub fn to_code(request: &Request, lang: Lang) -> String {
    let parts = Parts::new(request);
    match lang {
        Lang::Fetch => fetch(&parts),
        Lang::Axios => axios(&parts),
        Lang::Python => python(&parts),
        Lang::Go => go(&parts),
        Lang::Rust => rust(&parts),
        Lang::CSharp => csharp(&parts),
        Lang::Java => java(&parts),
        Lang::Php => php(&parts),
        Lang::Swift => swift(&parts),
        Lang::Kotlin => kotlin(&parts),
    }
}

// ---------------------------------------------------------------------------
// What every language needs, worked out once
// ---------------------------------------------------------------------------

enum Payload<'a> {
    None,
    Text(String),
    Form(Vec<&'a KeyValue>),
    Multipart(Vec<&'a FormPart>),
    File(&'a str),
}

/// Basic or Digest credentials, which each language sends its own way.
struct Login<'a> {
    user: &'a str,
    password: &'a str,
    digest: bool,
}

impl Login<'_> {
    fn pair(&self) -> String {
        format!("{}:{}", self.user, self.password)
    }
}

struct Parts<'a> {
    method: Method,
    url: String,
    /// As the engine sends them, Basic and Digest aside.
    headers: Vec<(String, String)>,
    login: Option<Login<'a>>,
    payload: Payload<'a>,
    settings: &'a RequestSettings,
}

/// Which request settings a generator writes itself; the rest become notes.
#[derive(Default, Clone, Copy)]
struct Handles {
    redirects_off: bool,
    max_redirects: bool,
    timeout: bool,
    insecure: bool,
    proxy_url: bool,
    proxy_off: bool,
    cert: bool,
    digest: bool,
}

impl<'a> Parts<'a> {
    fn new(request: &'a Request) -> Self {
        let payload = if let Some(text) = text_body(&request.body) {
            Payload::Text(text)
        } else if let Some(fields) = form_fields(&request.body) {
            Payload::Form(fields)
        } else if let Some(parts) = multipart_parts(&request.body) {
            Payload::Multipart(parts)
        } else if let Body::Binary { file } = &request.body {
            Payload::File(file)
        } else {
            Payload::None
        };
        let mut headers = effective_headers(request, false);
        if matches!(payload, Payload::Multipart(_)) {
            // Every multipart builder writes its own boundary.
            headers.retain(|(k, v)| {
                !(k.eq_ignore_ascii_case("content-type")
                    && v.to_ascii_lowercase().starts_with("multipart/"))
            });
        }
        let login = match &request.auth {
            Auth::Basic { username, password } => Some(Login {
                user: username,
                password,
                digest: false,
            }),
            Auth::Digest { username, password } => Some(Login {
                user: username,
                password,
                digest: true,
            }),
            _ => None,
        };
        Self {
            method: request.method,
            url: full_url(request),
            headers,
            login,
            payload,
            settings: &request.settings,
        }
    }

    fn content_type(&self) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.as_str())
    }

    /// Headers other than Content-Type, for languages that put it on the body.
    fn other_headers(&self) -> impl Iterator<Item = &(String, String)> {
        self.headers
            .iter()
            .filter(|(k, _)| !k.eq_ignore_ascii_case("content-type"))
    }

    fn basic(&self) -> Option<&Login<'a>> {
        self.login.as_ref().filter(|l| !l.digest)
    }

    fn digest(&self) -> Option<&Login<'a>> {
        self.login.as_ref().filter(|l| l.digest)
    }

    fn timeout_ms(&self) -> Option<u64> {
        self.settings.timeout_ms.filter(|ms| *ms > 0)
    }

    fn redirects_off(&self) -> bool {
        self.settings.follow_redirects == Some(false)
    }

    /// A redirect cap, when redirects are followed and one is set.
    fn max_redirects(&self) -> Option<u32> {
        self.settings
            .max_redirects
            .filter(|_| !self.redirects_off())
    }

    fn insecure(&self) -> bool {
        self.settings.tls_verify == Some(false)
    }

    fn proxy_off(&self) -> bool {
        self.settings.proxy.as_deref() == Some("")
    }

    fn proxy_url(&self) -> Option<&str> {
        self.settings.proxy.as_deref().filter(|p| !p.is_empty())
    }

    fn cert(&self) -> Option<&str> {
        self.settings
            .client_cert
            .as_deref()
            .filter(|c| !c.is_empty())
    }

    /// The key file, or the certificate file when it holds the key too.
    fn key(&self) -> Option<&str> {
        self.settings
            .client_key
            .as_deref()
            .filter(|k| !k.is_empty())
            .or(self.cert())
    }

    fn has_files(&self) -> bool {
        match &self.payload {
            Payload::File(_) => true,
            Payload::Multipart(parts) => parts.iter().any(|p| p.file),
            _ => false,
        }
    }

    /// One comment line per setting the snippet does not express.
    fn notes(&self, handles: Handles, comment: &str) -> String {
        let mut notes = Vec::new();
        if self.redirects_off() && !handles.redirects_off {
            notes.push("redirects are not followed".to_owned());
        }
        if let Some(n) = self.max_redirects()
            && !handles.max_redirects
        {
            notes.push(format!("at most {n} redirects"));
        }
        if let Some(ms) = self.timeout_ms()
            && !handles.timeout
        {
            notes.push(format!("timeout {} s", seconds(ms)));
        }
        if self.insecure() && !handles.insecure {
            notes.push("no TLS certificate check".to_owned());
        }
        if self.proxy_off() && !handles.proxy_off {
            notes.push("no proxy".to_owned());
        }
        if let Some(url) = self.proxy_url()
            && !handles.proxy_url
        {
            notes.push(format!("proxy {url}"));
        }
        if let Some(cert) = self.cert()
            && !handles.cert
        {
            notes.push(format!("client certificate {cert}"));
        }
        if let Some(login) = self.digest()
            && !handles.digest
        {
            notes.push(format!(
                "Digest auth as {} (needs a library here; sent without credentials)",
                login.user
            ));
        }
        notes
            .iter()
            .map(|n| format!("{comment} Not in this snippet: {n}\n"))
            .collect()
    }
}

/// `5000` → `5`, `2500` → `2.5`.
fn seconds(ms: u64) -> String {
    if ms.is_multiple_of(1000) {
        (ms / 1000).to_string()
    } else {
        format!("{}", ms as f64 / 1000.0)
    }
}

/// `a=1&b=2`, encoded the way the engine encodes forms.
fn urlencoded(fields: &[&KeyValue]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields.iter().map(|f| (&f.key, &f.value)))
        .finish()
}

/// The last component of a path, for multipart file names.
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|n| !n.is_empty())
        .unwrap_or("file")
}

fn part_type(part: &FormPart) -> Option<&str> {
    part.content_type
        .as_deref()
        .filter(|c| !c.trim().is_empty())
}

/// `(host, port)` of a proxy URL, for languages that take them apart.
fn proxy_host_port(url: &str) -> Option<(String, u16)> {
    let parsed = url::Url::parse(url).ok()?;
    Some((
        parsed.host_str()?.to_owned(),
        parsed.port_or_known_default()?,
    ))
}

/// A multipart body written out by hand, for languages without a builder.
enum Segment<'a> {
    Text(String),
    File(&'a str),
}

const BOUNDARY: &str = "----ApixFormBoundary7MA4YWxkTrZu0gW";

fn multipart_segments<'a>(parts: &[&'a FormPart]) -> Vec<Segment<'a>> {
    let mut segments = Vec::new();
    let mut text = String::new();
    for part in parts {
        let _ = write!(
            text,
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{}\"",
            part.key.replace('"', "%22")
        );
        if part.file {
            let _ = write!(
                text,
                "; filename=\"{}\"",
                file_name(&part.value).replace('"', "%22")
            );
        }
        let content_type = part_type(part).or(part.file.then_some("application/octet-stream"));
        if let Some(ct) = content_type {
            let _ = write!(text, "\r\nContent-Type: {ct}");
        }
        text.push_str("\r\n\r\n");
        if part.file {
            segments.push(Segment::Text(std::mem::take(&mut text)));
            segments.push(Segment::File(&part.value));
        } else {
            text.push_str(&part.value);
        }
        text.push_str("\r\n");
    }
    let _ = write!(text, "--{BOUNDARY}--\r\n");
    segments.push(Segment::Text(text));
    segments
}

// ---------------------------------------------------------------------------
// String literals
// ---------------------------------------------------------------------------

/// `"…"` with C-style escapes. `braces` writes `\u{1b}` (Swift, Rust)
/// instead of `\u001b`; `dollar` escapes `$` (Kotlin).
fn c_quote(s: &str, braces: bool, dollar: bool) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '$' if dollar => out.push_str("\\$"),
            c if (c as u32) < 0x20 => {
                if braces {
                    let _ = write!(out, "\\u{{{:x}}}", c as u32);
                } else {
                    let _ = write!(out, "\\u{:04x}", c as u32);
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// JavaScript, Go, Java, C# and Python read these the same way.
fn dq(s: &str) -> String {
    c_quote(s, false, false)
}

/// Long text as one quoted literal per line joined with `+`, so a JSON body
/// keeps its shape.
fn joined(text: &str, quote: impl Fn(&str) -> String, indent: &str) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    if lines.len() <= 1 {
        return quote(text);
    }
    lines
        .iter()
        .map(|l| quote(l))
        .collect::<Vec<_>>()
        .join(&format!(" +\n{indent}"))
}

fn js_str(s: &str) -> String {
    if s.contains('\n') {
        let escaped = s
            .replace('\\', "\\\\")
            .replace('`', "\\`")
            .replace("${", "\\${")
            .replace('\r', "\\r");
        format!("`{escaped}`")
    } else {
        dq(s)
    }
}

fn py_str(s: &str, indent: &str) -> String {
    let lines: Vec<&str> = s.split_inclusive('\n').collect();
    if lines.len() <= 1 {
        return dq(s);
    }
    let inner = format!("{indent}    ");
    let body: Vec<String> = lines.iter().map(|l| dq(l)).collect();
    format!("(\n{inner}{}\n{indent})", body.join(&format!("\n{inner}")))
}

fn go_str(s: &str) -> String {
    if s.contains('\n') && !s.contains('`') && !s.contains('\r') {
        format!("`{s}`")
    } else {
        dq(s)
    }
}

fn rust_str(s: &str) -> String {
    if !s.contains('\n') && !s.contains('"') && !s.contains('\\') {
        return c_quote(s, true, false);
    }
    let mut hashes = 1;
    while s.contains(&format!("\"{}", "#".repeat(hashes))) {
        hashes += 1;
    }
    let h = "#".repeat(hashes);
    format!("r{h}\"{s}\"{h}")
}

fn cs_str(s: &str) -> String {
    if s.contains('\n') {
        format!("@\"{}\"", s.replace('"', "\"\""))
    } else {
        dq(s)
    }
}

fn php_str(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn swift_str(s: &str) -> String {
    c_quote(s, true, false)
}

fn kt_str(s: &str) -> String {
    c_quote(s, false, true)
}

// ---------------------------------------------------------------------------
// JavaScript
// ---------------------------------------------------------------------------

/// `{ "k": v, … }`, or a list of pairs when a header name repeats.
fn js_headers(entries: &[(String, String)], indent: &str) -> String {
    let mut seen = BTreeSet::new();
    let repeats = entries
        .iter()
        .any(|(k, _)| !seen.insert(k.to_ascii_lowercase()));
    let inner = format!("{indent}  ");
    let mut out = String::from(if repeats { "[\n" } else { "{\n" });
    for (k, v) in entries {
        if repeats {
            let _ = writeln!(out, "{inner}[{}, {v}],", dq(k));
        } else {
            let _ = writeln!(out, "{inner}{}: {v},", dq(k));
        }
    }
    out.push_str(indent);
    out.push(if repeats { ']' } else { '}' });
    out
}

/// Statements that build `form`, and the body expression.
fn js_body(parts: &Parts) -> (String, Option<String>) {
    match &parts.payload {
        Payload::None => (String::new(), None),
        Payload::Text(text) => (String::new(), Some(js_str(text))),
        Payload::Form(fields) => {
            let pairs: Vec<String> = fields
                .iter()
                .map(|f| format!("[{}, {}]", dq(&f.key), dq(&f.value)))
                .collect();
            (
                String::new(),
                Some(format!("new URLSearchParams([{}])", pairs.join(", "))),
            )
        }
        Payload::Multipart(fields) => {
            let mut out = String::from("const form = new FormData();\n");
            for p in fields {
                let typed = part_type(p)
                    .map(|t| format!(", {{ type: {} }}", dq(t)))
                    .unwrap_or_default();
                if p.file {
                    let _ = writeln!(
                        out,
                        "form.append({}, new Blob([readFileSync({})]{typed}), {});",
                        dq(&p.key),
                        dq(&p.value),
                        dq(file_name(&p.value))
                    );
                } else if typed.is_empty() {
                    let _ = writeln!(out, "form.append({}, {});", dq(&p.key), dq(&p.value));
                } else {
                    let _ = writeln!(
                        out,
                        "form.append({}, new Blob([{}]{typed}));",
                        dq(&p.key),
                        dq(&p.value)
                    );
                }
            }
            out.push('\n');
            (out, Some("form".into()))
        }
        Payload::File(path) => (String::new(), Some(format!("readFileSync({})", dq(path)))),
    }
}

fn js_header_entries(parts: &Parts, with_basic: bool) -> Vec<(String, String)> {
    let mut entries: Vec<(String, String)> = parts
        .headers
        .iter()
        .map(|(k, v)| (k.clone(), dq(v)))
        .collect();
    if with_basic && let Some(login) = parts.basic() {
        entries.push((
            "Authorization".into(),
            format!("\"Basic \" + btoa({})", dq(&login.pair())),
        ));
    }
    entries
}

fn fetch(parts: &Parts) -> String {
    let mut out = String::new();
    if parts.has_files() {
        out.push_str("// Node.js 18+, saved as an .mjs file.\n");
        out.push_str("import { readFileSync } from \"node:fs\";\n\n");
    } else {
        out.push_str("// Browsers and Node.js 18+ (as an .mjs file).\n");
    }
    out.push_str(&parts.notes(
        Handles {
            redirects_off: true,
            timeout: true,
            ..Handles::default()
        },
        "//",
    ));
    let (setup, body) = js_body(parts);
    out.push_str(&setup);
    let _ = writeln!(out, "const response = await fetch({}, {{", dq(&parts.url));
    if parts.method != Method::Get {
        let _ = writeln!(out, "  method: {},", dq(parts.method.as_str()));
    }
    let entries = js_header_entries(parts, true);
    if !entries.is_empty() {
        let _ = writeln!(out, "  headers: {},", js_headers(&entries, "  "));
    }
    if let Some(body) = body {
        let _ = writeln!(out, "  body: {body},");
    }
    if parts.redirects_off() {
        out.push_str("  redirect: \"manual\",\n");
    }
    if let Some(ms) = parts.timeout_ms() {
        let _ = writeln!(out, "  signal: AbortSignal.timeout({ms}),");
    }
    out.push_str("});\n");
    out.push_str("console.log(response.status);\n");
    out.push_str("console.log(await response.text());\n");
    out
}

fn axios(parts: &Parts) -> String {
    let agent = parts.insecure() || parts.cert().is_some();
    let mut out = String::from("// npm install axios; run as an .mjs file.\n");
    out.push_str("import axios from \"axios\";\n");
    if parts.has_files() || parts.cert().is_some() {
        out.push_str("import { readFileSync } from \"node:fs\";\n");
    }
    if agent {
        out.push_str("import https from \"node:https\";\n");
    }
    out.push('\n');
    let proxy_parts = parts.proxy_url().and_then(|url| {
        let scheme = url::Url::parse(url).ok()?.scheme().to_owned();
        proxy_host_port(url).map(|(host, port)| (scheme, host, port))
    });
    out.push_str(&parts.notes(
        Handles {
            redirects_off: true,
            max_redirects: true,
            timeout: true,
            insecure: true,
            proxy_off: true,
            proxy_url: proxy_parts.is_some(),
            cert: true,
            ..Handles::default()
        },
        "//",
    ));
    let (setup, body) = js_body(parts);
    out.push_str(&setup);
    out.push_str("const response = await axios({\n");
    let _ = writeln!(
        out,
        "  method: {},",
        dq(&parts.method.as_str().to_ascii_lowercase())
    );
    let _ = writeln!(out, "  url: {},", dq(&parts.url));
    let entries = js_header_entries(parts, false);
    if !entries.is_empty() {
        let _ = writeln!(out, "  headers: {},", js_headers(&entries, "  "));
    }
    if let Some(login) = parts.basic() {
        let _ = writeln!(
            out,
            "  auth: {{ username: {}, password: {} }},",
            dq(login.user),
            dq(login.password)
        );
    }
    if let Some(body) = body {
        let _ = writeln!(out, "  data: {body},");
    }
    if let Some(ms) = parts.timeout_ms() {
        let _ = writeln!(out, "  timeout: {ms},");
    }
    if parts.redirects_off() {
        out.push_str("  maxRedirects: 0,\n");
    } else if let Some(n) = parts.max_redirects() {
        let _ = writeln!(out, "  maxRedirects: {n},");
    }
    if agent {
        let mut options = Vec::new();
        if parts.insecure() {
            options.push("rejectUnauthorized: false".to_owned());
        }
        if let (Some(cert), Some(key)) = (parts.cert(), parts.key()) {
            options.push(format!("cert: readFileSync({})", dq(cert)));
            options.push(format!("key: readFileSync({})", dq(key)));
        }
        let _ = writeln!(
            out,
            "  httpsAgent: new https.Agent({{ {} }}),",
            options.join(", ")
        );
    }
    if parts.proxy_off() {
        out.push_str("  proxy: false,\n");
    } else if let Some((scheme, host, port)) = proxy_parts {
        let _ = writeln!(
            out,
            "  proxy: {{ protocol: {}, host: {}, port: {port} }},",
            dq(&scheme),
            dq(&host)
        );
    }
    out.push_str("  // Like Apix, show every status instead of throwing on 4xx and 5xx.\n");
    out.push_str("  validateStatus: () => true,\n");
    out.push_str("});\n");
    out.push_str("console.log(response.status);\n");
    out.push_str("console.log(response.data);\n");
    out
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

fn python(parts: &Parts) -> String {
    let mut out = String::from("import requests\n");
    if parts.digest().is_some() {
        out.push_str("from requests.auth import HTTPDigestAuth\n");
    }
    out.push('\n');
    out.push_str(&parts.notes(
        Handles {
            redirects_off: true,
            timeout: true,
            insecure: true,
            proxy_url: true,
            cert: true,
            digest: true,
            ..Handles::default()
        },
        "#",
    ));
    let mut args = vec![dq(parts.method.as_str()), dq(&parts.url)];
    if !parts.headers.is_empty() {
        let mut h = String::from("headers={\n");
        for (k, v) in &parts.headers {
            let _ = writeln!(h, "        {}: {},", dq(k), dq(v));
        }
        h.push_str("    }");
        args.push(h);
    }
    if let Some(login) = &parts.login {
        let pair = format!("{}, {}", dq(login.user), dq(login.password));
        args.push(if login.digest {
            format!("auth=HTTPDigestAuth({pair})")
        } else {
            format!("auth=({pair})")
        });
    }
    match &parts.payload {
        Payload::None => {}
        Payload::Text(text) => args.push(format!("data={}.encode()", py_str(text, "    "))),
        Payload::Form(fields) => {
            let pairs: Vec<String> = fields
                .iter()
                .map(|f| format!("({}, {})", dq(&f.key), dq(&f.value)))
                .collect();
            args.push(format!("data=[{}]", pairs.join(", ")));
        }
        Payload::Multipart(fields) => {
            let mut f = String::from("files=[\n");
            for p in fields {
                let typed = part_type(p)
                    .map(|t| format!(", {}", dq(t)))
                    .unwrap_or_default();
                let inner = if p.file {
                    format!(
                        "{}, open({}, \"rb\"){typed}",
                        dq(file_name(&p.value)),
                        dq(&p.value)
                    )
                } else {
                    format!("None, {}{typed}", dq(&p.value))
                };
                let _ = writeln!(f, "        ({}, ({inner})),", dq(&p.key));
            }
            f.push_str("    ]");
            args.push(f);
        }
        Payload::File(path) => args.push(format!("data=open({}, \"rb\")", dq(path))),
    }
    if let Some(ms) = parts.timeout_ms() {
        args.push(format!("timeout={}", seconds(ms)));
    }
    if parts.redirects_off() {
        args.push("allow_redirects=False".into());
    }
    if parts.insecure() {
        args.push("verify=False".into());
    }
    if let Some(url) = parts.proxy_url() {
        args.push(format!(
            "proxies={{\"http\": {0}, \"https\": {0}}}",
            dq(url)
        ));
    }
    if let (Some(cert), Some(key)) = (parts.cert(), parts.key()) {
        args.push(if cert == key {
            format!("cert={}", dq(cert))
        } else {
            format!("cert=({}, {})", dq(cert), dq(key))
        });
    }
    out.push_str("response = requests.request(\n");
    for arg in args {
        let _ = writeln!(out, "    {arg},");
    }
    out.push_str(")\n");
    out.push_str("print(response.status_code)\n");
    out.push_str("print(response.text)\n");
    out
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

const GO_CHECK: &str = "\tif err != nil {\n\t\tpanic(err)\n\t}\n";
const GO_CHECK_INNER: &str = "\t\tif err != nil {\n\t\t\tpanic(err)\n\t\t}\n";

fn go(parts: &Parts) -> String {
    let mut imports: BTreeSet<&str> = ["fmt", "io", "net/http"].into();
    let mut code = String::new();

    let body_arg = match &parts.payload {
        Payload::None => "nil".to_owned(),
        Payload::Text(text) => {
            imports.insert("strings");
            let _ = writeln!(code, "\tbody := strings.NewReader({})", go_str(text));
            "body".into()
        }
        Payload::Form(fields) => {
            imports.insert("strings");
            let _ = writeln!(
                code,
                "\tbody := strings.NewReader({})",
                dq(&urlencoded(fields))
            );
            "body".into()
        }
        Payload::File(path) => {
            imports.insert("os");
            let _ = writeln!(code, "\tbody, err := os.Open({})", dq(path));
            code.push_str(GO_CHECK);
            code.push_str("\tdefer body.Close()\n");
            "body".into()
        }
        Payload::Multipart(fields) => {
            imports.insert("bytes");
            imports.insert("mime/multipart");
            code.push_str("\tvar body bytes.Buffer\n");
            code.push_str("\tform := multipart.NewWriter(&body)\n");
            for p in fields {
                code.push_str("\t{\n");
                if let Some(ct) = part_type(p) {
                    imports.insert("net/textproto");
                    let disposition = if p.file {
                        format!(
                            "form-data; name=\"{}\"; filename=\"{}\"",
                            p.key,
                            file_name(&p.value)
                        )
                    } else {
                        format!("form-data; name=\"{}\"", p.key)
                    };
                    let _ = writeln!(
                        code,
                        "\t\tpart, err := form.CreatePart(textproto.MIMEHeader{{\n\t\t\t\"Content-Disposition\": {{{}}},\n\t\t\t\"Content-Type\":        {{{}}},\n\t\t}})",
                        dq(&disposition),
                        dq(ct)
                    );
                } else if p.file {
                    let _ = writeln!(
                        code,
                        "\t\tpart, err := form.CreateFormFile({}, {})",
                        dq(&p.key),
                        dq(file_name(&p.value))
                    );
                } else {
                    let _ = writeln!(
                        code,
                        "\t\tpart, err := form.CreateFormField({})",
                        dq(&p.key)
                    );
                }
                code.push_str(GO_CHECK_INNER);
                if p.file {
                    imports.insert("os");
                    let _ = writeln!(code, "\t\tfile, err := os.Open({})", dq(&p.value));
                    code.push_str(GO_CHECK_INNER);
                    code.push_str("\t\tif _, err := io.Copy(part, file); err != nil {\n\t\t\tpanic(err)\n\t\t}\n");
                    code.push_str("\t\tfile.Close()\n");
                } else {
                    let _ = writeln!(
                        code,
                        "\t\tif _, err := io.WriteString(part, {}); err != nil {{\n\t\t\tpanic(err)\n\t\t}}",
                        dq(&p.value)
                    );
                }
                code.push_str("\t}\n");
            }
            code.push_str("\tform.Close()\n");
            "&body".into()
        }
    };

    let _ = writeln!(
        code,
        "\treq, err := http.NewRequest({}, {}, {body_arg})",
        dq(parts.method.as_str()),
        dq(&parts.url)
    );
    code.push_str(GO_CHECK);
    for (k, v) in &parts.headers {
        let _ = writeln!(code, "\treq.Header.Add({}, {})", dq(k), dq(v));
    }
    if matches!(parts.payload, Payload::Multipart(_)) {
        code.push_str("\treq.Header.Set(\"Content-Type\", form.FormDataContentType())\n");
    }
    if let Some(login) = parts.basic() {
        let _ = writeln!(
            code,
            "\treq.SetBasicAuth({}, {})",
            dq(login.user),
            dq(login.password)
        );
    }

    // The client: timeout, redirects and the transport (TLS, proxy).
    let transport = parts.insecure() || parts.cert().is_some() || parts.settings.proxy.is_some();
    if let Some(url) = parts.proxy_url() {
        imports.insert("net/url");
        let _ = writeln!(code, "\tproxyURL, err := url.Parse({})", dq(url));
        code.push_str(GO_CHECK);
    }
    if let (Some(cert), Some(key)) = (parts.cert(), parts.key()) {
        imports.insert("crypto/tls");
        let _ = writeln!(
            code,
            "\tcert, err := tls.LoadX509KeyPair({}, {})",
            dq(cert),
            dq(key)
        );
        code.push_str(GO_CHECK);
    }
    let mut fields = String::new();
    if let Some(ms) = parts.timeout_ms() {
        imports.insert("time");
        let _ = writeln!(fields, "\t\tTimeout: {ms} * time.Millisecond,");
    }
    if parts.redirects_off() {
        fields.push_str("\t\tCheckRedirect: func(req *http.Request, via []*http.Request) error {\n\t\t\treturn http.ErrUseLastResponse\n\t\t},\n");
    } else if let Some(n) = parts.max_redirects() {
        let _ = writeln!(
            fields,
            "\t\tCheckRedirect: func(req *http.Request, via []*http.Request) error {{\n\t\t\tif len(via) >= {n} {{\n\t\t\t\treturn fmt.Errorf(\"stopped after {n} redirects\")\n\t\t\t}}\n\t\t\treturn nil\n\t\t}},"
        );
    }
    if transport {
        fields.push_str("\t\tTransport: &http.Transport{\n");
        match parts.settings.proxy.as_deref() {
            None => fields.push_str("\t\t\tProxy: http.ProxyFromEnvironment,\n"),
            Some("") => {}
            Some(_) => fields.push_str("\t\t\tProxy: http.ProxyURL(proxyURL),\n"),
        }
        if parts.insecure() || parts.cert().is_some() {
            imports.insert("crypto/tls");
            fields.push_str("\t\t\tTLSClientConfig: &tls.Config{\n");
            if parts.insecure() {
                fields.push_str("\t\t\t\tInsecureSkipVerify: true,\n");
            }
            if parts.cert().is_some() {
                fields.push_str("\t\t\t\tCertificates: []tls.Certificate{cert},\n");
            }
            fields.push_str("\t\t\t},\n");
        }
        fields.push_str("\t\t},\n");
    }
    if fields.is_empty() {
        code.push_str("\tclient := &http.Client{}\n");
    } else {
        let _ = write!(code, "\tclient := &http.Client{{\n{fields}\t}}\n");
    }
    code.push_str("\tresp, err := client.Do(req)\n");
    code.push_str(GO_CHECK);
    code.push_str("\tdefer resp.Body.Close()\n");
    code.push_str("\tdata, err := io.ReadAll(resp.Body)\n");
    code.push_str(GO_CHECK);
    code.push_str("\tfmt.Println(resp.Status)\n");
    code.push_str("\tfmt.Println(string(data))\n");

    let mut out = parts.notes(
        Handles {
            redirects_off: true,
            max_redirects: true,
            timeout: true,
            insecure: true,
            proxy_off: true,
            proxy_url: true,
            cert: true,
            ..Handles::default()
        },
        "//",
    );
    out.push_str("package main\n\nimport (\n");
    for import in imports {
        let _ = writeln!(out, "\t\"{import}\"");
    }
    out.push_str(")\n\nfunc main() {\n");
    out.push_str(&code);
    out.push_str("}\n");
    out
}

// ---------------------------------------------------------------------------
// Rust
// ---------------------------------------------------------------------------

fn rust(parts: &Parts) -> String {
    let multipart = matches!(parts.payload, Payload::Multipart(_));
    let mut features = Vec::new();
    if multipart {
        features.push("\"multipart\"");
    }
    if parts.cert().is_some() {
        // Identity::from_pem is a rustls feature.
        features.push("\"rustls-tls\"");
    }
    let features = if features.is_empty() {
        String::new()
    } else {
        format!(", features = [{}]", features.join(", "))
    };
    let mut out = format!(
        "// Cargo.toml: reqwest = {{ version = \"0.12\"{features} }}, tokio = {{ version = \"1\", features = [\"full\"] }}\n"
    );
    out.push_str(&parts.notes(
        Handles {
            redirects_off: true,
            max_redirects: true,
            timeout: true,
            insecure: true,
            proxy_off: true,
            proxy_url: true,
            cert: true,
            ..Handles::default()
        },
        "//",
    ));
    out.push_str("#[tokio::main]\nasync fn main() -> Result<(), Box<dyn std::error::Error>> {\n");
    out.push_str("    let client = reqwest::Client::builder()\n");
    if let Some(ms) = parts.timeout_ms() {
        let _ = writeln!(
            out,
            "        .timeout(std::time::Duration::from_millis({ms}))"
        );
    }
    if parts.redirects_off() {
        out.push_str("        .redirect(reqwest::redirect::Policy::none())\n");
    } else if let Some(n) = parts.max_redirects() {
        let _ = writeln!(
            out,
            "        .redirect(reqwest::redirect::Policy::limited({n}))"
        );
    }
    if parts.insecure() {
        out.push_str("        .danger_accept_invalid_certs(true)\n");
    }
    if parts.proxy_off() {
        out.push_str("        .no_proxy()\n");
    } else if let Some(url) = parts.proxy_url() {
        let _ = writeln!(
            out,
            "        .proxy(reqwest::Proxy::all({})?)",
            rust_str(url)
        );
    }
    if let (Some(cert), Some(key)) = (parts.cert(), parts.key()) {
        let pem = if cert == key {
            format!("&std::fs::read({})?", rust_str(cert))
        } else {
            format!(
                "&[std::fs::read({})?, std::fs::read({})?].concat()",
                rust_str(cert),
                rust_str(key)
            )
        };
        let _ = writeln!(
            out,
            "        .identity(reqwest::Identity::from_pem({pem})?)"
        );
    }
    out.push_str("        .build()?;\n");

    if let Payload::Multipart(fields) = &parts.payload {
        out.push_str("    let form = reqwest::multipart::Form::new()");
        for p in fields {
            let typed = part_type(p)
                .map(|t| format!(".mime_str({})?", rust_str(t)))
                .unwrap_or_default();
            if p.file {
                let _ = write!(
                    out,
                    "\n        .part(\n            {},\n            reqwest::multipart::Part::bytes(std::fs::read({})?)\n                .file_name({}){typed},\n        )",
                    rust_str(&p.key),
                    rust_str(&p.value),
                    rust_str(file_name(&p.value))
                );
            } else if typed.is_empty() {
                let _ = write!(
                    out,
                    "\n        .text({}, {})",
                    rust_str(&p.key),
                    rust_str(&p.value)
                );
            } else {
                let _ = write!(
                    out,
                    "\n        .part({}, reqwest::multipart::Part::text({}){typed})",
                    rust_str(&p.key),
                    rust_str(&p.value)
                );
            }
        }
        out.push_str(";\n");
    }

    out.push_str("    let response = client\n");
    let _ = writeln!(
        out,
        "        .request(reqwest::Method::{}, {})",
        parts.method.as_str(),
        rust_str(&parts.url)
    );
    for (k, v) in &parts.headers {
        let _ = writeln!(out, "        .header({}, {})", rust_str(k), rust_str(v));
    }
    if let Some(login) = parts.basic() {
        let _ = writeln!(
            out,
            "        .basic_auth({}, Some({}))",
            rust_str(login.user),
            rust_str(login.password)
        );
    }
    match &parts.payload {
        Payload::None => {}
        Payload::Text(text) => {
            let _ = writeln!(out, "        .body({})", rust_str(text));
        }
        Payload::Form(fields) => {
            let pairs: Vec<String> = fields
                .iter()
                .map(|f| format!("({}, {})", rust_str(&f.key), rust_str(&f.value)))
                .collect();
            let _ = writeln!(out, "        .form(&[{}])", pairs.join(", "));
        }
        Payload::Multipart(_) => out.push_str("        .multipart(form)\n"),
        Payload::File(path) => {
            let _ = writeln!(out, "        .body(std::fs::read({})?)", rust_str(path));
        }
    }
    out.push_str("        .send()\n        .await?;\n");
    out.push_str("    println!(\"{}\", response.status());\n");
    out.push_str("    println!(\"{}\", response.text().await?);\n");
    out.push_str("    Ok(())\n}\n");
    out
}

// ---------------------------------------------------------------------------
// C#
// ---------------------------------------------------------------------------

fn csharp(parts: &Parts) -> String {
    let mut usings: BTreeSet<&str> = BTreeSet::new();
    let mut code = String::new();

    let mut handler = Vec::new();
    if parts.redirects_off() {
        handler.push("AllowAutoRedirect = false".to_owned());
    } else if let Some(n) = parts.max_redirects() {
        handler.push(format!("MaxAutomaticRedirections = {n}"));
    }
    if parts.insecure() {
        handler.push(
            "ServerCertificateCustomValidationCallback = HttpClientHandler.DangerousAcceptAnyServerCertificateValidator"
                .into(),
        );
    }
    if parts.proxy_off() {
        handler.push("UseProxy = false".into());
    } else if let Some(url) = parts.proxy_url() {
        usings.insert("System.Net");
        handler.push(format!("Proxy = new WebProxy({})", dq(url)));
    }
    if let Some(login) = parts.digest() {
        usings.insert("System.Net");
        handler.push(format!(
            "Credentials = new NetworkCredential({}, {})",
            dq(login.user),
            dq(login.password)
        ));
    }
    let has_handler = !handler.is_empty() || parts.cert().is_some();
    if has_handler {
        code.push_str("var handler = new HttpClientHandler");
        if handler.is_empty() {
            code.push_str("();\n");
        } else {
            code.push_str("\n{\n");
            for h in &handler {
                let _ = writeln!(code, "    {h},");
            }
            code.push_str("};\n");
        }
    }
    if let (Some(cert), Some(key)) = (parts.cert(), parts.key()) {
        usings.insert("System.Security.Cryptography.X509Certificates");
        let args = if cert == key {
            dq(cert)
        } else {
            format!("{}, {}", dq(cert), dq(key))
        };
        let _ = writeln!(
            code,
            "handler.ClientCertificates.Add(X509Certificate2.CreateFromPemFile({args}));"
        );
    }
    let client_arg = if has_handler { "handler" } else { "" };
    match parts.timeout_ms() {
        Some(ms) => {
            let _ = writeln!(
                code,
                "using var client = new HttpClient({client_arg}) {{ Timeout = TimeSpan.FromMilliseconds({ms}) }};"
            );
        }
        None => {
            let _ = writeln!(code, "using var client = new HttpClient({client_arg});");
        }
    }
    let _ = writeln!(
        code,
        "using var request = new HttpRequestMessage(new HttpMethod({}), {});",
        dq(parts.method.as_str()),
        dq(&parts.url)
    );
    for (k, v) in parts.other_headers() {
        let _ = writeln!(
            code,
            "request.Headers.TryAddWithoutValidation({}, {});",
            dq(k),
            dq(v)
        );
    }
    if let Some(login) = parts.basic() {
        usings.insert("System.Text");
        let _ = writeln!(
            code,
            "request.Headers.TryAddWithoutValidation(\"Authorization\", \"Basic \" + Convert.ToBase64String(Encoding.UTF8.GetBytes({})));",
            dq(&login.pair())
        );
    }
    let has_content = match &parts.payload {
        Payload::None => false,
        Payload::Text(text) => {
            let _ = writeln!(
                code,
                "request.Content = new StringContent({});",
                cs_str(text)
            );
            true
        }
        Payload::Form(fields) => {
            code.push_str("request.Content = new FormUrlEncodedContent(new KeyValuePair<string, string>[]\n{\n");
            for f in fields {
                let _ = writeln!(code, "    new({}, {}),", dq(&f.key), dq(&f.value));
            }
            code.push_str("});\n");
            true
        }
        Payload::Multipart(fields) => {
            code.push_str("var form = new MultipartFormDataContent();\n");
            for (ix, p) in fields.iter().enumerate() {
                let content = if p.file {
                    format!("new ByteArrayContent(File.ReadAllBytes({}))", dq(&p.value))
                } else {
                    format!("new StringContent({})", dq(&p.value))
                };
                let name = if p.file {
                    format!("{}, {}", dq(&p.key), dq(file_name(&p.value)))
                } else {
                    dq(&p.key)
                };
                let content_type = part_type(p).or(p.file.then_some("application/octet-stream"));
                match content_type {
                    Some(ct) => {
                        usings.insert("System.Net.Http.Headers");
                        let _ = writeln!(code, "var part{ix} = {content};");
                        let _ = writeln!(
                            code,
                            "part{ix}.Headers.ContentType = MediaTypeHeaderValue.Parse({});",
                            dq(ct)
                        );
                        let _ = writeln!(code, "form.Add(part{ix}, {name});");
                    }
                    None => {
                        let _ = writeln!(code, "form.Add({content}, {name});");
                    }
                }
            }
            code.push_str("request.Content = form;\n");
            true
        }
        Payload::File(path) => {
            let _ = writeln!(
                code,
                "request.Content = new ByteArrayContent(File.ReadAllBytes({}));",
                dq(path)
            );
            true
        }
    };
    if has_content && let Some(ct) = parts.content_type() {
        usings.insert("System.Net.Http.Headers");
        let _ = writeln!(
            code,
            "request.Content.Headers.ContentType = MediaTypeHeaderValue.Parse({});",
            dq(ct)
        );
    }
    code.push_str("using var response = await client.SendAsync(request);\n");
    code.push_str("Console.WriteLine((int)response.StatusCode);\n");
    code.push_str("Console.WriteLine(await response.Content.ReadAsStringAsync());\n");

    let mut out = String::from("// .NET 6+ top-level program (Program.cs).\n");
    out.push_str(&parts.notes(
        Handles {
            redirects_off: true,
            max_redirects: true,
            timeout: true,
            insecure: true,
            proxy_off: true,
            proxy_url: true,
            cert: true,
            digest: true,
        },
        "//",
    ));
    for using in &usings {
        let _ = writeln!(out, "using {using};");
    }
    if !usings.is_empty() {
        out.push('\n');
    }
    out.push_str(&code);
    out
}

// ---------------------------------------------------------------------------
// Java
// ---------------------------------------------------------------------------

/// Headers Java's HttpClient refuses to set.
const JAVA_RESTRICTED: [&str; 5] = ["connection", "content-length", "expect", "host", "upgrade"];

fn java(parts: &Parts) -> String {
    let mut imports: BTreeSet<&str> = [
        "java.net.URI",
        "java.net.http.HttpClient",
        "java.net.http.HttpRequest",
        "java.net.http.HttpResponse",
    ]
    .into();
    let java_str = |s: &str| joined(s, dq, "                        ");
    let mut setup = String::new();

    let publisher = match &parts.payload {
        Payload::None => "HttpRequest.BodyPublishers.noBody()".to_owned(),
        Payload::Text(text) => format!("HttpRequest.BodyPublishers.ofString({})", java_str(text)),
        Payload::Form(fields) => format!(
            "HttpRequest.BodyPublishers.ofString({})",
            dq(&urlencoded(fields))
        ),
        Payload::File(path) => {
            imports.insert("java.nio.file.Path");
            format!("HttpRequest.BodyPublishers.ofFile(Path.of({}))", dq(path))
        }
        Payload::Multipart(fields) => {
            imports.insert("java.io.ByteArrayOutputStream");
            imports.insert("java.nio.charset.StandardCharsets");
            setup.push_str("        var body = new ByteArrayOutputStream();\n");
            for segment in multipart_segments(fields) {
                match segment {
                    Segment::Text(text) => {
                        let _ = writeln!(
                            setup,
                            "        body.write({}.getBytes(StandardCharsets.UTF_8));",
                            dq(&text)
                        );
                    }
                    Segment::File(path) => {
                        imports.insert("java.nio.file.Files");
                        imports.insert("java.nio.file.Path");
                        let _ = writeln!(
                            setup,
                            "        body.write(Files.readAllBytes(Path.of({})));",
                            dq(path)
                        );
                    }
                }
            }
            "HttpRequest.BodyPublishers.ofByteArray(body.toByteArray())".to_owned()
        }
    };

    let mut client = String::from("        HttpClient client = HttpClient.newBuilder()\n");
    let redirect = if parts.redirects_off() {
        "NEVER"
    } else {
        "NORMAL"
    };
    let _ = writeln!(
        client,
        "                .followRedirects(HttpClient.Redirect.{redirect})"
    );
    let mut proxy_handled = false;
    if parts.proxy_off() {
        client.push_str("                .proxy(HttpClient.Builder.NO_PROXY)\n");
    } else if let Some((host, port)) = parts.proxy_url().and_then(proxy_host_port) {
        imports.insert("java.net.InetSocketAddress");
        imports.insert("java.net.ProxySelector");
        let _ = writeln!(
            client,
            "                .proxy(ProxySelector.of(new InetSocketAddress({}, {port})))",
            dq(&host)
        );
        proxy_handled = true;
    }
    client.push_str("                .build();\n");

    let mut request = String::from("        HttpRequest request = HttpRequest.newBuilder()\n");
    let _ = writeln!(
        request,
        "                .uri(URI.create({}))",
        dq(&parts.url)
    );
    if let Some(ms) = parts.timeout_ms() {
        imports.insert("java.time.Duration");
        let _ = writeln!(request, "                .timeout(Duration.ofMillis({ms}))");
    }
    let mut skipped = Vec::new();
    for (k, v) in &parts.headers {
        if JAVA_RESTRICTED.contains(&k.to_ascii_lowercase().as_str()) {
            skipped.push(k.as_str());
            continue;
        }
        let _ = writeln!(request, "                .header({}, {})", dq(k), dq(v));
    }
    if matches!(parts.payload, Payload::Multipart(_)) {
        let _ = writeln!(
            request,
            "                .header(\"Content-Type\", \"multipart/form-data; boundary={BOUNDARY}\")"
        );
    }
    if let Some(login) = parts.basic() {
        imports.insert("java.util.Base64");
        imports.insert("java.nio.charset.StandardCharsets");
        let _ = writeln!(
            request,
            "                .header(\"Authorization\", \"Basic \" + Base64.getEncoder().encodeToString({}.getBytes(StandardCharsets.UTF_8)))",
            dq(&login.pair())
        );
    }
    if parts.method != Method::Get || !matches!(parts.payload, Payload::None) {
        let _ = writeln!(
            request,
            "                .method({}, {publisher})",
            dq(parts.method.as_str())
        );
    }
    request.push_str("                .build();\n");

    let mut out = parts.notes(
        Handles {
            redirects_off: true,
            timeout: true,
            proxy_off: true,
            proxy_url: proxy_handled,
            ..Handles::default()
        },
        "//",
    );
    for header in skipped {
        let _ = writeln!(
            out,
            "// Not in this snippet: the {header} header (Java sets it itself)"
        );
    }
    for import in &imports {
        let _ = writeln!(out, "import {import};");
    }
    out.push_str(
        "\npublic class Main {\n    public static void main(String[] args) throws Exception {\n",
    );
    out.push_str(&setup);
    out.push_str(&client);
    out.push_str(&request);
    out.push_str("        HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());\n");
    out.push_str("        System.out.println(response.statusCode());\n");
    out.push_str("        System.out.println(response.body());\n");
    out.push_str("    }\n}\n");
    out
}

// ---------------------------------------------------------------------------
// PHP
// ---------------------------------------------------------------------------

fn php(parts: &Parts) -> String {
    let mut out = String::from("<?php\n");
    out.push_str(&parts.notes(
        Handles {
            redirects_off: true,
            max_redirects: true,
            timeout: true,
            insecure: true,
            proxy_off: true,
            proxy_url: true,
            cert: true,
            digest: true,
        },
        "//",
    ));
    let mut options = vec![("CURLOPT_URL", php_str(&parts.url))];
    match parts.method {
        Method::Get => {}
        Method::Head => options.push(("CURLOPT_NOBODY", "true".into())),
        other => options.push(("CURLOPT_CUSTOMREQUEST", php_str(other.as_str()))),
    }
    if !parts.headers.is_empty() {
        let mut h = String::from("[\n");
        for (k, v) in &parts.headers {
            let _ = writeln!(h, "        {},", php_str(&format!("{k}: {v}")));
        }
        h.push_str("    ]");
        options.push(("CURLOPT_HTTPHEADER", h));
    }
    if let Some(login) = &parts.login {
        options.push(("CURLOPT_USERPWD", php_str(&login.pair())));
        if login.digest {
            options.push(("CURLOPT_HTTPAUTH", "CURLAUTH_DIGEST".into()));
        }
    }
    match &parts.payload {
        Payload::None => {}
        Payload::Text(text) => options.push(("CURLOPT_POSTFIELDS", php_str(text))),
        Payload::Form(fields) => options.push(("CURLOPT_POSTFIELDS", php_str(&urlencoded(fields)))),
        Payload::Multipart(fields) => {
            let mut f = String::from("[\n");
            for p in fields {
                let value = if p.file {
                    format!(
                        "new CURLFile({}, {}, {})",
                        php_str(&p.value),
                        php_str(part_type(p).unwrap_or("application/octet-stream")),
                        php_str(file_name(&p.value))
                    )
                } else {
                    php_str(&p.value)
                };
                let _ = writeln!(f, "        {} => {value},", php_str(&p.key));
            }
            f.push_str("    ]");
            options.push(("CURLOPT_POSTFIELDS", f));
        }
        Payload::File(path) => options.push((
            "CURLOPT_POSTFIELDS",
            format!("file_get_contents({})", php_str(path)),
        )),
    }
    if parts.redirects_off() {
        options.push(("CURLOPT_FOLLOWLOCATION", "false".into()));
    } else {
        // libcurl does not follow redirects unless told to; Apix does.
        options.push(("CURLOPT_FOLLOWLOCATION", "true".into()));
        options.push((
            "CURLOPT_MAXREDIRS",
            parts.max_redirects().unwrap_or(10).to_string(),
        ));
    }
    if let Some(ms) = parts.timeout_ms() {
        options.push(("CURLOPT_TIMEOUT_MS", ms.to_string()));
    }
    if parts.insecure() {
        options.push(("CURLOPT_SSL_VERIFYPEER", "false".into()));
        options.push(("CURLOPT_SSL_VERIFYHOST", "0".into()));
    }
    if let Some(proxy) = parts.settings.proxy.as_deref() {
        // An empty string turns the proxy off, environment included.
        options.push(("CURLOPT_PROXY", php_str(proxy)));
    }
    if let (Some(cert), Some(key)) = (parts.cert(), parts.key()) {
        options.push(("CURLOPT_SSLCERT", php_str(cert)));
        if key != cert {
            options.push(("CURLOPT_SSLKEY", php_str(key)));
        }
    }
    options.push(("CURLOPT_RETURNTRANSFER", "true".into()));

    out.push_str("$ch = curl_init();\ncurl_setopt_array($ch, [\n");
    for (name, value) in options {
        let _ = writeln!(out, "    {name} => {value},");
    }
    out.push_str("]);\n");
    out.push_str("$response = curl_exec($ch);\n");
    out.push_str("if ($response === false) {\n    die(curl_error($ch) . \"\\n\");\n}\n");
    out.push_str("echo curl_getinfo($ch, CURLINFO_RESPONSE_CODE), \"\\n\", $response, \"\\n\";\n");
    out.push_str("curl_close($ch);\n");
    out
}

// ---------------------------------------------------------------------------
// Swift
// ---------------------------------------------------------------------------

fn swift(parts: &Parts) -> String {
    let mut out = String::from("import Foundation\n\n");
    out.push_str(&parts.notes(
        Handles {
            timeout: true,
            ..Handles::default()
        },
        "//",
    ));
    let _ = writeln!(
        out,
        "var request = URLRequest(url: URL(string: {})!)",
        swift_str(&parts.url)
    );
    let _ = writeln!(
        out,
        "request.httpMethod = {}",
        swift_str(parts.method.as_str())
    );
    if let Some(ms) = parts.timeout_ms() {
        let _ = writeln!(out, "request.timeoutInterval = {}", seconds(ms));
    }
    for (k, v) in &parts.headers {
        let _ = writeln!(
            out,
            "request.addValue({}, forHTTPHeaderField: {})",
            swift_str(v),
            swift_str(k)
        );
    }
    if let Some(login) = parts.basic() {
        let _ = writeln!(
            out,
            "request.addValue(\"Basic \" + Data({}.utf8).base64EncodedString(), forHTTPHeaderField: \"Authorization\")",
            swift_str(&login.pair())
        );
    }
    match &parts.payload {
        Payload::None => {}
        Payload::Text(text) => {
            let literal = joined(text, swift_str, "    ");
            if literal.contains('\n') {
                let _ = writeln!(out, "request.httpBody = Data((\n    {literal}\n).utf8)");
            } else {
                let _ = writeln!(out, "request.httpBody = Data({literal}.utf8)");
            }
        }
        Payload::Form(fields) => {
            let _ = writeln!(
                out,
                "request.httpBody = Data({}.utf8)",
                swift_str(&urlencoded(fields))
            );
        }
        Payload::File(path) => {
            let _ = writeln!(
                out,
                "request.httpBody = try Data(contentsOf: URL(fileURLWithPath: {}))",
                swift_str(path)
            );
        }
        Payload::Multipart(fields) => {
            out.push_str("var body = Data()\n");
            for segment in multipart_segments(fields) {
                match segment {
                    Segment::Text(text) => {
                        let _ = writeln!(out, "body.append(Data({}.utf8))", swift_str(&text));
                    }
                    Segment::File(path) => {
                        let _ = writeln!(
                            out,
                            "body.append(try Data(contentsOf: URL(fileURLWithPath: {})))",
                            swift_str(path)
                        );
                    }
                }
            }
            out.push_str("request.httpBody = body\n");
            let _ = writeln!(
                out,
                "request.setValue(\"multipart/form-data; boundary={BOUNDARY}\", forHTTPHeaderField: \"Content-Type\")"
            );
        }
    }
    out.push_str("let (data, response) = try await URLSession.shared.data(for: request)\n");
    out.push_str("print((response as! HTTPURLResponse).statusCode)\n");
    out.push_str("print(String(decoding: data, as: UTF8.self))\n");
    out
}

// ---------------------------------------------------------------------------
// Kotlin
// ---------------------------------------------------------------------------

fn kotlin(parts: &Parts) -> String {
    let mut imports: BTreeSet<&str> = ["okhttp3.OkHttpClient", "okhttp3.Request"].into();
    let media = |ct: &str| format!("{}.toMediaType()", kt_str(ct));
    let mut code = String::new();

    let mut client = String::from("    val client = OkHttpClient.Builder()\n");
    if let Some(ms) = parts.timeout_ms() {
        imports.insert("java.util.concurrent.TimeUnit");
        let _ = writeln!(client, "        .callTimeout({ms}, TimeUnit.MILLISECONDS)");
    }
    if parts.redirects_off() {
        client.push_str("        .followRedirects(false)\n        .followSslRedirects(false)\n");
    }
    let mut proxy_handled = false;
    if parts.proxy_off() {
        imports.insert("java.net.Proxy");
        client.push_str("        .proxy(Proxy.NO_PROXY)\n");
    } else if let Some((host, port)) = parts.proxy_url().and_then(proxy_host_port) {
        imports.insert("java.net.Proxy");
        imports.insert("java.net.InetSocketAddress");
        let _ = writeln!(
            client,
            "        .proxy(Proxy(Proxy.Type.HTTP, InetSocketAddress({}, {port})))",
            kt_str(&host)
        );
        proxy_handled = true;
    }
    client.push_str("        .build()\n");
    code.push_str(&client);

    let body = match &parts.payload {
        Payload::None => {
            if matches!(parts.method, Method::Post | Method::Put | Method::Patch) {
                imports.insert("okhttp3.RequestBody.Companion.toRequestBody");
                "\"\".toRequestBody()".to_owned()
            } else {
                "null".to_owned()
            }
        }
        Payload::Text(text) => {
            imports.insert("okhttp3.RequestBody.Companion.toRequestBody");
            let literal = joined(text, kt_str, "        ");
            let typed = match parts.content_type() {
                Some(ct) => {
                    imports.insert("okhttp3.MediaType.Companion.toMediaType");
                    media(ct)
                }
                None => String::new(),
            };
            if literal.contains('\n') {
                let _ = writeln!(
                    code,
                    "    val body = (\n        {literal}\n    ).toRequestBody({typed})"
                );
            } else {
                let _ = writeln!(code, "    val body = {literal}.toRequestBody({typed})");
            }
            "body".into()
        }
        Payload::Form(fields) => {
            imports.insert("okhttp3.FormBody");
            code.push_str("    val body = FormBody.Builder()\n");
            for f in fields {
                let _ = writeln!(
                    code,
                    "        .add({}, {})",
                    kt_str(&f.key),
                    kt_str(&f.value)
                );
            }
            code.push_str("        .build()\n");
            "body".into()
        }
        Payload::Multipart(fields) => {
            imports.insert("okhttp3.MultipartBody");
            code.push_str(
                "    val body = MultipartBody.Builder()\n        .setType(MultipartBody.FORM)\n",
            );
            for p in fields {
                if p.file {
                    imports.insert("java.io.File");
                    imports.insert("okhttp3.MediaType.Companion.toMediaType");
                    imports.insert("okhttp3.RequestBody.Companion.asRequestBody");
                    let _ = writeln!(
                        code,
                        "        .addFormDataPart({}, {}, File({}).asRequestBody({}))",
                        kt_str(&p.key),
                        kt_str(file_name(&p.value)),
                        kt_str(&p.value),
                        media(part_type(p).unwrap_or("application/octet-stream"))
                    );
                } else if let Some(ct) = part_type(p) {
                    imports.insert("okhttp3.MediaType.Companion.toMediaType");
                    imports.insert("okhttp3.RequestBody.Companion.toRequestBody");
                    let _ = writeln!(
                        code,
                        "        .addPart(MultipartBody.Part.createFormData({}, null, {}.toRequestBody({})))",
                        kt_str(&p.key),
                        kt_str(&p.value),
                        media(ct)
                    );
                } else {
                    let _ = writeln!(
                        code,
                        "        .addFormDataPart({}, {})",
                        kt_str(&p.key),
                        kt_str(&p.value)
                    );
                }
            }
            code.push_str("        .build()\n");
            "body".into()
        }
        Payload::File(path) => {
            imports.insert("java.io.File");
            imports.insert("okhttp3.RequestBody.Companion.asRequestBody");
            let typed = match parts.content_type() {
                Some(ct) => {
                    imports.insert("okhttp3.MediaType.Companion.toMediaType");
                    media(ct)
                }
                None => String::new(),
            };
            let _ = writeln!(
                code,
                "    val body = File({}).asRequestBody({typed})",
                kt_str(path)
            );
            "body".into()
        }
    };

    code.push_str("    val request = Request.Builder()\n");
    let _ = writeln!(code, "        .url({})", kt_str(&parts.url));
    let _ = writeln!(
        code,
        "        .method({}, {body})",
        kt_str(parts.method.as_str())
    );
    // OkHttp takes Content-Type from the body.
    let headers: Vec<_> = if matches!(parts.payload, Payload::None) {
        parts.headers.iter().collect()
    } else {
        parts.other_headers().collect()
    };
    for (k, v) in headers {
        let _ = writeln!(code, "        .addHeader({}, {})", kt_str(k), kt_str(v));
    }
    if let Some(login) = parts.basic() {
        imports.insert("okhttp3.Credentials");
        let _ = writeln!(
            code,
            "        .addHeader(\"Authorization\", Credentials.basic({}, {}))",
            kt_str(login.user),
            kt_str(login.password)
        );
    }
    code.push_str("        .build()\n");
    code.push_str("    client.newCall(request).execute().use { response ->\n");
    code.push_str("        println(response.code)\n");
    code.push_str("        println(response.body?.string())\n");
    code.push_str("    }\n");

    let mut out = String::from(
        "// build.gradle.kts: implementation(\"com.squareup.okhttp3:okhttp:4.12.0\")\n",
    );
    out.push_str(&parts.notes(
        Handles {
            redirects_off: true,
            timeout: true,
            proxy_off: true,
            proxy_url: proxy_handled,
            ..Handles::default()
        },
        "//",
    ));
    for import in &imports {
        let _ = writeln!(out, "import {import}");
    }
    out.push_str("\nfun main() {\n");
    out.push_str(&code);
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use apix_core::KeyValue;

    fn post_json() -> Request {
        let mut r = Request::new(Method::Post, "https://api.example.com/users");
        r.query.push(KeyValue::new("page", "2"));
        r.headers.push(KeyValue::new("X-Trace", "on"));
        r.auth = Auth::Bearer {
            token: "t0k".into(),
        };
        r.body = Body::Json("{\n  \"name\": \"Ada $1\"\n}".into());
        r
    }

    fn with_everything() -> Request {
        let mut r = post_json();
        r.settings = RequestSettings {
            timeout_ms: Some(2500),
            follow_redirects: Some(false),
            tls_verify: Some(false),
            proxy: Some("http://proxy.local:8080".into()),
            client_cert: Some("certs/client.pem".into()),
            client_key: Some("certs/client.key".into()),
            ..Default::default()
        };
        r
    }

    #[test]
    fn every_language_writes_url_headers_and_body() {
        let r = post_json();
        for lang in Lang::ALL {
            let code = to_code(&r, lang);
            assert!(
                code.contains("https://api.example.com/users?page=2"),
                "{lang:?}:\n{code}"
            );
            assert!(code.contains("X-Trace"), "{lang:?}:\n{code}");
            assert!(code.contains("Bearer t0k"), "{lang:?}:\n{code}");
            assert!(code.contains("Ada"), "{lang:?}:\n{code}");
            assert!(
                !code.contains("Not in this snippet"),
                "nothing to note without settings: {lang:?}\n{code}"
            );
        }
    }

    #[test]
    fn kotlin_escapes_dollar_and_strings_keep_their_shape() {
        let r = post_json();
        let kt = to_code(&r, Lang::Kotlin);
        assert!(kt.contains("Ada \\$1"), "{kt}");
        assert!(kt.contains("\"application/json\".toMediaType()"), "{kt}");
        // JS keeps the JSON as a template literal; Rust as a raw string.
        assert!(to_code(&r, Lang::Fetch).contains("`{\n  \"name\": \"Ada $1\"\n}`"));
        assert!(to_code(&r, Lang::Rust).contains("r#\"{\n  \"name\": \"Ada $1\"\n}\"#"));
        assert!(to_code(&r, Lang::Go).contains("`{\n  \"name\": \"Ada $1\"\n}`"));
        assert!(to_code(&r, Lang::CSharp).contains("@\"{\n  \"\"name\"\": \"\"Ada $1\"\"\n}\""));
        // Swift and Rust write control characters with braces.
        assert_eq!(swift_str("a\u{1}"), "\"a\\u{1}\"");
        assert_eq!(dq("a\u{1}"), "\"a\\u0001\"");
        assert_eq!(rust_str("say \"#hi\""), "r##\"say \"#hi\"\"##");
        assert_eq!(php_str("it's \\o/"), "'it\\'s \\\\o/'");
    }

    #[test]
    fn settings_are_written_or_noted() {
        let r = with_everything();
        let py = to_code(&r, Lang::Python);
        for needle in [
            "timeout=2.5",
            "allow_redirects=False",
            "verify=False",
            "\"https\": \"http://proxy.local:8080\"",
            "cert=(\"certs/client.pem\", \"certs/client.key\")",
        ] {
            assert!(py.contains(needle), "{needle}\n{py}");
        }
        let go = to_code(&r, Lang::Go);
        for needle in [
            "Timeout: 2500 * time.Millisecond",
            "http.ErrUseLastResponse",
            "InsecureSkipVerify: true",
            "Proxy: http.ProxyURL(proxyURL)",
            "tls.LoadX509KeyPair(\"certs/client.pem\", \"certs/client.key\")",
            "\t\"crypto/tls\"",
            "\t\"net/url\"",
            "\t\"time\"",
        ] {
            assert!(go.contains(needle), "{needle}\n{go}");
        }
        assert!(!go.contains("Not in this snippet"), "{go}");

        // Swift has no switch for most of these; each is noted instead.
        let swift = to_code(&r, Lang::Swift);
        assert!(swift.contains("request.timeoutInterval = 2.5"), "{swift}");
        for note in [
            "redirects are not followed",
            "no TLS certificate check",
            "proxy http://proxy.local:8080",
            "client certificate certs/client.pem",
        ] {
            assert!(
                swift.contains(&format!("// Not in this snippet: {note}")),
                "{note}\n{swift}"
            );
        }
        let java = to_code(&r, Lang::Java);
        assert!(java.contains("HttpClient.Redirect.NEVER"), "{java}");
        assert!(
            java.contains("new InetSocketAddress(\"proxy.local\", 8080)"),
            "{java}"
        );
        assert!(
            java.contains("Not in this snippet: no TLS certificate check"),
            "{java}"
        );

        let php = to_code(&r, Lang::Php);
        assert!(php.contains("CURLOPT_FOLLOWLOCATION => false"), "{php}");
        assert!(
            php.contains("CURLOPT_SSLKEY => 'certs/client.key'"),
            "{php}"
        );
        let mut off = post_json();
        off.settings.proxy = Some(String::new());
        assert!(to_code(&off, Lang::Php).contains("CURLOPT_PROXY => ''"));
        assert!(to_code(&off, Lang::Rust).contains(".no_proxy()"));
        assert!(to_code(&off, Lang::Axios).contains("proxy: false"));
        // Without redirect settings PHP still follows, like Apix.
        assert!(to_code(&post_json(), Lang::Php).contains("CURLOPT_MAXREDIRS => 10"));
    }

    #[test]
    fn basic_digest_form_and_files() {
        let mut r = Request::new(Method::Post, "https://x.dev/up");
        r.auth = Auth::Basic {
            username: "ada".into(),
            password: "pw".into(),
        };
        r.body = Body::Form(vec![KeyValue::new("a", "1 2"), KeyValue::new("b", "&")]);
        assert!(to_code(&r, Lang::Python).contains("auth=(\"ada\", \"pw\")"));
        assert!(to_code(&r, Lang::Go).contains("req.SetBasicAuth(\"ada\", \"pw\")"));
        assert!(to_code(&r, Lang::Php).contains("CURLOPT_POSTFIELDS => 'a=1+2&b=%26'"));
        assert!(to_code(&r, Lang::Fetch).contains("btoa(\"ada:pw\")"));

        r.auth = Auth::Digest {
            username: "ada".into(),
            password: "pw".into(),
        };
        assert!(to_code(&r, Lang::Python).contains("HTTPDigestAuth(\"ada\", \"pw\")"));
        assert!(to_code(&r, Lang::CSharp).contains("new NetworkCredential(\"ada\", \"pw\")"));
        assert!(to_code(&r, Lang::Go).contains("Not in this snippet: Digest auth as ada"));

        r.auth = Auth::None;
        r.headers
            .push(KeyValue::new("Content-Type", "multipart/form-data"));
        r.body = Body::Multipart(vec![
            FormPart::text("name", "Ada"),
            FormPart {
                content_type: Some("image/png".into()),
                ..FormPart::file("doc", "C:\\pics\\a.png")
            },
        ]);
        for lang in Lang::ALL {
            let code = to_code(&r, lang);
            assert!(code.contains("a.png"), "{lang:?}\n{code}");
            assert!(
                !code.contains("\"multipart/form-data\""),
                "the boundary-less header is dropped: {lang:?}\n{code}"
            );
        }
        let java = to_code(&r, Lang::Java);
        assert!(
            java.contains("filename=\\\"a.png\\\"\\r\\nContent-Type: image/png"),
            "{java}"
        );
        assert!(java.contains(&format!("--{BOUNDARY}--")), "{java}");
    }

    #[test]
    fn methods_without_body() {
        let r = Request::new(Method::Get, "https://x.dev");
        let java = to_code(&r, Lang::Java);
        assert!(!java.contains(".method("), "GET is the default: {java}");
        let kt = to_code(&r, Lang::Kotlin);
        assert!(kt.contains(".method(\"GET\", null)"), "{kt}");
        let post = Request::new(Method::Post, "https://x.dev");
        assert!(to_code(&post, Lang::Kotlin).contains(".method(\"POST\", \"\".toRequestBody())"));
        let head = Request::new(Method::Head, "https://x.dev");
        assert!(to_code(&head, Lang::Php).contains("CURLOPT_NOBODY => true"));
        assert!(!to_code(&r, Lang::Fetch).contains("method:"));
    }
}
