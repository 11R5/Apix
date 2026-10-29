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
    Html(String),
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
            Body::Html(_) => Some("text/html; charset=utf-8"),
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
    /// Path to a PEM file with certificate authorities to trust besides the
    /// usual ones (a company or self-signed CA).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ca_cert: Option<String>,
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
        self.ca_cert = self.ca_cert.take().or_else(|| parent.ca_cert.clone());
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
            ca_cert: self.ca_cert.clone().filter(|s| !s.is_empty()),
            send_cookies: self.send_cookies.unwrap_or(true),
        }
    }

    /// Certificate and key paths made absolute against `dir`, the folder
    /// of the file that defined them.
    pub fn with_files_relative_to(mut self, dir: &std::path::Path) -> Self {
        for path in [
            &mut self.client_cert,
            &mut self.client_key,
            &mut self.ca_cert,
        ]
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
    pub ca_cert: Option<String>,
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
    /// Checks on the response written without code, run after the test scripts.
    #[serde(default, rename = "assert", skip_serializing_if = "Vec::is_empty")]
    pub asserts: Vec<Assertion>,
    /// Saved messages of a WebSocket request (`ws://` or `wss://`), sent
    /// when asked; stored as `[[message]]`.
    #[serde(default, rename = "message", skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<WsMessage>,
    /// What to call when the URL is `grpc://` or `grpcs://`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grpc: Option<GrpcCall>,
    /// Options of a `ws://` or `wss://` request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub websocket: Option<WebSocketOptions>,
}

/// What a gRPC request calls, stored as `[grpc]`. The body (JSON) is the
/// request message; a JSON array is a stream of messages.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrpcCall {
    /// `package.Service/Method`.
    #[serde(default)]
    pub method: String,
    /// `.proto` files that describe the service, relative to the request
    /// file. Without them the server is asked (reflection).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protos: Vec<String>,
    /// Folders their `import`s are looked up in, relative to the request file.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub import_paths: Vec<String>,
}

/// A message saved with a WebSocket request. `{{variables}}` in the body
/// are filled in when it is sent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WsMessage {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub body: String,
    /// How `body` is read: as text, or as bytes written in base64 or hex.
    #[serde(default, skip_serializing_if = "MessageFormat::is_text")]
    pub format: MessageFormat,
}

/// How a WebSocket message's text stands for what is sent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageFormat {
    /// A text message, as written.
    #[default]
    Text,
    /// A binary message, its bytes in base64.
    Base64,
    /// A binary message, its bytes in hex (spaces and `0x` allowed).
    Hex,
}

impl MessageFormat {
    pub const ALL: [MessageFormat; 3] = [
        MessageFormat::Text,
        MessageFormat::Base64,
        MessageFormat::Hex,
    ];

    pub fn is_text(&self) -> bool {
        *self == MessageFormat::Text
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            MessageFormat::Text => "text",
            MessageFormat::Base64 => "base64",
            MessageFormat::Hex => "hex",
        }
    }

    /// The bytes a binary message's text stands for (`None` for text).
    pub fn decode(&self, text: &str) -> Option<Result<Vec<u8>, String>> {
        use base64::Engine as _;
        match self {
            MessageFormat::Text => None,
            MessageFormat::Base64 => Some(
                base64::engine::general_purpose::STANDARD
                    .decode(text.split_whitespace().collect::<String>())
                    .map_err(|e| format!("not valid base64: {e}")),
            ),
            MessageFormat::Hex => {
                let digits: String = text
                    .split_whitespace()
                    .map(|w| w.trim_start_matches("0x").trim_start_matches("0X"))
                    .collect();
                Some(if !digits.len().is_multiple_of(2) {
                    Err("hex needs two digits per byte".into())
                } else {
                    (0..digits.len())
                        .step_by(2)
                        .map(|i| {
                            u8::from_str_radix(&digits[i..i + 2], 16)
                                .map_err(|_| format!("`{}` is not a hex byte", &digits[i..i + 2]))
                        })
                        .collect()
                })
            }
        }
    }
}

impl std::str::FromStr for MessageFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|f| f.as_str().eq_ignore_ascii_case(s))
            .ok_or_else(|| format!("`{s}` is not a message format (text, base64, hex)"))
    }
}

/// WebSocket options, stored as `[websocket]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSocketOptions {
    /// Subprotocols offered in the handshake (`Sec-WebSocket-Protocol`), in
    /// order of preference; the server picks one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subprotocols: Vec<String>,
}

impl WsMessage {
    /// The name, or the start of the body when it has none.
    pub fn label(&self) -> String {
        if !self.name.is_empty() {
            return self.name.clone();
        }
        let first = self.body.trim().lines().next().unwrap_or_default();
        let mut label: String = first.chars().take(40).collect();
        if first.chars().count() > 40 || self.body.trim().lines().count() > 1 {
            label.push('…');
        }
        label
    }
}

/// A check on the response without code, like `status == 200` or
/// `body.users[0].id exists`. Stored as `[[assert]]` in the request file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assertion {
    /// `status`, `time` (ms), `size` (bytes), `body`, `body.<path>` (JSON,
    /// like `body.users[0].id`) or `header.<name>`.
    pub target: String,
    pub op: AssertOp,
    /// Compared as JSON when it parses as JSON (`200`, `true`, `"x"`), else
    /// as text; `{{variables}}` are filled in.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value: String,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
}

/// How an [`Assertion`] compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AssertOp {
    #[default]
    #[serde(rename = "==")]
    Eq,
    #[serde(rename = "!=")]
    Ne,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = ">=")]
    Ge,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = "contains")]
    Contains,
    #[serde(rename = "not contains")]
    NotContains,
    #[serde(rename = "exists")]
    Exists,
    #[serde(rename = "not exists")]
    NotExists,
    /// A regular expression.
    #[serde(rename = "matches")]
    Matches,
    /// A JSON type: string, number, boolean, array, object or null.
    #[serde(rename = "is")]
    Is,
}

impl AssertOp {
    pub const ALL: [AssertOp; 12] = [
        AssertOp::Eq,
        AssertOp::Ne,
        AssertOp::Gt,
        AssertOp::Ge,
        AssertOp::Lt,
        AssertOp::Le,
        AssertOp::Contains,
        AssertOp::NotContains,
        AssertOp::Exists,
        AssertOp::NotExists,
        AssertOp::Matches,
        AssertOp::Is,
    ];

    /// As written in files and shown in the app.
    pub fn as_str(self) -> &'static str {
        match self {
            AssertOp::Eq => "==",
            AssertOp::Ne => "!=",
            AssertOp::Gt => ">",
            AssertOp::Ge => ">=",
            AssertOp::Lt => "<",
            AssertOp::Le => "<=",
            AssertOp::Contains => "contains",
            AssertOp::NotContains => "not contains",
            AssertOp::Exists => "exists",
            AssertOp::NotExists => "not exists",
            AssertOp::Matches => "matches",
            AssertOp::Is => "is",
        }
    }

    /// `exists` and `not exists` take no value.
    pub fn takes_value(self) -> bool {
        !matches!(self, AssertOp::Exists | AssertOp::NotExists)
    }
}

impl Assertion {
    pub fn new(target: impl Into<String>, op: AssertOp, value: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            op,
            value: value.into(),
            enabled: true,
        }
    }

    /// `status == 200`, the test's name.
    pub fn label(&self) -> String {
        if self.op.takes_value() {
            format!("{} {} {}", self.target, self.op.as_str(), self.value)
        } else {
            format!("{} {}", self.target, self.op.as_str())
        }
    }
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
        if let Some(grpc) = &mut self.grpc {
            let absolute = |p: &mut String| {
                if !p.is_empty() && std::path::Path::new(p.as_str()).is_relative() {
                    *p = dir.join(p.as_str()).to_string_lossy().into_owned();
                }
            };
            grpc.protos.iter_mut().for_each(absolute);
            grpc.import_paths.iter_mut().for_each(absolute);
        }
        self
    }

    /// A `grpc://` or `grpcs://` request.
    pub fn is_grpc(&self) -> bool {
        let url = self.url.trim_start().to_ascii_lowercase();
        url.starts_with("grpc://") || url.starts_with("grpcs://")
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
        // A CA file is inherited like the rest and found next to its file.
        let parent = RequestSettings {
            ca_cert: Some("certs/ca.pem".into()),
            ..Default::default()
        };
        let merged = RequestSettings::default().or_inherit(&parent);
        let dir = std::path::Path::new("/collection");
        assert_eq!(
            merged.with_files_relative_to(dir).effective().ca_cert,
            Some(dir.join("certs/ca.pem").to_string_lossy().into_owned())
        );
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
    fn grpc_calls_round_trip() {
        let text = r#"
name = "Say hello"
url = "grpcs://api.example.com"

[body]
json = '{"name": "Ada"}'

[grpc]
method = "helloworld.Greeter/SayHello"
protos = ["../protos/hello.proto", "/abs/other.proto"]
import_paths = ["../protos"]
"#;
        let req: Request = toml::from_str(text).unwrap();
        assert!(req.is_grpc());
        let call = req.grpc.clone().unwrap();
        assert_eq!(call.method, "helloworld.Greeter/SayHello");
        let back: Request = toml::from_str(&toml::to_string_pretty(&req).unwrap()).unwrap();
        assert_eq!(req, back);
        // Paths are relative to the request file; absolute ones stay.
        let dir = std::path::Path::new("/collection/api");
        let moved = req.with_files_relative_to(dir).grpc.unwrap();
        assert_eq!(
            moved.protos[0],
            dir.join("../protos/hello.proto").to_string_lossy()
        );
        assert_eq!(moved.protos[1], "/abs/other.proto");
        assert_eq!(
            moved.import_paths[0],
            dir.join("../protos").to_string_lossy()
        );
        assert!(!Request::new(Method::Get, "https://x").is_grpc());
        assert!(Request::new(Method::Get, "GRPC://x:1").is_grpc());
    }

    #[test]
    fn websocket_messages_round_trip() {
        let text = r#"
name = "Chat"
url = "wss://example.com/chat"

[[message]]
name = "Join"
body = '{"join": "{{room}}"}'

[[message]]
body = "ping"

[[message]]
name = "Raw"
body = "01 02 ff"
format = "hex"

[websocket]
subprotocols = ["graphql-transport-ws", "graphql-ws"]
"#;
        let req: Request = toml::from_str(text).unwrap();
        assert_eq!(req.messages.len(), 3);
        assert_eq!(req.messages[2].format, MessageFormat::Hex);
        assert_eq!(
            req.websocket.as_ref().unwrap().subprotocols,
            ["graphql-transport-ws", "graphql-ws"]
        );
        // Text messages do not write a format.
        assert!(
            !toml::to_string(&req.messages[0])
                .unwrap()
                .contains("format")
        );
        assert_eq!(req.messages[0].label(), "Join");
        assert_eq!(req.messages[1].label(), "ping");
        let back: Request = toml::from_str(&toml::to_string_pretty(&req).unwrap()).unwrap();
        assert_eq!(req, back);
        // Variables in messages wait until each is sent.
        let mut vars = crate::Variables::new();
        vars.set("room", "7");
        assert_eq!(
            req.resolve(&vars).request.messages[0].body,
            r#"{"join": "{{room}}"}"#
        );
        let long = WsMessage {
            name: String::new(),
            body: "a much longer first line that goes on and on and on\nsecond".into(),
            format: MessageFormat::Text,
        };
        assert_eq!(long.label().chars().count(), 41);
        assert!(long.label().ends_with('…'));
    }

    #[test]
    fn binary_message_formats() {
        assert_eq!(MessageFormat::Text.decode("hi"), None);
        assert_eq!(
            MessageFormat::Hex.decode("0x01 02\nFF").unwrap().unwrap(),
            [1, 2, 255]
        );
        assert!(MessageFormat::Hex.decode("abc").unwrap().is_err());
        assert!(MessageFormat::Hex.decode("zz").unwrap().is_err());
        assert_eq!(
            MessageFormat::Base64.decode("AQL/").unwrap().unwrap(),
            [1, 2, 255]
        );
        assert!(MessageFormat::Base64.decode("***").unwrap().is_err());
        assert_eq!("HEX".parse::<MessageFormat>(), Ok(MessageFormat::Hex));
        assert!("bin".parse::<MessageFormat>().is_err());
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
