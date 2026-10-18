use std::fs;
use std::path::{Path, PathBuf};

use apix_core::store::load_request;
use apix_core::{
    ApiKeyLocation, Auth, Body, ClientAuth, Collection, FormPart, GraphQl, KeyValue, Method,
    OAuth1Method, OAuth2, OAuth2Grant, Request,
};
use apix_import::postman::{Kind, convert_collection, convert_environment, detect};
use apix_import::{ImportError, PlannedFile};

const SHOP: &str = include_str!("fixtures/shop.postman_collection.json");
const STAGING: &str = include_str!("fixtures/staging.postman_environment.json");

fn request<'a>(plan: &'a apix_import::Plan, path: &str) -> &'a Request {
    plan.files
        .iter()
        .find_map(|f| match f {
            PlannedFile::Request { path: p, request } if p == Path::new(path) => Some(request),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no request planned at {path}"))
}

fn kv(key: &str, value: &str, enabled: bool) -> KeyValue {
    KeyValue {
        key: key.into(),
        value: value.into(),
        enabled,
    }
}

fn warned(plan: &apix_import::Plan, item: &str, needle: &str) -> bool {
    plan.report
        .warnings
        .iter()
        .any(|w| w.item == item && w.message.contains(needle))
}

fn bearer() -> Auth {
    Auth::Bearer {
        token: "{{token}}".into(),
    }
}

#[test]
fn detects_export_kinds() {
    assert_eq!(detect(SHOP).unwrap(), Kind::Collection);
    assert_eq!(detect(STAGING).unwrap(), Kind::Environment);
    let v1 = r#"{"id":"x","name":"Old","order":[],"requests":[]}"#;
    assert!(matches!(detect(v1), Err(ImportError::Unsupported(m)) if m.contains("v1")));
    assert!(matches!(detect("not json"), Err(ImportError::Json(_))));
}

#[test]
fn converts_collection_metadata_and_counts() {
    let plan = convert_collection(SHOP).unwrap();
    assert_eq!(plan.meta.name, "Shop API");
    assert_eq!(plan.meta.description, "Orders and users for the demo shop.");
    assert_eq!(plan.meta.vars.len(), 1);
    assert_eq!(plan.meta.vars["baseUrl"], "https://shop.example.com/api");

    let r = &plan.report;
    assert_eq!(
        (r.requests, r.folders, r.scripts, r.examples_skipped),
        (9, 2, 2, 1)
    );
    assert_eq!(plan.request_count(), 9);
}

#[test]
fn keeps_query_headers_and_auth_where_postman_set_it() {
    let plan = convert_collection(SHOP).unwrap();
    let list = request(&plan, "users/list-users.apix");
    assert_eq!(list.method, Method::Get);
    assert_eq!(list.url, "{{baseUrl}}/users");
    assert_eq!(
        list.query,
        [
            kv("page", "2", true),
            kv("limit", "20", true),
            kv("sort", "name", false)
        ]
    );
    assert_eq!(
        list.headers,
        [
            kv("Accept", "application/json", true),
            kv("X-Debug", "1", false)
        ]
    );
    assert_eq!(list.description, "Paginated list.");
    assert_eq!(list.auth, Auth::Inherit, "no auth of its own");
    assert_eq!(
        plan.meta.auth,
        bearer(),
        "the collection keeps Postman's bearer"
    );
}

#[test]
fn handles_path_variables() {
    let plan = convert_collection(SHOP).unwrap();
    assert_eq!(
        request(&plan, "users/get-user.apix").url,
        "{{baseUrl}}/users/42"
    );
    assert!(warned(&plan, "Users / Get user", ":userId was replaced"));

    let delete = request(&plan, "users/delete-user.apix");
    assert_eq!(delete.url, "{{baseUrl}}/users/{{id}}");
    assert_eq!(delete.method, Method::Delete);
    assert_eq!(
        delete.auth,
        Auth::None,
        "noauth overrides the collection's bearer"
    );
    assert!(warned(&plan, "Users / Delete user", "became {{id}}"));
}

#[test]
fn converts_every_body_mode() {
    let plan = convert_collection(SHOP).unwrap();
    assert_eq!(
        request(&plan, "users/create-user.apix").body,
        Body::Json("{\n  \"name\": \"Ada\"\n}".into())
    );

    let form = request(&plan, "orders/create-order-form.apix");
    assert_eq!(
        form.body,
        Body::Form(vec![kv("sku", "A-1", true), kv("qty", "2", true)])
    );
    assert_eq!(form.auth, Auth::Inherit);
    let folder = plan
        .files
        .iter()
        .find_map(|f| match f {
            PlannedFile::Folder { path, meta } if path == Path::new("orders") => Some(meta),
            _ => None,
        })
        .expect("the orders folder keeps its auth in folder.toml");
    assert_eq!(
        folder.auth,
        Auth::ApiKey {
            key: "api_key".into(),
            value: "{{apiKey}}".into(),
            location: ApiKeyLocation::Query
        }
    );

    let upload = request(&plan, "orders/upload-invoice.apix");
    assert_eq!(
        upload.body,
        Body::Multipart(vec![
            FormPart::text("note", "march"),
            FormPart {
                content_type: Some("application/json".into()),
                enabled: false,
                ..FormPart::text("meta", "{}")
            },
            FormPart::file("file", "/tmp/invoice.pdf"),
            FormPart::file("file", "/tmp/receipt.pdf"),
        ])
    );

    assert_eq!(
        request(&plan, "orders/search-orders.apix").body,
        Body::GraphQl(GraphQl {
            query: "query ($q: String) { orders(q: $q) { id } }".into(),
            variables: "{\"q\": \"open\"}".into(),
        })
    );
}

#[test]
fn carries_over_signing_auth_types() {
    let json = r#"{
        "info": { "name": "Auth", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json" },
        "item": [
            { "name": "Digest", "request": { "method": "GET", "url": "https://x.dev/d",
                "auth": { "type": "digest", "digest": [
                    { "key": "username", "value": "ada" }, { "key": "password", "value": "{{pw}}" },
                    { "key": "algorithm", "value": "MD5" } ] } } },
            { "name": "AWS", "request": { "method": "GET", "url": "https://x.dev/a",
                "auth": { "type": "awsv4", "awsv4": [
                    { "key": "accessKey", "value": "AK" }, { "key": "secretKey", "value": "SK" },
                    { "key": "region", "value": "eu-west-1" }, { "key": "service", "value": "s3" } ] } } },
            { "name": "OAuth1", "request": { "method": "GET", "url": "https://x.dev/o",
                "auth": { "type": "oauth1", "oauth1": [
                    { "key": "consumerKey", "value": "ck" }, { "key": "consumerSecret", "value": "cs" },
                    { "key": "signatureMethod", "value": "HMAC-SHA256" },
                    { "key": "addParamsToHeader", "value": false } ] } } },
            { "name": "OAuth2", "request": { "method": "GET", "url": "https://x.dev/t",
                "auth": { "type": "oauth2", "oauth2": [
                    { "key": "grant_type", "value": "authorization_code_with_pkce" },
                    { "key": "authUrl", "value": "https://id.x.dev/authorize" },
                    { "key": "accessTokenUrl", "value": "https://id.x.dev/token" },
                    { "key": "clientId", "value": "app" },
                    { "key": "scope", "value": "read" },
                    { "key": "redirect_uri", "value": "https://oauth.pstmn.io/v1/callback" },
                    { "key": "client_authentication", "value": "body" } ] } } }
        ]
    }"#;
    let plan = convert_collection(json).unwrap();
    assert_eq!(
        request(&plan, "digest.apix").auth,
        Auth::Digest {
            username: "ada".into(),
            password: "{{pw}}".into()
        }
    );
    let Auth::AwsSigV4(aws) = &request(&plan, "aws.apix").auth else {
        panic!("AWS expected");
    };
    assert_eq!(
        (aws.region.as_str(), aws.service.as_str()),
        ("eu-west-1", "s3")
    );
    let Auth::OAuth1(o) = &request(&plan, "oauth1.apix").auth else {
        panic!("OAuth 1.0 expected");
    };
    assert_eq!(o.signature_method, OAuth1Method::HmacSha256);
    assert_eq!(o.location, ApiKeyLocation::Query);
    assert_eq!(
        request(&plan, "oauth2.apix").auth,
        Auth::OAuth2(OAuth2 {
            grant: OAuth2Grant::AuthorizationCode,
            token_url: "https://id.x.dev/token".into(),
            auth_url: "https://id.x.dev/authorize".into(),
            client_id: "app".into(),
            scope: "read".into(),
            pkce: true,
            client_auth: ClientAuth::Body,
            ..OAuth2::default()
        })
    );
    assert!(warned(
        &plan,
        "OAuth2",
        "redirect URL https://oauth.pstmn.io/v1/callback was dropped"
    ));
}

#[test]
fn carries_over_file_bodies() {
    let json = r#"{
        "info": { "name": "Files", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json" },
        "item": [
            { "name": "Put blob", "request": { "method": "PUT", "url": "https://x.dev/b",
                "body": { "mode": "file", "file": { "src": "/data/dump.bin" } } } },
            { "name": "Empty", "request": { "method": "PUT", "url": "https://x.dev/b",
                "body": { "mode": "formdata", "formdata": [ { "key": "f", "type": "file" } ] } } }
        ]
    }"#;
    let plan = convert_collection(json).unwrap();
    assert_eq!(
        request(&plan, "put-blob.apix").body,
        Body::Binary {
            file: "/data/dump.bin".into()
        }
    );
    assert_eq!(
        request(&plan, "empty.apix").body,
        Body::Multipart(vec![FormPart::file("f", "")])
    );
    assert!(warned(&plan, "Empty", "no file chosen"));
}

#[test]
fn reports_what_it_cannot_carry_over() {
    let plan = convert_collection(SHOP).unwrap();
    let health = request(&plan, "health.apix");
    assert_eq!(health.method, Method::Get);
    assert_eq!(health.auth, Auth::None);
    assert!(warned(&plan, "Health", "PROPFIND is not supported"));
    assert!(warned(
        &plan,
        "Health",
        "ntlm authorization is not supported"
    ));

    // Same name twice in one folder: the second file gets a suffix.
    let head = request(&plan, "health-2.apix");
    assert_eq!(head.method, Method::Head);
    assert_eq!(head.auth, Auth::Inherit);
}

#[test]
fn keeps_scripts_as_files() {
    let plan = convert_collection(SHOP).unwrap();
    let scripts: Vec<(&Path, &str)> = plan
        .files
        .iter()
        .filter_map(|f| match f {
            PlannedFile::Script { path, text } => Some((path.as_path(), text.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(scripts.len(), 2);
    assert!(scripts.contains(&(
        Path::new("_collection.prerequest.js"),
        "pm.variables.set('ts', Date.now());"
    )));
    let (_, test) = scripts
        .iter()
        .find(|(p, _)| *p == Path::new("users/list-users.test.js"))
        .expect("test script next to its request");
    assert!(test.contains("pm.response.to.have.status(200);"));
}

#[test]
fn reads_postman_v2_0_shapes() {
    let v2_0 = r#"{
        "info": { "name": "Legacy", "schema": "https://schema.getpostman.com/json/collection/v2.0.0/collection.json" },
        "auth": { "type": "basic", "basic": { "username": "ada", "password": "pw" } },
        "item": [
            { "name": "Ping", "request": "https://legacy.example.com/ping" },
            { "name": "Echo", "request": {
                "method": "post",
                "header": "Content-Type: text/plain\nX-Trace: on",
                "body": { "mode": "raw", "raw": "hello" },
                "url": { "protocol": "https", "host": "legacy.example.com", "path": "/echo" }
            } }
        ]
    }"#;
    let plan = convert_collection(v2_0).unwrap();
    let basic = Auth::Basic {
        username: "ada".into(),
        password: "pw".into(),
    };

    let ping = request(&plan, "ping.apix");
    assert_eq!(ping.url, "https://legacy.example.com/ping");
    assert_eq!(ping.auth, Auth::Inherit);
    assert_eq!(plan.meta.auth, basic);

    let echo = request(&plan, "echo.apix");
    assert_eq!(echo.method, Method::Post);
    assert_eq!(echo.url, "https://legacy.example.com/echo");
    assert_eq!(
        echo.headers,
        [
            kv("Content-Type", "text/plain", true),
            kv("X-Trace", "on", true)
        ]
    );
    assert_eq!(echo.body, Body::Text("hello".into()));
    assert_eq!(echo.auth, Auth::Inherit);
}

#[test]
fn splits_secrets_out_of_environments() {
    let env = convert_environment(STAGING).unwrap();
    assert_eq!(env.title, "Staging EU");
    assert_eq!(env.name, "staging-eu");
    assert_eq!(env.vars.len(), 1);
    assert_eq!(env.vars["baseUrl"], "https://staging.shop.example.com/api");
    assert_eq!(env.secrets.len(), 2);
    assert_eq!(env.secrets["token"], "s3cr3t");
    assert_eq!(env.skipped_disabled, 1);
    assert!(convert_environment(SHOP).is_err());
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("apix-import-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

#[test]
fn writes_a_collection_apix_can_open() {
    let root = temp_dir("write");
    let plan = convert_collection(SHOP).unwrap();
    plan.write(&root).unwrap();

    let collection = Collection::open(&root).unwrap();
    assert_eq!(collection.meta.name, "Shop API");
    let paths = collection.request_paths();
    assert_eq!(paths.len(), 9);
    assert!(root.join("users/list-users.test.js").is_file());

    // Auth is resolved the way Postman would: folder, then collection.
    assert_eq!(collection.meta.auth, bearer());
    let (form_auth, from) =
        apix_core::project::inherited_auth(&root.join("orders"), Some(&collection), None).unwrap();
    assert!(matches!(form_auth, Auth::ApiKey { .. }));
    assert_eq!(from.as_deref(), Some("folder \"orders\""));

    // What is on disk is exactly what was planned.
    for file in &plan.files {
        if let PlannedFile::Request { path, request } = file {
            assert_eq!(
                &load_request(&root.join(path)).unwrap(),
                request,
                "{}",
                path.display()
            );
        }
    }

    // Environments go into the same collection; secrets into the gitignored file.
    let env = convert_environment(STAGING).unwrap();
    let written = env.write(&root).unwrap();
    assert_eq!(written.len(), 2);
    assert_eq!(collection.environment_names(), ["staging-eu"]);
    let staging = collection.environment("staging-eu").unwrap();
    assert_eq!(staging.vars["token"], "s3cr3t");
    assert_eq!(
        staging.vars["baseUrl"],
        "https://staging.shop.example.com/api"
    );
    assert!(matches!(env.write(&root), Err(ImportError::Exists(_))));

    // Never import over existing files.
    assert!(matches!(plan.write(&root), Err(ImportError::NotEmpty(_))));
    fs::remove_dir_all(root).unwrap();
}
