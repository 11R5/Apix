//! Request settings and the cookie jar against a tiny local server.

use apix_core::{Method, Request};
use apix_http::{HttpClient, HttpError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// `/start` redirects to `/end` and sets a cookie; `/end` answers "ok" and
/// echoes the received Cookie header in `X-Got-Cookie`. `/slow` waits 2 s.
async fn serve(listener: TcpListener) {
    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]).into_owned();
            let answer = if head.starts_with("GET /start") {
                "HTTP/1.1 302 Found\r\nLocation: /end\r\nSet-Cookie: sid=abc123; Path=/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned()
            } else if head.starts_with("GET /slow") {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_owned()
            } else {
                let cookie = head
                    .lines()
                    .find_map(|l| l.strip_prefix("cookie: ").or(l.strip_prefix("Cookie: ")))
                    .unwrap_or("")
                    .to_owned();
                format!(
                    "HTTP/1.1 200 OK\r\nX-Got-Cookie: {cookie}\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
                )
            };
            let _ = socket.write_all(answer.as_bytes()).await;
            let _ = socket.shutdown().await;
        });
    }
}

async fn start_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(serve(listener));
    port
}

#[tokio::test]
async fn redirects_off_returns_the_3xx_itself() {
    let port = start_server().await;
    let mut request = Request::new(Method::Get, format!("http://localhost:{port}/start"));
    request.settings.follow_redirects = Some(false);
    let response = HttpClient::new().unwrap().send(&request).await.unwrap();
    assert_eq!(response.status, 302);
    assert_eq!(response.header("location"), Some("/end"));
    assert!(response.trace.redirects.is_empty());
}

#[tokio::test]
async fn a_timeout_is_reported_as_timed_out() {
    let port = start_server().await;
    let mut request = Request::new(Method::Get, format!("http://localhost:{port}/slow"));
    request.settings.timeout_ms = Some(200);
    let err = HttpClient::new().unwrap().send(&request).await.unwrap_err();
    assert!(matches!(err, HttpError::TimedOut), "{err:?}");
}

#[tokio::test]
async fn cookies_are_kept_listed_and_removable() {
    let port = start_server().await;
    let client = HttpClient::new().unwrap();
    let base = format!("http://localhost:{port}");

    // /start sets the cookie on the redirect hop; /end must get it back.
    let request = Request::new(Method::Get, format!("{base}/start"));
    let response = client.send(&request).await.unwrap();
    assert_eq!(response.header("x-got-cookie"), Some("sid=abc123"));

    let cookies = client.cookies();
    assert_eq!(cookies.len(), 1, "{cookies:?}");
    assert_eq!(cookies[0].name, "sid");
    assert_eq!(cookies[0].value, "abc123");
    assert_eq!(cookies[0].path, "/");
    assert!(cookies[0].expires.is_none(), "a session cookie");
    let domain = cookies[0].domain.clone();

    // With cookies off for one request, nothing is sent (and the jar is kept).
    let mut request = Request::new(Method::Get, format!("{base}/end"));
    request.settings.send_cookies = Some(false);
    let response = client.send(&request).await.unwrap();
    assert_eq!(response.header("x-got-cookie"), Some(""));
    assert_eq!(client.cookies().len(), 1);

    client.remove_cookie(&domain, "/", "sid");
    assert!(client.cookies().is_empty());
    let response = client.send(&Request::new(Method::Get, format!("{base}/end"))).await.unwrap();
    assert_eq!(response.header("x-got-cookie"), Some(""));
}

#[tokio::test]
async fn a_cookie_file_persists_across_clients() {
    let port = start_server().await;
    let dir = std::env::temp_dir().join(format!("apix-cookies-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("cookies.json");

    // Expiring cookies persist; session cookies (like /start's) do not.
    let client = HttpClient::with_cookie_file(&file).unwrap();
    let request = Request::new(Method::Get, format!("http://localhost:{port}/start"));
    client.send(&request).await.unwrap();
    assert_eq!(client.cookies().len(), 1);
    assert!(file.is_file());

    let reopened = HttpClient::with_cookie_file(&file).unwrap();
    assert!(
        reopened.cookies().is_empty(),
        "session cookies die with the client"
    );
    std::fs::remove_dir_all(&dir).ok();
}
