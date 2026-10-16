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
.toml file ──load──▶ Request ──resolve(Variables)──▶ Request (resolved)
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
│   ├── list-users.toml
│   └── create-user.toml
└── health.toml
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
  The plan is `environments/*.local.toml` (gitignored) layered on top, plus
  OS keychain storage through the `keyring` crate.
- **Ordering in the sidebar.** Folders and files are sorted by name for now. An
  optional `order` field may be added later.

## Threading model (desktop)

- GPUI owns the main thread, which runs the event loop and rendering.
- A single multi-threaded **tokio runtime** is started once on a background
  thread. reqwest, tonic and tungstenite need it.
- The UI sends work to tokio and receives the result in a GPUI task
  (`cx.spawn`). It then updates the view's state and calls `cx.notify()`.
- Large bodies stay as `Bytes`. They are never cloned into `String`s for the
  whole document. The viewer indexes line offsets once, in the background, and
  renders only the visible slice with `uniform_list`.

## Performance budget

| Metric                              | Target            |
|-------------------------------------|-------------------|
| Cold start to first frame           | < 150 ms          |
| Idle RAM                            | < 60 MB           |
| Release binary                      | < 15 MB           |
| Input to paint latency              | 1 frame (≤ 8 ms at 120 Hz) |
| Open and scroll a 100 MB JSON body  | no dropped frames |

The release profile already uses fat LTO, one codegen unit and stripped
symbols (see the root `Cargo.toml`).
