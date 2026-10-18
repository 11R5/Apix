<p align="center"><img src="packaging/icons/apix-128.png" width="128" height="128" alt="Apix logo"></p>

# Apix
🚀 The high-performance, minimalist API client crafted with Rust and GPUI. Built for speed, designed for humans.

> **Status:** early development, not released yet. The desktop app and the CLI can
> already open projects and collections, send requests and import from Postman.
> See the [roadmap](Docs/ROADMAP.md) and [architecture](Docs/ARCHITECTURE.md).

## Why Apix
- **Native, GPU-rendered.** No Electron, no WebView. The UI is drawn with GPUI, the engine behind the Zed editor.
- **Local-first.** Collections are plain TOML files in a folder. Commit them, diff them, review them.
- **One engine everywhere.** The desktop app and the `apix` CLI send requests exactly the same way, so CI matches your machine.

## What works today
- **Requests:** GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS with params, headers,
  JSON / Text / XML bodies and an Auth tab (Bearer, Basic, API key). Folders,
  collections and projects can set auth once; requests inherit it.
- **Responses:** status, timing (TTFB and total), size, highlighted and foldable JSON,
  raw and headers views. A 100 MB JSON response opens in about a third of a second.
- **Network tab:** where the time went (DNS, connect, waiting, download), the server
  and local address, interface, gateway, DNS servers, proxy, redirects and the
  server certificate with its days left.
- **Collections and projects:** a sidebar built from the files on disk, with new,
  rename, duplicate and delete (to the trash); drag a request or folder to move it.
  Projects group collections, share environments, and can show the sidebar as
  Project → Environment → Collections.
- **Tabs:** open several requests at once. Each tab keeps its unsaved edits, its last
  response and even a request still in flight.
- **Variables:** project, collection and environment layers, secrets kept out of git,
  an in-app editor and dynamic values such as `{{$guid}}`. In the URL bar every
  `{{variable}}` is a coloured chip; hover it to see its value and where it comes from.
- **Postman:** collection v2.0/v2.1 and environment import, with a report.
- **Copy as / paste:** copy a request as `curl`, `curl.exe` or PowerShell; paste a
  `curl` command to fill the request.

The menu bar at the top (File, Request, Environment, View, Help) holds every
command: open, new collection or project, import, environments, auth and more.

### Keyboard

| Keys | Action |
|---|---|
| Ctrl+Enter | Send |
| Esc | Cancel the request |
| Ctrl+S | Save |
| Ctrl+N | New GET request (the sidebar "+" offers every method) |
| Ctrl+O | Open a folder, project or collection |
| Ctrl+K or Ctrl+P | Command palette: find any request or command |
| Ctrl+E | Variables and environments |
| Alt+1…7 | Switch the method |
| Ctrl+W | Close the tab (middle-click works too) |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab |

On macOS, use ⌘ instead of Ctrl (Ctrl+Tab stays Ctrl).

## Build

Requirements: [Rust](https://rustup.rs) (stable). On Windows you also need the MSVC
"Desktop development with C++" workload (Visual Studio or Build Tools).

```sh
cargo test                     # engine + CLI
cargo run -p apix-desktop      # desktop app
cargo build --release          # optimized binaries in target/release
```

## CLI

```sh
apix send examples/jsonplaceholder/users/list-users.apix
apix send examples/jsonplaceholder/health.apix --env local --var token=abc -i
apix send examples/jsonplaceholder/health.apix --network     # DNS, connection, TLS certificate, local side
apix run examples/jsonplaceholder
apix shell examples/jsonplaceholder/users/create-user.apix --as powershell   # or bash, cmd
```

In the desktop app, the copy button next to Send copies the open request as a
`curl` (bash), `curl.exe` (cmd) or PowerShell command. Pasting a `curl` command
into the URL field fills in the whole request.

`apix run` exits with a non-zero code if any request fails, so it can be used as a CI step.

### Encrypted collections

A collection can be encrypted with a password: every request, environment and
file name is sealed on disk (XChaCha20-Poly1305, Argon2id), and git only sees which
blob changed. In the app, right-click a collection and choose **Encrypt…**, or use
**File › New encrypted collection…**; you get a one-time recovery key. Collections
lock again after 15 idle minutes.

```sh
apix encrypt my-api --out my-api-encrypted        # asks for a new password
APIX_PASSWORD=… apix run my-api-encrypted --env staging
apix decrypt my-api-encrypted --out my-api-plain  # --recovery to use the recovery key
```

### Coming from Postman

```sh
apix import "My API.postman_collection.json"              # creates ./my-api
apix import "Staging.postman_environment.json" --into my-api
apix run my-api --env staging
```

Collection v2.0 and v2.1 are supported. Anything Apix cannot carry over yet (multipart
files, OAuth 2.0, saved examples) is listed in the import report. Pre-request and test
scripts are kept as `.js` files next to their requests. Secrets from Postman
environments go to a gitignored `*.local.toml`. In the desktop app, use the sidebar's
import button or drop the export onto the window.

## Collection format

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

Variables come from the project, the collection and the active environment
(edit them in the app with Ctrl+E). Secrets live in `environments/<name>.local.toml`,
which is gitignored. Dynamic variables such as `{{$guid}}`, `{{$timestamp}}`,
`{{$isoTimestamp}}` and `{{$randomEmail}}` get a new value every time they are used.

A request without `auth` inherits it from the nearest folder that sets one in its
`folder.toml`, then from the collection's `apix.toml`, then from the project.
`auth = "none"` sends no auth at all.

## License

[Apache-2.0](LICENSE)

The desktop app embeds [Inter](https://github.com/rsms/inter) and
[JetBrains Mono](https://github.com/JetBrains/JetBrainsMono), both under the
SIL Open Font License 1.1 ([licenses](crates/apix-desktop/assets/fonts)).
