# Apix architecture

Apix is a native API client written in Rust. The UI is drawn on the GPU with
[GPUI](https://github.com/zed-industries/zed) (Zed's UI engine) and the
[gpui-kit](https://github.com/longbridge/gpui-kit) component library. It has
no WebView and no Electron.

## Principles

1. **Local-first, git-friendly.** A collection is a folder of small TOML files.
   There is no account and no cloud requirement. Collections are committed,
   diffed and reviewed like code.
2. **The UI never blocks.** Network, disk, parsing and scripting run off the UI
   thread. The UI thread only lays out and paints.
3. **One engine, many frontends.** The desktop app, the `apix` CLI and future
   frontends (a TUI, an MCP server) share the same crates, so a request behaves
   the same everywhere, including in CI.
4. **Scale to huge responses.** A 100 MB JSON response has to scroll smoothly.
   Only the visible rows are laid out (virtual lists), and the body is parsed
   lazily.
5. **Small and fast to start.** The target is a release binary under about 15 MB
   that opens a window in under 150 ms.

## Crates

```
crates/
├── apix-core     request model, {{variables}}, on-disk format   (no I/O deps beyond std/serde)
├── apix-http     HTTP engine: reqwest + rustls, timings, response capture
├── apix-cli      `apix` binary: send / run collections from terminal & CI
└── apix-desktop  GPUI desktop app
```

Planned crates:

| Crate          | Purpose                                                            |
|----------------|--------------------------------------------------------------------|
| `apix-script`  | Pre-request and test scripts (`pm.*`-compatible API) on QuickJS via `rquickjs` |
| `apix-proto`   | GraphQL, WebSocket (`tokio-tungstenite`), gRPC (`tonic` + reflection), SSE |
| `apix-import`  | Importers for Postman v2.1, Insomnia, OpenAPI 3, HAR and cURL      |
| `apix-codegen` | Code snippet generation (curl, fetch, Python, Go, Rust, and others) |
| `apix-mock`    | A local mock server that serves saved examples (`hyper`)           |

Dependency direction: `core` ← `http`/`script`/`proto` ← `cli`/`desktop`.
Nothing depends on `desktop`.

## Data flow of a request

```
.apix file ──load──▶ Request ──resolve(Variables)──▶ Request (resolved)
                                                        │
                       pre-request script (later) ◀─────┤
                                                        ▼
                                         apix-http::HttpClient::send
                                                        │  (tokio runtime, background thread)
                                                        ▼
                    Response {status, headers, body: Bytes, timing}
                                                        │
                           test script (later) ◀────────┤
                                                        ▼
                         UI: status line, virtualized body viewer
```

`{{variables}}` resolve in layers. Later layers win:
**globals → collection (`apix.toml`) → environment → overrides (CLI `--var`, script `pm.variables.set`)**.
Undefined names are left in place and reported, so the UI can underline them
and the CLI can refuse to send.

## On-disk format

```
my-api/
├── apix.toml               name + collection-level [vars]
├── environments/
│   ├── dev.toml            [vars] base_url = "http://localhost:8080"
│   └── prod.toml
├── users/
│   ├── list-users.apix
│   └── create-user.apix
└── health.apix
```

A request file:

```toml
name = "Create user"
method = "POST"
url = "{{base_url}}/users"

[[headers]]
key = "X-Trace"
value = "on"
enabled = false          # kept in the file, not sent

[body]
json = '''
{ "name": "Ada" }
'''

[auth.bearer]
token = "{{token}}"
```

Open questions:
- **Secrets.** Environment values such as tokens should not be committed.
  `environments/<name>.local.toml` (gitignored) is already layered over `<name>.toml`;
  OS keychain storage through the `keyring` crate comes next.
- **Ordering in the sidebar.** Folders and files are sorted by name for now. An
  optional `order` field may be added later.

## Threading model (desktop)

- GPUI owns the main thread, which runs the event loop and rendering.
- A single multi-threaded **tokio runtime** is started once on a background
  thread. reqwest, tonic and tungstenite need it.
- The UI sends work to tokio and receives the result in a GPUI task
  (`cx.spawn_in`). Dropping that task aborts the request, so Esc and
  re-sending cancel cleanly.
- After a response arrives, the body is formatted and indexed on GPUI's
  background executor before the UI sees it. Formatting uses
  `apix_core::text::pretty_json`, a single-pass re-indenter that builds no
  tree and keeps numbers exactly as sent.

## Large response viewer

Bodies of 512 KB and up use `large_view.rs` instead of the code editor:

- `apix_core::text::RowIndex` records where every display row starts, 8 bytes
  per row. Lines over 2 KB (minified JSON) are split into several rows.
- The scroll position is a **row number**, not a pixel offset. Ten million
  rows at 20 px would be about 200 million px, well past the range where
  `f32` pixels are exact. The viewer lays out only the ~120 rows that fit on
  screen. It handles the wheel, its own scrollbar and the keyboard
  (↑ ↓ PgUp PgDn, Ctrl+Home/End).
- The Raw tab of a large JSON body is built only when it is opened.

Measured on Windows in a release build, for 100 MB of minified JSON from a
local server (`cargo run -p apix-http --release --example big_json_server`):
the response is on screen about 0.33 s after Enter. That covers the transfer,
formatting into 9.6 million lines, indexing and the first frame. Memory is
about 407 MB with the body loaded, against 69 MB idle.

## Performance budget

| Metric                              | Target            | Measured (Windows, release) |
|-------------------------------------|-------------------|-----------------------------|
| Cold start to first frame           | < 150 ms          | window handle at ~340 ms, not yet profiled |
| Idle RAM                            | < 60 MB           | 69 MB |
| Release binary                      | < 15 MB           | 27.7 MB |
| Input to paint latency              | 1 frame (≤ 8 ms at 120 Hz) | not measured |
| Open and scroll a 100 MB JSON body  | no dropped frames | opens in ~0.33 s; frame times not measured |

The release profile already uses fat LTO, one codegen unit and stripped
symbols (see the root `Cargo.toml`).
