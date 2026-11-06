//! Load tests against a small local server that keeps connections open:
//! counts, reuse of connections, a steady rate, a time limit, stopping, and
//! failures counted by kind.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use apix_core::{Method, Request};
use apix_http::load::Load;
use apix_runner::bench::{Bench, Plan};

/// Serves HTTP/1.1 with keep-alive; `/fail` answers 500, `/slow` waits 50 ms.
/// Returns the address and how many connections were opened.
fn serve() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let connections = Arc::new(AtomicUsize::new(0));
    let count = connections.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            count.fetch_add(1, Ordering::Relaxed);
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut stream = stream;
                loop {
                    let mut first = String::new();
                    if reader.read_line(&mut first).unwrap_or(0) == 0 {
                        return;
                    }
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap_or(0) == 0 {
                            return;
                        }
                        if line.trim().is_empty() {
                            break;
                        }
                    }
                    let path = first.split_whitespace().nth(1).unwrap_or("/").to_owned();
                    if path == "/slow" {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    let status = if path == "/fail" {
                        "500 Internal Server Error"
                    } else {
                        "200 OK"
                    };
                    let body = "{\"ok\":true}";
                    if write!(
                        stream,
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                        body.len()
                    )
                    .is_err()
                    {
                        return;
                    }
                }
            });
        }
    });
    (format!("http://{addr}"), connections)
}

async fn load(url: &str) -> Load {
    Load::new(&Request::new(Method::Get, url)).await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_number_of_requests_over_kept_connections() {
    let (base, connections) = serve();
    let load = load(&format!("{base}/users")).await;
    let plan = Plan {
        concurrency: 8,
        requests: Some(400),
        duration: None,
        rate: None,
    };
    let summary = Bench::new().run(&load, &plan).await;
    assert_eq!(summary.requests, 400);
    assert_eq!((summary.ok, summary.failed), (400, 0));
    assert_eq!(summary.statuses[&200], 400);
    assert_eq!(summary.bytes, 400 * 11);
    assert!(summary.done);
    assert!(summary.min_ms <= summary.p50_ms && summary.p50_ms <= summary.p99_ms);
    assert!(summary.p99_ms <= summary.max_ms);
    assert_eq!(summary.seconds.iter().map(|s| s.requests).sum::<u64>(), 400);
    // About one connection per worker, kept and reused (not one per request).
    let opened = connections.load(Ordering::Relaxed);
    assert!(
        opened <= 16,
        "{opened} connections for 8 workers and 400 requests"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_steady_rate() {
    let (base, _) = serve();
    let load = load(&base).await;
    let plan = Plan {
        concurrency: 4,
        requests: Some(20),
        duration: None,
        rate: Some(50.0),
    };
    let started = Instant::now();
    let summary = Bench::new().run(&load, &plan).await;
    assert_eq!(summary.requests, 20);
    // 20 requests at 50 a second: the last starts after 380 ms.
    assert!(
        started.elapsed() >= Duration::from_millis(370),
        "{:?}",
        started.elapsed()
    );
    assert!(summary.per_second < 60.0, "{}", summary.per_second);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_time_limit_and_stopping() {
    let (base, _) = serve();
    let load = load(&format!("{base}/slow")).await;
    let plan = Plan {
        concurrency: 2,
        requests: None,
        duration: Some(Duration::from_millis(300)),
        rate: None,
    };
    let started = Instant::now();
    let summary = Bench::new().run(&load, &plan).await;
    let took = started.elapsed();
    assert!(
        took >= Duration::from_millis(300) && took < Duration::from_millis(900),
        "{took:?}"
    );
    // Two workers, 50 ms each: about 12 requests, fewer on a slow CI machine.
    assert!((3..=16).contains(&summary.requests), "{}", summary.requests);
    assert!(summary.p50_ms >= 50.0, "{}", summary.p50_ms);

    // Stopped from elsewhere while it runs.
    let bench = Bench::new();
    let stopper = bench.clone();
    let long = Plan {
        duration: Some(Duration::from_secs(30)),
        ..plan
    };
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let partial = stopper.summary();
        assert!(!partial.done);
        stopper.stop();
    });
    let started = Instant::now();
    let summary = bench.run(&load, &long).await;
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert!(summary.done && summary.requests > 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failures_by_kind() {
    let (base, _) = serve();
    let summary = Bench::new()
        .run(
            &load(&format!("{base}/fail")).await,
            &Plan {
                concurrency: 2,
                requests: Some(10),
                duration: None,
                rate: None,
            },
        )
        .await;
    assert_eq!((summary.ok, summary.failed), (0, 10));
    assert_eq!(summary.statuses[&500], 10);

    // Nothing listens there.
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let summary = Bench::new()
        .run(
            &load(&url).await,
            &Plan {
                concurrency: 2,
                requests: Some(2),
                duration: None,
                rate: None,
            },
        )
        .await;
    assert_eq!(summary.failed, 2);
    assert_eq!(
        summary.errors.get("could not connect"),
        Some(&2),
        "{:?}",
        summary.errors
    );
    assert!(summary.statuses.is_empty());
}
