//! MQTT 3.1.1 over TCP or TLS: connect to a broker, subscribe to topics,
//! publish messages, and report everything that happens until either side
//! closes. A `mqtt://` or `mqtts://` request is one connection; the user name
//! and password come from its Basic auth or its URL.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use apix_core::{Auth, MessageFormat, MqttSettings, Request};
use percent_encoding::percent_decode_str;
use rumqttc::{
    AsyncClient, ConnectReturnCode, ConnectionError, Event, EventLoop, MqttOptions, Outgoing,
    Packet, QoS, StateError, SubscribeReasonCode, TlsConfiguration, Transport,
};
use tokio::sync::mpsc;

/// The biggest packet either way: what MQTT itself allows.
const MAX_PACKET: usize = 256 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum MqttError {
    #[error("invalid URL `{url}`: {reason}")]
    InvalidUrl { url: String, reason: String },
    /// Options that cannot be used, in words.
    #[error("{0}")]
    Settings(String),
    /// TLS settings that cannot be used.
    #[error(transparent)]
    Tls(#[from] apix_http::HttpError),
    #[error("the broker refused the connection: {0}")]
    Refused(&'static str),
    #[error("the connection timed out")]
    TimedOut,
    #[error("cannot reach the broker: {0}")]
    Connection(String),
}

/// A message, published or received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MqttMessage {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: u8,
    pub retain: bool,
}

/// What happens on an open connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MqttEvent {
    Sent(MqttMessage),
    Received(MqttMessage),
    /// The broker confirmed a message of quality of service 1 or 2.
    Delivered {
        topic: String,
    },
    /// The broker accepted a subscription, at this quality of service.
    Subscribed {
        topic: String,
        qos: u8,
    },
    /// The broker refused a subscription.
    SubscribeRefused {
        topic: String,
    },
    Unsubscribed {
        topic: String,
    },
    /// A command that could not be carried out; the connection goes on.
    Rejected(String),
    /// The connection ended, by us or by the broker.
    Closed {
        by_broker: bool,
    },
    /// Something went wrong; the connection is over.
    Error(String),
}

/// What the user asks of an open connection.
#[derive(Debug, Clone)]
pub enum MqttCommand {
    Publish(MqttMessage),
    Subscribe { topic: String, qos: u8 },
    Unsubscribe { topic: String },
    Close,
}

/// A connection the broker has accepted.
pub struct MqttConnection {
    pub client_id: String,
    /// The broker still had a session for this client id.
    pub session_present: bool,
    subscribe: Vec<(String, u8)>,
    client: AsyncClient,
    events: EventLoop,
}

/// A message's bytes from its text: as is, or written in base64 or hex.
pub fn payload_from_text(text: &str, format: MessageFormat) -> Result<Vec<u8>, String> {
    match format.decode(text) {
        None => Ok(text.as_bytes().to_vec()),
        Some(bytes) => bytes,
    }
}

/// The URL for logs and messages: without a password it may hold.
pub fn display_url(url: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(url.trim()) else {
        return url.to_owned();
    };
    if parsed.password().is_some() {
        let _ = parsed.set_password(None);
    }
    parsed.to_string()
}

/// A payload for the eye: its text, or its size and first bytes.
pub fn show_payload(payload: &[u8]) -> String {
    match std::str::from_utf8(payload) {
        Ok(text) => text.to_owned(),
        Err(_) => apix_http::ws::hex_preview(payload),
    }
}

fn quality(qos: u8) -> Option<QoS> {
    rumqttc::qos(qos).ok()
}

fn level(qos: QoS) -> u8 {
    match qos {
        QoS::AtMostOnce => 0,
        QoS::AtLeastOnce => 1,
        QoS::ExactlyOnce => 2,
    }
}

/// A client id for one connection, when the request names none.
fn new_client_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
        .unwrap_or_default();
    format!("apix-{nanos:x}")
}

/// The broker's options for a resolved request.
fn options(req: &Request) -> Result<(MqttOptions, Vec<(String, u8)>), MqttError> {
    let invalid = |reason: String| MqttError::InvalidUrl {
        url: req.url.clone(),
        reason,
    };
    let url = url::Url::parse(req.url.trim()).map_err(|e| invalid(e.to_string()))?;
    let default_port = match url.scheme() {
        "mqtt" => 1883,
        "mqtts" => 8883,
        other => {
            return Err(invalid(format!(
                "MQTT needs mqtt:// or mqtts://, not {other}://"
            )));
        }
    };
    let host = url
        .host_str()
        .filter(|h| !h.is_empty())
        .ok_or_else(|| invalid("it names no broker".into()))?;
    // The URL holds an IPv6 address in brackets; a connection wants it without.
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let settings: MqttSettings = req.mqtt.clone().unwrap_or_default();
    let client_id = match settings.client_id.trim() {
        "" if settings.keep_session => {
            return Err(MqttError::Settings(
                "keeping the session needs a client id".into(),
            ));
        }
        "" => new_client_id(),
        id => id.to_owned(),
    };
    let mut options = MqttOptions::new(client_id, host, url.port().unwrap_or(default_port));
    options
        .set_clean_session(!settings.keep_session)
        .set_max_packet_size(MAX_PACKET, MAX_PACKET)
        .set_request_channel_capacity(256);
    if let Some(seconds) = settings.keep_alive {
        options.set_keep_alive(Duration::from_secs(seconds.into()));
    }
    match &req.auth {
        Auth::Basic { username, password } => {
            options.set_credentials(username.clone(), password.clone());
        }
        Auth::None | Auth::Inherit => {
            if !url.username().is_empty() {
                let decode = |s: &str| percent_decode_str(s).decode_utf8_lossy().into_owned();
                options.set_credentials(
                    decode(url.username()),
                    decode(url.password().unwrap_or_default()),
                );
            }
        }
        _ => {
            return Err(MqttError::Settings(
                "MQTT takes a user name and password: use Basic auth".into(),
            ));
        }
    }
    let effective = req.settings.effective();
    if url.scheme() == "mqtts" {
        let tls = apix_http::tls::client_config(&effective, &[])?;
        options.set_transport(Transport::tls_with_config(TlsConfiguration::Rustls(
            Arc::new(tls),
        )));
    }
    let mut subscribe = Vec::new();
    for subscription in &settings.subscribe {
        let topic = subscription.topic.trim();
        if topic.is_empty() {
            continue;
        }
        if !rumqttc::valid_filter(topic) {
            return Err(MqttError::Settings(format!(
                "`{topic}` is not a topic filter"
            )));
        }
        if quality(subscription.qos).is_none() {
            return Err(MqttError::Settings(format!(
                "`{topic}`: the quality of service is 0, 1 or 2"
            )));
        }
        subscribe.push((topic.to_owned(), subscription.qos));
    }
    Ok((options, subscribe))
}

/// Connects a resolved request and waits for the broker's answer, for the
/// request's timeout (10 s without one).
pub async fn connect(req: &Request) -> Result<MqttConnection, MqttError> {
    let (options, subscribe) = options(req)?;
    let client_id = options.client_id();
    let limit = req
        .settings
        .effective()
        .timeout
        .unwrap_or(Duration::from_secs(10));
    let (client, mut events) = AsyncClient::new(options, 256);
    let mut network = events.network_options();
    network.set_connection_timeout(limit.as_secs().max(1));
    events.set_network_options(network);
    let waiting = async {
        loop {
            match events.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(answer))) => return Ok(answer.session_present),
                Ok(_) => {}
                Err(e) => return Err(connection_error(e)),
            }
        }
    };
    let session_present = tokio::time::timeout(limit + Duration::from_secs(1), waiting)
        .await
        .map_err(|_| MqttError::TimedOut)??;
    Ok(MqttConnection {
        client_id,
        session_present,
        subscribe,
        client,
        events,
    })
}

fn connection_error(error: ConnectionError) -> MqttError {
    match error {
        ConnectionError::ConnectionRefused(code) => MqttError::Refused(match code {
            ConnectReturnCode::RefusedProtocolVersion => "it does not speak MQTT 3.1.1",
            ConnectReturnCode::BadClientId => "it does not accept this client id",
            ConnectReturnCode::ServiceUnavailable => "the service is unavailable",
            ConnectReturnCode::BadUserNamePassword => "wrong user name or password",
            ConnectReturnCode::NotAuthorized => "not authorized",
            ConnectReturnCode::Success => "no reason given",
        }),
        ConnectionError::NetworkTimeout => MqttError::TimedOut,
        other => MqttError::Connection(other.to_string()),
    }
}

/// Whether an error is only the other side having hung up.
fn is_hang_up(error: &ConnectionError) -> bool {
    use std::io::ErrorKind::{ConnectionAborted, ConnectionReset, UnexpectedEof};
    let io = match error {
        ConnectionError::MqttState(StateError::ConnectionAborted) => return true,
        ConnectionError::MqttState(StateError::Io(io)) | ConnectionError::Io(io) => io,
        _ => return false,
    };
    matches!(
        io.kind(),
        ConnectionAborted | ConnectionReset | UnexpectedEof
    )
}

/// What was handed to the client and waits for its packet id, oldest first.
#[derive(Default)]
struct Waiting {
    publishes: VecDeque<MqttMessage>,
    subscribes: VecDeque<(String, u8)>,
    unsubscribes: VecDeque<String>,
}

impl MqttConnection {
    /// Subscribes to the request's topics, then does what `commands` asks and
    /// reports everything to `on_event` until either side closes. Pings and
    /// the acknowledgements of incoming messages go by themselves.
    pub async fn run(
        self,
        mut commands: mpsc::UnboundedReceiver<MqttCommand>,
        on_event: impl FnMut(MqttEvent) + Send,
    ) {
        let MqttConnection {
            subscribe,
            client,
            mut events,
            ..
        } = self;
        let on_event = Mutex::new(on_event);
        let report = |event: MqttEvent| {
            let mut on_event = on_event.lock().unwrap_or_else(|e| e.into_inner());
            (*on_event)(event)
        };
        let waiting = Mutex::new(Waiting::default());
        let lock = || waiting.lock().unwrap_or_else(|e| e.into_inner());

        // Hands commands to the client. It waits when the client's queue is
        // full, so it runs beside the event loop that empties that queue.
        let asking = async {
            let subscribe = subscribe
                .into_iter()
                .map(|(topic, qos)| MqttCommand::Subscribe { topic, qos });
            let mut first: VecDeque<MqttCommand> = subscribe.collect();
            loop {
                let command = match first.pop_front() {
                    Some(command) => Some(command),
                    None => commands.recv().await,
                };
                match command {
                    Some(MqttCommand::Publish(message)) => {
                        let Some(qos) = quality(message.qos) else {
                            report(MqttEvent::Rejected(
                                "the quality of service is 0, 1 or 2".into(),
                            ));
                            continue;
                        };
                        if !rumqttc::valid_topic(&message.topic) {
                            report(MqttEvent::Rejected(format!(
                                "`{}` is not a topic to publish to",
                                message.topic
                            )));
                            continue;
                        }
                        let (topic, retain, payload) = (
                            message.topic.clone(),
                            message.retain,
                            message.payload.clone(),
                        );
                        lock().publishes.push_back(message);
                        if client.publish(topic, qos, retain, payload).await.is_err() {
                            break;
                        }
                    }
                    Some(MqttCommand::Subscribe { topic, qos }) => {
                        let Some(level) = quality(qos) else {
                            report(MqttEvent::Rejected(
                                "the quality of service is 0, 1 or 2".into(),
                            ));
                            continue;
                        };
                        if !rumqttc::valid_filter(&topic) {
                            report(MqttEvent::Rejected(format!(
                                "`{topic}` is not a topic filter"
                            )));
                            continue;
                        }
                        lock().subscribes.push_back((topic.clone(), qos));
                        if client.subscribe(topic, level).await.is_err() {
                            break;
                        }
                    }
                    Some(MqttCommand::Unsubscribe { topic }) => {
                        lock().unsubscribes.push_back(topic.clone());
                        if client.unsubscribe(topic).await.is_err() {
                            break;
                        }
                    }
                    // Asked to close, or nobody can ask anything any more.
                    Some(MqttCommand::Close) | None => {
                        let _ = client.disconnect().await;
                        break;
                    }
                }
            }
            std::future::pending::<()>().await
        };

        // Follows the connection until it ends; what the broker confirms is
        // matched by packet id with what was handed over.
        let following = async {
            let mut publishes: HashMap<u16, String> = HashMap::new();
            let mut subscribes: HashMap<u16, (String, u8)> = HashMap::new();
            let mut unsubscribes: HashMap<u16, String> = HashMap::new();
            loop {
                let event = match events.poll().await {
                    Ok(event) => event,
                    Err(e) if is_hang_up(&e) => {
                        // What it never answered may be why it hung up.
                        let mut unanswered: Vec<&String> =
                            subscribes.values().map(|(topic, _)| topic).collect();
                        unanswered.sort();
                        for topic in unanswered {
                            report(MqttEvent::Rejected(format!(
                                "the broker closed the connection without answering the subscription to `{topic}`"
                            )));
                        }
                        let mut unconfirmed: Vec<&String> = publishes.values().collect();
                        unconfirmed.sort();
                        for topic in unconfirmed {
                            report(MqttEvent::Rejected(format!(
                                "the broker closed the connection without confirming a message to `{topic}`"
                            )));
                        }
                        report(MqttEvent::Closed { by_broker: true });
                        return;
                    }
                    Err(e) => {
                        report(MqttEvent::Error(connection_error(e).to_string()));
                        return;
                    }
                };
                match event {
                    Event::Outgoing(Outgoing::Publish(id)) => {
                        let message = lock().publishes.pop_front();
                        if let Some(message) = message {
                            if id != 0 {
                                publishes.insert(id, message.topic.clone());
                            }
                            report(MqttEvent::Sent(message));
                        }
                    }
                    Event::Outgoing(Outgoing::Subscribe(id)) => {
                        if let Some(subscription) = lock().subscribes.pop_front() {
                            subscribes.insert(id, subscription);
                        }
                    }
                    Event::Outgoing(Outgoing::Unsubscribe(id)) => {
                        if let Some(topic) = lock().unsubscribes.pop_front() {
                            unsubscribes.insert(id, topic);
                        }
                    }
                    Event::Outgoing(Outgoing::Disconnect) => {
                        report(MqttEvent::Closed { by_broker: false });
                        return;
                    }
                    Event::Incoming(Packet::Publish(message)) => {
                        report(MqttEvent::Received(MqttMessage {
                            topic: message.topic,
                            payload: message.payload.to_vec(),
                            qos: level(message.qos),
                            retain: message.retain,
                        }))
                    }
                    Event::Incoming(Packet::PubAck(rumqttc::PubAck { pkid, .. }))
                    | Event::Incoming(Packet::PubComp(rumqttc::PubComp { pkid, .. })) => {
                        if let Some(topic) = publishes.remove(&pkid) {
                            report(MqttEvent::Delivered { topic });
                        }
                    }
                    Event::Incoming(Packet::SubAck(answer)) => {
                        if let Some((topic, _)) = subscribes.remove(&answer.pkid) {
                            report(match answer.return_codes.first() {
                                Some(SubscribeReasonCode::Success(qos)) => MqttEvent::Subscribed {
                                    topic,
                                    qos: level(*qos),
                                },
                                _ => MqttEvent::SubscribeRefused { topic },
                            });
                        }
                    }
                    Event::Incoming(Packet::UnsubAck(answer)) => {
                        if let Some(topic) = unsubscribes.remove(&answer.pkid) {
                            report(MqttEvent::Unsubscribed { topic });
                        }
                    }
                    _ => {}
                }
            }
        };
        tokio::select! {
            _ = following => {}
            _ = asking => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(url: &str) -> Request {
        Request::new(apix_core::Method::Get, url)
    }

    #[test]
    fn addresses_and_logins() {
        let (o, _) = options(&request("mqtt://broker.example.com")).unwrap();
        assert_eq!(o.broker_address(), ("broker.example.com".to_owned(), 1883));
        assert!(o.client_id().starts_with("apix-"));
        assert_eq!(o.credentials(), None);

        let (o, _) = options(&request("mqtts://b:9000")).unwrap();
        assert_eq!(o.broker_address(), ("b".to_owned(), 9000));
        assert!(matches!(o.transport(), Transport::Tls(_)));
        let (o, _) = options(&request("mqtts://b")).unwrap();
        assert_eq!(o.broker_address().1, 8883);

        let (o, _) = options(&request("mqtt://user%40x:p%3Ass@b")).unwrap();
        assert_eq!(
            o.credentials().map(|l| (l.username, l.password)),
            Some(("user@x".to_owned(), "p:ss".to_owned()))
        );

        let mut basic = request("mqtt://ignored:x@b");
        basic.auth = Auth::Basic {
            username: "u".into(),
            password: "p".into(),
        };
        let (o, _) = options(&basic).unwrap();
        assert_eq!(o.credentials().map(|l| l.username), Some("u".to_owned()));

        let mut bearer = request("mqtt://b");
        bearer.auth = Auth::Bearer { token: "t".into() };
        assert!(options(&bearer).unwrap_err().to_string().contains("Basic"));
        assert_eq!(display_url("mqtt://ann:secret@b:1883"), "mqtt://ann@b:1883");
        assert_eq!(display_url("mqtts://b"), "mqtts://b");
        assert_eq!(display_url("not a url"), "not a url");
        assert!(options(&request("ws://b")).is_err());
        assert!(options(&request("mqtt://")).is_err());
    }

    #[test]
    fn session_and_subscription_options() {
        let mut req = request("mqtt://b");
        req.mqtt = Some(MqttSettings {
            keep_session: true,
            ..Default::default()
        });
        assert!(options(&req).unwrap_err().to_string().contains("client id"));

        let subscription = |topic: &str, qos| apix_core::MqttSubscription {
            topic: topic.into(),
            qos,
        };
        req.mqtt = Some(MqttSettings {
            client_id: " me ".into(),
            keep_session: true,
            keep_alive: Some(0),
            subscribe: vec![subscription("a/+/#", 2), subscription("  ", 0)],
        });
        let (o, subscribe) = options(&req).unwrap();
        assert_eq!(o.client_id(), "me");
        assert!(!o.clean_session());
        assert_eq!(o.keep_alive(), Duration::ZERO);
        assert_eq!(subscribe, [("a/+/#".to_owned(), 2)]);

        req.mqtt.as_mut().unwrap().subscribe = vec![subscription("a/#/b", 0)];
        assert!(options(&req).is_err());
        req.mqtt.as_mut().unwrap().subscribe = vec![subscription("a", 3)];
        assert!(options(&req).is_err());
    }

    #[test]
    fn payloads() {
        assert_eq!(
            payload_from_text("hi", MessageFormat::Text).unwrap(),
            b"hi".to_vec()
        );
        assert_eq!(
            payload_from_text("01 ff", MessageFormat::Hex).unwrap(),
            vec![1, 255]
        );
        assert!(payload_from_text("zz", MessageFormat::Hex).is_err());
        assert_eq!(show_payload(b"hello"), "hello");
        assert_eq!(show_payload(&[0xff, 1]), "2 bytes: ff 01");
    }
}
