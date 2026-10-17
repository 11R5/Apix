//! Serves one large, minified JSON body for testing how Apix handles huge
//! responses:
//!
//! ```sh
//! cargo run -p apix-http --release --example big_json_server -- 100
//! ```
//!
//! then send `GET http://127.0.0.1:8765/` (the argument is the size in MB).

use std::io::Write as _;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let mb: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(100);
    let body = Arc::new(generate(mb * 1024 * 1024));
    let listener = TcpListener::bind("127.0.0.1:8765").await?;
    println!(
        "serving {:.1} MB of JSON on http://127.0.0.1:8765/",
        body.len() as f64 / 1_048_576.0
    );

    loop {
        let (mut socket, _) = listener.accept().await?;
        let body = body.clone();
        tokio::spawn(async move {
            let mut request = [0u8; 8192];
            if socket.read(&mut request).await.is_err() {
                return;
            }
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(head.as_bytes()).await;
            let _ = socket.write_all(&body).await;
        });
    }
}

fn generate(target: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(target + 512);
    out.push(b'[');
    let mut i: usize = 0;
    while out.len() < target {
        if i > 0 {
            out.push(b',');
        }
        write!(
            out,
            r#"{{"id":{i},"name":"User {i}","email":"user{i}@example.com","active":{},"score":{}.{},"tags":["alpha","beta"],"address":{{"city":"Istanbul","zip":"34{:03}"}}}}"#,
            i.is_multiple_of(2),
            i % 100,
            i % 10,
            i % 1000
        )
        .expect("writing to a Vec cannot fail");
        i += 1;
    }
    out.push(b']');
    out
}
