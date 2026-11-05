//! The optional AI assist: writing tests for a response and explaining a
//! failed one, with the user's own API key for Claude (Anthropic), ChatGPT
//! (OpenAI) or Gemini (Google). Off unless the user turns it on and gives a
//! key. Each provider is called over plain HTTPS (Rust has no official SDK
//! for them).
//!
//! What is sent: the request's method, URL and headers (credential headers
//! masked), the status, headers and body of the response (cut at 12 KB).
//! The request is built and the answer read here; sending is the caller's.

use serde_json::{Value, json};

use crate::{KeyValue, Method, Request};

/// Body text sent at most.
const MAX_BODY: usize = 12 * 1024;
const ANTHROPIC_VERSION: &str = "2026-10-21";

/// Who answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Provider {
    /// Claude, through the Anthropic Messages API.
    #[default]
    Anthropic,
    /// ChatGPT models, through the OpenAI Chat Completions API.
    OpenAi,
    /// Gemini, through the Gemini API.
    Google,
}

impl Provider {
    pub const ALL: [Provider; 3] = [Provider::Anthropic, Provider::OpenAi, Provider::Google];

    /// The name kept in settings: `anthropic`, `openai`, `google`.
    pub fn id(self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::OpenAi => "openai",
            Provider::Google => "google",
        }
    }

    pub fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|p| p.id() == id.trim())
            .unwrap_or_default()
    }

    /// What people call it.
    pub fn name(self) -> &'static str {
        match self {
            Provider::Anthropic => "Claude (Anthropic)",
            Provider::OpenAi => "ChatGPT (OpenAI)",
            Provider::Google => "Gemini (Google)",
        }
    }

    /// The model used when none is picked.
    pub fn default_model(self) -> &'static str {
        match self {
            Provider::Anthropic => "claude-opus-5-5",
            Provider::OpenAi => "gpt-5.5",
            Provider::Google => "gemini-2.5-flash",
        }
    }

    /// The environment variable read when no key is kept.
    pub fn key_variable(self) -> &'static str {
        match self {
            Provider::Anthropic => "ANTHROPIC_API_KEY",
            Provider::OpenAi => "OPENAI_API_KEY",
            Provider::Google => "GEMINI_API_KEY",
        }
    }

    /// Where to get a key.
    pub fn key_page(self) -> &'static str {
        match self {
            Provider::Anthropic => "https://console.anthropic.com/settings/keys",
            Provider::OpenAi => "https://platform.openai.com/api-keys",
            Provider::Google => "https://aistudio.google.com/apikey",
        }
    }

    fn base(self) -> String {
        // APIX_AI_URL points every provider at a proxy or a test server.
        if let Ok(url) = std::env::var("APIX_AI_URL")
            && !url.trim().is_empty()
        {
            return url.trim_end_matches('/').to_owned();
        }
        match self {
            Provider::Anthropic => "https://api.anthropic.com/v1",
            Provider::OpenAi => "https://api.openai.com/v1",
            Provider::Google => "https://generativelanguage.googleapis.com/v1beta",
        }
        .to_owned()
    }

    fn key_account(self) -> String {
        format!("ai:{}-api-key", self.id())
    }

    fn auth(self, key: &str) -> KeyValue {
        match self {
            Provider::Anthropic => KeyValue::new("x-api-key", key),
            Provider::OpenAi => KeyValue::new("Authorization", format!("Bearer {key}")),
            Provider::Google => KeyValue::new("x-goog-api-key", key),
        }
    }
}

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

fn json_request(url: String, headers: Vec<KeyValue>, body: Value) -> Request {
    let mut request = Request::new(Method::Post, url);
    request.name = "AI assist".into();
    request.headers = headers;
    request
        .headers
        .push(KeyValue::new("content-type", "application/json"));
    request.body = crate::Body::Json(body.to_string());
    request.settings.timeout_ms = Some(300_000);
    request
}

/// The request to send to `provider`.
pub fn request(
    provider: Provider,
    task: Task,
    context: &Context,
    api_key: &str,
    model: &str,
    language: &str,
) -> Request {
    let model = if model.trim().is_empty() {
        provider.default_model()
    } else {
        model.trim()
    };
    let prompt = format!(
        "{}\n\n{} Write any prose in the language with code \"{}\"; keep code, field \
         names and text quoted from the exchange as they are.",
        context.describe(),
        instruction(task),
        if language.trim().is_empty() {
            "en"
        } else {
            language.trim()
        }
    );
    let base = provider.base();
    let auth = provider.auth(api_key);
    match provider {
        Provider::Anthropic => {
            let mut body = json!({
                "model": model,
                "max_tokens": 16000,
                "system": SYSTEM,
                "messages": [{ "role": "user", "content": prompt }],
            });
            let mut headers = vec![auth, KeyValue::new("anthropic-version", ANTHROPIC_VERSION)];
            // When a safety classifier declines, let the API retry on a model it routes to.
            if model == provider.default_model() {
                body["fallbacks"] = json!("default");
                headers.push(KeyValue::new(
                    "anthropic-beta",
                    "server-side-fallback-2026-07-01",
                ));
            }
            json_request(format!("{base}/messages"), headers, body)
        }
        Provider::OpenAi => json_request(
            format!("{base}/chat/completions"),
            vec![auth],
            json!({
                "model": model,
                "messages": [
                    { "role": "system", "content": SYSTEM },
                    { "role": "user", "content": prompt },
                ],
            }),
        ),
        Provider::Google => json_request(
            format!("{base}/models/{model}:generateContent"),
            vec![auth],
            json!({
                "systemInstruction": { "parts": [{ "text": SYSTEM }] },
                "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
            }),
        ),
    }
}

/// The answer's text, or why there is none.
pub fn answer(provider: Provider, status: u16, body: &[u8]) -> Result<String, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|_| format!("HTTP {status}: the answer is not JSON"))?;
    if let Some(error) = value.get("error").filter(|e| !e.is_null()) {
        let kind = error
            .get("type")
            .or_else(|| error.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("error");
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Err(match (status, kind) {
            (401 | 403, _)
            | (_, "authentication_error" | "invalid_api_key" | "PERMISSION_DENIED") => {
                format!("The {} API key was not accepted.", provider.name())
            }
            (429 | 529, _)
            | (_, "rate_limit_error" | "overloaded_error" | "RESOURCE_EXHAUSTED") => {
                format!(
                    "{} is busy or the quota is used up; try again later. {message}",
                    provider.name()
                )
            }
            _ => format!("{kind}: {message}"),
        });
    }
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {status}"));
    }
    let declined = || format!("{} declined to answer this one.", provider.name());
    let text = match provider {
        Provider::Anthropic => {
            if value.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
                return Err(declined());
            }
            value
                .get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        }
        Provider::OpenAi => {
            let message = &value["choices"][0]["message"];
            if message
                .get("refusal")
                .and_then(Value::as_str)
                .is_some_and(|r| !r.is_empty())
            {
                return Err(declined());
            }
            message["content"].as_str().unwrap_or_default().to_owned()
        }
        Provider::Google => {
            let candidate = &value["candidates"][0];
            if matches!(
                candidate["finishReason"].as_str(),
                Some("SAFETY" | "PROHIBITED_CONTENT" | "BLOCKLIST")
            ) || value["promptFeedback"]["blockReason"].is_string()
            {
                return Err(declined());
            }
            candidate["content"]["parts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|p| p["thought"].as_bool() != Some(true))
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .join("")
        }
    };
    let text = text.trim().to_owned();
    if text.is_empty() {
        return Err("The answer was empty.".to_owned());
    }
    Ok(text)
}

/// The request that lists `provider`'s models, for picking one.
pub fn models_request(provider: Provider, api_key: &str) -> Request {
    let mut request = Request::new(Method::Get, format!("{}/models", provider.base()));
    request.name = "AI models".into();
    request.headers = vec![provider.auth(api_key)];
    if provider == Provider::Anthropic {
        request
            .headers
            .push(KeyValue::new("anthropic-version", ANTHROPIC_VERSION));
    }
    request.settings.timeout_ms = Some(30_000);
    request
}

/// The model ids in the answer to [`models_request`] that can write text,
/// newest names first as the provider lists them.
pub fn models(provider: Provider, status: u16, body: &[u8]) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|_| format!("HTTP {status}: the answer is not JSON"))?;
    if !(200..300).contains(&status) {
        let message = value["error"]["message"].as_str().unwrap_or_default();
        return Err(format!("HTTP {status} {message}").trim().to_owned());
    }
    let ids: Vec<String> = match provider {
        Provider::Anthropic => value["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| m["id"].as_str().map(str::to_owned))
            .collect(),
        Provider::OpenAi => value["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| m["id"].as_str())
            // Chat models only: no embeddings, audio, images or moderation.
            .filter(|id| {
                (id.starts_with("gpt-") || id.starts_with('o') || id.starts_with("chatgpt"))
                    && ![
                        "audio",
                        "realtime",
                        "tts",
                        "transcribe",
                        "image",
                        "embedding",
                        "search",
                    ]
                    .iter()
                    .any(|bad| id.contains(bad))
            })
            .map(str::to_owned)
            .collect(),
        Provider::Google => value["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| {
                m["supportedGenerationMethods"]
                    .as_array()
                    .is_some_and(|ms| ms.iter().any(|x| x == "generateContent"))
            })
            .filter_map(|m| m["name"].as_str())
            .map(|name| name.trim_start_matches("models/").to_owned())
            .collect(),
    };
    Ok(ids)
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

/// Keeps `provider`'s API key in the system keychain (never a file); an
/// empty key removes it.
pub fn save_key(provider: Provider, key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        apix_vault::keychain::forget_text(&provider.key_account())
    } else {
        apix_vault::keychain::save_text(&provider.key_account(), key)
    }
}

/// `provider`'s kept API key, or its environment variable.
pub fn load_key(provider: Provider) -> Option<String> {
    apix_vault::keychain::load_text(&provider.key_account())
        .ok()
        .flatten()
        .or_else(|| std::env::var(provider.key_variable()).ok())
        .or_else(|| {
            (provider == Provider::Google)
                .then(|| std::env::var("GOOGLE_API_KEY").ok())
                .flatten()
        })
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

    fn body(r: &Request) -> Value {
        let crate::Body::Json(text) = &r.body else {
            panic!()
        };
        serde_json::from_str(text).unwrap()
    }

    fn header<'a>(r: &'a Request, name: &str) -> Option<&'a str> {
        r.headers
            .iter()
            .find(|h| h.key.eq_ignore_ascii_case(name))
            .map(|h| h.value.as_str())
    }

    #[test]
    fn requests_for_each_provider() {
        let r = request(
            Provider::Anthropic,
            Task::Explain,
            &context(),
            "sk-test",
            "",
            "tr",
        );
        assert!(r.url.ends_with("/v1/messages"));
        assert_eq!(header(&r, "x-api-key"), Some("sk-test"));
        assert!(header(&r, "anthropic-version").is_some());
        let b = body(&r);
        assert_eq!(b["model"], "claude-opus-5-5");
        assert_eq!(b["fallbacks"], "default");
        let prompt = b["messages"][0]["content"].as_str().unwrap();
        assert!(
            !prompt.contains("s3cret") && !prompt.contains("k3y") && !prompt.contains("sid=abc"),
            "{prompt}"
        );
        assert!(prompt.contains("Accept: application/json") && prompt.contains("status 404"));
        assert!(prompt.contains("did not succeed") && prompt.contains("code \"tr\""));
        let other = request(
            Provider::Anthropic,
            Task::WriteTests,
            &context(),
            "k",
            "claude-sonnet-5-5",
            "",
        );
        assert!(body(&other).get("fallbacks").is_none());

        let o = request(
            Provider::OpenAi,
            Task::WriteTests,
            &context(),
            "sk-o",
            "",
            "en",
        );
        assert!(o.url.ends_with("/v1/chat/completions"));
        assert_eq!(header(&o, "authorization"), Some("Bearer sk-o"));
        let b = body(&o);
        assert_eq!(b["model"], "gpt-5.5");
        assert_eq!(b["messages"][0]["role"], "system");
        assert!(
            b["messages"][1]["content"]
                .as_str()
                .unwrap()
                .contains("pm.test")
        );

        let g = request(
            Provider::Google,
            Task::Explain,
            &context(),
            "AIza",
            "gemini-x",
            "en",
        );
        assert!(
            g.url.ends_with("/v1beta/models/gemini-x:generateContent"),
            "{}",
            g.url
        );
        assert_eq!(header(&g, "x-goog-api-key"), Some("AIza"));
        let b = body(&g);
        assert!(
            b["systemInstruction"]["parts"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Apix")
        );
        assert!(
            b["contents"][0]["parts"][0]["text"]
                .as_str()
                .unwrap()
                .contains("did not succeed")
        );
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
    fn answers_from_each_provider() {
        let a = Provider::Anthropic;
        let ok = br#"{"type":"message","stop_reason":"end_turn","content":[{"type":"thinking","thinking":""},{"type":"text","text":"Here:\n```javascript\npm.test(\"ok\", () => {});\n```"}]}"#;
        assert_eq!(
            script(&answer(a, 200, ok).unwrap()),
            "pm.test(\"ok\", () => {});"
        );
        assert_eq!(script("pm.test(1)"), "pm.test(1)");
        assert!(
            answer(a, 200, br#"{"stop_reason":"refusal","content":[]}"#)
                .unwrap_err()
                .contains("declined")
        );
        let bad_key = br#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
        assert_eq!(
            answer(a, 401, bad_key).unwrap_err(),
            "The Claude (Anthropic) API key was not accepted."
        );
        let busy =
            br#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#;
        assert!(answer(a, 529, busy).unwrap_err().contains("busy"));
        assert!(answer(a, 502, b"<html>").is_err());
        assert!(
            answer(a, 200, br#"{"content":[]}"#)
                .unwrap_err()
                .contains("empty")
        );

        let o = Provider::OpenAi;
        let ok = br#"{"choices":[{"message":{"role":"assistant","content":"It failed.","refusal":null},"finish_reason":"stop"}]}"#;
        assert_eq!(answer(o, 200, ok).unwrap(), "It failed.");
        let refused = br#"{"choices":[{"message":{"content":null,"refusal":"I can't help"}}]}"#;
        assert!(answer(o, 200, refused).unwrap_err().contains("ChatGPT"));
        let bad = br#"{"error":{"message":"Incorrect API key provided","type":"invalid_request_error","code":"invalid_api_key"}}"#;
        assert!(answer(o, 401, bad).unwrap_err().contains("not accepted"));

        let g = Provider::Google;
        let ok = br#"{"candidates":[{"content":{"parts":[{"text":"thinking","thought":true},{"text":"Because "},{"text":"404."}],"role":"model"},"finishReason":"STOP"}]}"#;
        assert_eq!(answer(g, 200, ok).unwrap(), "Because 404.");
        let blocked = br#"{"candidates":[{"finishReason":"SAFETY"}]}"#;
        assert!(answer(g, 200, blocked).unwrap_err().contains("Gemini"));
        let quota =
            br#"{"error":{"code":429,"message":"Quota exceeded","status":"RESOURCE_EXHAUSTED"}}"#;
        assert!(answer(g, 429, quota).unwrap_err().contains("quota"));
    }

    #[test]
    fn model_lists() {
        let a = br#"{"data":[{"id":"claude-opus-5-5","type":"model"},{"id":"claude-haiku-5-5","type":"model"}]}"#;
        assert_eq!(
            models(Provider::Anthropic, 200, a).unwrap(),
            ["claude-opus-5-5", "claude-haiku-5-5"]
        );
        let o = br#"{"data":[{"id":"gpt-5.5"},{"id":"text-embedding-3-large"},{"id":"gpt-4o-audio-preview"},{"id":"o4-mini"},{"id":"dall-e-3"}]}"#;
        assert_eq!(
            models(Provider::OpenAi, 200, o).unwrap(),
            ["gpt-5.5", "o4-mini"]
        );
        let g = br#"{"models":[{"name":"models/gemini-2.5-flash","supportedGenerationMethods":["generateContent","countTokens"]},{"name":"models/text-embedding-004","supportedGenerationMethods":["embedContent"]}]}"#;
        assert_eq!(
            models(Provider::Google, 200, g).unwrap(),
            ["gemini-2.5-flash"]
        );
        assert!(
            models(Provider::OpenAi, 401, br#"{"error":{"message":"bad key"}}"#)
                .unwrap_err()
                .contains("bad key")
        );
        let r = models_request(Provider::Google, "AIza");
        assert!(r.url.ends_with("/v1beta/models"));
        assert_eq!(Provider::from_id("openai"), Provider::OpenAi);
        assert_eq!(Provider::from_id("nonsense"), Provider::Anthropic);
    }
}
