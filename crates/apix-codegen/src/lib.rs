//! Requests as shell commands, both ways:
//!
//! - [`to_shell`] writes a request as a `curl` command for bash/zsh/sh, a
//!   `curl.exe` command for Windows `cmd`, or a PowerShell
//!   `Invoke-RestMethod` call. Each one sends the same request the Apix engine
//!   would send.
//! - [`from_curl`] reads a pasted `curl` command (bash or cmd style, as
//!   browsers' "Copy as cURL" produce them) back into a request.

mod curl_import;
mod shell;

pub use curl_import::{CurlImport, from_curl, looks_like_curl};
pub use shell::{Shell, to_shell};

use apix_core::{ApiKeyLocation, Auth, Body, FormPart, KeyValue, OAuth1, Request, sign};

/// The URL with enabled query rows (and a query API key) appended, encoded
/// the same way the HTTP engine encodes them.
fn full_url(request: &Request) -> String {
    let mut pairs: Vec<(&str, &str)> = request
        .query
        .iter()
        .filter(|q| q.enabled)
        .map(|q| (q.key.as_str(), q.value.as_str()))
        .collect();
    if let Auth::ApiKey {
        key,
        value,
        location: ApiKeyLocation::Query,
    } = &request.auth
    {
        pairs.push((key, value));
    }
    let base = if pairs.is_empty() {
        request.url.clone()
    } else {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(pairs)
            .finish();
        let separator = if request.url.contains('?') { '&' } else { '?' };
        format!("{}{separator}{query}", request.url)
    };
    // OAuth 1.0 in the URL signs the URL built so far.
    if let Auth::OAuth1(o) = &request.auth
        && o.location == ApiKeyLocation::Query
        && let Some(mut url) = signable_url(request, &base)
    {
        let params = oauth1_params(request, o, &url);
        url.query_pairs_mut().extend_pairs(params);
        return url.into();
    }
    base
}

/// The URL to sign, unless the request still holds `{{variables}}`.
fn signable_url(request: &Request, url: &str) -> Option<url::Url> {
    let unresolved = match &request.auth {
        Auth::OAuth1(o) => [
            &o.consumer_key,
            &o.consumer_secret,
            &o.token,
            &o.token_secret,
        ]
        .iter()
        .any(|v| v.contains("{{")),
        Auth::AwsSigV4(a) => [&a.access_key, &a.secret_key, &a.region, &a.service]
            .iter()
            .any(|v| v.contains("{{")),
        _ => false,
    };
    if unresolved || url.contains("{{") {
        return None;
    }
    url::Url::parse(url).ok()
}

fn oauth1_params(request: &Request, o: &OAuth1, url: &url::Url) -> Vec<(String, String)> {
    let form: Vec<(String, String)> = form_fields(&request.body)
        .unwrap_or_default()
        .into_iter()
        .map(|f| (f.key.clone(), f.value.clone()))
        .collect();
    sign::oauth1_params(
        o,
        request.method.as_str(),
        url,
        &form,
        sign::unix_now(),
        &sign::random_token(16),
    )
}

/// Headers as the engine sends them: enabled rows, auth headers, and the
/// body's default Content-Type unless one was set. Signatures (OAuth 1.0,
/// AWS) are computed now; `curl_signs_aws` leaves AWS to curl's `--aws-sigv4`.
fn effective_headers(request: &Request, curl_signs_aws: bool) -> Vec<(String, String)> {
    let mut headers: Vec<(String, String)> = request
        .headers
        .iter()
        .filter(|h| h.enabled && !h.key.is_empty())
        .map(|h| (h.key.clone(), h.value.clone()))
        .collect();
    let default_content_type = request
        .body
        .default_content_type()
        .filter(|_| !request.has_header("content-type"));
    let content_type = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.as_str())
        .or(default_content_type)
        .map(str::to_owned);
    let authorization = |value: String| ("Authorization".to_owned(), value);
    match &request.auth {
        Auth::Bearer { token } => headers.push(authorization(format!("Bearer {token}"))),
        Auth::ApiKey {
            key,
            value,
            location: ApiKeyLocation::Header,
        } => headers.push((key.clone(), value.clone())),
        // The desktop app passes a bearer token instead when it has one.
        Auth::OAuth2(_) => headers.push(authorization("Bearer <access token>".into())),
        Auth::OAuth1(o) if o.location == ApiKeyLocation::Header => {
            headers.push(authorization(
                match signable_url(request, &full_url(request)) {
                    Some(url) => sign::oauth1_header(&oauth1_params(request, o, &url), &o.realm),
                    None => "<OAuth 1.0 signature: copy with variables resolved>".into(),
                },
            ));
        }
        Auth::AwsSigV4(aws) if curl_signs_aws => {
            if !aws.session_token.is_empty() {
                headers.push(("X-Amz-Security-Token".into(), aws.session_token.clone()));
            }
        }
        Auth::AwsSigV4(aws) => match signable_url(request, &full_url(request)) {
            Some(url) => {
                let payload_hash = match text_body(&request.body) {
                    Some(text) => sign::sha256_hex(text.as_bytes()),
                    None => match form_fields(&request.body) {
                        Some(fields) => sign::sha256_hex(
                            url::form_urlencoded::Serializer::new(String::new())
                                .extend_pairs(fields.iter().map(|f| (&f.key, &f.value)))
                                .finish()
                                .as_bytes(),
                        ),
                        None if request.body.is_none() => sign::sha256_hex(b""),
                        None => sign::UNSIGNED_PAYLOAD.into(),
                    },
                };
                headers.extend(sign::aws_sigv4(
                    aws,
                    request.method.as_str(),
                    &url,
                    content_type.as_deref(),
                    &payload_hash,
                    sign::unix_now(),
                ));
            }
            None => headers.push(authorization(
                "<AWS signature: copy with variables resolved>".into(),
            )),
        },
        _ => {}
    }
    if let Some(content_type) = default_content_type {
        headers.push(("Content-Type".into(), content_type.into()));
    }
    headers
}

fn form_fields(body: &Body) -> Option<Vec<&KeyValue>> {
    match body {
        Body::Form(rows) => Some(
            rows.iter()
                .filter(|r| r.enabled && !r.key.is_empty())
                .collect(),
        ),
        _ => None,
    }
}

/// The body as text, for bodies sent as text (GraphQL becomes its JSON).
fn text_body(body: &Body) -> Option<String> {
    match body {
        Body::Json(t) | Body::Text(t) | Body::Xml(t) => Some(t.clone()),
        // Invalid variables: show the query alone rather than nothing.
        Body::GraphQl(gql) => Some(Body::graphql_json(gql).unwrap_or_else(|_| gql.query.clone())),
        _ => None,
    }
}

/// Enabled multipart fields that have a name.
fn multipart_parts(body: &Body) -> Option<Vec<&FormPart>> {
    match body {
        Body::Multipart(parts) => Some(
            parts
                .iter()
                .filter(|p| p.enabled && !p.key.is_empty())
                .collect(),
        ),
        _ => None,
    }
}
