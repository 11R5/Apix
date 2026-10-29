//! TLS without checking who answers, for "Verify TLS certificate" turned
//! off. tonic has no such switch, so the channel is opened as plain HTTP/2
//! to a connector that does the TLS itself (with ALPN h2) and accepts any
//! certificate; the requests still name the `https` origin.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use hyper_util::rt::TokioIo;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::WebPkiSupportedAlgorithms;
use rustls::pki_types::pem::PemObject as _;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;
use tonic::codegen::http::Uri;

use crate::GrpcError;

/// Accepts any server certificate; signatures are still checked, so the
/// connection is encrypted with the key the server holds.
#[derive(Debug)]
struct AcceptAny(WebPkiSupportedAlgorithms);

impl ServerCertVerifier for AcceptAny {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.0)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.0)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.supported_schemes()
    }
}

/// The TLS settings: any certificate accepted, h2 offered, and the client
/// certificate (PEM chain and key) when there is one.
pub(crate) fn config(identity: Option<(Vec<u8>, Vec<u8>)>) -> Result<ClientConfig, GrpcError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = AcceptAny(provider.signature_verification_algorithms);
    let builder = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| GrpcError::Tls(e.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier));
    let mut config = match identity {
        None => builder.with_no_client_auth(),
        Some((cert_pem, key_pem)) => {
            let certs = CertificateDer::pem_slice_iter(&cert_pem)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| GrpcError::Tls(format!("cannot read the client certificate: {e}")))?;
            let key = PrivateKeyDer::from_pem_slice(&key_pem)
                .map_err(|e| GrpcError::Tls(format!("cannot read the client key: {e}")))?;
            builder
                .with_client_auth_cert(certs, key)
                .map_err(|e| GrpcError::Tls(format!("cannot use the client certificate: {e}")))?
        }
    };
    config.alpn_protocols = vec![b"h2".to_vec()];
    Ok(config)
}

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
