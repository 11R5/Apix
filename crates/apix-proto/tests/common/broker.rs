//! A small MQTT 3.1.1 broker for the tests and the `mqtt_broker` example.
//! Topics with `+` and `#`, QoS 0, 1 and 2 (delivered at the lower of the
//! publisher's and subscriber's), retained messages, and:
//!
//! - a user name with a password other than `secret` is refused,
//! - a subscription under `forbidden/` is refused,
//! - publishing to `kick` makes the broker hang up,
//! - a client id seen before with `keep_session` finds its session present.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use bytes::BytesMut;
use rumqttc::{
    ConnAck, ConnectReturnCode, Packet, PubAck, PubComp, PubRec, PubRel, Publish, QoS, SubAck,
    SubscribeReasonCode, UnsubAck,
};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

const MAX: usize = 1024 * 1024;

#[derive(Default)]
struct State {
    /// Connection number → its subscriptions and where to send what matches.
    clients: HashMap<u64, Client>,
    retained: HashMap<String, Publish>,
    sessions: HashSet<String>,
    next: u64,
}

struct Client {
    filters: Vec<(String, QoS)>,
    to_client: mpsc::UnboundedSender<Publish>,
}

#[derive(Clone, Default)]
pub struct Broker {
    state: Arc<Mutex<State>>,
    /// The logins and client ids that connected.
    pub seen: Arc<Mutex<Vec<String>>>,
}

impl Broker {
    pub async fn serve(self, listener: TcpListener) {
        while let Ok((socket, _)) = listener.accept().await {
            tokio::spawn(self.clone().session(socket));
        }
    }

    /// Serves one connection, over any stream.
    pub async fn session<S: AsyncRead + AsyncWrite + Unpin>(self, mut socket: S) {
        let (to_client, mut outgoing) = mpsc::unbounded_channel();
        let number = {
            let mut state = self.state.lock().unwrap();
            state.next += 1;
            state.next
        };
        let mut input = BytesMut::new();
        let mut output = BytesMut::new();
        let mut next_id = 0u16;
        loop {
            // Whole packets first, then whichever comes next.
            loop {
                match Packet::read(&mut input, MAX) {
                    Ok(packet) => {
                        let go_on = self.handle(packet, number, &to_client, &mut output);
                        if !output.is_empty() && socket.write_all(&output).await.is_err() {
                            return self.forget(number);
                        }
                        output.clear();
                        if !go_on {
                            return self.forget(number);
                        }
                    }
                    Err(rumqttc::Error::InsufficientBytes(_)) => break,
                    Err(_) => return self.forget(number),
                }
            }
            tokio::select! {
                read = socket.read_buf(&mut input) => match read {
                    Ok(0) | Err(_) => return self.forget(number),
                    Ok(_) => {}
                },
                Some(mut message) = outgoing.recv() => {
                    if message.qos != QoS::AtMostOnce {
                        next_id = next_id % 1000 + 1;
                        message.pkid = next_id;
                    }
                    Packet::Publish(message).write(&mut output, MAX).unwrap();
                    if socket.write_all(&output).await.is_err() {
                        return self.forget(number);
                    }
                    output.clear();
                }
            }
        }
    }

    fn forget(&self, number: u64) {
        self.state.lock().unwrap().clients.remove(&number);
    }

    /// Answers a packet; false when the connection should end.
    fn handle(
        &self,
        packet: Packet,
        number: u64,
        to_client: &mpsc::UnboundedSender<Publish>,
        output: &mut BytesMut,
    ) -> bool {
        let mut answer = |packet: Packet| {
            packet.write(output, MAX).unwrap();
        };
        match packet {
            Packet::Connect(connect) => {
                let login = connect.login.as_ref();
                self.seen.lock().unwrap().push(format!(
                    "{} {} clean={}",
                    login.map(|l| l.username.as_str()).unwrap_or("-"),
                    connect.client_id,
                    connect.clean_session
                ));
                if login.is_some_and(|l| l.password != "secret") {
                    let refused = ConnAck::new(ConnectReturnCode::BadUserNamePassword, false);
                    answer(Packet::ConnAck(refused));
                    return false;
                }
                let mut state = self.state.lock().unwrap();
                let present = !connect.clean_session && state.sessions.contains(&connect.client_id);
                if !connect.clean_session {
                    state.sessions.insert(connect.client_id);
                }
                state.clients.insert(
                    number,
                    Client {
                        filters: Vec::new(),
                        to_client: to_client.clone(),
                    },
                );
                answer(Packet::ConnAck(ConnAck::new(
                    ConnectReturnCode::Success,
                    present,
                )));
            }
            Packet::Publish(publish) => {
                match publish.qos {
                    QoS::AtMostOnce => {}
                    QoS::AtLeastOnce => answer(Packet::PubAck(PubAck::new(publish.pkid))),
                    QoS::ExactlyOnce => answer(Packet::PubRec(PubRec::new(publish.pkid))),
                }
                if publish.topic == "kick" {
                    return false;
                }
                self.deliver(publish);
            }
            Packet::PubRel(PubRel { pkid, .. }) => answer(Packet::PubComp(PubComp::new(pkid))),
            // A subscriber's half of QoS 2.
            Packet::PubRec(PubRec { pkid, .. }) => answer(Packet::PubRel(PubRel::new(pkid))),
            Packet::Subscribe(subscribe) => {
                let mut state = self.state.lock().unwrap();
                let mut codes = Vec::new();
                let mut retained = Vec::new();
                for filter in subscribe.filters {
                    if filter.path.starts_with("forbidden/") {
                        codes.push(SubscribeReasonCode::Failure);
                        continue;
                    }
                    codes.push(SubscribeReasonCode::Success(filter.qos));
                    for message in state.retained.values() {
                        if rumqttc::matches(&message.topic, &filter.path) {
                            let mut message = message.clone();
                            message.qos = lower(message.qos, filter.qos);
                            retained.push(message);
                        }
                    }
                    if let Some(client) = state.clients.get_mut(&number) {
                        client.filters.push((filter.path, filter.qos));
                    }
                }
                answer(Packet::SubAck(SubAck::new(subscribe.pkid, codes)));
                for message in retained {
                    let _ = to_client.send(message);
                }
            }
            Packet::Unsubscribe(unsubscribe) => {
                let mut state = self.state.lock().unwrap();
                if let Some(client) = state.clients.get_mut(&number) {
                    client
                        .filters
                        .retain(|(path, _)| !unsubscribe.topics.contains(path));
                }
                answer(Packet::UnsubAck(UnsubAck::new(unsubscribe.pkid)));
            }
            Packet::PingReq => answer(Packet::PingResp),
            Packet::Disconnect => return false,
            _ => {}
        }
        true
    }

    /// A message from the broker itself.
    #[allow(dead_code)] // Only the example uses it.
    pub fn publish(&self, topic: &str, payload: &str) {
        self.deliver(Publish::new(topic, QoS::AtMostOnce, payload.as_bytes()));
    }

    /// Hands a message to every subscriber whose filter matches.
    fn deliver(&self, publish: Publish) {
        let mut state = self.state.lock().unwrap();
        if publish.retain {
            if publish.payload.is_empty() {
                state.retained.remove(&publish.topic);
            } else {
                state
                    .retained
                    .insert(publish.topic.clone(), publish.clone());
            }
        }
        for client in state.clients.values() {
            let matching = client
                .filters
                .iter()
                .filter(|(path, _)| rumqttc::matches(&publish.topic, path))
                .map(|(_, qos)| *qos)
                .max_by_key(|qos| *qos as u8);
            if let Some(qos) = matching {
                let mut message = publish.clone();
                message.qos = lower(publish.qos, qos);
                // Retain is only for messages kept from before subscribing.
                message.retain = false;
                let _ = client.to_client.send(message);
            }
        }
    }
}

fn lower(a: QoS, b: QoS) -> QoS {
    if (a as u8) < (b as u8) { a } else { b }
}
