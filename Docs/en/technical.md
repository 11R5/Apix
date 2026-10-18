# Technical notes

How Apix is built, what it depends on, and how to measure it. The
[architecture](architecture.md) explains the design; this page is the practical
side.

## Repository layout

```
crates/
├── apix-core       request model, variables, on-disk format, projects, signing (sign.rs)
├── apix-http       the HTTP engine (reqwest), network trace, OAuth 2.0 tokens
├── apix-import     Postman collection and environment import
├── apix-codegen    "Copy as" (curl, curl.exe, PowerShell) and curl paste
├── apix-vault      encrypted collections
├── apix-cli        the `apix` command
└── apix-desktop    the desktop app (GPUI); interface texts in locales/
tools/
├── icongen         builds the app icons from Logo/
└── docs-check      lists stale documentation translations
vendor/
└── gpui-pre-windows  gpui's Windows backend with a right-to-left text fix
scripts/
└── measure.ps1     performance measurements for the README
examples/
└── jsonplaceholder a small collection the app opens when run from the repo
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

The release profile uses fat LTO, one codegen unit and stripped symbols. A full
release build of the desktop app takes a few minutes.

## Main dependencies

| Area | Crates |
|---|---|
| UI | `gpui-kit` 0.7 (GPUI and its component library), tree-sitter grammars for JSON and GraphQL |
| HTTP | `reqwest` 0.12 with rustls, `tokio`, `hyper-util`, `tower` layers for timing |
| Network details | `x509-parser` (certificates), `netdev` (interfaces, gateway, DNS servers) |
| Signing | `sha2`, `sha1`, `hmac`, `md-5`, `base64`, `rand_core` (AWS SigV4, OAuth 1.0, Digest, PKCE) |
| Encryption | `chacha20poly1305` (XChaCha20-Poly1305), `argon2`, `zeroize` |
| Files | `serde`, `toml`, `serde_json`, `notify` (file watching), `trash` |
| Languages | `rust-i18n` (shared with gpui-kit), `sys-locale` |

## Files Apix writes outside collections

| File | Contents |
|---|---|
| `<config>/Apix/settings.toml` | language, theme, auto-lock time |
| `<config>/Apix/state.toml` | recently opened collections, the chosen environment per collection |

`<config>` is `%APPDATA%` on Windows, `~/Library/Application Support` on macOS
and `~/.config` on Linux. OAuth 2.0 tokens and passwords of encrypted
collections are never written to disk.

## Environment variables

| Variable | Effect |
|---|---|
| `APIX_PASSWORD` | Password of an encrypted collection for the CLI (`--password-env` picks another variable). |
| `APIX_THEME` | `obsidian` or `cyber`; overrides the theme setting. |
| `APIX_BENCH` | `startup` or `json:<url>`: measure and quit (see below). |
| `APIX_BENCH_OUT` | File to append the measurements to. |

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

## Tests

| Crate | What is covered |
|---|---|
| apix-core | model and file format round trips, variables, signing against the AWS test suite, RFC 7616 (Digest), RFC 7636 (PKCE) and the OAuth 1.0 example |
| apix-http | request building, every body type, a local server for OAuth 2.0, Digest, AWS and OAuth 1.0, the network trace |
| apix-codegen | shell quoting for bash, cmd and PowerShell; curl paste; round trips |
| apix-import | Postman v2.0/v2.1 fixtures |
| apix-vault | encryption format, tampering, password change, recovery key |
| apix-desktop | translations complete, language helpers, file paths, highlighting query |
| docs-check | documentation structure and stamps |
