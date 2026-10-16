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

## M1 — Send a request from the UI
- [ ] Design tokens (Obsidian and Cyber themes) and embedded Inter / JetBrains Mono fonts
- [ ] tokio runtime bridged into GPUI, with cancellable requests
- [ ] Editable URL bar, method picker, SEND button, Ctrl+Enter
- [ ] Response panel: status, time, size, Pretty / Raw / Headers
- [ ] Highlighted, virtualized JSON viewer (100 MB without dropped frames)
- [ ] Params / Headers tables with enable toggles; JSON / Text / XML body editor

## M2 — Collections and Postman import
- [ ] Open folder, recent folders, sidebar tree built from files on disk
- [ ] Edit and save (Ctrl+S), create / rename / duplicate / delete (to trash), drag to reorder
- [ ] Tabs, file watching
- [ ] **Postman Collection v2.1/v2.0 and environment import**, with an import report
- [ ] `apix import postman <file> --out <dir>`

## M3 — Environments, variables, auth
- [ ] Environment switcher and editor, secrets in `*.local.toml` and the OS keychain
- [ ] Dynamic variables (`{{$guid}}`, `{{$timestamp}}`, …), variable chips with hover preview
- [ ] Auth tab with inheritance from folder or collection; OAuth 2.0 flows
- [ ] Command palette (Ctrl/⌘+K)

## M4 — Daily driver
- [ ] multipart, binary and GraphQL bodies; Digest, AWS SigV4 and OAuth 1.0 auth
- [ ] Request settings (timeout, TLS, proxy, client certificates), cookie manager
- [ ] Code snippets, cURL import/export, history
- [ ] Insomnia, OpenAPI 3 and HAR import; export to Postman

## M5 — Scripting, testing, runner
- [ ] `apix-script` on QuickJS with a `pm.*`-compatible API
- [ ] Pre-request and test scripts, built-in libraries, no-code assertions
- [ ] Test results tab and console
- [ ] Runner UI, data-driven runs (CSV/JSON), JUnit / HTML reports

## M6 — Protocols
- [ ] WebSocket, SSE, GraphQL introspection, gRPC, Socket.IO, MQTT, SOAP

## M7 — Advanced
- [ ] Local mock server, docs generation, monitors, visualizer, capture proxy
- [ ] Git integration, light theme, Turkish and English UI, settings, auto-update

## M8 — Beyond Postman
- [ ] Load testing (`apix bench`), response diff, time-travel history
- [ ] MCP server mode (`apix mcp`), optional AI assist, WASM plugins, Flows

## M9 — Release & distribution
- [ ] GitHub Actions: test and build on Windows, macOS and Linux
- [ ] Installers: `.msi`/winget, `.dmg`/Homebrew cask, AppImage/Flatpak/`.deb`
- [ ] Signed Windows builds (so Smart App Control allows them) and notarized macOS builds
- [ ] Docs site, logo, and a launch post (r/rust, Hacker News)
