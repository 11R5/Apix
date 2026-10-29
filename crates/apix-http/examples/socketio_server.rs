//! A Socket.IO 4 server for trying Apix's Socket.IO requests:
//!
//! ```sh
//! cargo run -p apix-http --example socketio_server
//! ```
//!
//! Connect to `http://127.0.0.1:8771` (namespaces `/`, `/chat`, `/ticker`
//! and `/private`, which wants the auth `{"token": "letmein"}`). It greets
//! with `welcome`, echoes `echo` (binary too), acknowledges every event that
//! asks for it, answers `ask` with a `question` that wants an ack, and makes
//! you leave on `bye`; `/ticker` sends `tick` every second.

#[path = "../tests/common/socketio_server.rs"]
mod server;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8771").await?;
    println!("Socket.IO on http://127.0.0.1:8771 (namespaces /, /chat, /ticker, /private)");
    let seen = server::Seen::default();
    server::serve(listener, seen).await;
    Ok(())
}
