//! Serves server-sent events for trying Apix's live event view:
//!
//! ```sh
//! cargo run -p apix-http --example sse_server
//! ```
//!
//! then send `GET http://127.0.0.1:8767/` (endless: a `tick` every second and
//! a `price` now and then) or `GET http://127.0.0.1:8767/5` (five events, then
//! the stream ends). A client that reconnects with Last-Event-ID gets the
//! events after that id.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8767").await?;
    println!("serving server-sent events on http://127.0.0.1:8767/ (and /5 for five)");
    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]);
            let path = head.split_whitespace().nth(1).unwrap_or("/").to_owned();
            let limit: Option<u64> = path.trim_start_matches('/').parse().ok();
            // A client that reconnects carries on after the last id it saw.
            let resume: u64 = head
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.trim()
                        .eq_ignore_ascii_case("last-event-id")
                        .then(|| v.trim().parse().ok())?
                })
                .unwrap_or(0);
            let start = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n: connected\n\nretry: 5000\n\n";
            if socket.write_all(start).await.is_err() {
                return;
            }
            let mut n = resume;
            loop {
                n += 1;
                let event = if n.is_multiple_of(4) {
                    format!(
                        "event: price\nid: {n}\ndata: {{\"symbol\":\"APX\",\"price\":{}.{:02}}}\n\n",
                        100 + n % 17,
                        (n * 37) % 100
                    )
                } else {
                    format!("event: tick\nid: {n}\ndata: {{\"n\":{n}}}\n\n")
                };
                if socket.write_all(event.as_bytes()).await.is_err() {
                    return;
                }
                if limit.is_some_and(|limit| n - resume >= limit) {
                    let _ = socket.shutdown().await;
                    return;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
    }
}
