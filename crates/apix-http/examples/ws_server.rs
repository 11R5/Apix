//! A WebSocket server for trying Apix's WebSocket requests:
//!
//! ```sh
//! cargo run -p apix-http --example ws_server
//! ```
//!
//! `ws://127.0.0.1:8768/` greets you and echoes what you send (`bye` makes it
//! close the connection); `ws://127.0.0.1:8768/ticker` also pushes a message
//! every second.

use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::protocol::CloseFrame;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8768").await?;
    println!("WebSocket echo on ws://127.0.0.1:8768/ (and /ticker)");
    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let mut path = String::new();
            #[allow(clippy::result_large_err)]
            let callback = |req: &Request, response: Response| {
                path = req.uri().path().to_owned();
                Ok(response)
            };
            let Ok(stream) = tokio_tungstenite::accept_hdr_async(socket, callback).await else {
                return;
            };
            let (mut write, mut read) = stream.split();
            let _ = write
                .send(Message::text(
                    r#"{"type":"welcome","server":"apix example"}"#,
                ))
                .await;
            let mut ticker = tokio::time::interval(Duration::from_secs(1));
            let mut n = 0u64;
            loop {
                tokio::select! {
                    _ = ticker.tick(), if path == "/ticker" => {
                        n += 1;
                        let tick = format!(r#"{{"type":"tick","n":{n}}}"#);
                        if write.send(Message::text(tick)).await.is_err() {
                            return;
                        }
                    }
                    message = read.next() => match message {
                        Some(Ok(Message::Text(text))) if text.as_str().trim() == "bye" => {
                            let frame = CloseFrame { code: 1000.into(), reason: "bye".into() };
                            let _ = write.send(Message::Close(Some(frame))).await;
                            return;
                        }
                        Some(Ok(Message::Text(text))) => {
                            let echo = format!("echo: {}", text.as_str());
                            if write.send(Message::text(echo)).await.is_err() {
                                return;
                            }
                        }
                        Some(Ok(Message::Binary(bytes))) => {
                            let _ = write.send(Message::Binary(bytes)).await;
                        }
                        Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return,
                        Some(Ok(_)) => {}
                    },
                }
            }
        });
    }
}
