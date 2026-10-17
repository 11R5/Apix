use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    #[default]
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl Method {
    pub const ALL: [Method; 7] = [
        Method::Get,
        Method::Post,
        Method::Put,
        Method::Patch,
        Method::Delete,
        Method::Head,
        Method::Options,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
            Method::Head => "HEAD",
            Method::Options => "OPTIONS",
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Method {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Method::ALL
            .into_iter()
            .find(|m| m.as_str().eq_ignore_ascii_case(s))
            .ok_or_else(|| format!("unknown HTTP method `{s}`"))
    }
}

/// A header, query parameter or form field. Disabled rows stay in the file
/// (like Postman's checkbox) but are never sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
}

impl KeyValue {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            enabled: true,
        }
    }
}

fn default_true() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Body {
    #[default]
    None,
    Json(String),
    Text(String),
    Xml(String),
    Form(Vec<KeyValue>),
}

impl Body {
    pub fn is_none(&self) -> bool {
        matches!(self, Body::None)
    }

    /// The `Content-Type` sent when the user has not set one explicitly.
    pub fn default_content_type(&self) -> Option<&'static str> {
        match self {
            Body::None => None,
            Body::Json(_) => Some("application/json"),
            Body::Text(_) => Some("text/plain; charset=utf-8"),
            Body::Xml(_) => Some("application/xml"),
            Body::Form(_) => Some("application/x-www-form-urlencoded"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyLocation {
    #[default]
    Header,
    Query,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Auth {
    /// Use the auth of the nearest folder, collection or project that sets
    /// one (Postman's "Inherit auth from parent"). Not written to files.
    #[default]
    Inherit,
    /// Send no auth, even if a parent sets one. Written as `auth = "none"`.
    None,
    Bearer {
        token: String,
    },
    Basic {
        username: String,
        #[serde(default)]
        password: String,
    },
    ApiKey {
        key: String,
        value: String,
        #[serde(default)]
        location: ApiKeyLocation,
    },
}

impl Auth {
    /// Whether nothing is sent: `None`, or `Inherit` left unresolved.
    pub fn is_none(&self) -> bool {
        matches!(self, Auth::None | Auth::Inherit)
    }

    pub fn is_inherit(&self) -> bool {
        matches!(self, Auth::Inherit)
    }

    /// A short name for the UI, e.g. "Bearer token".
    pub fn label(&self) -> &'static str {
        match self {
            Auth::Inherit => "Inherit",
            Auth::None => "No auth",
            Auth::Bearer { .. } => "Bearer token",
            Auth::Basic { .. } => "Basic auth",
            Auth::ApiKey { .. } => "API key",
        }
    }
}

/// One request, stored as one human-readable TOML file.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Request {
    pub name: String,
    /// Markdown notes shown with the request (Postman's description).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default)]
    pub method: Method,
    pub url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query: Vec<KeyValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<KeyValue>,
    #[serde(default, skip_serializing_if = "Body::is_none")]
    pub body: Body,
    #[serde(default, skip_serializing_if = "Auth::is_inherit")]
    pub auth: Auth,
}

impl Request {
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            ..Default::default()
        }
    }

    pub fn has_header(&self, name: &str) -> bool {
        self.headers
            .iter()
            .any(|h| h.enabled && h.key.eq_ignore_ascii_case(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
name = "Create user"
method = "POST"
url = "{{base_url}}/users"

[[headers]]
key = "X-Trace"
value = "on"
enabled = false

[body]
json = '{ "name": "Ada" }'

[auth.bearer]
token = "{{token}}"
"#;

    #[test]
    fn parses_request_file() {
        let req: Request = toml::from_str(SAMPLE).unwrap();
        assert_eq!(req.method, Method::Post);
        assert_eq!(req.url, "{{base_url}}/users");
        assert!(!req.headers[0].enabled);
        assert_eq!(req.body, Body::Json(r#"{ "name": "Ada" }"#.into()));
        assert_eq!(
            req.auth,
            Auth::Bearer {
                token: "{{token}}".into()
            }
        );
    }

    #[test]
    fn round_trips_through_toml() {
        let req: Request = toml::from_str(SAMPLE).unwrap();
        let text = toml::to_string_pretty(&req).unwrap();
        let back: Request = toml::from_str(&text).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn parses_methods_case_insensitively() {
        assert_eq!("delete".parse::<Method>(), Ok(Method::Delete));
        assert_eq!("PATCH".parse::<Method>(), Ok(Method::Patch));
        assert!("FETCH".parse::<Method>().is_err());
    }

    #[test]
    fn minimal_file_defaults_to_get() {
        let req: Request =
            toml::from_str("name = \"Ping\"\nurl = \"https://example.com\"").unwrap();
        assert_eq!(req.method, Method::Get);
        assert!(req.body.is_none() && req.auth.is_inherit());
    }

    #[test]
    fn no_auth_is_written_but_inherit_is_not() {
        let mut req = Request::new(Method::Get, "x");
        assert!(!toml::to_string(&req).unwrap().contains("auth"));
        req.auth = Auth::None;
        let text = toml::to_string(&req).unwrap();
        assert!(text.contains("auth = \"none\""), "{text}");
        assert_eq!(toml::from_str::<Request>(&text).unwrap().auth, Auth::None);
    }
}
