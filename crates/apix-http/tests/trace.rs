//! The network trace of a real request against a tiny local server.

use apix_core::{Method, Request};
use apix_http::HttpClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Answers `/start` with a redirect to `/end`, and `/end` with "ok".
/// Every answer closes the connection, so each hop needs a new one.
async fn serve(listener: TcpListener) {
    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]);
            let answer = if head.starts_with("GET /start") {
                "HTTP/1.1 302 Found\r\nLocation: /end\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            } else {
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
            };
            let _ = socket.write_all(answer.as_bytes()).await;
            let _ = socket.shutdown().await;
        });
    }
}

#[tokio::test]
async fn records_dns_connections_redirects_and_addresses() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(serve(listener));

    let request = Request::new(Method::Get, format!("http://localhost:{port}/start"));
    let response = HttpClient::new().unwrap().send(&request).await.unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(&response.body[..], b"ok");

    let trace = &response.trace;
    assert_eq!(trace.redirects.len(), 1, "{trace:#?}");
    assert_eq!(trace.redirects[0].status, 302);
    assert!(trace.redirects[0].to.ends_with("/end"));
    assert!(trace.dns.iter().all(|d| d.host == "localhost"));
    assert!(!trace.dns.is_empty() && !trace.dns[0].addrs.is_empty());
    assert_eq!(trace.connects.len(), 2, "one connection per hop");
    assert_eq!(trace.remote_addr.map(|a| a.port()), Some(port));
    assert!(trace.local_addr.is_some_and(|a| a.ip().is_loopback()));
    assert!(trace.certificate.is_none(), "plain HTTP has no certificate");

    let phases = trace.phases(response.timing.ttfb, response.timing.total);
    assert!(
        phases.dns + phases.connect + phases.waiting + phases.download <= response.timing.total
    );
}
