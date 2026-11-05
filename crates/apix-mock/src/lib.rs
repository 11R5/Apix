//! A local mock server: it answers requests with the examples saved with a
//! collection's requests (`[[example]]`), so a front end can be built, or a
//! test run, before the real API exists or while it is down.
//!
//! A request is matched by method and path. The path is the request's URL
//! after its host, or after a leading `{{variable}}` (`{{base_url}}/users/:id`
//! serves `/users/<anything>`); `:name` and `{{variable}}` segments match any
//! one segment. The example's own request URL, when it was saved, wins over
//! the request's, so two examples can answer two different paths. Among
//! several matches the most literal path wins, then:
//!
//! - `x-mock-response-name: <example name>` picks an example by name, and
//!   `x-mock-response-code: 404` by status (the headers Postman's mock
//!   servers read);
//! - otherwise the first example with a 2xx status, else the first.
//!
//! Every answer allows any origin (CORS), and a preflight `OPTIONS` without
//! an example of its own gets `204`, so a page in a browser can call it.
//! The collection is read again when its files change.

use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use apix_core::{Collection, Example, Method};
use base64::Engine as _;
use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

/// One part of a path pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Literal(String),
    /// `:id` or `{{id}}`: any one segment.
    Any,
}

/// The path pattern of a request URL as written.
pub fn pattern(url: &str) -> Vec<Segment> {
    let mut rest = url.trim();
    if let Some(i) = rest.find(['?', '#']) {
        rest = &rest[..i];
    }
    if let Some(i) = rest.find("://") {
        rest = &rest[i + 3..];
        rest = rest.find('/').map_or("", |j| &rest[j..]);
    } else if rest.starts_with("{{") {
        // `{{base_url}}/users`: the variable is the server.
        if let Some(end) = rest.find("}}") {
            rest = &rest[end + 2..];
        }
    } else if !rest.starts_with('/') {
        // `localhost:3000/users`
        rest = rest.find('/').map_or("", |j| &rest[j..]);
    }
    rest.split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            if s.starts_with(':') || s.contains("{{") {
                Segment::Any
            } else {
                Segment::Literal(s.to_owned())
            }
        })
        .collect()
}

/// How many segments of `path` the pattern matches literally, or `None` when
/// it does not match.
fn matches(pattern: &[Segment], path: &[&str]) -> Option<usize> {
    if pattern.len() != path.len() {
        return None;
    }
    let mut literal = 0;
    for (segment, part) in pattern.iter().zip(path) {
        match segment {
            Segment::Any => {}
            Segment::Literal(text) if text == part => literal += 1,
            Segment::Literal(text) if percent_decode(text) == percent_decode(part) => literal += 1,
            Segment::Literal(_) => return None,
        }
    }
    Some(literal)
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(v) = u8::from_str_radix(&text[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// One example the server can answer with.
#[derive(Debug, Clone)]
pub struct Route {
    pub method: Method,
    pub pattern: Vec<Segment>,
    /// The path as written, for listing (`/users/:id`).
    pub path: String,
    /// The request file, relative to the collection, with `/`.
    pub request: String,
    pub example: Example,
}

/// Every example of the collection's requests, in sidebar order.
pub fn routes(collection: &Collection) -> Vec<Route> {
    let mut out = Vec::new();
    for path in collection.request_paths() {
        let Ok(request) = apix_core::store::load_request(&path) else {
            continue;
        };
        let label = path
            .strip_prefix(&collection.root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        for example in &request.examples {
            let (method, url) = match &example.request {
                Some(r) if !r.url.trim().is_empty() => (r.method, r.url.as_str()),
                _ => (request.method, request.url.as_str()),
            };
            let pattern = pattern(url);
            let path = display_path(url);
            out.push(Route {
                method,
                pattern,
                path,
                request: label.clone(),
                example: example.clone(),
            });
        }
    }
    out
}

/// `/users/:id` for `{{base_url}}/users/:id?x=1`: the path a route serves, as written.
pub fn display_path(url: &str) -> String {
    let pattern = pattern(url);
    format!(
        "/{}",
        pattern
            .iter()
            .zip(url_segments(url))
            .map(|(s, raw)| match s {
                Segment::Literal(text) => text.clone(),
                Segment::Any => raw,
            })
            .collect::<Vec<_>>()
            .join("/")
    )
}

/// The raw segments `pattern` was made from, for display.
fn url_segments(url: &str) -> Vec<String> {
    let probe = pattern(url);
    let mut rest = url.trim();
    if let Some(i) = rest.find(['?', '#']) {
        rest = &rest[..i];
    }
    let parts: Vec<String> = rest
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    parts[parts.len().saturating_sub(probe.len())..].to_vec()
}

/// What the server answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// The request file and example that answered.
    pub matched: Option<(String, String)>,
}

/// Picks the answer to `method path` among `routes`; `headers` may choose an
/// example by name or status.
pub fn answer(routes: &[Route], method: &str, path: &str, headers: &[(String, String)]) -> Answer {
    let header = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.trim())
    };
    let path = path.split(['?', '#']).next().unwrap_or("/");
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut found: Vec<(usize, &Route)> = routes
        .iter()
        .filter(|r| r.method.as_str().eq_ignore_ascii_case(method))
        .filter_map(|r| matches(&r.pattern, &parts).map(|n| (n, r)))
        .collect();
    let best = found.iter().map(|(n, _)| *n).max();
    found.retain(|(n, _)| Some(*n) == best);
    let candidates: Vec<&Route> = found.into_iter().map(|(_, r)| r).collect();

    let chosen = if let Some(name) = header("x-mock-response-name") {
        candidates.iter().find(|r| r.example.name == name)
    } else if let Some(code) = header("x-mock-response-code").and_then(|c| c.parse::<u16>().ok()) {
        candidates.iter().find(|r| r.example.status == code)
    } else {
        candidates
            .iter()
            .find(|r| (200..300).contains(&r.example.status))
            .or(candidates.first())
    };

    match chosen {
        Some(route) => {
            let example = &route.example;
            let body = if example.body_base64.is_empty() {
                example.body.clone().into_bytes()
            } else {
                base64::engine::general_purpose::STANDARD
                    .decode(example.body_base64.trim())
                    .unwrap_or_default()
            };
            let mut headers: Vec<(String, String)> = example
                .headers
                .iter()
                .filter(|h| h.enabled)
                // The body is kept decoded, and its length is ours to set.
                .filter(|h| {
                    ![
                        "content-length",
                        "transfer-encoding",
                        "content-encoding",
                        "connection",
                    ]
                    .iter()
                    .any(|skip| h.key.eq_ignore_ascii_case(skip))
                })
                .map(|h| (h.key.clone(), h.value.clone()))
                .collect();
            headers.push((
                "X-Apix-Mock".into(),
                format!("{} · {}", route.request, example.name),
            ));
            Answer {
                status: if example.status == 0 {
                    200
                } else {
                    example.status
                },
                headers,
                body,
                matched: Some((route.request.clone(), example.name.clone())),
            }
        }
        None if method.eq_ignore_ascii_case("OPTIONS") => Answer {
            status: 204,
            headers: Vec::new(),
            body: Vec::new(),
            matched: None,
        },
        None => {
            let known: Vec<String> = routes
                .iter()
                .map(|r| format!("{} {}", r.method.as_str(), r.path))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            let why = if candidates_for_path(routes, &parts) {
                "an example matches the path, but not with this method or the asked name or code"
            } else {
                "no saved example matches this method and path"
            };
            let body = serde_json::to_vec_pretty(&serde_json::json!({
                "error": why,
                "request": format!("{method} {path}"),
                "examples": known,
            }))
            .unwrap_or_default();
            Answer {
                status: 404,
                headers: vec![("Content-Type".into(), "application/json".into())],
                body,
                matched: None,
            }
        }
    }
}

fn candidates_for_path(routes: &[Route], parts: &[&str]) -> bool {
    routes.iter().any(|r| matches(&r.pattern, parts).is_some())
}

/// What the server did with one request.
#[derive(Debug, Clone)]
pub struct Hit {
    pub method: String,
    pub path: String,
    pub status: u16,
    /// The request file and example that answered.
    pub matched: Option<(String, String)>,
    pub at: SystemTime,
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Wait this long before answering, like a slow server.
    pub delay: Duration,
}

/// The routes, read again when the collection's files changed.
struct Routes {
    root: PathBuf,
    read_at: Instant,
    stamp: Option<SystemTime>,
    routes: Arc<Vec<Route>>,
}

impl Routes {
    fn new(root: &Path) -> Self {
        let mut routes = Self {
            root: root.to_path_buf(),
            read_at: Instant::now(),
            stamp: None,
            routes: Arc::default(),
        };
        routes.reload();
        routes
    }

    /// The newest modification time of the collection's files.
    fn stamp(&self) -> Option<SystemTime> {
        fn walk(dir: &Path, newest: &mut Option<SystemTime>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if entry.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        *newest = (*newest).max(Some(modified));
                    }
                    if meta.is_dir() {
                        walk(&path, newest);
                    }
                }
            }
        }
        let mut newest = None;
        walk(&self.root, &mut newest);
        newest
    }

    fn reload(&mut self) {
        self.stamp = self.stamp();
        self.routes = Arc::new(
            Collection::open(&self.root)
                .map(|c| routes(&c))
                .unwrap_or_default(),
        );
    }

    /// The routes, looking for changes at most once a second.
    fn current(&mut self) -> Arc<Vec<Route>> {
        if self.read_at.elapsed() >= Duration::from_secs(1) {
            self.read_at = Instant::now();
            if self.stamp() != self.stamp {
                self.reload();
            }
        }
        self.routes.clone()
    }
}

/// The routes the collection at `root` gives now.
pub fn load(root: &Path) -> Vec<Route> {
    Collection::open(root)
        .map(|c| routes(&c))
        .unwrap_or_default()
}

/// Serves the collection at `root` on `listener` until the future is dropped.
/// `on_hit` hears about every request.
pub async fn serve(
    listener: TcpListener,
    root: PathBuf,
    options: Options,
    on_hit: impl Fn(Hit) + Send + Sync + 'static,
) -> std::io::Result<()> {
    let routes = Arc::new(Mutex::new(Routes::new(&root)));
    let on_hit: Arc<dyn Fn(Hit) + Send + Sync> = Arc::new(on_hit);
    loop {
        let (stream, _) = listener.accept().await?;
        let (routes, on_hit, options) = (routes.clone(), on_hit.clone(), options.clone());
        tokio::spawn(async move {
            let service = service_fn(move |request: hyper::Request<Incoming>| {
                let (routes, on_hit, options) = (routes.clone(), on_hit.clone(), options.clone());
                async move {
                    let method = request.method().as_str().to_owned();
                    let path = request
                        .uri()
                        .path_and_query()
                        .map_or("/".to_owned(), |p| p.as_str().to_owned());
                    let headers: Vec<(String, String)> = request
                        .headers()
                        .iter()
                        .map(|(k, v)| {
                            (
                                k.to_string(),
                                String::from_utf8_lossy(v.as_bytes()).into_owned(),
                            )
                        })
                        .collect();
                    let current = routes.lock().map(|mut r| r.current()).unwrap_or_default();
                    let answer = answer(&current, &method, &path, &headers);
                    if !options.delay.is_zero() {
                        tokio::time::sleep(options.delay).await;
                    }
                    on_hit(Hit {
                        method,
                        path,
                        status: answer.status,
                        matched: answer.matched.clone(),
                        at: SystemTime::now(),
                    });
                    Ok::<_, Infallible>(to_response(answer))
                }
            });
            let _ = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await;
        });
    }
}

fn to_response(answer: Answer) -> hyper::Response<Full<Bytes>> {
    let mut response = hyper::Response::new(Full::new(Bytes::from(answer.body)));
    *response.status_mut() =
        hyper::StatusCode::from_u16(answer.status).unwrap_or(hyper::StatusCode::OK);
    let headers = response.headers_mut();
    for (key, value) in answer.headers {
        if let (Ok(k), Ok(v)) = (
            hyper::header::HeaderName::from_bytes(key.as_bytes()),
            hyper::header::HeaderValue::from_str(&value),
        ) {
            headers.append(k, v);
        }
    }
    for (key, value) in [
        ("access-control-allow-origin", "*"),
        ("access-control-allow-methods", "*"),
        ("access-control-allow-headers", "*"),
        ("access-control-expose-headers", "*"),
    ] {
        headers
            .entry(key)
            .or_insert(hyper::header::HeaderValue::from_static(value));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use apix_core::KeyValue;

    fn route(method: Method, url: &str, name: &str, status: u16, body: &str) -> Route {
        Route {
            method,
            pattern: pattern(url),
            path: display_path(url),
            request: "r.apix".into(),
            example: Example {
                name: name.into(),
                status,
                headers: vec![
                    KeyValue::new("Content-Type", "application/json"),
                    KeyValue::new("Content-Encoding", "gzip"),
                ],
                body: body.into(),
                ..Example::default()
            },
        }
    }

    #[test]
    fn patterns() {
        use Segment::*;
        assert_eq!(
            pattern("{{base_url}}/users/:id?x=1"),
            vec![Literal("users".into()), Any]
        );
        assert_eq!(
            pattern("https://api.test/v1/{{org}}/items"),
            vec![Literal("v1".into()), Any, Literal("items".into())]
        );
        assert_eq!(
            pattern("localhost:3000/health"),
            vec![Literal("health".into())]
        );
        assert_eq!(pattern("{{base}}"), Vec::<Segment>::new());
    }

    #[test]
    fn answers_pick_the_most_specific_and_the_asked_example() {
        let routes = vec![
            route(Method::Get, "{{b}}/users/:id", "Found", 200, "{\"id\":1}"),
            route(Method::Get, "{{b}}/users/:id", "Missing", 404, "{}"),
            route(Method::Get, "{{b}}/users/me", "Me", 200, "{\"me\":true}"),
            route(Method::Post, "{{b}}/users", "Created", 201, "{}"),
        ];
        let got = answer(&routes, "GET", "/users/7?full=1", &[]);
        assert_eq!(got.status, 200);
        assert_eq!(got.body, b"{\"id\":1}");
        assert!(got.headers.iter().any(|(k, _)| k == "Content-Type"));
        assert!(!got.headers.iter().any(|(k, _)| k == "Content-Encoding"));
        assert_eq!(
            answer(&routes, "GET", "/users/me", &[]).body,
            b"{\"me\":true}"
        );
        let by_name = answer(
            &routes,
            "GET",
            "/users/7",
            &[("X-Mock-Response-Name".into(), "Missing".into())],
        );
        assert_eq!(by_name.status, 404);
        let by_code = answer(
            &routes,
            "get",
            "/users/7",
            &[("x-mock-response-code".into(), "404".into())],
        );
        assert_eq!(by_code.matched.unwrap().1, "Missing");
        assert_eq!(answer(&routes, "POST", "/users", &[]).status, 201);
        let none = answer(&routes, "DELETE", "/users/7", &[]);
        assert_eq!(none.status, 404);
        let text = String::from_utf8(none.body).unwrap();
        assert!(text.contains("not with this method"), "{text}");
        assert!(text.contains("GET /users/:id"));
        assert_eq!(answer(&routes, "OPTIONS", "/users/7", &[]).status, 204);
        assert_eq!(percent_decode("a%20b"), "a b");
    }
}
