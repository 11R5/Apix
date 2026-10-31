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

/// The same request, with a definition kept in it.
fn with_definition(mut req: Request, definition: &str, guessed: bool) -> Request {
    if let Some(grpc) = &mut req.grpc {
        grpc.definition = definition.into();
        grpc.guessed = guessed;
    }
    req
}

/// The Greeter's service as one `.proto` text, as generated code would give it.
fn greeter_definition() -> String {
    let pool = server::pool();
    let set = prost_types::FileDescriptorSet {
        file: pool.file_descriptor_protos().cloned().collect(),
    };
    apix_grpc::convert_bytes(&prost::Message::encode_to_vec(&set), None, None)
        .unwrap()
        .definition
}

#[tokio::test]
async fn a_definition_needs_neither_reflection_nor_files() {
    let addr = start(false).await;
    let req = request(addr, "apix.test.Greeter/SayHello", r#"{"name": "Ada"}"#);
    let no_way = apix_grpc::send(&req, |_| {}).await.unwrap_err().to_string();
    assert!(no_way.contains("does not offer reflection"), "{no_way}");

    let req = with_definition(req, &greeter_definition(), false);
    let mut notes = Vec::new();
    let outcome = apix_grpc::send(&req, |event| {
        if let CallEvent::Note(note) = event {
            notes.push(note);
        }
    })
    .await
    .unwrap();
    assert!(outcome.is_ok(), "{outcome:?}");
    assert!(outcome.received[0].contains("Ada"), "{outcome:?}");
    assert!(notes.is_empty(), "{notes:?}");
}

#[tokio::test]
async fn runs_call_through_a_definition_too() {
    let addr = start(false).await;
    let req = with_definition(
        request(addr, "apix.test.Greeter/SayHello", r#"{"name": "Ada"}"#),
        &greeter_definition(),
        false,
    );
    let response = apix_grpc::send_for_tests(&req).await.unwrap();
    assert_eq!(response.status, 200);
    assert!(response.raw_body().contains("Ada"));
}

#[tokio::test]
async fn a_descriptor_set_file_describes_the_service() {
    let addr = start(false).await;
    let pool = server::pool();
    let set = prost_types::FileDescriptorSet {
        file: pool.file_descriptor_protos().cloned().collect(),
    };
    let dir = std::env::temp_dir().join(format!("apix-grpc-set-{}", addr.port()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("greeter.pb");
    std::fs::write(&path, prost::Message::encode_to_vec(&set)).unwrap();

    let mut req = request(addr, "apix.test.Greeter/SayHello", r#"{"name": "Ada"}"#);
    if let Some(grpc) = &mut req.grpc {
        grpc.protos = vec![path.to_string_lossy().into_owned()];
    }
    let outcome = apix_grpc::send(&req, |_| {}).await.unwrap();
    assert!(outcome.is_ok(), "{outcome:?}");

    std::fs::write(&path, b"not a set").unwrap();
    let error = apix_grpc::send(&req, |_| {}).await.unwrap_err().to_string();
    assert!(error.contains("greeter.pb"), "{error}");
    std::fs::remove_dir_all(&dir).ok();
}

/// A guess at `HelloRequest`: its name in a field number the server does not use.
const WRONG_GUESS: &str = r#"syntax = "proto3";
package apix.test;
service Greeter { rpc SayHello(Guess) returns (Answer); }
message Guess { string name = 9; }
message Answer { }
"#;

#[tokio::test]
async fn a_guess_gives_way_to_reflection_and_the_differences_are_told() {
    let addr = start(true).await;
    let req = with_definition(
        request(addr, "apix.test.Greeter/SayHello", r#"{"name": "Ada"}"#),
        WRONG_GUESS,
        true,
    );
    let mut notes = Vec::new();
    let outcome = apix_grpc::send(&req, |event| {
        if let CallEvent::Note(note) = event {
            notes.push(note);
        }
    })
    .await
    .unwrap();
    assert!(outcome.is_ok(), "{outcome:?}");
    assert!(outcome.received[0].contains("Ada"), "{outcome:?}");
    let told = notes.join("\n");
    assert!(told.contains("reflection was used"), "{told}");
    assert!(told.contains("number 9 here"), "{told}");
    assert_eq!(outcome.notes, notes);

    // The same call, not marked as a guess, is taken at its word.
    let req = with_definition(
        request(addr, "apix.test.Greeter/SayHello", r#"{"name": "Ada"}"#),
        WRONG_GUESS,
        false,
    );
    let described = apix_grpc::load_description(&req).await.unwrap();
    assert!(described.notes.is_empty());
    assert!(apix_grpc::find_method(&described.pool, "apix.test.Greeter/SayHello").is_some());
}

#[tokio::test]
async fn a_wrong_guess_without_reflection_is_called_out() {
    let addr = start(false).await;
    let req = with_definition(
        request(addr, "apix.test.Greeter/SayHello", r#"{"name": "Ada"}"#),
        WRONG_GUESS,
        true,
    );
    let outcome = apix_grpc::send(&req, |_| {}).await.unwrap();
    let told = outcome.notes.join("\n");
    assert!(told.contains("no reflection to check against"), "{told}");
    // The server answered, or said the name was missing; either way nothing
    // of what it may have sent back was understood, and the user is told.
    if outcome.is_ok() {
        assert!(told.contains("does not know"), "{told}");
    }
}

#[tokio::test]
async fn an_answer_with_unknown_fields_is_called_out_once() {
    let addr = start(false).await;
    let definition = r#"syntax = "proto3";
package apix.test;
service Greeter { rpc CountDown(In) returns (stream Out); }
message In { int32 from = 1; }
message Out { }
"#;
    let req = with_definition(
        request(addr, "apix.test.Greeter/CountDown", r#"{"from": 3}"#),
        definition,
        false,
    );
    let outcome = apix_grpc::send(&req, |_| {}).await.unwrap();
    assert!(outcome.is_ok(), "{outcome:?}");
    let about_fields: Vec<_> = outcome
        .notes
        .iter()
        .filter(|n| n.contains("does not know"))
        .collect();
    assert!(!about_fields.is_empty(), "{:?}", outcome.notes);
    assert!(about_fields.iter().all(|n| n.contains("apix.test.Out")));
}

#[tokio::test]
async fn an_answer_of_another_type_says_the_definition_is_at_fault() {
    let addr = start(false).await;
    let definition = r#"syntax = "proto3";
package apix.test;
service Greeter { rpc SayHello(In) returns (Out); }
message In { string name = 1; }
message Out { int32 message = 1; }
"#;
    let req = with_definition(
        request(addr, "apix.test.Greeter/SayHello", r#"{"name": "Ada"}"#),
        definition,
        false,
    );
    let outcome = apix_grpc::send(&req, |_| {}).await.unwrap();
    assert!(!outcome.is_ok(), "{outcome:?}");
    assert!(
        outcome.notes.iter().any(|n| n.contains("does not fit")),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn a_description_without_a_server_loads_all_the_same() {
    // Nothing listens on this port.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let req = with_definition(request(addr, "", ""), WRONG_GUESS, false);
    let described = apix_grpc::load_description(&req).await.unwrap();
    assert_eq!(apix_grpc::methods(&described.pool).len(), 1);

    let req = with_definition(request(addr, "", ""), WRONG_GUESS, true);
    let described = apix_grpc::load_description(&req).await.unwrap();
    assert_eq!(apix_grpc::methods(&described.pool).len(), 1);
    assert!(
        described.notes[0].contains("could not be checked"),
        "{:?}",
        described.notes
    );
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

#[tokio::test]
async fn runs_see_a_call_as_an_http_response() {
    let addr = start(true).await;
    let req = request(addr, "apix.test.Greeter/SayHello", r#"{"name": "Ada"}"#);
    let response = apix_grpc::send_for_tests(&req).await.unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.reason, Some("OK"));
    assert_eq!(response.header("grpc-status"), Some("0"));
    assert_eq!(json(&response.raw_body())["message"], "Hello, Ada!");

    let req = request(addr, "apix.test.Greeter/CountDown", r#"{"from": 2}"#);
    let response = apix_grpc::send_for_tests(&req).await.unwrap();
    assert_eq!(json(&response.raw_body()), json(r#"[{"n": 2}, {"n": 1}]"#));

    let req = request(addr, "apix.test.Greeter/SayHello", "{}");
    let response = apix_grpc::send_for_tests(&req).await.unwrap();
    assert_eq!(response.status, 400);
    assert_eq!(response.header("grpc-message"), Some("name is required"));
}

/// A certificate authority, and a certificate it signed for 127.0.0.1 (PEM):
/// (CA, server certificate, server key).
fn certificates() -> (String, String, String) {
    use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair};
    let mut ca = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca.distinguished_name
        .push(DnType::CommonName, "Apix test CA");
    let ca_key = KeyPair::generate().unwrap();
    let ca_pem = ca.self_signed(&ca_key).unwrap().pem();
    let issuer = Issuer::new(ca, ca_key);
    let server = CertificateParams::new(vec!["127.0.0.1".to_owned()]).unwrap();
    let server_key = KeyPair::generate().unwrap();
    let server_pem = server.signed_by(&server_key, &issuer).unwrap().pem();
    (ca_pem, server_pem, server_key.serialize_pem())
}

#[tokio::test]
async fn tls_with_an_own_authority_or_without_verifying() {
    let (ca, cert, key) = certificates();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(server::serve_tls(listener, cert, key));
    let dir = std::env::temp_dir().join(format!("apix-grpc-tls-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ca_file = dir.join("ca.pem");
    std::fs::write(&ca_file, &ca).unwrap();

    let mut req = request(addr, "apix.test.Greeter/SayHello", r#"{"name": "Ada"}"#);
    req.url = format!("grpcs://{addr}");
    // The authority is unknown, so the certificate is refused.
    let err = apix_grpc::send(&req, |_| {}).await.unwrap_err();
    assert!(matches!(err, GrpcError::Connect { .. }), "{err}");

    // Trusting the authority.
    req.settings.ca_cert = Some(ca_file.to_string_lossy().into_owned());
    let outcome = apix_grpc::send(&req, |_| {}).await.unwrap();
    assert_eq!(json(&outcome.received[0])["message"], "Hello, Ada!");

    // Not verifying at all, with reflection over the same connection kind.
    req.settings.ca_cert = None;
    req.settings.tls_verify = Some(false);
    let outcome = apix_grpc::send(&req, |_| {}).await.unwrap();
    assert!(outcome.is_ok(), "{outcome:?}");

    // A file that is not a certificate says so.
    std::fs::write(&ca_file, "not a certificate").unwrap();
    req.settings.tls_verify = None;
    req.settings.ca_cert = Some(ca_file.to_string_lossy().into_owned());
    let err = apix_grpc::send(&req, |_| {}).await.unwrap_err();
    assert!(err.to_string().contains("no PEM certificate"), "{err}");
    std::fs::remove_dir_all(&dir).ok();
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
