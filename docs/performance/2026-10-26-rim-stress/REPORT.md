# Actual Rim load boundary — 2026-10-26

The native Rim example now has an optional **0–2,048 additional static Drone Orbiter models**. The default is zero; the selector and meaningful `stress=N` URL parameter survive refresh. The native spatial diagnostics panel reports `ZATEZ NAVIC` beside its existing FPS/render counters. These are shared-asset render instances without AI, projectile simulation or shadow casting. Original combat remains available. Created count is not a visible-object count when the user looks elsewhere.

## Measured desktop boundary

Apple M1, Chrome 155, gate camera, **2880×1920**, same native resolution/effects, profiler off. FPS means reciprocal mean application RAF interval, not compositor presentation or uncapped headroom.

| Additional models | Native WebGPU observed FPS | Evidence |
|---|---:|---|
| 256 | 60.003 | One 8 s warmup / 25 s sample; capped |
| 512 | **50.275 median** | Three 8 s / 25 s samples: 55.522, 50.275, 48.050 |
| 1,024 | 28.311 | Short 3 s / 8 s pilot only |

This brackets a repeatable below-60 workload; it does not establish a precise sustainable capacity. Short calibration selected 400 as its conservative near level and 512 as overload; longer repeated results take precedence over its short probes. At 512, native submits about **10.55 million triangles per frame**. This deliberately heavy static load is not a prediction of live-enemy capacity or headset FPS.

## Three.js comparison at 512

Three alternating AB/BA/AB pairs, same 8 s / 25 s protocol, camera, authored assets, static pose, requested model count and positions:

| Renderer | Median observed FPS | Individual samples |
|---|---:|---|
| Native WebGL2 | **18.746** | 18.666, 18.801, 18.746 |
| Original Three.js WebGL2 plus ordinary shared-geometry clones | **12.786** | 12.786, 13.147, 12.697 |

Native's existing automatic batching records **5,266 actual draw calls** including postprocessing, versus **34,345** actual Three calls. Native legacy `renderer_stats().drawCalls=34343` is a primitive estimate, not the submitted-call count; use `debugStats().drawCalls` or the existing VR panel for actual calls. No counter implementation change was needed.

The Three adapter uses ordinary root clones, not a hand-optimized `InstancedMesh` implementation; the generic `threeRepresentation=instanced` setting in raw reports does not describe this authored-Rim adapter. Native submits 10,553,756 triangles versus Three's 10,554,110: 354 fewer (about 0.0034%). Native reports eight culled primitives; their exact contribution to the difference is not fully attributed. Native FXAA and Three MSAA4, shaders and shadow policies remain different. Therefore this is an authored-scene diagnostic, not pixel/full-game parity or a universal renderer verdict. Earlier zero-load/high-resolution results favoring Three remain valid. **No new renderer speedup is claimed for this load-test feature.** Original port-8878 sources/server were preserved.

## Separate profiling diagnosis

One separate profiled 4 s / 10 s WebGPU run at 512 produced 15 valid timestamp samples: `scene.main` median **15.711 ms**, `fxaa` **8.409 ms**. CPU callback median was 21.9 ms; sampled `gltf.queue` 13.8 ms and `render.encode.color` 17.4 ms are nested scopes and must not be added. These are separate measurements, not an end-to-end GPU latency sum. Profiler-on 43.901 FPS is excluded from the primary figures. Both submission/queue work and GPU rendering merit investigation before any optimization; this feature does not change their kernels.

## Verification and release

Native Rim tests **28 passed**, strict WASM Clippy, release WASM build and TypeScript passed; focused tooling/stress/diagnostics tests **40 passed**. The minified production package passed **eight maintained browser cases** across WebGPU and WebGL2: actual Rim verification, mobile controls/refresh/load removal, stress with simulated XR and cutting, and shadow-cache behavior. At 32 additional models, both simulated eyes show changed pixels; native counters and spatial panel content are checked. This is browser/simulated-XR evidence, not physical Quest acceptance.

Acceptance caught two real overlaps before delivery: touch controls covered the load selector, and the footer covered the expanded verification button. Touch controls now participate in footer layout and appear during play; the expanded, scrollable verification panel has explicit stacking. The maintained runner checks actual center-point hit targets.

Release WASM **`36cabbf4ac596d867181c8278a848b99fc3197910f344835b02a3aba519c7ea2`**, source **`eae9f0a3d0b60d131a4031465d86d7006cdc5ab816fa83912967a023cfab48e0`**, production app **`6082380ab4`**. The owned server at **192.168.11.134:8443** was restarted through `npm run serve:quest`; its complete manifest matches the browser-tested staging package. Eight actual HTTPS decoded payloads, including stress/diagnostics modules and WASM, match their hashes and carry `no-store`. TLS chain and IP were verified using the existing local development CA; this does not establish headset trust. Previous package: `output/production-before-rim-stress-20261026`.

Changes remain local on isolated `codex/wasm-render-performance`, HEAD `64bfda68`; no commit, push or CI is claimed. Main-checkout sources/package are preserved. For physical Quest, start at 16 or 32 and increase while reading the in-scene panel; its refresh target and stereo workload differ from desktop 60 Hz.

## Reproduction and retained evidence

[Summary](summary.json), [archive checksums](archive-sha256.json), [deployment check](evidence/deployment/deployment-check.json). `collect-evidence.py` archives existing outputs without rerunning timing; all compressed archives were round-trip/hash checked. Raw reports preserve errors/limits, observed phase boundaries, frozen asset manifests, driver identities and cleanup. Every timing run reports stable inputs, no server errors and complete cleanup.

The early smoke/calibration driver is archived separately from the later fixed-load driver; exact source hashes match each report. The later change added explicit single-probe calibration for the Three comparison. After timing, a unit test's error-message expectation and a redundant unreachable bound guard were corrected; the actually measured driver remains archived. Final UI layout/feedback and the helper's pure pose-function extraction followed timing without altering measured Rust/WASM or model positions. Production acceptance used the final application.
