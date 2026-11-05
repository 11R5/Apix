//! The example plugins in `plugins/` (built from `apix-plugin-sdk`'s
//! examples) and hand-written modules for what a plugin may not do.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use apix_plugins::{AuthInput, Plugin, load_dir};
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins")
}

fn example(name: &str) -> Plugin {
    Plugin::load(&examples().join(format!("{name}.wasm"))).unwrap()
}

fn wat(text: &str) -> Result<Plugin, String> {
    Plugin::from_bytes(text.as_bytes(), Path::new("test.wat"))
}

#[test]
fn the_examples_load_with_their_manifests() {
    let loaded = load_dir(&examples());
    let names: Vec<String> = loaded
        .iter()
        .map(|p| p.as_ref().unwrap().manifest.name.clone())
        .collect();
    assert_eq!(names, ["csv-endpoints", "hmac-signature", "json-stats"]);
    let hmac = example("hmac-signature");
    let auth = hmac.manifest.auth.as_ref().unwrap();
    assert_eq!(auth.label, "HMAC signature");
    assert!(auth.fields.iter().any(|f| f.name == "secret" && f.secret));
    assert!(example("csv-endpoints").manifest.import.is_some());
    assert!(example("json-stats").manifest.visualize.is_some());
}

#[test]
fn an_auth_plugin_signs() {
    let plugin = example("hmac-signature");
    let body = r#"{"name":"Ada"}"#;
    let out = plugin
        .auth(&AuthInput {
            method: "post".into(),
            url: "https://api.test/v1/users?x=1#top".into(),
            headers: vec![],
            body: body.into(),
            config: BTreeMap::from([
                ("key_id".into(), "k1".into()),
                ("secret".into(), "s3cret".into()),
            ]),
        })
        .unwrap();
    // The same signature computed here.
    let hash: String = Sha256::digest(body.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let mut mac = Hmac::<Sha256>::new_from_slice(b"s3cret").unwrap();
    mac.update(format!("POST\n/v1/users?x=1\n{hash}").as_bytes());
    let expected = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
    assert_eq!(
        out.headers,
        [(
            "Signature".to_owned(),
            format!("keyId=\"k1\",algorithm=\"hmac-sha256\",signature=\"{expected}\"")
        )]
    );
    // The plugin's own error comes back as one.
    let error = plugin.auth(&AuthInput::default()).unwrap_err();
    assert_eq!(error, "the secret is empty");
    assert!(
        plugin
            .call("import", &serde_json::json!({}))
            .unwrap_err()
            .contains("no import hook")
    );
}

#[test]
fn an_importer_and_a_visualizer() {
    let csv = "method,url,name,folder\n# a comment\nGET,{{base_url}}/users,List users,users\npost,\"{{base_url}}/users?a=1,2\",,users\nDELETE,{{base_url}}/users/1\n";
    let imported = example("csv-endpoints").import("shop.csv", csv).unwrap();
    assert_eq!(imported.name, "shop");
    assert_eq!(imported.requests.len(), 3);
    assert_eq!(imported.requests[1].method, "POST");
    assert_eq!(imported.requests[1].url, "{{base_url}}/users?a=1,2");
    assert_eq!(imported.requests[1].name, "POST /users?a=1,2");
    assert_eq!(imported.requests[2].folder, "");
    assert!(
        example("csv-endpoints")
            .import("x.csv", "GET\n")
            .unwrap_err()
            .contains("line 1")
    );

    let body = r#"{"data":[{"id":1,"name":"Ada","age":36},{"id":2,"name":"Bob","age":null},{"id":3,"name":"Ada","email":"c@x"}]}"#;
    let view = example("json-stats").visualize(200, &[], body).unwrap();
    assert_eq!(view.title, "3 records, 4 fields");
    assert_eq!(view.columns[0], "field");
    let id = &view.rows[0];
    assert_eq!(id, &["id", "number", "3/3", "3", "1 … 3"]);
    let name = view.rows.iter().find(|r| r[0] == "name").unwrap();
    assert_eq!(name[3], "2");
    let age = view.rows.iter().find(|r| r[0] == "age").unwrap();
    assert_eq!(age[1], "null | number");
    assert_eq!(age[2], "1/3");
    assert!(
        example("json-stats")
            .visualize(200, &[], "<html>")
            .unwrap_err()
            .contains("not JSON")
    );
}

const GOOD: &str = r#"
(module
  (memory (export "memory") 1)
  (data (i32.const 16) "{\"name\":\"loop\",\"version\":\"1\"}")
  (func (export "apix_alloc") (param i32) (result i32) (i32.const 1024))
  (func (export "apix_manifest") (result i64) (i64.const 0x100000001D))
  (func (export "apix_call") (param i32 i32 i32 i32) (result i64)
    (loop $forever (br $forever))
    (i64.const 0)))
"#;

#[test]
fn what_a_plugin_may_not_do() {
    // A plugin that never stops is stopped.
    let looping = wat(GOOD).unwrap();
    assert_eq!(looping.manifest.name, "loop");
    let error = looping
        .call("visualize", &serde_json::json!({}))
        .unwrap_err();
    assert!(error.contains("ran too long"), "{error}");

    // No imports at all: no way out of the sandbox.
    let importing =
        r#"(module (import "env" "open" (func (param i32))) (memory (export "memory") 1))"#;
    assert!(wat(importing).unwrap_err().contains("may not import"));

    // A bad name, a missing export, an answer outside its memory, not WebAssembly.
    let bad_name = GOOD.replace("\\\"loop\\\"", "\\\"LOOP\\\"");
    assert!(wat(&bad_name).unwrap_err().contains("[a-z0-9-]"));
    let no_manifest = r#"(module (memory (export "memory") 1))"#;
    assert!(wat(no_manifest).unwrap_err().contains("apix_manifest"));
    let outside = GOOD.replace("0x100000001D", "0x7FFFFFF00000001D");
    assert!(wat(&outside).unwrap_err().contains("outside its memory"));
    assert!(
        wat("not wasm")
            .unwrap_err()
            .contains("not a WebAssembly module")
    );
}
