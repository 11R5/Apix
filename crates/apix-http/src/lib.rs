//! Turns a resolved [`apix_core::Request`] into a real HTTP call and captures
//! everything the UI needs to show: status, headers, raw body and timings.

pub mod graphql;
mod local;
pub mod oauth2;
pub mod sse;
mod trace;
pub mod ws;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use apix_core::{ApiKeyLocation, Auth, Body, EffectiveSettings, ProxyChoice, Request, sign};
use bytes::Bytes;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, WWW_AUTHENTICATE};
use reqwest_cookie_store::CookieStoreMutex;

pub use local::{LocalNetwork, local_network};
pub use trace::{Certificate, DnsLookup, NetTrace, Phases, Redirect, read_certificate};
use trace::{Recorder, TimedResolver, TimingLayer};

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("invalid URL `{url}`: {reason}")]
    InvalidUrl { url: String, reason: String },
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error("{0}")]
    Body(String),
    #[error("{0}")]
    Auth(String),
    #[error("cannot read {path}")]
    File {
        path: String,
        #[source]
        source: std::io::Error,
    },
    /// reqwest names the first URL even when a redirect target failed.
    #[error("while following a redirect to {to}")]
    Redirected {
        to: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("the request timed out")]
    TimedOut,
    /// A request setting (proxy URL, client certificate) that cannot be used.
    #[error("{0}")]
    Settings(String),
}

#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// Until the status line and headers arrived.
    pub ttfb: Duration,
    /// Until the last body byte arrived.
    pub total: Duration,
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub reason: Option<&'static str>,
    pub version: String,
    pub headers: Vec<(String, String)>,
    pub body: Bytes,
    pub timing: Timing,
    /// What happened on the network while sending.
    pub trace: NetTrace,
}

impl Response {
    /// A response read back from somewhere else (the history), with no
    /// network trace.
    pub fn from_parts(
        status: u16,
        version: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
        timing: Timing,
    ) -> Self {
        Self {
            status,
            reason: reqwest::StatusCode::from_u16(status)
                .ok()
                .and_then(|s| s.canonical_reason()),
            version,
            headers,
            body: Bytes::from(body),
            timing,
            trace: NetTrace::default(),
        }
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// What the body is, for picking a viewer: by `Content-Type` first, then
    /// by looking at the bytes when the type says nothing useful.
    pub fn body_kind(&self) -> BodyKind {
        let ct = self
            .header("content-type")
            .map(|v| {
                v.split(';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase()
            })
            .unwrap_or_default();
        if ct.contains("json") {
            return BodyKind::Json;
        }
        if let Some(image) = ct.strip_prefix("image/") {
            return match image {
                "png" => BodyKind::Image(ImageKind::Png),
                "jpeg" | "jpg" | "pjpeg" => BodyKind::Image(ImageKind::Jpeg),
                "gif" => BodyKind::Image(ImageKind::Gif),
                "webp" => BodyKind::Image(ImageKind::Webp),
                "bmp" | "x-bmp" => BodyKind::Image(ImageKind::Bmp),
                "svg+xml" => BodyKind::Image(ImageKind::Svg),
                _ => sniff(&self.body).unwrap_or(BodyKind::Binary),
            };
        }
        if ct.contains("html") {
            return BodyKind::Html;
        }
        if ct.ends_with("xml") || ct.contains("+xml") {
            return BodyKind::Xml;
        }
        let textual = ct.starts_with("text/")
            || ct.contains("javascript")
            || ct.contains("x-www-form-urlencoded")
            || ct.contains("yaml")
            || ct.contains("graphql");
        if textual {
            return BodyKind::Text;
        }
        sniff(&self.body).unwrap_or(BodyKind::Text)
    }

    pub fn is_json(&self) -> bool {
        self.header("content-type")
            .is_some_and(|ct| ct.contains("json"))
    }

    /// A `text/event-stream` body (server-sent events).
    pub fn is_event_stream(&self) -> bool {
        self.header("content-type")
            .is_some_and(sse::is_event_stream)
    }

    pub fn is_success(&self) -> bool {
        (200..400).contains(&self.status)
    }

    /// The body as text; JSON is pretty-printed, anything else is decoded lossily.
    /// Linear and tree-free, so it is safe for very large bodies.
    pub fn pretty_body(&self) -> String {
        if self.is_json()
            && let Some(pretty) = apix_core::text::pretty_json(&self.body)
        {
            return pretty;
        }
        self.raw_body()
    }

    pub fn raw_body(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// What a response body holds, for choosing how to show it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Json,
    Xml,
    Html,
    Text,
    Image(ImageKind),
    /// Anything else that is not text: shown as hex, or saved to a file.
    Binary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    Jpeg,
    Gif,
    Webp,
    Bmp,
    Svg,
}

/// The kind of `body` from its first bytes; `None` for an empty body.
fn sniff(body: &[u8]) -> Option<BodyKind> {
    if body.is_empty() {
        return None;
    }
    let image = |kind| Some(BodyKind::Image(kind));
    if body.starts_with(b"\x89PNG\r\n\x1a\n") {
        return image(ImageKind::Png);
    }
    if body.starts_with(&[0xff, 0xd8, 0xff]) {
        return image(ImageKind::Jpeg);
    }
    if body.starts_with(b"GIF87a") || body.starts_with(b"GIF89a") {
        return image(ImageKind::Gif);
    }
    if body.len() > 12 && &body[..4] == b"RIFF" && &body[8..12] == b"WEBP" {
        return image(ImageKind::Webp);
    }
    if body.starts_with(b"BM") && body.len() > 26 {
        return image(ImageKind::Bmp);
    }
    let head = &body[..body.len().min(8192)];
    // Cut at a character boundary so a multi-byte letter does not look broken.
    let valid = match std::str::from_utf8(head) {
        Ok(_) => true,
        Err(e) => e.error_len().is_none() && e.valid_up_to() + 4 > head.len(),
    };
    if !valid || head.contains(&0) {
        return Some(BodyKind::Binary);
    }
    let start = String::from_utf8_lossy(&head[..head.len().min(256)])
        .trim_start_matches('\u{feff}')
        .trim_start()
        .to_ascii_lowercase();
    if start.starts_with("<!doctype html") || start.starts_with("<html") {
        Some(BodyKind::Html)
    } else if start.starts_with("<svg") {
        image(ImageKind::Svg)
    } else if start.starts_with("<?xml") {
        Some(BodyKind::Xml)
    } else {
        Some(BodyKind::Text)
    }
}

/// `512 B`, `1.6 KB`, `3.2 MB`.
pub fn human_size(bytes: usize) -> String {
    match bytes {
        0..1024 => format!("{bytes} B"),
        1024..1_048_576 => format!("{:.1} KB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

/// `9656462` → `9,656,462`.
pub fn group_digits(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `840 µs`, `57 ms`, `1.24 s`.
pub fn human_duration(d: Duration) -> String {
    let micros = d.as_micros();
    match micros {
        0..1_000 => format!("{micros} µs"),
        1_000..1_000_000 => format!("{} ms", micros / 1_000),
        _ => format!("{:.2} s", d.as_secs_f64()),
    }
}

pub(crate) const USER_AGENT: &str = concat!("apix/", env!("CARGO_PKG_VERSION"));

/// One cookie in the shared jar, for the cookie manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookieInfo {
    pub domain: String,
    pub path: String,
    pub name: String,
    pub value: String,
    /// `None` is a session cookie.
    pub expires: Option<std::time::SystemTime>,
    pub secure: bool,
    pub http_only: bool,
}

/// Sends requests. Each send builds its own short-lived connection so its
/// network trace (DNS, connection setup, redirects) belongs to that request.
/// All sends share one cookie jar, which `cookies`, `remove_cookie` and
/// `clear_cookies` expose to the cookie manager.
#[derive(Debug, Clone)]
pub struct HttpClient {
    cookies: Arc<CookieStoreMutex>,
    /// When set, the jar is written back here (JSON) after every change.
    cookie_file: Option<PathBuf>,
}

impl HttpClient {
    /// A client whose cookies live only for this run.
    pub fn new() -> Result<Self, HttpError> {
        Ok(Self {
            cookies: Arc::new(CookieStoreMutex::default()),
            cookie_file: None,
        })
    }

    /// A client whose cookies persist in `path` across runs. A missing or
    /// unreadable file starts an empty jar.
    pub fn with_cookie_file(path: impl Into<PathBuf>) -> Result<Self, HttpError> {
        let path = path.into();
        let store = std::fs::File::open(&path)
            .ok()
            .and_then(|f| cookie_store::serde::json::load(std::io::BufReader::new(f)).ok())
            .unwrap_or_default();
        Ok(Self {
            cookies: Arc::new(CookieStoreMutex::new(store)),
            cookie_file: Some(path),
        })
    }

    /// Every unexpired cookie, sorted by domain, then name.
    pub fn cookies(&self) -> Vec<CookieInfo> {
        let store = self.cookies.lock().unwrap();
        let mut out: Vec<CookieInfo> = store
            .iter_unexpired()
            .map(|c| CookieInfo {
                // The domain as stored (host-only cookies carry no Domain
                // attribute, but the store files them under the host).
                domain: String::from(&c.domain),
                path: c.path().unwrap_or("/").to_owned(),
                name: c.name().to_owned(),
                value: c.value().to_owned(),
                // The store's own expiry covers Max-Age as well as Expires.
                expires: match c.expires {
                    cookie_store::CookieExpiration::AtUtc(at) => {
                        Some(std::time::SystemTime::from(at))
                    }
                    cookie_store::CookieExpiration::SessionEnd => None,
                },
                secure: c.secure().unwrap_or(false),
                http_only: c.http_only().unwrap_or(false),
            })
            .collect();
        drop(store);
        out.sort_by(|a, b| (a.domain.as_str(), a.name.as_str()).cmp(&(&b.domain, &b.name)));
        out
    }

    pub fn remove_cookie(&self, domain: &str, path: &str, name: &str) {
        self.cookies.lock().unwrap().remove(domain, path, name);
        self.persist_cookies();
    }

    pub fn clear_cookies(&self) {
        self.cookies.lock().unwrap().clear();
        self.persist_cookies();
    }

    fn persist_cookies(&self) {
        let Some(path) = &self.cookie_file else {
            return;
        };
        let store = self.cookies.lock().unwrap();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut file) = std::fs::File::create(path) {
            let _ = cookie_store::serde::json::save(&store, &mut file);
        }
    }

    /// A client whose DNS lookups, connections and redirects are recorded,
    /// configured by the request's effective settings.
    fn traced(
        &self,
        recorder: &Arc<Recorder>,
        eff: &EffectiveSettings,
    ) -> Result<reqwest::Client, HttpError> {
        let redirects = recorder.clone();
        let follow = eff.follow_redirects;
        let max_redirects = eff.max_redirects as usize;
        let policy = reqwest::redirect::Policy::custom(move |attempt| {
            if !follow {
                // The 3xx answer itself is the response.
                return attempt.stop();
            }
            redirects.redirect(attempt.status().as_u16(), attempt.url().to_string());
            if attempt.previous().len() > max_redirects {
                attempt.error("too many redirects")
            } else {
                attempt.follow()
            }
        });
        let mut builder = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .tls_info(true)
            .dns_resolver(Arc::new(TimedResolver(recorder.clone())))
            .connector_layer(TimingLayer(recorder.clone()))
            .redirect(policy);
        if let Some(timeout) = eff.timeout {
            builder = builder.timeout(timeout);
        }
        if !eff.tls_verify {
            builder = builder.danger_accept_invalid_certs(true);
        }
        match &eff.proxy {
            // reqwest reads HTTP_PROXY/HTTPS_PROXY/ALL_PROXY by itself.
            ProxyChoice::Environment => {}
            ProxyChoice::Off => builder = builder.no_proxy(),
            ProxyChoice::Url(url) => {
                let proxy = reqwest::Proxy::all(url)
                    .map_err(|e| HttpError::Settings(format!("invalid proxy `{url}`: {e}")))?;
                builder = builder.proxy(proxy);
            }
        }
        if let Some(cert) = &eff.client_cert {
            let mut pem = read_file(cert)?;
            if let Some(key) = eff.client_key.as_deref().filter(|key| *key != cert) {
                pem.push(b'\n');
                pem.extend(read_file(key)?);
            }
            let identity = reqwest::Identity::from_pem(&pem).map_err(|e| {
                HttpError::Settings(format!("cannot load the client certificate: {e}"))
            })?;
            builder = builder.identity(identity);
        }
        if eff.send_cookies {
            builder = builder.cookie_provider(self.cookies.clone());
        }
        Ok(builder.build()?)
    }

    /// Sends a request whose `{{variables}}` have already been resolved.
    pub async fn send(&self, req: &Request) -> Result<Response, HttpError> {
        self.send_with(req, |_| {}).await
    }

    /// Like `send`, reporting the status and headers as soon as they are in
    /// and then every piece of the body as it arrives, so a stream (server-
    /// sent events) can be shown while it lasts. The response still holds
    /// the whole body at the end.
    pub async fn send_with(
        &self,
        req: &Request,
        mut progress: impl FnMut(Progress<'_>) + Send,
    ) -> Result<Response, HttpError> {
        // OAuth 2.0 becomes a bearer token first (from the cache, or fetched).
        let with_token;
        let req = match &req.auth {
            Auth::OAuth2(config) => {
                let token = oauth2::access_token(config).await?;
                with_token = Request {
                    auth: Auth::Bearer { token },
                    ..req.clone()
                };
                &with_token
            }
            _ => req,
        };

        let recorder = Arc::new(Recorder::default());
        let client = self.traced(&recorder, &req.settings.effective())?;
        let started = Instant::now();
        let mut response = execute(&client, prepare(&client, req)?, &recorder).await?;

        // Digest: the first answer is a 401 with the challenge; answer it once.
        if let Auth::Digest { username, password } = &req.auth
            && response.status() == reqwest::StatusCode::UNAUTHORIZED
            && let Some(challenge) = response
                .headers()
                .get_all(WWW_AUTHENTICATE)
                .iter()
                .filter_map(|v| v.to_str().ok())
                .find_map(sign::parse_digest_challenge)
        {
            let mut retry = prepare(&client, req)?;
            let url = retry.url();
            let uri = match url.query() {
                Some(query) => format!("{}?{query}", url.path()),
                None => url.path().to_owned(),
            };
            let value = sign::digest_authorization(
                &challenge,
                username,
                password,
                retry.method().as_str(),
                &uri,
                &sign::random_token(16),
            )
            .map_err(HttpError::Auth)?;
            retry
                .headers_mut()
                .insert(AUTHORIZATION, header_value(&value)?);
            // Reading the 401's body lets the retry reuse the connection.
            let _ = response.bytes().await;
            response = execute(&client, retry, &recorder).await?;
        }
        let ttfb = started.elapsed();

        let connection = response
            .extensions()
            .get::<hyper_util::client::legacy::connect::HttpInfo>()
            .map(|info| (info.local_addr(), info.remote_addr()));
        let certificate = response
            .extensions()
            .get::<reqwest::tls::TlsInfo>()
            .and_then(|tls| tls.peer_certificate())
            .and_then(read_certificate);

        let status = response.status();
        let version = format!("{:?}", response.version());
        let headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    String::from_utf8_lossy(v.as_bytes()).into_owned(),
                )
            })
            .collect();
        progress(Progress::Head {
            status: status.as_u16(),
            headers: &headers,
        });
        let expected = response.content_length().unwrap_or(0).min(64 * 1024 * 1024) as usize;
        let mut body = Vec::with_capacity(expected);
        loop {
            let chunk = response.chunk().await.map_err(|e| {
                if e.is_timeout() {
                    HttpError::TimedOut
                } else {
                    HttpError::Request(e)
                }
            })?;
            let Some(chunk) = chunk else {
                break;
            };
            progress(Progress::Chunk(&chunk));
            body.extend_from_slice(&chunk);
        }
        let body = Bytes::from(body);
        let total = started.elapsed();
        self.persist_cookies();

        let mut trace = recorder.take();
        trace.local_addr = connection.map(|(local, _)| local);
        trace.remote_addr = connection.map(|(_, remote)| remote);
        trace.certificate = certificate;
        Ok(Response {
            status: status.as_u16(),
            reason: status.canonical_reason(),
            version,
            headers,
            body,
            timing: Timing { ttfb, total },
            trace,
        })
    }
}

/// What `HttpClient::send_with` reports while a response comes in.
pub enum Progress<'a> {
    /// The status line and headers.
    Head {
        status: u16,
        headers: &'a [(String, String)],
    },
    /// A piece of the body.
    Chunk(&'a [u8]),
}

/// Sends one request; a failure after a redirect names the redirect target.
async fn execute(
    client: &reqwest::Client,
    request: reqwest::Request,
    recorder: &Recorder,
) -> Result<reqwest::Response, HttpError> {
    client.execute(request).await.map_err(|source| {
        if source.is_timeout() {
            return HttpError::TimedOut;
        }
        match recorder.take().redirects.pop() {
            Some(last) => HttpError::Redirected {
                to: last.to,
                source,
            },
            None => HttpError::Request(source),
        }
    })
}

/// The request as it goes on the wire, signed for AWS or OAuth 1.0.
pub(crate) fn prepare(
    client: &reqwest::Client,
    req: &Request,
) -> Result<reqwest::Request, HttpError> {
    let mut request = build(client, req)?.build()?;
    match &req.auth {
        Auth::AwsSigV4(aws) => {
            let payload_hash = match request.body() {
                None => sign::sha256_hex(b""),
                Some(body) => match body.as_bytes() {
                    Some(bytes) => sign::sha256_hex(bytes),
                    // A multipart form is a stream.
                    None => sign::UNSIGNED_PAYLOAD.to_owned(),
                },
            };
            let content_type = request
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let headers = sign::aws_sigv4(
                aws,
                request.method().as_str(),
                request.url(),
                content_type.as_deref(),
                &payload_hash,
                sign::unix_now(),
            );
            for (name, value) in headers {
                let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                    .expect("signing header names are valid");
                request.headers_mut().insert(name, header_value(&value)?);
            }
        }
        Auth::OAuth1(o) => {
            let form: Vec<(String, String)> = match &req.body {
                Body::Form(rows) => rows
                    .iter()
                    .filter(|r| r.enabled)
                    .map(|r| (r.key.clone(), r.value.clone()))
                    .collect(),
                _ => Vec::new(),
            };
            let params = sign::oauth1_params(
                o,
                request.method().as_str(),
                request.url(),
                &form,
                sign::unix_now(),
                &sign::random_token(16),
            );
            match o.location {
                ApiKeyLocation::Header => {
                    let value = sign::oauth1_header(&params, &o.realm);
                    request
                        .headers_mut()
                        .insert(AUTHORIZATION, header_value(&value)?);
                }
                ApiKeyLocation::Query => {
                    request.url_mut().query_pairs_mut().extend_pairs(params);
                }
            }
        }
        _ => {}
    }
    Ok(request)
}

fn header_value(value: &str) -> Result<reqwest::header::HeaderValue, HttpError> {
    reqwest::header::HeaderValue::from_str(value)
        .map_err(|_| HttpError::Auth(format!("`{value}` cannot be sent as a header")))
}

/// The reqwest request for `req` on `client`.
fn build(client: &reqwest::Client, req: &Request) -> Result<reqwest::RequestBuilder, HttpError> {
    let mut url = reqwest::Url::parse(&req.url).map_err(|e| HttpError::InvalidUrl {
        url: req.url.clone(),
        reason: e.to_string(),
    })?;

    let mut query: Vec<(&str, &str)> = req
        .query
        .iter()
        .filter(|q| q.enabled)
        .map(|q| (q.key.as_str(), q.value.as_str()))
        .collect();
    if let Auth::ApiKey {
        key,
        value,
        location: ApiKeyLocation::Query,
    } = &req.auth
    {
        query.push((key.as_str(), value.as_str()));
    }
    if !query.is_empty() {
        url.query_pairs_mut().extend_pairs(query);
    }

    let method = reqwest::Method::from_bytes(req.method.as_str().as_bytes())
        .expect("every apix Method is a valid HTTP method");
    let mut builder = client.request(method, url);

    let multipart_body = matches!(req.body, Body::Multipart(_));
    for header in req.headers.iter().filter(|h| h.enabled) {
        // A multipart Content-Type without our boundary would break the body
        // (Postman collections often carry one); the engine writes its own.
        if multipart_body
            && header.key.eq_ignore_ascii_case(CONTENT_TYPE.as_str())
            && header.value.to_ascii_lowercase().starts_with("multipart/")
        {
            continue;
        }
        builder = builder.header(&header.key, &header.value);
    }

    builder = match &req.auth {
        // Callers resolve `Inherit` first; left over, it sends nothing.
        Auth::None | Auth::Inherit => builder,
        Auth::Bearer { token } => builder.bearer_auth(token),
        Auth::Basic { username, password } => builder.basic_auth(username, Some(password)),
        Auth::ApiKey {
            key,
            value,
            location: ApiKeyLocation::Header,
        } => builder.header(key, value),
        Auth::ApiKey {
            location: ApiKeyLocation::Query,
            ..
        } => builder,
        // Signed in `prepare`, answered in `send`, or turned into a bearer token first.
        Auth::Digest { .. } | Auth::OAuth1(_) | Auth::AwsSigV4(_) | Auth::OAuth2(_) => builder,
    };

    if let Some(content_type) = req.body.default_content_type()
        && !req.has_header(CONTENT_TYPE.as_str())
    {
        builder = builder.header(CONTENT_TYPE, content_type);
    }
    builder = match &req.body {
        Body::None => builder,
        Body::Json(text) | Body::Text(text) | Body::Xml(text) | Body::Html(text) => {
            builder.body(text.clone())
        }
        Body::Form(rows) => {
            let fields: Vec<(&str, &str)> = rows
                .iter()
                .filter(|r| r.enabled)
                .map(|r| (r.key.as_str(), r.value.as_str()))
                .collect();
            builder.form(&fields)
        }
        Body::Multipart(parts) => builder.multipart(multipart(parts)?),
        Body::Binary { file } => builder.body(read_file(file)?),
        Body::GraphQl(gql) => builder.body(Body::graphql_json(gql).map_err(HttpError::Body)?),
    };

    Ok(builder)
}

/// Reads a file the request sends. Files are read whole when sending.
fn read_file(path: &str) -> Result<Vec<u8>, HttpError> {
    if path.trim().is_empty() {
        return Err(HttpError::Body("choose a file to send".into()));
    }
    std::fs::read(path).map_err(|source| HttpError::File {
        path: path.to_owned(),
        source,
    })
}

fn multipart(parts: &[apix_core::FormPart]) -> Result<reqwest::multipart::Form, HttpError> {
    let mut form = reqwest::multipart::Form::new();
    for part in parts.iter().filter(|p| p.enabled && !p.key.is_empty()) {
        let mut piece = if part.file {
            let name = std::path::Path::new(&part.value)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file".into());
            reqwest::multipart::Part::bytes(read_file(&part.value)?)
                .file_name(name)
                .mime_str("application/octet-stream")
                .expect("a valid MIME type")
        } else {
            reqwest::multipart::Part::text(part.value.clone())
        };
        if let Some(mime) = part
            .content_type
            .as_deref()
            .filter(|m| !m.trim().is_empty())
        {
            piece = piece.mime_str(mime).map_err(|_| {
                HttpError::Body(format!(
                    "`{mime}` is not a valid content type for {}",
                    part.key
                ))
            })?;
        }
        form = form.part(part.key.clone(), piece);
    }
    Ok(form)
}

#[cfg(test)]
mod tests {
    use super::*;
    use apix_core::{KeyValue, Method};

    #[test]
    fn builds_query_auth_and_body() {
        let mut req = Request::new(Method::Post, "https://example.com/items?page=1");
        req.query.push(KeyValue::new("sort", "name"));
        req.query.push(KeyValue {
            enabled: false,
            ..KeyValue::new("debug", "1")
        });
        req.auth = Auth::ApiKey {
            key: "api_key".into(),
            value: "secret".into(),
            location: ApiKeyLocation::Query,
        };
        req.body = Body::Json("{}".into());

        let built = build(&reqwest::Client::new(), &req)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(
            built.url().as_str(),
            "https://example.com/items?page=1&sort=name&api_key=secret"
        );
        assert_eq!(built.headers()[CONTENT_TYPE], "application/json");
        assert_eq!(built.body().and_then(|b| b.as_bytes()), Some(&b"{}"[..]));
    }

    #[test]
    fn explicit_content_type_wins() {
        let mut req = Request::new(Method::Post, "https://example.com");
        req.headers
            .push(KeyValue::new("content-type", "application/vnd.api+json"));
        req.body = Body::Json("{}".into());
        let built = build(&reqwest::Client::new(), &req)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(built.headers()[CONTENT_TYPE], "application/vnd.api+json");
    }

    #[test]
    fn builds_multipart_binary_and_graphql_bodies() {
        let dir = std::env::temp_dir().join(format!("apix-http-body-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.bin");
        std::fs::write(&file, b"\x00\x01data").unwrap();
        let path = file.to_string_lossy().into_owned();
        let client = reqwest::Client::new();

        let mut req = Request::new(Method::Post, "https://example.com");
        req.headers
            .push(KeyValue::new("Content-Type", "multipart/form-data"));
        req.body = Body::Multipart(vec![
            apix_core::FormPart::text("name", "Ada"),
            apix_core::FormPart::file("doc", &path),
        ]);
        let built = build(&client, &req).unwrap().build().unwrap();
        let content_type = built.headers()[CONTENT_TYPE].to_str().unwrap();
        assert!(
            content_type.starts_with("multipart/form-data; boundary="),
            "the boundary-less header is replaced: {content_type}"
        );

        req.headers.clear();
        req.body = Body::Binary { file: path.clone() };
        let built = build(&client, &req).unwrap().build().unwrap();
        assert_eq!(built.headers()[CONTENT_TYPE], "application/octet-stream");
        assert_eq!(
            built.body().and_then(|b| b.as_bytes()),
            Some(&b"\x00\x01data"[..])
        );

        req.body = Body::Binary {
            file: dir.join("missing.bin").to_string_lossy().into_owned(),
        };
        assert!(matches!(build(&client, &req), Err(HttpError::File { .. })));

        req.body = Body::GraphQl(apix_core::GraphQl {
            query: "{ me }".into(),
            variables: "{\"a\":1}".into(),
        });
        let built = build(&client, &req).unwrap().build().unwrap();
        assert_eq!(built.headers()[CONTENT_TYPE], "application/json");
        assert_eq!(
            built.body().and_then(|b| b.as_bytes()),
            Some(&br#"{"query":"{ me }","variables":{"a":1}}"#[..])
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tells_body_kinds_apart() {
        let kind = |ct: Option<&str>, body: &[u8]| {
            let headers = ct
                .map(|c| vec![("Content-Type".to_owned(), c.to_owned())])
                .unwrap_or_default();
            let timing = Timing {
                ttfb: Duration::ZERO,
                total: Duration::ZERO,
            };
            Response::from_parts(200, "HTTP/1.1".into(), headers, body.to_vec(), timing).body_kind()
        };
        assert_eq!(
            kind(Some("application/json; charset=utf-8"), b"{}"),
            BodyKind::Json
        );
        assert_eq!(
            kind(Some("application/problem+json"), b"{}"),
            BodyKind::Json
        );
        assert_eq!(kind(Some("text/xml"), b"<a/>"), BodyKind::Xml);
        assert_eq!(kind(Some("application/atom+xml"), b"<a/>"), BodyKind::Xml);
        assert_eq!(
            kind(Some("text/html; charset=utf-8"), b"<p>"),
            BodyKind::Html
        );
        assert_eq!(
            kind(Some("image/png"), b"\x89PNG"),
            BodyKind::Image(ImageKind::Png)
        );
        assert_eq!(
            kind(Some("image/svg+xml"), b"<svg/>"),
            BodyKind::Image(ImageKind::Svg)
        );
        assert_eq!(kind(Some("text/plain"), b"hi"), BodyKind::Text);
        assert_eq!(
            kind(Some("application/pdf"), b"%PDF-1.7\n\x00\x01"),
            BodyKind::Binary
        );
        // No useful type: the bytes decide.
        assert_eq!(
            kind(Some("application/octet-stream"), b"\x89PNG\r\n\x1a\nrest"),
            BodyKind::Image(ImageKind::Png)
        );
        assert_eq!(
            kind(None, &[0xff, 0xd8, 0xff, 0xe0]),
            BodyKind::Image(ImageKind::Jpeg)
        );
        assert_eq!(kind(None, b"<!DOCTYPE html><html>"), BodyKind::Html);
        assert_eq!(kind(None, b"<?xml version=\"1.0\"?><a/>"), BodyKind::Xml);
        assert_eq!(kind(None, "plain café".as_bytes()), BodyKind::Text);
        assert_eq!(kind(None, &[1, 2, 0, 4]), BodyKind::Binary);
        assert_eq!(kind(None, b""), BodyKind::Text);
    }

    #[test]
    fn formats_human_numbers() {
        assert_eq!(group_digits(0), "0");
        assert_eq!(group_digits(999), "999");
        assert_eq!(group_digits(1_000), "1,000");
        assert_eq!(group_digits(9_656_462), "9,656,462");
        assert_eq!(human_size(1_536), "1.5 KB");
        assert_eq!(human_duration(Duration::from_micros(57_400)), "57 ms");
    }

    #[test]
    fn pretty_body_matches_serde_json_layout() {
        let raw = r#"[{"id":1,"name":"Leanne","address":{"geo":{"lat":"-37.3159"},"tags":[]},"ok":true,"n":null},{"id":2,"x":{}}]"#;
        let response = Response {
            status: 200,
            reason: Some("OK"),
            version: "HTTP/1.1".into(),
            headers: vec![(
                "Content-Type".into(),
                "application/json; charset=utf-8".into(),
            )],
            body: Bytes::from_static(raw.as_bytes()),
            timing: Timing {
                ttfb: Duration::ZERO,
                total: Duration::ZERO,
            },
            trace: NetTrace::default(),
        };
        let value: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert_eq!(
            response.pretty_body(),
            serde_json::to_string_pretty(&value).unwrap()
        );
    }
}
