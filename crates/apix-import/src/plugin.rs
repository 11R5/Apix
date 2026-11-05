//! What an import plugin read (`apix_plugins::Imported`) as a collection
//! plan: requests in their folders, file names made from their names.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use apix_core::store::CollectionMeta;
use apix_core::{Body, KeyValue, Method, Request};
use apix_plugins::Imported;

use crate::{Plan, PlannedFile, Report, Warning, request_file, slug, unique};

/// The plan for `imported`; `fallback` names the collection when the plugin did not.
pub fn plan(imported: &Imported, fallback: &str) -> Plan {
    let name = if imported.name.trim().is_empty() {
        fallback.to_owned()
    } else {
        imported.name.trim().to_owned()
    };
    let mut report = Report {
        collection: name.clone(),
        ..Report::default()
    };
    let mut files = Vec::new();
    // Names used in each folder, and the folders made.
    let mut used: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    let mut folders: HashSet<PathBuf> = HashSet::new();
    for r in &imported.requests {
        let mut dir = PathBuf::new();
        for part in r.folder.split('/').map(str::trim).filter(|p| !p.is_empty()) {
            dir.push(slug(part, "folder"));
            folders.insert(dir.clone());
        }
        let method = r
            .method
            .trim()
            .to_ascii_uppercase()
            .parse::<Method>()
            .unwrap_or_else(|_| {
                report.warnings.push(Warning {
                    item: r.name.clone(),
                    message: format!("unknown method {}, sent as GET", r.method),
                });
                Method::Get
            });
        let mut request = Request::new(method, r.url.trim());
        request.name = r.name.trim().to_owned();
        request.headers = r
            .headers
            .iter()
            .map(|(k, v)| KeyValue::new(k.clone(), v.clone()))
            .collect();
        if let Some(body) = r.body.as_ref().filter(|b| !b.is_empty()) {
            request.body = if serde_json::from_str::<serde_json::Value>(body).is_ok() {
                Body::Json(body.clone())
            } else {
                Body::Text(body.clone())
            };
        }
        let names = used.entry(dir.clone()).or_default();
        let stem = unique(slug(&request.name, "request"), names);
        files.push(PlannedFile::Request {
            path: dir.join(request_file(&stem)),
            request,
        });
        report.requests += 1;
    }
    report.folders = folders.len();
    Plan {
        meta: CollectionMeta {
            name,
            ..CollectionMeta::default()
        },
        files,
        report,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apix_plugins::ImportedRequest;

    fn req(folder: &str, name: &str, method: &str) -> ImportedRequest {
        ImportedRequest {
            folder: folder.into(),
            name: name.into(),
            method: method.into(),
            url: "{{base_url}}/x".into(),
            headers: vec![("Accept".into(), "application/json".into())],
            body: None,
        }
    }

    #[test]
    fn requests_in_folders() {
        let mut posted = req("users", "Create user", "post");
        posted.body = Some("{\"a\":1}".into());
        let mut text = req("", "Ping", "PUT");
        text.body = Some("hello".into());
        let imported = Imported {
            name: String::new(),
            requests: vec![
                req("users", "List users", "GET"),
                req("users", "List users", "GET"),
                posted,
                req("admin / audit", "Log", "BREW"),
                text,
            ],
        };
        let plan = plan(&imported, "From plugin");
        assert_eq!(plan.meta.name, "From plugin");
        assert_eq!(plan.report.requests, 5);
        assert_eq!(plan.report.folders, 3);
        let paths: Vec<String> = plan
            .files
            .iter()
            .map(|f| match f {
                PlannedFile::Request { path, .. } => path.to_string_lossy().replace('\\', "/"),
                _ => String::new(),
            })
            .collect();
        assert_eq!(paths[0], "users/list-users.apix");
        assert_eq!(paths[1], "users/list-users-2.apix");
        assert_eq!(paths[3], "admin/audit/log.apix");
        let PlannedFile::Request { request, .. } = &plan.files[2] else {
            panic!()
        };
        assert_eq!(request.method, Method::Post);
        assert_eq!(request.body, Body::Json("{\"a\":1}".into()));
        let PlannedFile::Request { request, .. } = &plan.files[4] else {
            panic!()
        };
        assert_eq!(request.body, Body::Text("hello".into()));
        assert_eq!(plan.report.warnings.len(), 1, "{:?}", plan.report.warnings);
    }
}
