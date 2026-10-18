//! What happened on the network while a request was sent: DNS answers and
//! time, connection setup time, redirects, the local and remote address and
//! the server certificate. Each send uses its own short-lived client, so
//! every event recorded here belongs to that one request and the times are
//! those of a real, fresh connection.
//!
//! reqwest reports the server's leaf certificate but not the TLS version or
//! cipher suite, so those are not shown rather than guessed.

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

/// The server's certificate (the leaf; reqwest does not hand out the chain).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate {
    pub subject: String,
    pub issuer: String,
    /// `2026-01-02` (UTC).
    pub not_before: String,
    pub not_after: String,
    /// Negative once expired.
    pub days_left: i64,
    /// Host names the certificate is valid for.
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetTrace {
    pub dns: Vec<DnsLookup>,
    /// Connection setups (DNS, TCP and TLS together), one per new connection.
    pub connects: Vec<Duration>,
    pub redirects: Vec<Redirect>,
    pub local_addr: Option<SocketAddr>,
    pub remote_addr: Option<SocketAddr>,
    pub certificate: Option<Certificate>,
}

/// Where the time went, for the waterfall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phases {
    pub dns: Duration,
    /// TCP and TLS together (reqwest does not separate them).
    pub connect: Duration,
    /// From the request leaving to the first byte of the answer.
    pub waiting: Duration,
    pub download: Duration,
}

impl NetTrace {
    /// Splits the total time into phases. `ttfb` and `total` are measured
    /// from the start of the send; DNS happens inside connection setup.
    pub fn phases(&self, ttfb: Duration, total: Duration) -> Phases {
        let dns: Duration = self.dns.iter().map(|d| d.duration).sum();
        let setup: Duration = self.connects.iter().sum();
        Phases {
            dns,
            connect: setup.saturating_sub(dns),
            waiting: ttfb.saturating_sub(setup.max(dns)),
            download: total.saturating_sub(ttfb),
        }
    }
}

/// Collects events from the resolver, the connector and the redirect policy.
#[derive(Debug, Default)]
pub(crate) struct Recorder(Mutex<NetTrace>);

impl Recorder {
    fn with(&self, f: impl FnOnce(&mut NetTrace)) {
        if let Ok(mut trace) = self.0.lock() {
            f(&mut trace);
        }
    }

    pub(crate) fn redirect(&self, status: u16, to: String) {
        self.with(|t| t.redirects.push(Redirect { status, to }));
    }

    pub(crate) fn take(&self) -> NetTrace {
        self.0
            .lock()
            .map(|mut t| std::mem::take(&mut *t))
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
                let took = started.elapsed();
                recorder.with(|t| t.connects.push(took));
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
    Some(Certificate {
        subject: name(cert.subject()),
        issuer: name(cert.issuer()),
        not_before: date(&validity.not_before),
        not_after: date(&validity.not_after),
        days_left: (validity.not_after.timestamp() - now).div_euclid(86_400),
        names,
    })
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
            connects: vec![Duration::from_millis(80)],
            ..NetTrace::default()
        };
        let p = trace.phases(Duration::from_millis(200), Duration::from_millis(260));
        assert_eq!(p.dns, Duration::from_millis(20));
        assert_eq!(p.connect, Duration::from_millis(60));
        assert_eq!(p.waiting, Duration::from_millis(120));
        assert_eq!(p.download, Duration::from_millis(60));
        assert_eq!(
            p.dns + p.connect + p.waiting + p.download,
            Duration::from_millis(260)
        );

        // A reused or proxied connection: nothing recorded, all waiting.
        let p = NetTrace::default().phases(Duration::from_millis(50), Duration::from_millis(40));
        assert_eq!(
            (p.dns, p.connect, p.waiting, p.download),
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
