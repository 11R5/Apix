# Technical notes

How Apix is built, what it depends on, and how to measure it. The
[architecture](architecture.md) explains the design; this page is the practical
side.

## Repository layout

```
crates/
├── apix-core       request model, variables, on-disk format, projects, signing (sign.rs), GraphQL schemas, git (git.rs, diff.rs)
├── apix-http       the HTTP engine (reqwest), network trace, OAuth 2.0 tokens, SSE, WebSocket, Socket.IO, GraphQL schema fetch
├── apix-import     import of Postman, Insomnia, OpenAPI, HAR and WSDL
├── apix-codegen    API documentation (HTML, Markdown), "Copy as" (curl, curl.exe, PowerShell, code in 10 languages) and curl paste
├── apix-vault      encrypted collections
├── apix-script     pre-request and test scripts (QuickJS)
├── apix-runner     collection runs, data files, JUnit/JSON/HTML/Markdown reports
├── apix-grpc       gRPC calls: .proto files, server reflection or definitions from code and JSON, messages as JSON
├── apix-proto      other protocols: MQTT
├── apix-mock       a local mock server answering from saved examples
├── apix-capture    a recording proxy that saves passing requests
├── apix-route      the route to a server and where its hops are
├── apix-plugins    WebAssembly plugins (wasmi): loading, the sandbox, auth, import and view hooks
├── apix-plugin-sdk writing plugins in Rust; its examples/ are the plugins in plugins/
├── apix-cli        the `apix` command
└── apix-desktop    the desktop app (GPUI); interface texts in locales/
tools/
├── icongen         builds the app icons from Logo/
└── docs-check      lists stale documentation translations
vendor/
└── gpui-pre-windows  gpui's Windows backend with a right-to-left text fix
scripts/
├── package-*.sh/.ps1  release packages: Windows installer and zip, macOS .dmg, .deb, tarball, Arch package
└── measure.ps1     performance measurements for the README
examples/
├── jsonplaceholder a small collection the app opens when run from the repo
└── ci/             GitHub Actions (also scheduled monitors), GitLab CI and Azure Pipelines examples
plugins/            example plugins (.wasm), built from apix-plugin-sdk's examples
site/               the download page (GitHub Pages), reading the latest release from GitHub
action.yml          the GitHub Action (`uses: 11R5/Apix@…`)
Dockerfile          the `apix` command line as a container image
.github/workflows/  CI on three systems, and the image published on version tags
Docs/
├── languages.toml  documentation languages
├── en/ tr/         user guide, architecture, technical notes, workflows
└── IS-PLANI.md, ROADMAP.md   the plan (Turkish detail, English summary)
```

## Building

Requirements: [Rust](https://rustup.rs) stable (the repo is tested with 1.99).

- **Windows:** the MSVC toolchain (Visual Studio or Build Tools with "Desktop
  development with C++" and a Windows 10/11 SDK).
- **macOS:** Xcode command line tools.
- **Linux:** the X11/Wayland, Vulkan and fontconfig development packages GPUI
  needs (the gpui-kit README lists them per distribution); not tested yet.

```sh
cargo test                       # engine, CLI, import, codegen, vault, docs check
cargo run -p apix-desktop        # the desktop app (debug)
cargo test -p apix-desktop       # desktop unit tests (translations, paths, …)
cargo build --release            # optimized CLI
cargo build --release -p apix-desktop
```

`cargo build` and `cargo test` without `-p` skip the desktop app, which pulls in
the whole GPU stack; build it explicitly. Dependencies are optimised even in
debug builds so the UI stays smooth while developing.

The example plugins in `plugins/` need the WebAssembly target once
(`rustup target add wasm32-unknown-unknown`), then
`cargo build -p apix-plugin-sdk --examples --release --target wasm32-unknown-unknown`;
copy `target/wasm32-unknown-unknown/release/examples/*.wasm` into `plugins/`
(with `-` for `_` in the names). The `apix-plugins` tests load those files.
`wasmi` is optimised and built without debug assertions in debug builds too
(`[profile.dev.package.wasmi]`): its interpreter loop relies on tail calls
that otherwise overflow the stack.

The release profile uses fat LTO, one codegen unit and stripped symbols. A full
release build of the desktop app takes a few minutes.

## Main dependencies

| Area | Crates |
|---|---|
| UI | `gpui-kit` 0.7 (GPUI and its component library), tree-sitter grammars for JSON and GraphQL |
| HTTP | `reqwest` 0.12 with rustls, `tokio`, `hyper-util`, `tower` layers for timing |
| gRPC | `tonic` 0.14 (client, over the same hyper and rustls), `prost-reflect` (messages known at run time, JSON), `protox` (reads .proto files without `protoc`), `regex` (reading generated code and types), `tonic-reflection` (the reflection client), `rustls` and `tokio-rustls` for gRPC with certificate checks off; `rcgen` makes test certificates (tests only) |
| Network details | `x509-parser` (certificates), `sha2` (fingerprints), `netdev` (interfaces, gateway, DNS servers); TLS details through rustls' own hooks (no extra crate); `flate2` and `brotli-decompressor` decode compressed bodies (already built for reqwest); the route from the system's `tracert`, `traceroute` or `tracepath`, places from `maxminddb` |
| Signing | `sha2`, `sha1`, `hmac`, `md-5`, `base64`, `rand_core` (AWS SigV4, OAuth 1.0, Digest, PKCE) |
| Encryption | `chacha20poly1305` (XChaCha20-Poly1305), `argon2`, `zeroize`, `keyring` (OS credential store) |
| Scripts | `rquickjs` (QuickJS, built from C with the system compiler); `require` libraries are Apix's own JavaScript, hashing through `sha1`, `sha2`, `md-5`, `hmac` |
| Files | `serde`, `toml`, `serde_json`, `notify` (file watching), `trash`; git through the installed `git` program (no library) |
| Responses | `serde_json_path` (JSONPath, RFC 9535) and `jaq-core`, `jaq-std`, `jaq-json` (jq) for the filter; `regex` for searching large bodies |
| Plugins | `wasmi` 2 (a WebAssembly interpreter with fuel and memory limits) |
| Languages | `rust-i18n` (shared with gpui-kit), `sys-locale` |

## Files Apix writes outside collections

| File | Contents |
|---|---|
| `<config>/Apix/settings.toml` | language, appearance, dark and light theme, accent colour, transparency, auto-lock time, history on or off, where environment secrets go, whether monitors run while the app is open |
| `<config>/Apix/capture-ca.pem`, `capture-ca.key` | the capture proxy's local certificate authority, made the first time HTTPS is opened; trusted only if you add it yourself |
| `<config>/Apix/geo/*.mmdb` | the IP location database for the route map (downloaded from the Route and map window, or put there) |
| `<config>/Claude/claude_desktop_config.json` | written only by Connect to Claude: the `apix-<collection>` entry under `mcpServers` (the original kept as `.json.bak`) |
| `<config>/Apix/monitors/` | each monitor's last 500 results, one JSON line per run (`<hash of the collection folder>-<monitor name>.jsonl`), written by `apix monitor` and the app |
| `<config>/Apix/state.toml` | recently opened collections, the chosen environment and the open tabs per collection or project (not for encrypted collections) |
| `<config>/Apix/cookies.json` | the cookie jar (session cookies are not persisted) |
| `<config>/Apix/graphql/*.json` | the GraphQL schema last fetched from each endpoint (the server's introspection answer), named by a hash of the URL |
| `<config>/Apix/themes/*.toml` | your own themes (written only when you press **Open the folder** in Settings and the folder is empty: one example) |
| OS credential store, service `Apix` | only when asked for: remembered passwords of encrypted collections (`vault:<folder>`) and environment secrets (`env:<id>`, plus `env:<id>#1`, … for long values), AI assist keys (`ai:anthropic-api-key`, `ai:openai-api-key`, `ai:google-api-key`) |
| `<config>/Apix/plugins/*.wasm` | installed plugins (the Plugins window, `apix plugins install`) |
| `<config>/Apix/history/` | sent requests (`{{variables}}` unresolved) and their responses: `index.jsonl`, one `<id>.json` and `<id>.body` per entry; the last 300, bodies cut at 256 KB; a gRPC call keeps its log as the body |

`<config>` is `%APPDATA%` on Windows, `~/Library/Application Support` on macOS
and `~/.config` on Linux. OAuth 2.0 tokens and passwords of encrypted
collections are never written to disk.

## Environment variables

| Variable | Effect |
|---|---|
| `APIX_PASSWORD` | Password of an encrypted collection for the CLI (`--password-env` picks another variable). |
| `APIX_VAR_<NAME>` | A variable for the CLI: `APIX_VAR_TOKEN` fills `{{token}}` (matched to the collection's names whatever their case); wins over environments, `--var` wins over it. |
| `APIX_THEME` | a theme id (`obsidian`, `cyber`, `dracula`, `nord`, `solarized-dark`, `daylight`, `solarized-light` or a theme file's name); shows that theme whatever the settings say. |
| `APIX_BENCH` | `startup` or `json:<url>`: measure and quit (see below). |
| `APIX_BENCH_OUT` | File to append the measurements to. |
| `APIX_BENCH_PHASES` | With `APIX_BENCH=startup`: also report when each step of starting up was reached (`phase_…` lines), to see where the time goes. |
| `APIX_UPDATE_URL` | Where the update check asks instead of GitHub's latest release (a fork, a mirror, a test server). |
| `OPENAI_API_KEY`, `GEMINI_API_KEY` (or `GOOGLE_API_KEY`), `ANTHROPIC_API_KEY` | The AI assist key when none is kept in the keychain for that provider. |
| `APIX_CLAUDE_CONFIG` | The Claude Desktop configuration file that Connect to Claude writes, instead of the real one (for tests). |
| `APIX_AI_URL` | Where the AI assist sends its Messages API requests instead of Anthropic (a proxy, a test server). |
| `APIX_GEO_URL` | Where **Download DB-IP Lite** downloads from instead of DB-IP. |
| `APIX_PLUGINS_DIR` | The plugins folder, instead of `<config>/Apix/plugins`. |
| `GITHUB_BASE_REF` | Set by GitHub Actions on pull requests: `apix run --changed` without a value compares with `origin/<it>`. |

## Measuring performance

`scripts/measure.ps1` builds the release app and measures:

- **Release binary** size.
- **Start to first frame:** from `main` until the first frame is drawn
  (`APIX_BENCH=startup`), median of several runs.
- **Memory when idle:** the working set five seconds after starting with the
  example collection open.
- **Large JSON on screen:** a GET of a 100 MB JSON body from the local
  `big_json_server` example, from sending until the response is drawn
  (`APIX_BENCH=json:<url>`), so the network is not part of the number.

```powershell
powershell -File scripts/measure.ps1          # Windows PowerShell
pwsh scripts/measure.ps1 -NoBuild -Runs 7     # PowerShell 7, also on macOS/Linux
```

The script prints a Markdown table with the machine it ran on; the README shows
the latest one. The timings come from `crates/apix-desktop/src/bench.rs`, which
does nothing unless `APIX_BENCH` is set.

Other helpers:

```sh
cargo run -p apix-http --release --example big_json_server -- 100   # 100 MB of JSON on :8765
cargo run -p apix-http --example oauth_server                       # OAuth 2.0 test server on :8766
cargo run -p apix-http --example sse_server                         # server-sent events on :8767 (resumes after Last-Event-ID)
cargo run -p apix-http --example ws_server                          # WebSocket echo (binary too) and ticker on :8768
cargo run -p apix-http --example graphql_server                     # a GraphQL endpoint (schema and a few users) on :8769
cargo run -p apix-http --example socketio_server                    # Socket.IO 4: namespaces /, /chat, /ticker, /private (auth) on :8771
cargo run -p apix-grpc --example grpc_server                        # a gRPC Greeter with reflection on :8770
cargo run -p apix-http --example soap_server                        # a SOAP calculator on :8773 (WSDL at ?wsdl; faults with 500 and 200)
cargo run -p apix-proto --example mqtt_broker                       # an MQTT broker on :8772 (`clock` ticks every second)
```

## Interface languages

Interface texts are in `crates/apix-desktop/locales/<code>.yml`, one flat
`key: value` per line, read by rust-i18n through the `tr!` macro. A unit test
fails when a language lacks a key or uses other `%{placeholders}` than English.
To add a language, copy `en.yml`, translate it, and add one line to `LANGUAGES`
in `src/i18n.rs`.

## Documentation languages

`Docs/languages.toml` lists the documentation languages. Translations start
with a stamp naming the English text they come from:

```
<!-- translated from en/user-guide.md sha256:0123456789abcdef -->
```

```sh
cargo run -p docs-check            # lists missing or out-of-date translations
cargo run -p docs-check -- stamps  # prints the current stamp of each English document
```

## The vendored gpui backend

GPUI's Windows text renderer (crate `gpui-pre-windows` 0.3.9) placed every run
of glyphs left to right, so Arabic words appeared mirrored. `vendor/gpui-pre-windows`
is that crate with `DrawGlyphRun` fixed (the change is marked "Apix patch"), and
the workspace `Cargo.toml` patches it in. It can be removed once the upstream
crate is fixed.

## Continuous integration

`.github/workflows/ci.yml` runs on every push and pull request:

| Job | What it checks |
|---|---|
| Format and docs | `cargo fmt --check`; `docs-check` (translations follow the English docs) |
| Engine and CLI | `cargo clippy -D warnings` and `cargo test` for everything but the desktop app, on Windows, macOS and Linux |
| Desktop app | the same for `apix-desktop` on the three systems (Linux installs the window, font and keychain libraries first) |
| GitHub Action | the repository's own action (`uses: ./`) against `examples/jsonplaceholder`, on Linux and Windows, and that it wrote its reports |

`.github/workflows/release.yml` builds the release packages on a version tag
(`v1.2.3`, which must match `Cargo.toml`) and publishes them as a GitHub
release with `SHA256SUMS.txt`; started by hand it only builds them, as the
run's artifacts. Windows: `scripts/package-windows.ps1` (Inno Setup 6,
`packaging/windows/apix.iss`: per-user or per-machine install, optional PATH
and `.apix` association, uninstaller). macOS: `scripts/package-macos.sh`
builds both architectures, joins them with `lipo` into `Apix.app` (ad-hoc
signed) and makes a `.dmg`. Linux (on Ubuntu 22.04, so glibc 2.35 is enough):
`scripts/package-linux.sh` makes the tarball with `install.sh`, the `.deb`
(dependencies from `dpkg-shlibdeps`, plus `libvulkan1`) and a filled-in
`PKGBUILD`; `scripts/package-arch.sh` builds the Arch package from them in
an `archlinux` container. The `.deb` and the Arch package are installed in
the workflow and checked for missing libraries. `.github/workflows/pages.yml`
publishes `site/` to https://11r5.github.io/Apix/ .
Each release also carries a filled-in Homebrew cask (`apix.rb`, from
`packaging/homebrew`) and winget manifests (`AhmetDemir.Apix*.yaml`, from
`packaging/winget`), made by `scripts/channel-manifests.sh`, to copy into the
tap repository ([11R5/homebrew-apix](https://github.com/11R5/homebrew-apix))
and a `microsoft/winget-pkgs` pull request. `scripts/action-repo.sh` writes
the files of [11R5/apix-action](https://github.com/11R5/apix-action), the
Marketplace copy of `action.yml` that always downloads a release (the
Marketplace does not list a repository with workflow files); after each
release it gets a `v<version>` tag and `v1` moves to the new commit.
`.github/workflows/docker.yml` publishes the container image on version tags.
The action caches the `apix` it builds under a hash of the engine's sources,
so a new commit rebuilds it and an unchanged one reuses it.

## Tests

| Crate | What is covered |
|---|---|
| apix-core | model and file format round trips, variables, environment secrets in the keychain (skipped without a credential store), signing against the AWS test suite, RFC 7616 (Digest), RFC 7636 (PKCE) and the OAuth 1.0 example; GraphQL schemas (reading introspection, SDL) and what is suggested and described at each place in a query (fields, aliases, arguments, enum values, input objects, lists, fragments, variable types, comments and strings); the `[grpc]` table; history entries, gRPC calls included; GraphQL variable declarations and completion in the variables (JSON); the trusted CA setting; filtering JSON with JSONPath and jq (matches, outputs in order, errors in words, endless filters stopped); the URL's query and its rows both ways, `:name` path variables found and filled (only in the path, encoded, a second fill changing nothing), tidying a request's URL; header names and values suggested (start before contains, none once typed in full); examples (text and bytes round trips, the request as written, no Set-Cookie); git against a real repository (changes since a branch point, not committed, between commits, a file at a commit, an unknown ref), what a changed file touches (scripts, folders, environments, the project, files a request names), request diffs (rows by name, formatting ignored, body line by line, moves); monitor intervals and `[[monitor]]` round trips; AI assist requests (credentials masked, bodies cut, the language, fallbacks only for the default model) and answers (text blocks, refusals, error types, the script in a code block); comparing responses (headers by name with volatile ones left out, JSON field by field with records matched by id, formatting ignored, text and binary bodies); records found in JSON for tables (envelopes, plain values, number and label columns, aligned text) |
| apix-http | request building, every body type, a local server for OAuth 2.0, Digest, AWS and OAuth 1.0, the network trace; the SSE parser (pieces split anywhere), streaming against a server that sends slowly, WebSocket (echo, auth and params in the handshake, closing from either side, a refused handshake), fetching a GraphQL schema with the request's auth and the errors when there is none, HTTPS with an own certificate authority; WebSocket subprotocols and binary messages, `wss://` with an own CA or unchecked; what an event stream carries over to a reconnection; Socket.IO packets, handshake addresses and binary attachments, and against a local Socket.IO 4 server: events, acks both ways, pings, namespaces with auth and refusals, binary attachments, the server making the client leave, a plain WebSocket server told apart; WebSocket pings at an interval with their pongs timed, a ping left unanswered ending the connection, `ping_interval` in the file; TLS details of the request's own handshake against a local HTTPS server (version, cipher suite, group, the chain it sent with key and fingerprint, handshake time matching the TCP/TLS split); gzip, zlib, raw deflate and brotli bodies decoded in pieces, cut-short bodies refused, `Accept-Encoding` asked for unless the request sets one, `Content-Encoding` kept |
| apix-grpc | addresses, status codes, headers and auth as metadata; against a local Greeter (built from dynamic messages, with and without reflection): listing and describing the service, all four kinds of method, auth reaching the server, a failed status, .proto files with imports, and the errors for no description, an unknown method, a message that does not fit, a list for a unary method, a missing file and no server; example messages; a call as an HTTP response for runs (status mapping, `grpc-status`, a stream as a list); TLS against a server with a certificate from its own authority (refused, trusted with the CA, accepted unchecked); definitions: real generator output kept in `tests/fixtures/generated` (protoc 3.19 for Python, Java, C#, C++, PHP; protoc-gen-es; ts-proto) must give back the original service, Go, Rust, betterproto and annotations give numbers and types, the same type read from 10 languages, JSON examples, comparing with a server, a guess giving way to reflection, unknown fields reported, a wrong answer type explained |
| apix-proto | MQTT addresses, logins and options; against a local broker: every quality of service, wildcards, subscribing and leaving while connected, refused subscriptions and bad topics, retained messages, logins and kept sessions, the broker hanging up, `mqtts://` with an own CA or unchecked |
| apix-codegen | API documentation (folders, masked credentials, no `.local.toml` values, Markdown descriptions without raw HTML, anchors); shell quoting for bash, cmd and PowerShell; curl paste; round trips; every language's URL, headers, bodies, string escaping and settings |
| apix-import | Postman v2.0/v2.1 fixtures; WSDL: both SOAP versions, ports and addresses, document and rpc styles, headers, examples from the schema (extension, enums, choices, `form`, recursion), imports and what could not be read, locations, WSDL 2.0 refused |
| apix-script | variables and request changes from pre-request scripts, `pm.test` / `pm.expect` results and messages, the older `tests[...]` API, errors naming the file, endless scripts stopped, `setNextRequest`, `atob`/`btoa`, `pm.sendRequest` (callback and `await`, waiting not counted), the built-in libraries against known digests, checks without code, values and expressions for response targets |
| apix-runner | CSV (quotes, line breaks, `;`, BOM) and JSON data files, dotenv files and `APIX_VAR_*` names, iteration counts and data rows, pause/stop timing, JUnit/JSON/HTML/Markdown output and escaping (Markdown for pull requests: marker, escaping, what failed, a `--changed` selection), dates; whole runs against a local server (data rows, sidebar order, `bail`, `setNextRequest`), iterations side by side against a slow server (faster than one after another, listed by iteration); monitors: when the state turns, logs kept to their last results and apart per collection, schedules that follow the log, webhook messages |
| apix-runner (load tests) | percentiles by nearest rank, duration parsing; against a local keep-alive server: a number of requests over kept connections (about one per worker for 400 requests), a steady rate, a time limit, stopping from elsewhere, 500s and refused connections counted by kind |
| apix-runner (flows) | flow files read and written, start node, edges replaced and removed, problems named, subjects of a response (status, time, headers, JSONPath, jq, nothing there), comparisons; against a local server: a value kept by one request used in the next one's URL, a polling loop with a check and a wait, a failing request with and without a fail edge, a value that is not there, an unknown request file, an endless loop stopped at 1000 steps, a flow that cannot run |
| apix-cli (MCP) | the protocol (versions, notifications, unknown methods, the tool list with and without `--list-only`); against a local server: listing requests and environments (names, no values), sending with an environment and with the agent's variables, scripts' tests in the result, running every request, paths outside the collection and missing files refused |
| apix-mock | path patterns (`{{base_url}}/users/:id`, hosts, queries), the most literal match, picking by `x-mock-response-name` and `-code`, 2xx first, decoded bodies without `Content-Encoding`, the 404 list, preflights; a real server on a port answering through the HTTP client, with CORS, and serving an example added while it runs |
| apix-capture | saved requests keep no cookies or credentials, host filters; through real local servers: plain HTTP recorded and saved, HTTPS opened with the local authority (the client trusting only it), HTTPS tunnels passed on unread |
| apix-route | lines of tracert (a translated one too), traceroute and tracepath; local addresses; the map projection and distances; a database written in the MaxMind DB format: places, addresses it does not know, private ones, picking the file; installing a download (gzip, refused when not a database, the old one replaced only after); the system's own tool tracing 127.0.0.1 |
| apix-plugins | the example plugins loaded with their manifests; the HMAC signature against one computed independently; CSV import and field stats; modules written as text: an endless loop stopped, imports refused, a bad name, a missing manifest, an answer outside memory, a file that is not WebAssembly; also `[auth.plugin]` round trips (apix-core) and an importer's answer as request files (apix-import) |
| apix-vault | encryption format, tampering, password change, recovery key, keychain entries (against the real credential store under a test service name; skipped where none exists) |
| apix-desktop | translations complete, language helpers, file paths, highlighting query, theme files (reading, errors, round trip), accent and transparency colours, GraphQL schema file names, a gRPC call's log in the history |
| docs-check | documentation structure and stamps |
