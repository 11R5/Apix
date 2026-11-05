//! An auth type: signs each request with HMAC-SHA256 over its method, path
//! and a hash of its body, as many partner APIs ask:
//!
//! ```text
//! <header>: keyId="<key id>",algorithm="hmac-sha256",signature="<base64>"
//! string to sign: METHOD \n /path?query \n hex(sha256(body))
//! ```

use apix_plugin_sdk::serde_json::{Value, json};
use apix_plugin_sdk::{Plugin, export};
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

struct Signature;

/// `/path?query` of a URL.
fn path_and_query(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    match rest.find('/') {
        Some(i) => rest[i..].split('#').next().unwrap_or("/"),
        None => "/",
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sign(method: &str, url: &str, body: &str, secret: &str) -> (String, String) {
    let to_sign = format!(
        "{}\n{}\n{}",
        method.to_ascii_uppercase(),
        path_and_query(url),
        hex(&Sha256::digest(body.as_bytes()))
    );
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("any key length");
    mac.update(to_sign.as_bytes());
    let signature = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
    (to_sign, signature)
}

impl Plugin for Signature {
    fn manifest() -> Value {
        json!({
            "name": "hmac-signature",
            "version": "1.0.0",
            "description": "Signs requests with HMAC-SHA256 over the method, path and body hash.",
            "auth": {
                "label": "HMAC signature",
                "fields": [
                    { "name": "key_id", "label": "Key id" },
                    { "name": "secret", "label": "Secret", "secret": true },
                    { "name": "header", "label": "Header (Signature)" }
                ]
            }
        })
    }

    fn call(hook: &str, input: Value) -> Result<Value, String> {
        if hook != "auth" {
            return Err(format!("hmac-signature has no {hook} hook"));
        }
        let text = |v: &Value| v.as_str().unwrap_or_default().to_owned();
        let config = &input["config"];
        let secret = text(&config["secret"]);
        if secret.is_empty() {
            return Err("the secret is empty".into());
        }
        let header = match text(&config["header"]) {
            h if h.trim().is_empty() => "Signature".to_owned(),
            h => h.trim().to_owned(),
        };
        let (_, signature) = sign(
            &text(&input["method"]),
            &text(&input["url"]),
            &text(&input["body"]),
            &secret,
        );
        Ok(json!({
            "headers": [[header, format!(
                "keyId=\"{}\",algorithm=\"hmac-sha256\",signature=\"{signature}\"",
                text(&config["key_id"])
            )]]
        }))
    }
}

export!(Signature);
