//! OAuth 2.0 access tokens. Apix gets the token itself (client credentials,
//! password, or authorization code with PKCE through the browser), keeps it
//! in memory while the program runs and renews it when it expires. Tokens
//! are never written to disk.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use apix_core::{ClientAuth, OAuth2, OAuth2Grant, sign};
use reqwest::Url;
use reqwest::header::ACCEPT;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};

use crate::HttpError;

/// How long the browser sign-in may take.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);
/// A token this close to expiring is renewed before it is used.
const EXPIRY_MARGIN: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct Token {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// When the server said it expires; `None` if it did not say.
    pub expires_at: Option<Instant>,
    pub scope: Option<String>,
}

impl Token {
    fn usable(&self) -> bool {
        self.expires_at
            .is_none_or(|at| at > Instant::now() + EXPIRY_MARGIN)
    }

    /// Time left, if the server gave an expiry.
    pub fn expires_in(&self) -> Option<Duration> {
        self.expires_at
            .map(|at| at.saturating_duration_since(Instant::now()))
    }
}

static CACHE: LazyLock<Mutex<HashMap<String, Token>>> = LazyLock::new(Default::default);

/// Requests with the same server, client, grant, scope and user share a token.
fn key(c: &OAuth2) -> String {
    [
        format!("{:?}", c.grant),
        c.token_url.clone(),
        c.auth_url.clone(),
        c.client_id.clone(),
        c.scope.clone(),
        c.username.clone(),
    ]
    .join("\n")
}

/// The token kept for `config`, even if it has expired.
pub fn cached(config: &OAuth2) -> Option<Token> {
    CACHE.lock().ok()?.get(&key(config)).cloned()
}

/// Drops the kept token, so the next request gets a new one.
pub fn forget(config: &OAuth2) {
    if let Ok(mut cache) = CACHE.lock() {
        cache.remove(&key(config));
    }
}

fn keep(config: &OAuth2, token: Token) {
    if let Ok(mut cache) = CACHE.lock() {
        cache.insert(key(config), token);
    }
}

/// A token to send now: the kept one while it is valid, then a refreshed
/// one, otherwise a new one.
pub async fn access_token(config: &OAuth2) -> Result<String, HttpError> {
    if let Some(token) = cached(config) {
        if token.usable() {
            return Ok(token.access_token);
        }
        if let Some(refresh) = &token.refresh_token
            && let Ok(mut renewed) = request_token(
                config,
                &[("grant_type", "refresh_token"), ("refresh_token", refresh)],
            )
            .await
        {
            renewed.refresh_token = renewed.refresh_token.or(token.refresh_token.clone());
            let access = renewed.access_token.clone();
            keep(config, renewed);
            return Ok(access);
        }
    }
    Ok(fetch(config).await?.access_token)
}

/// Gets a new token now, whatever is kept, and keeps it.
pub async fn fetch(config: &OAuth2) -> Result<Token, HttpError> {
    if config.token_url.trim().is_empty() {
        return Err(HttpError::Auth("Set the OAuth 2.0 token URL.".into()));
    }
    if config.client_id.trim().is_empty() {
        return Err(HttpError::Auth("Set the OAuth 2.0 client ID.".into()));
    }
    let scope = config.scope.trim();
    let token = match config.grant {
        OAuth2Grant::ClientCredentials => {
            let mut params = vec![("grant_type", "client_credentials")];
            if !scope.is_empty() {
                params.push(("scope", scope));
            }
            request_token(config, &params).await?
        }
        OAuth2Grant::Password => {
            let mut params = vec![
                ("grant_type", "password"),
                ("username", config.username.as_str()),
                ("password", config.password.as_str()),
            ];
            if !scope.is_empty() {
                params.push(("scope", scope));
            }
            request_token(config, &params).await?
        }
        OAuth2Grant::AuthorizationCode => {
            let login = authorize(config).await?;
            let mut params = vec![
                ("grant_type", "authorization_code"),
                ("code", login.code.as_str()),
                ("redirect_uri", login.redirect_uri.as_str()),
            ];
            if let Some(verifier) = &login.verifier {
                params.push(("code_verifier", verifier));
            }
            request_token(config, &params).await?
        }
    };
    keep(config, token.clone());
    Ok(token)
}

async fn request_token(config: &OAuth2, params: &[(&str, &str)]) -> Result<Token, HttpError> {
    let mut form = params.to_vec();
    let client = reqwest::Client::builder()
        .user_agent(crate::USER_AGENT)
        .build()?;
    let mut builder = client
        .post(&config.token_url)
        .header(ACCEPT, "application/json");
    // A public client (no secret) always names itself in the body.
    if config.client_auth == ClientAuth::Basic && !config.client_secret.is_empty() {
        builder = builder.basic_auth(&config.client_id, Some(&config.client_secret));
    } else {
        form.push(("client_id", &config.client_id));
        if !config.client_secret.is_empty() {
            form.push(("client_secret", &config.client_secret));
        }
    }
    let response = builder.form(&form).send().await?;
    let status = response.status();
    let text = response.text().await?;
    let fields = response_fields(&text);
    if let Some(error) = fields.get("error") {
        let detail = fields
            .get("error_description")
            .map(|d| format!(" ({d})"))
            .unwrap_or_default();
        return Err(HttpError::Auth(format!(
            "The token endpoint refused: {error}{detail}"
        )));
    }
    if !status.is_success() {
        let snippet: String = text.chars().take(200).collect();
        return Err(HttpError::Auth(format!(
            "The token endpoint answered {status}: {snippet}"
        )));
    }
    let access_token = fields
        .get("access_token")
        .filter(|t| !t.is_empty())
        .cloned()
        .ok_or_else(|| HttpError::Auth("The token endpoint sent no access_token.".into()))?;
    Ok(Token {
        access_token,
        refresh_token: fields.get("refresh_token").cloned(),
        expires_at: fields
            .get("expires_in")
            .and_then(|s| s.parse::<u64>().ok())
            .map(|secs| Instant::now() + Duration::from_secs(secs)),
        scope: fields.get("scope").cloned(),
    })
}

/// A token response as text fields: JSON (the standard) or a URL-encoded
/// form, which some servers still send.
fn response_fields(text: &str) -> HashMap<String, String> {
    if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(text) {
        return map
            .into_iter()
            .filter_map(|(k, v)| match v {
                serde_json::Value::String(s) => Some((k, s)),
                serde_json::Value::Number(n) => Some((k, n.to_string())),
                _ => None,
            })
            .collect();
    }
    Url::parse(&format!("http://x/?{}", text.trim()))
        .map(|url| url.query_pairs().into_owned().collect())
        .unwrap_or_default()
}

struct Login {
    code: String,
    redirect_uri: String,
    verifier: Option<String>,
}

/// Opens the sign-in page in the browser and waits for the code to come
/// back to a loopback address.
async fn authorize(config: &OAuth2) -> Result<Login, HttpError> {
    let mut url = Url::parse(config.auth_url.trim()).map_err(|_| {
        HttpError::Auth("Set the authorization URL for the authorization code grant.".into())
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(HttpError::Auth(
            "The authorization URL must start with https://.".into(),
        ));
    }
    let (listener, redirect_uri, path) = listen(config.redirect_uri.trim()).await?;
    let state = sign::random_token(16);
    let verifier = config.pkce.then(|| sign::random_token(32));
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("response_type", "code")
            .append_pair("client_id", &config.client_id)
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("state", &state);
        if !config.scope.trim().is_empty() {
            query.append_pair("scope", config.scope.trim());
        }
        if let Some(verifier) = &verifier {
            query
                .append_pair("code_challenge", &sign::pkce_challenge(verifier))
                .append_pair("code_challenge_method", "S256");
        }
    }
    open_browser(url.as_str())?;
    let code = tokio::time::timeout(LOGIN_TIMEOUT, wait_for_code(&listener, &path, &state))
        .await
        .map_err(|_| HttpError::Auth("The sign-in was not finished within 5 minutes.".into()))??;
    Ok(Login {
        code,
        redirect_uri,
        verifier,
    })
}

/// Listens where the sign-in sends the browser back: the configured loopback
/// address, or a free port on 127.0.0.1. Returns the redirect URI and its path.
async fn listen(configured: &str) -> Result<(TcpListener, String, String), HttpError> {
    let failed = |e: std::io::Error| HttpError::Auth(format!("Cannot wait for the sign-in: {e}"));
    if configured.is_empty() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.map_err(failed)?;
        let port = listener.local_addr().map_err(failed)?.port();
        return Ok((
            listener,
            format!("http://127.0.0.1:{port}/callback"),
            "/callback".into(),
        ));
    }
    let not_loopback = || {
        HttpError::Auth(
            "The redirect URL must be a loopback address such as \
             http://127.0.0.1:8765/callback, so Apix can receive the sign-in."
                .into(),
        )
    };
    let url = Url::parse(configured).map_err(|_| not_loopback())?;
    let ip = match url.host_str() {
        Some("127.0.0.1" | "localhost") => "127.0.0.1",
        Some("[::1]") => "::1",
        _ => return Err(not_loopback()),
    };
    if url.scheme() != "http" {
        return Err(not_loopback());
    }
    let listener = TcpListener::bind((ip, url.port().unwrap_or(80)))
        .await
        .map_err(failed)?;
    Ok((listener, configured.to_owned(), url.path().to_owned()))
}

/// Answers browser requests until the one carrying the code (or an error).
async fn wait_for_code(
    listener: &TcpListener,
    path: &str,
    state: &str,
) -> Result<String, HttpError> {
    let failed = |e: std::io::Error| HttpError::Auth(format!("Cannot read the sign-in: {e}"));
    loop {
        let (mut stream, _) = listener.accept().await.map_err(failed)?;
        let target = read_request_target(&mut stream).await.map_err(failed)?;
        let url = Url::parse(&format!("http://localhost{target}"))
            .unwrap_or_else(|_| Url::parse("http://localhost/").expect("a valid URL"));
        if url.path() != path {
            respond(&mut stream, "404 Not Found", "Not found.").await;
            continue;
        }
        let params: HashMap<String, String> = url.query_pairs().into_owned().collect();
        let result = if let Some(error) = params.get("error") {
            let detail = params
                .get("error_description")
                .map(|d| format!(" ({d})"))
                .unwrap_or_default();
            Err(HttpError::Auth(format!(
                "The sign-in failed: {error}{detail}"
            )))
        } else if params.get("state").map(String::as_str) != Some(state) {
            Err(HttpError::Auth(
                "The sign-in answer does not belong to this request.".into(),
            ))
        } else if let Some(code) = params.get("code") {
            Ok(code.clone())
        } else {
            Err(HttpError::Auth("The sign-in sent no code.".into()))
        };
        let page = if result.is_ok() {
            "Signed in. You can close this tab and go back to Apix."
        } else {
            "The sign-in did not work. Apix shows why."
        };
        respond(&mut stream, "200 OK", page).await;
        return result;
    }
}

/// The target of the request line (`/callback?code=…`).
async fn read_request_target(stream: &mut TcpStream) -> std::io::Result<String> {
    let mut head = Vec::new();
    let mut buf = [0u8; 2048];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") && head.len() < 16 * 1024 {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        head.extend_from_slice(&buf[..n]);
    }
    let head = String::from_utf8_lossy(&head);
    Ok(head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/")
        .to_owned())
}

async fn respond(stream: &mut TcpStream, status: &str, text: &str) {
    let body = format!(
        "<!doctype html><meta charset=utf-8><title>Apix</title>\
         <body style=\"font:16px system-ui;margin:4em;text-align:center\">{text}</body>"
    );
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// Opens `url` in the default browser.
fn open_browser(url: &str) -> Result<(), HttpError> {
    #[cfg(windows)]
    let mut command = {
        // Unlike `cmd /c start`, this passes `&` in the URL through untouched.
        let mut c = std::process::Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    command.spawn().map(|_| ()).map_err(|e| {
        HttpError::Auth(format!(
            "Cannot open the browser for the sign-in ({e}). Open this address yourself: {url}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_json_and_form_token_responses() {
        let json = response_fields(r#"{"access_token":"a","expires_in":3600,"ok":true}"#);
        assert_eq!(json["access_token"], "a");
        assert_eq!(json["expires_in"], "3600");
        assert!(!json.contains_key("ok"));
        let form = response_fields("access_token=b%2Bc&token_type=bearer");
        assert_eq!(form["access_token"], "b+c");
    }

    #[tokio::test]
    async fn the_loopback_redirect_carries_the_code() {
        let (listener, redirect, path) = listen("").await.unwrap();
        assert!(redirect.starts_with("http://127.0.0.1:") && redirect.ends_with("/callback"));
        let port = listener.local_addr().unwrap().port();
        let browser = tokio::spawn(async move {
            for target in ["/favicon.ico", "/callback?code=abc&state=s1"] {
                let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
                let request = format!("GET {target} HTTP/1.1\r\nHost: x\r\n\r\n");
                s.write_all(request.as_bytes()).await.unwrap();
                let mut answer = String::new();
                s.read_to_string(&mut answer).await.unwrap();
            }
        });
        assert_eq!(wait_for_code(&listener, &path, "s1").await.unwrap(), "abc");
        browser.await.unwrap();

        assert!(listen("https://example.com/cb").await.is_err());
    }
}
