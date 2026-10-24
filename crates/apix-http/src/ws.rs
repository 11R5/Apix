//! WebSocket connections. The handshake is the request as Apix would send
//! it over HTTP (URL with params, headers, auth), so the same `.apix` file
//! works for both; then messages go both ways until either side closes.

use apix_core::{Auth, Request};
use futures_util::{SinkExt as _, StreamExt as _};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::{HttpError, oauth2};

/// A message's payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsData {
    Text(String),
    Binary(Vec<u8>),
}

impl WsData {
    pub fn len(&self) -> usize {
        match self {
            WsData::Text(text) => text.len(),
            WsData::Binary(bytes) => bytes.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// What happens on an open connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsEvent {
    Sent(WsData),
    Received(WsData),
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
pub enum WsCommand {
    Send(WsData),
    Close,
}

/// A connection after a successful handshake.
pub struct WsConnection {
    /// `101` and the server's headers.
    pub status: u16,
    pub headers: Vec<(String, String)>,
    stream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
}

/// Whether `url` (already resolved) is a WebSocket address.
pub fn is_websocket_url(url: &str) -> bool {
    let url = url.trim_start().to_ascii_lowercase();
    url.starts_with("ws://") || url.starts_with("wss://")
}

/// Opens a connection for a resolved request: its URL, params, headers and
/// auth make the handshake. Gives up after the request's timeout.
pub async fn connect(req: &Request) -> Result<WsConnection, HttpError> {
    let with_token;
    let req = match &req.auth {
        Auth::OAuth2(config) => {
            let token = oauth2::access_token(config).await?;
            with_token = Request {
                auth: Auth::Bearer { token },
                ..req.clone()
            };
            &with_token
        }
        _ => req,
    };
    // The HTTP side builds the URL (params, API key in the query) and the
    // headers (auth included) exactly as for a normal send.
    let client = reqwest::Client::builder().build()?;
    let prepared = crate::prepare(&client, req)?;
    let mut handshake = prepared
        .url()
        .as_str()
        .into_client_request()
        .map_err(|e| HttpError::Settings(format!("not a WebSocket address: {e}")))?;
    for (name, value) in prepared.headers() {
        // The handshake sets its own; a body header makes no sense here.
        if matches!(
            name.as_str(),
            "host" | "connection" | "upgrade" | "content-length" | "content-type"
        ) || name.as_str().starts_with("sec-websocket-key")
            || name.as_str() == "sec-websocket-version"
        {
            continue;
        }
        handshake.headers_mut().append(name.clone(), value.clone());
    }
    let connecting = tokio_tungstenite::connect_async(handshake);
    let result = match req.settings.effective().timeout {
        Some(limit) => tokio::time::timeout(limit, connecting)
            .await
            .map_err(|_| HttpError::TimedOut)?,
        None => connecting.await,
    };
    let (stream, response) = result.map_err(|e| HttpError::Settings(describe(&e)))?;
    Ok(WsConnection {
        status: response.status().as_u16(),
        headers: response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    String::from_utf8_lossy(v.as_bytes()).into_owned(),
                )
            })
            .collect(),
        stream,
    })
}

/// A handshake failure in words, with the server's answer when there was one.
fn describe(err: &tungstenite::Error) -> String {
    match err {
        tungstenite::Error::Http(response) => {
            let body = response
                .body()
                .as_deref()
                .map(|b| {
                    String::from_utf8_lossy(b)
                        .trim()
                        .chars()
                        .take(300)
                        .collect::<String>()
                })
                .filter(|b| !b.is_empty())
                .map(|b| format!(": {b}"))
                .unwrap_or_default();
            format!(
                "the server answered {} instead of switching to WebSocket{body}",
                response.status()
            )
        }
        other => format!("WebSocket handshake failed: {other}"),
    }
}

impl WsConnection {
    /// Sends what `commands` asks and reports everything to `on_event`
    /// until either side closes. Ping and pong are answered by themselves.
    pub async fn run(
        mut self,
        mut commands: mpsc::UnboundedReceiver<WsCommand>,
        mut on_event: impl FnMut(WsEvent) + Send,
    ) {
        let mut closing = false;
        loop {
            tokio::select! {
                command = commands.recv(), if !closing => match command {
                    Some(WsCommand::Send(data)) => {
                        let message = match &data {
                            WsData::Text(text) => Message::text(text.clone()),
                            WsData::Binary(bytes) => Message::binary(bytes.clone()),
                        };
                        match self.stream.send(message).await {
                            Ok(()) => on_event(WsEvent::Sent(data)),
                            Err(e) => {
                                on_event(WsEvent::Error(e.to_string()));
                                return;
                            }
                        }
                    }
                    // Asked to close, or nobody can ask anything any more.
                    Some(WsCommand::Close) | None => {
                        closing = true;
                        let frame = CloseFrame {
                            code: CloseCode::Normal,
                            reason: "".into(),
                        };
                        if self.stream.close(Some(frame)).await.is_err() {
                            on_event(WsEvent::Closed { code: Some(1000), reason: String::new() });
                            return;
                        }
                    }
                },
                message = next_or_wait(&mut self.stream, closing) => match message {
                    Some(Ok(Message::Text(text))) => {
                        on_event(WsEvent::Received(WsData::Text(text.as_str().to_owned())))
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        on_event(WsEvent::Received(WsData::Binary(bytes.to_vec())))
                    }
                    Some(Ok(Message::Close(frame))) => {
                        let (code, reason) = frame
                            .map(|f| (Some(u16::from(f.code)), f.reason.as_str().to_owned()))
                            .unwrap_or((None, String::new()));
                        // Let the closing handshake finish, briefly.
                        let _ = tokio::time::timeout(
                            std::time::Duration::from_secs(1),
                            self.stream.close(None),
                        )
                        .await;
                        on_event(WsEvent::Closed { code, reason });
                        return;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(tungstenite::Error::ConnectionClosed)) | None => {
                        on_event(WsEvent::Closed { code: None, reason: String::new() });
                        return;
                    }
                    // We asked to close; a server that just hangs up is done too.
                    Some(Err(_)) if closing => {
                        on_event(WsEvent::Closed { code: Some(1000), reason: String::new() });
                        return;
                    }
                    Some(Err(e)) => {
                        on_event(WsEvent::Error(e.to_string()));
                        return;
                    }
                },
            }
        }
    }
}

/// The next message; after we asked to close, wait at most 3 s for the
/// server's answer.
async fn next_or_wait(
    stream: &mut WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    closing: bool,
) -> Option<Result<Message, tungstenite::Error>> {
    if closing {
        tokio::time::timeout(std::time::Duration::from_secs(3), stream.next())
            .await
            .unwrap_or(None)
    } else {
        stream.next().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_addresses() {
        assert!(is_websocket_url("ws://localhost:8080/chat"));
        assert!(is_websocket_url("  WSS://example.com"));
        assert!(!is_websocket_url("https://example.com"));
        assert!(!is_websocket_url("{{ws}}"));
    }
}
