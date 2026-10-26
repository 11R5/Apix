//! gRPC calls against a local Greeter server: reflection and .proto files,
//! all four kinds of method, metadata, status codes and mistakes.

#[path = "common/server.rs"]
mod server;

use std::net::SocketAddr;
use std::time::Duration;

use apix_core::{Auth, Body, GrpcCall, Method, Request};
use apix_grpc::{CallEvent, GrpcError};
use tokio::net::TcpListener;

async fn start(reflection: bool) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(server::serve(
        listener,
        reflection,
        Duration::from_millis(5),
    ));
    addr
}

fn request(addr: SocketAddr, method: &str, body: &str) -> Request {
    let mut req = Request::new(Method::Post, format!("grpc://{addr}"));
    req.grpc = Some(GrpcCall {
        method: method.into(),
        ..Default::default()
    });
    req.body = Body::Json(body.into());
    req
}

/// The same request, described by the .proto files instead of reflection.
fn with_protos(mut req: Request) -> Request {
    if let Some(grpc) = &mut req.grpc {
        grpc.protos = vec![server::fixture("greeter.proto")];
    }
    req
}

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap()
}

#[tokio::test]
async fn reflection_lists_and_describes_the_service() {
    let addr = start(true).await;
    let req = request(addr, "", "");
    let channel = apix_grpc::connect(&req).await.unwrap();
    let pool = apix_grpc::reflect(&channel, &Default::default())
        .await
        .unwrap();
    let methods = apix_grpc::methods(&pool);
    let names: Vec<_> = methods
        .iter()
        .map(|m| (m.name.as_str(), m.kind()))
        .collect();
    assert_eq!(
        names,
        [
            ("apix.test.Greeter/Chat", "bidirectional"),
            ("apix.test.Greeter/CountDown", "server streaming"),
            ("apix.test.Greeter/SayHello", "unary"),
            ("apix.test.Greeter/Sum", "client streaming"),
        ]
    );
    let text = apix_grpc::describe(&pool);
    assert!(text.starts_with("service apix.test.Greeter {\n"), "{text}");
    assert!(
        text.contains("  rpc CountDown(apix.test.CountRequest) returns (stream apix.test.Tick);\n")
    );
    assert!(text.contains(
        "  rpc Chat(stream apix.test.ChatMessage) returns (stream apix.test.ChatMessage);\n"
    ));
    assert!(text.contains("  map<string, int32> counts = 4;\n"));
    assert!(text.contains("  repeated string tags = 3;\n"));
    assert!(text.contains("  google.protobuf.Timestamp at = 2;\n"));
    assert!(text.contains("enum apix.test.Mood {\n  MOOD_UNSPECIFIED = 0;\n  HAPPY = 1;"));
    assert!(!text.contains("message google.protobuf.Timestamp"));
}

#[tokio::test]
async fn a_unary_call_carries_auth_and_answers() {
    let addr = start(true).await;
    let mut req = request(
        addr,
        "apix.test.Greeter/SayHello",
        r#"{"name": "Ada", "mood": "HAPPY"}"#,
    );
    req.auth = Auth::Bearer {
        token: "t0k".into(),
    };
    let mut events = Vec::new();
    let outcome = apix_grpc::send(&req, |e| events.push(e)).await.unwrap();
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(outcome.code_name(), "OK");
    assert_eq!(json(&outcome.received[0])["message"], "Hello, Ada!");
    assert!(
        outcome
            .headers
            .contains(&("x-seen-auth".into(), "Bearer t0k".into()))
    );
    assert!(matches!(events[0], CallEvent::Sent(_)));
    assert!(matches!(events[1], CallEvent::Headers(_)));
    assert!(matches!(events[2], CallEvent::Received(_)));
}

#[tokio::test]
async fn a_failed_call_reports_its_status() {
    let addr = start(true).await;
    let req = request(addr, "apix.test.Greeter/SayHello", "{}");
    let outcome = apix_grpc::send(&req, |_| {}).await.unwrap();
    assert_eq!(outcome.code, 3);
    assert_eq!(outcome.code_name(), "INVALID_ARGUMENT");
    assert_eq!(outcome.status_message, "name is required");
    assert!(outcome.received.is_empty());
}

#[tokio::test]
async fn server_streaming_from_proto_files() {
    // No reflection: the .proto files describe the service.
    let addr = start(false).await;
    let req = with_protos(request(
        addr,
        "apix.test.Greeter/CountDown",
        r#"{"from": 3}"#,
    ));
    let mut received = Vec::new();
    let outcome = apix_grpc::send(&req, |e| {
        if let CallEvent::Received(m) = e {
            received.push(json(&m)["n"].clone());
        }
    })
    .await
    .unwrap();
    assert!(outcome.is_ok());
    assert_eq!(received, [3, 2, 1]);
    assert_eq!(outcome.received.len(), 3);

    let req = with_protos(request(
        addr,
        "apix.test.Greeter/CountDown",
        r#"{"from": 500}"#,
    ));
    let outcome = apix_grpc::send(&req, |_| {}).await.unwrap();
    assert_eq!(outcome.code_name(), "OUT_OF_RANGE");
}

#[tokio::test]
async fn client_and_bidirectional_streams_send_a_list() {
    let addr = start(true).await;
    let sum = request(
        addr,
        "apix.test.Greeter/Sum",
        r#"[{"value": "1"}, {"value": 2}, {"value": 3}]"#,
    );
    let outcome = apix_grpc::send(&sum, |_| {}).await.unwrap();
    assert_eq!(outcome.sent, 3);
    let total = json(&outcome.received[0]);
    // 64-bit numbers are strings in protobuf's JSON.
    assert_eq!(total["sum"], "6");
    assert_eq!(total["count"], 3);

    let chat = request(
        addr,
        "apix.test.Greeter/Chat",
        r#"[{"text": "a"}, {"text": "b"}]"#,
    );
    let outcome = apix_grpc::send(&chat, |_| {}).await.unwrap();
    let texts: Vec<_> = outcome
        .received
        .iter()
        .map(|m| json(m)["text"].clone())
        .collect();
    assert_eq!(texts, ["echo: a", "echo: b"]);
}

#[tokio::test]
async fn mistakes_are_explained() {
    let addr = start(false).await;
    // No reflection and no .proto files.
    let req = request(addr, "apix.test.Greeter/SayHello", "{}");
    let err = apix_grpc::send(&req, |_| {}).await.unwrap_err();
    assert!(matches!(err, GrpcError::Reflection(_)));
    assert!(err.to_string().contains(".proto"), "{err}");

    let req = with_protos(request(addr, "apix.test.Greeter/Nope", "{}"));
    let err = apix_grpc::send(&req, |_| {}).await.unwrap_err();
    assert!(matches!(err, GrpcError::UnknownMethod(_)));

    let req = with_protos(request(
        addr,
        "apix.test.Greeter/SayHello",
        r#"{"nmae": "x"}"#,
    ));
    let err = apix_grpc::send(&req, |_| {}).await.unwrap_err();
    assert!(err.to_string().contains("apix.test.HelloRequest"), "{err}");

    let req = with_protos(request(addr, "apix.test.Greeter/SayHello", "[{}, {}]"));
    let err = apix_grpc::send(&req, |_| {}).await.unwrap_err();
    assert!(err.to_string().contains("takes one message"), "{err}");

    let mut req = with_protos(request(addr, "apix.test.Greeter/SayHello", "{}"));
    req.grpc.as_mut().unwrap().protos = vec!["missing.proto".into()];
    let err = apix_grpc::send(&req, |_| {}).await.unwrap_err();
    assert!(err.to_string().contains("missing.proto"), "{err}");

    // Nothing listens there.
    let closed = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap();
    let req = with_protos(request(closed, "apix.test.Greeter/SayHello", "{}"));
    let err = apix_grpc::send(&req, |_| {}).await.unwrap_err();
    assert!(matches!(err, GrpcError::Connect { .. }), "{err}");
}

#[test]
fn an_example_message_shows_every_field() {
    let pool = server::pool();
    let method = apix_grpc::find_method(&pool, "apix.test.Greeter/SayHello").unwrap();
    let example = json(&apix_grpc::example_message(&method.input()));
    assert_eq!(
        example,
        json(
            r#"{"name": "", "mood": "MOOD_UNSPECIFIED", "tags": [""], "counts": {"key": 0}, "inner": {"flag": false}}"#
        )
    );
    let reply = apix_grpc::find_method(&pool, "apix.test.Greeter.SayHello")
        .unwrap()
        .output();
    assert_eq!(
        json(&apix_grpc::example_message(&reply))["at"],
        "1970-01-01T00:00:00Z"
    );
}
