<p align="center"><img src="packaging/icons/apix-128.png" width="128" height="128" alt="Apix logo"></p>

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
  `.local.toml`, OAuth 2.0 tokens only in memory, and a whole collection can be
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
| Release binary | 33.1 MB |
| Start to first frame | 274 ms |
| Memory when idle (working set) | 83 MB |
| 100 MB JSON response on screen | 325 ms |

Windows 11 (build 26300), AMD Ryzen 9 7900X, release build, median of 5 runs, 2026-10-18.

Reproduce it with `scripts/measure.ps1` (see the [technical notes](Docs/en/technical.md#measuring-performance)).
Measurements of the other tools on the same machine are planned.

## What works today

- **Requests:** GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS with params and
  headers; JSON, text, XML, URL-encoded form, multipart with files, binary file
  and GraphQL bodies.
- **Auth:** Bearer, Basic, API key, Digest, OAuth 1.0, OAuth 2.0 (client
  credentials, password, authorization code with PKCE in the browser) and AWS
  Signature V4. Folders, collections and projects can set auth once; requests
  inherit it.
- **Request settings:** timeout, redirects, TLS verification, proxy, client
  certificates and cookies on/off per request, inherited from folders,
  collections and projects like auth.
- **Cookies:** one persistent jar filled from `Set-Cookie` answers, with a
  manager to list, delete and clear.
- **Responses:** status, timing, size, highlighted and foldable JSON, raw and
  headers views, and the Network tab.
- **Collections and projects:** a sidebar built from the files on disk, tabs that
  keep their edits and responses, drag to move, projects that share
  environments.
- **Variables:** project, collection and environment layers, secrets kept out of
  git, dynamic values such as `{{$guid}}`, coloured chips in the URL bar.
- **Encrypted collections** with a recovery key and auto-lock.
- **Postman import** of collections and environments, with a report; a whole
  "Export data" zip or folder becomes one project. **Export to Postman** for
  teammates who stay on it.
- **Copy as** `curl`, `curl.exe` or PowerShell, or as code in fetch, axios,
  Python, Go, Rust, C#, Java, PHP, Swift and Kotlin; **paste** a `curl` command
  to fill a request.
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
