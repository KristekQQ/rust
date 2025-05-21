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

This repository ships the file `vendor.tar.zst` containing all required
crates so that builds can happen without network connectivity.  Run the
provided `./evendor` script to unpack the archive and prepare the `vendor/`
directory before building. The script relies on the `zstd` tool to
decompress the archive, so make sure it is installed:

```bash
./evendor
```

If `zstd` is not available you can repackage the vendor directory as
`vendor.tar.gz` or `vendor.zip` on another machine and extract that
instead. After extracting, run `cargo vendor --sync ./vendor` once to
update Cargo's metadata:

```bash
# On a machine with `zstd` available
tar -I zstd -xf vendor.tar.zst
tar -czf vendor.tar.gz vendor  # or: zip -r vendor.zip vendor

# On the target system without `zstd`
tar -xzf vendor.tar.gz         # or: unzip vendor.zip
cargo vendor --sync ./vendor
```

The script is idempotent: if a `vendor/` directory already exists it will skip
the extraction step so previously downloaded crates are reused.

The script also synchronizes Cargo's metadata with the extracted crates. If
you later change dependencies you can refresh the vendor directory using:

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
