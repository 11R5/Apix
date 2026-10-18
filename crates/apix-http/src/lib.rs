//! Turns a resolved [`apix_core::Request`] into a real HTTP call and captures
//! everything the UI needs to show: status, headers, raw body and timings.

mod local;
mod trace;

use std::sync::Arc;
use std::time::{Duration, Instant};

use apix_core::{ApiKeyLocation, Auth, Body, Request};
use bytes::Bytes;
use reqwest::header::CONTENT_TYPE;

pub use local::{LocalNetwork, local_network};
pub use trace::{Certificate, DnsLookup, NetTrace, Phases, Redirect, read_certificate};
use trace::{Recorder, TimedResolver, TimingLayer};

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("invalid URL `{url}`: {reason}")]
    InvalidUrl { url: String, reason: String },
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    /// reqwest names the first URL even when a redirect target failed.
    #[error("while following a redirect to {to}")]
    Redirected {
        to: String,
        #[source]
        source: reqwest::Error,
    },
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
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn is_json(&self) -> bool {
        self.header("content-type")
            .is_some_and(|ct| ct.contains("json"))
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

const USER_AGENT: &str = concat!("apix/", env!("CARGO_PKG_VERSION"));
/// Redirects followed before giving up, like browsers and reqwest's default.
const MAX_REDIRECTS: usize = 10;

/// Sends requests. Each send builds its own short-lived connection so its
/// network trace (DNS, connection setup, redirects) belongs to that request.
#[derive(Debug, Clone)]
pub struct HttpClient {
    _private: (),
}

impl HttpClient {
    pub fn new() -> Result<Self, HttpError> {
        Ok(Self { _private: () })
    }

    /// A client whose DNS lookups, connections and redirects are recorded.
    fn traced(recorder: &Arc<Recorder>) -> Result<reqwest::Client, HttpError> {
        let redirects = recorder.clone();
        let policy = reqwest::redirect::Policy::custom(move |attempt| {
            redirects.redirect(attempt.status().as_u16(), attempt.url().to_string());
            if attempt.previous().len() > MAX_REDIRECTS {
                attempt.error("too many redirects")
            } else {
                attempt.follow()
            }
        });
        Ok(reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .tls_info(true)
            .dns_resolver(Arc::new(TimedResolver(recorder.clone())))
            .connector_layer(TimingLayer(recorder.clone()))
            .redirect(policy)
            .build()?)
    }

    /// Sends a request whose `{{variables}}` have already been resolved.
    pub async fn send(&self, req: &Request) -> Result<Response, HttpError> {
        let recorder = Arc::new(Recorder::default());
        let client = Self::traced(&recorder)?;
        let builder = build(&client, req)?;
        let started = Instant::now();
        let response = match builder.send().await {
            Ok(response) => response,
            Err(source) => {
                return Err(match recorder.take().redirects.pop() {
                    Some(last) => HttpError::Redirected {
                        to: last.to,
                        source,
                    },
                    None => HttpError::Request(source),
                });
            }
        };
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
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    String::from_utf8_lossy(v.as_bytes()).into_owned(),
                )
            })
            .collect();
        let body = response.bytes().await?;
        let total = started.elapsed();

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

    for header in req.headers.iter().filter(|h| h.enabled) {
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
    };

    if let Some(content_type) = req.body.default_content_type()
        && !req.has_header(CONTENT_TYPE.as_str())
    {
        builder = builder.header(CONTENT_TYPE, content_type);
    }
    builder = match &req.body {
        Body::None => builder,
        Body::Json(text) | Body::Text(text) | Body::Xml(text) => builder.body(text.clone()),
        Body::Form(rows) => {
            let fields: Vec<(&str, &str)> = rows
                .iter()
                .filter(|r| r.enabled)
                .map(|r| (r.key.as_str(), r.value.as_str()))
                .collect();
            builder.form(&fields)
        }
    };

    Ok(builder)
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
