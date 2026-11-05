//! The optional AI assist: writing tests for a response and explaining a
//! failed one, with Claude through the Anthropic Messages API and the
//! user's own API key. Off unless the user turns it on and gives a key.
//!
//! What is sent: the request's method, URL and headers (credential headers
//! masked), the status, headers and body of the response (cut at 12 KB).
//! The request is built and the answer read here; sending is the caller's.

use serde_json::{Value, json};

use crate::{KeyValue, Method, Request};

pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2026-10-21";
/// Body text sent at most.
const MAX_BODY: usize = 12 * 1024;

/// What to ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    /// A Tests script (`pm.test` …) for this response.
    WriteTests,
    /// Why this response failed and what to try.
    Explain,
}

/// The exchange to ask about.
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub method: String,
    pub url: String,
    pub request_headers: Vec<(String, String)>,
    pub request_body: String,
    pub status: u16,
    pub response_headers: Vec<(String, String)>,
    pub response_body: String,
    pub time_ms: f64,
}

/// Headers whose values are never sent.
fn secret(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n == "authorization"
        || n == "proxy-authorization"
        || n == "cookie"
        || n == "set-cookie"
        || n.contains("api-key")
        || n.contains("apikey")
        || n.contains("token")
        || n.contains("secret")
        || n.contains("signature")
}

fn cut(text: &str) -> String {
    if text.len() <= MAX_BODY {
        return text.to_owned();
    }
    let mut end = MAX_BODY;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n… ({} more bytes not shown)",
        &text[..end],
        text.len() - end
    )
}

impl Context {
    /// The exchange as text for the prompt, credentials masked.
    pub fn describe(&self) -> String {
        let headers = |list: &[(String, String)]| {
            list.iter()
                .map(|(k, v)| {
                    let v = if secret(k) { "•••" } else { v.as_str() };
                    format!("{k}: {v}")
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let mut out = format!(
            "<request>\n{} {}\n{}\n",
            self.method,
            self.url,
            headers(&self.request_headers)
        );
        if !self.request_body.trim().is_empty() {
            out += &format!("\n{}\n", cut(&self.request_body));
        }
        out += &format!(
            "</request>\n<response>\nstatus {} · {:.0} ms\n{}\n\n{}\n</response>",
            self.status,
            self.time_ms,
            headers(&self.response_headers),
            cut(&self.response_body)
        );
        out
    }
}

const SYSTEM: &str = "You help developers test HTTP APIs inside Apix, an API client. \
Scripts run in Apix's Postman-compatible sandbox: pm.test(name, fn), pm.expect (Chai style), \
pm.response.code, pm.response.json(), pm.response.text(), pm.response.headers.get(name), \
pm.response.responseTime, pm.response.to.have.status(n), pm.response.to.have.header(name), \
pm.response.to.have.jsonSchema(schema). Credential values are masked as •••.";

fn instruction(task: Task) -> &'static str {
    match task {
        Task::WriteTests => {
            "Write a Tests script for this request that checks what a client relies on: the \
             status, the content type, the shape and types of the body's important fields, and \
             values that should hold for any correct answer (not values that change, such as \
             dates or ids). Keep it to a few focused pm.test blocks. Reply with only the \
             JavaScript in one ```javascript block."
        }
        Task::Explain => {
            "This request did not succeed. Explain in a few short paragraphs, plainly, what the \
             response says went wrong and the most likely causes on the client side, then list \
             concrete things to check or change in the request. Do not invent details that are \
             not in the exchange."
        }
    }
}

/// The request to send to the Messages API. `APIX_AI_URL` points it
/// elsewhere (a proxy, a test server).
pub fn request(
    task: Task,
    context: &Context,
    api_key: &str,
    model: &str,
    language: &str,
) -> Request {
    let url = std::env::var("APIX_AI_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| MESSAGES_URL.to_owned());
    let model = if model.trim().is_empty() {
        DEFAULT_MODEL
    } else {
        model.trim()
    };
    let mut body = json!({
        "model": model,
        "max_tokens": 16000,
        "system": SYSTEM,
        "messages": [{
            "role": "user",
            "content": format!(
                "{}\n\n{} Write any prose in the language with code \"{}\"; keep code, field \
                 names and text quoted from the exchange as they are.",
                context.describe(),
                instruction(task),
                if language.trim().is_empty() { "en" } else { language.trim() }
            ),
        }],
    });
    let mut headers = vec![
        KeyValue::new("x-api-key", api_key),
        KeyValue::new("anthropic-version", API_VERSION),
        KeyValue::new("content-type", "application/json"),
    ];
    // When a safety classifier declines, let the API retry on a model it routes to.
    if model == DEFAULT_MODEL {
        body["fallbacks"] = json!("default");
        headers.push(KeyValue::new(
            "anthropic-beta",
            "server-side-fallback-2026-07-01",
        ));
    }
    let mut request = Request::new(Method::Post, url);
    request.name = "AI assist".into();
    request.headers = headers;
    request.body = crate::Body::Json(body.to_string());
    request.settings.timeout_ms = Some(300_000);
    request
}

/// The answer's text, or why there is none.
pub fn answer(status: u16, body: &[u8]) -> Result<String, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|_| format!("HTTP {status}: the answer is not JSON"))?;
    if let Some(error) = value.get("error") {
        let kind = error.get("type").and_then(Value::as_str).unwrap_or("error");
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Err(match kind {
            "authentication_error" => "The API key was not accepted.".to_owned(),
            "rate_limit_error" | "overloaded_error" => {
                format!("The API is busy ({kind}); try again in a moment.")
            }
            _ => format!("{kind}: {message}"),
        });
    }
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {status}"));
    }
    if value.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
        return Err("Claude declined to answer this one.".to_owned());
    }
    let text: Vec<&str> = value
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect();
    let text = text.join("").trim().to_owned();
    if text.is_empty() {
        return Err("The answer was empty.".to_owned());
    }
    Ok(text)
}

/// The script inside the answer's first code block, or the whole answer.
pub fn script(answer: &str) -> String {
    if let Some(start) = answer.find("```") {
        let rest = &answer[start + 3..];
        let rest = rest.split_once('\n').map_or(rest, |(_, after)| after);
        if let Some(end) = rest.find("```") {
            return rest[..end].trim().to_owned();
        }
    }
    answer.trim().to_owned()
}

/// Where the API key is kept: the system keychain, never a file.
const KEY_ACCOUNT: &str = "ai:anthropic-api-key";

/// Keeps the API key in the system keychain; an empty key removes it.
pub fn save_key(key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        apix_vault::keychain::forget_text(KEY_ACCOUNT)
    } else {
        apix_vault::keychain::save_text(KEY_ACCOUNT, key)
    }
}

/// The kept API key; `ANTHROPIC_API_KEY` when none is kept.
pub fn load_key() -> Option<String> {
    apix_vault::keychain::load_text(KEY_ACCOUNT)
        .ok()
        .flatten()
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
        .filter(|k| !k.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> Context {
        Context {
            method: "GET".into(),
            url: "https://api.test/users/1".into(),
            request_headers: vec![
                ("Authorization".into(), "Bearer s3cret".into()),
                ("X-Api-Key".into(), "k3y".into()),
                ("Accept".into(), "application/json".into()),
            ],
            status: 404,
            response_headers: vec![("Set-Cookie".into(), "sid=abc".into())],
            response_body: "{\"error\":\"not found\"}".into(),
            ..Default::default()
        }
    }

    #[test]
    fn the_request_masks_credentials() {
        let r = request(Task::Explain, &context(), "sk-test", "", "tr");
        assert_eq!(r.method, Method::Post);
        let crate::Body::Json(body) = &r.body else {
            panic!()
        };
        let body: Value = serde_json::from_str(body).unwrap();
        assert_eq!(body["model"], DEFAULT_MODEL);
        assert_eq!(body["fallbacks"], "default");
        let prompt = body["messages"][0]["content"].as_str().unwrap();
        assert!(
            !prompt.contains("s3cret") && !prompt.contains("k3y") && !prompt.contains("sid=abc"),
            "{prompt}"
        );
        assert!(prompt.contains("Accept: application/json") && prompt.contains("status 404"));
        assert!(prompt.contains("did not succeed") && prompt.contains("code \"tr\""));
        assert!(
            r.headers
                .iter()
                .any(|h| h.key == "x-api-key" && h.value == "sk-test")
        );
        assert!(r.headers.iter().any(|h| h.key == "anthropic-version"));
        // Another model: no fallback setting.
        let other = request(Task::WriteTests, &context(), "k", "claude-sonnet-5-5", "");
        let crate::Body::Json(body) = &other.body else {
            panic!()
        };
        assert!(!body.contains("fallbacks"));
        assert!(body.contains("pm.test"));
    }

    #[test]
    fn long_bodies_are_cut() {
        let mut c = context();
        c.response_body = "x".repeat(20_000);
        let text = c.describe();
        assert!(text.contains("more bytes not shown"));
        assert!(text.len() < 14_000);
    }

    #[test]
    fn answers() {
        let ok = br#"{"type":"message","stop_reason":"end_turn","content":[{"type":"thinking","thinking":""},{"type":"text","text":"Here:\n```javascript\npm.test(\"ok\", () => {});\n```"}]}"#;
        let text = answer(200, ok).unwrap();
        assert_eq!(script(&text), "pm.test(\"ok\", () => {});");
        assert_eq!(script("pm.test(1)"), "pm.test(1)");
        let refused = br#"{"stop_reason":"refusal","content":[]}"#;
        assert!(answer(200, refused).unwrap_err().contains("declined"));
        let bad_key = br#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
        assert_eq!(
            answer(401, bad_key).unwrap_err(),
            "The API key was not accepted."
        );
        let busy =
            br#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#;
        assert!(answer(529, busy).unwrap_err().contains("busy"));
        assert!(answer(502, b"<html>").is_err());
        assert!(
            answer(200, br#"{"content":[]}"#)
                .unwrap_err()
                .contains("empty")
        );
    }
}
