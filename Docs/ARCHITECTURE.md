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
5. **Light and fast to start.** Speed and features come before size. The release
   binary stays under about 40 MB (27.7 MB today; Electron apps are 300 MB+) and
   opens a window in under 150 ms.

## Crates

```
crates/
├── apix-core     request model, {{variables}} and {{$dynamic}} values, on-disk format,
│                 projects, environments and secrets, tree and file operations
├── apix-http     HTTP engine: reqwest + rustls, timings, response capture
├── apix-import   Postman v2.0/v2.1 collections and environments → Apix collections
├── apix-codegen  requests ↔ shell commands: curl (bash), curl.exe (cmd), PowerShell; parses pasted curl
├── apix-cli      `apix` binary: send / run / import / shell from terminal & CI
└── apix-desktop  GPUI desktop app (Windows, macOS and Linux from one crate)
tools/
└── icongen       builds every app icon (ico, icns, hicolor PNGs) from Logo/LogoFirst.jpg
```

Inside `apix-desktop/src`: `workspace.rs` (request editor, response panel,
actions and key bindings), `workspace/collections.rs` (sidebar, projects,
file operations, import, file watching), `workspace/env_editor.rs` (the
variables dialog), `workspace/shell_copy.rs` (copy as / paste curl),
`large_view.rs` (row-based viewer for huge bodies), `net.rs` (tokio bridge),
`state.rs` (recent items and per-folder environment in the user's config
folder), `theme.rs` and `fonts.rs`.

Planned crates:

| Crate          | Purpose                                                            |
|----------------|--------------------------------------------------------------------|
| `apix-script`  | Pre-request and test scripts (`pm.*`-compatible API) on QuickJS via `rquickjs` |
| `apix-proto`   | GraphQL, WebSocket (`tokio-tungstenite`), gRPC (`tonic` + reflection), SSE |
| `apix-vault`   | Encrypted collections: XChaCha20-Poly1305 per file, Argon2id key derivation, opaque file names |
| `apix-mock`    | A local mock server that serves saved examples (`hyper`)           |

### One desktop crate for three operating systems

GPUI and gpui-kit run on Windows, macOS and Linux from the same code, which is
how Zed ships. So there are no per-OS UI folders. Code that really differs per OS
goes behind `cfg` in `crates/apix-desktop/src/platform/{windows,macos,linux}.rs`:
title bar and window controls, blur/vibrancy, the macOS menu bar, file
associations for `.apix`, and keychain access. Installers and icons live in
`packaging/{windows,macos,linux}/`. A separate UI would only be split out if one
OS ever needs a genuinely different interface.

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
  `environments/<name>.local.toml` is layered over `<name>.toml`. Writing one adds
  `*.local.toml` to the `.gitignore` next to `environments/` if it is missing. A
  secret may keep a placeholder in the shared file, so teammates see the name.
  OS keychain storage through the `keyring` crate comes next.
- **Dynamic variables.** `{{$name}}` is filled by `apix_core::dynamic` when no
  variable of that name is defined, with a new value for every occurrence
  (`$guid`, `$timestamp`, `$isoTimestamp`, `$randomEmail`, …). No extra crates:
  a small xorshift generator and a hand-written UTC date formatter.
- **Ordering in the sidebar.** Folders and files are sorted by name for now. An
  optional `order` field may be added later.

### Projects

A project groups collections and owns environments shared by all of them:

```
shop/
├── apix-project.toml       name, shared [vars], collection order, group_by, pins
├── environments/           test.toml, prod.toml (+ *.local.toml secrets)
├── users/                  a collection (apix.toml + .apix files)
└── payments/               a collection; may be pinned to some environments
```

Variable layers, later wins: project vars → collection vars → the project's environment
→ the collection's environment of the same name (each with its `.local.toml` secrets).
The CLI (`apix run --env`) and the desktop app use the same function,
`apix_core::project::variables`. `group_by = "environment"` lays the sidebar out as
Project → Environment → Collections, and `[pins]` limits a collection to some environments.

### Planned: encrypted collections (M4)

An encrypted collection keeps the same folder layout, but every file is an
encrypted blob (XChaCha20-Poly1305, random nonce per file). File and folder names
are random IDs, and the real names live in an encrypted index. The key is derived
from a password with Argon2id. Its salt and parameters sit in a small plaintext
header in `apix.toml`, which reveals only that the collection is encrypted. Git
still sees which file changed, but not what is inside it. Security comes from the
cipher and the password, not from the format being secret; Apix is open source.

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
| Release binary                      | < 40 MB           | 27.7 MB |
| Input to paint latency              | 1 frame (≤ 8 ms at 120 Hz) | not measured |
| Open and scroll a 100 MB JSON body  | no dropped frames | opens in ~0.33 s; frame times not measured |

The release profile already uses fat LTO, one codegen unit and stripped
symbols (see the root `Cargo.toml`).
