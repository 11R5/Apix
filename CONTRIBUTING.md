# Contributing to Apix

Apix is open source under the [Apache-2.0](LICENSE) license: you may fork it,
change it, build it and use your version freely.

Changes to this repository go through pull requests:

1. Fork the repository (or, if you are a member of the 11R5 organisation,
   push a branch of your own).
2. Make the change with its tests, and run `cargo test`, `cargo clippy
   --all-targets -- -D warnings` and `cargo fmt --check`.
3. Update the documentation when behaviour changes (`Docs/en`, then
   `cargo run -p docs-check` lists the translations to bring up to date).
4. Open a pull request against `main`.

Only the maintainer merges into `main` and publishes releases; the branch and
the release tags are protected. Bug reports and ideas are welcome in the
[issues](https://github.com/11R5/Apix/issues).

## Building it yourself

```sh
cargo run -p apix-desktop      # the desktop app
cargo build --release          # the apix command line, in target/release
```

On Windows you also need the MSVC "Desktop development with C++" workload;
the [technical notes](Docs/en/technical.md) list what Linux and macOS need.
