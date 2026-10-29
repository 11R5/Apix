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
   binary stays under 100 MB (38.9 MB today; Electron apps are 300 MB+) and
   opens a window in under 150 ms.

## Crates

```
crates/
├── apix-core     request model, {{variables}} and {{$dynamic}} values, on-disk format,
│                 projects, environments and secrets, tree and file operations,
│                 GraphQL schemas (introspection, SDL) and query completion
├── apix-http     HTTP engine: reqwest + rustls, timings, response capture, network trace,
│                 server-sent events, WebSocket (tokio-tungstenite), GraphQL schema fetch
│                 (DNS, connection setup, redirects, certificate, local interface)
├── apix-import   Postman v2.0/v2.1 collections and environments → Apix collections
├── apix-codegen  requests ↔ shell commands: curl (bash), curl.exe (cmd), PowerShell; parses pasted curl; code in 10 languages
├── apix-vault    encrypted collections: per-file XChaCha20-Poly1305, Argon2id, recovery key
├── apix-script   pre-request and test scripts: QuickJS with a pm.*-compatible API
├── apix-runner   runs collections: order, iterations, data files, delay, bail, reports
├── apix-grpc     gRPC calls: tonic, messages as JSON (prost-reflect), .proto files (protox)
│                 or server reflection
├── apix-cli      `apix` binary: send / run / schema / import / shell from terminal & CI
└── apix-desktop  GPUI desktop app (Windows, macOS and Linux from one crate)
tools/
└── icongen       builds every app icon (ico, icns, hicolor PNGs) from Logo/LogoFirst.jpg
```

Inside `apix-desktop/src`: `workspace.rs` (request editor, response panel,
actions and key bindings), `workspace/collections.rs` (sidebar, projects,
file operations, import, file watching), `workspace/env_editor.rs` (the
variables dialog), `workspace/auth_editor.rs` (the Auth tab and the folder/collection/project
Auth dialog), `workspace/palette.rs` (the command palette and its fuzzy matcher),
`workspace/network.rs` (the response's Network tab), `workspace/runner_ui.rs` (the Runner), `workspace/vault_ui.rs` (unlock, lock and auto-lock, encrypt, decrypt, change
password, recovery key), `assets.rs` (gpui-kit icons plus the two lock icons),
`workspace/menu_bar.rs` (File/Request/Environment/View/Help in the title bar;
the buttons `occlude()` the title bar's drag area so Windows hands them the
click), `workspace/tabs.rs` (open requests; the front tab lives in the editor widgets,
the others keep their draft, response and in-flight task), `workspace/drag.rs`
(drag requests and folders to move them), `workspace/shell_copy.rs` (copy as / paste curl), `workspace/var_chips.rs`
(`{{variable}}` chips in the URL bar and their hover card), `workspace/graphql_ui.rs`
(GraphQL schema fetch, cache, completion and hover), `workspace/grpc_editor.rs` and
`workspace/grpc_ui.rs` (a gRPC request's Message tab, and the call in the response pane),
`large_view.rs` (row-based viewer for huge bodies), `net.rs` (tokio bridge),
`bidi.rs` (right-to-left layout helpers, see Languages),
`state.rs` (recent items and per-folder environment in the user's config
folder), `theme.rs` and `fonts.rs`.

Planned crates:

| Crate          | Purpose                                                            |
|----------------|--------------------------------------------------------------------|
| `apix-proto`   | Socket.IO, MQTT, SOAP                                              |
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
**globals → collection (`apix.toml`) → environment → overrides (CLI `--env-file`, `APIX_VAR_*` and `--var`, matched to the collection's names whatever their case; script `pm.variables.set`)**.
Undefined names are left in place and reported, so the UI can underline them
and the CLI can refuse to send.

## On-disk format

```
my-api/
├── apix.toml               name + collection-level [vars] and [auth]
├── environments/
│   ├── dev.toml            [vars] base_url = "http://localhost:8080"
│   └── prod.toml
├── users/
│   ├── folder.toml         optional: [auth] for the requests in this folder
│   ├── list-users.apix
│   └── create-user.apix
└── health.apix
```

**Auth inheritance.** A request without `auth` inherits it from the nearest
folder whose `folder.toml` sets one, then the collection (`apix.toml`), then the
project (`apix-project.toml`); with none, nothing is sent. `auth = "none"` in a
request (or folder) stops inheritance. `apix_core::project::inherited_auth`
does the lookup for the desktop app, the CLI and "Copy as" alike.

**Request settings.** A `[settings]` table (timeout, redirects, TLS verify,
proxy, client certificate, trusted CA, cookies) lives at the same four levels. Every field
is an `Option`: `None` inherits per field through the same chain
(`apix_core::project::inherited_settings`), and what is still unset falls back
to the defaults in `RequestSettings::effective` — no timeout, follow up to 10
redirects, verify TLS, proxy from the environment, cookies on. The engine
applies the result when it builds each send's `reqwest::Client` (a trusted CA
file adds its certificates as roots); the shared
cookie jar (a `cookie_store` the cookie manager can list and edit) is handed to
that client unless the request turns cookies off.

**Signed auth.** AWS Signature V4, OAuth 1.0a and Digest are pure functions in
`apix_core::sign`, checked against the AWS test suite, RFC 7616, RFC 7636 and
the classic OAuth 1.0 example, so the engine and "Copy as" sign the same way.
The engine builds the `reqwest::Request` first and then signs it (the AWS
payload hash needs the final body; a multipart stream is sent as
`UNSIGNED-PAYLOAD`). Digest sends once, reads the 401's challenge and answers
it on the same connection. OAuth 2.0 lives in `apix_http::oauth2`: the token is
fetched before sending (client credentials, password, or authorization code
with PKCE, where the browser comes back to a one-shot listener on 127.0.0.1),
kept in memory per server, client, grant, scope and user, refreshed with its
refresh token, and never written to disk.

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

[[assert]]               # a check without code, run after the test scripts
target = "status"
op = "=="
value = "201"
```

**Checks.** `[[assert]]` rows compare a target (`status`, `time`, `size`,
`body`, `body.<path>`, `header.<name>`) with a value using `==`, `!=`, `>`,
`>=`, `<`, `<=`, `contains`, `not contains`, `exists`, `not exists`, `matches`
or `is` (a JSON type). `apix_script::assertions_script` turns them into one
more test script, so they report like `pm.test` everywhere.

**Bodies.** `[body]` holds one of `json`, `text`, `xml`, `form` (URL-encoded rows),
`multipart` (rows with `file = true` for file fields and an optional
`content_type`), `binary = { file = "…" }` or `graphql` (`query` and `variables`).
File paths may contain variables and are relative to the request file;
`Request::with_files_relative_to` makes them absolute after variables are
resolved, in the app and the CLI alike. The engine reads files only when it
sends, so a missing file is a send error, not a load error. A GraphQL body is
sent as `{"query", "variables"}` JSON; invalid variables stop the send. With a
multipart body, a `Content-Type: multipart/…` header without the engine's
boundary (common in Postman exports) is dropped so the body stays readable.

**gRPC.** A request whose URL is `grpc://host:port` (or `grpcs://` for TLS)
names its method in `[grpc]`; the JSON body is the message, and a JSON list is
a stream of messages for a method that takes one. Without `protos` the server
is asked (reflection). Paths are relative to the request file.

```toml
name = "Say hello"
url = "grpc://localhost:50051"

[body]
json = '{"name": "Ada"}'

[grpc]
method = "helloworld.Greeter/SayHello"
protos = ["../protos/helloworld.proto"]   # optional
import_paths = ["../protos"]              # optional
```

Open questions:
- **Secrets.** Environment values such as tokens should not be committed.
  `environments/<name>.local.toml` is layered over `<name>.toml`. Writing one adds
  `*.local.toml` to the `.gitignore` next to `environments/` if it is missing. A
  secret may keep a placeholder in the shared file, so teammates see the name.
  Optionally (Settings › Environment secrets) the secrets go to the OS keychain
  instead: `<name>.local.toml` then holds only `keychain = "<id>"` and the
  values live in the entry `env:<id>` of the service `Apix`
  (`apix_vault::keychain::save_text`, split over several entries for Windows'
  2560-byte limit). `apix_core::secrets::use_keychain` decides where new
  secrets are written; reading follows the file, so the CLI needs no setting.
  The id travels with the file, so renaming the environment or moving the
  folder keeps the link. Inside an encrypted collection secrets stay in the
  vault.
- **Dynamic variables.** `{{$name}}` is filled by `apix_core::dynamic` when no
  variable of that name is defined, with a new value for every occurrence
  (`$guid`, `$timestamp`, `$isoTimestamp`, `$randomEmail`, …). No extra crates:
  a small xorshift generator and a hand-written UTC date formatter.
- **Ordering in the sidebar.** Without anything saved, folders come first, then
  requests, each by name. Dragging saves an `order` list of child names in the
  folder's `folder.toml` (or the collection's `apix.toml`); `store::place` puts
  one item before or after another, and renames and moves keep the list in step.
  Names the list does not know follow in the default order, so a new file never
  disappears. Project collections use `order` in `apix-project.toml`
  (`Project::place_collection`). The Postman importer writes `order` only when
  Postman's order differs from the default.

### Projects

A project groups collections and owns environments shared by all of them:

```
shop/
├── apix-project.toml       name, shared [vars], collection order, group_by, pins, colors
├── environments/           test.toml, prod.toml (+ *.local.toml secrets)
├── users/                  a collection (apix.toml + .apix files)
└── payments/               a collection; may be pinned to some environments
```

`[colors]` maps a collection folder to a colour name (`red`, `orange`, `yellow`,
`green`, `teal`, `blue`, `purple`, `pink`); the app draws it as a dot in the
sidebar and on tabs. `project::move_collection_into` moves a collection folder
into a project (copy and delete across drives).

Variable layers, later wins: project vars → collection vars → the project's environment
→ the collection's environment of the same name (each with its `.local.toml` secrets).
The CLI (`apix run --env`) and the desktop app use the same function,
`apix_core::project::variables`. `group_by = "environment"` lays the sidebar out as
Project → Environment → Collections, and `[pins]` limits a collection to some environments.

### Encrypted collections (M4)

The format and engine live in `crates/apix-vault`. The rest of Apix reaches them
through `apix_core::vfs`, a thin layer with the same calls as `std::fs`: a path
under the root of an unlocked vault is served from the vault, decrypted in
memory; every other path goes to the disk. So the store, the sidebar, tabs and
the CLI keep using ordinary paths, and nothing decrypted is ever written to disk.
Unlocked vaults live in a process-wide registry; locking (by hand, after 15 idle
minutes, or on quit) drops the vault and wipes its key. Deleting inside an
encrypted collection is permanent: a system trash would hold decrypted files.

```
my-api/
├── apix-vault.toml   plaintext header: format, Argon2id cost and salts,
│                     the data key wrapped by the password and by the recovery key
├── .gitignore        objects/local/
└── objects/
    ├── index.apixe   the real paths of every file, encrypted
    ├── 3f9c…e1.apixe one encrypted file each; random names, flat (no folder shape)
    └── local/        *.local.toml secrets, same format, never committed
```

- A random 256-bit **data key** encrypts every file with XChaCha20-Poly1305 and
  a fresh nonce per write. The object id is the associated data, so blobs cannot
  be swapped or renamed unnoticed.
- The data key is stored twice: wrapped by a key derived from the **password**
  (Argon2id, 64 MiB, 3 passes) and by one derived from the **recovery key**
  (160 random bits, shown once as `XXXX-XXXX-…` Crockford base32). Changing the
  password rewraps the key; files are not re-encrypted.
- Renames and moves only touch the encrypted index.
- The unlocked key lives in memory only (`zeroize` wipes it on lock).
- Git still sees which object changed, never what is in it. Security comes from
  the cipher and the password, not from the format being secret; Apix is open
  source.
- The password can be remembered in the OS credential store
  (`apix_vault::keychain`, through the `keyring` crate: Credential Manager,
  macOS Keychain, Secret Service). One entry per vault under the service `Apix`,
  named `vault:<absolute folder>` (lower case on Windows). It is opt-in from the
  unlock prompt; the app and the CLI try it before asking, a password that no
  longer opens the vault is deleted and asked for again, changing the password
  updates it and decrypting the collection deletes it. When no store is
  available Apix simply asks, as before.

## Scripts (M5)

`apix_core::scripts` finds the scripts of a request (`_collection.<event>.js`,
each folder's `_folder.<event>.js`, then `<request>.<event>.js`; events are
`prerequest` and `test`) and holds `ScriptVars`, what scripts set or unset per
scope. `ScriptVars::apply` lays them over the request's `Variables`, so
resolving works as before. Nothing scripts set is written to disk.

`apix-script` runs the scripts of one event in a fresh QuickJS runtime
(`rquickjs`; 10 s and 256 MB limits). The `pm` object is JavaScript
(`prelude.js`): Rust passes the context as JSON (the unresolved request, the
response for tests, the scope values from `project::scope_values`, earlier
`ScriptVars`, `pm.info`), the scripts run one by one wrapped in a function,
and `__apix_output()` returns the changed request and variables, test results,
console lines, `setNextRequest` and `skipRequest` as JSON. Keeping the API in
JavaScript keeps the Rust side to a few calls and makes Postman behaviour easy
to match.

`pm.sendRequest` calls a native `__apix_send` with the request as JSON; the
caller of `apix_script::run` passes a `Sender` that sends it and blocks until
the response is in (the CLI on a fresh runtime on the script thread, the app
on its network runtime). Scripts are wrapped in async functions, so the
callback style and `await` both work, and the time spent waiting is not
counted against the limit. `require` comes from `libs.js`: subsets of
crypto-js (digests and HMACs done in Rust through `__apix_hash` /
`__apix_hmac`), uuid, lodash, moment, ajv, tv4 and chai, written for Apix
rather than downloaded.

A send with scripts: pre-request scripts, then `ScriptVars::apply` and
resolving, the HTTP request, then test scripts with a `ResponseView`.
`apix_runner::exchange` does this for `apix send`, `apix run` and the Runner;
the desktop app's single send does it in `script_run.rs`, with the scripts on
a background thread and the request's own scripts taken from the editor, saved
or not.

`apix_runner::run` takes a `Plan` (request files in order, variables and
script scopes, data rows, iterations, delay, `bail`, transport overrides, the
script state to start from) and goes through it: per iteration it lays the
data row (then `--var` values) over the variables and `pm.iterationData`,
follows `setNextRequest` by request name, waits between requests, and reports
`Progress` (iteration, sending, item done) to a callback. A `Control` pauses
or stops it from another thread. It returns a `RunReport` of `ItemResult`s,
which `report.rs` writes as JSON, JUnit XML (one suite per request and
iteration, one case per test, a status case for requests without tests) and a
self-contained HTML page. `data.rs` reads CSV (RFC 4180, BOM, `;` headers) and
JSON arrays. The CLI prints progress as it comes; the desktop Runner
(`workspace/runner_ui.rs`) runs it on the network runtime and copies progress
out of a shared `Live` struct a few times a second; the view stays in the
workspace, so a run outlives its dialog.

## Streams and WebSocket (M6)

`HttpClient::send_with` reports `Progress::Head` (status and headers) as soon
as they are in and then every body `Progress::Chunk`; `send` is `send_with`
with nothing listening, and the response still holds the whole body.
`apix_http::sse` reads `text/event-stream` bodies piece by piece (`SseParser`:
any line ending, a piece may split anywhere, `id` carries over, an unfinished
last event is dropped).

`apix_http::ws::connect` builds the handshake from the request as the HTTP
side would send it (`prepare`: params, headers, auth; OAuth 2.0 becomes a
bearer token first) and connects with tokio-tungstenite over the same
rustls. `WsConnection::run` sends what a channel of `WsCommand`s asks and
reports `WsEvent`s (sent, received, closed with code and reason, error)
until either side closes; ping and pong are answered by tungstenite.

In the desktop app the network runtime fills shared structs
(`net::Incoming` for a send, `net::SocketIncoming` for a socket) and a UI
task reads them every 60–80 ms. An event-stream response becomes
`ResponseState::Streaming` (`workspace/stream_ui.rs`) with a live event list
that follows new events unless scrolled up; Esc turns what arrived into a
finished response. A `ws://` URL switches the request pane to the Messages
tab (`workspace/ws_editor.rs`) and the response pane to
`ResponseState::Socket` (`workspace/socket_ui.rs`). Both states are routed to
their tab like a finished response, so a stream or socket keeps running in a
tab in the back. The CLI prints events and messages from the same callbacks.

## GraphQL schemas (M6)

`apix_core::graphql` holds the standard `INTROSPECTION_QUERY`, a `Schema`
read from its answer (types with fields, arguments, input fields, enum
values, interfaces and possible types; `TypeRef` keeps lists and `!`) and
`to_sdl`, which writes the root types first and leaves out built-in scalars
and `__` types. For the editor it reads a query only as far as the cursor:
`spot_at` tokenises the text (comments, strings and `$variables` dropped)
and walks a stack of selections, argument lists, input objects and lists,
following aliases, `... on Type`, fragments and variable definitions, to say
what fits there (`Spot`). `suggestions` turns that into labels with their
types and descriptions, ranked by prefix, then substring, deprecated last;
`describe_at` gives the Markdown for hover. Nothing is validated: a broken
query still completes as well as it can. The variables editor completes too:
`variable_types` reads the `$name: Type` declarations, a small JSON scanner
finds the path of keys and list items down to the cursor and whether a key
or a value goes there, and `variable_suggestions` walks that path through
input fields and list items to suggest the variables not given yet, input
fields, enum values (as JSON strings) and `true`/`false`; a string being
typed is replaced with its quotes, including one the editor closed.

`apix_http::graphql::fetch_schema` sends the introspection query as a POST
with the request's URL, headers, auth and settings (the request is resolved
first) and reads the schema, or says why there is none (a non-2xx status with
the start of the body, or the server's `errors[0].message`).

In the app, `workspace/graphql_ui.rs` implements gpui-kit's
`CompletionProvider` and `HoverProvider` over a shared
`Rc<RefCell<Option<Rc<Schema>>>>` that the query editor holds; completions
replace the word being typed through an LSP text edit. The Body editor's
**Fetch schema** emits `FetchSchema`; the workspace builds the request as for
a send (inherited auth and settings, variables) and fetches it on the network
runtime. Each endpoint's answer is kept in `<config>/Apix/graphql/<FNV-1a of
the URL>.json` and loaded when a GraphQL request opens. The SDL view is a
read-only editor filled while drawing, only once it is shown.

## gRPC (M6)

`apix-grpc` makes calls without generated code. The service description is a
`prost_reflect::DescriptorPool`: from the request's `.proto` files through
protox (Rust, no `protoc`; imports are looked up in `import_paths` and next to
each file, Google's well-known types are built in), or from the server through
gRPC reflection (v1, then v1alpha; files the server leaves out are asked for
by name, then added in dependency order). Messages are `DynamicMessage`s:
JSON in through prost-reflect's serde support, JSON out with the `.proto`
field names and default values shown. `DynamicCodec` plugs them into tonic,
so `tonic::client::Grpc` runs all four kinds of method (unary, server
streaming, client streaming, bidirectional) over one `Channel` (HTTP/2 on the
hyper and rustls the HTTP engine already uses).

The request's headers and auth become metadata (Bearer, Basic, an API key
header, OAuth 2.0 through `apix_http::oauth2`; `-bin` keys are base64);
auth types that sign HTTP requests are refused with a reason. The timeout,
client certificate and trusted CA come from the request settings. With TLS
verification off, tonic (which has no such switch) is given a plain
`http://` address and the real `https` origin, and a connector of our own
(`insecure.rs`) does the TLS: rustls with ALPN h2 and a verifier that accepts
any certificate but still checks the handshake signatures. `call` reports `Sent`,
`Headers` and `Received` events as they happen and ends with a `CallOutcome`:
the status code and message, response metadata, trailers and every message.
A status other than OK is an outcome, not an error; errors are things that
stop a call from happening (no connection, no description, a message that
does not fit its type).

In the app, a `grpc://` URL turns the Body tab into the Message tab
(`workspace/grpc_editor.rs`): a searchable method picker filled by
**Load methods**, the `.proto` files, and the message with an example built
from the input type. The call runs on the network runtime and fills a shared
struct the UI reads every 60 ms; the response pane is a log
(`ResponseState::Grpc`, `workspace/grpc_ui.rs`) like a WebSocket's, and Esc
drops the call. The CLI prints a unary answer whole and streams line by line,
and `apix schema` writes the service as `.proto`-like text.

Runs call gRPC requests through `apix_grpc::send_for_tests`, which turns the
`CallOutcome` into an `apix_http::Response` (`CallOutcome::to_response`): the
HTTP status the code stands for (Google's mapping) with the code's name as the
reason, the metadata and trailers plus `grpc-status` and `grpc-message` as
headers, and the message (a JSON list for a server stream) as the body. Test
scripts, checks and reports then need nothing gRPC-specific. The history keeps
a call with `Summary::grpc` set: the status is the gRPC code, the trailers and
status message have their own fields, and the body is the log
(`[{"at_ms", "sent", "message"}]`), which `grpc_ui::restored` turns back into
the log view.

## Themes

`theme.rs` holds the colours. A `ThemeDef` is a theme (built in, or read from
`<config>/Apix/themes/*.toml`; colours a file leaves out come from Obsidian or
Daylight) with `Colors` and a dark/light flag. `Palette::build` turns one into
what views paint with: the accent setting (a named colour with a dark and a
deeper light shade, or `#RRGGBB`), the text on the accent picked by relative
luminance, hover and input tints for dark or light, and the transparency
setting: the window background's alpha is `1 - t`, panels clear at 60% of
that rate, borders brighten towards white as `t` grows, and light themes use
70% of `t`. `from_settings` picks the dark or light theme by the appearance
setting and, for "follow the system", `App::window_appearance`.

gpui-kit's root view paints `Theme::background` under the whole window, which
would hide the OS blur, so with any transparency it is set transparent and
dialogs get their own background through `bidi::DialogBidi`, which every
dialog uses. The OS does the blur: `WindowBackgroundAppearance::Blurred` is
acrylic on Windows (`SetWindowCompositionAttribute`) and vibrancy on macOS.

Views never keep a copy of the palette: they call `theme::current()` when they
paint. `theme::set` stores the new palette, points gpui-kit's component theme
at it (switching its mode, so the code editor's colours follow), sets every
window's background to blurred or opaque and refreshes the windows, so a
change shows at once. The workspace observes the window appearance and calls
it again when the system switches between light and dark.

## Languages (M4)

Every text in the desktop UI goes through a key: `tr!("menu.file")` (a thin
wrapper over `rust_i18n::t!` that returns a `SharedString` to show; `tr_plain!`
gives the same text for what is saved, such as a new request's default name).
rust-i18n is the
same system gpui-kit uses, so both follow one process-wide locale; Chinese uses
`zh-CN`, the code gpui-kit's own texts use. Each language is one flat file,
`crates/apix-desktop/locales/<code>.yml` (one `key: value` per line, sorted,
`%{name}` placeholders), compiled into the binary; `build.rs` rebuilds when a
file changes. A missing text falls back to English, and a unit test fails when a
language lacks a key, has an unknown one, or uses other placeholders than
English. Sentences are whole keys with placeholders, never glued from pieces,
because word order differs between languages.

The language is chosen in Settings (or follows the system language on first
start) and switches at once: texts are read on every render, and the few texts
kept inside widgets (input placeholders, picker entries) are set again by
`Workspace::retranslate`. Glyphs for Japanese, Chinese and Arabic come from the
platform's font fallback (DirectWrite, Core Text, fontconfig) next to the
embedded Inter. Arabic text is shaped and placed right to left; on Windows
that needs `vendor/gpui-pre-windows`, gpui 0.3.9's DirectWrite backend with its
glyph-run placement fixed (patched in from the workspace `Cargo.toml`; drop it
once upstream fixes it). gpui cannot wrap right-to-left text: it shapes a
paragraph as one line and cuts it at glyph x positions that it assumes grow
from left to right. So text that may wrap (notes, hints, descriptions,
messages) goes through `bidi::para`, which in RTL breaks the text at spaces in
reading order, measuring each candidate line, and paints each line unwrapped
(`src/bidi/paragraph.rs`). In other languages it is plain gpui text. gpui also
lays every line out with a left-to-right base direction, so in an RTL language
`tr!` wraps each text in a right-to-left embedding (U+202B…U+202C; DirectWrite
ignores the newer isolates). A sentence that starts or ends with a Latin word
("URL", "OAuth 2.0", "Ctrl+S") then still reads right to left, and
`i18n::display` does the same for composed texts such as the status bar. The
marks never reach files: saved texts use `tr_plain!`. Error messages from the
engine crates stay English.

In a right-to-left language (`Language::rtl`, Arabic) the window mirrors: the
sidebar moves to the right, the response pane to the left of the request pane,
the window controls to the left, and menus, tab strips, form rows and the status
bar run from the right, with text aligned to the right. GPUI has no layout
direction, so this is done by `src/bidi.rs`: a `Bidi` trait on every styled
element (`dir_row` reverses a row, `text_start` / `text_end`, `ps` / `pe`
paddings and `border_e_1` pick the side by direction, `ltr` pins text left),
`tab_bar` / `fill_tabs` for gpui-kit's `TabBar` (tabs given in reverse, indices
mapped back), and `bidi_title` for dialogs (in RTL the close button moves into
the title row on the left). The helpers read the language while a view renders,
so a language change mirrors the window at once. Code and response viewers, the
URL bar, key–value tables (params, headers, form fields, variables) and response
headers stay left to right in every language. gpui-kit's popup menus and buttons
keep their own left-to-right inner layout (shortcut on the right, icon before the
label); menus open towards the left from their button.

Adding a language: copy `locales/en.yml` to `locales/<code>.yml`, translate
the values, and add one line to `LANGUAGES` in `src/i18n.rs`.

User documentation lives apart from the plan, one folder per language
(`Docs/<code>/`: user guide, architecture, technical, workflows, images), listed
in `Docs/languages.toml`. English is the source; each translation records the
English version it was made from, so stale translations show up.

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
| Cold start to first frame           | < 150 ms          | 278 ms (`main` to first frame) |
| Idle RAM                            | < 60 MB           | 84 MB working set, example collection open |
| Release binary                      | < 100 MB          | 38.9 MB |
| Input to paint latency              | 1 frame (≤ 8 ms at 120 Hz) | not measured |
| Open and scroll a 100 MB JSON body  | no dropped frames | on screen 306 ms after sending (local server); frame times not measured |

Start time and idle memory are above budget today; profiling them is planned.
The numbers come from `scripts/measure.ps1` (see the [technical notes](technical.md#measuring-performance)).

The release profile already uses fat LTO, one codegen unit and stripped
symbols (see the root `Cargo.toml`).
