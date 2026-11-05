//! TLS details of the request's own connection. reqwest hands out neither
//! the TLS connection nor a way to make it ourselves, so the details come
//! from the places rustls calls into while it shakes hands with the server:
//!
//! - the certificate verifier sees the whole chain the server sent, and
//!   which version's signature it is asked to check (TLS 1.2 or 1.3);
//! - the session store is asked for a key exchange hint just before the
//!   client says hello (the TLS handshake starts there; the time before it,
//!   after DNS, is TCP), and told which group the server chose (TLS 1.3);
//! - the cipher suites are rustls' own with a thin wrapper around their
//!   encryption that notes which one was set up. Suites are `'static`, so
//!   the wrapper tells the request apart through [`Scoped`]: the handshake
//!   runs while the request's future is being polled, which sets the
//!   recorder for this thread for that time.
//!
//! Nothing is guessed: what a server does not show (the cipher of a TLS 1.3
//! connection whose handshake ran elsewhere) stays empty.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::{ClientSessionStore, Tls12ClientSessionValue, Tls13ClientSessionValue};
use rustls::crypto::cipher::{
    AeadKey, Iv, KeyBlockShape, MessageDecrypter, MessageEncrypter, Tls12AeadAlgorithm,
    Tls13AeadAlgorithm, UnsupportedOperationError,
};
use rustls::crypto::{CipherSuiteCommon, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    CipherSuite, ConnectionTrafficSecrets, DigitallySignedStruct, NamedGroup, SignatureScheme,
    SupportedCipherSuite, Tls12CipherSuite, Tls13CipherSuite,
};

use crate::trace::{Recorder, read_certificate};

thread_local! {
    /// The recorder of the request being polled on this thread, if any.
    static CURRENT: RefCell<Option<Arc<Recorder>>> = const { RefCell::new(None) };
}

/// Polls `inner` with `recorder` as this thread's current recorder, so the
/// cipher suite wrappers know whose handshake they are part of.
pub(crate) struct Scoped<F> {
    inner: Pin<Box<F>>,
    recorder: Arc<Recorder>,
}

impl<F> Scoped<F> {
    pub(crate) fn new(inner: F, recorder: Arc<Recorder>) -> Self {
        Self {
            inner: Box::pin(inner),
            recorder,
        }
    }
}

impl<F: Future> Future for Scoped<F> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        let previous = CURRENT.with(|c| c.replace(Some(self.recorder.clone())));
        let polled = self.inner.as_mut().poll(cx);
        CURRENT.with(|c| *c.borrow_mut() = previous);
        polled
    }
}

fn note_cipher(suite: CipherSuite) {
    CURRENT.with(|current| {
        if let Some(recorder) = current.borrow().as_ref() {
            recorder.tls(|tls| {
                tls.cipher.get_or_insert_with(|| format!("{suite:?}"));
            });
        }
    });
}

struct Aead13 {
    inner: &'static dyn Tls13AeadAlgorithm,
    suite: CipherSuite,
}

impl Tls13AeadAlgorithm for Aead13 {
    fn encrypter(&self, key: AeadKey, iv: Iv) -> Box<dyn MessageEncrypter> {
        note_cipher(self.suite);
        self.inner.encrypter(key, iv)
    }

    fn decrypter(&self, key: AeadKey, iv: Iv) -> Box<dyn MessageDecrypter> {
        note_cipher(self.suite);
        self.inner.decrypter(key, iv)
    }

    fn key_len(&self) -> usize {
        self.inner.key_len()
    }

    fn extract_keys(
        &self,
        key: AeadKey,
        iv: Iv,
    ) -> Result<ConnectionTrafficSecrets, UnsupportedOperationError> {
        self.inner.extract_keys(key, iv)
    }

    fn fips(&self) -> bool {
        self.inner.fips()
    }
}

struct Aead12 {
    inner: &'static dyn Tls12AeadAlgorithm,
    suite: CipherSuite,
}

impl Tls12AeadAlgorithm for Aead12 {
    fn encrypter(&self, key: AeadKey, iv: &[u8], extra: &[u8]) -> Box<dyn MessageEncrypter> {
        note_cipher(self.suite);
        self.inner.encrypter(key, iv, extra)
    }

    fn decrypter(&self, key: AeadKey, iv: &[u8]) -> Box<dyn MessageDecrypter> {
        note_cipher(self.suite);
        self.inner.decrypter(key, iv)
    }

    fn key_block_shape(&self) -> KeyBlockShape {
        self.inner.key_block_shape()
    }

    fn extract_keys(
        &self,
        key: AeadKey,
        iv: &[u8],
        explicit: &[u8],
    ) -> Result<ConnectionTrafficSecrets, UnsupportedOperationError> {
        self.inner.extract_keys(key, iv, explicit)
    }

    fn fips(&self) -> bool {
        self.inner.fips()
    }
}

fn common(c: &CipherSuiteCommon) -> CipherSuiteCommon {
    CipherSuiteCommon {
        suite: c.suite,
        hash_provider: c.hash_provider,
        confidentiality_limit: c.confidentiality_limit,
    }
}

/// rustls' ring cipher suites, each noting when it is set up. Made once
/// for the whole program.
fn suites() -> &'static [SupportedCipherSuite] {
    static SUITES: OnceLock<Vec<SupportedCipherSuite>> = OnceLock::new();
    SUITES.get_or_init(|| {
        rustls::crypto::ring::default_provider()
            .cipher_suites
            .into_iter()
            .map(|suite| match suite {
                SupportedCipherSuite::Tls13(s) => {
                    let aead: &'static Aead13 = Box::leak(Box::new(Aead13 {
                        inner: s.aead_alg,
                        suite: s.common.suite,
                    }));
                    let wrapped: &'static Tls13CipherSuite =
                        Box::leak(Box::new(Tls13CipherSuite {
                            common: common(&s.common),
                            hkdf_provider: s.hkdf_provider,
                            aead_alg: aead,
                            quic: s.quic,
                        }));
                    SupportedCipherSuite::Tls13(wrapped)
                }
                SupportedCipherSuite::Tls12(s) => {
                    let aead: &'static Aead12 = Box::leak(Box::new(Aead12 {
                        inner: s.aead_alg,
                        suite: s.common.suite,
                    }));
                    let wrapped: &'static Tls12CipherSuite =
                        Box::leak(Box::new(Tls12CipherSuite {
                            common: common(&s.common),
                            prf_provider: s.prf_provider,
                            kx: s.kx,
                            sign: s.sign,
                            aead_alg: aead,
                        }));
                    SupportedCipherSuite::Tls12(wrapped)
                }
            })
            .collect()
    })
}

/// rustls' ring provider with the noting cipher suites.
pub(crate) fn provider() -> CryptoProvider {
    CryptoProvider {
        cipher_suites: suites().to_vec(),
        ..rustls::crypto::ring::default_provider()
    }
}

/// A session store that keeps nothing (each send is a fresh client anyway)
/// and notes when a handshake starts and which group the server chose.
#[derive(Debug)]
pub(crate) struct Sessions(pub(crate) Arc<Recorder>);

impl ClientSessionStore for Sessions {
    fn set_kx_hint(&self, _: ServerName<'static>, group: NamedGroup) {
        self.0.tls(|tls| tls.group = Some(format!("{group:?}")));
    }

    /// Asked while the client hello is being written.
    fn kx_hint(&self, server_name: &ServerName<'_>) -> Option<NamedGroup> {
        self.0.tls_started(server_name.to_str().into_owned());
        None
    }

    fn set_tls12_session(&self, _: ServerName<'static>, _: Tls12ClientSessionValue) {}

    fn tls12_session(&self, _: &ServerName<'_>) -> Option<Tls12ClientSessionValue> {
        None
    }

    fn remove_tls12_session(&self, _: &ServerName<'static>) {}

    fn insert_tls13_ticket(&self, _: ServerName<'static>, _: Tls13ClientSessionValue) {}

    fn take_tls13_ticket(&self, _: &ServerName<'static>) -> Option<Tls13ClientSessionValue> {
        None
    }
}

/// Checks certificates with `inner`, noting the chain and the TLS version.
#[derive(Debug)]
pub(crate) struct Watching {
    pub(crate) inner: Arc<dyn ServerCertVerifier>,
    pub(crate) recorder: Arc<Recorder>,
}

impl ServerCertVerifier for Watching {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let chain = std::iter::once(end_entity)
            .chain(intermediates)
            .filter_map(|der| read_certificate(der))
            .collect();
        self.recorder.chain(chain);
        self.inner
            .verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now)
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.recorder
            .tls(|tls| tls.version = Some("TLS 1.2".to_owned()));
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.recorder
            .tls(|tls| tls.version = Some("TLS 1.3".to_owned()));
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }

    fn requires_raw_public_keys(&self) -> bool {
        self.inner.requires_raw_public_keys()
    }

    fn root_hint_subjects(&self) -> Option<&[rustls::DistinguishedName]> {
        self.inner.root_hint_subjects()
    }
}
