//! OpenAPI 3.x and Swagger 2.0 (JSON or YAML) → one collection with a
//! request per operation, a folder per tag. The first server becomes the
//! `baseUrl` collection variable (and every server an environment), path
//! parameters become `{{variables}}`, bodies are filled from examples or
//! built from the schema, and security schemes become auth with empty
//! credential variables to fill in.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use apix_core::store::{CollectionMeta, VarMap};
use apix_core::{
    ApiKeyLocation, Auth, Body, FormPart, KeyValue, Method, OAuth2, OAuth2Grant, Request,
};
use serde_json::{Map, Value, json};

use crate::postman::EnvironmentPlan;
use crate::{Found, ImportError, Plan, PlannedFile, Report, request_file, slug, unique};

/// Whether `value` is an OpenAPI 3 or Swagger 2 document.
pub(crate) fn is_openapi(value: &Value) -> bool {
    (value.get("openapi").is_some() || value.get("swagger").is_some())
        && value.get("paths").is_some()
}

/// Parses JSON, or YAML when it is not JSON.
pub(crate) fn parse(text: &str) -> Option<Value> {
    if let Ok(value) = serde_json::from_str(text) {
        return Some(value);
    }
    let yaml: serde_yaml::Value = serde_yaml::from_str(text).ok()?;
    serde_json::to_value(yaml).ok()
}

const METHODS: [(&str, Method); 7] = [
    ("get", Method::Get),
    ("post", Method::Post),
    ("put", Method::Put),
    ("patch", Method::Patch),
    ("delete", Method::Delete),
    ("head", Method::Head),
    ("options", Method::Options),
];

fn text(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// A value as a variable's text.
fn plain(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

struct Spec<'a> {
    doc: &'a Value,
    swagger: bool,
}

impl<'a> Spec<'a> {
    /// Follows `$ref`s inside the document (`#/components/schemas/User`).
    fn resolve(&self, mut v: &'a Value) -> &'a Value {
        for _ in 0..32 {
            let Some(reference) = v.get("$ref").and_then(Value::as_str) else {
                return v;
            };
            let Some(pointer) = reference.strip_prefix('#') else {
                return v;
            };
            match self.doc.pointer(pointer) {
                Some(target) => v = target,
                None => return v,
            }
        }
        v
    }

    /// An example value for `schema`: its example or default, else one built
    /// from its type, a few levels deep.
    fn sample(&self, schema: &Value, depth: usize) -> Value {
        let schema = self.resolve(schema);
        if depth > 6 {
            return Value::Null;
        }
        for key in ["example", "default"] {
            if let Some(v) = schema.get(key) {
                return v.clone();
            }
        }
        if let Some(first) = schema.get("examples").and_then(|e| e.as_array()?.first()) {
            return first.clone();
        }
        if let Some(first) = schema.get("enum").and_then(|e| e.as_array()?.first()) {
            return first.clone();
        }
        if let Some(all) = schema.get("allOf").and_then(Value::as_array) {
            let mut merged = Map::new();
            for part in all {
                if let Value::Object(map) = self.sample(part, depth + 1) {
                    merged.extend(map);
                }
            }
            return Value::Object(merged);
        }
        for key in ["oneOf", "anyOf"] {
            if let Some(first) = schema.get(key).and_then(|e| e.as_array()?.first()) {
                return self.sample(first, depth + 1);
            }
        }
        let kind = match schema.get("type") {
            Some(Value::String(t)) => t.as_str(),
            // OpenAPI 3.1: ["string", "null"]
            Some(Value::Array(types)) => types
                .iter()
                .filter_map(Value::as_str)
                .find(|t| *t != "null")
                .unwrap_or("null"),
            _ if schema.get("properties").is_some() => "object",
            _ => "",
        };
        match kind {
            "object" | "" if schema.get("properties").is_some() => {
                let mut out = Map::new();
                if let Some(props) = schema.get("properties").and_then(Value::as_object) {
                    for (name, prop) in props {
                        if self.resolve(prop).get("readOnly") == Some(&Value::Bool(true)) {
                            continue;
                        }
                        out.insert(name.clone(), self.sample(prop, depth + 1));
                    }
                }
                Value::Object(out)
            }
            "object" => json!({}),
            "array" => match schema.get("items") {
                Some(items) => json!([self.sample(items, depth + 1)]),
                None => json!([]),
            },
            "integer" => json!(0),
            "number" => json!(0.0),
            "boolean" => json!(false),
            "string" => json!(match text(schema, "format").as_str() {
                "date-time" => "2024-01-01T00:00:00Z",
                "date" => "2024-01-01",
                "email" => "user@example.com",
                "uuid" => "00000000-0000-0000-0000-000000000000",
                "uri" | "url" => "https://example.com",
                "binary" => "",
                _ => "string",
            }),
            _ => Value::Null,
        }
    }

    /// The example value of a parameter, as text.
    fn param_value(&self, p: &Value) -> String {
        if let Some(v) = p.get("example") {
            return plain(v);
        }
        if let Some(first) = p
            .get("examples")
            .and_then(Value::as_object)
            .and_then(|m| m.values().next())
        {
            return plain(self.resolve(first).get("value").unwrap_or(&Value::Null));
        }
        // Swagger 2 keeps type and default on the parameter itself.
        let schema = p.get("schema").unwrap_or(p);
        let schema = self.resolve(schema);
        match schema
            .get("default")
            .or_else(|| schema.get("example"))
            .or_else(|| schema.get("enum").and_then(|e| e.as_array()?.first()))
        {
            Some(v) => plain(v),
            None => String::new(),
        }
    }

    /// `(base URL, every server as (name, URL))`.
    fn servers(&self) -> (String, Vec<(String, String)>) {
        if self.swagger {
            let host = text(self.doc, "host");
            let base = text(self.doc, "basePath");
            let scheme = self
                .doc
                .get("schemes")
                .and_then(|s| s.as_array()?.first()?.as_str())
                .unwrap_or("https");
            let url = if host.is_empty() {
                format!("http://localhost{base}")
            } else {
                format!("{scheme}://{host}{base}")
            };
            return (url.trim_end_matches('/').to_owned(), Vec::new());
        }
        let servers: Vec<(String, String)> = self
            .doc
            .get("servers")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let mut url = text(s, "url");
                        if let Some(vars) = s.get("variables").and_then(Value::as_object) {
                            for (name, var) in vars {
                                url = url.replace(&format!("{{{name}}}"), &plain(&var["default"]));
                            }
                        }
                        let name = match text(s, "description") {
                            d if d.is_empty() => format!("server {}", i + 1),
                            d => d,
                        };
                        (name, url.trim_end_matches('/').to_owned())
                    })
                    .collect()
            })
            .unwrap_or_default();
        let base = servers
            .first()
            .map(|(_, u)| u.clone())
            .unwrap_or_else(|| "http://localhost".into());
        (base, servers)
    }

    fn schemes(&self) -> Option<&'a Map<String, Value>> {
        let at = if self.swagger {
            "/securityDefinitions"
        } else {
            "/components/securitySchemes"
        };
        self.doc.pointer(at).and_then(Value::as_object)
    }
}

pub fn convert(source: &str) -> Result<Found, ImportError> {
    let doc = parse(source).filter(is_openapi).ok_or_else(|| {
        ImportError::Unsupported("This is not an OpenAPI or Swagger document.".into())
    })?;
    let spec = Spec {
        doc: &doc,
        swagger: doc.get("swagger").is_some(),
    };
    let title = doc
        .pointer("/info/title")
        .and_then(Value::as_str)
        .filter(|t| !t.trim().is_empty())
        .unwrap_or("OpenAPI")
        .to_owned();
    let mut report = Report {
        collection: title.clone(),
        ..Report::default()
    };
    let mut vars = VarMap::new();
    let (base, servers) = spec.servers();
    vars.insert("baseUrl".into(), base);

    // Credentials stay empty variables; nobody's secrets are in a spec.
    let auth_of = |requirement: &Value, report: &mut Report, vars: &mut VarMap| -> Auth {
        let Some(name) = requirement
            .as_object()
            .and_then(|m| m.keys().next())
            .cloned()
        else {
            return Auth::None;
        };
        let Some(scheme) = spec
            .schemes()
            .and_then(|s| s.get(&name))
            .map(|s| spec.resolve(s))
        else {
            report.warn("", format!("Security scheme \"{name}\" is not defined."));
            return Auth::Inherit;
        };
        let mut var = |key: &str| {
            vars.entry(key.to_owned()).or_default();
            format!("{{{{{key}}}}}")
        };
        match (
            text(scheme, "type").as_str(),
            text(scheme, "scheme").to_ascii_lowercase().as_str(),
        ) {
            ("http", "bearer") => Auth::Bearer {
                token: var("bearerToken"),
            },
            ("http", "basic") | ("basic", _) => Auth::Basic {
                username: var("username"),
                password: var("password"),
            },
            ("apiKey", _) => match text(scheme, "in").as_str() {
                "cookie" => {
                    report.warn(
                        "",
                        format!("API key \"{name}\" goes in a cookie; set it as a header instead."),
                    );
                    Auth::None
                }
                place => Auth::ApiKey {
                    key: text(scheme, "name"),
                    value: var("apiKey"),
                    location: if place == "query" {
                        ApiKeyLocation::Query
                    } else {
                        ApiKeyLocation::Header
                    },
                },
            },
            ("oauth2", _) => {
                let flows = if spec.swagger {
                    // Swagger 2: one flow on the scheme itself.
                    let flow = match text(scheme, "flow").as_str() {
                        "application" => "clientCredentials".to_owned(),
                        "accessCode" => "authorizationCode".to_owned(),
                        other => other.to_owned(),
                    };
                    json!({ flow: scheme })
                } else {
                    scheme.get("flows").cloned().unwrap_or_default()
                };
                let (grant, flow) = if let Some(f) = flows.get("clientCredentials") {
                    (OAuth2Grant::ClientCredentials, f.clone())
                } else if let Some(f) = flows.get("authorizationCode") {
                    (OAuth2Grant::AuthorizationCode, f.clone())
                } else if let Some(f) = flows.get("password") {
                    (OAuth2Grant::Password, f.clone())
                } else {
                    report.warn("", "The OAuth 2.0 implicit flow is not supported; use authorization code with PKCE.");
                    (
                        OAuth2Grant::AuthorizationCode,
                        flows.get("implicit").cloned().unwrap_or_default(),
                    )
                };
                let scope = flow
                    .get("scopes")
                    .and_then(Value::as_object)
                    .map(|s| s.keys().cloned().collect::<Vec<_>>().join(" "))
                    .unwrap_or_default();
                Auth::OAuth2(OAuth2 {
                    grant,
                    token_url: text(&flow, "tokenUrl"),
                    auth_url: text(&flow, "authorizationUrl"),
                    client_id: var("clientId"),
                    client_secret: var("clientSecret"),
                    scope,
                    pkce: true,
                    ..OAuth2::default()
                })
            }
            (kind, _) => {
                report.warn(
                    "",
                    format!("{kind} security (\"{name}\") is not supported yet."),
                );
                Auth::None
            }
        }
    };

    let global = doc
        .get("security")
        .and_then(|s| s.as_array()?.first().cloned());
    let collection_auth = match &global {
        Some(req) => auth_of(req, &mut report, &mut vars),
        None => Auth::Inherit,
    };

    let mut files = Vec::new();
    let mut used: BTreeMap<PathBuf, HashSet<String>> = BTreeMap::new();
    let mut folders = HashSet::new();
    let paths = doc
        .get("paths")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (path, item) in &paths {
        let item = spec.resolve(item);
        let shared = item
            .get("parameters")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for (key, method) in METHODS {
            let Some(op) = item.get(key) else { continue };
            let label = format!("{} {path}", method.as_str());
            let name = [text(op, "summary"), text(op, "operationId")]
                .into_iter()
                .find(|n| !n.trim().is_empty())
                .unwrap_or_else(|| label.clone());

            // Path parameters become variables: /users/{id} → /users/{{id}}.
            let mut url = format!("{{{{baseUrl}}}}{path}");
            let mut request = Request::new(method, String::new());
            request.name = name.trim().to_owned();
            request.description = text(op, "description");

            let mut params: Vec<Value> = shared.clone();
            for p in op
                .get("parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let p = spec.resolve(p);
                params.retain(|q| {
                    let q = spec.resolve(q);
                    !(text(q, "name") == text(p, "name") && text(q, "in") == text(p, "in"))
                });
                params.push(p.clone());
            }
            let mut form: Vec<FormPart> = Vec::new();
            for p in &params {
                let p = spec.resolve(p);
                let pname = text(p, "name");
                let required = p.get("required") == Some(&Value::Bool(true));
                match text(p, "in").as_str() {
                    "path" => {
                        url = url.replace(&format!("{{{pname}}}"), &format!("{{{{{pname}}}}}"));
                        let value = spec.param_value(p);
                        vars.entry(pname).or_insert(value);
                    }
                    "query" => request.query.push(KeyValue {
                        enabled: required,
                        ..KeyValue::new(pname, spec.param_value(p))
                    }),
                    "header" => request.headers.push(KeyValue {
                        enabled: required,
                        ..KeyValue::new(pname, spec.param_value(p))
                    }),
                    "cookie" => report.warn(
                        &label,
                        format!("Cookie parameter \"{pname}\" was left out."),
                    ),
                    // Swagger 2 bodies.
                    "body" => {
                        let sample = spec.sample(p.get("schema").unwrap_or(&Value::Null), 0);
                        request.body =
                            Body::Json(serde_json::to_string_pretty(&sample).unwrap_or_default());
                    }
                    "formData" => form.push(if text(p, "type") == "file" {
                        FormPart::file(pname, String::new())
                    } else {
                        FormPart::text(pname, spec.param_value(p))
                    }),
                    _ => {}
                }
            }
            if !form.is_empty() {
                request.body = if form.iter().any(|f| f.file) {
                    Body::Multipart(form)
                } else {
                    Body::Form(
                        form.into_iter()
                            .map(|f| KeyValue::new(f.key, f.value))
                            .collect(),
                    )
                };
            }
            if let Some(body) = op.get("requestBody").map(|b| spec.resolve(b)) {
                request.body = request_body(&spec, body, &label, &mut report);
            }
            request.url = url;

            // Operation security: [] turns auth off; a different scheme is set here.
            request.auth = match op.get("security").and_then(Value::as_array) {
                Some(list) if list.is_empty() => Auth::None,
                Some(list) => match list.first() {
                    Some(req) if Some(req) != global.as_ref() => {
                        auth_of(req, &mut report, &mut vars)
                    }
                    _ => Auth::Inherit,
                },
                None => Auth::Inherit,
            };
            if op.get("callbacks").is_some() {
                report.warn(&label, "Callbacks were left out.");
            }

            let dir = match op.get("tags").and_then(|t| t.as_array()?.first()?.as_str()) {
                Some(tag) => {
                    let dir = PathBuf::from(slug(tag, "folder"));
                    if folders.insert(dir.clone()) {
                        report.folders += 1;
                    }
                    dir
                }
                None => PathBuf::new(),
            };
            let stem = unique(
                slug(&request.name, "request"),
                used.entry(dir.clone()).or_default(),
            );
            files.push(PlannedFile::Request {
                path: dir.join(request_file(&stem)),
                request,
            });
            report.requests += 1;
        }
    }
    if doc.get("webhooks").is_some() {
        report.warn(
            "",
            "Webhooks were left out (they are calls to you, not from you).",
        );
    }
    let credentials: Vec<&str> = [
        "bearerToken",
        "apiKey",
        "username",
        "password",
        "clientId",
        "clientSecret",
    ]
    .into_iter()
    .filter(|k| vars.get(*k).is_some_and(String::is_empty))
    .collect();
    if !credentials.is_empty() {
        report.warn(
            "",
            format!(
                "Fill in {} in an environment and mark them secret.",
                credentials.join(", ")
            ),
        );
    }
    report.variables = vars.len();

    let environments = if servers.len() > 1 {
        servers
            .iter()
            .map(|(name, url)| EnvironmentPlan {
                title: name.clone(),
                name: slug(name, "environment"),
                vars: [("baseUrl".to_owned(), url.clone())].into(),
                secrets: VarMap::new(),
                skipped_disabled: 0,
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(Found {
        collections: vec![(
            String::new(),
            Plan {
                meta: CollectionMeta {
                    name: title,
                    description: doc
                        .pointer("/info/description")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    vars,
                    auth: collection_auth,
                    ..CollectionMeta::default()
                },
                files,
                report,
            },
        )],
        environments,
        ..Found::default()
    })
}

/// The body of an OpenAPI 3 operation, in the first type Apix can send.
fn request_body(spec: &Spec, body: &Value, label: &str, report: &mut Report) -> Body {
    let Some(content) = body.get("content").and_then(Value::as_object) else {
        return Body::None;
    };
    let pick = |test: &dyn Fn(&str) -> bool| {
        content
            .iter()
            .find(|(ct, _)| test(&ct.to_ascii_lowercase()))
    };
    let chosen = pick(&|ct| ct.contains("json"))
        .or_else(|| pick(&|ct| ct.contains("x-www-form-urlencoded")))
        .or_else(|| pick(&|ct| ct.starts_with("multipart/")))
        .or_else(|| pick(&|ct| ct.contains("xml")))
        .or_else(|| pick(&|ct| ct.starts_with("text/")))
        .or_else(|| content.iter().next());
    let Some((ct, media)) = chosen else {
        return Body::None;
    };
    let ct = ct.to_ascii_lowercase();
    let schema = media.get("schema").unwrap_or(&Value::Null);
    let example = media.get("example").cloned().or_else(|| {
        media
            .get("examples")
            .and_then(Value::as_object)
            .and_then(|m| m.values().next())
            .and_then(|e| spec.resolve(e).get("value").cloned())
    });
    let value = example.unwrap_or_else(|| spec.sample(schema, 0));
    let props = || {
        spec.resolve(schema)
            .get("properties")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
    };
    if ct.contains("json") {
        Body::Json(serde_json::to_string_pretty(&value).unwrap_or_default())
    } else if ct.contains("x-www-form-urlencoded") {
        Body::Form(
            props()
                .iter()
                .map(|(k, v)| {
                    KeyValue::new(
                        k.clone(),
                        plain(&value.get(k).cloned().unwrap_or_else(|| spec.sample(v, 1))),
                    )
                })
                .collect(),
        )
    } else if ct.starts_with("multipart/") {
        Body::Multipart(
            props()
                .iter()
                .map(|(k, v)| {
                    let v = spec.resolve(v);
                    if text(v, "format") == "binary" || text(v, "type") == "file" {
                        FormPart::file(k.clone(), String::new())
                    } else {
                        FormPart::text(k.clone(), plain(&spec.sample(v, 1)))
                    }
                })
                .collect(),
        )
    } else if ct.contains("xml") {
        match value {
            Value::String(xml) => Body::Xml(xml),
            _ => {
                report.warn(label, "The XML body has no example; write it in Apix.");
                Body::Xml(String::new())
            }
        }
    } else if ct == "application/octet-stream" {
        Body::Binary {
            file: String::new(),
        }
    } else {
        Body::Text(plain(&value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PETSTORE: &str = r#"
openapi: 3.0.3
info:
  title: Pet Store
  description: Sample shop
servers:
  - url: https://{region}.api.pets.dev/v1
    description: Production
    variables:
      region:
        default: eu
  - url: http://localhost:8080/v1
    description: Local
security:
  - bearerAuth: []
paths:
  /pets:
    get:
      tags: [pets]
      summary: List pets
      parameters:
        - name: limit
          in: query
          schema: { type: integer, default: 20 }
        - name: X-Trace
          in: header
          required: true
          schema: { type: string }
      responses:
        200: { description: ok }
    post:
      tags: [pets]
      operationId: createPet
      requestBody:
        content:
          application/json:
            schema: { $ref: '#/components/schemas/NewPet' }
      responses:
        201: { description: created }
  /pets/{petId}:
    parameters:
      - name: petId
        in: path
        required: true
        schema: { type: string }
        example: p-42
    delete:
      tags: [pets]
      summary: Delete pet
      security:
        - apiKey: []
      responses:
        204: { description: gone }
  /health:
    get:
      summary: Health
      security: []
      responses:
        200: { description: ok }
components:
  securitySchemes:
    bearerAuth: { type: http, scheme: bearer }
    apiKey: { type: apiKey, in: header, name: X-API-Key }
  schemas:
    NewPet:
      type: object
      required: [name]
      properties:
        id: { type: string, format: uuid, readOnly: true }
        name: { type: string, example: Rex }
        born: { type: string, format: date }
        tags:
          type: array
          items: { type: string }
        owner: { $ref: '#/components/schemas/Owner' }
    Owner:
      allOf:
        - type: object
          properties:
            email: { type: string, format: email }
        - type: object
          properties:
            vip: { type: boolean }
"#;

    fn requests(plan: &Plan) -> Vec<(String, &Request)> {
        plan.files
            .iter()
            .filter_map(|f| match f {
                PlannedFile::Request { path, request } => {
                    Some((path.to_string_lossy().replace('\\', "/"), request))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn converts_an_openapi_3_yaml_spec() {
        let found = convert(PETSTORE).unwrap();
        let plan = &found.collections[0].1;
        assert_eq!(plan.meta.name, "Pet Store");
        assert_eq!(plan.meta.vars["baseUrl"], "https://eu.api.pets.dev/v1");
        assert_eq!(plan.meta.vars["petId"], "p-42");
        assert_eq!(plan.meta.vars["bearerToken"], "");
        assert_eq!(
            plan.meta.auth,
            Auth::Bearer {
                token: "{{bearerToken}}".into()
            }
        );

        let reqs = requests(plan);
        let paths: Vec<&str> = reqs.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            [
                "pets/list-pets.apix",
                "pets/createpet.apix",
                "pets/delete-pet.apix",
                "health.apix"
            ]
        );
        let health = reqs[3].1;
        assert_eq!(health.auth, Auth::None, "security: [] turns auth off");

        let list = reqs[0].1;
        assert_eq!(list.url, "{{baseUrl}}/pets");
        assert_eq!(
            list.query,
            [KeyValue {
                enabled: false,
                ..KeyValue::new("limit", "20")
            }]
        );
        assert_eq!(list.headers, [KeyValue::new("X-Trace", "")]);
        assert_eq!(list.auth, Auth::Inherit);

        let Body::Json(body) = &reqs[1].1.body else {
            panic!("{:?}", reqs[1].1.body)
        };
        let body: Value = serde_json::from_str(body).unwrap();
        assert_eq!(
            body,
            json!({
                "name": "Rex",
                "born": "2024-01-01",
                "tags": ["string"],
                "owner": { "email": "user@example.com", "vip": false }
            }),
            "readOnly id left out, $ref and allOf followed"
        );

        let delete = reqs[2].1;
        assert_eq!(delete.url, "{{baseUrl}}/pets/{{petId}}");
        assert!(matches!(&delete.auth, Auth::ApiKey { key, .. } if key == "X-API-Key"));

        assert_eq!(found.environments.len(), 2, "one per server");
        assert_eq!(
            found.environments[1].vars["baseUrl"],
            "http://localhost:8080/v1"
        );
        assert!(
            plan.report.to_string().contains("Fill in"),
            "{}",
            plan.report
        );
    }

    #[test]
    fn converts_a_swagger_2_json_spec() {
        let spec = r#"{
          "swagger": "2.0",
          "info": { "title": "Old API" },
          "host": "old.dev", "basePath": "/api", "schemes": ["http"],
          "securityDefinitions": {
            "oauth": { "type": "oauth2", "flow": "application", "tokenUrl": "https://old.dev/token",
                       "scopes": { "read": "", "write": "" } }
          },
          "security": [ { "oauth": ["read"] } ],
          "paths": {
            "/items": {
              "post": {
                "summary": "Add item",
                "parameters": [ { "in": "body", "name": "item",
                  "schema": { "type": "object", "properties": { "n": { "type": "integer" } } } } ]
              }
            },
            "/upload": {
              "post": {
                "summary": "Upload",
                "consumes": ["multipart/form-data"],
                "parameters": [ { "in": "formData", "name": "file", "type": "file" },
                                { "in": "formData", "name": "note", "type": "string", "default": "hi" } ]
              }
            }
          }
        }"#;
        let found = convert(spec).unwrap();
        let plan = &found.collections[0].1;
        assert_eq!(plan.meta.vars["baseUrl"], "http://old.dev/api");
        let Auth::OAuth2(o) = &plan.meta.auth else {
            panic!("{:?}", plan.meta.auth)
        };
        assert_eq!(o.grant, OAuth2Grant::ClientCredentials);
        assert_eq!(o.token_url, "https://old.dev/token");
        assert_eq!(o.scope, "read write");
        let reqs = requests(plan);
        assert_eq!(reqs[0].1.body, Body::Json("{\n  \"n\": 0\n}".into()));
        assert_eq!(
            reqs[1].1.body,
            Body::Multipart(vec![
                FormPart::file("file", ""),
                FormPart::text("note", "hi")
            ])
        );
        assert!(found.environments.is_empty());
    }

    #[test]
    fn rejects_other_documents() {
        assert!(convert("name: not a spec").is_err());
        assert!(parse("{ broken").is_none());
    }
}
