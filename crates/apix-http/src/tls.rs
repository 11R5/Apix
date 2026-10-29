//! TLS settings for connections reqwest does not make itself (WebSocket,
//! gRPC without verification): the usual root certificates plus a trusted CA
//! file, a client certificate, or no verification at all, from the
//! request's settings.

use std::sync::Arc;

use apix_core::EffectiveSettings;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::WebPkiSupportedAlgorithms;
use rustls::pki_types::pem::PemObject as _;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme};

use crate::{HttpError, read_file};

/// Accepts any server certificate; the handshake signatures are still
/// checked, so the connection is encrypted with the key the server holds.
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

fn settings_error(message: String) -> HttpError {
    HttpError::Settings(message)
}

/// The certificates in a PEM file; an error when there are none.
pub fn pem_certificates(path: &str) -> Result<Vec<CertificateDer<'static>>, HttpError> {
    let pem = read_file(path)?;
    let certs = CertificateDer::pem_slice_iter(&pem)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| settings_error(format!("cannot read the certificates in {path}: {e}")))?;
    if certs.is_empty() {
        return Err(settings_error(format!("{path} holds no PEM certificate")));
    }
    Ok(certs)
}

/// The TLS client settings for `settings`, offering `alpn` (such as `h2`).
pub fn client_config(
    settings: &EffectiveSettings,
    alpn: &[&[u8]],
) -> Result<ClientConfig, HttpError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let algorithms = provider.signature_verification_algorithms;
    let builder = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| settings_error(e.to_string()))?;
    let builder = if settings.tls_verify {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        if let Some(ca) = &settings.ca_cert {
            for cert in pem_certificates(ca)? {
                roots.add(cert).map_err(|e| {
                    settings_error(format!("cannot use the CA certificate {ca}: {e}"))
                })?;
            }
        }
        builder.with_root_certificates(roots)
    } else {
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAny(algorithms)))
    };
    let mut config = match &settings.client_cert {
        None => builder.with_no_client_auth(),
        Some(cert) => {
            let certs = pem_certificates(cert)?;
            let key_file = settings.client_key.as_deref().unwrap_or(cert);
            let key = PrivateKeyDer::from_pem_slice(&read_file(key_file)?).map_err(|e| {
                settings_error(format!("cannot read the client key {key_file}: {e}"))
            })?;
            builder
                .with_client_auth_cert(certs, key)
                .map_err(|e| settings_error(format!("cannot use the client certificate: {e}")))?
        }
    };
    config.alpn_protocols = alpn.iter().map(|p| p.to_vec()).collect();
    Ok(config)
}
