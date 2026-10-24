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
