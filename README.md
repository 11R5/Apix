<p align="center"><img src="packaging/icons/apix-128.png" width="128" height="128" alt="Apix logo"></p>

# Apix
🚀 The high-performance, minimalist API client crafted with Rust and GPUI. Built for speed, designed for humans.

> **Status:** early development. The engine and CLI work; the desktop UI is a shell.
> See the [roadmap](Docs/ROADMAP.md) and [architecture](Docs/ARCHITECTURE.md).

## Why Apix
- **Native, GPU-rendered.** No Electron, no WebView. The UI is drawn with GPUI, the engine behind the Zed editor.
- **Local-first.** Collections are plain TOML files in a folder. Commit them, diff them, review them.
- **One engine everywhere.** The desktop app and the `apix` CLI send requests exactly the same way, so CI matches your machine.

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
apix run examples/jsonplaceholder
apix shell examples/jsonplaceholder/users/create-user.apix --as powershell   # or bash, cmd
```

In the desktop app, the copy button next to Send copies the open request as a
`curl` (bash), `curl.exe` (cmd) or PowerShell command. Pasting a `curl` command
into the URL field fills in the whole request.

`apix run` exits with a non-zero code if any request fails, so it can be used as a CI step.

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

## License

[Apache-2.0](LICENSE)

The desktop app embeds [Inter](https://github.com/rsms/inter) and
[JetBrains Mono](https://github.com/JetBrains/JetBrainsMono), both under the
SIL Open Font License 1.1 ([licenses](crates/apix-desktop/assets/fonts)).
