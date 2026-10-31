//! MQTT connections against a local broker.

#[path = "common/broker.rs"]
mod broker;

use std::sync::Arc;
use std::time::Duration;

use apix_core::{Auth, Method, MqttSettings, MqttSubscription, Request};
use apix_proto::mqtt::{self, MqttCommand, MqttEvent, MqttMessage};
use broker::Broker;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

async fn start() -> (String, Broker) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("mqtt://{}", listener.local_addr().unwrap());
    let broker = Broker::default();
    tokio::spawn(broker.clone().serve(listener));
    (url, broker)
}

fn request(url: &str, subscribe: &[(&str, u8)]) -> Request {
    Request {
        mqtt: Some(MqttSettings {
            subscribe: subscribe
                .iter()
                .map(|(topic, qos)| MqttSubscription {
                    topic: (*topic).into(),
                    qos: *qos,
                })
                .collect(),
            ..Default::default()
        }),
        ..Request::new(Method::Get, url)
    }
}

fn message(topic: &str, payload: &[u8], qos: u8, retain: bool) -> MqttMessage {
    MqttMessage {
        topic: topic.into(),
        payload: payload.to_vec(),
        qos,
        retain,
    }
}

fn publish(topic: &str, payload: &[u8], qos: u8) -> MqttCommand {
    MqttCommand::Publish(message(topic, payload, qos, false))
}

/// Runs a connection with these commands, each after a short wait so the one
/// before has gone through, then closes; gives what happened.
async fn run(connection: mqtt::MqttConnection, commands: Vec<MqttCommand>) -> Vec<MqttEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        for command in commands {
            tokio::time::sleep(Duration::from_millis(60)).await;
            let _ = tx.send(command);
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
        let _ = tx.send(MqttCommand::Close);
    });
    let mut events = Vec::new();
    connection.run(rx, |event| events.push(event)).await;
    events
}

#[tokio::test]
async fn publish_and_receive_at_every_quality_of_service() {
    let (url, _) = start().await;
    let connection = mqtt::connect(&request(&url, &[("sensors/#", 2), ("one/+/x", 1)]))
        .await
        .unwrap();
    assert!(connection.client_id.starts_with("apix-"));
    assert!(!connection.session_present);
    let events = run(
        connection,
        vec![
            publish("sensors/a", b"zero", 0),
            publish("sensors/b", b"one", 1),
            publish("sensors/c", b"two", 2),
            // Delivered at the lower quality of the subscription.
            publish("one/two/x", &[0xff, 1], 2),
            publish("nobody/listens", b"lost", 0),
        ],
    )
    .await;
    for wanted in [
        MqttEvent::Subscribed {
            topic: "sensors/#".into(),
            qos: 2,
        },
        MqttEvent::Subscribed {
            topic: "one/+/x".into(),
            qos: 1,
        },
        MqttEvent::Sent(message("sensors/a", b"zero", 0, false)),
        MqttEvent::Received(message("sensors/a", b"zero", 0, false)),
        MqttEvent::Delivered {
            topic: "sensors/b".into(),
        },
        MqttEvent::Received(message("sensors/b", b"one", 1, false)),
        MqttEvent::Delivered {
            topic: "sensors/c".into(),
        },
        MqttEvent::Received(message("sensors/c", b"two", 2, false)),
        MqttEvent::Received(message("one/two/x", &[0xff, 1], 1, false)),
        MqttEvent::Sent(message("nobody/listens", b"lost", 0, false)),
        MqttEvent::Closed { by_broker: false },
    ] {
        assert!(events.contains(&wanted), "{wanted:?} in {events:#?}");
    }
    assert!(
        !events.contains(&MqttEvent::Received(message(
            "nobody/listens",
            b"lost",
            0,
            false
        ))),
        "{events:#?}"
    );
    // QoS 0 has nothing to confirm.
    assert!(!events.contains(&MqttEvent::Delivered {
        topic: "sensors/a".into()
    }));
}

#[tokio::test]
async fn subscribing_and_unsubscribing_on_an_open_connection() {
    let (url, _) = start().await;
    let connection = mqtt::connect(&request(&url, &[])).await.unwrap();
    let events = run(
        connection,
        vec![
            MqttCommand::Subscribe {
                topic: "live".into(),
                qos: 1,
            },
            publish("live", b"seen", 0),
            MqttCommand::Unsubscribe {
                topic: "live".into(),
            },
            publish("live", b"not seen", 0),
            MqttCommand::Subscribe {
                topic: "forbidden/x".into(),
                qos: 0,
            },
            // Refused before they reach the broker; the connection goes on.
            publish("no/+/wildcards", b"", 0),
            MqttCommand::Subscribe {
                topic: "a/#/b".into(),
                qos: 0,
            },
            publish("still/ok", b"", 0),
        ],
    )
    .await;
    assert!(events.contains(&MqttEvent::Received(message("live", b"seen", 0, false))));
    assert!(!events.contains(&MqttEvent::Received(message("live", b"not seen", 0, false))));
    assert!(events.contains(&MqttEvent::Unsubscribed {
        topic: "live".into()
    }));
    assert!(events.contains(&MqttEvent::SubscribeRefused {
        topic: "forbidden/x".into()
    }));
    let rejected = events
        .iter()
        .filter(|e| matches!(e, MqttEvent::Rejected(_)))
        .count();
    assert_eq!(rejected, 2, "{events:#?}");
    assert!(events.contains(&MqttEvent::Sent(message("still/ok", b"", 0, false))));
}

#[tokio::test]
async fn retained_messages_reach_later_subscribers() {
    let (url, _) = start().await;
    let first = mqtt::connect(&request(&url, &[])).await.unwrap();
    let commands = vec![MqttCommand::Publish(message("state", b"on", 1, true))];
    run(first, commands).await;
    let second = mqtt::connect(&request(&url, &[("state", 1)]))
        .await
        .unwrap();
    let events = run(second, Vec::new()).await;
    assert!(
        events.contains(&MqttEvent::Received(message("state", b"on", 1, true))),
        "{events:#?}"
    );
}

#[tokio::test]
async fn logins_sessions_and_refusals() {
    let (url, broker) = start().await;
    let mut req = request(&url, &[]);
    req.auth = Auth::Basic {
        username: "ann".into(),
        password: "wrong".into(),
    };
    let refused = mqtt::connect(&req).await.err().unwrap();
    assert_eq!(
        refused.to_string(),
        "the broker refused the connection: wrong user name or password"
    );

    req.auth = Auth::Basic {
        username: "ann".into(),
        password: "secret".into(),
    };
    let settings = req.mqtt.as_mut().unwrap();
    settings.client_id = "me".into();
    settings.keep_session = true;
    let connection = mqtt::connect(&req).await.unwrap();
    assert_eq!(connection.client_id, "me");
    assert!(!connection.session_present);
    run(connection, Vec::new()).await;
    assert!(mqtt::connect(&req).await.unwrap().session_present);
    assert_eq!(
        broker.seen.lock().unwrap()[1..],
        ["ann me clean=false", "ann me clean=false"]
    );
}

#[tokio::test]
async fn the_broker_hanging_up_and_nobody_home() {
    let (url, _) = start().await;
    let connection = mqtt::connect(&request(&url, &[])).await.unwrap();
    let events = run(connection, vec![publish("kick", b"", 1)]).await;
    assert_eq!(
        events.last(),
        Some(&MqttEvent::Closed { by_broker: true }),
        "{events:#?}"
    );

    // A port nobody listens on.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let free = format!("mqtt://{}", listener.local_addr().unwrap());
    drop(listener);
    let error = mqtt::connect(&request(&free, &[])).await.err().unwrap();
    assert!(
        error.to_string().starts_with("cannot reach the broker"),
        "{error}"
    );
}

#[tokio::test]
async fn mqtts_with_an_own_authority_or_without_verifying() {
    use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair};
    use rustls::pki_types::pem::PemObject as _;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};

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

    let certs = CertificateDer::pem_slice_iter(server_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let key = PrivateKeyDer::from_pem_slice(server_key.serialize_pem().as_bytes()).unwrap();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(certs, key)
    .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("mqtts://{}", listener.local_addr().unwrap());
    let broker = Broker::default();
    tokio::spawn(async move {
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            let (acceptor, broker) = (acceptor.clone(), broker.clone());
            tokio::spawn(async move {
                if let Ok(tls) = acceptor.accept(socket).await {
                    broker.session(tls).await;
                }
            });
        }
    });

    let dir = std::env::temp_dir().join(format!("apix-mqtts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ca_file = dir.join("ca.pem");
    std::fs::write(&ca_file, &ca_pem).unwrap();

    let mut req = request(&url, &[("t", 0)]);
    assert!(mqtt::connect(&req).await.is_err(), "an unknown authority");
    req.settings.ca_cert = Some(ca_file.to_string_lossy().into_owned());
    let connection = mqtt::connect(&req).await.unwrap();
    let events = run(connection, vec![publish("t", b"over tls", 0)]).await;
    assert!(
        events.contains(&MqttEvent::Received(message("t", b"over tls", 0, false))),
        "{events:#?}"
    );
    req.settings.ca_cert = None;
    req.settings.tls_verify = Some(false);
    mqtt::connect(&req).await.unwrap();
    std::fs::remove_dir_all(&dir).ok();
}
