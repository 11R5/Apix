# Apix roadmap

Each milestone produces something you can use on its own. Postman parity
comes first, then the features Postman does not have.

The detailed, feature-by-feature plan (in Turkish) is [IS-PLANI.md](IS-PLANI.md).
The visual direction is [design/mockup-v1.webp](design/mockup-v1.webp).

## M0 — Foundation ✅
- [x] Cargo workspace, release profile tuned for size and speed
- [x] `apix-core`: request model, layered `{{variables}}`, TOML collection format
- [x] `apix-http`: reqwest + rustls, query/header/body/auth, TTFB and total timing
- [x] `apix` CLI: `apix send <file>`, `apix run <dir>` with `--env` / `--var`
- [x] `apix-desktop`: blurred, transparent window with its own title bar and window controls

## M1 — Send a request from the UI ✅
- [x] Design tokens (Obsidian and Cyber themes; `APIX_THEME=cyber`; more themes after M5)
- [x] Embedded Inter 4.1 and JetBrains Mono 2.304 fonts (OFL 1.1)
- [x] tokio runtime bridged into GPUI, with cancellable requests (Esc)
- [x] Editable URL bar, method picker, Send button, Enter / Ctrl+Enter
- [x] Response panel: status, time, size, Pretty / Raw / Headers
- [x] Highlighted, foldable JSON viewer
- [x] Row-based virtual viewer for large bodies: 100 MB of JSON (9.6M lines) shows in ~0.33 s in release builds, scrolls by wheel, scrollbar and keyboard
- [x] Params / Headers tables with enable toggles; JSON / Text / XML body editor

## M2 — Collections and Postman import ✅
- [x] Open folder (Ctrl+O, or drop it on the window; offers to start a collection), sidebar tree built from files on disk
- [x] Recent collections (empty state)
- [x] Edit and save (Ctrl+S) with an unsaved marker and a save/discard prompt; new (Ctrl+N), rename, duplicate, delete to trash from a context menu
- [x] Drag a request or folder onto a folder or collection to move it (done in M3); manual ordering is still by name
- [x] File watching (outside edits show up within ~0.5 s)
- [x] Tabs (done in M3)
- [x] **Postman Collection v2.1/v2.0 and environment import**, with an import report (sidebar button, drag and drop, CLI)
- [x] `apix import <file> [--out dir] [--into collection]`

## M3 — Icon, projects, environments, copy-as, auth ✅ (smaller leftovers below stay open)
- [x] App icon from `Logo/LogoFirst.jpg` (`cargo run -p icongen --release`): transparent squircle, tighter crop at 32 px and below; `.ico` embedded in the exe, `.icns`, Linux hicolor PNGs + `.desktop`
- [x] Environment switcher in the sidebar, remembered per collection, with `*.local.toml` secrets; red line for prod
- [x] Variables dialog (Ctrl+E): edit shared variables and environments, add, rename and delete environments, secrets kept in `*.local.toml` (added to `.gitignore` automatically)
- [x] Dynamic variables: `{{$guid}}`, `{{$timestamp}}`, `{{$isoTimestamp}}`, `{{$randomEmail}}` and 24 more, a fresh value for every use
- [x] Secrets in the OS keychain (done in M4): Settings › Environment secrets; `.local.toml` then keeps only the id of a keychain entry, read by the app and the CLI alike
- [x] Projects (`apix-project.toml`): New project / New collection / Open from the sidebar menu, recent items, project-wide environments layered under collection ones, grouping as Project → Environment → Collections, pin collections to environments
- [x] Projects (done in M4): a colour per collection (sidebar dot and tab dot), move a collection into a project (File menu, or drop its folder on an open project), confirm DELETE/PUT/PATCH in a prod environment (can be turned off in Settings)
- [x] Copy as bash `curl`, cmd `curl.exe` or PowerShell `Invoke-RestMethod` (URL bar menu, sidebar, `apix shell`); pasting a `curl` command fills the request
- [x] Copy as fish `curl`, HTTPie or wget (done in M4; also `apix shell --as fish|httpie|wget`)
- [x] New request with a chosen method (sidebar "+" menu, "New request ▸" in folder and collection menus); POST, PUT and PATCH start with a JSON body. Alt+1…7 switch the method
- [x] Variable chips in the URL bar (defined, dynamic or missing, by colour); hovering one shows its value, with secrets masked, and the scope it comes from
- [x] The same chips in Params, Headers and form/multipart body fields (done in M4); not yet in the raw body editor
- [x] Auth tab (inherit, none, Bearer, Basic, API key); folders (`folder.toml`), collections and projects can set auth that requests inherit, in the app and the CLI; Postman imports keep auth where it was set
- [ ] OAuth 2.0 flows (moved to M4)
- [x] Command palette (Ctrl+K or Ctrl+P): fuzzy search over every request and command (new request by method, environments, copy as, auth, grouping, recent folders)
- [x] Menu bar in the title bar (File, Request, Environment, View, Help) with shortcuts; it replaces the sidebar's "Open or create" menu and works on Windows' drag area
- [x] From M2: tabs (each keeps its draft, response and in-flight request; Ctrl+W, Ctrl+Tab, middle-click), drag to move requests and folders, also across collections
- [x] Manual ordering in the sidebar (done in M4): drag a request, folder or project collection onto the top or bottom edge of a row; the order is saved as `order` in `apix.toml`, `folder.toml` and `apix-project.toml`, kept through renames and moves, kept from Postman imports and followed by Postman exports
- [x] Reopen the tabs that were open, per project or collection (done in M4)

## M4 — Daily driver
- [x] **Encrypted collections**: password-protected (XChaCha20-Poly1305, Argon2id), per-file encryption with opaque file names, unlock prompt and auto-lock, recovery key, `--password-env` for CI
  - [x] Format and engine (`apix-vault`): per-file XChaCha20-Poly1305 with opaque names and an encrypted index, Argon2id password key, recovery key, password change without re-encrypting, tamper detection, local secrets kept out of git
  - [x] Open and edit encrypted collections in the app (unlock prompt, recovery key, lock, auto-lock after 15 idle minutes, encrypt or decrypt in place, change password, new encrypted collection) and the CLI (`--password-env`, `apix encrypt`, `apix decrypt`); nothing decrypted is written to disk
  - [x] Remember the password in the OS keychain (Windows Credential Manager, macOS Keychain, Linux Secret Service): opt in when unlocking, forget it from the File or collection menu; a stale password is dropped and asked for again; the CLI uses it too
- [ ] **Network inspector**: local interface/IP/DNS/proxy, DNS answers, TCP/TLS details and certificate chain, timing waterfall, redirects
  - [x] Network tab and `apix send --network`: timing waterfall (DNS, connect, waiting, download), DNS answers, server and local address, interface and VPN hint, gateway, DNS servers, proxy from the environment, redirect chain, server certificate (issuer, validity, days left, names); every send uses a fresh traced connection
  - [ ] TLS version and cipher, the full certificate chain, separate TCP and TLS times, compressed size (moved to M7)
- [x] **Bodies**: editable URL-encoded form, multipart (text and file fields, each with its own Content-Type), binary file and GraphQL (highlighted query, JSON variables) in the app, the CLI, "Copy as", curl paste (`-F`, `--form-string`, `--data-binary @file`) and Postman import; relative file paths resolve against the request's folder
- [x] **Auth types**: Digest (MD5, SHA-256), OAuth 1.0 (HMAC-SHA1/SHA256, PLAINTEXT, header or query), OAuth 2.0 (client credentials, password, authorization code with PKCE in the browser; token refresh; tokens kept in memory only) and AWS Signature V4, in the app, the CLI, "Copy as", curl paste and Postman import
- [ ] **Settings and UI language** (moved from M7): settings screen; English, Turkish, Arabic, German, French, Russian, Japanese, Chinese and Spanish; right-to-left layout for Arabic; switch without restarting; a new language is one file plus one line
  - [x] Settings dialog (Ctrl+,): language, theme, auto-lock time for encrypted collections; saved to `settings.toml` at once
  - [x] Every UI text behind a key (about 414), nine languages, switch without restarting, first start follows the system language, a test for missing keys and placeholders
  - [x] Right-to-left layout for Arabic: sidebar, panes, window controls, menus, tabs, form rows, dialogs and the status bar mirror; code, URLs and key–value tables stay left to right; switches at once
  - [x] Wrapped right-to-left text (notes, hints, descriptions) breaks into lines in reading order
  - [ ] Translated engine error messages, native-speaker review
- [x] **Documentation and README**: per-language docs in `Docs/<lang>/` (user guide with screenshots, architecture, technical, workflows) in English and Turkish, `Docs/languages.toml`, `cargo run -p docs-check` for stale translations; README on how Apix differs from Postman, Insomnia, Bruno and Hoppscotch; `scripts/measure.ps1` for reproducible performance numbers
- [x] **Request settings and cookies**: a `[settings]` table (timeout, redirects, TLS verify, proxy, PEM client certificate, trusted CA file, cookies on/off) inherited per field through folder → collection → project like auth; the Settings tab plus shared-settings dialogs; a persistent cookie jar with a manager dialog (Request › Cookies…); CLI flags, Copy-as flags, curl paste and Postman `protocolProfileBehavior` import
- [x] **Code and history**: Copy as code in 10 languages (fetch, axios, Python requests, Go, Rust reqwest, C#, Java, PHP, Swift, Kotlin; settings a language cannot express are noted in the snippet), `apix code --as <lang>`; a searchable history of sent requests and their responses (Ctrl+H), kept per machine with variables unresolved, never for encrypted collections
- [x] **Postman, complete**: Postman exports in an opened or dropped folder are recognised by content; a folder or "Export data" zip becomes one project (globals as project variables); export a collection to Postman v2.1 with its environments (`apix import <folder|zip>`, `apix export`)
- [x] **Content types**: HTML request bodies; XML and HTML colouring and indentation (bodies and responses); response kind detection; image preview; a Hex tab; save the response body to a file
- [x] **More importers**: Insomnia v4, OpenAPI 3 / Swagger 2 (JSON or YAML) and HAR, recognised by content in a file, a folder or a zip (`apix import` reads them all)
- [ ] gRPC (unary and streaming, `.proto` or server reflection) — asked for on 2026-10-23, after the importers

## M5 — Scripting, testing, runner
- [x] `apix-script` on QuickJS with a `pm.*`-compatible API: variable scopes, `pm.request` (editable in pre-request scripts), `pm.response`, `pm.test`, Chai-style `pm.expect`, `pm.response.to.have.*`, the older `tests[...]` / `postman.*` API, `console`
- [x] Pre-request and test scripts next to requests, folders and the collection (`<name>.prerequest.js`, `_folder.test.js`, `_collection.prerequest.js`, …), run collection → folders → request; what scripts set lasts for the app session or the run, never written to disk
- [x] Scripts tab in the app; Tests and Console tabs on the response
- [x] `apix send` / `apix run` run scripts, print test results and console lines, follow `setNextRequest`, and fail when a test fails
- [x] `pm.sendRequest` (callback and `await`), built-in libraries (`require` of crypto-js, uuid, lodash, moment, ajv, tv4, chai: the parts API scripts use, written for Apix), checks without code (`[[assert]]`, a Checks table in the Scripts tab), save a response value as a variable
- [x] Runner: the Runner window (Ctrl+Shift+R, or Run… on a collection or folder: pick and order requests, iterations, delay, environment, data file, stop at the first failure, pause/resume/stop, results with tests and console) and `apix run` options (`--data`, `--iterations`, `--delay`, `--bail`, folder runs); data-driven runs from CSV or JSON; JUnit XML / JSON / HTML reports; a GitHub Actions example (`examples/ci/github-actions.yml`); shared crate `apix-runner`
- [x] CI: secrets from `APIX_VAR_<NAME>` environment variables and `--env-file` (names matched whatever their case), the repository as a GitHub Action (`uses: 11R5/Apix@…`, builds and caches `apix`), a Docker image (`Dockerfile`, published to ghcr.io on version tags), an environment-matrix workflow example
- [ ] Later: `--parallel`; PR comments, `--changed` (only what a PR touched), `apix diff`, scheduled monitor runs

## After M5 — Themes
- [x] Switch themes without restarting (views read the current palette)
- [x] Choose the accent colour (the theme's own or 8 colours, with deeper shades on light themes)
- [x] Light theme and "follow the system" (Daylight, Solarized Light; code colours follow)
- [x] Custom themes from files (`<config>/Apix/themes/*.toml`, "Open the folder" writes an example)
- [x] Transparency slider (0–90 %, frosted glass: acrylic on Windows, vibrancy on macOS), preset themes (Dracula, Nord, Solarized Dark)
- [x] Discussed after M5: the CLI's CI side (secrets, GitHub Action, Docker) is done; git-aware runs, PR comments, `apix diff` and monitors wait

## CI foundation (before M6)
- [x] CI for the repository on every push and pull request: format and `docs-check`; clippy (`-D warnings`) and tests for the engine, CLI and desktop app on Windows, macOS and Linux
- [x] The GitHub Action tests itself (`uses: ./` on the example collection, Linux and Windows); its `apix` cache follows a hash of the sources
- [x] GitLab CI and Azure Pipelines examples
- [x] Linux and macOS: the first run built and tested everything on all three systems with no fixes needed

## M6 — Protocols
- [x] WebSocket: `ws://`/`wss://` requests connect with the request's params, headers and auth; a Messages tab with saved `[[message]]`s; a live log of both directions; `apix send --message/--listen`; binary messages (base64/hex), subprotocols, and the TLS settings (trusted CA, client certificate, no verification) for `wss://`
- [x] Server-sent events: `text/event-stream` responses show their events as they arrive, in the app and in `apix send`; Esc keeps what came; narrowing to one event type; reconnecting after the server's retry time with `Last-Event-ID`
- [x] GraphQL: the schema fetched with the request's own URL, headers and auth (introspection) and kept per endpoint; completion of fields, arguments, enum values, input fields, fragments and variable types; docs on hover; completion in the variables (declared variables, input fields, enum values); the schema as SDL beside the query; `apix schema`
- [x] gRPC: `grpc://`/`grpcs://` requests; services from server reflection or `.proto` files (protox, no `protoc`); unary, server, client and bidirectional streaming with messages as JSON; metadata from headers and auth; an example message per method; a live log with status, metadata and trailers; `apix send` and `apix schema`; runs and test scripts see a call as an HTTP response (OK is 200, `grpc-status`); calls kept in the history; TLS with a trusted CA or without certificate checks
- [x] Socket.IO (servers of Socket.IO 3 and 4, over WebSocket): a `[socketio]` table (namespace, path, auth) on an `http(s)://` or `ws(s)://` request; joining the namespace with JSON auth; events with names, acks both ways shown in the log, binary attachments; a WebSocket/Socket.IO switch in the Messages tab; `apix send --message … --event …`
- [x] MQTT 3.1.1: `mqtt://`/`mqtts://` requests with a `[mqtt]` table (client id, session, keep alive, subscriptions); subscribing and leaving while connected; publishing at QoS 0, 1 and 2 with retain, text or bytes; a user name and password from Basic auth or the URL; the TLS settings for `mqtts://`; a live log by topic; `apix send --subscribe/--topic/--qos/--retain`
- [x] A password in a URL stays out of everything Apix prints or writes: command-line output, the connection log, error messages for every protocol, redirects, and JUnit/JSON/HTML reports
- [x] SOAP: WSDL 1.1 imports as ordinary POST requests, one per operation, for SOAP 1.1 and 1.2 ports, with the right headers and an example envelope built from the XML schema (nested types, extensions, enums, choices, namespaces, imported documents); faults shown in the app and `apix send`, and failing a run even with status 200; SOAP envelope templates in the XML body; importing any format from a URL (File menu, `apix import <address>`)
- [ ] gRPC without a `.proto` file: descriptor sets, generated code, language types or JSON turned into a protobuf definition, with a preview to edit field numbers and method names, and a comparison with server reflection when it is there (JSON and hand-written types have no field numbers, so an inferred definition is shown as a guess)

## M7 — Advanced
- [ ] Route (traceroute) and network map, with an offline IP-geolocation database
- [ ] TLS details in the network inspector with our own connector: TLS version, cipher, ALPN, full chain, separate TCP and TLS times, compressed size
- [ ] Local mock server, docs generation, monitors, visualizer, capture proxy
- [ ] Git integration, auto-update (the light theme was done in the theme work after M5)
- [ ] Everyday gaps: search in the response (Ctrl+F), JSONPath/jq filter, params ↔ URL sync, `/users/:id` path variables, header autocomplete, sidebar search, save examples
- [ ] Git-aware CLI: `apix run --changed`, `apix diff`, a run summary as a PR comment
- [ ] Monitors (scheduled runs), `apix run --parallel`
- [ ] Theme leftovers: code editor colours from the theme, font size and density

## M8 — Beyond Postman
- [ ] Load testing (`apix bench`), response diff, time-travel history
- [ ] MCP server mode (`apix mcp`), optional AI assist, WASM plugins, Flows

## M9 — Release & distribution
- [x] GitHub Actions: test on Windows, macOS and Linux (CI foundation); build on version tags
- [ ] Prebuilt binaries on GitHub Releases, which the GitHub Action downloads instead of building
- [ ] Build, try and publish the Docker image (ghcr.io)
- [ ] A small `apix-action` repository for the GitHub Marketplace (it does not list repositories that have workflow files)
- [ ] Meet the start-up (150 ms) and idle memory (60 MB) budgets
- [ ] Installers: `.msi`/winget, `.dmg`/Homebrew cask, AppImage/Flatpak/`.deb`, using the M3 icons
- [ ] Keep the release binary under 100 MB (38.9 MB today); speed and smoothness come first
- [ ] Signed Windows builds (so Smart App Control allows them) and notarized macOS builds
- [ ] Documentation in all nine languages, an illustrated user guide with screenshots taken from each language's UI, a docs site with a language picker
- [ ] Side-by-side performance numbers against Postman, Insomnia, Bruno and Hoppscotch, measured on one machine with one script
- [ ] Launch post (r/rust, Hacker News)

## Later (decide after M9)
- [ ] Team mode over Tailscale: share projects across a tailnet without a cloud; to be weighed against git-based collaboration
