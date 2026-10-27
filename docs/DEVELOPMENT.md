# Development and verification

## Produkční sestavení pro Quest

`npm run build:production` sestaví rychlostně optimalizované release WASM
(`opt-level=3`, fat LTO, jedna codegen jednotka, `wasm-opt -O3`), minifikuje JS/CSS
a připraví gzip/Brotli varianty do izolovaného `output/production/`.
Moduly ukázek zůstávají oddělené; nejde o sdílený runtime bootstrap.
Manifest obsahuje režim sestavení a SHA-256 jednotlivých souborů.

`npm run serve:quest` (také `scripts/run-quest.sh`) vždy připraví tento balíček,
matching WASM znovu nepřekládá, a servíruje jej přes důvěryhodné HTTPS.
Vývojový `scripts/run-local.sh` nadále používá `--dev`, ale jeho server neslouží
k porovnávání FPS. Produkční server čte izolovaný balíček, nikoli pracovní `pkg/`.
HTTP odpovědi záměrně zakazují cache, aby Quest po nové iteraci nezůstal na staré verzi;
předkomprimované soubory zrychlují přenos, ne GPU shadery.

Ověření skutečně minifikovaného balíčku pomocí stejného browser runneru:

```sh
npm run build:production
npm run test:production
VERIFY_WASM_PROFILE=release npm run test:browser -- --production-root output/production --page neon-arena,neon-font-preview,webxr-webgpu,neon-arena-xr --backend all
```

Runner připojí `/tests/` výhradně pro ověření; produkční server tento mount nemá.
Nižší velikost a vyšší úroveň optimalizace samy nedokazují vyšší FPS.
Quest přijetí a výkon se posuzují samostatně na zařízení.

## Automated entry points

`npm ci` installs the committed lockfile. Use `npm run verify` for native tests/Clippy/format, SDK and harness tests, locked wasm32 check/Clippy/build, typecheck, then **every registered browser scenario on WebGPU and WebGL**. Install the pinned Rust toolchain, Clippy (`rustup component add clippy`) and wasm-pack 0.15.0 first. The runner uses this repository's pinned Puppeteer, closes each page before the next, and starts/stops its own loopback server on a free port.

Celý gate zahrnuje i přímé testy rendereru: render targety, stíny/odrazy a cache,
postprocessing, zrcadlené transformace, glTF deformace/PBR a culling se skutečnými
pixelovými kontrolami. Úspěch jedné ukázky ani samotných cutting testů tuto sadu
nenahrazuje. Registr scénářů je v `scripts/test-browser.mjs`; cílené `--page`
používej pro iteraci, nikoli jako náhradu plného předpushového gate.

Projektové skilly jsou verzované v [`.agents/skills/`](../.agents/skills/rust-webgpu-project/SKILL.md).
Osobní instalace téhož skillu není zdroj pravdy; obecné nástrojové skilly do
repozitáře nekopíruj.

```sh
npm run verify:native
npm run verify:push                            # jednou před commitem/pushem; hook výsledek použije znovu
npm run verify:fast                            # rychlý lokální gate bez Rust/GPU buildu
npm run verify:browser                         # matching WASM build + both backends
npm run verify:browser -- --page lifecycle,gltf-full
VERIFY_WASM_PROFILE=release npm run verify:browser -- --page cutting-resident --backend webgpu
npm run test:browser -- --backend webgpu       # reuse a matching build
npm run test:browser -- --backend webgl --software
node screenshot_render.js --backend webgl
git config --local core.hooksPath scripts
```

`test:browser` compares source and generated-WASM/binding hashes to the successful build marker; it fails on stale `pkg/`. `--backend webgpu` uses the app's auto request but requires the actual `BrowserWebGpu` backend, so fallback never counts as WebGPU success. `--backend webgl` requires `Gl`. `--software` explicitly selects SwiftShader; CI runs only that software WebGL state, not a hardware WebGPU benchmark. Set `BROWSER_EXECUTABLE` to use another installed Chromium/Chrome binary. On Apple Silicon use an arm64 Node installation to avoid Rosetta browser startup. `--headed` opens the automated browser; `--capture` saves canvas images from passed scenarios.

Při běžné úpravě spusť jen nejmenší užitečnou kontrolu: konkrétní `cargo test <filter>`, `test:sdk`, `test:harness`, typecheck nebo jednu dotčenou browserovou stránku na jednom backendu. Ani `verify:fast` není povinné po každé editaci. Dokumentace nevyžaduje GPU test. Celou sadu neopakuj po každé změně ani milestone; patří až před požadovaný commit/push.

Před commitem spusť **jednou** `npm run verify:push`, commitni i aktualizovaný dump a pushni. Hook použije uložený PASS pro stejné vstupy, nikoliv druhý běh testů. `verify`, `verify:native` a `verify:browser` ukládají lokální výsledek v ignorovaném `output/verification/pass-*.json`. Otisk zahrnuje zdroje, SDK, testy, příklady, assety, skripty, konfiguraci, závislosti, Rust a parametry/prostředí; commit stejného obsahu ho nemění. Cílený běh nenahrazuje plnou sadu, WebGL nenahrazuje WebGPU. Změny a selhání výsledek zneplatní; kontrolují se i aktuální WASM bytes. `VERIFY_FORCE=1 npm run verify` vynutí nové měření. Shodný WASM build se znovu negeneruje. Cache není sdílená s CI: čistý GitHub runner musí ověřit své vlastní prostředí.

Results, browser version, source/WASM and authored SDK/test/example hashes, and failed-page screenshots are saved under ignored `output/verification/`. Timeouts, failed page assertions, uncaught JS errors, error-level console output and HTTP failures produce a nonzero exit. Transport cancellations (`ERR_ABORTED`) are recorded separately because SDK tests intentionally cancel loads; awaited imports and pixel assertions still reject failed loading. The native harness regression deliberately fails the hook's test and dump stages and verifies that an existing source dump survives failed generation. The lifecycle browser page checks dispose during initialization, three dispose/reinit cycles, removed listeners, canceled RAF and rejected stale SDK handles. Presentation and glTF tests additionally inspect real canvas pixels. None of these establish performance across hardware or full glTF conformance.

For machine-specific executables, set `BROWSER_NODE` / `BROWSER_EXECUTABLE` or local Git options `verification.node` / `verification.browser` to absolute executable paths. These overrides are not committed or inherited by CI. The Node override also works when npm itself uses an Intel Node on an Apple Silicon Mac.

GitHub CI builds/tests native code and release WASM once, including a software-WebGL smoke
case, then transfers the matching `pkg/` artifact to four browser jobs. Each runs
`test:browser -- --backend webgl --software --shard index/4`; partitions apply
after backend filtering and cover the complete registry without duplication.
Reports record the planned cases, shard, per-case budget and compact progress so
cancelled runs retain useful evidence. The default remains 90 seconds per case;
observed heavy suites declare a 600-second `softwareTimeoutMs` budget in the registry.
Producer and consumer jobs both set `VERIFY_WASM_PROFILE=release` so software browser
acceptance uses the same optimized profile as local release verification.
An explicit `--timeout` overrides that policy. Hardware/local full-push coverage
and all scene, image and XR assertions are unchanged.

The installed `scripts/pre-push` requires the full successful gate (or a matching PASS receipt) before atomically generating `all_sources.txt`; any failure blocks push. Run it before committing so the dump is included. Keep unrelated staged changes intact. The hook is local configuration; each new clone installs it explicitly.

GitHub Ubuntu can block Puppeteer's Chromium sandbox through AppArmor. The disposable `software-webgl` job explicitly sets `BROWSER_CI_NO_SANDBOX=1`; the runner honors it only on Linux with `GITHUB_ACTIONS=true`. It runs only trusted loopback fixtures. Ordinary local browsers (including root sessions) do not automatically disable their sandbox; sandbox state is recorded in the browser report. This does not disable test assertions or backend validation.

Use checks for the changed area, after dependent writers finish. Group native build/check commands into one verification stage; inspect the affected page and console in one browser session. Keep a compact PASS/FAIL record with artifact/version. After fixes repeat affected checks; required final checks still apply. Do not rebuild merely for a new screenshot or unchanged page.

Následující matice určuje pokrytí pro závěrečnou kontrolu. Při iteraci z ní vyber jen dotčený test, nikoliv celou řádku po každé editaci.

| Change | Final coverage |
|---|---|
| CPU state/camera/visibility/loader | `cargo test`; actual browser/input workflow when visible behavior changes |
| Triangle cutting / sword sweep | `cargo test cutting` (caps, volume, concavity, repeated cuts, transforms, BVH, opt-in, atomic errors); matching WASM and `test:browser -- --page cutting,cutting-compute,cutting-resident,scene-sdk` on WebGPU + WebGL (actual controls/cap pixels, CPU-compute parity, actual resident GPU topology, readback-inclusive timings and stale-request cancellation); lifecycle after async/initialization changes; [API/limits](CUTTING.md) |
| Přímý WebGPU XR | `cargo test xr_ --lib`; matching WASM a `webxr-webgpu` na WebGPU (skutečné color/depth GPU array slices, stereo pixely, projekce, lifecycle), `webxr` + `neon-arena` na obou backendech a `neon-arena-xr` na WebGL. Simulátor nenahrazuje Quest. |
| Renderer/WGSL/web exports | wasm32 check, WASM build, affected Rust page and console; WebGPU + WebGL for shared rendering paths |
| SDK/types/callers | Build matching `pkg/`, `npm run typecheck`, `tests/scene-sdk.html` on affected backends |
| Canvas/output/targets | `tests/render-target.html` and its verify button on WebGPU + WebGL |
| Shadow/reflection/pass cache | `tests/render-effects.html` verify button + SDK integration on WebGPU + WebGL |
| Postprocessing/PBR interaction | Affected example and its verify control on WebGPU + WebGL; inspect real effect on/off or pose change |
| Frame graph / transient allocation / profiler | `cargo test frame_graph --lib` and `cargo test postprocess_plan --lib`; matching WASM + `frame-graph` (actual storage-buffer compute/readback), `postprocessing`, `webxr-webgpu` (including SSR pixels) on WebGPU and affected shared paths on WebGL. Check graph slot/lifetime diagnostics, stable-frame allocations, effect toggles/resize and both eye pixels; GPU timestamps may be unsupported. See [FRAME_PIPELINE.md](FRAME_PIPELINE.md). |
| glTF runtime/PBR | `tests/gltf-full.html`, `tests/gltf-sdk.html` on WebGPU + WebGL; actual textured/animated model for rendering changes |
| Mirrored transforms / winding | `tests/render-mirrored.html` pixel assertions and `tests/render-effects.html` verify on WebGPU + WebGL |
| SDK lifecycle/loading | `npm run test:sdk`; `tests/lifecycle.html`, `tests/scene-sdk.html` and `tests/gltf-sdk.html` on both backends |
| Character movement/collisions/chase camera | `cargo test` (BVH, sweep, gravity, jumping, invalidation); matching WASM build and `test:browser -- --page fox-control` on WebGPU + WebGL (real keyboard/mouse, solid wall, two platform landings, reset, clips and pixels) |
| Frustum BVH / Rust HUD | Native brute-force pass/invalidation/pruning tests; `test:browser -- --page culling,fox-control` on both backends (10k pruning, F3, glyph pixels, cached refresh, disabled counters, texture output) |
| Backend initialization | `index.html?backend=auto` and `index.html?backend=webgl` |
| Guidance/docs only | Validate links/config/skill as relevant; no GPU rebuild. At push reuse matching full PASS if tested inputs did not change |

```sh
cargo test
RUSTFLAGS=--cfg=web_sys_unstable_apis cargo check --target wasm32-unknown-unknown
RUSTFLAGS=--cfg=web_sys_unstable_apis wasm-pack build --target web
npm run typecheck
npm run test:sdk
```

`scripts/run-local.sh [port]` builds then serves loopback HTTP with no-store headers. Reuse a healthy running server; do not build a second time just to start serving. Generated `pkg/` is ignored. Match SDK exports to freshly built WASM and use final resource URLs after redirects. Initialize with `await initEngine(...)` before commands. SDK owns no simulation/RAF loop.

Native tests omit the wasm32 renderer/web bridge. PASS text checks only its explicit assertions: it does not alone prove shading, animation pixels or hardware performance. Inspect the affected real Rust page (`cube.html`/`render_webgpu.js` are separate demos). Capture only images needed to judge the change. Avoid concurrent render tabs, and restore/close temporary test pages.

Source ownership: `renderer.rs` owns scene GPU resources and accepts views/targets; `output.rs` presentation; `state.rs` connects them. `effects.rs` pass ordering/dirty caches; `textures.rs` reusable targets. Preserve per-pass visibility and static revision invalidation. Both `data.rs`/`shader.wgsl` and `gltf_gpu.rs`/`gltf_pbr.wgsl` require matching GPU layouts.

Inspect Cargo sources before dependency/offline changes. The offline config uses `vendor/`; toolchain restoration in `offline.sh` assumes Linux x86_64 and needs review on Apple Silicon. Packaging/restoration details are in README; do not run them for ordinary development.

Before requested commit/push use `npm run verify:push` once; do not separately repeat cargo/full/browser/hook checks. Commit only owned paths and the regenerated source dump. Reuse matching PASS receipts; invalidate only when their inputs change.

## WebXR / Meta Quest 3

Společný tester pro všechny VR hry: [VR_TESTING.md](VR_TESTING.md).
Spuštění zařízení a limity: [WEBXR.md](WEBXR.md).

`npm run verify:vr` spustí testy XR platformy, matching WASM a registrované
obecné/herní VR scénáře. `webxr-contract` kontroluje společný kontrakt enginu
ve třech virtuálních profilech, `webxr` backend a start/dispose, `webxr-simulation`
je samostatný adaptér skutečné hry s jablky. Vše používá jednu platformu
`tests/xr/simulator.js`; nekopíruj vlastní XR mock do další hry.

Reporty a přehratelné stopy jsou v `output/verification/*-report.json` a
`*-trace.json`. Pomalé přehrávání lze ověřit stejným runnerem přes `--xr-slow`.
`webxr-preview` zůstává běžný WebGL náhled. Automatika nepotvrzuje skutečný
Quest compositor, tracking, důvěru certifikátu, komfort ani výkon headsetu.
