# Apix roadmap

Each milestone produces something you can use on its own. Postman parity
comes first, then the features Postman does not have.

## M0 — Foundation ✅ (this commit)
- [x] Cargo workspace, release profile tuned for size and speed
- [x] `apix-core`: request model, layered `{{variables}}`, TOML collection format
- [x] `apix-http`: reqwest + rustls, query/header/body/auth, TTFB and total timing
- [x] `apix` CLI: `apix send <file>`, `apix run <dir>` with `--env` / `--var`
- [x] `apix-desktop`: blurred, transparent GPUI window shell

## M1 — A request you can actually send from the UI
- [ ] tokio runtime bridged into GPUI (`cx.spawn`)
- [ ] Editable URL bar, method picker, Ctrl+Enter to send
- [ ] Params / Headers / Body / Auth tabs (key–value tables with enable toggles)
- [ ] Response panel: status, time, size, headers, pretty JSON with syntax highlighting
- [ ] Virtualized body viewer (100 MB without dropped frames)
- [ ] Sidebar loads a collection folder from disk and saves edits back

## M2 — Daily-driver parity
- [ ] Environments switcher with secret (`*.local.toml` + OS keychain) values
- [ ] Command palette (Ctrl/⌘+K): switch env, import, run, open, settings
- [ ] Tabs, history, duplicate, rename, drag to reorder
- [ ] Cookies jar, redirects/TLS/proxy/timeout settings, client certificates
- [ ] Body types: multipart/form-data with files, binary, GraphQL
- [ ] Auth: Digest, OAuth 2.0 (auth code + PKCE, client credentials, refresh), AWS SigV4
- [ ] Import: cURL paste detection, Postman v2.1, Insomnia, OpenAPI 3, HAR
- [ ] Code snippets: curl, fetch, axios, Python requests, Go, Rust, C#, Java, PHP, Swift

## M3 — Scripting & testing
- [ ] `apix-script` on QuickJS, with a `pm.*`-compatible API so Postman scripts keep working
- [ ] Pre-request and test scripts, `expect`-style assertions, JSON Schema validation
- [ ] Declarative no-code assertions (`status == 200`, `body.users.length > 0`)
- [ ] Collection runner UI, data-driven runs (CSV/JSON), JUnit report for CI
- [ ] Request chaining: extract values from a response into variables

## M4 — Protocols
- [ ] WebSocket, Server-Sent Events
- [ ] GraphQL with schema introspection and autocomplete
- [ ] gRPC (unary + streaming) with server reflection or `.proto` files
- [ ] SOAP helpers, MQTT

## M5 — Beyond Postman
Features where a native Rust app has an edge:
- [ ] **Built-in load testing.** `apix bench` and a UI chart, with thousands of
      concurrent requests from tokio and latency percentiles. Postman has no real equivalent.
- [ ] **Response diff.** Compare two runs or two environments side by side.
- [ ] **Time-travel history.** Every sent request is stored with its response.
      It can be searched and replayed.
- [ ] **Local mock server** generated from saved examples or an OpenAPI file.
- [ ] **Capture proxy.** Record traffic from a browser or phone into a collection.
- [ ] **MCP server mode** (`apix mcp`). AI agents can list and run your collections.
- [ ] **AI assist.** Generate tests and explain errors with your own API key (optional, off by default).
- [ ] **Plugins** via WASM (custom auth, importers, visualizers).
- [ ] **Flows.** A visual node graph for request chaining.
- [ ] **Monitors**, run by a local scheduler or by the CLI in CI. No cloud lock-in.

## Release & distribution
- [ ] GitHub Actions: test on Windows/macOS/Linux, build release artifacts
- [ ] Installers: `.msi`/winget, `.dmg`/Homebrew cask, AppImage/Flatpak/`.deb`
- [ ] Signed and notarized macOS builds, signed Windows builds
- [ ] Docs site, logo, and a launch post (r/rust, Hacker News)
