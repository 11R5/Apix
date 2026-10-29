//! WebSocket connections against a local echo server.

use std::sync::{Arc, Mutex};

use apix_core::{Auth, KeyValue, Method, Request};
use apix_http::ws::{self, WsCommand, WsData, WsEvent};
use futures_util::{SinkExt as _, StreamExt as _};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{Request as Handshake, Response};

/// Echoes text as `echo: …` and binary as is; `bye` makes the server close
/// with 4000. Remembers the handshake's path and Authorization header.
// The handshake callback's error type is tungstenite's, not ours.
#[allow(clippy::result_large_err)]
async fn serve(listener: TcpListener, seen: Arc<Mutex<Vec<String>>>) {
    let (socket, _) = listener.accept().await.unwrap();
    let callback = |req: &Handshake, response: Response| {
        let mut seen = seen.lock().unwrap();
        seen.push(req.uri().to_string());
        seen.push(
            req.headers()
                .get("authorization")
                .map(|v| v.to_str().unwrap().to_owned())
                .unwrap_or_default(),
        );
        seen.push(
            req.headers()
                .get("x-client")
                .map(|v| v.to_str().unwrap().to_owned())
                .unwrap_or_default(),
        );
        Ok(response)
    };
    let mut stream = tokio_tungstenite::accept_hdr_async(socket, callback)
        .await
        .unwrap();
    while let Some(Ok(message)) = stream.next().await {
        match message {
            Message::Text(text) if text.as_str() == "bye" => {
                let frame = tokio_tungstenite::tungstenite::protocol::CloseFrame {
                    code: 4000.into(),
                    reason: "see you".into(),
                };
                let _ = stream.close(Some(frame)).await;
                return;
            }
            Message::Text(text) => {
                stream
                    .send(Message::text(format!("echo: {}", text.as_str())))
                    .await
                    .unwrap();
            }
            Message::Binary(bytes) => stream.send(Message::Binary(bytes)).await.unwrap(),
            Message::Close(_) => return,
            _ => {}
        }
    }
}

fn request(url: &str) -> Request {
    let mut req = Request::new(Method::Get, url);
    req.auth = Auth::Bearer {
        token: "t0k".into(),
    };
    req.query.push(KeyValue::new("room", "7"));
    req.headers.push(KeyValue::new("X-Client", "apix"));
    req
}

#[tokio::test]
async fn messages_go_both_ways_and_the_server_closes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/chat", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    tokio::spawn(serve(listener, seen.clone()));

    let connection = ws::connect(&request(&url)).await.unwrap();
    assert_eq!(connection.status, 101);
    assert_eq!(
        *seen.lock().unwrap(),
        ["/chat?room=7", "Bearer t0k", "apix"]
    );

    let (tx, rx) = mpsc::unbounded_channel();
    tx.send(WsCommand::Send(WsData::Text("hello".into())))
        .unwrap();
    tx.send(WsCommand::Send(WsData::Binary(vec![1, 2, 3])))
        .unwrap();
    tx.send(WsCommand::Send(WsData::Text("bye".into())))
        .unwrap();
    let mut events = Vec::new();
    connection.run(rx, |event| events.push(event)).await;

    let received: Vec<&WsEvent> = events
        .iter()
        .filter(|e| !matches!(e, WsEvent::Sent(_)))
        .collect();
    assert_eq!(
        received,
        [
            &WsEvent::Received(WsData::Text("echo: hello".into())),
            &WsEvent::Received(WsData::Binary(vec![1, 2, 3])),
            &WsEvent::Closed {
                code: Some(4000),
                reason: "see you".into()
            },
        ]
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, WsEvent::Sent(_)))
            .count(),
        3
    );
}

#[tokio::test]
async fn we_can_close() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/", listener.local_addr().unwrap());
    tokio::spawn(serve(listener, Arc::default()));
    let connection = ws::connect(&Request::new(Method::Get, &url)).await.unwrap();
    let (tx, rx) = mpsc::unbounded_channel();
    tx.send(WsCommand::Close).unwrap();
    let mut events = Vec::new();
    connection.run(rx, |event| events.push(event)).await;
    assert!(
        matches!(events.last(), Some(WsEvent::Closed { .. })),
        "{events:?}"
    );
}

#[tokio::test]
async fn subprotocols_are_offered_and_binary_goes_through() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        // Picks the second protocol it is offered, then echoes.
        #[allow(clippy::result_large_err)]
        let callback = |req: &Handshake, mut response: Response| {
            let offered = req
                .headers()
                .get("sec-websocket-protocol")
                .map(|v| v.to_str().unwrap().to_owned())
                .unwrap_or_default();
            let pick = offered
                .split(',')
                .nth(1)
                .unwrap_or_default()
                .trim()
                .to_owned();
            response
                .headers_mut()
                .insert("sec-websocket-protocol", pick.parse().unwrap());
            Ok(response)
        };
        let mut stream = tokio_tungstenite::accept_hdr_async(socket, callback)
            .await
            .unwrap();
        while let Some(Ok(message)) = stream.next().await {
            if message.is_binary() {
                stream.send(message).await.unwrap();
            } else if message.is_close() {
                return;
            }
        }
    });
    let mut req = Request::new(Method::Get, &url);
    req.websocket = Some(apix_core::WebSocketOptions {
        subprotocols: vec!["v1.chat".into(), "v2.chat".into()],
    });
    let connection = ws::connect(&req).await.unwrap();
    assert_eq!(connection.protocol.as_deref(), Some("v2.chat"));

    let data = WsData::from_text("0x01 ff".into(), apix_core::MessageFormat::Hex).unwrap();
    assert_eq!(data, WsData::Binary(vec![1, 255]));
    assert_eq!(ws::hex_preview(&[1, 255]), "2 bytes: 01 ff");
    let (tx, rx) = mpsc::unbounded_channel();
    tx.send(WsCommand::Send(data)).unwrap();
    tx.send(WsCommand::Close).unwrap();
    let mut events = Vec::new();
    connection.run(rx, |event| events.push(event)).await;
    assert!(
        events.contains(&WsEvent::Received(WsData::Binary(vec![1, 255]))),
        "{events:?}"
    );
}

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

#[tokio::test]
async fn wss_with_an_own_authority_or_without_verifying() {
    use rustls::pki_types::pem::PemObject as _;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};

    let (ca, cert, key) = certificates();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("wss://{}/", listener.local_addr().unwrap());
    let certs = CertificateDer::pem_slice_iter(cert.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let key = PrivateKeyDer::from_pem_slice(key.as_bytes()).unwrap();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(certs, key)
    .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    tokio::spawn(async move {
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let Ok(tls) = acceptor.accept(socket).await else {
                    return;
                };
                if let Ok(mut stream) = tokio_tungstenite::accept_async(tls).await {
                    let _ = stream.close(None).await;
                }
            });
        }
    });
    let dir = std::env::temp_dir().join(format!("apix-wss-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ca_file = dir.join("ca.pem");
    std::fs::write(&ca_file, &ca).unwrap();

    let mut req = Request::new(Method::Get, &url);
    assert!(ws::connect(&req).await.is_err(), "an unknown authority");
    req.settings.ca_cert = Some(ca_file.to_string_lossy().into_owned());
    assert_eq!(ws::connect(&req).await.unwrap().status, 101);
    req.settings.ca_cert = None;
    req.settings.tls_verify = Some(false);
    assert_eq!(ws::connect(&req).await.unwrap().status, 101);
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn a_refused_handshake_says_why() {
    // A plain HTTP server that answers 401.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/", listener.local_addr().unwrap());
    tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 4096];
        let _ = socket.read(&mut buf).await;
        let _ = socket
            .write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 12\r\n\r\nbad token!!!")
            .await;
    });
    let Err(err) = ws::connect(&Request::new(Method::Get, &url)).await else {
        panic!("expected an error");
    };
    let message = err.to_string();
    assert!(message.contains("401"), "{message}");
}
