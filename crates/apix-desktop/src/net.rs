//! The bridge between GPUI's executor and tokio. reqwest needs a tokio
//! runtime, so one multi-threaded runtime runs for the whole app and the UI
//! awaits its results. Dropping the returned future aborts the request, so
//! cancelling is just dropping the GPUI task that awaits it.

use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;
use std::task::{Context, Poll};

use apix_core::Request;
use apix_http::{HttpClient, Response};
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

struct Net {
    runtime: Runtime,
    client: HttpClient,
}

fn net() -> &'static Net {
    static NET: OnceLock<Net> = OnceLock::new();
    NET.get_or_init(|| Net {
        runtime: tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("apix-net")
            .enable_all()
            .build()
            .expect("failed to start the network runtime"),
        client: match dirs::config_dir() {
            // Cookies live next to settings.toml and survive restarts.
            Some(dir) => HttpClient::with_cookie_file(dir.join("Apix").join("cookies.json")),
            None => HttpClient::new(),
        }
        .expect("failed to build the HTTP client"),
    })
}

/// The shared client, for the cookie manager.
pub fn client() -> &'static HttpClient {
    &net().client
}

/// Sends an already-resolved request on the network runtime.
pub fn send(request: Request) -> impl Future<Output = Result<Response, String>> + Send + 'static {
    let net = net();
    let client = net.client.clone();
    let handle = net
        .runtime
        .spawn(async move { client.send(&request).await.map_err(|e| error_chain(&e)) });
    AbortOnDrop(handle)
}

/// Sends a request and waits for it, for `pm.sendRequest` in scripts. Call
/// it from a background thread, never from the UI thread.
pub fn send_blocking(request: &Request) -> Result<Response, String> {
    let net = net();
    let client = net.client.clone();
    let request = request.clone();
    net.runtime
        .block_on(async move { client.send(&request).await.map_err(|e| error_chain(&e)) })
}

/// What scripts use for `pm.sendRequest`: the app's client and cookies.
pub fn script_sender() -> apix_script::Sender {
    apix_script::Sender::new(|request| {
        send_blocking(request).map(|response| {
            apix_script::ResponseView::new(
                response.status,
                response.reason.unwrap_or(""),
                &response.headers,
                &response.body,
                response.timing.total,
            )
        })
    })
}

/// Gets a new OAuth 2.0 token for an already-resolved configuration (and
/// keeps it for the requests that follow).
pub fn fetch_token(
    config: apix_core::OAuth2,
) -> impl Future<Output = Result<apix_http::oauth2::Token, String>> + Send + 'static {
    let handle = net().runtime.spawn(async move {
        apix_http::oauth2::fetch(&config)
            .await
            .map_err(|e| error_chain(&e))
    });
    AbortOnDrop(handle)
}

/// reqwest's top-level message ("error sending request") hides the cause,
/// such as "connection refused", so the whole chain is shown.
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

struct AbortOnDrop<T>(JoinHandle<Result<T, String>>);

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl<T> Future for AbortOnDrop<T> {
    type Output = Result<T, String>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0)
            .poll(cx)
            .map(|joined| joined.unwrap_or_else(|e| Err(format!("request task failed: {e}"))))
    }
}

/// Runs `work` on the network runtime (a collection run); dropping the
/// returned future aborts it.
pub fn spawn<T: Send + 'static>(
    work: impl Future<Output = T> + Send + 'static,
) -> impl Future<Output = Result<T, String>> + Send + 'static {
    AbortOnDrop(net().runtime.spawn(async move { Ok(work.await) }))
}

/// What a streamed send has received so far; the UI reads it while the
/// request runs.
#[derive(Default)]
pub struct Incoming {
    /// The status and headers, once they are in.
    pub head: Option<(u16, Vec<(String, String)>)>,
    /// The response is server-sent events.
    pub event_stream: bool,
    /// Body bytes the UI has not taken yet (event streams only).
    pub fresh: Vec<u8>,
    /// The finished response, or why it failed.
    pub result: Option<Result<Response, String>>,
}

/// Sends an already-resolved request on the network runtime, filling the
/// returned `Incoming` as the response arrives. Dropping the guard aborts
/// the request.
pub fn send_streaming(
    request: Request,
) -> (
    std::sync::Arc<std::sync::Mutex<Incoming>>,
    impl Drop + Send + 'static,
) {
    let incoming = std::sync::Arc::new(std::sync::Mutex::new(Incoming::default()));
    let shared = incoming.clone();
    let net = net();
    let client = net.client.clone();
    let handle = net.runtime.spawn(async move {
        let lock = || shared.lock().unwrap_or_else(|e| e.into_inner());
        let result = client
            .send_with(&request, |progress| match progress {
                apix_http::Progress::Head { status, headers } => {
                    let mut incoming = lock();
                    incoming.event_stream = headers.iter().any(|(k, v)| {
                        k.eq_ignore_ascii_case("content-type") && apix_http::sse::is_event_stream(v)
                    });
                    incoming.head = Some((status, headers.to_vec()));
                }
                apix_http::Progress::Chunk(chunk) => {
                    let mut incoming = lock();
                    if incoming.event_stream {
                        incoming.fresh.extend_from_slice(chunk);
                    }
                }
            })
            .await
            .map_err(|e| error_chain(&e));
        lock().result = Some(result);
        Ok(())
    });
    (incoming, AbortOnDrop(handle))
}

/// When a WebSocket handshake finished, with its status and headers.
pub type Opened = (std::time::Instant, u16, Vec<(String, String)>);

/// What an open WebSocket has to report; the UI takes it as it comes.
#[derive(Default)]
pub struct SocketIncoming {
    /// When the handshake finished, its status and headers.
    pub opened: Option<Opened>,
    /// Messages and the closing, each with when it happened.
    pub events: Vec<(std::time::Instant, apix_http::ws::WsEvent)>,
    /// Why the connection could not be opened.
    pub failed: Option<String>,
    /// The connection is over (closed or failed).
    pub done: bool,
}

/// Opens a WebSocket for an already-resolved request on the network
/// runtime. Commands sent before it is open wait their turn. Dropping the
/// guard drops the connection.
pub fn ws_connect(
    request: Request,
) -> (
    std::sync::Arc<std::sync::Mutex<SocketIncoming>>,
    tokio::sync::mpsc::UnboundedSender<apix_http::ws::WsCommand>,
    impl Drop + Send + 'static,
) {
    let incoming = std::sync::Arc::new(std::sync::Mutex::new(SocketIncoming::default()));
    let shared = incoming.clone();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = net().runtime.spawn(async move {
        let lock = || shared.lock().unwrap_or_else(|e| e.into_inner());
        match apix_http::ws::connect(&request).await {
            Err(e) => lock().failed = Some(error_chain(&e)),
            Ok(connection) => {
                lock().opened = Some((
                    std::time::Instant::now(),
                    connection.status,
                    connection.headers.clone(),
                ));
                connection
                    .run(rx, |event| {
                        lock().events.push((std::time::Instant::now(), event));
                    })
                    .await;
            }
        }
        lock().done = true;
        Ok(())
    });
    (incoming, tx, AbortOnDrop(handle))
}
