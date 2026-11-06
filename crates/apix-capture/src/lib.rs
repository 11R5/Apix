//! A recording proxy. Point a browser, an app or a script at it, and every
//! request that passes through becomes an Apix request, with the answer kept
//! as its example, so real traffic turns into a collection.
//!
//! Plain HTTP is recorded as it passes. HTTPS goes through a `CONNECT`
//! tunnel, which the proxy cannot read: by default it passes the tunnel on
//! untouched and only notes the host. With [`Options::open_https`] it
//! answers the TLS handshake itself with a certificate for the host signed
//! by a local authority ([`Authority`]), reads the request, and makes its own
//! HTTPS request to the server. That works only once the user has told their
//! system or browser to trust the authority's certificate; Apix never
//! installs it.
//!
//! Nothing secret is written: `Cookie` is dropped, and the values of
//! `Authorization` and API-key headers become `{{variables}}`.

use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use apix_core::{Body, Example, KeyValue, Method, Request};
use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::net::{TcpListener, TcpStream};

/// The local certificate authority that signs a certificate for each host
/// when HTTPS is opened.
pub struct Authority {
    issuer: Issuer<'static, KeyPair>,
    cert_der: CertificateDer<'static>,
    /// What the user adds to their trusted roots.
    pub cert_path: PathBuf,
    leaves: Mutex<std::collections::HashMap<String, Arc<rustls::ServerConfig>>>,
}

const CA_NAME: &str = "Apix capture authority (local)";

/// Where the authority is kept by default: `<config>/Apix`.
pub fn default_dir() -> Option<PathBuf> {
    apix_core::app_dir()
}

/// How to make the system trust the authority, for the user to run.
pub fn trust_hint(cert: &Path) -> String {
    let path = cert.display();
    if cfg!(windows) {
        format!("certutil -user -addstore Root \"{path}\"")
    } else if cfg!(target_os = "macos") {
        format!(
            "security add-trusted-cert -r trustRoot -k ~/Library/Keychains/login.keychain-db \"{path}\""
        )
    } else {
        format!(
            "sudo cp \"{path}\" /usr/local/share/ca-certificates/apix-capture.crt && sudo update-ca-certificates"
        )
    }
}

fn ca_params() -> CertificateParams {
    let mut params = CertificateParams::default();
    let mut name = DistinguishedName::new();
    name.push(DnType::CommonName, CA_NAME);
    name.push(DnType::OrganizationName, "Apix");
    params.distinguished_name = name;
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    params
}

fn io_error(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

/// The current year, for certificate dates.
fn this_year() -> i32 {
    let iso = apix_core::dynamic::iso8601(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64),
    );
    iso[..4].parse().unwrap_or(2026)
}

impl Authority {
    /// The authority kept in `dir` (`capture-ca.pem` and `capture-ca.key`),
    /// made there the first time.
    pub fn load_or_create(dir: &Path) -> std::io::Result<Self> {
        let cert_path = dir.join("capture-ca.pem");
        let key_path = dir.join("capture-ca.key");
        let (key, cert_pem) = match (
            std::fs::read_to_string(&key_path),
            std::fs::read_to_string(&cert_path),
        ) {
            (Ok(key), Ok(cert)) => (KeyPair::from_pem(&key).map_err(io_error)?, cert),
            _ => {
                let key = KeyPair::generate().map_err(io_error)?;
                let mut params = ca_params();
                let year = this_year();
                params.not_before = rcgen::date_time_ymd(year - 1, 1, 1);
                params.not_after = rcgen::date_time_ymd(year + 10, 1, 1);
                let cert = params.self_signed(&key).map_err(io_error)?;
                std::fs::create_dir_all(dir)?;
                std::fs::write(&key_path, key.serialize_pem())?;
                std::fs::write(&cert_path, cert.pem())?;
                (key, cert.pem())
            }
        };
        let cert_der = rustls::pki_types::pem::PemObject::from_pem_slice(cert_pem.as_bytes())
            .map_err(io_error)?;
        Ok(Self {
            // The same name and key as the certificate the user trusts.
            issuer: Issuer::new(ca_params(), key),
            cert_der,
            cert_path,
            leaves: Mutex::default(),
        })
    }

    /// A TLS server setup that presents a certificate for `host`.
    fn server_for(&self, host: &str) -> std::io::Result<Arc<rustls::ServerConfig>> {
        if let Some(config) = self.leaves.lock().ok().and_then(|l| l.get(host).cloned()) {
            return Ok(config);
        }
        let key = KeyPair::generate().map_err(io_error)?;
        let mut params = CertificateParams::new(vec![host.to_owned()]).map_err(io_error)?;
        params
            .distinguished_name
            .push(DnType::CommonName, host.to_owned());
        let year = this_year();
        params.not_before = rcgen::date_time_ymd(year - 1, 1, 1);
        params.not_after = rcgen::date_time_ymd(year + 1, 12, 31);
        let leaf = params.signed_by(&key, &self.issuer).map_err(io_error)?;
        let chain = vec![leaf.der().clone(), self.cert_der.clone()];
        let private = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der()));
        let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(io_error)?
        .with_no_client_auth()
        .with_single_cert(chain, private)
        .map_err(io_error)?;
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        let config = Arc::new(config);
        if let Ok(mut leaves) = self.leaves.lock() {
            leaves.insert(host.to_owned(), config.clone());
        }
        Ok(config)
    }
}

#[derive(Clone, Default)]
pub struct Options {
    /// Record only hosts that end with one of these (`example.com` takes
    /// `api.example.com`); empty records every host.
    pub hosts: Vec<String>,
    /// Open HTTPS with this authority instead of passing tunnels through.
    pub open_https: Option<Arc<Authority>>,
    /// More certificate authorities to trust when talking to servers (PEM).
    pub upstream_ca: Option<Vec<u8>>,
}

impl Options {
    fn wanted(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        self.hosts.is_empty()
            || self.hosts.iter().any(|h| {
                let h = h.trim().trim_start_matches('.').to_ascii_lowercase();
                host == h || host.ends_with(&format!(".{h}"))
            })
    }
}

/// What passed through the proxy.
#[derive(Debug, Clone)]
pub enum Event {
    /// A request and its answer.
    Exchange(Box<Exchange>),
    /// An HTTPS tunnel passed on unread.
    Tunnel { host: String },
    /// A request the proxy could not forward.
    Failed { url: String, error: String },
}

#[derive(Debug, Clone)]
pub struct Exchange {
    /// As it will be saved: secrets turned into variables, no cookies.
    pub request: Request,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Bytes,
    pub took: Duration,
}

/// Headers that belong to one connection, not to the request.
const HOP: [&str; 9] = [
    "connection",
    "keep-alive",
    "proxy-connection",
    "proxy-authorization",
    "proxy-authenticate",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

/// Header values that are credentials: kept out of the saved request.
const SECRET: [&str; 6] = [
    "authorization",
    "x-api-key",
    "api-key",
    "x-auth-token",
    "x-access-token",
    "x-csrf-token",
];

/// The request as it is saved: `Cookie` dropped, credentials as variables.
pub fn to_request(method: &str, url: &str, headers: &[(String, String)], body: &[u8]) -> Request {
    let method: Method = method.parse().unwrap_or(Method::Get);
    let mut request = Request::new(method, url);
    let path = url
        .split("://")
        .nth(1)
        .and_then(|rest| rest.find('/').map(|i| &rest[i..]))
        .unwrap_or("/");
    request.name = format!(
        "{} {}",
        method.as_str(),
        path.split('?').next().unwrap_or("/")
    );
    let content_type = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.to_ascii_lowercase())
        .unwrap_or_default();
    for (key, value) in headers {
        let lower = key.to_ascii_lowercase();
        if HOP.contains(&lower.as_str())
            || ["host", "content-length", "cookie", "accept-encoding"].contains(&lower.as_str())
        {
            continue;
        }
        let value = if SECRET.contains(&lower.as_str()) {
            format!("{{{{{}}}}}", lower.replace('-', "_"))
        } else {
            value.clone()
        };
        request.headers.push(KeyValue::new(key.clone(), value));
    }
    if !body.is_empty() {
        request.body = match std::str::from_utf8(body) {
            Ok(text) if content_type.contains("json") => Body::Json(text.to_owned()),
            Ok(text) if content_type.contains("xml") => Body::Xml(text.to_owned()),
            Ok(text) => Body::Text(text.to_owned()),
            // A binary body is not kept in the file.
            Err(_) => Body::None,
        };
    }
    request
}

/// Writes `exchange` into `dir` as a request file with its answer as the
/// example `Captured`. Returns the file.
pub fn save(exchange: &Exchange, dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let host = exchange
        .request
        .url
        .split("://")
        .nth(1)
        .and_then(|r| r.split(['/', ':']).next())
        .unwrap_or("request");
    let stem = apix_core::store::slug(&format!("{host} {}", exchange.request.name), "request");
    let path = apix_core::store::unique_path(dir, &stem, apix_core::store::REQUEST_EXTENSION);
    let mut request = exchange.request.clone();
    if exchange.body.len() <= apix_core::model::MAX_EXAMPLE_BODY {
        request.examples.push(Example::new(
            "Captured",
            exchange.status,
            &exchange.headers,
            &exchange.body,
            Some(&exchange.request),
        ));
    }
    apix_core::store::save_request(&path, &request).map_err(io_error)?;
    Ok(path)
}

struct Proxy {
    options: Options,
    client: reqwest::Client,
    on_event: Arc<dyn Fn(Event) + Send + Sync>,
}

/// Runs the proxy on `listener` until the future is dropped; `on_event`
/// hears about everything that passes.
pub async fn run(
    listener: TcpListener,
    options: Options,
    on_event: impl Fn(Event) + Send + Sync + 'static,
) -> std::io::Result<()> {
    let mut builder = reqwest::Client::builder()
        // Never through a proxy (that could be this one).
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(120));
    if let Some(pem) = &options.upstream_ca {
        for cert in reqwest::Certificate::from_pem_bundle(pem).map_err(io_error)? {
            builder = builder.add_root_certificate(cert);
        }
    }
    let proxy = Arc::new(Proxy {
        options,
        client: builder.build().map_err(io_error)?,
        on_event: Arc::new(on_event),
    });
    loop {
        let (stream, _) = listener.accept().await?;
        let proxy = proxy.clone();
        tokio::spawn(async move {
            let service = service_fn(move |request| {
                let proxy = proxy.clone();
                async move { Ok::<_, Infallible>(proxy.outer(request).await) }
            });
            let _ = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .with_upgrades()
                .await;
        });
    }
}

fn text(status: u16, message: &str) -> hyper::Response<Full<Bytes>> {
    let mut response = hyper::Response::new(Full::new(Bytes::from(message.to_owned())));
    *response.status_mut() = hyper::StatusCode::from_u16(status).unwrap_or_default();
    response
}

impl Proxy {
    /// A request to the proxy itself: `CONNECT host:port`, or a plain HTTP
    /// request with a whole URL.
    async fn outer(
        self: Arc<Self>,
        request: hyper::Request<Incoming>,
    ) -> hyper::Response<Full<Bytes>> {
        if request.method() == hyper::Method::CONNECT {
            let Some(authority) = request.uri().authority().map(|a| a.to_string()) else {
                return text(400, "CONNECT needs host:port");
            };
            let host = authority.split(':').next().unwrap_or_default().to_owned();
            let open = self
                .options
                .open_https
                .clone()
                .filter(|_| self.options.wanted(&host));
            tokio::spawn(async move {
                let Ok(upgraded) = hyper::upgrade::on(request).await else {
                    return;
                };
                let client = TokioIo::new(upgraded);
                match open {
                    Some(authority_ca) => {
                        let Ok(config) = authority_ca.server_for(&host) else {
                            return;
                        };
                        let Ok(tls) = tokio_rustls::TlsAcceptor::from(config).accept(client).await
                        else {
                            return;
                        };
                        let base = format!("https://{authority}");
                        let base = base.trim_end_matches(":443").to_owned();
                        let proxy = self.clone();
                        let service = service_fn(move |request| {
                            let (proxy, base) = (proxy.clone(), base.clone());
                            async move {
                                let path = request
                                    .uri()
                                    .path_and_query()
                                    .map_or("/".to_owned(), |p| p.as_str().to_owned());
                                Ok::<_, Infallible>(
                                    proxy.forward(request, format!("{base}{path}")).await,
                                )
                            }
                        });
                        let _ = hyper::server::conn::http1::Builder::new()
                            .serve_connection(TokioIo::new(tls), service)
                            .await;
                    }
                    None => {
                        if self.options.wanted(&host) {
                            (self.on_event)(Event::Tunnel { host: host.clone() });
                        }
                        let Ok(mut server) = TcpStream::connect(&authority).await else {
                            return;
                        };
                        let mut client = client;
                        let _ = tokio::io::copy_bidirectional(&mut client, &mut server).await;
                    }
                }
            });
            return hyper::Response::new(Full::new(Bytes::new()));
        }
        let uri = request.uri().clone();
        if uri.scheme().is_none() {
            return text(
                400,
                "This is the Apix capture proxy: set it as the HTTP proxy rather than calling it directly.",
            );
        }
        self.forward(request, uri.to_string()).await
    }

    /// Sends `request` on to `url`, records it, and returns the answer.
    async fn forward(
        &self,
        request: hyper::Request<Incoming>,
        url: String,
    ) -> hyper::Response<Full<Bytes>> {
        let started = Instant::now();
        let method = request.method().as_str().to_owned();
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
        let body = match request.into_body().collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(e) => return text(400, &format!("cannot read the request body: {e}")),
        };
        let host = url
            .split("://")
            .nth(1)
            .and_then(|r| r.split(['/', ':', '?']).next())
            .unwrap_or_default()
            .to_owned();

        let mut outgoing = self.client.request(
            reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &url,
        );
        for (key, value) in &headers {
            let lower = key.to_ascii_lowercase();
            // reqwest asks for and decodes compressed answers itself.
            if HOP.contains(&lower.as_str())
                || ["host", "content-length", "accept-encoding"].contains(&lower.as_str())
            {
                continue;
            }
            outgoing = outgoing.header(key, value);
        }
        let answer = match outgoing.body(body.clone()).send().await {
            Ok(answer) => answer,
            Err(e) => {
                let error = format!("{e}");
                if self.options.wanted(&host) {
                    (self.on_event)(Event::Failed {
                        url: apix_core::display_url(&url),
                        error: error.clone(),
                    });
                }
                return text(
                    502,
                    &format!("Apix capture could not reach the server: {error}"),
                );
            }
        };
        let status = answer.status().as_u16();
        let answer_headers: Vec<(String, String)> = answer
            .headers()
            .iter()
            .filter(|(k, _)| {
                let k = k.as_str();
                !HOP.contains(&k) && k != "content-length" && k != "content-encoding"
            })
            .map(|(k, v)| {
                (
                    k.to_string(),
                    String::from_utf8_lossy(v.as_bytes()).into_owned(),
                )
            })
            .collect();
        let answer_body = answer.bytes().await.unwrap_or_default();

        if self.options.wanted(&host) {
            (self.on_event)(Event::Exchange(Box::new(Exchange {
                request: to_request(&method, &url, &headers, &body),
                status,
                headers: answer_headers.clone(),
                body: answer_body.clone(),
                took: started.elapsed(),
            })));
        }
        let mut response = hyper::Response::new(Full::new(answer_body));
        *response.status_mut() = hyper::StatusCode::from_u16(status).unwrap_or_default();
        for (key, value) in answer_headers {
            if let (Ok(k), Ok(v)) = (
                hyper::header::HeaderName::from_bytes(key.as_bytes()),
                hyper::header::HeaderValue::from_str(&value),
            ) {
                response.headers_mut().append(k, v);
            }
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_keep_no_secrets() {
        let headers = vec![
            ("Host".to_owned(), "api.test".to_owned()),
            ("Authorization".to_owned(), "Bearer s3cret".to_owned()),
            ("Cookie".to_owned(), "session=abc".to_owned()),
            ("X-Api-Key".to_owned(), "k".to_owned()),
            ("Content-Type".to_owned(), "application/json".to_owned()),
            ("Proxy-Connection".to_owned(), "keep-alive".to_owned()),
        ];
        let r = to_request("POST", "http://api.test/users?x=1", &headers, b"{\"a\":1}");
        assert_eq!(r.name, "POST /users");
        assert_eq!(r.method, Method::Post);
        let names: Vec<&str> = r.headers.iter().map(|h| h.key.as_str()).collect();
        assert_eq!(names, ["Authorization", "X-Api-Key", "Content-Type"]);
        assert_eq!(r.headers[0].value, "{{authorization}}");
        assert_eq!(r.headers[1].value, "{{x_api_key}}");
        assert_eq!(r.body, Body::Json("{\"a\":1}".into()));
        assert_eq!(
            to_request("GET", "http://x", &[], &[0xff, 0xfe]).body,
            Body::None
        );
    }

    #[test]
    fn host_filters() {
        let options = Options {
            hosts: vec!["example.com".into()],
            ..Options::default()
        };
        assert!(options.wanted("api.example.com"));
        assert!(options.wanted("EXAMPLE.com"));
        assert!(!options.wanted("badexample.com"));
        assert!(Options::default().wanted("anything"));
    }
}
