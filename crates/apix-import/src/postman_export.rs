//! Apix collection → Postman Collection v2.1 and Postman environments, so a
//! teammate can keep using Postman. The mapping mirrors the importer, so a
//! collection exported and imported again comes back the same.
//!
//! Nothing is dropped silently: what Postman cannot hold (a per-request
//! timeout, proxy or client certificate) is listed in `Export::notes`, and
//! secret environment values are exported with their names only.

use std::path::{Path, PathBuf};

use apix_core::store::{self, Collection, EnvironmentParts, TreeNode, VarMap};
use apix_core::{
    ApiKeyLocation, Auth, Body, ClientAuth, KeyValue, OAuth2Grant, Project, Request,
    RequestSettings, vfs,
};
use serde_json::{Map, Value, json};

use crate::{ImportError, io_err};

const SCHEMA: &str = "https://schema.getpostman.com/json/collection/v2.1.0/collection.json";

/// A collection ready to be written as Postman files.
#[derive(Debug, Clone)]
pub struct Export {
    pub name: String,
    /// The `.postman_collection.json` text.
    pub collection: String,
    /// `(name, .postman_environment.json text)`.
    pub environments: Vec<(String, String)>,
    /// What could not be exported exactly.
    pub notes: Vec<String>,
}

impl Export {
    /// Writes `<name>.postman_collection.json` and one
    /// `<name>.<env>.postman_environment.json` per environment into `dir`,
    /// never replacing a file that is already there.
    pub fn write(&self, dir: &Path) -> Result<Vec<PathBuf>, ImportError> {
        std::fs::create_dir_all(dir).map_err(io_err(dir))?;
        let base = store::slug(&self.name, "collection");
        let mut written = Vec::new();
        let mut put = |stem: String, ext: &str, text: &str| -> Result<(), ImportError> {
            let path = store::unique_path(dir, &stem, ext);
            std::fs::write(&path, text).map_err(io_err(&path))?;
            written.push(path);
            Ok(())
        };
        put(base.clone(), "postman_collection.json", &self.collection)?;
        for (env, text) in &self.environments {
            put(
                format!("{base}.{}", store::slug(env, "environment")),
                "postman_environment.json",
                text,
            )?;
        }
        Ok(written)
    }
}

/// Exports the collection at `root`. In a project, the project's
/// environments, variables and auth are folded in, since a Postman
/// collection has nothing above it.
pub fn export_collection(root: &Path, project: Option<&Project>) -> Result<Export, ImportError> {
    let collection = Collection::open(root)?;
    let mut ex = Exporter { notes: Vec::new() };

    let mut auth = collection.meta.auth.clone();
    let mut settings = collection.meta.settings.clone();
    let mut vars = collection.meta.vars.clone();
    if let Some(p) = project {
        if auth.is_inherit() && !p.meta.auth.is_inherit() {
            auth = p.meta.auth.clone();
            ex.notes.push(format!(
                "The auth of project \"{}\" was copied to the collection.",
                p.meta.name
            ));
        }
        settings = settings.or_inherit(&p.meta.settings);
        for (k, v) in &p.meta.vars {
            vars.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }

    let mut info = Map::new();
    info.insert("name".into(), json!(collection.meta.name));
    if !collection.meta.description.is_empty() {
        info.insert("description".into(), json!(collection.meta.description));
    }
    info.insert("schema".into(), json!(SCHEMA));

    let mut out = Map::new();
    out.insert("info".into(), Value::Object(info));
    out.insert("item".into(), json!(ex.items(&collection.tree())?));
    if let Some(auth) = auth_json(&auth) {
        out.insert("auth".into(), auth);
    }
    let events = events(root, "_collection");
    if !events.is_empty() {
        out.insert("event".into(), json!(events));
    }
    if !vars.is_empty() {
        let list: Vec<Value> = vars
            .iter()
            .map(|(k, v)| json!({ "key": k, "value": v }))
            .collect();
        out.insert("variable".into(), json!(list));
    }
    if let Some(ppb) = ex.profile(&settings, "the collection") {
        out.insert("protocolProfileBehavior".into(), ppb);
    }

    // Environments: the collection's, then the project's of other names.
    let mut names: Vec<(String, PathBuf)> = store::environment_names_at(root)
        .into_iter()
        .map(|n| (n, root.to_owned()))
        .collect();
    if let Some(p) = project {
        for n in store::environment_names_at(&p.root) {
            if !names.iter().any(|(have, _)| *have == n) {
                names.push((n, p.root.clone()));
            }
        }
    }
    let mut environments = Vec::new();
    for (name, at) in names {
        let parts = EnvironmentParts::read(&at, &name)?;
        if !parts.secrets.is_empty() {
            ex.notes.push(format!(
                "Environment \"{name}\": secret values were left empty ({}); fill them in Postman.",
                parts.secrets.keys().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        environments.push((
            name.clone(),
            environment_json(&name, &parts.vars, &parts.secrets),
        ));
    }

    Ok(Export {
        name: collection.meta.name.clone(),
        collection: pretty(&Value::Object(out)),
        environments,
        notes: ex.notes,
    })
}

fn pretty(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("a JSON value always serializes");
    text.push('\n');
    text
}

fn environment_json(name: &str, vars: &VarMap, secrets: &VarMap) -> String {
    let mut values: Vec<Value> = vars
        .iter()
        .filter(|(k, _)| !secrets.contains_key(*k))
        .map(|(k, v)| json!({ "key": k, "value": v, "type": "default", "enabled": true }))
        .collect();
    values.extend(
        secrets
            .keys()
            .map(|k| json!({ "key": k, "value": "", "type": "secret", "enabled": true })),
    );
    pretty(&json!({
        "name": name,
        "values": values,
        "_postman_variable_scope": "environment",
    }))
}

/// `prerequest` / `test` events from `<stem>.prerequest.js` / `<stem>.test.js` in `dir`.
fn events(dir: &Path, stem: &str) -> Vec<Value> {
    ["prerequest", "test"]
        .into_iter()
        .filter_map(|kind| {
            let text = vfs::read_to_string(&dir.join(format!("{stem}.{kind}.js"))).ok()?;
            let lines: Vec<&str> = text.lines().collect();
            Some(json!({
                "listen": kind,
                "script": { "type": "text/javascript", "exec": lines },
            }))
        })
        .collect()
}

fn params(kind: &str, pairs: &[(&str, Value)]) -> Value {
    let list: Vec<Value> = pairs
        .iter()
        .map(|(k, v)| json!({ "key": k, "value": v, "type": "string" }))
        .collect();
    json!({ "type": kind, kind: list })
}

/// Postman's form of `auth`; `None` for "inherit", which Postman writes as no `auth`.
fn auth_json(auth: &Auth) -> Option<Value> {
    Some(match auth {
        Auth::Inherit => return None,
        Auth::None => json!({ "type": "noauth" }),
        Auth::Bearer { token } => params("bearer", &[("token", json!(token))]),
        Auth::Basic { username, password } => params(
            "basic",
            &[("username", json!(username)), ("password", json!(password))],
        ),
        Auth::Digest { username, password } => params(
            "digest",
            &[("username", json!(username)), ("password", json!(password))],
        ),
        Auth::ApiKey {
            key,
            value,
            location,
        } => params(
            "apikey",
            &[
                ("key", json!(key)),
                ("value", json!(value)),
                (
                    "in",
                    json!(match location {
                        ApiKeyLocation::Header => "header",
                        ApiKeyLocation::Query => "query",
                    }),
                ),
            ],
        ),
        Auth::OAuth1(o) => params(
            "oauth1",
            &[
                ("consumerKey", json!(o.consumer_key)),
                ("consumerSecret", json!(o.consumer_secret)),
                ("token", json!(o.token)),
                ("tokenSecret", json!(o.token_secret)),
                ("signatureMethod", json!(o.signature_method.as_str())),
                ("realm", json!(o.realm)),
                (
                    "addParamsToHeader",
                    json!(o.location == ApiKeyLocation::Header),
                ),
            ],
        ),
        Auth::OAuth2(o) => {
            let grant = match o.grant {
                OAuth2Grant::ClientCredentials => "client_credentials",
                OAuth2Grant::Password => "password_credentials",
                OAuth2Grant::AuthorizationCode if o.pkce => "authorization_code_with_pkce",
                OAuth2Grant::AuthorizationCode => "authorization_code",
            };
            params(
                "oauth2",
                &[
                    ("grant_type", json!(grant)),
                    ("accessTokenUrl", json!(o.token_url)),
                    ("authUrl", json!(o.auth_url)),
                    ("clientId", json!(o.client_id)),
                    ("clientSecret", json!(o.client_secret)),
                    ("scope", json!(o.scope)),
                    ("username", json!(o.username)),
                    ("password", json!(o.password)),
                    ("redirect_uri", json!(o.redirect_uri)),
                    (
                        "client_authentication",
                        json!(match o.client_auth {
                            ClientAuth::Body => "body",
                            ClientAuth::Basic => "header",
                        }),
                    ),
                ],
            )
        }
        Auth::AwsSigV4(a) => params(
            "awsv4",
            &[
                ("accessKey", json!(a.access_key)),
                ("secretKey", json!(a.secret_key)),
                ("sessionToken", json!(a.session_token)),
                ("region", json!(a.region)),
                ("service", json!(a.service)),
            ],
        ),
    })
}

fn kv_list(rows: &[KeyValue]) -> Vec<Value> {
    rows.iter()
        .map(|r| {
            let mut v = json!({ "key": r.key, "value": r.value });
            if !r.enabled {
                v["disabled"] = json!(true);
            }
            v
        })
        .collect()
}

fn body_json(body: &Body) -> Option<Value> {
    let raw = |text: &str, language: &str| {
        json!({
            "mode": "raw",
            "raw": text,
            "options": { "raw": { "language": language } },
        })
    };
    Some(match body {
        Body::None => return None,
        Body::Json(text) => raw(text, "json"),
        Body::Xml(text) => raw(text, "xml"),
        Body::Text(text) => raw(text, "text"),
        Body::Form(rows) => json!({ "mode": "urlencoded", "urlencoded": kv_list(rows) }),
        Body::Multipart(parts) => {
            let fields: Vec<Value> = parts
                .iter()
                .map(|p| {
                    let mut v = if p.file {
                        json!({ "key": p.key, "type": "file", "src": p.value })
                    } else {
                        json!({ "key": p.key, "value": p.value, "type": "text" })
                    };
                    if let Some(ct) = p.content_type.as_deref().filter(|c| !c.is_empty()) {
                        v["contentType"] = json!(ct);
                    }
                    if !p.enabled {
                        v["disabled"] = json!(true);
                    }
                    v
                })
                .collect();
            json!({ "mode": "formdata", "formdata": fields })
        }
        Body::Binary { file } => json!({ "mode": "file", "file": { "src": file } }),
        Body::GraphQl(gql) => json!({
            "mode": "graphql",
            "graphql": { "query": gql.query, "variables": gql.variables },
        }),
    })
}

/// `raw` holds the enabled query rows; the `query` list keeps them all.
fn url_json(request: &Request) -> Value {
    let enabled: Vec<String> = request
        .query
        .iter()
        .filter(|q| q.enabled)
        .map(|q| format!("{}={}", q.key, q.value))
        .collect();
    let raw = if enabled.is_empty() {
        request.url.clone()
    } else {
        let sep = if request.url.contains('?') { '&' } else { '?' };
        format!("{}{sep}{}", request.url, enabled.join("&"))
    };
    let mut url = json!({ "raw": raw });
    if !request.query.is_empty() {
        url["query"] = json!(kv_list(&request.query));
    }
    url
}

struct Exporter {
    notes: Vec<String>,
}

impl Exporter {
    /// `protocolProfileBehavior`, noting the settings Postman keeps app-wide.
    fn profile(&mut self, s: &RequestSettings, label: &str) -> Option<Value> {
        let mut missing = Vec::new();
        if s.timeout_ms.is_some() {
            missing.push("timeout");
        }
        if s.proxy.is_some() {
            missing.push("proxy");
        }
        if s.client_cert.is_some() || s.client_key.is_some() {
            missing.push("client certificate");
        }
        if !missing.is_empty() {
            self.notes.push(format!(
                "{label}: the {} setting was not exported (Postman sets it app-wide).",
                missing.join(", ")
            ));
        }
        let mut ppb = Map::new();
        if let Some(v) = s.follow_redirects {
            ppb.insert("followRedirects".into(), json!(v));
        }
        if let Some(v) = s.max_redirects {
            ppb.insert("maxRedirects".into(), json!(v));
        }
        if let Some(v) = s.tls_verify {
            ppb.insert("strictSSL".into(), json!(v));
        }
        if let Some(v) = s.send_cookies {
            ppb.insert("disableCookies".into(), json!(!v));
        }
        (!ppb.is_empty()).then_some(Value::Object(ppb))
    }

    fn items(&mut self, nodes: &[TreeNode]) -> Result<Vec<Value>, ImportError> {
        // `health.apix` before `health-2.apix`: the order the names were
        // handed out in, so importing the export names them the same way.
        let mut nodes: Vec<&TreeNode> = nodes.iter().collect();
        nodes.sort_by_key(|node| {
            let (folder, path) = match node {
                TreeNode::Folder { path, .. } => (0, path),
                TreeNode::Request { path } => (1, path),
            };
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let (base, n) = match stem.rsplit_once('-') {
                Some((base, n)) if !base.is_empty() && n.parse::<u32>().is_ok() => {
                    (base.to_owned(), n.parse().unwrap_or(1))
                }
                _ => (stem, 1),
            };
            (folder, base, n)
        });
        let mut out = Vec::new();
        for node in nodes {
            match node {
                TreeNode::Folder {
                    name,
                    path,
                    children,
                } => {
                    let meta = store::load_folder_meta(path)?;
                    let mut folder = Map::new();
                    folder.insert("name".into(), json!(name));
                    folder.insert("item".into(), json!(self.items(children)?));
                    if let Some(auth) = auth_json(&meta.auth) {
                        folder.insert("auth".into(), auth);
                    }
                    let events = events(path, "_folder");
                    if !events.is_empty() {
                        folder.insert("event".into(), json!(events));
                    }
                    if let Some(ppb) = self.profile(&meta.settings, &format!("Folder \"{name}\"")) {
                        folder.insert("protocolProfileBehavior".into(), ppb);
                    }
                    out.push(Value::Object(folder));
                }
                TreeNode::Request { path } => out.push(self.request(path)?),
            }
        }
        Ok(out)
    }

    fn request(&mut self, path: &Path) -> Result<Value, ImportError> {
        let r = store::load_request(path)?;
        let mut request = Map::new();
        if let Some(auth) = auth_json(&r.auth) {
            request.insert("auth".into(), auth);
        }
        request.insert("method".into(), json!(r.method.as_str()));
        request.insert("header".into(), json!(kv_list(&r.headers)));
        if let Some(body) = body_json(&r.body) {
            request.insert("body".into(), body);
        }
        request.insert("url".into(), url_json(&r));
        if !r.description.is_empty() {
            request.insert("description".into(), json!(r.description));
        }

        let mut item = Map::new();
        item.insert("name".into(), json!(r.name));
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let events = path.parent().map(|d| events(d, &stem)).unwrap_or_default();
        if !events.is_empty() {
            item.insert("event".into(), json!(events));
        }
        if let Some(ppb) = self.profile(&r.settings, &format!("Request \"{}\"", r.name)) {
            item.insert("protocolProfileBehavior".into(), ppb);
        }
        item.insert("request".into(), Value::Object(request));
        item.insert("response".into(), json!([]));
        Ok(Value::Object(item))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postman::{convert_collection, convert_environment};
    use crate::{Plan, PlannedFile};

    const SHOP: &str = include_str!("../tests/fixtures/shop.postman_collection.json");

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("apix-export-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn requests(plan: &Plan) -> Vec<(PathBuf, Request)> {
        let mut out: Vec<_> = plan
            .files
            .iter()
            .filter_map(|f| match f {
                PlannedFile::Request { path, request } => Some((path.clone(), request.clone())),
                _ => None,
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    #[test]
    fn a_collection_survives_export_and_import() {
        // Postman → Apix → Postman → Apix: the second import equals the first.
        let first = convert_collection(SHOP).unwrap();
        let root = temp("roundtrip");
        first.write(&root).unwrap();
        let mut settings_request = store::load_request(&root.join("health.apix")).unwrap();
        settings_request.settings = RequestSettings {
            follow_redirects: Some(false),
            tls_verify: Some(false),
            timeout_ms: Some(1000),
            ..Default::default()
        };
        store::save_request(&root.join("health.apix"), &settings_request).unwrap();
        store::save_environment(
            &root,
            "dev",
            &[("base".into(), "http://x".into())].into(),
            false,
        )
        .unwrap();
        store::save_environment(
            &root,
            "dev",
            &[("token".into(), "s3cret".into())].into(),
            true,
        )
        .unwrap();

        let export = export_collection(&root, None).unwrap();
        let second = convert_collection(&export.collection).unwrap();

        assert_eq!(second.meta.name, first.meta.name);
        assert_eq!(second.meta.auth, first.meta.auth);
        assert_eq!(second.meta.vars, first.meta.vars);
        let (a, b) = (requests(&first), requests(&second));
        assert_eq!(a.len(), b.len());
        for ((pa, ra), (pb, rb)) in a.iter().zip(&b) {
            assert_eq!(pa, pb);
            if pa == Path::new("health.apix") {
                assert_eq!(rb.settings.follow_redirects, Some(false));
                assert_eq!(rb.settings.tls_verify, Some(false));
                assert_eq!(
                    rb.settings.timeout_ms, None,
                    "Postman has no per-request timeout"
                );
                continue;
            }
            assert_eq!(ra, rb, "{}", pa.display());
        }
        let scripts = |p: &Plan| {
            p.files
                .iter()
                .filter(|f| matches!(f, PlannedFile::Script { .. }))
                .count()
        };
        assert_eq!(scripts(&first), scripts(&second));
        let folders = |p: &Plan| {
            p.files
                .iter()
                .filter(|f| matches!(f, PlannedFile::Folder { .. }))
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(folders(&first), folders(&second));
        assert!(
            export
                .notes
                .iter()
                .any(|n| n.contains("timeout setting was not exported")),
            "{:?}",
            export.notes
        );

        // The environment keeps its secret's name but not its value.
        assert_eq!(export.environments.len(), 1);
        let env = convert_environment(&export.environments[0].1).unwrap();
        assert_eq!(env.vars.get("base").map(String::as_str), Some("http://x"));
        assert_eq!(env.secrets.get("token").map(String::as_str), Some(""));
        assert!(!export.environments[0].1.contains("s3cret"));
        assert!(export.notes.iter().any(|n| n.contains("token")));

        let out = temp("roundtrip-out");
        let written = export.write(&out).unwrap();
        assert_eq!(written.len(), 2);
        assert!(
            written[0]
                .to_string_lossy()
                .ends_with(".postman_collection.json")
        );
        assert_eq!(export.write(&out).unwrap().len(), 2, "never overwrites");
        std::fs::remove_dir_all(&root).ok();
        std::fs::remove_dir_all(&out).ok();
    }
}
