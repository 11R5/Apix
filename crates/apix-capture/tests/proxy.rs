//! The capture proxy between Apix's own HTTP client and local servers: plain
//! HTTP recorded, HTTPS opened with the local authority, HTTPS passed on.

use std::sync::{Arc, Mutex};

use apix_capture::{Authority, Event, Options};
use apix_core::{Method, Request};
use apix_http::HttpClient;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpListener;

fn temp(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("apix-capture-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Answers every request with its first line, as JSON.
async fn answer<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(mut stream: S) {
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await.unwrap_or(0);
    let head = String::from_utf8_lossy(&buf[..n]);
    let first = head.lines().next().unwrap_or_default().to_owned();
    let body = format!("{{\"got\":\"{first}\"}}");
    let _ = stream
        .write_all(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await;
    let _ = stream.shutdown().await;
}

async fn plain_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(answer(socket));
        }
    });
    format!("http://{addr}")
}

/// An HTTPS server with a certificate from an authority of its own; returns
/// its address and that authority (PEM).
async fn tls_server() -> (String, String) {
    use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair};
    let mut ca = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca.distinguished_name
        .push(DnType::CommonName, "Upstream CA");
    let ca_key = KeyPair::generate().unwrap();
    let ca_pem = ca.self_signed(&ca_key).unwrap().pem();
    let issuer = Issuer::new(ca, ca_key);
    let leaf_key = KeyPair::generate().unwrap();
    let leaf = CertificateParams::new(vec!["127.0.0.1".to_owned()])
        .unwrap()
        .signed_by(&leaf_key, &issuer)
        .unwrap();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![leaf.der().clone()],
        rustls::pki_types::PrivateKeyDer::Pkcs8(leaf_key.serialize_der().into()),
    )
    .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                return;
            };
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                if let Ok(tls) = acceptor.accept(socket).await {
                    answer(tls).await;
                }
            });
        }
    });
    (format!("https://{addr}"), ca_pem)
}

async fn proxy(options: Options) -> (String, Arc<Mutex<Vec<Event>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = events.clone();
    tokio::spawn(apix_capture::run(listener, options, move |e| {
        seen.lock().unwrap().push(e)
    }));
    (format!("http://{addr}"), events)
}

fn through(url: &str, proxy: &str) -> Request {
    let mut request = Request::new(Method::Post, url);
    request.settings.proxy = Some(proxy.to_owned());
    request
        .headers
        .push(apix_core::KeyValue::new("Authorization", "Bearer s3cret"));
    request.body = apix_core::Body::Json("{\"a\":1}".into());
    request
}

#[tokio::test]
async fn plain_http_is_recorded_and_saved() {
    let server = plain_server().await;
    let (proxy, events) = proxy(Options::default()).await;
    let response = HttpClient::new()
        .unwrap()
        .send(&through(&format!("{server}/users?x=1"), &proxy))
        .await
        .unwrap();
    assert_eq!(response.status, 200);
    assert!(
        response.raw_body().contains("POST /users?x=1"),
        "{}",
        response.raw_body()
    );

    let events = events.lock().unwrap();
    let Some(Event::Exchange(exchange)) = events.first() else {
        panic!("{events:?}")
    };
    assert_eq!(exchange.request.name, "POST /users");
    assert_eq!(exchange.request.headers[0].value, "{{authorization}}");
    let dir = temp("save");
    let path = apix_capture::save(exchange, &dir).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("s3cret"));
    let saved = apix_core::store::load_request(&path).unwrap();
    assert_eq!(saved.examples[0].status, 200);
    assert!(saved.examples[0].body.contains("POST /users"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn https_is_opened_with_the_local_authority() {
    let (server, upstream_ca) = tls_server().await;
    let dir = temp("ca");
    let authority = Arc::new(Authority::load_or_create(&dir).unwrap());
    let (proxy, events) = proxy(Options {
        open_https: Some(authority.clone()),
        upstream_ca: Some(upstream_ca.into_bytes()),
        ..Options::default()
    })
    .await;
    let mut request = through(&format!("{server}/orders"), &proxy);
    // The client trusts the capture authority, as the user would.
    request.settings.ca_cert = Some(authority.cert_path.to_string_lossy().into_owned());
    let response = HttpClient::new().unwrap().send(&request).await.unwrap();
    assert_eq!(response.status, 200);
    assert!(response.raw_body().contains("POST /orders"));
    let cert = response.trace.certificate.clone().unwrap();
    assert_eq!(cert.issuer, "Apix capture authority (local) (Apix)");

    let events = events.lock().unwrap();
    assert!(
        matches!(events.first(), Some(Event::Exchange(e)) if e.request.url.ends_with("/orders")),
        "{events:?}"
    );
    // The authority is kept, and read back the next time.
    assert!(Authority::load_or_create(&dir).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn https_tunnels_pass_through_unread() {
    let (server, upstream_ca) = tls_server().await;
    let dir = temp("tunnel");
    let ca_file = dir.join("upstream.pem");
    std::fs::write(&ca_file, upstream_ca).unwrap();
    let (proxy, events) = proxy(Options::default()).await;
    let mut request = through(&format!("{server}/secret"), &proxy);
    request.settings.ca_cert = Some(ca_file.to_string_lossy().into_owned());
    let response = HttpClient::new().unwrap().send(&request).await.unwrap();
    assert_eq!(response.status, 200);
    let events = events.lock().unwrap();
    assert!(
        matches!(events.as_slice(), [Event::Tunnel { host }] if host == "127.0.0.1"),
        "{events:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
