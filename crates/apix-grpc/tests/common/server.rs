//! The `apix.test.Greeter` service from tests/fixtures/greeter.proto,
//! answered with dynamic messages (nothing generated), optionally with
//! server reflection. Used by the tests and the `grpc_server` example.

use std::convert::Infallible;
use std::task::{Context, Poll};
use std::time::Duration;

use apix_grpc::DynamicCodec;
use apix_grpc::prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor, Value};
use apix_grpc::tonic::codegen::{BoxFuture, Service, http};
use apix_grpc::tonic::server::{
    ClientStreamingService, NamedService, ServerStreamingService, StreamingService, UnaryService,
};
use apix_grpc::tonic::{self, Request, Response, Status, Streaming};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};

pub fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

pub fn pool() -> DescriptorPool {
    apix_grpc::from_protos(&[fixture("greeter.proto")], &[]).expect("the test protos compile")
}

/// Serves the Greeter on `listener` until the process ends.
pub async fn serve(listener: TcpListener, reflection: bool, tick: Duration) {
    let pool = pool();
    let greeter = Greeter {
        pool: pool.clone(),
        tick,
    };
    let reflection = reflection.then(|| {
        tonic_reflection::server::Builder::configure()
            // The reflection service keeps the bytes for good.
            .register_encoded_file_descriptor_set(Box::leak(
                pool.encode_to_vec().into_boxed_slice(),
            ))
            .build_v1()
            .expect("the reflection service builds")
    });
    tonic::transport::Server::builder()
        .add_service(greeter)
        .add_optional_service(reflection)
        .serve_with_incoming(TcpListenerStream::new(listener))
        .await
        .expect("the test server runs");
}

#[derive(Clone)]
struct Greeter {
    pool: DescriptorPool,
    tick: Duration,
}

impl NamedService for Greeter {
    const NAME: &'static str = "apix.test.Greeter";
}

fn text(message: &DynamicMessage, field: &str) -> String {
    match message.get_field_by_name(field).as_deref() {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

fn number(message: &DynamicMessage, field: &str) -> i64 {
    match message.get_field_by_name(field).as_deref() {
        Some(Value::I32(n)) => i64::from(*n),
        Some(Value::I64(n)) => *n,
        _ => 0,
    }
}

fn make(desc: &MessageDescriptor, fields: Vec<(&str, Value)>) -> DynamicMessage {
    let mut message = DynamicMessage::new(desc.clone());
    for (name, value) in fields {
        message.set_field_by_name(name, value);
    }
    message
}

type Messages = ReceiverStream<Result<DynamicMessage, Status>>;

struct SayHello(MessageDescriptor);

impl UnaryService<DynamicMessage> for SayHello {
    type Response = DynamicMessage;
    type Future = BoxFuture<Response<DynamicMessage>, Status>;

    fn call(&mut self, request: Request<DynamicMessage>) -> Self::Future {
        let output = self.0.clone();
        Box::pin(async move {
            let seen = request
                .metadata()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_owned();
            let name = text(request.get_ref(), "name");
            if name.is_empty() {
                return Err(Status::invalid_argument("name is required"));
            }
            let mut response = Response::new(make(
                &output,
                vec![("message", Value::String(format!("Hello, {name}!")))],
            ));
            if !seen.is_empty() {
                response
                    .metadata_mut()
                    .insert("x-seen-auth", seen.parse().expect("ASCII"));
            }
            Ok(response)
        })
    }
}

struct CountDown(MessageDescriptor, Duration);

impl ServerStreamingService<DynamicMessage> for CountDown {
    type Response = DynamicMessage;
    type ResponseStream = Messages;
    type Future = BoxFuture<Response<Messages>, Status>;

    fn call(&mut self, request: Request<DynamicMessage>) -> Self::Future {
        let (output, tick) = (self.0.clone(), self.1);
        Box::pin(async move {
            let from = number(request.get_ref(), "from");
            if !(0..=100).contains(&from) {
                return Err(Status::out_of_range("count from 0 to 100"));
            }
            let (tx, rx) = mpsc::channel(4);
            tokio::spawn(async move {
                for n in (1..=from).rev() {
                    let tick_message = make(&output, vec![("n", Value::I32(n as i32))]);
                    if tx.send(Ok(tick_message)).await.is_err() {
                        return;
                    }
                    tokio::time::sleep(tick).await;
                }
            });
            Ok(Response::new(ReceiverStream::new(rx)))
        })
    }
}

struct Sum(MessageDescriptor);

impl ClientStreamingService<DynamicMessage> for Sum {
    type Response = DynamicMessage;
    type Future = BoxFuture<Response<DynamicMessage>, Status>;

    fn call(&mut self, request: Request<Streaming<DynamicMessage>>) -> Self::Future {
        let output = self.0.clone();
        Box::pin(async move {
            let mut numbers = request.into_inner();
            let (mut sum, mut count) = (0i64, 0i32);
            while let Some(message) = numbers.message().await? {
                sum += number(&message, "value");
                count += 1;
            }
            Ok(Response::new(make(
                &output,
                vec![("sum", Value::I64(sum)), ("count", Value::I32(count))],
            )))
        })
    }
}

struct Chat(MessageDescriptor);

impl StreamingService<DynamicMessage> for Chat {
    type Response = DynamicMessage;
    type ResponseStream = Messages;
    type Future = BoxFuture<Response<Messages>, Status>;

    fn call(&mut self, request: Request<Streaming<DynamicMessage>>) -> Self::Future {
        let output = self.0.clone();
        Box::pin(async move {
            let mut incoming = request.into_inner();
            let (tx, rx) = mpsc::channel(4);
            tokio::spawn(async move {
                while let Ok(Some(message)) = incoming.message().await {
                    let reply = format!("echo: {}", text(&message, "text"));
                    let answer = make(&output, vec![("text", Value::String(reply))]);
                    if tx.send(Ok(answer)).await.is_err() {
                        return;
                    }
                }
            });
            Ok(Response::new(ReceiverStream::new(rx)))
        })
    }
}

impl Service<http::Request<tonic::body::Body>> for Greeter {
    type Response = http::Response<tonic::body::Body>;
    type Error = Infallible;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<tonic::body::Body>) -> Self::Future {
        let (pool, tick) = (self.pool.clone(), self.tick);
        Box::pin(async move {
            let name = req
                .uri()
                .path()
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_owned();
            let method = pool
                .get_service_by_name(Self::NAME)
                .and_then(|s| s.methods().find(|m| m.name() == name));
            let Some(method) = method else {
                return Ok(Status::unimplemented(format!("no method {name}")).into_http());
            };
            let mut grpc = tonic::server::Grpc::new(DynamicCodec::new(method.input()));
            let output = method.output();
            Ok(match name.as_str() {
                "SayHello" => grpc.unary(SayHello(output), req).await,
                "CountDown" => grpc.server_streaming(CountDown(output, tick), req).await,
                "Sum" => grpc.client_streaming(Sum(output), req).await,
                _ => grpc.streaming(Chat(output), req).await,
            })
        })
    }
}
