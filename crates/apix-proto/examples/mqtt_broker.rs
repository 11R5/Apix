//! An MQTT broker for trying Apix's MQTT requests:
//!
//! ```sh
//! cargo run -p apix-proto --example mqtt_broker
//! ```
//!
//! Connect to `mqtt://127.0.0.1:8772`. Topics with `+` and `#`, QoS 0, 1 and
//! 2 and retained messages work. A login needs the password `secret`, a
//! subscription under `forbidden/` is refused, and publishing to `kick` makes
//! the broker hang up. `clock` gets the time every second.

#[path = "../tests/common/broker.rs"]
mod broker;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8772").await?;
    println!("MQTT broker on mqtt://127.0.0.1:8772");
    let broker = broker::Broker::default();
    let clock = broker.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or_default();
            clock.publish("clock", &now.to_string());
        }
    });
    broker.serve(listener).await;
    Ok(())
}
