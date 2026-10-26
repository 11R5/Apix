//! gRPC calls for Apix. A request whose URL is `grpc://host:port` (or
//! `grpcs://` for TLS) names a method in `[grpc]`; the service is described
//! by `.proto` files or asked from the server (reflection). Messages go in
//! and come out as JSON, so nothing is generated at build time.

mod codec;
mod descriptors;

use std::time::{Duration, Instant};

use apix_core::{ApiKeyLocation, Auth, Body, Request};
use base64::Engine as _;
use prost_reflect::{
    DescriptorPool, DeserializeOptions, DynamicMessage, MethodDescriptor, SerializeOptions,
};
use tonic::Status;
use tonic::metadata::{MetadataKey, MetadataMap, MetadataValue};
use tonic::transport::{Channel, ClientTlsConfig, Endpoint, Identity};

pub use codec::DynamicCodec;
pub use descriptors::{
    MethodInfo, describe, example_message, find_method, from_protos, methods, reflect,
};
pub use prost_reflect;
pub use tonic;

#[derive(Debug, thiserror::Error)]
pub enum GrpcError {
    #[error("invalid gRPC address `{0}`: use grpc://host:port, or grpcs:// for TLS")]
    Url(String),
    #[error("cannot connect to {target}: {reason}")]
    Connect { target: String, reason: String },
    #[error("{0}")]
    Protos(String),
    #[error("{0}")]
    Reflection(String),
    #[error("no method `{0}`; pick one of the service's methods")]
    UnknownMethod(String),
    #[error("no method chosen: set `method` in [grpc] (package.Service/Method)")]
    NoMethod,
    #[error("{0}")]
    Message(String),
    #[error("{0}")]
    Auth(String),
    #[error("invalid metadata `{0}`")]
    Metadata(String),
    #[error("cannot read {path}: {reason}")]
    File { path: String, reason: String },
}

/// A `grpc://` or `grpcs://` URL.
pub fn is_grpc_url(url: &str) -> bool {
    let url = url.trim_start().to_ascii_lowercase();
    url.starts_with("grpc://") || url.starts_with("grpcs://")
}

/// `grpc://host:port/…` as the `http://host:port` tonic connects to, and
/// whether it is TLS. A path after the host is ignored: the method names it.
fn endpoint_uri(url: &str) -> Result<(String, bool), GrpcError> {
    let trimmed = url.trim();
    let (scheme, rest) = trimmed
        .split_once("://")
        .ok_or_else(|| GrpcError::Url(url.into()))?;
    let tls = match scheme.to_ascii_lowercase().as_str() {
        "grpc" | "http" => false,
        "grpcs" | "https" => true,
        _ => return Err(GrpcError::Url(url.into())),
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() {
        return Err(GrpcError::Url(url.into()));
    }
    let scheme = if tls { "https" } else { "http" };
    Ok((format!("{scheme}://{authority}"), tls))
}

fn read(path: &str) -> Result<Vec<u8>, GrpcError> {
    std::fs::read(path).map_err(|e| GrpcError::File {
        path: path.into(),
        reason: e.to_string(),
    })
}

/// Opens a connection to the request's server, with its timeout and client
/// certificate. TLS checks the server against the usual root certificates.
pub async fn connect(req: &Request) -> Result<Channel, GrpcError> {
    let (uri, tls) = endpoint_uri(&req.url)?;
    let target = uri.clone();
    let fail = |reason: String| GrpcError::Connect {
        target: target.clone(),
        reason,
    };
    let settings = req.settings.effective();
    let mut endpoint = Endpoint::from_shared(uri.clone())
        .map_err(|_| GrpcError::Url(req.url.clone()))?
        .connect_timeout(Duration::from_secs(10));
    if let Some(timeout) = settings.timeout {
        endpoint = endpoint.timeout(timeout).connect_timeout(timeout);
    }
    if tls {
        let mut config = ClientTlsConfig::new().with_webpki_roots();
        if let Some(cert) = &settings.client_cert {
            let cert_pem = read(cert)?;
            let key_pem = match &settings.client_key {
                Some(key) => read(key)?,
                None => cert_pem.clone(),
            };
            config = config.identity(Identity::from_pem(cert_pem, key_pem));
        }
        endpoint = endpoint
            .tls_config(config)
            .map_err(|e| fail(error_chain(&e)))?;
    }
    endpoint.connect().await.map_err(|e| fail(error_chain(&e)))
}

/// tonic's top-level message ("transport error") hides the cause.
fn error_chain(err: &dyn std::error::Error) -> String {
    let mut message = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        let text = cause.to_string();
        if !message.contains(&text) {
            message.push_str(": ");
            message.push_str(&text);
        }
        source = cause.source();
    }
    message
}

/// The request's headers and auth as gRPC metadata. OAuth 2.0 becomes a
/// bearer token first; auth types that sign an HTTP request do not apply.
pub async fn metadata(req: &Request) -> Result<MetadataMap, GrpcError> {
    let mut map = MetadataMap::new();
    let mut put = |key: &str, value: &str| -> Result<(), GrpcError> {
        let key = key.trim().to_ascii_lowercase();
        if key.ends_with("-bin") {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(value.trim())
                .unwrap_or_else(|_| value.as_bytes().to_vec());
            let key = MetadataKey::from_bytes(key.as_bytes())
                .map_err(|_| GrpcError::Metadata(key.clone()))?;
            map.append_bin(key, MetadataValue::from_bytes(&bytes));
        } else {
            let k = MetadataKey::from_bytes(key.as_bytes())
                .map_err(|_| GrpcError::Metadata(key.clone()))?;
            let v = MetadataValue::try_from(value).map_err(|_| GrpcError::Metadata(key.clone()))?;
            map.append(k, v);
        }
        Ok(())
    };
    for header in req
        .headers
        .iter()
        .filter(|h| h.enabled && !h.key.trim().is_empty())
    {
        put(&header.key, &header.value)?;
    }
    match &req.auth {
        Auth::Inherit | Auth::None => {}
        Auth::Bearer { token } => put("authorization", &format!("Bearer {token}"))?,
        Auth::Basic { username, password } => {
            let encoded =
                base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
            put("authorization", &format!("Basic {encoded}"))?
        }
        Auth::ApiKey {
            key,
            value,
            location: ApiKeyLocation::Header,
        } => put(key, value)?,
        Auth::ApiKey { .. } => {
            return Err(GrpcError::Auth(
                "an API key in the query does not apply to gRPC; send it as a header".into(),
            ));
        }
        Auth::OAuth2(config) => {
            let token = apix_http::oauth2::access_token(config)
                .await
                .map_err(|e| GrpcError::Auth(e.to_string()))?;
            put("authorization", &format!("Bearer {token}"))?
        }
        Auth::Digest { .. } | Auth::OAuth1(_) | Auth::AwsSigV4(_) => {
            return Err(GrpcError::Auth(
                "this auth type signs HTTP requests and does not apply to gRPC; use Bearer, Basic, API key or OAuth 2.0".into(),
            ));
        }
    }
    Ok(map)
}

/// The service description the request asks for: its `.proto` files, or
/// else the server's reflection.
pub async fn descriptors(
    req: &Request,
    channel: &Channel,
    metadata: &MetadataMap,
) -> Result<DescriptorPool, GrpcError> {
    match &req.grpc {
        Some(call) if !call.protos.is_empty() => from_protos(&call.protos, &call.import_paths),
        _ => reflect(channel, metadata).await,
    }
}

/// Something that happened during a call, as it happens.
#[derive(Debug, Clone, PartialEq)]
pub enum CallEvent {
    /// A message went out (JSON).
    Sent(String),
    /// The server's response metadata arrived.
    Headers(Vec<(String, String)>),
    /// A message came in (JSON).
    Received(String),
}

/// How a call ended.
#[derive(Debug, Clone, PartialEq)]
pub struct CallOutcome {
    /// The gRPC status code (`0` is OK).
    pub code: i32,
    pub status_message: String,
    pub headers: Vec<(String, String)>,
    pub trailers: Vec<(String, String)>,
    /// Every message that came in, as pretty JSON.
    pub received: Vec<String>,
    pub sent: usize,
    pub elapsed: Duration,
}

impl CallOutcome {
    pub fn is_ok(&self) -> bool {
        self.code == 0
    }

    /// `OK`, `NOT_FOUND`, …
    pub fn code_name(&self) -> &'static str {
        code_name(self.code)
    }
}

/// The canonical name of a gRPC status code.
pub fn code_name(code: i32) -> &'static str {
    match code {
        0 => "OK",
        1 => "CANCELLED",
        2 => "UNKNOWN",
        3 => "INVALID_ARGUMENT",
        4 => "DEADLINE_EXCEEDED",
        5 => "NOT_FOUND",
        6 => "ALREADY_EXISTS",
        7 => "PERMISSION_DENIED",
        8 => "RESOURCE_EXHAUSTED",
        9 => "FAILED_PRECONDITION",
        10 => "ABORTED",
        11 => "OUT_OF_RANGE",
        12 => "UNIMPLEMENTED",
        13 => "INTERNAL",
        14 => "UNAVAILABLE",
        15 => "DATA_LOSS",
        16 => "UNAUTHENTICATED",
        _ => "UNKNOWN",
    }
}

/// `NOT_FOUND: no such user`.
pub(crate) fn status_text(status: &Status) -> String {
    let name = code_name(status.code() as i32);
    if status.message().is_empty() {
        name.to_owned()
    } else {
        format!("{name}: {}", status.message())
    }
}

fn pairs(map: &MetadataMap) -> Vec<(String, String)> {
    map.clone()
        .into_headers()
        .iter()
        .filter(|(k, _)| !k.as_str().starts_with("grpc-") && k.as_str() != "content-type")
        .map(|(k, v)| {
            let value = if k.as_str().ends_with("-bin") {
                base64::engine::general_purpose::STANDARD.encode(v.as_bytes())
            } else {
                String::from_utf8_lossy(v.as_bytes()).into_owned()
            };
            (k.as_str().to_owned(), value)
        })
        .collect()
}

fn json_options() -> SerializeOptions {
    // Fields left at their default values too (not unset messages), named
    // as in the .proto, so the shape is visible.
    SerializeOptions::new()
        .use_proto_field_name(true)
        .skip_default_fields(false)
}

/// A message as pretty JSON.
pub fn to_json(message: &DynamicMessage) -> String {
    let mut out = Vec::new();
    let mut serializer =
        serde_json::Serializer::with_formatter(&mut out, serde_json::ser::PrettyFormatter::new());
    match message.serialize_with_options(&mut serializer, &json_options()) {
        Ok(()) => String::from_utf8(out).unwrap_or_default(),
        Err(e) => format!("\"cannot show the message: {e}\""),
    }
}

/// The request's body as the messages to send: one JSON object, or a JSON
/// array of them for a method that takes a stream. An empty body is `{}`.
pub fn messages(
    req: &Request,
    method: &MethodDescriptor,
) -> Result<Vec<DynamicMessage>, GrpcError> {
    let text = match &req.body {
        Body::None => String::new(),
        Body::Json(t) | Body::Text(t) => t.clone(),
        _ => {
            return Err(GrpcError::Message(
                "a gRPC request's body is its message as JSON".into(),
            ));
        }
    };
    let text = if text.trim().is_empty() {
        "{}".to_owned()
    } else {
        text
    };
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| GrpcError::Message(format!("the message is not valid JSON: {e}")))?;
    let values = match value {
        serde_json::Value::Array(items) => items,
        other => vec![other],
    };
    if values.len() != 1 && !method.is_client_streaming() {
        return Err(GrpcError::Message(format!(
            "{} takes one message, not a list",
            method.name()
        )));
    }
    let options = DeserializeOptions::new();
    values
        .iter()
        .enumerate()
        .map(|(ix, value)| {
            DynamicMessage::deserialize_with_options(method.input(), value, &options).map_err(|e| {
                let which = if values.len() > 1 {
                    format!("message {}", ix + 1)
                } else {
                    "the message".into()
                };
                GrpcError::Message(format!(
                    "{which} does not fit {}: {e}",
                    method.input().full_name()
                ))
            })
        })
        .collect()
}

/// A request carrying `body` and the call's metadata.
fn with<T>(body: T, metadata: &MetadataMap) -> tonic::Request<T> {
    let mut request = tonic::Request::new(body);
    *request.metadata_mut() = metadata.clone();
    request
}
/// Calls `method` with the request's messages and metadata, reporting what
/// happens as it happens. A status other than OK is an outcome, not an error.
pub async fn call(
    channel: Channel,
    method: &MethodDescriptor,
    messages: Vec<DynamicMessage>,
    metadata: MetadataMap,
    mut on_event: impl FnMut(CallEvent) + Send,
) -> Result<CallOutcome, GrpcError> {
    let started = Instant::now();
    let path = format!("/{}/{}", method.parent_service().full_name(), method.name());
    let path = tonic::codegen::http::uri::PathAndQuery::try_from(path.as_str())
        .map_err(|_| GrpcError::UnknownMethod(path.clone()))?;
    let codec = DynamicCodec::new(method.output());
    let sent = messages.len();
    for message in &messages {
        on_event(CallEvent::Sent(to_json(message)));
    }

    let mut grpc = tonic::client::Grpc::new(channel);
    let mut outcome = CallOutcome {
        code: 0,
        status_message: String::new(),
        headers: Vec::new(),
        trailers: Vec::new(),
        received: Vec::new(),
        sent,
        elapsed: Duration::ZERO,
    };
    let failed = |status: Status, mut outcome: CallOutcome| {
        outcome.code = status.code() as i32;
        outcome.status_message = status.message().to_owned();
        outcome.trailers = pairs(status.metadata());
        outcome.elapsed = started.elapsed();
        Ok(outcome)
    };
    if let Err(e) = grpc.ready().await {
        return Err(GrpcError::Connect {
            target: path.to_string(),
            reason: error_chain(&e),
        });
    }

    let response = match (method.is_client_streaming(), method.is_server_streaming()) {
        (false, false) | (true, false) => {
            let result = if method.is_client_streaming() {
                grpc.client_streaming(with(tokio_stream::iter(messages), &metadata), path, codec)
                    .await
            } else {
                let message = messages
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| DynamicMessage::new(method.input()));
                grpc.unary(with(message, &metadata), path, codec).await
            };
            match result {
                Ok(response) => response,
                Err(status) => return failed(status, outcome),
            }
            .map(Some)
            .into_parts()
        }
        (client, true) => {
            let result = if client {
                grpc.streaming(with(tokio_stream::iter(messages), &metadata), path, codec)
                    .await
            } else {
                let message = messages
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| DynamicMessage::new(method.input()));
                grpc.server_streaming(with(message, &metadata), path, codec)
                    .await
            };
            let response = match result {
                Ok(response) => response,
                Err(status) => return failed(status, outcome),
            };
            let (meta, mut stream, _) = response.into_parts();
            outcome.headers = pairs(&meta);
            on_event(CallEvent::Headers(outcome.headers.clone()));
            loop {
                match stream.message().await {
                    Ok(Some(message)) => {
                        let json = to_json(&message);
                        on_event(CallEvent::Received(json.clone()));
                        outcome.received.push(json);
                    }
                    Ok(None) => break,
                    Err(status) => return failed(status, outcome),
                }
            }
            if let Ok(Some(trailers)) = stream.trailers().await {
                outcome.trailers = pairs(&trailers);
            }
            outcome.elapsed = started.elapsed();
            return Ok(outcome);
        }
    };
    let (meta, message, _) = response;
    outcome.headers = pairs(&meta);
    on_event(CallEvent::Headers(outcome.headers.clone()));
    if let Some(message) = message {
        let json = to_json(&message);
        on_event(CallEvent::Received(json.clone()));
        outcome.received.push(json);
    }
    outcome.elapsed = started.elapsed();
    Ok(outcome)
}

/// Everything for one request, start to end: connect, describe, call.
pub async fn send(
    req: &Request,
    on_event: impl FnMut(CallEvent) + Send,
) -> Result<CallOutcome, GrpcError> {
    let name = req
        .grpc
        .as_ref()
        .map(|g| g.method.trim())
        .filter(|m| !m.is_empty())
        .ok_or(GrpcError::NoMethod)?
        .to_owned();
    let metadata = metadata(req).await?;
    let channel = connect(req).await?;
    let pool = descriptors(req, &channel, &metadata).await?;
    let method = find_method(&pool, &name).ok_or(GrpcError::UnknownMethod(name))?;
    let messages = messages(req, &method)?;
    call(channel, &method, messages, metadata, on_event).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_grpc_addresses() {
        assert_eq!(
            endpoint_uri("grpc://localhost:50051").unwrap(),
            ("http://localhost:50051".into(), false)
        );
        assert_eq!(
            endpoint_uri("grpcs://api.example.com/ignored/path").unwrap(),
            ("https://api.example.com".into(), true)
        );
        assert!(endpoint_uri("grpc://").is_err());
        assert!(endpoint_uri("ftp://x").is_err());
        assert!(is_grpc_url(" GRPCS://x:1"));
        assert!(!is_grpc_url("https://x"));
    }

    #[test]
    fn names_status_codes() {
        assert_eq!(code_name(0), "OK");
        assert_eq!(code_name(16), "UNAUTHENTICATED");
        assert_eq!(code_name(99), "UNKNOWN");
        assert_eq!(
            status_text(&Status::not_found("no such user")),
            "NOT_FOUND: no such user"
        );
    }

    #[tokio::test]
    async fn headers_and_auth_become_metadata() {
        let mut req = Request::new(apix_core::Method::Post, "grpc://x:1");
        req.headers
            .push(apix_core::KeyValue::new("X-Tenant", "acme"));
        req.headers
            .push(apix_core::KeyValue::new("trace-bin", "AQID"));
        req.auth = Auth::Basic {
            username: "ada".into(),
            password: "pw".into(),
        };
        let map = metadata(&req).await.unwrap();
        assert_eq!(map.get("x-tenant").unwrap(), "acme");
        assert_eq!(map.get("authorization").unwrap(), "Basic YWRhOnB3");
        assert_eq!(
            map.get_bin("trace-bin")
                .unwrap()
                .to_bytes()
                .unwrap()
                .as_ref(),
            [1, 2, 3]
        );
        req.auth = Auth::AwsSigV4(Default::default());
        assert!(matches!(metadata(&req).await, Err(GrpcError::Auth(_))));
    }
}
