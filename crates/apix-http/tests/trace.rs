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
    assert!(trace.tls.is_none() && trace.connects.iter().all(|c| c.tls.is_none()));

    let phases = trace.phases(response.timing.ttfb, response.timing.total);
    assert!(
        phases.dns + phases.tcp + phases.tls + phases.waiting + phases.download
            <= response.timing.total
    );
}

/// Answers with a gzip body, and says in a header which encodings were asked for.
async fn serve_gzip(listener: TcpListener) {
    use std::io::Write as _;
    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase();
            let asked = head
                .lines()
                .find_map(|l| l.strip_prefix("accept-encoding: "))
                .unwrap_or("none")
                .to_owned();
            let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            gz.write_all("hello ".repeat(1000).as_bytes()).unwrap();
            let body = gz.finish().unwrap();
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nX-Asked: {asked}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(head.as_bytes()).await;
            let _ = socket.write_all(&body).await;
            let _ = socket.shutdown().await;
        });
    }
}

#[tokio::test]
async fn compressed_bodies_are_decoded_and_measured() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    tokio::spawn(serve_gzip(listener));
    let client = HttpClient::new().unwrap();

    let response = client.send(&Request::new(Method::Get, &url)).await.unwrap();
    assert_eq!(response.header("x-asked"), Some("gzip, deflate, br"));
    assert_eq!(
        response.header("content-encoding"),
        Some("gzip"),
        "kept as sent"
    );
    assert_eq!(response.body.len(), 6000);
    assert!(response.raw_body().starts_with("hello hello"));
    let compression = response.trace.compression.clone().expect("compression");
    assert_eq!(compression.encoding, "gzip");
    assert_eq!(compression.decoded, 6000);
    assert!(compression.encoded < 200, "{compression:?}");

    // A request that says what it accepts keeps it.
    let mut request = Request::new(Method::Get, &url);
    request
        .headers
        .push(apix_core::KeyValue::new("Accept-Encoding", "gzip"));
    let response = client.send(&request).await.unwrap();
    assert_eq!(response.header("x-asked"), Some("gzip"));
}
