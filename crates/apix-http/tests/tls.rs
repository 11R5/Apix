//! TLS settings against a local HTTPS server whose certificate comes from
//! an authority of its own: refused by default, trusted with `ca_cert`,
//! accepted unchecked with `tls_verify = false`.

use std::sync::Arc;

use apix_core::{Method, Request};
use apix_http::HttpClient;
use rustls::pki_types::pem::PemObject as _;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpListener;

/// (CA, server certificate, server key), PEM, for 127.0.0.1.
fn certificates() -> (String, String, String) {
    use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair};
    let mut ca = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca.distinguished_name
        .push(DnType::CommonName, "Apix test CA");
    let ca_key = KeyPair::generate().unwrap();
    let ca_pem = ca.self_signed(&ca_key).unwrap().pem();
    let issuer = Issuer::new(ca, ca_key);
    let server = CertificateParams::new(vec!["127.0.0.1".to_owned()]).unwrap();
    let server_key = KeyPair::generate().unwrap();
    let server_pem = server.signed_by(&server_key, &issuer).unwrap().pem();
    (ca_pem, server_pem, server_key.serialize_pem())
}

/// Answers every HTTPS request with `200 OK` and `secure`.
async fn serve(listener: TcpListener, cert: String, key: String) {
    let certs = CertificateDer::pem_slice_iter(cert.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let key = PrivateKeyDer::from_pem_slice(key.as_bytes()).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let acceptor = acceptor.clone();
        tokio::spawn(async move {
            // A refused handshake is the point of some tests.
            let Ok(mut tls) = acceptor.accept(socket).await else {
                return;
            };
            let mut buf = vec![0u8; 4096];
            let _ = tls.read(&mut buf).await;
            let _ = tls
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nsecure",
                )
                .await;
            let _ = tls.shutdown().await;
        });
    }
}

#[tokio::test]
async fn an_own_authority_can_be_trusted() {
    let (ca, cert, key) = certificates();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("https://{}/", listener.local_addr().unwrap());
    // The server sends its whole chain: its own certificate, then the CA's.
    tokio::spawn(serve(listener, format!("{cert}{ca}"), key));
    let dir = std::env::temp_dir().join(format!("apix-http-tls-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ca_file = dir.join("ca.pem");
    std::fs::write(&ca_file, &ca).unwrap();
    let client = HttpClient::new().unwrap();

    let mut req = Request::new(Method::Get, &url);
    assert!(client.send(&req).await.is_err(), "an unknown authority");

    req.settings.ca_cert = Some(ca_file.to_string_lossy().into_owned());
    let response = client.send(&req).await.unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.raw_body(), "secure");

    // What the handshake settled on, from the request's own connection.
    let trace = &response.trace;
    let tls = trace.tls.as_ref().expect("TLS details");
    assert_eq!(tls.server_name, "127.0.0.1");
    assert_eq!(tls.version.as_deref(), Some("TLS 1.3"));
    assert!(
        tls.cipher
            .as_deref()
            .is_some_and(|c| c.starts_with("TLS13_")),
        "{tls:?}"
    );
    assert!(tls.group.is_some(), "{tls:?}");
    assert_eq!(tls.alpn, None, "HTTP/1.1 answer");
    assert_eq!(tls.chain.len(), 2);
    assert_eq!(tls.chain[0].issuer, "Apix test CA");
    assert_eq!(tls.chain[1].subject, "Apix test CA");
    assert!(
        tls.chain[0].key.starts_with("ECDSA P-256"),
        "{}",
        tls.chain[0].key
    );
    assert_eq!(tls.chain[0].sha256.len(), 32 * 3 - 1);
    assert_eq!(trace.certificate.as_ref(), tls.chain.first());
    let handshake = tls.handshake.expect("handshake time");
    assert_eq!(trace.connects.len(), 1);
    assert_eq!(trace.connects[0].tls, Some(handshake));
    assert!(handshake <= trace.connects[0].total);
    let phases = trace.phases(response.timing.ttfb, response.timing.total);
    assert_eq!(phases.tls, handshake);

    req.settings.ca_cert = None;
    req.settings.tls_verify = Some(false);
    assert_eq!(client.send(&req).await.unwrap().status, 200);

    std::fs::write(&ca_file, "not a certificate").unwrap();
    req.settings.tls_verify = None;
    req.settings.ca_cert = Some(ca_file.to_string_lossy().into_owned());
    let err = client.send(&req).await.unwrap_err();
    assert!(err.to_string().contains("no PEM certificate"), "{err}");
    std::fs::remove_dir_all(&dir).ok();
}
