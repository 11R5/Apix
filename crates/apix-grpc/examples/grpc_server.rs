//! A gRPC server for trying Apix's gRPC requests:
//!
//! ```sh
//! cargo run -p apix-grpc --example grpc_server
//! ```
//!
//! then call `grpc://127.0.0.1:8770` — `apix.test.Greeter/SayHello` with
//! `{"name": "Ada"}`, `CountDown` with `{"from": 5}` (a tick a second),
//! `Sum` with a list of `{"value": n}`, or `Chat`. It offers server
//! reflection; the service is in tests/fixtures/greeter.proto.

#[path = "../tests/common/server.rs"]
mod server;

use std::time::Duration;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8770").await?;
    println!("serving apix.test.Greeter on grpc://127.0.0.1:8770 (with reflection)");
    server::serve(listener, true, Duration::from_secs(1)).await;
    Ok(())
}
