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

fn is_false(value: &bool) -> bool {
    !*value
}

/// One field of a multipart form: text, or a file read when the request is sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormPart {
    pub key: String,
    /// The text, or for a file its path (relative to the request file's folder, or absolute).
    #[serde(default)]
    pub value: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub file: bool,
    /// The part's own `Content-Type`, e.g. `image/png`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
}

impl FormPart {
    pub fn text(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            file: false,
            content_type: None,
            enabled: true,
        }
    }

    pub fn file(key: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            file: true,
            ..Self::text(key, path)
        }
    }
}

/// A GraphQL operation, sent as `{"query": …, "variables": …}`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GraphQl {
    pub query: String,
    /// JSON object text; empty means no variables.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub variables: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Body {
    #[default]
    None,
    Json(String),
    Text(String),
    Xml(String),
    /// `application/x-www-form-urlencoded`
    Form(Vec<KeyValue>),
    /// `multipart/form-data`, with text and file fields.
    Multipart(Vec<FormPart>),
    /// A file sent as the raw body.
    Binary {
        /// Relative to the request file's folder, or absolute.
        file: String,
    },
    #[serde(rename = "graphql")]
    GraphQl(GraphQl),
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
            // The HTTP engine adds the boundary itself.
            Body::Multipart(_) => None,
            Body::Binary { .. } => Some("application/octet-stream"),
            Body::GraphQl(_) => Some("application/json"),
        }
    }

    /// Makes relative file paths absolute against `dir`, the request file's folder.
    pub fn with_files_relative_to(mut self, dir: &std::path::Path) -> Self {
        let absolute = |path: &mut String| {
            if !path.is_empty() && std::path::Path::new(path.as_str()).is_relative() {
                *path = dir.join(path.as_str()).to_string_lossy().into_owned();
            }
        };
        match &mut self {
            Body::Multipart(parts) => {
                for part in parts.iter_mut().filter(|p| p.file) {
                    absolute(&mut part.value);
                }
            }
            Body::Binary { file } => absolute(file),
            _ => {}
        }
        self
    }

    /// The GraphQL request body as JSON text, or why it cannot be built.
    pub fn graphql_json(gql: &GraphQl) -> Result<String, String> {
        let variables = if gql.variables.trim().is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&gql.variables)
                .map_err(|e| format!("GraphQL variables are not valid JSON: {e}"))?
        };
        Ok(serde_json::json!({ "query": gql.query, "variables": variables }).to_string())
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
    /// HTTP Digest (RFC 7616): the server's challenge picks the algorithm.
    Digest {
        username: String,
        #[serde(default)]
        password: String,
    },
    #[serde(rename = "oauth1")]
    OAuth1(OAuth1),
    #[serde(rename = "oauth2")]
    OAuth2(OAuth2),
    /// AWS Signature Version 4.
    #[serde(rename = "aws_sigv4")]
    AwsSigV4(AwsSigV4),
}

fn is_empty(s: &str) -> bool {
    s.is_empty()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OAuth1Method {
    #[default]
    #[serde(rename = "HMAC-SHA1")]
    HmacSha1,
    #[serde(rename = "HMAC-SHA256")]
    HmacSha256,
    #[serde(rename = "PLAINTEXT")]
    Plaintext,
}

impl OAuth1Method {
    pub const ALL: [OAuth1Method; 3] = [
        OAuth1Method::HmacSha1,
        OAuth1Method::HmacSha256,
        OAuth1Method::Plaintext,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            OAuth1Method::HmacSha1 => "HMAC-SHA1",
            OAuth1Method::HmacSha256 => "HMAC-SHA256",
            OAuth1Method::Plaintext => "PLAINTEXT",
        }
    }
}

/// OAuth 1.0a request signing (RFC 5849), with a token already obtained.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OAuth1 {
    pub consumer_key: String,
    #[serde(default)]
    pub consumer_secret: String,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub token: String,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub token_secret: String,
    #[serde(default)]
    pub signature_method: OAuth1Method,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub realm: String,
    /// Where the `oauth_*` parameters go: the Authorization header or the URL.
    #[serde(default)]
    pub location: ApiKeyLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuth2Grant {
    #[default]
    ClientCredentials,
    /// Resource owner password credentials.
    Password,
    /// Authorization code, with PKCE unless turned off; the login opens in
    /// the browser and the code comes back to a loopback address.
    AuthorizationCode,
}

impl OAuth2Grant {
    pub const ALL: [OAuth2Grant; 3] = [
        OAuth2Grant::ClientCredentials,
        OAuth2Grant::Password,
        OAuth2Grant::AuthorizationCode,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OAuth2Grant::ClientCredentials => "Client credentials",
            OAuth2Grant::Password => "Password",
            OAuth2Grant::AuthorizationCode => "Authorization code",
        }
    }
}

/// How the client proves who it is at the token endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientAuth {
    /// HTTP Basic with the client id and secret (RFC 6749's default).
    #[default]
    Basic,
    /// `client_id` and `client_secret` in the form body.
    Body,
}

/// OAuth 2.0: Apix gets the access token itself and sends it as a bearer token.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OAuth2 {
    #[serde(default)]
    pub grant: OAuth2Grant,
    pub token_url: String,
    /// The login page, for the authorization code grant.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub auth_url: String,
    pub client_id: String,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub client_secret: String,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub scope: String,
    /// For the password grant.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub username: String,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub password: String,
    /// Where the login sends the browser back. Must be a loopback address
    /// (`http://127.0.0.1:<port>/…` or `http://localhost:<port>/…`); empty
    /// picks a free port on 127.0.0.1.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub redirect_uri: String,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub pkce: bool,
    #[serde(default)]
    pub client_auth: ClientAuth,
}

/// AWS Signature Version 4.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AwsSigV4 {
    pub access_key: String,
    #[serde(default)]
    pub secret_key: String,
    /// For temporary credentials.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub session_token: String,
    pub region: String,
    pub service: String,
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
            Auth::Digest { .. } => "Digest auth",
            Auth::OAuth1(_) => "OAuth 1.0",
            Auth::OAuth2(_) => "OAuth 2.0",
            Auth::AwsSigV4(_) => "AWS Signature",
        }
    }
}

/// Transport settings for a request, stored as a `[settings]` table.
/// Every field is optional: `None` means "inherit", falling back
/// folder → collection → project and finally to [`EffectiveSettings`]'
/// built-in defaults, the same chain as [`Auth::Inherit`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RequestSettings {
    /// Whole-request timeout. `0` means "no timeout" explicitly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_redirects: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_redirects: Option<u32>,
    /// Verify the server certificate. Turning this off sends the request
    /// over TLS without checking who answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_verify: Option<bool>,
    /// Proxy URL. An empty string turns the proxy off even when the
    /// environment sets HTTP_PROXY/HTTPS_PROXY.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
    /// Path to a PEM file with the client certificate chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_cert: Option<String>,
    /// Path to the PEM private key; defaults to `client_cert` when the
    /// certificate file carries the key too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_cookies: Option<bool>,
}

impl RequestSettings {
    pub fn is_inherit(&self) -> bool {
        *self == Self::default()
    }

    /// Fill every `None` from `parent`: the nearest value wins, per field.
    pub fn or_inherit(mut self, parent: &RequestSettings) -> Self {
        self.timeout_ms = self.timeout_ms.or(parent.timeout_ms);
        self.follow_redirects = self.follow_redirects.or(parent.follow_redirects);
        self.max_redirects = self.max_redirects.or(parent.max_redirects);
        self.tls_verify = self.tls_verify.or(parent.tls_verify);
        self.proxy = self.proxy.take().or_else(|| parent.proxy.clone());
        self.client_cert = self
            .client_cert
            .take()
            .or_else(|| parent.client_cert.clone());
        self.client_key = self.client_key.take().or_else(|| parent.client_key.clone());
        self.send_cookies = self.send_cookies.or(parent.send_cookies);
        self
    }

    /// The final values the engine uses, after inheritance.
    pub fn effective(&self) -> EffectiveSettings {
        EffectiveSettings {
            timeout: match self.timeout_ms {
                None | Some(0) => None,
                Some(ms) => Some(std::time::Duration::from_millis(ms)),
            },
            follow_redirects: self.follow_redirects.unwrap_or(true),
            max_redirects: self.max_redirects.unwrap_or(10),
            tls_verify: self.tls_verify.unwrap_or(true),
            proxy: match self.proxy.as_deref() {
                None => ProxyChoice::Environment,
                Some("") => ProxyChoice::Off,
                Some(url) => ProxyChoice::Url(url.to_string()),
            },
            client_cert: self.client_cert.clone().filter(|s| !s.is_empty()),
            client_key: self.client_key.clone().filter(|s| !s.is_empty()),
            send_cookies: self.send_cookies.unwrap_or(true),
        }
    }

    /// Certificate and key paths made absolute against `dir`, the folder
    /// of the file that defined them.
    pub fn with_files_relative_to(mut self, dir: &std::path::Path) -> Self {
        for path in [&mut self.client_cert, &mut self.client_key]
            .into_iter()
            .flatten()
        {
            if !path.is_empty() && std::path::Path::new(path).is_relative() {
                *path = dir.join(&*path).to_string_lossy().into_owned();
            }
        }
        self
    }
}

/// What a request resolves to once every `None` fell through to the
/// defaults: no timeout, follow up to 10 redirects, verify TLS, proxy
/// from the environment, no client certificate, cookies on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveSettings {
    pub timeout: Option<std::time::Duration>,
    pub follow_redirects: bool,
    pub max_redirects: u32,
    pub tls_verify: bool,
    pub proxy: ProxyChoice,
    pub client_cert: Option<String>,
    pub client_key: Option<String>,
    pub send_cookies: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyChoice {
    /// Honour HTTP_PROXY/HTTPS_PROXY/ALL_PROXY.
    Environment,
    /// No proxy, even when the environment sets one.
    Off,
    Url(String),
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
    #[serde(default, skip_serializing_if = "RequestSettings::is_inherit")]
    pub settings: RequestSettings,
}

impl Request {
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            ..Default::default()
        }
    }

    /// The request with its body's relative file paths made absolute
    /// against `dir`, the folder of the request file.
    pub fn with_files_relative_to(mut self, dir: &std::path::Path) -> Self {
        self.body = self.body.with_files_relative_to(dir);
        self.settings = self.settings.with_files_relative_to(dir);
        self
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
    fn settings_round_trip_and_defaults() {
        let toml_text = r#"
name = "r"
url = "https://example.com"

[settings]
timeout_ms = 5000
tls_verify = false
proxy = ""
"#;
        let req: Request = toml::from_str(toml_text).unwrap();
        assert_eq!(req.settings.timeout_ms, Some(5000));
        assert_eq!(req.settings.tls_verify, Some(false));
        assert_eq!(req.settings.follow_redirects, None);

        let eff = req.settings.effective();
        assert_eq!(eff.timeout, Some(std::time::Duration::from_secs(5)));
        assert!(!eff.tls_verify);
        assert!(eff.follow_redirects, "unset falls back to the default");
        assert_eq!(eff.max_redirects, 10);
        assert_eq!(eff.proxy, ProxyChoice::Off, "empty string turns it off");
        assert!(eff.send_cookies);

        // An untouched [settings] table is not written back to disk.
        let out = toml::to_string(&Request::new(Method::Get, "x")).unwrap();
        assert!(!out.contains("[settings]"));
        // timeout_ms = 0 means "no timeout" explicitly.
        let eff = RequestSettings {
            timeout_ms: Some(0),
            ..Default::default()
        }
        .effective();
        assert_eq!(eff.timeout, None);
    }

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
    fn new_bodies_round_trip_through_toml() {
        let bodies = [
            Body::Multipart(vec![
                FormPart::text("title", "Invoice"),
                FormPart {
                    content_type: Some("application/pdf".into()),
                    ..FormPart::file("document", "files/invoice.pdf")
                },
            ]),
            Body::Binary {
                file: "dump.bin".into(),
            },
            Body::GraphQl(GraphQl {
                query: "query { me { id } }".into(),
                variables: "{\"x\": 1}".into(),
            }),
        ];
        for body in bodies {
            let mut req = Request::new(Method::Post, "x");
            req.body = body;
            let text = toml::to_string_pretty(&req).unwrap();
            assert_eq!(toml::from_str::<Request>(&text).unwrap(), req, "{text}");
        }
        let text = "name = \"G\"\nurl = \"x\"\n[body.graphql]\nquery = \"{ a }\"\n";
        let req: Request = toml::from_str(text).unwrap();
        assert_eq!(req.body.default_content_type(), Some("application/json"));
    }

    #[test]
    fn file_paths_resolve_against_the_request_folder() {
        let dir = std::path::Path::new("/collections/shop/orders");
        let body = Body::Multipart(vec![
            FormPart::file("a", "invoice.pdf"),
            FormPart::text("b", "invoice.pdf"),
        ])
        .with_files_relative_to(dir);
        let Body::Multipart(parts) = body else {
            unreachable!()
        };
        assert_eq!(
            std::path::Path::new(&parts[0].value),
            dir.join("invoice.pdf")
        );
        assert_eq!(parts[1].value, "invoice.pdf", "text stays text");
    }

    #[test]
    fn graphql_becomes_json_and_checks_variables() {
        let gql = GraphQl {
            query: "query($id: ID!) { user(id: $id) { name } }".into(),
            variables: "{\"id\": \"7\"}".into(),
        };
        let json: serde_json::Value =
            serde_json::from_str(&Body::graphql_json(&gql).unwrap()).unwrap();
        assert_eq!(json["variables"]["id"], "7");
        assert!(json["query"].as_str().unwrap().starts_with("query($id"));
        let empty = GraphQl {
            query: "{ a }".into(),
            variables: " ".into(),
        };
        assert!(
            Body::graphql_json(&empty)
                .unwrap()
                .contains("\"variables\":null")
        );
        let broken = GraphQl {
            query: "{ a }".into(),
            variables: "{".into(),
        };
        assert!(Body::graphql_json(&broken).is_err());
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

    #[test]
    fn signing_auth_types_round_trip_through_toml() {
        let auths = [
            Auth::Digest {
                username: "ada".into(),
                password: "{{pw}}".into(),
            },
            Auth::OAuth1(OAuth1 {
                consumer_key: "ck".into(),
                consumer_secret: "cs".into(),
                token: "t".into(),
                signature_method: OAuth1Method::HmacSha256,
                location: ApiKeyLocation::Query,
                ..OAuth1::default()
            }),
            Auth::OAuth2(OAuth2 {
                grant: OAuth2Grant::AuthorizationCode,
                token_url: "https://id.example.com/token".into(),
                auth_url: "https://id.example.com/authorize".into(),
                client_id: "app".into(),
                scope: "read write".into(),
                pkce: true,
                client_auth: ClientAuth::Body,
                ..OAuth2::default()
            }),
            Auth::AwsSigV4(AwsSigV4 {
                access_key: "{{aws_key}}".into(),
                secret_key: "{{aws_secret}}".into(),
                session_token: String::new(),
                region: "eu-central-1".into(),
                service: "execute-api".into(),
            }),
        ];
        for auth in auths {
            let mut req = Request::new(Method::Get, "x");
            req.auth = auth;
            let text = toml::to_string_pretty(&req).unwrap();
            assert_eq!(toml::from_str::<Request>(&text).unwrap(), req, "{text}");
        }

        let text =
            "name = \"x\"\nurl = \"x\"\n[auth.oauth2]\ntoken_url = \"t\"\nclient_id = \"c\"\n";
        let Auth::OAuth2(o) = toml::from_str::<Request>(text).unwrap().auth else {
            panic!("oauth2 expected");
        };
        assert_eq!(o.grant, OAuth2Grant::ClientCredentials);
        assert!(o.pkce, "PKCE is on unless turned off");
    }
}
