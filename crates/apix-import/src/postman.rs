//! Postman Collection v2.0 / v2.1 and Postman environment import.
//!
//! The parser is deliberately forgiving: Postman exports vary between
//! versions and hand-edited files, so most fields are optional and several
//! accept more than one shape (a URL as text or as parts, auth parameters as
//! a list or a map, descriptions as text or objects).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use apix_core::store::{self, CollectionMeta, VarMap};
use apix_core::{ApiKeyLocation, Auth, Body, KeyValue, Method, Request};
use serde::Deserialize;
use serde_json::Value;

use crate::{ImportError, Plan, PlannedFile, Report, request_file, slug, unique};

/// What a Postman JSON export contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Collection,
    Environment,
    Globals,
}

/// Tells collections, environments and globals apart without fully parsing them.
pub fn detect(json: &str) -> Result<Kind, ImportError> {
    let value: Value = serde_json::from_str(json)?;
    if value.get("info").is_some() && value.get("item").is_some() {
        return Ok(Kind::Collection);
    }
    if value.get("requests").is_some() && value.get("order").is_some() {
        return Err(ImportError::Unsupported(
            "This is a Postman v1 collection. Export it again from Postman as Collection v2.1 and import that file.".into(),
        ));
    }
    match value.get("_postman_variable_scope").and_then(Value::as_str) {
        Some("globals") => Ok(Kind::Globals),
        _ if value.get("values").is_some() => Ok(Kind::Environment),
        _ => Err(ImportError::Unsupported(
            "This file is not a Postman collection or environment export.".into(),
        )),
    }
}

// ---------------------------------------------------------------------------
// Postman's JSON shapes
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct PmCollection {
    info: PmInfo,
    #[serde(default)]
    item: Vec<PmItem>,
    #[serde(default)]
    variable: Vec<PmKv>,
    auth: Option<PmAuth>,
    #[serde(default)]
    event: Vec<PmEvent>,
}

#[derive(Deserialize)]
struct PmInfo {
    #[serde(default)]
    name: String,
    #[serde(default)]
    schema: String,
    description: Option<PmDescription>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PmDescription {
    Text(String),
    Object { content: Option<String> },
}

impl PmDescription {
    fn text(&self) -> String {
        match self {
            PmDescription::Text(t) => t.clone(),
            PmDescription::Object { content } => content.clone().unwrap_or_default(),
        }
    }
}

#[derive(Deserialize)]
struct PmItem {
    #[serde(default)]
    name: String,
    /// Present on folders.
    item: Option<Vec<PmItem>>,
    /// Present on requests.
    request: Option<PmRequest>,
    #[serde(default)]
    response: Vec<Value>,
    #[serde(default)]
    event: Vec<PmEvent>,
    auth: Option<PmAuth>,
    description: Option<PmDescription>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PmRequest {
    Url(String),
    Full(Box<PmRequestFull>),
}

#[derive(Deserialize)]
struct PmRequestFull {
    method: Option<String>,
    url: Option<PmUrl>,
    #[serde(default)]
    header: PmHeaders,
    body: Option<PmBody>,
    auth: Option<PmAuth>,
    description: Option<PmDescription>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PmHeaders {
    List(Vec<PmKv>),
    /// v2.0 allows a raw `Key: value\n` block.
    Text(String),
}

impl Default for PmHeaders {
    fn default() -> Self {
        PmHeaders::List(Vec::new())
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PmUrl {
    Raw(String),
    Parts(Box<PmUrlParts>),
}

#[derive(Deserialize)]
struct PmUrlParts {
    raw: Option<String>,
    protocol: Option<String>,
    host: Option<Value>,
    port: Option<String>,
    path: Option<Value>,
    #[serde(default)]
    query: Vec<PmKv>,
    #[serde(default)]
    variable: Vec<PmKv>,
}

#[derive(Deserialize, Default)]
struct PmKv {
    key: Option<String>,
    value: Option<Value>,
    #[serde(default)]
    disabled: bool,
    /// Environments use `enabled` instead of `disabled`.
    enabled: Option<bool>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

impl PmKv {
    fn key(&self) -> String {
        self.key.clone().unwrap_or_default()
    }

    fn value(&self) -> String {
        value_text(self.value.as_ref())
    }

    fn is_enabled(&self) -> bool {
        !self.disabled && self.enabled.unwrap_or(true)
    }

    fn to_key_value(&self) -> KeyValue {
        KeyValue {
            key: self.key(),
            value: self.value(),
            enabled: self.is_enabled(),
        }
    }
}

#[derive(Deserialize)]
struct PmBody {
    mode: Option<String>,
    raw: Option<String>,
    #[serde(default)]
    urlencoded: Vec<PmKv>,
    #[serde(default)]
    formdata: Vec<PmKv>,
    graphql: Option<PmGraphql>,
    options: Option<Value>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Deserialize)]
struct PmGraphql {
    query: Option<String>,
    variables: Option<Value>,
}

#[derive(Deserialize, Clone)]
struct PmAuth {
    #[serde(rename = "type")]
    kind: String,
    #[serde(flatten)]
    params: BTreeMap<String, Value>,
}

impl PmAuth {
    /// v2.1 stores parameters as `[{"key": .., "value": ..}]`, v2.0 as `{"key": value}`.
    fn param(&self, name: &str) -> String {
        match self.params.get(&self.kind) {
            Some(Value::Array(list)) => list
                .iter()
                .find(|p| p.get("key").and_then(Value::as_str) == Some(name))
                .map(|p| value_text(p.get("value")))
                .unwrap_or_default(),
            Some(Value::Object(map)) => value_text(map.get(name)),
            _ => String::new(),
        }
    }
}

#[derive(Deserialize)]
struct PmEvent {
    listen: String,
    script: Option<PmScript>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Deserialize)]
struct PmScript {
    exec: Option<Value>,
}

impl PmEvent {
    fn source(&self) -> Option<String> {
        if self.disabled {
            return None;
        }
        let text = match self.script.as_ref()?.exec.as_ref()? {
            Value::String(s) => s.clone(),
            Value::Array(lines) => lines
                .iter()
                .map(|l| l.as_str().unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\n"),
            _ => return None,
        };
        (!text.trim().is_empty()).then_some(text)
    }
}

fn value_text(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

/// `["api", "example", "com"]` or `"api.example.com"`; path segments may be objects in v2.0.
fn joined(value: Option<&Value>, sep: &str) -> String {
    match value {
        Some(Value::Array(parts)) => parts
            .iter()
            .map(|p| match p {
                Value::String(s) => s.clone(),
                Value::Object(o) => value_text(o.get("value")),
                other => other.to_string(),
            })
            .collect::<Vec<_>>()
            .join(sep),
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Collection conversion
// ---------------------------------------------------------------------------

/// Converts a Postman v2.0 / v2.1 collection export.
pub fn convert_collection(json: &str) -> Result<Plan, ImportError> {
    if detect(json)? != Kind::Collection {
        return Err(ImportError::Unsupported(
            "This is a Postman environment, not a collection. Import it into an existing collection instead.".into(),
        ));
    }
    let pm: PmCollection = serde_json::from_str(json)?;
    let mut report = Report {
        collection: pm.info.name.clone(),
        ..Report::default()
    };
    if !pm.info.schema.is_empty()
        && !pm.info.schema.contains("v2.1")
        && !pm.info.schema.contains("v2.0")
    {
        report.warn(
            "",
            format!("Unknown schema {}; imported as v2.1.", pm.info.schema),
        );
    }

    let mut vars = VarMap::new();
    for v in pm.variable.iter().filter(|v| v.is_enabled()) {
        let key = v.key();
        if !key.is_empty() {
            vars.insert(key, v.value());
        }
    }
    report.variables = vars.len();

    let mut files = Vec::new();
    let mut ctx = Converter {
        report: &mut report,
        files: &mut files,
    };
    ctx.scripts(&pm.event, Path::new(""), "_collection", "");
    ctx.items(&pm.item, Path::new(""), "", pm.auth.as_ref());

    let meta = CollectionMeta {
        name: if pm.info.name.is_empty() {
            "Imported collection".into()
        } else {
            pm.info.name.clone()
        },
        description: pm
            .info
            .description
            .as_ref()
            .map(PmDescription::text)
            .unwrap_or_default(),
        vars,
    };
    Ok(Plan {
        meta,
        files,
        report,
    })
}

struct Converter<'a> {
    report: &'a mut Report,
    files: &'a mut Vec<PlannedFile>,
}

impl Converter<'_> {
    /// `trail` is the folder path for messages, like `Users / Admin`.
    fn items(&mut self, items: &[PmItem], dir: &Path, trail: &str, inherited: Option<&PmAuth>) {
        let mut used: HashSet<String> = HashSet::new();
        for item in items {
            let label = if trail.is_empty() {
                item.name.clone()
            } else {
                format!("{trail} / {}", item.name)
            };

            if let Some(children) = &item.item {
                let name = unique(slug(&item.name, "folder"), &mut used);
                let sub = dir.join(&name);
                self.report.folders += 1;
                if item
                    .description
                    .as_ref()
                    .is_some_and(|d| !d.text().is_empty())
                {
                    self.report.warn(
                        &label,
                        "Folder description was not imported (folders have no notes yet).",
                    );
                }
                self.scripts(&item.event, &sub, "_folder", &label);
                let auth = item.auth.as_ref().or(inherited);
                self.items(children, &sub, &label, auth);
            } else if let Some(request) = &item.request {
                let stem = unique(slug(&item.name, "request"), &mut used);
                let request = self.request(item, request, inherited, &label);
                self.files.push(PlannedFile::Request {
                    path: dir.join(request_file(&stem)),
                    request,
                });
                self.report.requests += 1;
                self.report.examples_skipped += item.response.len();
                self.scripts(&item.event, dir, &stem, &label);
            }
        }
    }

    /// Saves `prerequest` / `test` scripts as `<stem>.prerequest.js` / `<stem>.test.js`.
    fn scripts(&mut self, events: &[PmEvent], dir: &Path, stem: &str, label: &str) {
        for event in events {
            let Some(text) = event.source() else { continue };
            let kind = match event.listen.as_str() {
                "prerequest" => "prerequest",
                "test" => "test",
                other => {
                    self.report.warn(
                        label,
                        format!("Skipped a script for the unknown event \"{other}\"."),
                    );
                    continue;
                }
            };
            self.files.push(PlannedFile::Script {
                path: dir.join(format!("{stem}.{kind}.js")),
                text,
            });
            self.report.scripts += 1;
        }
    }

    fn request(
        &mut self,
        item: &PmItem,
        request: &PmRequest,
        inherited: Option<&PmAuth>,
        label: &str,
    ) -> Request {
        let mut out = Request {
            name: item.name.clone(),
            description: item
                .description
                .as_ref()
                .map(PmDescription::text)
                .unwrap_or_default(),
            ..Request::default()
        };
        let full = match request {
            PmRequest::Url(url) => {
                out.url = url.clone();
                out.auth = self.auth(inherited, label);
                return out;
            }
            PmRequest::Full(full) => full,
        };

        if out.description.is_empty()
            && let Some(d) = &full.description
        {
            out.description = d.text();
        }

        let method = full.method.as_deref().unwrap_or("GET");
        out.method = method.parse().unwrap_or_else(|_| {
            self.report.warn(
                label,
                format!("Method {method} is not supported yet; imported as GET."),
            );
            Method::Get
        });

        if let Some(url) = &full.url {
            self.url(url, &mut out, label);
        }

        out.headers = match &full.header {
            PmHeaders::List(list) => list
                .iter()
                .filter(|h| !h.key().is_empty())
                .map(PmKv::to_key_value)
                .collect(),
            PmHeaders::Text(text) => text
                .lines()
                .filter_map(|line| line.split_once(':'))
                .map(|(k, v)| KeyValue::new(k.trim(), v.trim()))
                .collect(),
        };

        if let Some(body) = &full.body {
            out.body = self.body(body, &out.headers, label);
        }
        out.auth = self.auth(full.auth.as_ref().or(inherited), label);
        out
    }

    fn url(&mut self, url: &PmUrl, out: &mut Request, label: &str) {
        let (raw, query, variables) = match url {
            PmUrl::Raw(raw) => (raw.clone(), &[][..], &[][..]),
            PmUrl::Parts(parts) => {
                let raw = parts.raw.clone().unwrap_or_else(|| {
                    let mut s = String::new();
                    if let Some(p) = &parts.protocol {
                        s.push_str(p);
                        s.push_str("://");
                    }
                    s.push_str(&joined(parts.host.as_ref(), "."));
                    if let Some(port) = &parts.port {
                        s.push(':');
                        s.push_str(port);
                    }
                    let path = joined(parts.path.as_ref(), "/");
                    let path = path.trim_start_matches('/');
                    if !path.is_empty() {
                        s.push('/');
                        s.push_str(path);
                    }
                    s
                });
                (raw, &parts.query[..], &parts.variable[..])
            }
        };

        // With a query list, the rows carry the enabled flags, so they win over the raw text.
        let mut url = if query.is_empty() {
            raw
        } else {
            out.query = query
                .iter()
                .filter(|q| !q.key().is_empty())
                .map(PmKv::to_key_value)
                .collect();
            raw.split_once('?')
                .map_or(raw.clone(), |(base, _)| base.to_owned())
        };

        // Path variables (`/users/:id`) have no Apix equivalent yet.
        let values: HashMap<String, String> =
            variables.iter().map(|v| (v.key(), v.value())).collect();
        if url.contains("/:") {
            url = url
                .split('/')
                .map(|segment| match segment.strip_prefix(':') {
                    Some(name) if !name.is_empty() => {
                        match values.get(name).filter(|v| !v.is_empty()) {
                            Some(value) => {
                                self.report.warn(
                                    label,
                                    format!("Path variable :{name} was replaced with its value \"{value}\"."),
                                );
                                value.clone()
                            }
                            None => {
                                self.report.warn(
                                    label,
                                    format!("Path variable :{name} became {{{{{name}}}}}; define it in an environment."),
                                );
                                format!("{{{{{name}}}}}")
                            }
                        }
                    }
                    _ => segment.to_owned(),
                })
                .collect::<Vec<_>>()
                .join("/");
        }
        out.url = url;
    }

    fn body(&mut self, body: &PmBody, headers: &[KeyValue], label: &str) -> Body {
        if body.disabled {
            return Body::None;
        }
        match body.mode.as_deref() {
            None => Body::None,
            Some("raw") => {
                let text = body.raw.clone().unwrap_or_default();
                if text.is_empty() {
                    return Body::None;
                }
                let language = body
                    .options
                    .as_ref()
                    .and_then(|o| o.pointer("/raw/language"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let content_type = headers
                    .iter()
                    .find(|h| h.enabled && h.key.eq_ignore_ascii_case("content-type"))
                    .map(|h| h.value.to_ascii_lowercase())
                    .unwrap_or_default();
                if language == "json" || content_type.contains("json") {
                    Body::Json(text)
                } else if language == "xml" || content_type.contains("xml") {
                    Body::Xml(text)
                } else {
                    Body::Text(text)
                }
            }
            Some("urlencoded") => Body::Form(
                body.urlencoded
                    .iter()
                    .filter(|f| !f.key().is_empty())
                    .map(PmKv::to_key_value)
                    .collect(),
            ),
            Some("formdata") => {
                let files = body
                    .formdata
                    .iter()
                    .filter(|f| f.kind.as_deref() == Some("file"))
                    .count();
                let fields: Vec<KeyValue> = body
                    .formdata
                    .iter()
                    .filter(|f| f.kind.as_deref() != Some("file") && !f.key().is_empty())
                    .map(PmKv::to_key_value)
                    .collect();
                let mut message = String::from(
                    "multipart/form-data is not supported yet; the text fields were imported as a URL-encoded form.",
                );
                if files > 0 {
                    message.push_str(&format!(" {files} file field(s) were skipped."));
                }
                self.report.warn(label, message);
                Body::Form(fields)
            }
            Some("graphql") => {
                // GraphQL over HTTP is a JSON body: {"query": ..., "variables": ...}.
                let graphql = body.graphql.as_ref();
                let query = graphql.and_then(|g| g.query.clone()).unwrap_or_default();
                let variables = match graphql.and_then(|g| g.variables.clone()) {
                    Some(Value::String(s)) if s.trim().is_empty() => Value::Null,
                    Some(Value::String(s)) => serde_json::from_str(&s).unwrap_or(Value::String(s)),
                    Some(v) => v,
                    None => Value::Null,
                };
                let mut payload = serde_json::Map::new();
                payload.insert("query".into(), Value::String(query));
                if !variables.is_null() {
                    payload.insert("variables".into(), variables);
                }
                Body::Json(
                    serde_json::to_string_pretty(&Value::Object(payload))
                        .expect("a JSON object always serializes"),
                )
            }
            Some("file") => {
                self.report.warn(
                    label,
                    "Binary file bodies are not supported yet; the body was skipped.",
                );
                Body::None
            }
            Some(other) => {
                self.report.warn(
                    label,
                    format!("Unknown body mode \"{other}\"; the body was skipped."),
                );
                Body::None
            }
        }
    }

    fn auth(&mut self, auth: Option<&PmAuth>, label: &str) -> Auth {
        let Some(auth) = auth else {
            return Auth::None;
        };
        match auth.kind.as_str() {
            "noauth" => Auth::None,
            "bearer" => Auth::Bearer {
                token: auth.param("token"),
            },
            "basic" => Auth::Basic {
                username: auth.param("username"),
                password: auth.param("password"),
            },
            "apikey" => Auth::ApiKey {
                key: auth.param("key"),
                value: auth.param("value"),
                location: if auth.param("in") == "query" {
                    ApiKeyLocation::Query
                } else {
                    ApiKeyLocation::Header
                },
            },
            other => {
                self.report.warn(
                    label,
                    format!("{other} authorization is not supported yet; the request was imported without auth."),
                );
                Auth::None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Environments
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct PmEnvironment {
    #[serde(default)]
    name: String,
    #[serde(default)]
    values: Vec<PmKv>,
}

/// A converted Postman environment. Values marked `secret` in Postman go to
/// `<name>.local.toml`, which is gitignored, so they never end up in a commit.
#[derive(Debug, Clone)]
pub struct EnvironmentPlan {
    /// The display name from Postman.
    pub title: String,
    /// The file name used under `environments/`.
    pub name: String,
    pub vars: VarMap,
    pub secrets: VarMap,
    pub skipped_disabled: usize,
}

pub fn convert_environment(json: &str) -> Result<EnvironmentPlan, ImportError> {
    match detect(json)? {
        Kind::Environment | Kind::Globals => {}
        Kind::Collection => {
            return Err(ImportError::Unsupported(
                "This is a Postman collection, not an environment.".into(),
            ));
        }
    }
    let env: PmEnvironment = serde_json::from_str(json)?;
    let mut plan = EnvironmentPlan {
        name: slug(&env.name, "environment"),
        title: env.name,
        vars: VarMap::new(),
        secrets: VarMap::new(),
        skipped_disabled: 0,
    };
    for value in &env.values {
        let key = value.key();
        if key.is_empty() {
            continue;
        }
        if !value.is_enabled() {
            plan.skipped_disabled += 1;
            continue;
        }
        if value.kind.as_deref() == Some("secret") {
            plan.secrets.insert(key, value.value());
        } else {
            plan.vars.insert(key, value.value());
        }
    }
    Ok(plan)
}

impl EnvironmentPlan {
    /// Writes the environment into an existing collection. Refuses to
    /// replace an environment that already exists.
    pub fn write(&self, collection_root: &Path) -> Result<Vec<PathBuf>, ImportError> {
        let target = collection_root
            .join(store::ENVIRONMENTS_DIR)
            .join(format!("{}.toml", self.name));
        if target.exists() {
            return Err(ImportError::Exists(target));
        }
        let mut written = vec![store::save_environment(
            collection_root,
            &self.name,
            &self.vars,
            false,
        )?];
        if !self.secrets.is_empty() {
            written.push(store::save_environment(
                collection_root,
                &self.name,
                &self.secrets,
                true,
            )?);
        }
        Ok(written)
    }
}
