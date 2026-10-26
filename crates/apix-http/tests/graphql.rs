//! Fetching a GraphQL schema from a local server.

use std::sync::{Arc, Mutex};

use apix_core::{Auth, Method, Request};
use apix_http::HttpClient;
use apix_http::graphql::{SchemaError, fetch_schema};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const SCHEMA: &str = include_str!("../../apix-core/tests/fixtures/graphql-schema.json");

/// Answers one request with `status` and `body`, keeping what it was sent.
async fn serve(
    listener: TcpListener,
    status: &'static str,
    body: &'static str,
    seen: Arc<Mutex<String>>,
) {
    let (mut socket, _) = listener.accept().await.unwrap();
    let mut data = Vec::new();
    let mut buf = vec![0u8; 8192];
    // The introspection query ends the JSON body with `}`.
    while !String::from_utf8_lossy(&data).trim_end().ends_with('}') {
        let n = socket.read(&mut buf).await.unwrap();
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
    }
    *seen.lock().unwrap() = String::from_utf8_lossy(&data).into_owned();
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(response.as_bytes()).await.unwrap();
}

#[tokio::test]
async fn fetches_the_schema_with_the_requests_auth() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/graphql", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(String::new()));
    tokio::spawn(serve(listener, "200 OK", SCHEMA, seen.clone()));

    // A GET with a query of its own: the fetch still POSTs the introspection.
    let mut req = Request::new(Method::Get, &url);
    req.auth = Auth::Bearer {
        token: "s3cret".into(),
    };
    let fetched = fetch_schema(&HttpClient::new().unwrap(), &req)
        .await
        .unwrap();
    assert!(fetched.schema.get("User").is_some());
    assert_eq!(fetched.json, SCHEMA);

    let seen = seen.lock().unwrap();
    assert!(seen.starts_with("POST /graphql "), "{seen}");
    assert!(
        seen.to_ascii_lowercase()
            .contains("authorization: bearer s3cret")
    );
    assert!(seen.contains("__schema"));
}

#[tokio::test]
async fn says_why_there_is_no_schema() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/graphql", listener.local_addr().unwrap());
    tokio::spawn(serve(
        listener,
        "401 Unauthorized",
        r#"{"message":"missing token"}"#,
        Arc::default(),
    ));
    let err = fetch_schema(
        &HttpClient::new().unwrap(),
        &Request::new(Method::Post, &url),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, SchemaError::Schema(_)));
    assert_eq!(
        err.to_string(),
        r#"the server answered 401: {"message":"missing token"}"#
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/graphql", listener.local_addr().unwrap());
    tokio::spawn(serve(
        listener,
        "200 OK",
        r#"{"errors":[{"message":"introspection is disabled"}]}"#,
        Arc::default(),
    ));
    let err = fetch_schema(
        &HttpClient::new().unwrap(),
        &Request::new(Method::Post, &url),
    )
    .await
    .unwrap_err();
    assert!(
        err.to_string().contains("introspection is disabled"),
        "{err}"
    );
}
