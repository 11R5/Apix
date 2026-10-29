//! TLS without checking who answers, for "Verify TLS certificate" turned
//! off. tonic has no such switch, so the channel is opened as plain HTTP/2
//! to a connector that does the TLS itself (with ALPN h2, settings from
//! `apix_http::tls`); the requests still name the `https` origin.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use hyper_util::rt::TokioIo;
use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;
use tonic::codegen::http::Uri;

/// Opens TCP to the URI's host and port, then TLS with `config`.
#[derive(Clone)]
pub(crate) struct Connector {
    tls: TlsConnector,
}

impl Connector {
    pub(crate) fn new(config: ClientConfig) -> Self {
        Self {
            tls: TlsConnector::from(Arc::new(config)),
        }
    }
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

impl tower_service::Service<Uri> for Connector {
    type Response = TokioIo<TlsStream<TcpStream>>;
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, BoxError>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), BoxError>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, uri: Uri) -> Self::Future {
        let tls = self.tls.clone();
        Box::pin(async move {
            let host = uri.host().ok_or("the address has no host")?;
            // `[::1]` in a URI is `::1` to connect to and to name the server.
            let host = host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .to_owned();
            let port = uri.port_u16().unwrap_or(443);
            let tcp = TcpStream::connect((host.as_str(), port)).await?;
            let name = ServerName::try_from(host)?;
            Ok(TokioIo::new(tls.connect(name, tcp).await?))
        })
    }
}
