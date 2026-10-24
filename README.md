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
  connection, waiting and download time, addresses, VPN hints, redirects and the
  server certificate.
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
  bodies, save the body to a file, raw and headers views, and the Network tab.
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
- **Encrypted collections** with a recovery key, auto-lock and, if you choose,
  the password remembered in the OS keychain.
- **Import** from Postman (collections, environments, a whole "Export data"
  zip), Insomnia v4, OpenAPI 3 / Swagger 2 (JSON or YAML) and HAR, with a
  report; formats are recognised by content. **Export to Postman** for
  teammates who stay on it.
- **Copy as** `curl` (bash or fish), `curl.exe`, PowerShell, HTTPie or wget, or as code in fetch, axios,
  Python, Go, Rust, C#, Java, PHP, Swift and Kotlin; **paste** a `curl` command
  to fill a request.
- **WebSocket and server-sent events:** `ws://` requests connect, send typed
  or saved messages and log both directions live; `text/event-stream`
  responses show their events as they arrive. `apix send` does both in a
  terminal.
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
apix send chat/socket.apix --message '{"join":"room-1"}' --listen 10   # a ws:// request
apix send events/live.apix                   # server-sent events, printed as they arrive
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
