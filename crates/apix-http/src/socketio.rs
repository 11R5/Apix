//! Socket.IO for servers of Socket.IO 3 and 4 (Engine.IO protocol 4), over
//! WebSocket only. The connection is a WebSocket one (the same handshake,
//! headers, auth and TLS) to `/socket.io/?EIO=4&transport=websocket`; on it
//! Apix joins one namespace, then emits and receives events until either
//! side closes. Binary attachments travel as `{"$binary": "<base64>"}` in
//! the arguments, both ways.

use std::time::Duration;

use apix_core::{KeyValue, MessageFormat, Request, SocketIoOptions};
use base64::Engine as _;
use futures_util::{SinkExt as _, StreamExt as _};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::HttpError;
use crate::ws::{self, WsCommand, WsConnection, WsData, WsEvent};

const BASE64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// An event, sent or received.
#[derive(Debug, Clone, PartialEq)]
pub struct SioMessage {
    pub event: String,
    pub args: Vec<Value>,
    /// The acknowledgement id: one we asked for, or one the server asks of us.
    pub ack: Option<u64>,
}

/// What happens on an open connection.
#[derive(Debug, Clone, PartialEq)]
pub enum SioEvent {
    Sent(SioMessage),
    Received(SioMessage),
    /// The server acknowledged our event `id` with these arguments.
    Acked {
        id: u64,
        args: Vec<Value>,
    },
    /// The server made us leave the namespace; the connection closes.
    Disconnected,
    /// A frame Apix could not read, as it came.
    Unreadable(String),
    /// The connection ended; the code and reason the closing side gave.
    Closed {
        code: Option<u16>,
        reason: String,
    },
    /// Something went wrong; the connection is over.
    Error(String),
}

/// What the user asks of an open connection.
#[derive(Debug, Clone)]
pub enum SioCommand {
    Emit {
        event: String,
        args: Vec<Value>,
        /// Ask the server to acknowledge it.
        ack: bool,
    },
    /// Leave the namespace and close.
    Close,
}

/// A connection that has joined its namespace.
pub struct SioConnection {
    /// `101` and the server's headers.
    pub status: u16,
    pub headers: Vec<(String, String)>,
    /// The WebSocket address the handshake went to.
    pub url: String,
    /// The namespace joined and the socket id the server gave.
    pub namespace: String,
    pub sid: String,
    /// How often the server pings.
    pub ping_interval: Option<Duration>,
    ws: WsConnection,
}

/// The arguments a message's text stands for: JSON as one argument, other
/// text as one string, base64 or hex as one binary attachment; nothing for
/// an empty text.
pub fn args_from_text(text: &str, format: MessageFormat) -> Result<Vec<Value>, String> {
    match format.decode(text) {
        Some(bytes) => Ok(vec![binary_value(&bytes?)]),
        None if text.trim().is_empty() => Ok(Vec::new()),
        None => Ok(vec![
            serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_owned())),
        ]),
    }
}

/// Bytes as an argument: `{"$binary": "<base64>"}`.
pub fn binary_value(bytes: &[u8]) -> Value {
    json!({ "$binary": BASE64.encode(bytes) })
}

fn binary_of(value: &Value) -> Option<Vec<u8>> {
    match value {
        Value::Object(map) if map.len() == 1 => BASE64.decode(map.get("$binary")?.as_str()?).ok(),
        _ => None,
    }
}

/// Arguments for the eye, in compact JSON: one as itself, several as a
/// list; binary attachments as `<3 bytes: 01 02 03>`.
pub fn show_args(args: &[Value]) -> String {
    fn shown(value: &Value) -> Value {
        if let Some(bytes) = binary_of(value) {
            return Value::String(format!("<{}>", ws::hex_preview(&bytes)));
        }
        match value {
            Value::Array(items) => Value::Array(items.iter().map(shown).collect()),
            Value::Object(map) => {
                Value::Object(map.iter().map(|(k, v)| (k.clone(), shown(v))).collect())
            }
            other => other.clone(),
        }
    }
    match args {
        [] => String::new(),
        [one] => shown(one).to_string(),
        many => Value::Array(many.iter().map(shown).collect()).to_string(),
    }
}

/// Connects a resolved Socket.IO request and joins its namespace. Gives up
/// after the request's timeout (20 s without one) when the server does not
/// answer as a Socket.IO server.
pub async fn connect(req: &Request) -> Result<SioConnection, HttpError> {
    let options = req.socketio.clone().unwrap_or_default();
    let (handshake, namespace) = handshake_request(req, &options)?;
    let auth = match options.auth.trim() {
        "" => None,
        text => match serde_json::from_str::<Value>(text) {
            Ok(value @ Value::Object(_)) => Some(value),
            Ok(_) => {
                return Err(HttpError::Settings(
                    "the Socket.IO auth must be a JSON object".into(),
                ));
            }
            Err(e) => {
                return Err(HttpError::Settings(format!(
                    "the Socket.IO auth is not JSON: {e}"
                )));
            }
        },
    };
    let mut ws = ws::connect(&handshake).await?;
    let limit = req
        .settings
        .effective()
        .timeout
        .unwrap_or(Duration::from_secs(20));
    let (sid, ping_interval) =
        tokio::time::timeout(limit, join(&mut ws, &namespace, auth.as_ref()))
            .await
            .map_err(|_| HttpError::TimedOut)??;
    Ok(SioConnection {
        status: ws.status,
        headers: ws.headers.clone(),
        url: apix_core::display_url(&handshake.url),
        namespace,
        sid,
        ping_interval,
        ws,
    })
}

/// The WebSocket request a Socket.IO one makes, and the namespace to join.
/// `http(s)://` becomes `ws(s)://`; the URL's path is the namespace unless
/// `[socketio]` names one; the server's path (`/socket.io/`) replaces it.
fn handshake_request(
    req: &Request,
    options: &SocketIoOptions,
) -> Result<(Request, String), HttpError> {
    let invalid = |reason: String| HttpError::InvalidUrl {
        url: apix_core::display_url(&req.url),
        reason,
    };
    let mut url = reqwest::Url::parse(req.url.trim()).map_err(|e| invalid(e.to_string()))?;
    let scheme = match url.scheme() {
        "http" | "ws" => "ws",
        "https" | "wss" => "wss",
        other => {
            return Err(invalid(format!(
                "Socket.IO needs http(s):// or ws(s)://, not {other}://"
            )));
        }
    };
    url.set_scheme(scheme)
        .map_err(|_| invalid("cannot switch to WebSocket".into()))?;
    let path = match options.path.trim().trim_matches('/') {
        "" => "/socket.io/".to_owned(),
        path => format!("/{path}/"),
    };
    let namespace = match options.namespace.trim().trim_end_matches('/') {
        "" => match url.path().trim_end_matches('/') {
            // An address copied from a browser points at the server's path.
            p if p.is_empty() || format!("{p}/") == path => "/".to_owned(),
            p => p.to_owned(),
        },
        n if n.starts_with('/') => n.to_owned(),
        n => format!("/{n}"),
    };
    url.set_path(&path);
    // The protocol's own parameters are set below, whatever the URL said.
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| k != "EIO" && k != "transport")
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    url.set_query(None);
    if !kept.is_empty() {
        url.query_pairs_mut().extend_pairs(kept);
    }
    let mut handshake = req.clone();
    handshake.url = url.to_string();
    handshake
        .query
        .retain(|kv| kv.key != "EIO" && kv.key != "transport");
    handshake.query.push(KeyValue::new("EIO", "4"));
    handshake
        .query
        .push(KeyValue::new("transport", "websocket"));
    Ok((handshake, namespace))
}

/// Waits for Engine.IO's open packet, asks to join the namespace and waits
/// for the answer: the socket id and the ping interval.
async fn join(
    ws: &mut WsConnection,
    namespace: &str,
    auth: Option<&Value>,
) -> Result<(String, Option<Duration>), HttpError> {
    let text = next_text(ws).await?;
    let Some(open) = text.strip_prefix('0') else {
        return Err(not_socketio(&text));
    };
    let open: Value = serde_json::from_str(open).unwrap_or(Value::Null);
    let ping_interval = open
        .get("pingInterval")
        .and_then(Value::as_u64)
        .map(Duration::from_millis);
    send_text(ws, encode(0, namespace, None, auth, 0)).await?;
    loop {
        let text = next_text(ws).await?;
        match text.as_bytes().first() {
            Some(b'2') => send_text(ws, "3".into()).await?,
            Some(b'1') => {
                return Err(HttpError::Settings(
                    "the server ended the Socket.IO session".into(),
                ));
            }
            Some(b'4') => {
                let packet = parse_packet(&text[1..]).map_err(HttpError::Settings)?;
                if packet.namespace != namespace {
                    continue;
                }
                match packet.kind {
                    0 => {
                        let sid = packet
                            .data
                            .as_ref()
                            .and_then(|d| d.get("sid"))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        return Ok((sid, ping_interval));
                    }
                    4 => {
                        return Err(HttpError::Settings(format!(
                            "the server refused to join {namespace}: {}",
                            refusal(packet.data.as_ref())
                        )));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

/// The next text frame; binary frames are skipped.
async fn next_text(ws: &mut WsConnection) -> Result<String, HttpError> {
    loop {
        match ws.stream.next().await {
            Some(Ok(Message::Text(text))) => return Ok(text.as_str().to_owned()),
            Some(Ok(Message::Close(frame))) => {
                let why = frame
                    .map(|f| {
                        format!(
                            " ({}{})",
                            u16::from(f.code),
                            match f.reason.as_str() {
                                "" => String::new(),
                                reason => format!(": {reason}"),
                            }
                        )
                    })
                    .unwrap_or_default();
                return Err(HttpError::Settings(format!(
                    "the server closed the connection before the Socket.IO session started{why}"
                )));
            }
            Some(Ok(_)) => {}
            Some(Err(e)) => return Err(HttpError::Settings(e.to_string())),
            None => {
                return Err(HttpError::Settings(
                    "the server closed the connection before the Socket.IO session started".into(),
                ));
            }
        }
    }
}

async fn send_text(ws: &mut WsConnection, text: String) -> Result<(), HttpError> {
    ws.stream
        .send(Message::text(text))
        .await
        .map_err(|e| HttpError::Settings(e.to_string()))
}

fn not_socketio(text: &str) -> HttpError {
    let mut start: String = text.chars().take(80).collect();
    if text.chars().count() > 80 {
        start.push('…');
    }
    HttpError::Settings(format!(
        "not a Socket.IO server (Engine.IO 4): it sent `{start}` first"
    ))
}

/// Why the server refused: `{"message": …}` (Socket.IO 3/4) or a plain text.
fn refusal(data: Option<&Value>) -> String {
    match data {
        Some(Value::String(message)) => message.clone(),
        Some(value) => value
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| value.to_string()),
        None => "no reason given".into(),
    }
}

/// A Socket.IO packet, the part of an Engine.IO message after its `4`.
#[derive(Debug, Clone, PartialEq)]
struct Packet {
    /// 0 connect, 1 disconnect, 2 event, 3 ack, 4 connect error, 5 binary
    /// event, 6 binary ack.
    kind: u8,
    attachments: usize,
    namespace: String,
    id: Option<u64>,
    data: Option<Value>,
}

fn parse_packet(text: &str) -> Result<Packet, String> {
    let unreadable = |why: &str| format!("unreadable Socket.IO packet ({why}): {text}");
    let kind = text
        .chars()
        .next()
        .and_then(|c| c.to_digit(10))
        .filter(|k| *k <= 6)
        .ok_or_else(|| unreadable("unknown type"))? as u8;
    let mut rest = &text[1..];
    let mut attachments = 0;
    if kind == 5 || kind == 6 {
        let dash = rest
            .find('-')
            .ok_or_else(|| unreadable("no attachment count"))?;
        attachments = rest[..dash]
            .parse()
            .map_err(|_| unreadable("bad attachment count"))?;
        rest = &rest[dash + 1..];
    }
    let mut namespace = "/".to_owned();
    if rest.starts_with('/') {
        let end = rest.find(',').unwrap_or(rest.len());
        namespace = rest[..end].to_owned();
        rest = rest.get(end + 1..).unwrap_or_default();
    }
    let digits = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let id = match digits {
        0 => None,
        n => Some(rest[..n].parse().map_err(|_| unreadable("bad id"))?),
    };
    rest = &rest[digits..];
    let data = match rest {
        "" => None,
        json => Some(serde_json::from_str(json).map_err(|_| unreadable("not JSON"))?),
    };
    Ok(Packet {
        kind,
        attachments,
        namespace,
        id,
        data,
    })
}

/// An Engine.IO message (`4`) carrying a Socket.IO packet.
fn encode(
    kind: u8,
    namespace: &str,
    id: Option<u64>,
    data: Option<&Value>,
    attachments: usize,
) -> String {
    let mut out = format!("4{kind}");
    if attachments > 0 {
        out.push_str(&format!("{attachments}-"));
    }
    if namespace != "/" {
        out.push_str(namespace);
        out.push(',');
    }
    if let Some(id) = id {
        out.push_str(&id.to_string());
    }
    if let Some(data) = data {
        out.push_str(&data.to_string());
    }
    out
}

/// Takes binary values out of `value` into `attachments`, leaving
/// Socket.IO's placeholders in their place.
fn take_binary(value: &mut Value, attachments: &mut Vec<Vec<u8>>) {
    if let Some(bytes) = binary_of(value) {
        *value = json!({ "_placeholder": true, "num": attachments.len() });
        attachments.push(bytes);
        return;
    }
    match value {
        Value::Array(items) => items.iter_mut().for_each(|v| take_binary(v, attachments)),
        Value::Object(map) => map.values_mut().for_each(|v| take_binary(v, attachments)),
        _ => {}
    }
}

/// Puts received attachments where their placeholders are.
fn put_binary(value: &mut Value, attachments: &[Vec<u8>]) {
    if value.get("_placeholder").and_then(Value::as_bool) == Some(true)
        && let Some(bytes) = value
            .get("num")
            .and_then(Value::as_u64)
            .and_then(|n| attachments.get(n as usize))
    {
        *value = binary_value(bytes);
        return;
    }
    match value {
        Value::Array(items) => items.iter_mut().for_each(|v| put_binary(v, attachments)),
        Value::Object(map) => map.values_mut().for_each(|v| put_binary(v, attachments)),
        _ => {}
    }
}

/// An event's name and arguments from its packet data, `["name", …args]`.
fn event_of(data: Option<Value>) -> Option<(String, Vec<Value>)> {
    let Some(Value::Array(mut items)) = data else {
        return None;
    };
    if items.is_empty() {
        return None;
    }
    match items.remove(0) {
        Value::String(event) => Some((event, items)),
        _ => None,
    }
}

fn args_of(data: Option<Value>) -> Vec<Value> {
    match data {
        Some(Value::Array(items)) => items,
        Some(other) => vec![other],
        None => Vec::new(),
    }
}

impl SioConnection {
    /// Emits what `commands` asks and reports everything to `on_event`
    /// until either side closes. The server's pings are answered.
    pub async fn run(
        self,
        mut commands: mpsc::UnboundedReceiver<SioCommand>,
        mut on_event: impl FnMut(SioEvent) + Send,
    ) {
        let (to_socket, from_us) = mpsc::unbounded_channel();
        let (to_us, mut events) = mpsc::unbounded_channel();
        let socket = self.ws.run(from_us, move |event| {
            let _ = to_us.send(event);
        });
        let namespace = self.namespace;
        let talk = async move {
            let send = |text: String| {
                let _ = to_socket.send(WsCommand::Send(WsData::Text(text)));
            };
            let mut next_id = 0u64;
            // A binary packet waiting for its attachments.
            let mut waiting: Option<(Packet, Vec<Vec<u8>>)> = None;
            let mut asking = true;
            loop {
                tokio::select! {
                    command = commands.recv(), if asking => match command {
                        Some(SioCommand::Emit { event, args, ack }) => {
                            let mut data = Value::Array(
                                std::iter::once(Value::String(event.clone()))
                                    .chain(args.iter().cloned())
                                    .collect(),
                            );
                            let mut attachments = Vec::new();
                            take_binary(&mut data, &mut attachments);
                            let id = ack.then(|| {
                                next_id += 1;
                                next_id - 1
                            });
                            let kind = if attachments.is_empty() { 2 } else { 5 };
                            send(encode(kind, &namespace, id, Some(&data), attachments.len()));
                            for bytes in attachments {
                                let _ = to_socket.send(WsCommand::Send(WsData::Binary(bytes)));
                            }
                            on_event(SioEvent::Sent(SioMessage { event, args, ack: id }));
                        }
                        // Asked to close, or nobody can ask anything any more.
                        Some(SioCommand::Close) | None => {
                            asking = false;
                            send(encode(1, &namespace, None, None, 0));
                            let _ = to_socket.send(WsCommand::Close);
                        }
                    },
                    event = events.recv() => match event {
                        None => return,
                        Some(WsEvent::Sent(_)) => {}
                        Some(WsEvent::Received(WsData::Text(text))) => match text.as_bytes().first() {
                            Some(b'2') => send("3".into()),
                            Some(b'3' | b'6') => {}
                            Some(b'1') => {
                                let _ = to_socket.send(WsCommand::Close);
                            }
                            Some(b'4') => match parse_packet(&text[1..]) {
                                Ok(packet) if packet.attachments > 0 => {
                                    waiting = Some((packet, Vec::new()));
                                }
                                Ok(packet) => deliver(packet, &[], &mut on_event, &text, &to_socket),
                                Err(_) => on_event(SioEvent::Unreadable(text)),
                            },
                            _ => on_event(SioEvent::Unreadable(text)),
                        },
                        Some(WsEvent::Received(WsData::Binary(bytes))) => match &mut waiting {
                            Some((packet, got)) => {
                                got.push(bytes);
                                if got.len() >= packet.attachments
                                    && let Some((packet, got)) = waiting.take()
                                {
                                    deliver(packet, &got, &mut on_event, "", &to_socket);
                                }
                            }
                            None => on_event(SioEvent::Unreadable(ws::hex_preview(&bytes))),
                        },
                        Some(WsEvent::Closed { code, reason }) => {
                            on_event(SioEvent::Closed { code, reason });
                            return;
                        }
                        Some(WsEvent::Error(message)) => {
                            on_event(SioEvent::Error(message));
                            return;
                        }
                    },
                }
            }
        };
        tokio::join!(socket, talk);
    }
}

/// Reports a whole packet (its attachments in place).
fn deliver(
    mut packet: Packet,
    attachments: &[Vec<u8>],
    on_event: &mut impl FnMut(SioEvent),
    raw: &str,
    to_socket: &mpsc::UnboundedSender<WsCommand>,
) {
    if let Some(data) = &mut packet.data {
        put_binary(data, attachments);
    }
    let unreadable = || SioEvent::Unreadable(raw.to_owned());
    match packet.kind {
        2 | 5 => match event_of(packet.data) {
            Some((event, args)) => on_event(SioEvent::Received(SioMessage {
                event,
                args,
                ack: packet.id,
            })),
            None => on_event(unreadable()),
        },
        3 | 6 => match packet.id {
            Some(id) => on_event(SioEvent::Acked {
                id,
                args: args_of(packet.data),
            }),
            None => on_event(unreadable()),
        },
        1 => {
            on_event(SioEvent::Disconnected);
            let _ = to_socket.send(WsCommand::Close);
        }
        4 => on_event(SioEvent::Error(refusal(packet.data.as_ref()))),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(url: &str) -> Request {
        Request {
            socketio: Some(SocketIoOptions::default()),
            ..Request::new(apix_core::Method::Get, url)
        }
    }

    #[test]
    fn handshake_addresses() {
        let (ws, namespace) =
            handshake_request(&request("http://localhost:3000"), &Default::default()).unwrap();
        assert_eq!(ws.url, "ws://localhost:3000/socket.io/");
        assert_eq!(namespace, "/");
        let query: Vec<_> = ws.query.iter().map(|kv| (&*kv.key, &*kv.value)).collect();
        assert_eq!(query, [("EIO", "4"), ("transport", "websocket")]);

        // The URL's path is the namespace; the server's path can move.
        let options = SocketIoOptions {
            path: "realtime".into(),
            ..Default::default()
        };
        let (ws, namespace) =
            handshake_request(&request("https://api.example.com/admin?v=2"), &options).unwrap();
        assert_eq!(ws.url, "wss://api.example.com/realtime/?v=2");
        assert_eq!(namespace, "/admin");

        // A namespace set in the options wins; an address copied from the
        // browser's network tab works as is.
        let options = SocketIoOptions {
            namespace: "chat".into(),
            ..Default::default()
        };
        let copied = "ws://h:1/socket.io/?EIO=4&transport=websocket&token=x";
        let (ws, namespace) = handshake_request(&request(copied), &options).unwrap();
        assert_eq!(ws.url, "ws://h:1/socket.io/?token=x");
        assert_eq!(namespace, "/chat");
        let (_, namespace) = handshake_request(&request(copied), &Default::default()).unwrap();
        assert_eq!(namespace, "/");

        assert!(handshake_request(&request("ftp://h"), &Default::default()).is_err());
    }

    #[test]
    fn packets_read_and_written() {
        assert_eq!(
            parse_packet(r#"2["hello",1,{"a":true}]"#).unwrap(),
            Packet {
                kind: 2,
                attachments: 0,
                namespace: "/".into(),
                id: None,
                data: Some(json!(["hello", 1, {"a": true}])),
            }
        );
        let ack = parse_packet(r#"3/admin,13["ok"]"#).unwrap();
        assert_eq!((ack.kind, &*ack.namespace, ack.id), (3, "/admin", Some(13)));
        let binary = parse_packet(r#"52-/x,7["up",{"_placeholder":true,"num":0}]"#).unwrap();
        assert_eq!(
            (binary.kind, binary.attachments, binary.id),
            (5, 2, Some(7))
        );
        let leave = parse_packet("1/admin").unwrap();
        assert_eq!(
            (leave.kind, &*leave.namespace, leave.data),
            (1, "/admin", None)
        );
        assert!(parse_packet("9").is_err());
        assert!(parse_packet("2[oops").is_err());

        assert_eq!(encode(0, "/", None, None, 0), "40");
        assert_eq!(
            encode(0, "/admin", None, Some(&json!({"token": "t"})), 0),
            r#"40/admin,{"token":"t"}"#
        );
        assert_eq!(
            encode(2, "/", Some(4), Some(&json!(["hi"])), 0),
            r#"424["hi"]"#
        );
        assert_eq!(encode(5, "/x", None, Some(&json!([])), 1), "451-/x,[]");
    }

    #[test]
    fn binary_attachments_both_ways() {
        let mut data = json!(["file", {"name": "a", "data": binary_value(&[1, 2, 3])}]);
        let mut attachments = Vec::new();
        take_binary(&mut data, &mut attachments);
        assert_eq!(attachments, [vec![1, 2, 3]]);
        assert_eq!(data[1]["data"], json!({"_placeholder": true, "num": 0}));
        put_binary(&mut data, &attachments);
        assert_eq!(data[1]["data"], json!({"$binary": "AQID"}));
        assert_eq!(
            show_args(&[data[1].clone()]),
            r#"{"name":"a","data":"<3 bytes: 01 02 03>"}"#
        );
    }

    #[test]
    fn message_texts_become_arguments() {
        let text = MessageFormat::Text;
        assert_eq!(args_from_text("", text).unwrap(), Vec::<Value>::new());
        assert_eq!(
            args_from_text(r#"{"a":1}"#, text).unwrap(),
            [json!({"a": 1})]
        );
        assert_eq!(args_from_text("hello", text).unwrap(), [json!("hello")]);
        assert_eq!(
            args_from_text("01 ff", MessageFormat::Hex).unwrap(),
            [json!({"$binary": "Af8="})]
        );
        assert!(args_from_text("zz", MessageFormat::Hex).is_err());
        assert_eq!(show_args(&[json!("hi"), json!(2)]), r#"["hi",2]"#);
        assert_eq!(refusal(Some(&json!({"message": "nope"}))), "nope");
    }
}
