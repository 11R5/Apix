//! The signing auth types against a tiny local server.

use std::sync::{Arc, Mutex};

use apix_core::{ApiKeyLocation, Auth, AwsSigV4, Method, OAuth1, OAuth2, Request};
use apix_http::{HttpClient, oauth2};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Every request the server saw: request line, headers and body.
type Seen = Arc<Mutex<Vec<String>>>;

/// `/token` hands out `tok-1`, `tok-2`, …; `/digest` asks for Digest auth
/// until it gets an answer; anything else echoes its request line and
/// Authorization header.
async fn serve(listener: TcpListener, seen: Seen) {
    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let seen = seen.clone();
        tokio::spawn(async move {
            let mut data = Vec::new();
            let mut buf = [0u8; 4096];
            // Head, then as much body as Content-Length says.
            loop {
                let n = socket.read(&mut buf).await.unwrap_or(0);
                data.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&data).to_string();
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
                if n == 0 {
                    break;
                }
            }
            let text = String::from_utf8_lossy(&data).to_string();
            let line = text.lines().next().unwrap_or_default().to_owned();
            let auth = text
                .lines()
                .find_map(|l| l.strip_prefix("authorization: "))
                .unwrap_or_default()
                .to_owned();
            let count = {
                let mut seen = seen.lock().unwrap();
                seen.push(text.clone());
                seen.iter().filter(|r| r.starts_with("POST /token")).count()
            };
            let (status, extra, body) = if line.starts_with("POST /token") {
                let body = format!(r#"{{"access_token":"tok-{count}","expires_in":3600}}"#);
                ("200 OK", String::new(), body)
            } else if line.starts_with("GET /digest") && auth.is_empty() {
                (
                    "401 Unauthorized",
                    "WWW-Authenticate: Digest realm=\"r\", nonce=\"n1\", qop=\"auth\"\r\n".into(),
                    String::new(),
                )
            } else {
                ("200 OK", String::new(), format!("{line}\n{auth}"))
            };
            let answer = format!(
                "HTTP/1.1 {status}\r\n{extra}Content-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(answer.as_bytes()).await;
        });
    }
}

async fn server() -> (u16, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Seen::default();
    tokio::spawn(serve(listener, seen.clone()));
    (port, seen)
}

async fn send(url: String, auth: Auth) -> String {
    let mut request = Request::new(Method::Get, url);
    request.auth = auth;
    let response = HttpClient::new().unwrap().send(&request).await.unwrap();
    assert_eq!(response.status, 200, "{}", response.raw_body());
    response.raw_body()
}

#[tokio::test]
async fn oauth2_client_credentials_are_fetched_once_and_cached() {
    let (port, seen) = server().await;
    let config = OAuth2 {
        token_url: format!("http://127.0.0.1:{port}/token"),
        client_id: "app".into(),
        client_secret: "s3cret".into(),
        scope: "read".into(),
        ..OAuth2::default()
    };
    let api = format!("http://127.0.0.1:{port}/api");
    assert!(
        send(api.clone(), Auth::OAuth2(config.clone()))
            .await
            .ends_with("Bearer tok-1")
    );
    assert!(
        send(api.clone(), Auth::OAuth2(config.clone()))
            .await
            .ends_with("Bearer tok-1")
    );

    let token_request = seen.lock().unwrap()[0].clone();
    // base64("app:s3cret")
    assert!(
        token_request.contains("authorization: Basic YXBwOnMzY3JldA=="),
        "{token_request}"
    );
    assert!(
        token_request.ends_with("grant_type=client_credentials&scope=read"),
        "{token_request}"
    );
    assert!(
        oauth2::cached(&config)
            .unwrap()
            .expires_in()
            .unwrap()
            .as_secs()
            > 3500
    );

    oauth2::forget(&config);
    assert!(
        send(api, Auth::OAuth2(config))
            .await
            .ends_with("Bearer tok-2")
    );
}

#[tokio::test]
async fn digest_answers_the_challenge() {
    let (port, seen) = server().await;
    let auth = Auth::Digest {
        username: "u".into(),
        password: "p".into(),
    };
    let echoed = send(format!("http://127.0.0.1:{port}/digest?x=1"), auth).await;
    assert!(
        echoed.contains("Digest username=\"u\", realm=\"r\", nonce=\"n1\", uri=\"/digest?x=1\""),
        "{echoed}"
    );
    assert!(echoed.contains("qop=auth, nc=00000001"), "{echoed}");
    assert_eq!(seen.lock().unwrap().len(), 2, "one challenge, one answer");
}

#[tokio::test]
async fn aws_and_oauth1_sign_the_request() {
    let (port, _) = server().await;
    let aws = Auth::AwsSigV4(AwsSigV4 {
        access_key: "AK".into(),
        secret_key: "SK".into(),
        session_token: String::new(),
        region: "eu-west-1".into(),
        service: "execute-api".into(),
    });
    let echoed = send(format!("http://127.0.0.1:{port}/api"), aws).await;
    assert!(
        echoed.contains("AWS4-HMAC-SHA256 Credential=AK/")
            && echoed.contains("/eu-west-1/execute-api/aws4_request"),
        "{echoed}"
    );

    let oauth1 = OAuth1 {
        consumer_key: "ck".into(),
        consumer_secret: "cs".into(),
        ..OAuth1::default()
    };
    let echoed = send(
        format!("http://127.0.0.1:{port}/api"),
        Auth::OAuth1(oauth1.clone()),
    )
    .await;
    assert!(
        echoed.contains("OAuth oauth_consumer_key=\"ck\""),
        "{echoed}"
    );

    let in_query = Auth::OAuth1(OAuth1 {
        location: ApiKeyLocation::Query,
        ..oauth1
    });
    let echoed = send(format!("http://127.0.0.1:{port}/api?a=1"), in_query).await;
    let line = echoed.lines().next().unwrap();
    assert!(
        line.starts_with("GET /api?a=1&oauth_consumer_key=ck&"),
        "{line}"
    );
    assert!(line.contains("&oauth_signature="), "{line}");
}
