# Releasing TSVM

TSVM releases are tag-driven. Pushing a tag that starts with `v` runs the
release workflow, builds executables, packages them, and creates a GitHub
Release.

## First Preview

The first preview tag is:

```sh
git tag v0.1.0
git push origin v0.1.0
```

The workflow publishes these archives:

- `tsvm-linux-x64-v0.1.0.tar.gz`
- `tsvm-macos-arm64-v0.1.0.tar.gz`
- `tsvm-windows-x64-v0.1.0.zip`

Each archive contains:

- `tsvm-demo`
- `tsvm-benchmarks`
- `README.md`
- `LICENSE`

## Local Release Smoke

Before tagging, run:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p tsvm-demo
cargo run -p tsvm-benchmarks -- 100
```

On this Windows workspace, the GNU target can be used when the MSVC linker is
not installed:

```sh
cargo +stable-x86_64-pc-windows-gnu test --workspace
cargo +stable-x86_64-pc-windows-gnu run -p tsvm-demo
```
