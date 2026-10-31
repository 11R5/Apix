//! A small Socket.IO 4 server (Engine.IO 4, WebSocket only) for the tests
//! and the `socketio_server` example. It knows the namespaces `/`, `/chat`,
//! `/ticker` (an event every second) and `/private` (joins only with
//! `{"token": "letmein"}`), and on each:
//!
//! - greets with `welcome` (the namespace and the auth it was given),
//! - pings once after joining,
//! - echoes `echo` events back, binary attachments too,
//! - acknowledges every event that asks for it with `["got <event>"]`,
//! - makes the client leave the namespace on `bye`,
//! - asks the client to acknowledge `question` after `ask`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

/// What the server saw: the handshake address, then `pong`, `left` and
/// every client packet as it came.
pub type Seen = Arc<Mutex<Vec<String>>>;

pub async fn serve(listener: TcpListener, seen: Seen) {
    let mut n = 0u64;
    loop {
        let Ok((socket, _)) = listener.accept().await else {
            return;
        };
        n += 1;
        tokio::spawn(session(socket, n, seen.clone()));
    }
}

async fn session(socket: tokio::net::TcpStream, n: u64, seen: Seen) {
    let note = |text: String| seen.lock().unwrap().push(text);
    #[allow(clippy::result_large_err)]
    let callback = |req: &Request, response: Response| {
        note(req.uri().to_string());
        Ok(response)
    };
    let Ok(stream) = tokio_tungstenite::accept_hdr_async(socket, callback).await else {
        return;
    };
    let (mut write, mut read) = stream.split();
    let open = json!({
        "sid": format!("eio{n}"),
        "upgrades": [],
        "pingInterval": 25000,
        "pingTimeout": 20000,
        "maxPayload": 1000000,
    });
    if write.send(Message::text(format!("0{open}"))).await.is_err() {
        return;
    }
    let mut namespace = String::new();
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    let mut ticks = 0u64;
    // A binary event waiting for its attachment.
    let mut waiting: Option<(Option<u64>, Value)> = None;
    loop {
        let message = tokio::select! {
            message = read.next() => message,
            _ = ticker.tick(), if namespace == "/ticker" => {
                ticks += 1;
                let tick = json!(["tick", {"n": ticks}]);
                let _ = write.send(Message::text(format!("42/ticker,{tick}"))).await;
                continue;
            }
        };
        let Some(Ok(message)) = message else {
            return;
        };
        let text = match message {
            Message::Text(text) => text.as_str().to_owned(),
            Message::Binary(bytes) => {
                // The attachment of a binary event: echo it, then acknowledge.
                if let Some((id, data)) = waiting.take() {
                    let event = data[0].as_str().unwrap_or_default().to_owned();
                    let packet = format!("451-{}{data}", prefix(&namespace));
                    let _ = write.send(Message::text(packet)).await;
                    let _ = write.send(Message::binary(bytes)).await;
                    if let Some(id) = id {
                        let ack = json!([format!("got {event}")]);
                        let _ = write
                            .send(Message::text(format!("43{}{id}{ack}", prefix(&namespace))))
                            .await;
                    }
                }
                continue;
            }
            Message::Close(_) => return,
            _ => continue,
        };
        note(text.clone());
        if text == "3" {
            note("pong".into());
            continue;
        }
        let Some(packet) = text.strip_prefix('4') else {
            continue;
        };
        let (kind, rest) = packet.split_at(1);
        let (attachments, rest) = match kind {
            "5" => {
                let dash = rest.find('-').unwrap();
                (rest[..dash].parse::<usize>().unwrap(), &rest[dash + 1..])
            }
            _ => (0, rest),
        };
        let (nsp, rest) = match rest.strip_prefix('/') {
            Some(_) => {
                let end = rest.find(',').unwrap_or(rest.len());
                (&rest[..end], rest.get(end + 1..).unwrap_or_default())
            }
            None => ("/", rest),
        };
        let digits = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        let id: Option<u64> = rest[..digits].parse().ok();
        let data: Value = serde_json::from_str(&rest[digits..]).unwrap_or(Value::Null);
        match kind {
            "0" => {
                let to = prefix(nsp);
                let refused = match nsp {
                    "/" | "/chat" | "/ticker" => None,
                    "/private" if data["token"] == "letmein" => None,
                    "/private" => Some("not authorized"),
                    _ => Some("Invalid namespace"),
                };
                if let Some(message) = refused {
                    let error = json!({ "message": message });
                    let _ = write.send(Message::text(format!("44{to}{error}"))).await;
                    continue;
                }
                namespace = nsp.to_owned();
                let joined = json!({ "sid": format!("s{n}") });
                let welcome = json!(["welcome", { "namespace": nsp, "auth": data }]);
                let _ = write.send(Message::text(format!("40{to}{joined}"))).await;
                let _ = write.send(Message::text(format!("42{to}{welcome}"))).await;
                let _ = write.send(Message::text("2")).await;
            }
            "1" => note("left".into()),
            "2" => {
                let event = data[0].as_str().unwrap_or_default().to_owned();
                let to = prefix(nsp);
                match event.as_str() {
                    "echo" => {
                        let _ = write.send(Message::text(format!("42{to}{data}"))).await;
                    }
                    "bye" => {
                        let _ = write.send(Message::text(format!("41{to}"))).await;
                    }
                    "ask" => {
                        let question = json!(["question", "ready?"]);
                        let _ = write
                            .send(Message::text(format!("42{to}9{question}")))
                            .await;
                    }
                    _ => {}
                }
                if let Some(id) = id {
                    let ack = json!([format!("got {event}")]);
                    let _ = write.send(Message::text(format!("43{to}{id}{ack}"))).await;
                }
            }
            "5" if attachments > 0 => waiting = Some((id, data)),
            _ => {}
        }
    }
}

/// The namespace as packets write it: nothing for `/`, else `/chat,`.
fn prefix(namespace: &str) -> String {
    match namespace {
        "/" | "" => String::new(),
        nsp => format!("{nsp},"),
    }
}
