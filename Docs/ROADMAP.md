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
- [x] Design tokens (Obsidian and Cyber themes; `APIX_THEME=cyber`)
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
- [ ] Secrets in the OS keychain
- [x] Projects (`apix-project.toml`): New project / New collection / Open from the sidebar menu, recent items, project-wide environments layered under collection ones, grouping as Project → Environment → Collections, pin collections to environments
- [ ] Projects: colours, drag a collection into a project, prod confirmation dialog
- [x] Copy as bash `curl`, cmd `curl.exe` or PowerShell `Invoke-RestMethod` (URL bar menu, sidebar, `apix shell`); pasting a `curl` command fills the request
- [ ] Copy as fish, HTTPie, wget
- [x] New request with a chosen method (sidebar "+" menu, "New request ▸" in folder and collection menus); POST, PUT and PATCH start with a JSON body. Alt+1…7 switch the method
- [x] Variable chips in the URL bar (defined, dynamic or missing, by colour); hovering one shows its value, with secrets masked, and the scope it comes from
- [x] Auth tab (inherit, none, Bearer, Basic, API key); folders (`folder.toml`), collections and projects can set auth that requests inherit, in the app and the CLI; Postman imports keep auth where it was set
- [ ] OAuth 2.0 flows (moved to M4)
- [x] Command palette (Ctrl+K or Ctrl+P): fuzzy search over every request and command (new request by method, environments, copy as, auth, grouping, recent folders)
- [x] Menu bar in the title bar (File, Request, Environment, View, Help) with shortcuts; it replaces the sidebar's "Open or create" menu and works on Windows' drag area
- [x] From M2: tabs (each keeps its draft, response and in-flight request; Ctrl+W, Ctrl+Tab, middle-click), drag to move requests and folders, also across collections
- [ ] Manual ordering in the sidebar (now sorted by name)

## M4 — Daily driver
- [ ] **Encrypted collections**: password-protected (XChaCha20-Poly1305, Argon2id), per-file encryption with opaque file names, unlock prompt and auto-lock, recovery key, `--password-env` for CI
  - [x] Format and engine (`apix-vault`): per-file XChaCha20-Poly1305 with opaque names and an encrypted index, Argon2id password key, recovery key, password change without re-encrypting, tamper detection, local secrets kept out of git
  - [x] Open and edit encrypted collections in the app (unlock prompt, recovery key, lock, auto-lock after 15 idle minutes, encrypt or decrypt in place, change password, new encrypted collection) and the CLI (`--password-env`, `apix encrypt`, `apix decrypt`); nothing decrypted is written to disk
  - [ ] Remember the password in the OS keychain
- [ ] **Network inspector**: local interface/IP/DNS/proxy, DNS answers, TCP/TLS details and certificate chain, timing waterfall, redirects
- [ ] multipart, binary and GraphQL bodies; OAuth 2.0 (moved from M3), Digest, AWS SigV4 and OAuth 1.0 auth
- [ ] Request settings (timeout, TLS, proxy, client certificates), cookie manager
- [ ] Code snippets in other languages, history
- [ ] Postman auto-detection and full data-dump import; Insomnia, OpenAPI 3 and HAR import; export to Postman

## M5 — Scripting, testing, runner
- [ ] `apix-script` on QuickJS with a `pm.*`-compatible API
- [ ] Pre-request and test scripts, built-in libraries, no-code assertions
- [ ] Test results tab and console
- [ ] Runner UI, data-driven runs (CSV/JSON), JUnit / HTML reports

## M6 — Protocols
- [ ] WebSocket, SSE, GraphQL introspection, gRPC, Socket.IO, MQTT, SOAP

## M7 — Advanced
- [ ] Route (traceroute) and network map, with an offline IP-geolocation database
- [ ] Local mock server, docs generation, monitors, visualizer, capture proxy
- [ ] Git integration, light theme, Turkish and English UI, settings, auto-update

## M8 — Beyond Postman
- [ ] Load testing (`apix bench`), response diff, time-travel history
- [ ] MCP server mode (`apix mcp`), optional AI assist, WASM plugins, Flows

## M9 — Release & distribution
- [ ] GitHub Actions: test and build on Windows, macOS and Linux
- [ ] Installers: `.msi`/winget, `.dmg`/Homebrew cask, AppImage/Flatpak/`.deb`, using the M3 icons
- [ ] Keep the release binary under ~40 MB (27.7 MB today); take easy wins, never at the cost of speed or features
- [ ] Signed Windows builds (so Smart App Control allows them) and notarized macOS builds
- [ ] Docs site and a launch post (r/rust, Hacker News)

## Later (decide after M9)
- [ ] Team mode over Tailscale: share projects across a tailnet without a cloud; to be weighed against git-based collaboration
