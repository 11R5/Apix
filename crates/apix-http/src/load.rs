//! One request sent many times, for load tests. Unlike [`HttpClient::send`],
//! which opens a fresh connection for every send so its network trace shows
//! the whole handshake, a [`Load`] keeps one client whose connections stay
//! open and are reused, the way real clients behave under load, and records
//! nothing but the status, the time and the size.
//!
//! [`HttpClient::send`]: crate::HttpClient::send

use std::time::{Duration, Instant};

use apix_core::{Auth, ProxyChoice, Request};

use crate::{HttpError, oauth2, prepare, tls};

/// What one send gave.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// `None` when no answer came (refused, timed out, reset).
    pub status: Option<u16>,
    /// From sending to the last byte of the body.
    pub latency: Duration,
    /// The body's size as it came over the wire.
    pub bytes: u64,
    pub error: Option<String>,
}

/// A request ready to be sent again and again.
pub struct Load {
    client: reqwest::Client,
    request: Request,
}

impl Load {
    /// Builds the client for `request` (already resolved: no `{{variables}}`
    /// left). OAuth 2.0 is turned into its bearer token once, here. Digest
    /// is not supported: every send would need its own challenge.
    pub async fn new(request: &Request) -> Result<Self, HttpError> {
        let mut request = request.clone();
        match &request.auth {
            Auth::OAuth2(config) => {
                let token = oauth2::access_token(config).await?;
                request.auth = Auth::Bearer { token };
            }
            Auth::Digest { .. } => {
                return Err(HttpError::Settings(
                    "Digest auth is not supported in load tests".into(),
                ));
            }
            _ => {}
        }
        let eff = request.settings.effective();
        let tls = tls::client_config(&eff, &[b"h2", b"http/1.1"])?;
        let policy = if eff.follow_redirects {
            reqwest::redirect::Policy::limited(eff.max_redirects as usize)
        } else {
            reqwest::redirect::Policy::none()
        };
        let mut builder = reqwest::Client::builder()
            .user_agent(crate::USER_AGENT)
            .use_preconfigured_tls(tls)
            .redirect(policy)
            // Bodies are counted, not read: no decompression.
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .pool_max_idle_per_host(usize::MAX)
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_nodelay(true);
        if let Some(timeout) = eff.timeout {
            builder = builder.timeout(timeout);
        }
        match &eff.proxy {
            ProxyChoice::Environment => {}
            ProxyChoice::Off => builder = builder.no_proxy(),
            ProxyChoice::Url(url) => {
                let proxy = reqwest::Proxy::all(url)
                    .map_err(|e| HttpError::Settings(format!("invalid proxy `{url}`: {e}")))?;
                builder = builder.proxy(proxy);
            }
        }
        let client = builder.build()?;
        // Fail now, not on every send, when the request cannot be built.
        prepare(&client, &request)?;
        Ok(Self { client, request })
    }

    /// Sends the request once.
    pub async fn hit(&self) -> Hit {
        let started = Instant::now();
        let failed = |error: String| Hit {
            status: None,
            latency: started.elapsed(),
            bytes: 0,
            error: Some(error),
        };
        let prepared = match prepare(&self.client, &self.request) {
            Ok(r) => r,
            Err(e) => return failed(e.to_string()),
        };
        let mut response = match self.client.execute(prepared).await {
            Ok(r) => r,
            Err(e) => return failed(describe(&e)),
        };
        let status = response.status().as_u16();
        let mut bytes = 0u64;
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => bytes += chunk.len() as u64,
                Ok(None) => break,
                Err(e) => {
                    return Hit {
                        status: Some(status),
                        latency: started.elapsed(),
                        bytes,
                        error: Some(describe(&e)),
                    };
                }
            }
        }
        Hit {
            status: Some(status),
            latency: started.elapsed(),
            bytes,
            error: None,
        }
    }
}

/// A short reason, the same for every failure of a kind, so they can be counted.
fn describe(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "timed out".into()
    } else if e.is_connect() {
        "could not connect".into()
    } else if e.is_redirect() {
        "too many redirects".into()
    } else if e.is_body() || e.is_decode() {
        "the body was cut short".into()
    } else {
        let mut text = e.to_string();
        let mut source = std::error::Error::source(e);
        while let Some(inner) = source {
            text = inner.to_string();
            source = inner.source();
        }
        text
    }
}
