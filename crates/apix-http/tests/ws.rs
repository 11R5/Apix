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
