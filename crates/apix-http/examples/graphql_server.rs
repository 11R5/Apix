//! A small GraphQL server for trying Apix's schema fetch and completion:
//!
//! ```sh
//! cargo run -p apix-http --example graphql_server
//! ```
//!
//! then make a GraphQL request to `http://127.0.0.1:8769/graphql` and fetch
//! its schema. It answers the introspection query with a schema of users,
//! posts and roles, and any other query with a few users.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const SCHEMA: &str = include_str!("../../apix-core/tests/fixtures/graphql-schema.json");
const USERS: &str = r#"{"data":{"users":[{"id":"1","name":"Ada Lovelace","role":"ADMIN"},{"id":"2","name":"Alan Turing","role":"USER"}]}}"#;

/// The request's head and body (as much as Content-Length says).
async fn read_request(socket: &mut TcpStream) -> String {
    let mut data = Vec::new();
    let mut buf = vec![0u8; 8192];
    while let Ok(n) = socket.read(&mut buf).await {
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        let text = String::from_utf8_lossy(&data);
        if let Some(end) = text.find("\r\n\r\n") {
            let length = text[..end]
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("content-length")
                        .then(|| v.trim().parse::<usize>().ok())?
                })
                .unwrap_or(0);
            if data.len() >= end + 4 + length {
                break;
            }
        }
    }
    String::from_utf8_lossy(&data).into_owned()
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8769").await?;
    println!("serving GraphQL on http://127.0.0.1:8769/graphql");
    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let request = read_request(&mut socket).await;
            let body = if request.contains("__schema") {
                SCHEMA
            } else {
                USERS
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        });
    }
}
