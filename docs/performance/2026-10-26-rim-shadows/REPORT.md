# Rim shadow policy follow-up — 2026-10-26

## Scope

The user requested independent static/dynamic shadow layers in the generic engine and cheap NPC ground shadows in the actual Rim game. The generic engine already caches two shadow maps per light: `Effects::prepare` keys each layer by its own scene caster revision, the light shadow matrix and culling mode. A moving camera or non-caster must not redraw either map; a dynamic caster must redraw only the dynamic layer. Moving/deleting a static caster still invalidates its static layer. The color pass samples the cached depth maps every displayed frame; caching map generation does not eliminate fragment shadow filtering.

No generic shadow policy or GLSL/WGSL kernel was changed in this follow-up. The relevant renderer sources are exactly the retained a54 baseline; `output/render-performance-rim-blob/checks/renderer-baseline.json` records this check. The separate per-vertex shadow-coordinate experiment remains parked as a patch under `output/render-performance-vertex-shadow/`; it is not part of this delivery.

## Actual Rim change

The original Three scene's NPC exhibition uses flat translucent black circles rather than dynamic shadow-map casters. The native encounter already marked NPCs as non-casters, but hid the imported exhibition decorations without creating replacement moving ground circles. Authoring now creates 14 independent instances of one shared 128-triangle thin closed black disk asset, alpha .08 per face (~.154 compounded through two surfaces). Native Rust supplies their transforms and lifetime alongside enemy state: fixed pavement height .136, enemy-centered x/z, radius .3–.9 from authored model height, hidden while inactive, dead or paused and during teardown. No per-frame geometry creation, raycast, worker message or additional shadow depth pass is introduced. The circles are an intentional ground-contact approximation, not mapped silhouettes or terrain projection.

`configureRim` accepts optional `blobShadows`, absent/null becoming an empty pool for backwards compatibility. Rust requires an empty pool or exactly one distinct scene-owned handle per NPC. Blob mode suppresses those NPCs' mapped casting locally and keeps receiving city shadows. It saves and restores full original render options on teardown, allowing later configuration with mapped NPC shadows. Other objects retain the general dynamic-map path.

## Acceptance

- Focused native Rim tests: 27 passed, including living/moving/wounded/dead/teardown blobs, independent shadow revisions and restoration of original flags.
- SDK tests: 31 passed; TypeScript passed; strict WASM Clippy passed.
- Read-only independent review: lifecycle restoration issue found, fixed and re-reviewed with no blocking findings. Reviewer did not run builds or GPU tests.
- The first browser check rejected the initial open disk: the mesh pool requires a closed solid. The authoring geometry was corrected to a .001-thick closed disk without changing the engine or its validation; the final stereo pixel check must verify the actual result.
- Browser/release/deployment evidence is appended after completion below. Ordinary browser and simulated stereo XR evidence must not be described as physical Quest acceptance.

## Performance boundary

The map cache was already present; this follow-up does not claim a new FPS gain from adding it. Moving circles add a small explicit geometry/blending workload and restore the intended NPC contact cue. Earlier a54-vs-Three timings remain separate immutable evidence and do not establish equal graphics. No push or CI was requested.

## Final production acceptance

All eight maintained release/minified production cases passed: `shadow-cache`, `rim-gameplay`, actual `rim` entry and normal `rim-refresh`, each on actual WebGPU and WebGL. [Browser report](evidence/browser-report.json), [simulated stereo gameplay report (last WebGL run)](evidence/rim-gameplay-report.json), [production manifest](evidence/production-manifest.json). Both backend PASS results require the stereo blob pixel assertions; the runner's separate per-suite XR detail file retains only its last backend run.

In the three-light cache scenario, each backend recorded: camera motion **0 static / 0 dynamic** updates; visible non-caster motion **0 / 0**; dynamic caster motion **0 static / 3 dynamic** (one update per light); static caster motion **3 static**. New/removed casters and migration between layers also passed. This is direct work-count evidence, not FPS measurement.

Final release WASM `6c4bc5dd9e4e98c0ad42ebbc52aec25034511fb7ba75068c07fcc414b597219e`, source `fd8bbe73edfdc4922ae41eb8c66870e0e6c7751d6b3666098b8df3ade7ef113f`, app `de4fef8a1a`. The extra release build after the lifecycle fix is the one verified. The a54 renderer kernel stayed unchanged; WASM differs because of native gameplay policy/lifecycle code. No current FPS result or physical Quest acceptance is claimed for this feature change.

## Live delivery

`npm run serve:quest` rebuilt the production package from the matching release and now serves app `de4fef8a1a` on `https://192.168.11.134:8443`. All file records match the exact browser-tested staging package. Six actual decoded HTTPS payloads, including the changed scene module, match their hashes and have `Cache-Control: no-store`; engine/application HTML markers and TLS chain/hostname passed using the existing local CA. [Live deployment proof](evidence/deployment/deployment-check.json), [verification script](verify-deployment.mjs). The previous a54 served package was retained locally under `output/production-before-rim-blob-20261026/`. Original Three on8878 and the primary checkout were untouched. These changes remain local/uncommitted; no push or CI run.
