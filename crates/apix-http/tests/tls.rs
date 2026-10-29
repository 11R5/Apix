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
    tokio::spawn(serve(listener, cert, key));
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
