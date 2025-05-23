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

RUSTFLAGS=--cfg=web_sys_unstable_apis wasm-pack build --target web --offline
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
provided `./evendor` script to unpack the archive and prepare the
`vendor/` directory before building.

```bash
./evendor
```

The script extracts the archive and synchronizes Cargo's metadata via
`cargo vendor --sync ./vendor`.  It is idempotent: if a `vendor/`
directory already exists it will skip the extraction step so previously
downloaded crates are reused.

If you modify dependencies you can regenerate the archive with
`./build_vendor.sh` and refresh the vendor directory using:

```bash
cargo vendor --sync ./vendor
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
RUSTUP_TOOLCHAIN=stable-offline \
cargo test --offline --target x86_64-unknown-linux-gnu
cargo build --target wasm32-unknown-unknown --release --offline
```

Ensure that the `wasm32-unknown-unknown` target is installed **before**
going offline.  If it is missing the build will fail with an error like
`can't find crate for 'core'`.  Install it while online via:

```bash
rustup target add wasm32-unknown-unknown
```

Run `wasm-bindgen` before entering the offline sandbox and copy the resulting
files along with any required runners (for example `node` or `wasmtime`):

```bash
wasm-bindgen --target web --out-dir wasm_out \
  target/wasm32-unknown-unknown/release/webgpu_wasm.wasm
```

After copying the repository, `vendor/` and `wasm_out/` directories and the
runner binaries allow you to run the example without network access.
