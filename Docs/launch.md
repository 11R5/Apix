# Launch posts (drafts)

Drafts for announcing Apix. Check the numbers against the README before
posting, and post from your own account. Both communities dislike marketing
language: plain facts, honest limits, and answering comments yourself work best.

---

## r/rust

**Title:** Apix: a native API client (Postman alternative) in Rust and GPUI, 87 MB idle, collections as plain TOML files

**Body:**

Hi r/rust! I've been building **Apix**, a desktop API client and command line
written in Rust, and it's now usable day to day, so I'd like your feedback.

**Why:** Postman, Insomnia and Bruno are Electron apps. On my machine they
use 550–950 MB of memory when idle, spread over 4–7 processes. Apix is one
native process, drawn on the GPU with GPUI (Zed's UI framework), and uses
87 MB. It installs as a 55 MB binary. Numbers and the measuring script are in
the README.

**What it does:**
- HTTP, GraphQL (schema completion), gRPC (reflection, `.proto`, or a definition
  guessed from JSON), WebSocket, SSE, Socket.IO, MQTT and SOAP
- Collections are folders of small TOML files you commit and review; no
  account, no cloud. `apix run --changed` runs only the requests a git change
  touched.
- Pre-request/test scripts with a Postman-compatible `pm.*` API on embedded
  QuickJS, so imported Postman collections keep working
- Runner with data files, load testing, monitors, a mock server, a capture
  proxy, flows, response compare, a Network tab with TLS details and the
  certificate chain, even the route to the server on a map
- Password-encrypted collections (XChaCha20-Poly1305 + Argon2id, file names
  included) that can live in a public repository
- The same engine as a CLI (`apix run`, JUnit/HTML reports) and a GitHub Action
- WebAssembly plugins (wasmi, no imports, fuel-limited) for auth types,
  importers and response views
- An MCP server mode so an AI agent can run your collection without seeing
  its secrets
- 9 interface languages, including right-to-left Arabic

**Rust bits you might find interesting:**
- The workspace is 16 crates: the engine (`apix-core`, `apix-http` on
  reqwest/rustls) has no UI, so the desktop app, CLI and MCP server share it.
- The Network tab hooks into rustls (a wrapping verifier and session store)
  to report the TLS version, cipher suite, key exchange and full chain of the
  request's own connection.
- Start-up was dominated by Direct3D device creation (~150 ms of the graphics
  driver loading). I moved it to its own thread at the very start of GPUI's
  Windows platform setup: 278 → 222 ms to the first frame.
- The documentation's 432 screenshots (48 screens × 9 languages) are taken by
  the app itself: a tour mode sets up each screen and a script captures the
  window, so they can be retaken after every UI change.

It's Apache-2.0. Builds for Windows, macOS (universal) and Linux (.deb, Arch,
AppImage, tarball) are on the download page, or `cargo run -p apix-desktop`.

- Code: https://github.com/11R5/Apix
- Download: https://11r5.github.io/Apix/
- Docs: https://11r5.github.io/Apix/docs/

It's early (0.1.1), and the builds aren't code-signed, so Windows SmartScreen
and macOS Gatekeeper will warn the first time. I'd love to hear what's missing
for your workflow and what breaks.

---

## Hacker News

**Title:** Show HN: Apix – a native API client in Rust; requests are TOML files in your repo

**URL:** https://github.com/11R5/Apix

**Text (first comment, posted right after submitting):**

Hi HN, I made Apix because I wanted an API client that starts fast, doesn't
need an account, and keeps requests as plain files next to the code.

It's a single native process drawn on the GPU (GPUI, the framework behind the
Zed editor). On my Windows machine it idles at 87 MB, compared with 550–950 MB
for Postman, Insomnia, Bruno and Hoppscotch measured the same way. The script
and the numbers are in the README, so you can check them on yours.

A collection is a folder of TOML files: you diff, review and merge requests
like code, and `apix run --changed=origin/main` in CI sends only the requests
a pull request touched. Secrets stay in a gitignored file or the OS keychain,
and a collection can be encrypted with a password, file names included.

Beyond REST it speaks GraphQL, gRPC, WebSocket, SSE, Socket.IO, MQTT and SOAP.
It runs Postman-style test scripts, so imported collections keep working, and
it has a runner, load tests, monitors, a mock server, a capture proxy and an
MCP server mode for AI agents. The CLI uses the same engine as the app.

Limits, honestly: it's 0.1.1, the builds aren't signed, and there is no team
sync beyond git. Apache-2.0. Feedback very welcome.

---

## Before posting

- Post r/rust on a weekday morning (US time), HN around 8–10 am US Eastern.
- Have the download page and the README screenshots ready; HN readers click
  the GitHub link first.
- Reply to the first comments quickly; say plainly what Apix does not do yet.
