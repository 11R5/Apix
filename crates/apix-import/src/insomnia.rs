//! Insomnia v4 export (JSON, "Export data › Insomnia v4") → one collection
//! per workspace. Request groups become folders, the base environment's
//! values become collection variables and each sub-environment an Apix
//! environment. Insomnia's `{{ _.name }}` becomes `{{name}}`; nested
//! environment values are flattened to `a.b`.

use std::collections::HashSet;
use std::path::Path;

use apix_core::store::{CollectionMeta, FolderMeta, VarMap};
use apix_core::{
    ApiKeyLocation, Auth, AwsSigV4, Body, FormPart, GraphQl, KeyValue, Method, OAuth1,
    OAuth1Method, OAuth2, OAuth2Grant, Request,
};
use serde_json::Value;

use crate::postman::EnvironmentPlan;
use crate::{Found, ImportError, Plan, PlannedFile, Report, request_file, slug, unique};

/// Whether `value` is an Insomnia v4 export.
pub(crate) fn is_insomnia(value: &Value) -> bool {
    value.get("_type").and_then(Value::as_str) == Some("export")
        && value.get("resources").is_some_and(Value::is_array)
}

fn s(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(t)) => t.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

fn flag(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// `{{ _.base_url }}` and `{{_.token}}` → `{{base_url}}`, `{{token}}`.
/// Template tags (`{% response … %}`) are left as they are and reported.
fn vars(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let inner = after[..end].trim();
                let name = inner.strip_prefix("_.").unwrap_or(inner).trim();
                out.push_str("{{");
                out.push_str(name);
                out.push_str("}}");
                rest = &after[end + 2..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Environment data as flat variables: `{"api": {"url": "x"}}` → `api.url = x`.
fn flatten(prefix: &str, value: &Value, out: &mut VarMap) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&key, v, out);
            }
        }
        Value::String(t) => {
            out.insert(prefix.to_owned(), vars(t));
        }
        Value::Null => {
            out.insert(prefix.to_owned(), String::new());
        }
        other => {
            out.insert(prefix.to_owned(), other.to_string());
        }
    }
}

fn kv_rows(list: Option<&Value>) -> Vec<KeyValue> {
    list.and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter(|r| !s(r, "name").is_empty())
                .map(|r| KeyValue {
                    key: vars(&s(r, "name")),
                    value: vars(&s(r, "value")),
                    enabled: !flag(r, "disabled"),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn convert(json: &str) -> Result<Found, ImportError> {
    let value: Value = serde_json::from_str(json)?;
    if !is_insomnia(&value) {
        return Err(ImportError::Unsupported(
            "This is not an Insomnia v4 export.".into(),
        ));
    }
    let resources = value["resources"].as_array().cloned().unwrap_or_default();
    let children = |parent: &str| -> Vec<&Value> {
        let mut list: Vec<&Value> = resources
            .iter()
            .filter(|r| s(r, "parentId") == parent)
            .collect();
        // Insomnia orders siblings by metaSortKey.
        list.sort_by(|a, b| {
            let key = |v: &Value| v.get("metaSortKey").and_then(Value::as_f64).unwrap_or(0.0);
            key(a)
                .partial_cmp(&key(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        list
    };

    let mut found = Found::default();
    for workspace in resources.iter().filter(|r| s(r, "_type") == "workspace") {
        let id = s(workspace, "_id");
        let name = s(workspace, "name");
        let mut ctx = Converter {
            report: Report {
                collection: name.clone(),
                ..Report::default()
            },
            files: Vec::new(),
        };
        ctx.items(&children, &id, Path::new(""), "");

        // The base environment holds collection variables; its children are environments.
        let mut collection_vars = VarMap::new();
        for base in children(&id)
            .into_iter()
            .filter(|r| s(r, "_type") == "environment")
        {
            if let Some(data) = base.get("data") {
                flatten("", data, &mut collection_vars);
            }
            for sub in children(&s(base, "_id"))
                .into_iter()
                .filter(|r| s(r, "_type") == "environment")
            {
                let mut env_vars = VarMap::new();
                if let Some(data) = sub.get("data") {
                    flatten("", data, &mut env_vars);
                }
                let title = s(sub, "name");
                found.environments.push(EnvironmentPlan {
                    name: slug(&title, "environment"),
                    title,
                    vars: env_vars,
                    secrets: VarMap::new(),
                    skipped_disabled: 0,
                });
            }
        }
        ctx.report.variables = collection_vars.len();
        if resources
            .iter()
            .any(|r| s(r, "_type") == "cookie_jar" && s(r, "parentId") == id)
        {
            ctx.report.warn(
                "",
                "The cookie jar was not imported; Apix keeps cookies from responses itself.",
            );
        }
        found.collections.push((
            String::new(),
            Plan {
                meta: CollectionMeta {
                    name: if name.is_empty() {
                        "Insomnia".into()
                    } else {
                        name
                    },
                    description: s(workspace, "description"),
                    vars: collection_vars,
                    ..CollectionMeta::default()
                },
                files: ctx.files,
                report: ctx.report,
            },
        ));
    }
    if found.collections.is_empty() {
        return Err(ImportError::Unsupported(
            "This Insomnia export holds no workspace.".into(),
        ));
    }
    Ok(found)
}

struct Converter {
    report: Report,
    files: Vec<PlannedFile>,
}

impl Converter {
    fn items<'a>(
        &mut self,
        children: &dyn Fn(&str) -> Vec<&'a Value>,
        parent: &str,
        dir: &Path,
        trail: &str,
    ) {
        let mut used = HashSet::new();
        for item in children(parent) {
            let name = s(item, "name");
            let label = if trail.is_empty() {
                name.clone()
            } else {
                format!("{trail} / {name}")
            };
            match s(item, "_type").as_str() {
                "request_group" => {
                    let sub = dir.join(unique(slug(&name, "folder"), &mut used));
                    self.report.folders += 1;
                    let auth = self.auth(item.get("authentication"), &label);
                    if !auth.is_inherit() {
                        self.files.push(PlannedFile::Folder {
                            path: sub.clone(),
                            meta: FolderMeta {
                                auth,
                                ..FolderMeta::default()
                            },
                        });
                    }
                    self.items(children, &s(item, "_id"), &sub, &label);
                }
                "request" => {
                    let stem = unique(slug(&name, "request"), &mut used);
                    let request = self.request(item, &label);
                    self.files.push(PlannedFile::Request {
                        path: dir.join(request_file(&stem)),
                        request,
                    });
                    self.report.requests += 1;
                }
                "grpc_request" | "websocket_request" => {
                    self.report.warn(
                        &label,
                        format!("{} requests are not supported yet.", s(item, "_type")),
                    );
                }
                _ => {}
            }
        }
    }

    fn request(&mut self, item: &Value, label: &str) -> Request {
        let method = s(item, "method");
        let mut out = Request::new(
            method.parse().unwrap_or_else(|_| {
                self.report.warn(
                    label,
                    format!("Method {method} is not supported yet; imported as GET."),
                );
                Method::Get
            }),
            vars(&s(item, "url")),
        );
        out.name = s(item, "name");
        out.description = s(item, "description");
        out.query = kv_rows(item.get("parameters"));
        out.headers = kv_rows(item.get("headers"));
        if let Some(body) = item.get("body") {
            out.body = self.body(body, label);
            if matches!(out.body, Body::Multipart(_)) {
                out.headers.retain(|h| {
                    !(h.key.eq_ignore_ascii_case("content-type")
                        && h.value.to_ascii_lowercase().starts_with("multipart/"))
                });
            }
        }
        out.auth = self.auth(item.get("authentication"), label);
        if out.url.contains("{%") || format!("{:?}", out.body).contains("{%") {
            self.report.warn(
                label,
                "Insomnia template tags ({% … %}) were kept as text; replace them with variables.",
            );
        }
        out
    }

    fn body(&mut self, body: &Value, label: &str) -> Body {
        let mime = s(body, "mimeType").to_ascii_lowercase();
        let text = vars(&s(body, "text"));
        let params = || {
            body.get("params")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        if mime.is_empty() && text.is_empty() && params().is_empty() {
            return Body::None;
        }
        if mime.contains("x-www-form-urlencoded") {
            return Body::Form(kv_rows(body.get("params")));
        }
        if mime.starts_with("multipart/") {
            return Body::Multipart(
                params()
                    .iter()
                    .filter(|p| !s(p, "name").is_empty())
                    .map(|p| {
                        let part = if s(p, "type") == "file" {
                            FormPart::file(vars(&s(p, "name")), s(p, "fileName"))
                        } else {
                            FormPart::text(vars(&s(p, "name")), vars(&s(p, "value")))
                        };
                        FormPart {
                            enabled: !flag(p, "disabled"),
                            ..part
                        }
                    })
                    .collect(),
            );
        }
        if mime.contains("graphql") {
            // Insomnia stores {"query", "variables"} as the text.
            let parsed: Value = serde_json::from_str(&text).unwrap_or_default();
            let variables = match parsed.get("variables") {
                None | Some(Value::Null) => String::new(),
                Some(Value::String(v)) => v.clone(),
                Some(v) => serde_json::to_string_pretty(v).unwrap_or_default(),
            };
            return Body::GraphQl(GraphQl {
                query: s(&parsed, "query"),
                variables,
            });
        }
        if mime == "application/octet-stream" || !s(body, "fileName").is_empty() {
            let file = s(body, "fileName");
            if file.is_empty() {
                self.report.warn(
                    label,
                    "The binary body has no file chosen; pick one in Apix.",
                );
            }
            return Body::Binary { file };
        }
        if mime.contains("json") {
            Body::Json(text)
        } else if mime.contains("html") {
            Body::Html(text)
        } else if mime.contains("xml") {
            Body::Xml(text)
        } else {
            Body::Text(text)
        }
    }

    /// Insomnia's `authentication`; an empty object means none set (inherit).
    fn auth(&mut self, auth: Option<&Value>, label: &str) -> Auth {
        let Some(a) = auth.filter(|a| a.as_object().is_some_and(|m| !m.is_empty())) else {
            return Auth::Inherit;
        };
        if flag(a, "disabled") {
            return Auth::None;
        }
        let v = |key: &str| vars(&s(a, key));
        match s(a, "type").as_str() {
            "none" | "" => Auth::None,
            "bearer" => Auth::Bearer { token: v("token") },
            "basic" => Auth::Basic {
                username: v("username"),
                password: v("password"),
            },
            "digest" => Auth::Digest {
                username: v("username"),
                password: v("password"),
            },
            "apikey" => Auth::ApiKey {
                key: v("key"),
                value: v("value"),
                location: if s(a, "addTo") == "queryParams" {
                    ApiKeyLocation::Query
                } else {
                    ApiKeyLocation::Header
                },
            },
            "iam" => Auth::AwsSigV4(AwsSigV4 {
                access_key: v("accessKeyId"),
                secret_key: v("secretAccessKey"),
                session_token: v("sessionToken"),
                region: v("region"),
                service: v("service"),
            }),
            "oauth1" => Auth::OAuth1(OAuth1 {
                consumer_key: v("consumerKey"),
                consumer_secret: v("consumerSecret"),
                token: v("tokenKey"),
                token_secret: v("tokenSecret"),
                signature_method: OAuth1Method::ALL
                    .into_iter()
                    .find(|m| m.as_str().eq_ignore_ascii_case(&s(a, "signatureMethod")))
                    .unwrap_or(OAuth1Method::HmacSha1),
                realm: v("realm"),
                location: ApiKeyLocation::Header,
            }),
            "oauth2" => {
                let grant = match s(a, "grantType").as_str() {
                    "client_credentials" => OAuth2Grant::ClientCredentials,
                    "password" => OAuth2Grant::Password,
                    "authorization_code" => OAuth2Grant::AuthorizationCode,
                    other => {
                        self.report.warn(
                            label,
                            format!("The OAuth 2.0 grant \"{other}\" is not supported; authorization code with PKCE is used."),
                        );
                        OAuth2Grant::AuthorizationCode
                    }
                };
                Auth::OAuth2(OAuth2 {
                    grant,
                    token_url: v("accessTokenUrl"),
                    auth_url: v("authorizationUrl"),
                    client_id: v("clientId"),
                    client_secret: v("clientSecret"),
                    scope: v("scope"),
                    username: v("username"),
                    password: v("password"),
                    redirect_uri: String::new(),
                    pkce: true,
                    ..OAuth2::default()
                })
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const EXPORT: &str = r#"{
      "_type": "export", "__export_format": 4, "__export_source": "insomnia.desktop.app:v8.6.1",
      "resources": [
        { "_id": "wrk_1", "_type": "workspace", "parentId": null, "name": "Shop API" },
        { "_id": "env_base", "_type": "environment", "parentId": "wrk_1", "name": "Base",
          "data": { "base_url": "https://api.shop.dev", "api": { "version": "v1" } } },
        { "_id": "env_dev", "_type": "environment", "parentId": "env_base", "name": "Dev",
          "data": { "base_url": "http://localhost:8080", "token": "dev-token" } },
        { "_id": "jar_1", "_type": "cookie_jar", "parentId": "wrk_1", "cookies": [] },
        { "_id": "fld_1", "_type": "request_group", "parentId": "wrk_1", "name": "Users",
          "metaSortKey": 1, "authentication": { "type": "bearer", "token": "{{ _.token }}" } },
        { "_id": "req_2", "_type": "request", "parentId": "fld_1", "name": "Create user",
          "metaSortKey": 2, "method": "POST", "url": "{{ _.base_url }}/{{_.api.version}}/users",
          "headers": [ { "name": "Content-Type", "value": "application/json" },
                       { "name": "X-Debug", "value": "1", "disabled": true } ],
          "parameters": [], "authentication": {},
          "body": { "mimeType": "application/json", "text": "{\"name\": \"{{ _.user }}\"}" } },
        { "_id": "req_1", "_type": "request", "parentId": "fld_1", "name": "List users",
          "metaSortKey": 1, "method": "GET", "url": "{{ _.base_url }}/users",
          "parameters": [ { "name": "page", "value": "2" } ], "headers": [], "body": {},
          "authentication": { "type": "apikey", "key": "X-Key", "value": "k", "addTo": "queryParams" } },
        { "_id": "req_3", "_type": "request", "parentId": "wrk_1", "name": "Login",
          "metaSortKey": 3, "method": "POST", "url": "{{ _.base_url }}/login", "headers": [],
          "authentication": { "type": "basic", "username": "ada", "password": "pw" },
          "body": { "mimeType": "application/x-www-form-urlencoded",
                    "params": [ { "name": "remember", "value": "true" } ] } },
        { "_id": "req_4", "_type": "request", "parentId": "wrk_1", "name": "Query",
          "metaSortKey": 4, "method": "POST", "url": "{{ _.base_url }}/graphql", "headers": [],
          "authentication": {}, "body": { "mimeType": "application/graphql",
          "text": "{\"query\":\"{ me { id } }\",\"variables\":{\"a\":1}}" } },
        { "_id": "grpc_1", "_type": "grpc_request", "parentId": "wrk_1", "name": "Stream" }
      ]
    }"#;

    fn requests(plan: &Plan) -> Vec<(&PathBuf, &Request)> {
        plan.files
            .iter()
            .filter_map(|f| match f {
                PlannedFile::Request { path, request } => Some((path, request)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn converts_workspace_folders_requests_and_environments() {
        let found = convert(EXPORT).unwrap();
        assert_eq!(found.collections.len(), 1);
        let plan = &found.collections[0].1;
        assert_eq!(plan.meta.name, "Shop API");
        assert_eq!(
            plan.meta.vars.get("base_url").map(String::as_str),
            Some("https://api.shop.dev")
        );
        assert_eq!(
            plan.meta.vars.get("api.version").map(String::as_str),
            Some("v1")
        );

        let reqs = requests(plan);
        let paths: Vec<_> = reqs
            .iter()
            .map(|(p, _)| p.to_string_lossy().replace('\\', "/"))
            .collect();
        assert_eq!(
            paths,
            [
                "users/list-users.apix",
                "users/create-user.apix",
                "login.apix",
                "query.apix"
            ],
            "sorted by metaSortKey"
        );
        let list = reqs[0].1;
        assert_eq!(list.url, "{{base_url}}/users");
        assert_eq!(list.query, [KeyValue::new("page", "2")]);
        assert!(matches!(
            &list.auth,
            Auth::ApiKey {
                location: ApiKeyLocation::Query,
                ..
            }
        ));

        let create = reqs[1].1;
        assert_eq!(create.url, "{{base_url}}/{{api.version}}/users");
        assert_eq!(
            create.auth,
            Auth::Inherit,
            "an empty authentication inherits"
        );
        assert_eq!(create.body, Body::Json("{\"name\": \"{{user}}\"}".into()));
        assert!(!create.headers[1].enabled);

        assert_eq!(
            reqs[2].1.auth,
            Auth::Basic {
                username: "ada".into(),
                password: "pw".into()
            }
        );
        assert_eq!(
            reqs[2].1.body,
            Body::Form(vec![KeyValue::new("remember", "true")])
        );
        assert_eq!(
            reqs[3].1.body,
            Body::GraphQl(GraphQl {
                query: "{ me { id } }".into(),
                variables: "{\n  \"a\": 1\n}".into()
            })
        );

        // The folder's bearer token stays on the folder.
        let folder = plan.files.iter().find_map(|f| match f {
            PlannedFile::Folder { path, meta } => Some((path, meta)),
            _ => None,
        });
        let (path, meta) = folder.unwrap();
        assert_eq!(path, &PathBuf::from("users"));
        assert_eq!(
            meta.auth,
            Auth::Bearer {
                token: "{{token}}".into()
            }
        );

        assert_eq!(found.environments.len(), 1);
        assert_eq!(found.environments[0].name, "dev");
        assert_eq!(
            found.environments[0].vars.get("token").map(String::as_str),
            Some("dev-token")
        );
        let notes = plan.report.to_string();
        assert!(
            notes.contains("grpc_request requests are not supported"),
            "{notes}"
        );
        assert!(notes.contains("cookie jar"), "{notes}");
    }

    #[test]
    fn rewrites_insomnia_variables() {
        assert_eq!(
            vars("{{ _.a }}/{{_.b.c}}/{{ d }}/{{"),
            "{{a}}/{{b.c}}/{{d}}/{{"
        );
        assert!(convert(r#"{"_type":"export","resources":[]}"#).is_err());
    }
}
