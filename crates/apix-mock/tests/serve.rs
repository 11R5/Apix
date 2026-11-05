//! The mock server answering from a collection on disk, through the same
//! HTTP client `apix send` uses, and following edits to the collection.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use apix_core::{KeyValue, Method, Request};
use apix_http::HttpClient;

fn collection(tag: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("apix-mock-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("users")).unwrap();
    std::fs::write(root.join("apix.toml"), "name = \"Shop\"\n").unwrap();
    std::fs::write(
        root.join("users/get.apix"),
        r#"name = "Get user"
url = "{{base_url}}/users/:id"

[[example]]
name = "Found"
status = 200
body = '{"id": 1, "name": "Ada"}'

[[example.headers]]
key = "Content-Type"
value = "application/json"

[[example]]
name = "Gone"
status = 410
body = "gone"
"#,
    )
    .unwrap();
    root
}

#[tokio::test]
async fn answers_from_examples_and_follows_edits() {
    let root = collection("serve");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(Mutex::new(Vec::new()));
    let seen = hits.clone();
    tokio::spawn(apix_mock::serve(
        listener,
        root.clone(),
        apix_mock::Options::default(),
        move |hit| seen.lock().unwrap().push(hit),
    ));
    let client = HttpClient::new().unwrap();

    let got = client
        .send(&Request::new(Method::Get, format!("{base}/users/42")))
        .await
        .unwrap();
    assert_eq!(got.status, 200);
    assert_eq!(got.raw_body(), "{\"id\": 1, \"name\": \"Ada\"}");
    assert_eq!(got.header("content-type"), Some("application/json"));
    assert_eq!(got.header("access-control-allow-origin"), Some("*"));
    assert_eq!(got.header("x-apix-mock"), Some("users/get.apix · Found"));

    let mut gone = Request::new(Method::Get, format!("{base}/users/42"));
    gone.headers
        .push(KeyValue::new("x-mock-response-name", "Gone"));
    assert_eq!(client.send(&gone).await.unwrap().status, 410);

    let missing = client
        .send(&Request::new(Method::Get, format!("{base}/orders")))
        .await
        .unwrap();
    assert_eq!(missing.status, 404);
    assert!(missing.raw_body().contains("GET /users/:id"));

    // A new example is served without a restart.
    std::fs::write(
        root.join("health.apix"),
        "name = \"Health\"\nurl = \"{{base_url}}/health\"\n\n[[example]]\nname = \"Up\"\nstatus = 200\nbody = \"ok\"\n",
    )
    .unwrap();
    tokio::time::sleep(Duration::from_millis(1200)).await;
    let health = client
        .send(&Request::new(Method::Get, format!("{base}/health")))
        .await
        .unwrap();
    assert_eq!((health.status, health.raw_body()), (200, "ok".to_string()));

    let hits = hits.lock().unwrap();
    assert_eq!(hits.len(), 4);
    assert_eq!(
        hits[0].matched,
        Some(("users/get.apix".into(), "Found".into()))
    );
    assert_eq!(hits[2].status, 404);
    let _ = std::fs::remove_dir_all(&root);
}
