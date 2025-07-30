# WebGPU in Rust via WebAssembly

This repository contains a minimal example of using WebGPU from Rust
compiled to WebAssembly. The example clears a canvas with a color
inside the browser.

The build enables both the WebGPU and WebGL backends of `wgpu`. Browsers
that lack WebGPU will transparently fall back to WebGL so the demo still
runs, while WebGPU is preferred when available.

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

If the command fails with an error mentioning `console_error_panic_hook` or a
missing `vendor` directory, rename `.cargo/config.toml` to
`.cargo/config.offline.toml` and rerun the build. The default configuration uses
a vendored crate source which requires the `vendor/` directory produced by the
offline helper scripts.

## Running

Serve the `index.html` file with any static web server so that the
browser can load the WebAssembly module.

```bash
python3 -m http.server
```

Then open `http://localhost:8000` in a browser with WebGPU enabled.

### Checking WebGPU support

You can verify whether your browser runtime supports WebGPU and WebGL
without compiling the example. Install the Puppeteer dependency and run
the helper script:

```bash
npm i
node check_webgpu.js
```

The script will print whether WebGPU and WebGL are available in a fresh
headless Chromium instance.

### Capturing a screenshot

You can verify the WebGL cube renders correctly by taking a screenshot:

```bash
npm i
node screenshot_render.js
```

The script starts a temporary HTTP server, loads `cube.html` in headless
Chromium and saves `render.png` in the repository root (the file is ignored by
Git). The same image is also written to `render_base64.txt` for convenient
copy‑paste.

If Chromium fails to launch because of missing system libraries, install them
with `apt` first. A minimal set of packages is:

```bash
sudo apt-get install libatk1.0-0 libgtk-3-0 libnss3 libx11-xcb1 \
  libxcb-dri3-0 libxcomposite1 libxdamage1 libxfixes3 libxkbcommon0 \
  libxrandr2 libgbm1 libasound2
```

## Offline usage

This repository ships `vendor.tar.gz` together with the
`rustup_cache.part.*` and `cargo_cache.part.*` archives so builds can happen
without network connectivity. Use the helper to unpack everything:

```bash
./offline.sh unpack-all
```

To rebuild the archives with all dependencies and toolchain caches run:

```bash
./offline.sh pack-all
```

If you later change dependencies you can regenerate the archive and refresh the
metadata using:

```bash
cargo vendor --sync ./vendor
```


The script is idempotent: if a `vendor/` directory already exists it will skip
the extraction step so previously downloaded crates are reused.

### Creating offline archives

Run `./offline.sh pack-all` on a machine with an initialized toolchain to
produce `vendor.tar.gz`, `rustup_cache.part.*` and `cargo_cache.part.*`.

Copy these files next to the repository so they can be unpacked later with
`./offline.sh unpack-all`. This registers the toolchain under the name
`stable-offline` which you can use when building and testing:

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

The file `.cargo/config.offline.toml` configures Cargo to use these local
sources. Copy it to `.cargo/config.toml` when building without network
access:

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

