//! A tiny OAuth 2.0 server for trying the OAuth 2.0 auth type by hand:
//!
//! ```sh
//! cargo run -p apix-http --example oauth_server
//! ```
//!
//! - `POST http://127.0.0.1:8766/token` hands out `token-1`, `token-2`, …,
//!   valid for 120 seconds, to client `apix` with secret `secret` (Basic auth
//!   or in the body). Any grant type is accepted.
//! - `GET http://127.0.0.1:8766/authorize` sends the browser straight back
//!   to `redirect_uri` with a code, as if the user had signed in.
//! - `GET http://127.0.0.1:8766/me` answers 200 with the bearer token it got,
//!   or 401 without one.

use std::sync::atomic::{AtomicUsize, Ordering};

use reqwest::Url;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

static ISSUED: AtomicUsize = AtomicUsize::new(0);

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8766").await?;
    println!("OAuth 2.0 test server on http://127.0.0.1:8766 (client apix, secret secret)");
    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let mut buf = vec![0u8; 16 * 1024];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let target = request
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .unwrap_or("/")
                .to_owned();
            let url = Url::parse(&format!("http://127.0.0.1:8766{target}")).expect("a URL");
            let header = |name: &str| {
                request.lines().find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case(name).then(|| v.trim().to_owned())
                })
            };
            let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();
            let client_ok = header("authorization").as_deref() == Some("Basic YXBpeDpzZWNyZXQ=")
                || (body.contains("client_id=apix") && body.contains("client_secret=secret"))
                || (body.contains("client_id=apix") && body.contains("code_verifier="));

            let (status, extra, text) = match (request.split(' ').next(), url.path()) {
                (Some("POST"), "/token") if client_ok => {
                    let n = ISSUED.fetch_add(1, Ordering::Relaxed) + 1;
                    println!("issued token-{n}: {}", body.trim());
                    (
                        "200 OK",
                        "Content-Type: application/json\r\n".to_owned(),
                        format!(
                            r#"{{"access_token":"token-{n}","token_type":"Bearer","expires_in":120,"refresh_token":"refresh-{n}"}}"#
                        ),
                    )
                }
                (Some("POST"), "/token") => (
                    "401 Unauthorized",
                    "Content-Type: application/json\r\n".to_owned(),
                    r#"{"error":"invalid_client","error_description":"use client apix with secret secret"}"#.to_owned(),
                ),
                (_, "/authorize") => {
                    let query: std::collections::HashMap<_, _> =
                        url.query_pairs().into_owned().collect();
                    let mut back = Url::parse(&query["redirect_uri"]).expect("a redirect URI");
                    back.query_pairs_mut()
                        .append_pair("code", "code-123")
                        .append_pair("state", &query["state"]);
                    ("302 Found", format!("Location: {back}\r\n"), String::new())
                }
                (_, "/me") => match header("authorization") {
                    Some(auth) => ("200 OK", String::new(), format!("hello, {auth}")),
                    None => ("401 Unauthorized", String::new(), "no token".to_owned()),
                },
                _ => ("404 Not Found", String::new(), "not found".to_owned()),
            };
            let answer = format!(
                "HTTP/1.1 {status}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{text}",
                text.len()
            );
            let _ = socket.write_all(answer.as_bytes()).await;
            let _ = socket.shutdown().await;
        });
    }
}
