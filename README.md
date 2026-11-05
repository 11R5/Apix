<p align="center"><img src="packaging/icons/apix-128.png" width="128" height="128" alt="Apix logo"></p>
<p align="center"><a href="https://github.com/11R5/Apix/actions/workflows/ci.yml"><img src="https://github.com/11R5/Apix/actions/workflows/ci.yml/badge.svg" alt="CI"></a></p>

# Apix
🚀 The high-performance, minimalist API client crafted with Rust and GPUI. Built for speed, designed for humans.

> **Status:** early development, not released yet. The desktop app and the CLI
> already cover daily API work: requests, collections, environments, every common
> auth type, encrypted collections, Postman import and a network inspector.
> See the [roadmap](Docs/ROADMAP.md).

**Documentation:** [English](Docs/en/user-guide.md) · [Türkçe](Docs/tr/user-guide.md)
— user guide, workflows, architecture and technical notes in each language.

![Apix](Docs/en/images/main.png)

## Why Apix

- **Native and GPU-rendered.** No Electron, no web view: the interface is drawn
  with GPUI, the engine of the Zed editor. It starts fast, stays light and
  scrolls a 100 MB JSON response without effort.
- **Your files, your git.** A collection is a folder of small TOML files. Commit,
  diff and review them like code. No account, no cloud, nothing to sync.
- **Secrets stay out of git.** Secret values live in a gitignored
  `.local.toml` or, if you choose, the OS keychain; OAuth 2.0 tokens only in
  memory; and a whole collection can be
  **encrypted** with a password (XChaCha20-Poly1305, Argon2id), file names
  included, so it can be shared in a repository.
- **One engine everywhere.** The desktop app and the `apix` CLI send requests in
  exactly the same way, so `apix run` in CI does what you saw on your machine.
- **See the network.** A Network tab (and `apix send --network`) shows DNS,
  TCP, TLS handshake, waiting and download time, addresses, VPN hints,
  redirects, compression, the TLS version, cipher suite and ALPN, and the whole
  certificate chain, all from the request's own connection.
- **In your language.** English, Türkçe, العربية, Deutsch, Français, Русский,
  日本語, 简体中文 and Español, switched at once in Settings; in Arabic the
  window mirrors to right to left.
- **Your colours.** Dark and light themes (Obsidian, Cyber glass, Dracula,
  Nord, Solarized, Daylight) or follow the system, an accent colour, a
  transparency slider from solid to frosted glass, and your own themes as small
  TOML files. Changes apply at once.

### Compared with similar tools

| | Apix | Postman | Insomnia | Bruno | Hoppscotch |
|---|---|---|---|---|---|
| Desktop app built on | GPUI (native, GPU) | Electron | Electron | Electron | Tauri (system web view) |
| Collections as plain files you commit | ✅ TOML | ❌ cloud workspaces, export to JSON | ✅ with Git Sync (YAML) | ✅ `.bru` files | ❌ app storage, export to JSON |
| Use without an account | ✅ everything | only the lightweight client: no collections or environments | ✅ Scratch Pad (local only) | ✅ everything | ✅ local data in the app |
| Password-encrypted collection in a repository | ✅ | — | — | — | — |
| Command-line runner | ✅ `apix`, the same engine | Postman CLI | Inso CLI | Bruno CLI | Hoppscotch CLI |

Checked against each project's own documentation in October 2026. "—" means
the tool has no such built-in feature that we know of; corrections are welcome.
HTTPie is best known for its `http` command; its desktop app (Electron) is in beta.

## Performance

| Measure | Apix 0.1.0 |
|---|---|
| Release binary | 38.9 MB |
| Start to first frame | 278 ms |
| Memory when idle (working set) | 84 MB |
| 100 MB JSON response on screen | 306 ms |

Windows 11 (build 26300), AMD Ryzen 9 7900X, release build, median of 5 runs, 2026-10-24.

Reproduce it with `scripts/measure.ps1` (see the [technical notes](Docs/en/technical.md#measuring-performance)).
Measurements of the other tools on the same machine are planned.

## What works today

- **Requests:** GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS with params and
  headers; JSON, text, XML, HTML, URL-encoded form, multipart with files,
  binary file and GraphQL bodies.
- **Auth:** Bearer, Basic, API key, Digest, OAuth 1.0, OAuth 2.0 (client
  credentials, password, authorization code with PKCE in the browser) and AWS
  Signature V4. Folders, collections and projects can set auth once; requests
  inherit it.
- **Request settings:** timeout, redirects, TLS verification, proxy, client
  certificates and cookies on/off per request, inherited from folders,
  collections and projects like auth.
- **Cookies:** one persistent jar filled from `Set-Cookie` answers, with a
  manager to list, delete and clear.
- **Responses:** status, timing, size, highlighted and foldable JSON,
  indented and coloured XML and HTML, image preview, a Hex view for binary
  bodies, save the body to a file, raw and headers views, and the Network tab;
  Ctrl+F search in any body (100 MB too) and a JSONPath or jq filter, also on
  the command line (`apix send --filter`).
- **Params and path variables:** the URL bar and the Params table follow each
  other as you type, and `/users/:id` segments take their values from the
  request (Postman's path variables come along on import and export); header
  names and their usual values are suggested while typing, and the sidebar has
  a search.
- **Examples:** keep a response with its request (status, headers, body and
  the request as written), show it again, and bring Postman's saved responses
  along on import and export (`apix send --save-example`).
- **Collections and projects:** a sidebar built from the files on disk, tabs that
  keep their edits and responses and come back next time, drag to move and to
  reorder, projects that share environments, a colour per collection, and a
  confirmation before DELETE, PUT or PATCH in production.
- **Variables:** project, collection and environment layers, secrets kept out of
  git, dynamic values such as `{{$guid}}`, coloured chips in the URL bar, params,
  headers and form fields.
- **Scripts and tests:** pre-request and test scripts in JavaScript (QuickJS,
  built in) with a Postman-compatible `pm.*` API, so imported Postman scripts
  run: `pm.sendRequest`, `require('crypto-js' | 'lodash' | 'moment' | …)`,
  `pm.test` / `pm.expect`, `console`. Checks without code (`status == 200`,
  `body.users[0].id exists`), and saving a response value as a variable.
- **Runner** (Ctrl+Shift+R): run a collection or folder in the order you pick,
  several times or once per row of a CSV/JSON data file, with a delay between
  requests; pause, resume, stop at the first failure; save JUnit XML, JSON or
  HTML reports. `apix run` does the same in CI and fails the build when a test
  fails: as a GitHub Action (`uses: 11R5/Apix@…`,
  [example](examples/ci/github-actions.yml)), a Docker image, or plain
  `apix`, with secrets from `APIX_VAR_<NAME>` variables or a `.env` file.
- **Git-aware:** `apix run --changed` sends only the requests a change touches
  (their files, scripts, folder or environment settings), `apix diff` shows
  what changed in requests field by field, and the GitHub Action keeps a
  summary comment on the pull request. The status bar shows the branch and the
  changed files; the sidebar marks changed requests. Click the branch for the
  Git window: what changed in each request, commit, branches, pull and push.
- **Mock server:** serve the responses saved with requests on a local port
  (`apix mock`, or View › Mock server… in the app), matched by method and path,
  so a front end can be built before the API exists.
- **Table and chart:** a JSON response (or what a JSONPath/jq filter picks out)
  as a table of its records or a bar chart, in the app and `apix send --table`.
- **Capture proxy:** traffic through a local proxy becomes requests with their
  answers (`apix capture`, View › Capture proxy…); HTTPS too, with a local
  certificate authority you choose to trust.
- **Route and map:** the routers between you and the server (the system's
  traceroute, no admin rights) on a world map, placed with an offline IP
  location database (`apix trace`, View › Route and map…).
- **API documentation:** one HTML page or Markdown from a collection
  (`apix docs`, File › Export documentation…), with no secrets in it.
- **AI agents (MCP):** `apix mcp` lets Claude or any MCP client list, send and
  run a collection's requests with their tests, without seeing its secrets.
- **Compare responses:** today's answer against yesterday's from the
  history, or staging against production, field by field (`apix compare`,
  ⇄ next to the response).
- **Load testing:** send a request from many workers at once and see
  requests a second, p50/p95/p99 and a live chart (`apix bench`, Request ›
  Load test…); thresholds fail a CI job.
- **Monitors:** run a collection or folder every few minutes and hear about it
  on Slack, Discord or any webhook when it starts failing or passes again;
  with `apix monitor` on a server or a CI schedule, or from the app while it
  is open. Kept in `apix.toml`, so they go through git with the requests.
  `apix run --parallel` runs iterations side by side, like several users.
- **Encrypted collections** with a recovery key, auto-lock and, if you choose,
  the password remembered in the OS keychain.
- **Import** from Postman (collections, environments, a whole "Export data"
  zip), Insomnia v4, OpenAPI 3 / Swagger 2 (JSON or YAML), HAR and WSDL, from a
  file or a URL, with a
  report; formats are recognised by content. **Export to Postman** for
  teammates who stay on it.
- **Copy as** `curl` (bash or fish), `curl.exe`, PowerShell, HTTPie or wget, or as code in fetch, axios,
  Python, Go, Rust, C#, Java, PHP, Swift and Kotlin; **paste** a `curl` command
  to fill a request.
- **WebSocket and server-sent events:** `ws://` requests connect, send typed
  or saved messages and log both directions live; `text/event-stream`
  responses show their events as they arrive. `apix send` does both in a
  terminal.
- **SOAP:** import a WSDL from a file or a URL and get a request per operation,
  with the right headers and an example envelope built from the schema; faults
  are shown plainly and fail a run even with status 200.
- **MQTT:** `mqtt://` and `mqtts://` requests subscribe to topics and publish
  at QoS 0, 1 and 2, retained or not, with a live log by topic, in the app or
  with `apix send`.
- **Socket.IO:** join a namespace with auth, emit named events (with acks
  and binary attachments) and see every event, ack and attachment the
  server sends, in the app's Messages tab or with `apix send`.
- **GraphQL schemas:** fetch a schema with the request's own auth, then get
  completion for fields, arguments, enum values and fragments, docs on hover,
  and the whole schema as SDL beside the query; `apix schema` prints it.
- **gRPC:** `grpc://` requests call unary and streaming methods, described by
  server reflection or `.proto` files (no `protoc` needed), with messages as
  JSON, an example message for any method, and a live log of what goes both
  ways; `apix send`, `apix schema`, runs and test scripts work on them too.
  No `.proto` at hand? A definition can be made from a descriptor set, the code
  generated from it, types in a programming language or a JSON example; what is
  only a guess says so, and is checked against the server's reflection.
- **History** of sent requests with their responses (Ctrl+H): search it and
  send any of them again.
- **Command palette** (Ctrl+K) and a menu bar that holds every command.

## Build

Requirements: [Rust](https://rustup.rs) (stable). On Windows you also need the
MSVC "Desktop development with C++" workload.

```sh
cargo test                     # engine, CLI and tools
cargo run -p apix-desktop      # desktop app
cargo build --release          # optimized CLI in target/release
```

More in the [technical notes](Docs/en/technical.md).

## CLI

```sh
apix send examples/jsonplaceholder/users/list-users.apix
apix send examples/jsonplaceholder/health.apix --env local --network
apix run examples/jsonplaceholder            # non-zero exit if any request fails
apix run my-api --data users.csv --junit report.xml --html report.html --bail
APIX_VAR_TOKEN=… apix run my-api --env staging --env-file .env.ci
apix run my-api --changed=origin/main --markdown summary.md   # only what this branch touched
apix diff main -C my-api                     # what changed in the requests, field by field
apix monitor my-api                          # [[monitor]]s of apix.toml on their schedules
apix mock my-api --port 4010                 # serve saved examples as a mock API
apix docs my-api --out api.html              # API documentation (or --out API.md)
apix capture my-api/captured --only api.test # record traffic through a proxy on :8888
apix update                                  # is a newer Apix out? (nothing is downloaded)
apix trace https://api.example.com          # the route to the server, hop by hop
apix run my-api --data users.csv -p 8        # 8 iterations at once
apix bench my-api/users/list.apix -c 50 -d 30s --max-p95 300   # a load test
apix compare my-api/users/list.apix --env staging --against prod   # do they answer alike?
apix mcp my-api                              # serve the collection to an AI agent (MCP)
apix send chat/socket.apix --message '{"join":"room-1"}' --listen 10   # a ws:// request
apix send events/live.apix                   # server-sent events, printed as they arrive
apix schema graphql/users.apix               # a GraphQL endpoint's schema, as SDL
apix send grpc/say-hello.apix                # a grpc:// request; streams print as they arrive
apix shell examples/jsonplaceholder/users/create-user.apix --as powershell
apix import "My API.postman_collection.json"
APIX_PASSWORD=… apix run my-api-encrypted --env staging
```

## A request file

```toml
# users/create-user.apix
name = "Create user"
method = "POST"
url = "{{base_url}}/users"

[body]
json = '{ "name": "Ada" }'

[auth.bearer]
token = "{{token}}"
```

The [user guide](Docs/en/user-guide.md) and the
[architecture](Docs/en/architecture.md#on-disk-format) describe the format in
full: every body and auth type, folders, environments, secrets and projects.

## License

[Apache-2.0](LICENSE)

The desktop app embeds [Inter](https://github.com/rsms/inter) and
[JetBrains Mono](https://github.com/JetBrains/JetBrainsMono), both under the
SIL Open Font License 1.1 ([licenses](crates/apix-desktop/assets/fonts)).
`vendor/gpui-pre-windows` is part of [GPUI](https://github.com/zed-industries/zed)
(Apache-2.0) with a small fix.
