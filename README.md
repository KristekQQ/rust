# WebGPU in Rust via WebAssembly

This repository contains a minimal example of using WebGPU from Rust
compiled to WebAssembly. The example clears a canvas with a color
inside the browser.

## Building

Install the WebAssembly target for Rust and build with `wasm-pack`.
The WebGPU API in `web-sys` is still unstable, so compilation requires
enabling those APIs via `RUSTFLAGS`.

```bash
rustup target add wasm32-unknown-unknown

# Install `wasm-pack` if it is not already available.
# (Do **not** include this comment after the command.)
cargo install wasm-pack

RUSTFLAGS=--cfg=web_sys_unstable_apis wasm-pack build --target web
```

This will create a `pkg/` directory with the generated JavaScript and
WebAssembly files.

## Running

Serve the `index.html` file with any static web server so that the
browser can load the WebAssembly module.

```bash
python3 -m http.server
```

Then open `http://localhost:8000` in a browser with WebGPU enabled.

## Offline usage

This repository ships the file `vendor.tar.gz` containing all required
crates so that builds can happen without network connectivity. Run the
provided `./evendor.sh` script to unpack the archive and prepare the
`vendor/` directory. The script also accepts a `vendor.tar.zst` archive
if present. To regenerate the archive with all dependencies, run
`./build_vendor.sh`.

```bash
./evendor.sh
```

If you later change dependencies you can regenerate the archive with
`./build_vendor.sh` and refresh the metadata using:

```bash
cargo vendor --sync ./vendor
```

The script is idempotent: if a `vendor/` directory already exists it will skip
the extraction step so previously downloaded crates are reused.

### Creating toolchain caches

Run `./pack_toolchain.sh` on a machine with an initialized Rust toolchain to
produce the `rustup_cache.part.*` and `cargo_cache.part.*` archives. Each
archive is split into 50 MB chunks:

```bash
./pack_toolchain.sh
```

Copy these files next to the repository so they can be assembled later with
`./join_toolchain.sh`.

To build fully offline you also need a Rust toolchain that already contains
the `wasm32-unknown-unknown` target. Place `rustup_cache.part.*` and
`cargo_cache.part.*` next to the repository and assemble the caches with
`./join_toolchain.sh` (or equivalently `./offline.sh join-toolchain`):

```bash
./join_toolchain.sh
```

This registers the toolchain under the name `stable-offline`. Use it when
running tests and building:

```bash
RUSTUP_TOOLCHAIN=stable-offline \
cargo test --offline
cargo build --target wasm32-unknown-unknown --release --offline
```

When setting up a fresh machine or continuous integration worker, the
`ci_offline_setup.sh` script reconstructs the cached toolchain, unpacks the
`vendor` directory and runs the test suite in one step:

```bash
./ci_offline_setup.sh
```

Cargo is configured in `.cargo/config.toml` to use these local sources:

```toml
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
```

After unpacking the vendor directory you can build and test completely
offline:

```bash
cargo test --offline
cargo build --target wasm32-unknown-unknown --release --offline
```

Run `wasm-bindgen` before entering the offline sandbox and copy the resulting
files along with any required runners (for example `node` or `wasmtime`):

```bash
wasm-bindgen --target web --out-dir wasm_out \
  target/wasm32-unknown-unknown/release/webgpu_wasm.wasm
```

After copying the repository, `vendor/` and `wasm_out/` directories and the
runner binaries allow you to run the example without network access.


## Dumping sources

Run `scripts/dump_sources.sh` to generate `all_sources.txt` containing all Rust
sources, shader files and the `Cargo.toml`. This is convenient when sending the
project to GPT or other tools. To run it automatically before each push, copy
`scripts/pre-push` to `.git/hooks/pre-push` in your local clone.

