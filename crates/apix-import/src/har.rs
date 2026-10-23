//! HAR 1.2 (the browser DevTools "Save all as HAR" file) → one collection,
//! a folder per host. Only API calls are kept: static files (scripts,
//! styles, images, fonts), `data:` URLs and exact repeats are left out, and
//! `Cookie` headers are dropped because they carry the browser's session.

use std::collections::HashSet;
use std::path::PathBuf;

use apix_core::store::{CollectionMeta, FolderMeta};
use apix_core::{Auth, Body, FormPart, KeyValue, Method, Request};
use serde::Deserialize;

use crate::{Found, ImportError, Plan, PlannedFile, Report, request_file, slug, unique};

#[derive(Deserialize)]
struct Har {
    log: Log,
}

#[derive(Deserialize)]
struct Log {
    #[serde(default)]
    entries: Vec<Entry>,
    creator: Option<Creator>,
}

#[derive(Deserialize)]
struct Creator {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct Entry {
    request: HarRequest,
}

#[derive(Deserialize)]
struct HarRequest {
    method: String,
    url: String,
    #[serde(default)]
    headers: Vec<NameValue>,
    #[serde(rename = "postData")]
    post_data: Option<PostData>,
}

#[derive(Deserialize)]
struct NameValue {
    name: String,
    #[serde(default)]
    value: String,
    #[serde(rename = "fileName")]
    file_name: Option<String>,
    #[serde(rename = "contentType")]
    content_type: Option<String>,
}

#[derive(Deserialize)]
struct PostData {
    #[serde(rename = "mimeType", default)]
    mime_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    params: Vec<NameValue>,
}

/// Whether `value` is a HAR file.
pub(crate) fn is_har(value: &serde_json::Value) -> bool {
    value
        .get("log")
        .is_some_and(|log| log.get("entries").is_some())
}

/// Static resources a page loads, which nobody wants as API requests.
const STATIC: [&str; 17] = [
    ".js", ".mjs", ".css", ".map", ".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".avif",
    ".ico", ".woff", ".woff2", ".ttf", ".otf", ".mp4",
];

/// Headers the HTTP engine writes itself, or that HTTP/2 adds.
fn skipped_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    name.starts_with(':')
        || matches!(
            lower.as_str(),
            "host" | "content-length" | "connection" | "accept-encoding" | "cookie"
        )
}

pub fn convert(json: &str, name: &str) -> Result<Found, ImportError> {
    let har: Har = serde_json::from_str(json)?;
    let title = har
        .log
        .creator
        .as_ref()
        .map(|c| format!("{name} ({})", c.name))
        .filter(|_| !name.is_empty())
        .unwrap_or_else(|| name.to_owned());
    let mut report = Report {
        collection: title.clone(),
        ..Report::default()
    };
    let mut files = Vec::new();
    let mut seen = HashSet::new();
    let mut folders: Vec<String> = Vec::new();
    let mut used_in: std::collections::HashMap<String, HashSet<String>> = Default::default();
    let (mut static_skipped, mut repeats, mut cookies) = (0, 0, false);

    for entry in &har.log.entries {
        let r = &entry.request;
        let Ok(url) = url::Url::parse(&r.url) else {
            continue;
        };
        if !matches!(url.scheme(), "http" | "https") {
            continue;
        }
        let path = url.path().to_ascii_lowercase();
        if STATIC.iter().any(|ext| path.ends_with(ext)) {
            static_skipped += 1;
            continue;
        }
        let body_text = r
            .post_data
            .as_ref()
            .and_then(|p| p.text.clone())
            .unwrap_or_default();
        if !seen.insert((r.method.clone(), r.url.clone(), body_text)) {
            repeats += 1;
            continue;
        }

        let method: Method = match r.method.parse() {
            Ok(m) => m,
            Err(_) => {
                report.warn(&r.url, format!("Method {} is not supported yet.", r.method));
                continue;
            }
        };
        let mut request = Request::new(
            method,
            format!(
                "{}://{}{}",
                url.scheme(),
                url.host_str().unwrap_or_default(),
                url.port().map(|p| format!(":{p}")).unwrap_or_default()
            ) + url.path(),
        );
        request.query = url
            .query_pairs()
            .map(|(k, v)| KeyValue::new(k, v))
            .collect();
        for h in &r.headers {
            if h.name.eq_ignore_ascii_case("cookie") {
                cookies = true;
            }
            if skipped_header(&h.name) {
                continue;
            }
            // A bearer token becomes auth, as it does for pasted curl.
            if h.name.eq_ignore_ascii_case("authorization")
                && let Some(token) = h.value.strip_prefix("Bearer ")
            {
                request.auth = Auth::Bearer {
                    token: token.trim().to_owned(),
                };
                continue;
            }
            request
                .headers
                .push(KeyValue::new(h.name.clone(), h.value.clone()));
        }
        if let Some(post) = &r.post_data {
            request.body = body(post);
            if matches!(request.body, Body::Multipart(_)) {
                request.headers.retain(|h| {
                    !(h.key.eq_ignore_ascii_case("content-type")
                        && h.value.to_ascii_lowercase().starts_with("multipart/"))
                });
            }
            if let Body::Multipart(parts) = &request.body
                && parts.iter().any(|p| p.file)
            {
                report.warn(
                    &r.url,
                    "The HAR does not hold uploaded files; pick them again in Apix.",
                );
            }
        }
        let tail = url
            .path_segments()
            .and_then(|mut s| s.next_back())
            .filter(|s| !s.is_empty())
            .unwrap_or("/");
        request.name = format!("{} {}", r.method, url.path());

        let host = slug(url.host_str().unwrap_or("host"), "host");
        if !folders.contains(&host) {
            folders.push(host.clone());
            report.folders += 1;
            files.push(PlannedFile::Folder {
                path: PathBuf::from(&host),
                meta: FolderMeta::default(),
            });
        }
        let used = used_in.entry(host.clone()).or_default();
        let stem = unique(
            slug(
                &format!("{} {tail}", r.method.to_ascii_lowercase()),
                "request",
            ),
            used,
        );
        files.push(PlannedFile::Request {
            path: PathBuf::from(&host).join(request_file(&stem)),
            request,
        });
        report.requests += 1;
    }

    // Empty folder.toml files are not worth writing: folders appear by themselves.
    files.retain(|f| !matches!(f, PlannedFile::Folder { .. }));
    if static_skipped > 0 {
        report.warn(
            "",
            format!(
                "{static_skipped} static files (scripts, styles, images, fonts) were left out."
            ),
        );
    }
    if repeats > 0 {
        report.warn(
            "",
            format!("{repeats} repeated requests were imported once."),
        );
    }
    if cookies {
        report.warn(
            "",
            "Cookie headers were left out: they carry the browser's session. Apix keeps cookies from responses itself.",
        );
    }
    let plan = Plan {
        meta: CollectionMeta {
            name: if title.is_empty() {
                "HAR import".into()
            } else {
                title
            },
            ..CollectionMeta::default()
        },
        files,
        report,
    };
    Ok(Found {
        collections: vec![(String::new(), plan)],
        ..Found::default()
    })
}

fn body(post: &PostData) -> Body {
    let mime = post.mime_type.to_ascii_lowercase();
    let text = post.text.clone().unwrap_or_default();
    if mime.contains("x-www-form-urlencoded") {
        let rows = if post.params.is_empty() {
            url::form_urlencoded::parse(text.as_bytes())
                .map(|(k, v)| KeyValue::new(k, v))
                .collect()
        } else {
            post.params
                .iter()
                .map(|p| KeyValue::new(p.name.clone(), p.value.clone()))
                .collect()
        };
        return Body::Form(rows);
    }
    if mime.starts_with("multipart/") && !post.params.is_empty() {
        return Body::Multipart(
            post.params
                .iter()
                .map(|p| match &p.file_name {
                    Some(_) => FormPart {
                        content_type: p.content_type.clone(),
                        ..FormPart::file(p.name.clone(), String::new())
                    },
                    None => FormPart::text(p.name.clone(), p.value.clone()),
                })
                .collect(),
        );
    }
    if text.is_empty() {
        return Body::None;
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

#[cfg(test)]
mod tests {
    use super::*;

    const HAR: &str = r#"{
      "log": {
        "version": "1.2",
        "creator": { "name": "Firefox", "version": "130" },
        "entries": [
          { "request": { "method": "GET", "url": "https://api.shop.dev/v1/users?page=2&sort=name",
              "headers": [ { "name": ":authority", "value": "api.shop.dev" },
                           { "name": "Accept", "value": "application/json" },
                           { "name": "Cookie", "value": "sid=secret" },
                           { "name": "Authorization", "value": "Bearer abc.def" } ] } },
          { "request": { "method": "GET", "url": "https://api.shop.dev/v1/users?page=2&sort=name", "headers": [] } },
          { "request": { "method": "POST", "url": "https://api.shop.dev/v1/users",
              "headers": [ { "name": "Content-Type", "value": "application/json" } ],
              "postData": { "mimeType": "application/json", "text": "{\"name\":\"Ada\"}" } } },
          { "request": { "method": "POST", "url": "https://auth.shop.dev/login", "headers": [],
              "postData": { "mimeType": "application/x-www-form-urlencoded", "text": "user=ada&pass=p%26w" } } },
          { "request": { "method": "GET", "url": "https://cdn.shop.dev/app.js", "headers": [] } },
          { "request": { "method": "GET", "url": "https://cdn.shop.dev/logo.png", "headers": [] } },
          { "request": { "method": "GET", "url": "data:image/png;base64,AAA", "headers": [] } }
        ]
      }
    }"#;

    #[test]
    fn keeps_api_calls_and_drops_the_rest() {
        let found = convert(HAR, "shop.har").unwrap();
        let plan = &found.collections[0].1;
        assert_eq!(plan.meta.name, "shop.har (Firefox)");
        let requests: Vec<(&PathBuf, &Request)> = plan
            .files
            .iter()
            .filter_map(|f| match f {
                PlannedFile::Request { path, request } => Some((path, request)),
                _ => None,
            })
            .collect();
        assert_eq!(
            requests.len(),
            3,
            "one GET (deduplicated), one JSON POST, one form POST"
        );

        let (path, get) = requests[0];
        assert_eq!(path, &PathBuf::from("api-shop-dev/get-users.apix"));
        assert_eq!(get.name, "GET /v1/users");
        assert_eq!(get.url, "https://api.shop.dev/v1/users");
        assert_eq!(
            get.query,
            [KeyValue::new("page", "2"), KeyValue::new("sort", "name")]
        );
        assert_eq!(get.headers, [KeyValue::new("Accept", "application/json")]);
        assert_eq!(
            get.auth,
            Auth::Bearer {
                token: "abc.def".into()
            }
        );

        assert_eq!(requests[1].1.body, Body::Json("{\"name\":\"Ada\"}".into()));
        assert_eq!(
            requests[2].1.body,
            Body::Form(vec![
                KeyValue::new("user", "ada"),
                KeyValue::new("pass", "p&w")
            ])
        );
        assert_eq!(
            requests[2].0,
            &PathBuf::from("auth-shop-dev/post-login.apix")
        );

        let notes = plan.report.to_string();
        assert!(notes.contains("2 static files"), "{notes}");
        assert!(notes.contains("1 repeated"), "{notes}");
        assert!(notes.contains("Cookie headers were left out"), "{notes}");
        assert!(!format!("{:?}", plan.files).contains("sid=secret"));
    }
}
