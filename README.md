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
```

`apix run` exits with a non-zero code if any request fails, so it can be used as a CI step.

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
