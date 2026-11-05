//! What happened on the network while a request was sent: DNS answers and
//! time, connection setup time, redirects, the local and remote address and
//! the server certificate. Each send uses its own short-lived client, so
//! every event recorded here belongs to that one request and the times are
//! those of a real, fresh connection.
//!
//! TLS details (version, cipher suite, key exchange group, the whole
//! certificate chain, when the handshake started) come from the request's
//! own handshake through rustls' hooks; see `tls_trace.rs`.

use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

/// One DNS lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsLookup {
    pub host: String,
    pub addrs: Vec<IpAddr>,
    pub duration: Duration,
}

/// One followed redirect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirect {
    pub status: u16,
    pub to: String,
}

/// One certificate the server sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate {
    pub subject: String,
    pub issuer: String,
    /// `ECDSA P-256`, `RSA 2048` …
    pub key: String,
    /// The SHA-256 of the certificate, as colon-separated hex.
    pub sha256: String,
    /// `2026-01-02` (UTC).
    pub not_before: String,
    pub not_after: String,
    /// Negative once expired.
    pub days_left: i64,
    /// Host names the certificate is valid for.
    pub names: Vec<String>,
}

/// One new connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Connect {
    /// From asking for a connection to having it: DNS, TCP and TLS together.
    pub total: Duration,
    /// The TLS handshake alone (from the client's hello to the end).
    pub tls: Option<Duration>,
}

/// What the TLS handshake settled on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TlsDetails {
    /// The name the client asked for (SNI).
    pub server_name: String,
    /// `TLS 1.3` or `TLS 1.2`.
    pub version: Option<String>,
    /// `TLS13_AES_128_GCM_SHA256` …
    pub cipher: Option<String>,
    /// The key exchange group, e.g. `X25519` (TLS 1.3).
    pub group: Option<String>,
    /// The application protocol agreed through ALPN: `h2` or `http/1.1`.
    pub alpn: Option<String>,
    /// The certificates as the server sent them, its own first.
    pub chain: Vec<Certificate>,
    /// How long the handshake took.
    pub handshake: Option<Duration>,
}

/// A body sent compressed (`Content-Encoding`), decoded for showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compression {
    /// `gzip`, `br` or `deflate`.
    pub encoding: String,
    /// Bytes on the wire.
    pub encoded: u64,
    /// Bytes after decoding.
    pub decoded: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetTrace {
    pub dns: Vec<DnsLookup>,
    /// One per new connection.
    pub connects: Vec<Connect>,
    pub redirects: Vec<Redirect>,
    pub local_addr: Option<SocketAddr>,
    pub remote_addr: Option<SocketAddr>,
    /// The server's own certificate (the first of `tls.chain`).
    pub certificate: Option<Certificate>,
    /// The last TLS handshake (the one of the final response).
    pub tls: Option<TlsDetails>,
    pub compression: Option<Compression>,
}

/// Where the time went, for the waterfall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phases {
    pub dns: Duration,
    /// Opening the TCP connection.
    pub tcp: Duration,
    /// The TLS handshake.
    pub tls: Duration,
    /// From the request leaving to the first byte of the answer.
    pub waiting: Duration,
    pub download: Duration,
}

impl NetTrace {
    /// Splits the total time into phases. `ttfb` and `total` are measured
    /// from the start of the send; DNS happens inside connection setup.
    pub fn phases(&self, ttfb: Duration, total: Duration) -> Phases {
        let dns: Duration = self.dns.iter().map(|d| d.duration).sum();
        let setup: Duration = self.connects.iter().map(|c| c.total).sum();
        let tls: Duration = self.connects.iter().filter_map(|c| c.tls).sum();
        let tls = tls.min(setup.saturating_sub(dns));
        Phases {
            dns,
            tcp: setup.saturating_sub(dns).saturating_sub(tls),
            tls,
            waiting: ttfb.saturating_sub(setup.max(dns)),
            download: total.saturating_sub(ttfb),
        }
    }
}

/// Collects events from the resolver, the connector, the TLS handshake and
/// the redirect policy.
#[derive(Debug, Default)]
pub(crate) struct Recorder(Mutex<State>);

#[derive(Debug, Default)]
struct State {
    trace: NetTrace,
    /// When the TLS handshake in progress began, if one is.
    tls_started: Option<Instant>,
}

impl Recorder {
    fn with(&self, f: impl FnOnce(&mut NetTrace)) {
        if let Ok(mut state) = self.0.lock() {
            f(&mut state.trace);
        }
    }

    /// The client is about to say hello to `server_name`.
    pub(crate) fn tls_started(&self, server_name: String) {
        if let Ok(mut state) = self.0.lock() {
            // A second call within one handshake (after a retry request) changes nothing.
            if state.tls_started.is_none() {
                state.tls_started = Some(Instant::now());
                state.trace.tls = Some(TlsDetails {
                    server_name,
                    ..TlsDetails::default()
                });
            }
        }
    }

    /// Something learnt about the handshake in progress.
    pub(crate) fn tls(&self, f: impl FnOnce(&mut TlsDetails)) {
        self.with(|t| {
            if let Some(tls) = &mut t.tls {
                f(tls);
            }
        });
    }

    pub(crate) fn chain(&self, chain: Vec<Certificate>) {
        self.with(|t| {
            t.certificate = chain.first().cloned();
            if let Some(tls) = &mut t.tls {
                tls.chain = chain;
            }
        });
    }

    pub(crate) fn compression(&self, compression: Compression) {
        self.with(|t| t.compression = Some(compression));
    }

    /// A connection was set up `took` after it was asked for at `started`.
    fn connected(&self, started: Instant, took: Duration) {
        if let Ok(mut state) = self.0.lock() {
            let tls = state
                .tls_started
                .take()
                .filter(|t| *t >= started)
                .map(|t| (started + took).saturating_duration_since(t));
            if let (Some(took), Some(details)) = (tls, &mut state.trace.tls) {
                details.handshake = Some(took);
            }
            state.trace.connects.push(Connect { total: took, tls });
        }
    }

    pub(crate) fn redirect(&self, status: u16, to: String) {
        self.with(|t| t.redirects.push(Redirect { status, to }));
    }

    pub(crate) fn take(&self) -> NetTrace {
        self.0
            .lock()
            .map(|mut s| std::mem::take(&mut s.trace))
            .unwrap_or_default()
    }
}

/// Resolves like the system does (`getaddrinfo`), timing each lookup.
pub(crate) struct TimedResolver(pub(crate) Arc<Recorder>);

impl Resolve for TimedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let recorder = self.0.clone();
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let started = Instant::now();
            let addrs: Vec<SocketAddr> =
                tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
            let duration = started.elapsed();
            recorder.with(|t| {
                t.dns.push(DnsLookup {
                    host,
                    addrs: addrs.iter().map(SocketAddr::ip).collect(),
                    duration,
                })
            });
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

/// A Tower layer around reqwest's connector that times each connection setup.
#[derive(Clone)]
pub(crate) struct TimingLayer(pub(crate) Arc<Recorder>);

impl<S> tower_layer::Layer<S> for TimingLayer {
    type Service = Timed<S>;

    fn layer(&self, inner: S) -> Timed<S> {
        Timed {
            inner,
            recorder: self.0.clone(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct Timed<S> {
    inner: S,
    recorder: Arc<Recorder>,
}

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

impl<S, R> tower_service::Service<R> for Timed<S>
where
    S: tower_service::Service<R>,
    S::Future: Send + 'static,
    S::Response: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<Result<S::Response, S::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: R) -> Self::Future {
        let started = Instant::now();
        let connecting = self.inner.call(request);
        let recorder = self.recorder.clone();
        Box::pin(async move {
            let connected = connecting.await;
            if connected.is_ok() {
                recorder.connected(started, started.elapsed());
            }
            connected
        })
    }
}

/// Reads the parts of a DER certificate worth showing.
pub fn read_certificate(der: &[u8]) -> Option<Certificate> {
    use x509_parser::prelude::*;

    let (_, cert) = parse_x509_certificate(der).ok()?;
    let name = |n: &X509Name<'_>| -> String {
        let cn = n.iter_common_name().next().and_then(|a| a.as_str().ok());
        let org = n.iter_organization().next().and_then(|a| a.as_str().ok());
        match (cn, org) {
            (Some(cn), Some(org)) if cn != org => format!("{cn} ({org})"),
            (Some(cn), _) => cn.to_owned(),
            (None, Some(org)) => org.to_owned(),
            (None, None) => n.to_string(),
        }
    };
    let date = |t: &ASN1Time| -> String {
        apix_core::dynamic::iso8601(t.timestamp() * 1000)[..10].to_owned()
    };
    let validity = cert.validity();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default();
    let names = cert
        .subject_alternative_name()
        .ok()
        .flatten()
        .map(|san| {
            san.value
                .general_names
                .iter()
                .filter_map(|n| match n {
                    GeneralName::DNSName(dns) => Some((*dns).to_owned()),
                    GeneralName::IPAddress(bytes) => match bytes.len() {
                        4 => <[u8; 4]>::try_from(*bytes)
                            .ok()
                            .map(|b| IpAddr::from(b).to_string()),
                        16 => <[u8; 16]>::try_from(*bytes)
                            .ok()
                            .map(|b| IpAddr::from(b).to_string()),
                        _ => None,
                    },
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    use sha2::Digest as _;
    let sha256 = sha2::Sha256::digest(der)
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":");
    Some(Certificate {
        subject: name(cert.subject()),
        issuer: name(cert.issuer()),
        key: key_of(&cert),
        sha256,
        not_before: date(&validity.not_before),
        not_after: date(&validity.not_after),
        days_left: (validity.not_after.timestamp() - now).div_euclid(86_400),
        names,
    })
}

/// `RSA 2048`, `ECDSA P-256`, `Ed25519` …
fn key_of(cert: &x509_parser::certificate::X509Certificate<'_>) -> String {
    use x509_parser::public_key::PublicKey;

    let info = cert.public_key();
    match info.parsed() {
        Ok(PublicKey::RSA(rsa)) => format!("RSA {}", rsa.key_size()),
        Ok(PublicKey::EC(point)) => {
            let curve = info
                .algorithm
                .parameters
                .as_ref()
                .and_then(|p| p.as_oid().ok())
                .map(|oid| match oid.to_id_string().as_str() {
                    "1.2.840.10045.3.1.7" => "P-256".to_owned(),
                    "1.3.132.0.34" => "P-384".to_owned(),
                    "1.3.132.0.35" => "P-521".to_owned(),
                    other => other.to_owned(),
                })
                .unwrap_or_else(|| format!("{} bits", point.key_size()));
            format!("ECDSA {curve}")
        }
        _ => match info.algorithm.algorithm.to_id_string().as_str() {
            "1.3.101.112" => "Ed25519".to_owned(),
            "1.3.101.113" => "Ed448".to_owned(),
            other => other.to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_add_up_and_never_go_negative() {
        let trace = NetTrace {
            dns: vec![DnsLookup {
                host: "x".into(),
                addrs: vec![],
                duration: Duration::from_millis(20),
            }],
            connects: vec![Connect {
                total: Duration::from_millis(80),
                tls: Some(Duration::from_millis(30)),
            }],
            ..NetTrace::default()
        };
        let p = trace.phases(Duration::from_millis(200), Duration::from_millis(260));
        assert_eq!(p.dns, Duration::from_millis(20));
        assert_eq!(p.tcp, Duration::from_millis(30));
        assert_eq!(p.tls, Duration::from_millis(30));
        assert_eq!(p.waiting, Duration::from_millis(120));
        assert_eq!(p.download, Duration::from_millis(60));
        assert_eq!(
            p.dns + p.tcp + p.tls + p.waiting + p.download,
            Duration::from_millis(260)
        );

        // A reused or proxied connection: nothing recorded, all waiting.
        let p = NetTrace::default().phases(Duration::from_millis(50), Duration::from_millis(40));
        assert_eq!(
            (p.dns, p.tcp + p.tls, p.waiting, p.download),
            (
                Duration::ZERO,
                Duration::ZERO,
                Duration::from_millis(50),
                Duration::ZERO
            )
        );
    }

    #[test]
    fn ignores_garbage_certificates() {
        assert!(read_certificate(b"not a certificate").is_none());
    }
}
