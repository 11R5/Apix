//! Socket.IO connections against a local Socket.IO 4 server.

#[path = "common/socketio_server.rs"]
mod server;

use std::time::Duration;

use apix_core::{Auth, KeyValue, Method, Request, SocketIoOptions};
use apix_http::socketio::{self, SioCommand, SioEvent, SioMessage};
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

async fn start() -> (String, server::Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = server::Seen::default();
    tokio::spawn(server::serve(listener, seen.clone()));
    (url, seen)
}

fn request(url: &str, options: SocketIoOptions) -> Request {
    Request {
        socketio: Some(options),
        ..Request::new(Method::Get, url)
    }
}

/// Runs the connection with these commands (then closes) and gives what happened.
async fn run(connection: socketio::SioConnection, commands: Vec<SioCommand>) -> Vec<SioEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    for command in commands {
        tx.send(command).unwrap();
    }
    let closer = tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(400)).await;
        let _ = closer.send(SioCommand::Close);
    });
    let mut events = Vec::new();
    connection.run(rx, |event| events.push(event)).await;
    events
}

fn emit(event: &str, args: Vec<serde_json::Value>, ack: bool) -> SioCommand {
    SioCommand::Emit {
        event: event.into(),
        args,
        ack,
    }
}

#[tokio::test]
async fn events_acks_and_pings() {
    let (url, seen) = start().await;
    let mut req = request(&url, Default::default());
    req.query.push(KeyValue::new("room", "7"));
    req.auth = Auth::Bearer { token: "t".into() };
    let connection = socketio::connect(&req).await.unwrap();
    assert_eq!(connection.status, 101);
    assert_eq!(connection.namespace, "/");
    assert_eq!(connection.sid, "s1");
    assert_eq!(connection.ping_interval, Some(Duration::from_secs(25)));
    assert_eq!(
        seen.lock().unwrap()[0],
        "/socket.io/?room=7&EIO=4&transport=websocket"
    );

    let events = run(
        connection,
        vec![
            emit("echo", vec![json!({"hi": 1}), json!("two")], false),
            emit("save", vec![json!(5)], true),
            emit("save", vec![], true),
        ],
    )
    .await;
    let received: Vec<&SioEvent> = events
        .iter()
        .filter(|e| !matches!(e, SioEvent::Sent(_)))
        .collect();
    assert_eq!(
        received[..4],
        [
            &SioEvent::Received(SioMessage {
                event: "welcome".into(),
                args: vec![json!({"namespace": "/", "auth": null})],
                ack: None,
            }),
            &SioEvent::Received(SioMessage {
                event: "echo".into(),
                args: vec![json!({"hi": 1}), json!("two")],
                ack: None,
            }),
            &SioEvent::Acked {
                id: 0,
                args: vec![json!("got save")]
            },
            &SioEvent::Acked {
                id: 1,
                args: vec![json!("got save")]
            },
        ],
        "{events:?}"
    );
    assert!(matches!(events.last(), Some(SioEvent::Closed { .. })));
    let seen = seen.lock().unwrap();
    // The ping was answered; we left the namespace before closing.
    assert!(seen.contains(&"pong".to_string()), "{seen:?}");
    assert!(seen.contains(&"left".to_string()), "{seen:?}");
    assert!(seen.contains(&r#"42["echo",{"hi":1},"two"]"#.to_string()));
    assert!(seen.contains(&r#"420["save",5]"#.to_string()));
}

#[tokio::test]
async fn namespaces_auth_and_refusals() {
    let (url, seen) = start().await;
    // The namespace from the URL's path, the auth from the options.
    let options = SocketIoOptions {
        auth: r#"{"token": "letmein"}"#.into(),
        ..Default::default()
    };
    let connection = socketio::connect(&request(&format!("{url}/private"), options))
        .await
        .unwrap();
    assert_eq!(connection.namespace, "/private");
    assert!(
        seen.lock()
            .unwrap()
            .contains(&r#"40/private,{"token":"letmein"}"#.to_string())
    );
    let events = run(connection, vec![emit("echo", vec![json!(1)], false)]).await;
    assert!(events.contains(&SioEvent::Received(SioMessage {
        event: "echo".into(),
        args: vec![json!(1)],
        ack: None,
    })));

    let refused = socketio::connect(&request(&format!("{url}/private"), Default::default()))
        .await
        .err()
        .unwrap();
    assert_eq!(
        refused.to_string(),
        "the server refused to join /private: not authorized"
    );
    let options = SocketIoOptions {
        namespace: "nowhere".into(),
        ..Default::default()
    };
    let refused = socketio::connect(&request(&url, options))
        .await
        .err()
        .unwrap();
    assert!(
        refused.to_string().contains("Invalid namespace"),
        "{refused}"
    );

    let bad = SocketIoOptions {
        auth: "[1]".into(),
        ..Default::default()
    };
    let error = socketio::connect(&request(&url, bad)).await.err().unwrap();
    assert!(error.to_string().contains("JSON object"), "{error}");
}

#[tokio::test]
async fn binary_attachments_and_the_server_leaving() {
    let (url, _) = start().await;
    let options = SocketIoOptions {
        namespace: "/chat".into(),
        ..Default::default()
    };
    let connection = socketio::connect(&request(&url, options)).await.unwrap();
    let file = json!({"name": "a.bin", "data": socketio::binary_value(&[1, 2, 255])});
    let events = run(
        connection,
        vec![
            emit("echo", vec![file.clone()], true),
            emit("ask", vec![], false),
            emit("bye", vec![], false),
        ],
    )
    .await;
    assert!(
        events.contains(&SioEvent::Received(SioMessage {
            event: "echo".into(),
            args: vec![file],
            ack: None,
        })),
        "{events:?}"
    );
    assert!(events.contains(&SioEvent::Acked {
        id: 0,
        args: vec![json!("got echo")]
    }));
    // The server asks us to acknowledge.
    assert!(events.contains(&SioEvent::Received(SioMessage {
        event: "question".into(),
        args: vec![json!("ready?")],
        ack: Some(9),
    })));
    let end = &events[events.len() - 2..];
    assert_eq!(end[0], SioEvent::Disconnected, "{events:?}");
    assert!(matches!(end[1], SioEvent::Closed { .. }));
}

#[tokio::test]
async fn a_plain_websocket_server_is_not_socketio() {
    use futures_util::SinkExt as _;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/", listener.local_addr().unwrap());
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut stream = tokio_tungstenite::accept_async(socket).await.unwrap();
        let hello = tokio_tungstenite::tungstenite::Message::text(r#"{"type":"welcome"}"#);
        stream.send(hello).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
    });
    let error = socketio::connect(&request(&url, Default::default()))
        .await
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .starts_with("not a Socket.IO server (Engine.IO 4): it sent `{\"type\""),
        "{error}"
    );
}
