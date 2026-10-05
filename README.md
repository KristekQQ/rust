# WebGPU in Rust via WebAssembly

This repository is an interactive browser 3D scene built with Rust and
WebAssembly. It renders cubes, planes, spheres, a grid, and up to four lights,
with free/orbit cameras and scheduled animation/removal actions.

The project pins Rust 1.99.0 in `rust-toolchain.toml` and uses `wgpu` 30.0.1
and `glam` 0.34.0. `MEMORY.md` maps the sources; `AGENTS.md` describes the
verification and pre-commit workflow.

The build enables WebGPU and WebGL2. WebGPU is preferred when an adapter is
available; otherwise the renderer detects WebGL2. The page has an explicit
WebGL link (`index.html?backend=webgl`) and reports its active backend or a
startup error.

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

The default `.cargo/config.toml` uses the online crate registry. Only copy
`.cargo/config.offline.toml` into place after preparing `vendor/`. Generated
`pkg/` files are ignored by Git and must be rebuilt after changing Rust exports.

## Running

Serve the `index.html` file with any static web server so that the
browser can load the WebAssembly module.

```bash
python3 -m http.server --bind 127.0.0.1
```

Then open `http://127.0.0.1:8000/index.html`. For one-command development:

```bash
./scripts/run-local.sh
# Optional port: ./scripts/run-local.sh 8001
```

Drag on the canvas to look around; use W/A/S/D to move and +/− for Orbit
distance. The toolbar switches cameras, toggles the grid, and restores a fixed
sample scene. No objects, lights, rotations, or removals are scheduled automatically.

### Rust engine, JavaScript control

`src/scene.rs` contains the Rust `SceneManager`: objects, lights, transforms,
ID allocation, validation, animation scheduling, orbit calculations, removal,
and the sample scene definition. It has no browser or GPU dependencies and is
covered by native tests. `src/render/state.rs` owns GPU resources and renders
that manager's state. `src/web.rs` forwards WASM commands and runs the Rust loop.

JavaScript/TypeScript is a thin developer SDK. It stores handles (IDs), forwards
commands, and connects browser UI. It has no scene-state copy, animation loop,
interpolation, matrix calculations, or light movement calculations.

- `js/engine.js`: `initEngine(canvas)` awaits Rust and returns `engine.scene` plus the compatible low-level API.
- `js/engine.d.ts`: types for the scene API and object/light handles.
- `js/scene.js`: loads a JavaScript scene recipe through the command SDK.
- `examples/control-scene.ts`: a separate TypeScript usage example, not loaded by the main page.

Use the browser console or your own JS/TS code:

```js
const cube = engine.scene.createCube([-2, 0, 0]);
cube.setScale([0.7, 0.7, 0.7]);
cube.rotate([0, 180, 0], { duration: 2 }); // calculation and timing in Rust
cube.removeAfter(8);                     // Rust schedules removal
```

`engine.scene.clear()` clears objects, lights and their pending actions.
Import a `createScene` function from an example and call it with `engine.scene`;
the SDK does not contain sample loaders.
`engine.scene.objectCount` / `lightCount` read Rust state. Handles expose
`exists`; invalid commands throw when Rust rejects them. IDs are never reused,
including after scene reset, so old handles cannot target new objects.
Absolute rotations use radians; `rotate()` deltas use degrees and timing uses
seconds. The current bridge supports one `gpu-canvas` per page. `window.engine`
is available for console use; the old `window.scene` low-level API remains.

### Verification

```bash
cargo test
RUSTFLAGS=--cfg=web_sys_unstable_apis cargo check --target wasm32-unknown-unknown
```

Native tests cover the scene manager, simulation, camera input and projection. Renderer modules compile only
for WASM, so also rebuild `pkg/` and verify the main page, console, animation,
controls, and resize in a real browser. Before committing or pushing, run
`cargo test`, then `scripts/pre-push`.

The integration page tests the real WASM SDK (including Rust-timed removal):
`http://127.0.0.1:8000/tests/scene-sdk.html`, also with `?backend=webgl`.
For TypeScript declarations/example validation after building `pkg/`:

```bash
npm i
npm run typecheck
```

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

This helper captures the independent JavaScript/WebGL `cube.html` demo. It
does not test the Rust renderer or `index.html`:

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

**Host limitation:** restoration helpers currently assume Linux x86_64
toolchain paths. Review them before running on macOS/Apple Silicon. Offline
archives from older dependencies must be regenerated after this upgrade.
`pack-all` rebuilds vendor and archives the Cargo/Rustup caches.

The offline workflow uses `vendor.tar.gz` together with the
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
cargo vendor vendor
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
RUSTFLAGS=--cfg=web_sys_unstable_apis cargo build --target wasm32-unknown-unknown --release --offline
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
RUSTFLAGS=--cfg=web_sys_unstable_apis cargo build --target wasm32-unknown-unknown --release --offline
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

codex resume 019b6b22-2111-7001-88fb-070400b19da1
# Výkon rendereru

Rust provádí konzervativní frustum culling a instancing podle meshe.
Nezměněné transformace jsou cachované a instance se do GPU zapisují pouze
při změně dávky. Podrobná architektura, hranice a další kroky jsou v
[docs/RENDER_PERFORMANCE.md](docs/RENDER_PERFORMANCE.md).
Kontrolní stránka: `tests/render-stress.html` (také `?backend=webgl`).
SDK nabízí `engine.renderer_stats()` a `engine.set_frustum_culling(bool)`.

### Výstup rendereru

Renderer přijímá GPU barevnou/hloubkovou přílohu a pohled kamery nezávisle na
canvasu. Browser adaptér zajišťuje získání snímku a prezentaci. Přepnutí přes
JS/TS: `engine.set_output_mode("texture")`, návrat: `engine.set_output_mode("canvas")`.
[Ukázka renderovacích cílů](tests/render-target.html) vyžaduje lokální HTTP
server. Podrobnosti a omezení jsou v [plánu rendereru](docs/RENDER_PERFORMANCE.md).

### Stíny a zrcadlo

[Ukázka statických/dynamických stínů a odrazu](tests/render-effects.html) má
samostatné frustum culling a cache pro jednotlivé průchody; vše počítá Rust.
SDK přidává `setRenderOptions`, `spin`, `scene.setMirror`,
`set_render_effects`, `set_simulation_paused` a `effects_stats`.
[Architektura, použití a současná omezení](docs/RENDER_PASSES.md).

## JavaScript examples

[Rozcestník pěti samostatných ukázek](examples/index.html) odkazuje na aplikace
s vlastními main.js a scene.js; začni [základním příkladem](examples/basic/main.js)
přes Rust/WASM SDK. Spuštění: `scripts/run-local.sh`, potom
http://127.0.0.1:8000/examples/. Viz [návod](examples/README.md).
