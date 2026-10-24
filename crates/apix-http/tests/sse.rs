//! Server-sent events against a tiny local server that sends them slowly.

use std::time::{Duration, Instant};

use apix_core::{Method, Request};
use apix_http::sse::SseParser;
use apix_http::{HttpClient, Progress};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Answers with three events, 150 ms apart, then closes the stream.
async fn serve(listener: TcpListener) {
    let (mut socket, _) = listener.accept().await.unwrap();
    let mut buf = vec![0u8; 4096];
    let _ = socket.read(&mut buf).await;
    socket
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        )
        .await
        .unwrap();
    for (ix, piece) in [
        "data: one\n\n",
        "event: tick\nid: 2\ndata: two\n",
        "\ndata: three\n\n",
    ]
    .iter()
    .enumerate()
    {
        if ix > 0 {
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        socket.write_all(piece.as_bytes()).await.unwrap();
        socket.flush().await.unwrap();
    }
}

#[tokio::test]
async fn events_arrive_while_the_stream_is_open() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/events", listener.local_addr().unwrap());
    tokio::spawn(serve(listener));

    let started = Instant::now();
    let mut head_at = None;
    let mut parser = SseParser::new();
    let mut events = Vec::new();
    let response = HttpClient::new()
        .unwrap()
        .send_with(
            &Request::new(Method::Get, &url),
            |progress| match progress {
                Progress::Head { status, headers } => {
                    assert_eq!(status, 200);
                    assert!(
                        headers
                            .iter()
                            .any(|(k, v)| k == "content-type" && v == "text/event-stream")
                    );
                    head_at = Some(started.elapsed());
                }
                Progress::Chunk(chunk) => {
                    for event in parser.feed(chunk) {
                        events.push((started.elapsed(), event));
                    }
                }
            },
        )
        .await
        .unwrap();

    assert!(response.is_event_stream());
    let names: Vec<(&str, &str)> = events
        .iter()
        .map(|(_, e)| (e.event.as_str(), e.data.as_str()))
        .collect();
    assert_eq!(
        names,
        [("message", "one"), ("tick", "two"), ("message", "three")]
    );
    assert_eq!(events[1].1.id.as_deref(), Some("2"));
    // The head and the first event came before the stream was over.
    let total = started.elapsed();
    assert!(head_at.unwrap() + Duration::from_millis(200) < total);
    assert!(events[0].0 + Duration::from_millis(200) < events[2].0);
    // And the response still holds the whole body.
    assert_eq!(apix_http::sse::parse_all(&response.body).len(), 3);
}
