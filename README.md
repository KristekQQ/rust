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

If you need to work in an environment without internet access, vendor all
dependencies and prepare WebAssembly artifacts ahead of time.

1. Vendor the crates and configure Cargo to use them:

```bash
cargo vendor --sync ./vendor
```

Create `.cargo/config.toml` with:

```toml
[source.crates-io]
replace-with = "vendored"

[source.vendored]
directory = "vendor"
```

2. Build and test completely offline:

```bash
cargo test --offline
cargo build --target wasm32-unknown-unknown --release --offline
```

3. Run `wasm-bindgen` before entering the offline sandbox and copy the
   resulting files along with any required runners (for example `node` or
   `wasmtime`):

```bash
wasm-bindgen --target web --out-dir wasm_out \
  target/wasm32-unknown-unknown/release/webgpu_wasm.wasm
```

After copying the repository, `vendor/` and `wasm_out/` directories and the
runner binaries allow you to run the example without network access.
