//! API documentation from a collection: a page that opens in any browser
//! (one HTML file, light or dark, with a table of contents) or Markdown for a
//! repository. It shows what the files say: requests in sidebar order under
//! their folders, with method, URL, description, parameters, headers, body,
//! auth type, checks and saved examples, and the environments' variables.
//!
//! Nothing secret is printed: values of `.local.toml` files and the keychain
//! are never read, auth shows only its kind, and a header that usually holds
//! a credential (`Authorization`, `Cookie`, API keys) shows its value only
//! when it is a `{{variable}}`.

use std::path::Path;

use apix_core::store::{ENVIRONMENTS_DIR, TreeNode, load_request};
use apix_core::{Auth, Body, Collection, Example, KeyValue, Request};

/// The documented collection.
#[derive(Debug, Clone, Default)]
pub struct Docs {
    pub name: String,
    pub description: String,
    /// The collection's own variables, as in `apix.toml`.
    pub variables: Vec<(String, String)>,
    /// Each environment with the variables committed in its file.
    pub environments: Vec<(String, Vec<(String, String)>)>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub enum Item {
    Folder { name: String, items: Vec<Item> },
    Request(Box<Entry>),
}

/// One request, ready to print.
#[derive(Debug, Clone)]
pub struct Entry {
    /// For links: `users-get-user`.
    pub anchor: String,
    /// The file, relative to the collection.
    pub path: String,
    pub request: Request,
}

/// Header names whose values are credentials unless they are a variable.
const SECRET_HEADERS: [&str; 7] = [
    "authorization",
    "proxy-authorization",
    "cookie",
    "x-api-key",
    "api-key",
    "x-auth-token",
    "x-access-token",
];

/// The value to print for a header.
fn shown_value(row: &KeyValue) -> String {
    let secret = SECRET_HEADERS
        .iter()
        .any(|name| row.key.eq_ignore_ascii_case(name));
    let only_variables = row
        .value
        .split("{{")
        .skip(1)
        .all(|part| part.contains("}}"))
        && row.value.contains("{{");
    if secret && !only_variables && !row.value.is_empty() {
        "•••".into()
    } else {
        row.value.clone()
    }
}

fn anchor(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_end_matches('-').to_owned()
}

/// Reads the collection into [`Docs`].
pub fn collect(collection: &Collection) -> Docs {
    let mut used = std::collections::HashSet::new();
    let root = &collection.root;
    fn walk(
        nodes: &[TreeNode],
        root: &Path,
        used: &mut std::collections::HashSet<String>,
    ) -> Vec<Item> {
        let mut out = Vec::new();
        for node in nodes {
            match node {
                TreeNode::Folder { name, children, .. } => {
                    let items = walk(children, root, used);
                    if !items.is_empty() {
                        out.push(Item::Folder {
                            name: name.clone(),
                            items,
                        });
                    }
                }
                TreeNode::Request { path } => {
                    let Ok(mut request) = load_request(path) else {
                        continue;
                    };
                    let rel = path
                        .strip_prefix(root)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    if request.name.is_empty() {
                        request.name = rel.trim_end_matches(".apix").to_owned();
                    }
                    let base = anchor(rel.trim_end_matches(".apix"));
                    let mut id = base.clone();
                    let mut n = 2;
                    while !used.insert(id.clone()) {
                        id = format!("{base}-{n}");
                        n += 1;
                    }
                    out.push(Item::Request(Box::new(Entry {
                        anchor: id,
                        path: rel,
                        request,
                    })));
                }
            }
        }
        out
    }
    let environments = collection
        .environment_names()
        .into_iter()
        .map(|name| {
            let file = root.join(ENVIRONMENTS_DIR).join(format!("{name}.toml"));
            let vars = apix_core::vfs::read_to_string(&file)
                .ok()
                .and_then(|text| toml::from_str::<apix_core::Environment>(&text).ok())
                .map(|e| e.vars.into_iter().collect())
                .unwrap_or_default();
            (name, vars)
        })
        .collect();
    Docs {
        name: collection.meta.name.clone(),
        description: collection.meta.description.clone(),
        variables: collection
            .meta
            .vars
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        environments,
        items: walk(&collection.tree(), root, &mut used),
    }
}

/// What kind of request it is, when not plain HTTP.
fn protocol(request: &Request) -> Option<&'static str> {
    let url = request.url.trim_start().to_ascii_lowercase();
    if request.is_grpc() {
        Some("gRPC")
    } else if request.is_socketio() {
        Some("Socket.IO")
    } else if request.is_mqtt() {
        Some("MQTT")
    } else if url.starts_with("ws://") || url.starts_with("wss://") {
        Some("WebSocket")
    } else if matches!(request.body, Body::GraphQl(_)) {
        Some("GraphQL")
    } else {
        None
    }
}

/// The body as (language, text) blocks or a table of rows.
enum BodyView {
    None,
    Code(&'static str, String),
    Rows(Vec<(String, String)>),
    GraphQl(String, String),
}

fn body_view(body: &Body) -> BodyView {
    match body {
        Body::None => BodyView::None,
        Body::Json(text) => BodyView::Code(
            "json",
            apix_core::text::pretty_json(text.as_bytes()).unwrap_or_else(|| text.clone()),
        ),
        Body::Text(text) => BodyView::Code("text", text.clone()),
        Body::Xml(text) => BodyView::Code("xml", text.clone()),
        Body::Html(text) => BodyView::Code("html", text.clone()),
        Body::Form(rows) => BodyView::Rows(
            rows.iter()
                .filter(|r| r.enabled)
                .map(|r| (r.key.clone(), r.value.clone()))
                .collect(),
        ),
        Body::Multipart(parts) => BodyView::Rows(
            parts
                .iter()
                .filter(|p| p.enabled)
                .map(|p| {
                    let value = if p.file {
                        format!("file: {}", p.value)
                    } else {
                        p.value.clone()
                    };
                    (p.key.clone(), value)
                })
                .collect(),
        ),
        Body::Binary { file } => BodyView::Code("text", format!("file: {file}")),
        Body::GraphQl(g) => BodyView::GraphQl(g.query.clone(), g.variables.clone()),
    }
}

fn example_body(example: &Example) -> (&'static str, String) {
    if !example.body_base64.is_empty() {
        return ("text", "(binary)".into());
    }
    let pretty = apix_core::text::pretty_json(example.body.as_bytes());
    let mut text = pretty.clone().unwrap_or_else(|| example.body.clone());
    const MAX: usize = 4000;
    if text.len() > MAX {
        let cut = (0..=MAX)
            .rev()
            .find(|i| text.is_char_boundary(*i))
            .unwrap_or(0);
        text.truncate(cut);
        text.push_str("\n…");
    }
    (if pretty.is_some() { "json" } else { "text" }, text)
}

fn auth_line(auth: &Auth) -> Option<String> {
    match auth {
        Auth::Inherit => None,
        other => Some(other.label().to_owned()),
    }
}

// ---------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------

fn md_cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\r', '\n'], " ")
}

fn fence(lang: &str, text: &str) -> String {
    let ticks = if text.contains("```") { "````" } else { "```" };
    format!("{ticks}{lang}\n{}\n{ticks}\n", text.trim_end())
}

impl Docs {
    pub fn to_markdown(&self) -> String {
        let mut out = format!("# {}\n\n", self.name);
        if !self.description.trim().is_empty() {
            out += self.description.trim();
            out += "\n\n";
        }
        if !self.variables.is_empty() || !self.environments.is_empty() {
            out += "## Variables\n\n";
            if !self.variables.is_empty() {
                out += "| Variable | Value |\n|---|---|\n";
                for (k, v) in &self.variables {
                    out += &format!("| `{}` | `{}` |\n", md_cell(k), md_cell(v));
                }
                out += "\n";
            }
            for (name, vars) in &self.environments {
                out += &format!("**Environment `{name}`**");
                if vars.is_empty() {
                    out += ": no committed variables\n\n";
                    continue;
                }
                out += "\n\n| Variable | Value |\n|---|---|\n";
                for (k, v) in vars {
                    out += &format!("| `{}` | `{}` |\n", md_cell(k), md_cell(v));
                }
                out += "\n";
            }
        }
        fn walk_md(out: &mut String, items: &[Item], depth: usize) {
            for item in items {
                match item {
                    Item::Folder { name, items: inner } => {
                        *out += &format!("{} {name}\n\n", "#".repeat((depth + 2).min(6)));
                        walk_md(out, inner, depth + 1);
                    }
                    Item::Request(entry) => request_md(out, entry, depth),
                }
            }
        }
        walk_md(&mut out, &self.items, 0);
        out
    }
}

fn request_md(out: &mut String, entry: &Entry, depth: usize) {
    let r = &entry.request;
    let level = "#".repeat((depth + 2).min(6));
    *out += &format!("{level} {}\n\n", r.name);
    let kind = protocol(r).map(|p| format!(" · {p}")).unwrap_or_default();
    *out += &format!("`{}` `{}`{kind}\n\n", r.method.as_str(), r.url);
    if !r.description.trim().is_empty() {
        *out += r.description.trim();
        *out += "\n\n";
    }
    let table = |out: &mut String, title: &str, rows: Vec<(String, String)>| {
        if rows.is_empty() {
            return;
        }
        *out += &format!("**{title}**\n\n| Name | Value |\n|---|---|\n");
        for (k, v) in rows {
            *out += &format!("| `{}` | `{}` |\n", md_cell(&k), md_cell(&v));
        }
        *out += "\n";
    };
    let enabled = |rows: &[KeyValue]| -> Vec<(String, String)> {
        rows.iter()
            .filter(|r| r.enabled)
            .map(|r| (r.key.clone(), r.value.clone()))
            .collect()
    };
    table(out, "Path variables", enabled(&r.path_vars));
    table(out, "Query", enabled(&r.query));
    table(
        out,
        "Headers",
        r.headers
            .iter()
            .filter(|h| h.enabled)
            .map(|h| (h.key.clone(), shown_value(h)))
            .collect(),
    );
    if let Some(auth) = auth_line(&r.auth) {
        *out += &format!("**Auth:** {auth}\n\n");
    }
    match body_view(&r.body) {
        BodyView::None => {}
        BodyView::Code(lang, text) => *out += &format!("**Body**\n\n{}\n", fence(lang, &text)),
        BodyView::Rows(rows) => table(out, "Body", rows),
        BodyView::GraphQl(query, vars) => {
            *out += &format!("**Query**\n\n{}\n", fence("graphql", &query));
            if !vars.trim().is_empty() {
                *out += &format!("**Variables**\n\n{}\n", fence("json", &vars));
            }
        }
    }
    if let Some(grpc) = &r.grpc {
        *out += &format!("**Method:** `{}`\n\n", grpc.method);
    }
    let checks: Vec<String> = r
        .asserts
        .iter()
        .filter(|a| a.enabled)
        .map(|a| format!("- `{}`", a.label()))
        .collect();
    if !checks.is_empty() {
        *out += &format!("**Checks**\n\n{}\n\n", checks.join("\n"));
    }
    for example in &r.examples {
        let (lang, text) = example_body(example);
        *out += &format!(
            "**Example: {}** · `{}`\n\n{}\n",
            example.name,
            example.status,
            fence(lang, &text)
        );
    }
    *out += &format!("<sub>{}</sub>\n\n", entry.path);
}

// ---------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Markdown descriptions, with raw HTML left out.
fn markdown_html(text: &str) -> String {
    markdown::to_html(text)
}

impl Docs {
    /// A page that opens anywhere, with no files beside it.
    pub fn to_html(&self) -> String {
        let mut toc = String::new();
        let mut body = String::new();
        fn walk(items: &[Item], toc: &mut String, body: &mut String, depth: usize) {
            for item in items {
                match item {
                    Item::Folder { name, items: inner } => {
                        *toc += &format!("<li class=\"folder\">{}<ul>", esc(name));
                        *body += &format!(
                            "<h{l} class=\"folder\">{}</h{l}>\n",
                            esc(name),
                            l = (depth + 2).min(6)
                        );
                        walk(inner, toc, body, depth + 1);
                        *toc += "</ul></li>";
                    }
                    Item::Request(entry) => {
                        let r = &entry.request;
                        *toc += &format!(
                            "<li><a href=\"#{}\"><span class=\"m m-{}\">{}</span> {}</a></li>",
                            entry.anchor,
                            r.method.as_str().to_lowercase(),
                            r.method.as_str(),
                            esc(&r.name)
                        );
                        *body += &request_html(entry);
                    }
                }
            }
        }
        walk(&self.items, &mut toc, &mut body, 0);
        let mut vars = String::new();
        let rows = |pairs: &[(String, String)]| {
            pairs
                .iter()
                .map(|(k, v)| {
                    format!(
                        "<tr><td><code>{}</code></td><td><code>{}</code></td></tr>",
                        esc(k),
                        esc(v)
                    )
                })
                .collect::<String>()
        };
        if !self.variables.is_empty() {
            vars += &format!(
                "<h3>Collection</h3><table><tr><th>Variable</th><th>Value</th></tr>{}</table>",
                rows(&self.variables)
            );
        }
        for (name, pairs) in &self.environments {
            vars += &format!("<h3>Environment <code>{}</code></h3>", esc(name));
            vars += &if pairs.is_empty() {
                "<p class=\"muted\">No committed variables.</p>".to_owned()
            } else {
                format!(
                    "<table><tr><th>Variable</th><th>Value</th></tr>{}</table>",
                    rows(pairs)
                )
            };
        }
        let vars = if vars.is_empty() {
            String::new()
        } else {
            format!("<section id=\"variables\"><h2>Variables</h2>{vars}</section>")
        };
        format!(
            r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
:root {{ --bg:#f7f9fa; --panel:#fff; --text:#1a2226; --muted:#66757c; --line:#e3e8ea; --accent:#0e8fa3; --code:#eef3f5;
  --get:#14866d; --post:#b7791f; --put:#2b6cb0; --patch:#805ad5; --delete:#c62f3a; }}
@media (prefers-color-scheme: dark) {{ :root {{ --bg:#0d1417; --panel:#141e22; --text:#dce6e9; --muted:#8a9aa0; --line:#22313a; --accent:#3cc8dc; --code:#0f181c;
  --get:#3ccf9e; --post:#f6c35b; --put:#63b3ed; --patch:#b794f4; --delete:#ff6b73; }} }}
* {{ box-sizing:border-box; }}
body {{ margin:0; background:var(--bg); color:var(--text); font:15px/1.55 system-ui,-apple-system,"Segoe UI",sans-serif; }}
nav {{ position:fixed; top:0; bottom:0; left:0; width:280px; overflow:auto; padding:20px 14px; border-right:1px solid var(--line); background:var(--panel); }}
nav h1 {{ font-size:17px; margin:0 0 12px; }}
nav ul {{ list-style:none; margin:0; padding:0 0 0 10px; }}
nav > ul {{ padding:0; }}
nav li {{ margin:2px 0; font-size:13px; }}
nav li.folder {{ color:var(--muted); font-weight:600; margin-top:8px; }}
nav li.folder li {{ font-weight:400; }}
nav a {{ color:var(--text); text-decoration:none; }}
nav a:hover {{ color:var(--accent); }}
main {{ margin-left:280px; padding:28px 36px 60px; max-width:1060px; }}
@media (max-width: 800px) {{ nav {{ position:static; width:auto; border-right:none; border-bottom:1px solid var(--line); }} main {{ margin-left:0; padding:20px 16px; }} }}
h1 {{ font-size:26px; margin:0 0 6px; }}
h2 {{ font-size:21px; margin:34px 0 10px; }}
h2.folder, h3.folder, h4.folder {{ border-bottom:1px solid var(--line); padding-bottom:4px; }}
h3 {{ font-size:17px; margin:22px 0 8px; }}
.request {{ background:var(--panel); border:1px solid var(--line); border-radius:10px; padding:16px 18px; margin:16px 0; }}
.request h3 {{ margin:0 0 8px; }}
.url {{ font-family:ui-monospace,Consolas,monospace; font-size:13px; word-break:break-all; }}
.m {{ font-weight:700; font-size:11px; letter-spacing:.02em; }}
.m-get {{ color:var(--get); }} .m-post {{ color:var(--post); }} .m-put {{ color:var(--put); }} .m-patch {{ color:var(--patch); }} .m-delete {{ color:var(--delete); }}
.kind {{ font-size:11px; color:var(--muted); border:1px solid var(--line); border-radius:4px; padding:0 5px; margin-left:6px; }}
.label {{ font-size:12px; color:var(--muted); font-weight:600; text-transform:uppercase; letter-spacing:.04em; margin:14px 0 4px; }}
table {{ border-collapse:collapse; width:100%; font-size:13px; }}
th, td {{ text-align:left; border-bottom:1px solid var(--line); padding:5px 8px; vertical-align:top; }}
th {{ color:var(--muted); font-weight:600; font-size:12px; }}
code {{ font-family:ui-monospace,Consolas,monospace; font-size:12.5px; }}
pre {{ background:var(--code); border:1px solid var(--line); border-radius:8px; padding:10px 12px; overflow:auto; font-size:12.5px; max-height:420px; }}
.muted, .file {{ color:var(--muted); font-size:12px; }}
.status {{ font-weight:600; }}
</style>
</head>
<body>
<nav><h1>{name}</h1><ul><li><a href="#variables">Variables</a></li>{toc}</ul></nav>
<main>
<h1>{name}</h1>
{description}
{vars}
{body}
<p class="muted">Made by Apix</p>
</main>
</body>
</html>
"##,
            title = esc(&self.name),
            name = esc(&self.name),
            description = markdown_html(&self.description),
        )
    }
}

fn request_html(entry: &Entry) -> String {
    let r = &entry.request;
    let mut out = format!(
        "<section class=\"request\" id=\"{}\"><h3>{}</h3><div class=\"url\"><span class=\"m m-{}\">{}</span> {}{}</div>",
        entry.anchor,
        esc(&r.name),
        r.method.as_str().to_lowercase(),
        r.method.as_str(),
        esc(&r.url),
        protocol(r)
            .map(|p| format!("<span class=\"kind\">{p}</span>"))
            .unwrap_or_default()
    );
    if !r.description.trim().is_empty() {
        out += &markdown_html(&r.description);
    }
    let table = |title: &str, rows: Vec<(String, String)>| -> String {
        if rows.is_empty() {
            return String::new();
        }
        let body: String = rows
            .iter()
            .map(|(k, v)| {
                format!(
                    "<tr><td><code>{}</code></td><td><code>{}</code></td></tr>",
                    esc(k),
                    esc(v)
                )
            })
            .collect();
        format!("<div class=\"label\">{title}</div><table>{body}</table>")
    };
    let enabled = |rows: &[KeyValue]| -> Vec<(String, String)> {
        rows.iter()
            .filter(|r| r.enabled)
            .map(|r| (r.key.clone(), r.value.clone()))
            .collect()
    };
    out += &table("Path variables", enabled(&r.path_vars));
    out += &table("Query", enabled(&r.query));
    out += &table(
        "Headers",
        r.headers
            .iter()
            .filter(|h| h.enabled)
            .map(|h| (h.key.clone(), shown_value(h)))
            .collect(),
    );
    if let Some(auth) = auth_line(&r.auth) {
        out += &format!("<div class=\"label\">Auth</div><p>{}</p>", esc(&auth));
    }
    match body_view(&r.body) {
        BodyView::None => {}
        BodyView::Code(_, text) => {
            out += &format!("<div class=\"label\">Body</div><pre>{}</pre>", esc(&text));
        }
        BodyView::Rows(rows) => out += &table("Body", rows),
        BodyView::GraphQl(query, vars) => {
            out += &format!("<div class=\"label\">Query</div><pre>{}</pre>", esc(&query));
            if !vars.trim().is_empty() {
                out += &format!(
                    "<div class=\"label\">Variables</div><pre>{}</pre>",
                    esc(&vars)
                );
            }
        }
    }
    if let Some(grpc) = &r.grpc {
        out += &format!(
            "<div class=\"label\">Method</div><p><code>{}</code></p>",
            esc(&grpc.method)
        );
    }
    let checks: String = r
        .asserts
        .iter()
        .filter(|a| a.enabled)
        .map(|a| format!("<li><code>{}</code></li>", esc(&a.label())))
        .collect();
    if !checks.is_empty() {
        out += &format!("<div class=\"label\">Checks</div><ul>{checks}</ul>");
    }
    for example in &r.examples {
        let (_, text) = example_body(example);
        out += &format!(
            "<div class=\"label\">Example: {} · <span class=\"status\">{}</span></div><pre>{}</pre>",
            esc(&example.name),
            example.status,
            esc(&text)
        );
    }
    out += &format!("<div class=\"file\">{}</div></section>\n", esc(&entry.path));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collection() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("apix-docs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("users")).unwrap();
        std::fs::create_dir_all(root.join("environments")).unwrap();
        std::fs::write(
            root.join("apix.toml"),
            "name = \"Shop <API>\"\ndescription = \"The **shop**.\\n\\n<script>alert(1)</script>\"\n[vars]\nbase_url = \"https://shop.test\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("environments/dev.toml"),
            "[vars]\nbase_url = \"http://localhost\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("environments/dev.local.toml"),
            "[vars]\ntoken = \"s3cret\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("users/get.apix"),
            r#"name = "Get user"
description = "Returns one user."
url = "{{base_url}}/users/:id"

[[path]]
key = "id"
value = "1"

[[headers]]
key = "Authorization"
value = "Bearer abc123"

[[headers]]
key = "X-Api-Key"
value = "{{api_key}}"

[[assert]]
target = "status"
op = "=="
value = "200"

[[example]]
name = "Found"
status = 200
body = '{"id":1}'
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("health.apix"),
            "name = \"Health\"\nmethod = \"POST\"\nurl = \"{{base_url}}/health\"\n[body]\njson = '{\"ping\":true}'\n",
        )
        .unwrap();
        root
    }

    #[test]
    fn markdown_and_html_without_secrets() {
        let root = collection();
        let docs = collect(&Collection::open(&root).unwrap());
        assert_eq!(docs.environments.len(), 1);
        assert!(matches!(&docs.items[0], Item::Folder { name, .. } if name == "users"));

        let md = docs.to_markdown();
        assert!(md.starts_with("# Shop <API>\n"));
        assert!(md.contains("## users\n"));
        assert!(md.contains("### Get user\n"));
        assert!(md.contains("`GET` `{{base_url}}/users/:id`"));
        assert!(md.contains("| `Authorization` | `•••` |"), "{md}");
        assert!(md.contains("| `X-Api-Key` | `{{api_key}}` |"));
        assert!(md.contains("- `status == 200`"), "{md}");
        assert!(md.contains("**Example: Found** · `200`"));
        assert!(md.contains("\"ping\": true"));
        assert!(!md.contains("s3cret") && !md.contains("abc123"));

        let html = docs.to_html();
        assert!(html.contains("<title>Shop &lt;API&gt;</title>"));
        assert!(html.contains("<strong>shop</strong>"));
        assert!(!html.contains("<script>alert"), "raw HTML stays out");
        assert!(html.contains("id=\"users-get\""));
        assert!(html.contains("href=\"#users-get\""));
        assert!(!html.contains("s3cret") && !html.contains("abc123"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
