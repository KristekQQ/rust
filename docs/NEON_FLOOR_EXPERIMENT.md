# Neon floor approximation

The bounded experiment replaces 80 bevelled tiles and a recessed grout mesh
(972 triangles) with one glTF primitive (4 vertices, 2 triangles). A repeating
512 x 512 texture bakes tile colors, soft grout bevels and subtle grain. Footprint
and 0.75 m tile spacing are unchanged. No parallax or displacement is used.

The texture variant is now the default in both web and native authoring. In the web example use
`?floor=texture`, `?floor=normal`, or `?floor=baseline`. The normal variant adds a
subtle normal map. Both variants use the existing glTF material shader; the
emissive texture does not turn that shader into a dedicated unlit fast path.
They look more matte and have softer seams than the original geometry. SSR
reflectivity remains 0.94; renderer glow, SSR quality and resolution are unchanged.

Recreate both web and native authoring assets with:

```sh
node scripts/generate-neon-floor.mjs
NEON_EXPORT_QUERY='?floor=texture' NEON_EXPORT_OUT=output/floor-texture-assets \
  node native/neon/scripts/export-scene.mjs
NEON_EXPORT_QUERY='?floor=texture' NEON_EXPORT_OUT=examples/neon-arena/parallel-assets/textured-floor \
  node native/neon/scripts/export-scene.mjs
```

The native exporter uses the separately versioned authoring snapshot, so keep
both floor modules and generated asset trees identical. Export alternate asset
trees into separate directories; do not rewrite a measured baseline. Candidate
floor handles are models (`floorTiles.info`), and `floorGrout` aliases the model.
Use `floorMetadata` for mode-independent geometry counts. The browser simulation
selects the matching textured-floor export using `world.floorMode`; normal and
texture share its geometry/IDs. Baseline retains the original worker asset tree.
Changing only the visible floor without matching the worker causes object-ID
mismatches. Authoring must stay paused until worker initialization, as in the
production entry point.

## Evidence and limits (2026-11-12)

Local repeated frozen stereo GPU measurements showed a median 9.74% reduction
for the texture variant looking down, but player-view pairs had mixed signs.
The normal variant was not better in the downward local comparison.

On physical Quest 3, three 600-frame paired downward-view comparisons measured
12.674 -> 12.461 ms, 12.659 -> 12.472 ms and 12.632 -> 12.522 ms. Median paired
change: -1.48%, range -1.68% to -0.87%. Clocks were not locked. These are whole-pass
offscreen GPU timings, not live OpenXR FPS. The earlier short smoke result was
much larger and must not replace the repeated result.

These downward measurements used the pre-audio `a7de7df3f` base. The working
experiment was then fast-forwarded to audio merge `0b830b2a5`; the shared renderer
was unchanged. Both variants passed standing/grazing-view rendering checks on
WebGPU and WebGL with matching WASM from the audio base. Native Android release
compilation also passed. Actual in-headset motion/appearance acceptance remains
pending while room tracking is unavailable.

With a fresh Android release binary from the audio base, three 600-frame paired
player-view comparisons measured 12.456 -> 10.748 ms, 12.483 -> 10.957 ms and
12.476 -> 10.825 ms. Median paired change: -13.24%, range -13.71% to -12.23%.
This is stronger evidence for the fixed player view than the downward result;
it still does not measure live FPS. All pairs used the same binary, matched
settings and separate hashed asset trees. App/PID and scores were restored
unchanged after the run.

Local raw evidence and exact binary/asset hashes live under
`output/measurement/floor-20261112/`: `LOCAL-SUMMARY.json`,
`quest-texture-floor/`, `quest-texture-player-audio-base/` and
`browser-audio-base.log`. The initial smoke had a restoration timeout; the later
downward run restored the existing APK/PID successfully with scores unchanged.

## Delivery

APK 0.13.0 (code 13) was installed over the existing audio release and launched
on Quest via Wi-Fi. Its SHA-256 is
`da3d0c9d7c254fd9a79bc7ad782dc7bb45f8ad6070f330f62d281f19c9f06ca9`.
The installed APK scene operations/refs match the measured texture candidate.
Scores, Quest Stream and the existing follow-app awake helper were preserved.
Live logs show rendering, but the off-face camera/conditions were not matched
for an FPS comparison. Audible playback after this install and wearer visual
acceptance are unverified.

The final targeted checks passed: texture/normal floor rendering and worker
initialization on WebGPU/WebGL; actual default game startup on both backends;
VR entry and both-eye downward floor visibility on simulated WebGL XR. Initial
new-test failures (missing worker scene and authoring pause/unpause ordering)
were corrected and affected checks rerun. No commit/push or web publication was
performed; web source is in the isolated task checkout.

## Live regression report follow-up

Honza reported dips near40 FPS in the headset after the Wi-Fi move. Five bounded
OpenXR scripted cut captures used native libraries identical byte-for-byte,
changing only floor assets. Stream and observer remained active. Complete stable
runs: original floor31.19 and56.72 app FPS; texture69.12 app FPS. Texture frame
interval p95 was18.58ms and max25.70ms. Original p95 values were39.63 and21.52ms.
The first texture capture had long focus/refresh gaps and mixed72/90/120Hz; exclude
it from matched comparisons. The last texture capture ended at41s with
`recorder_dropped`; exclude its incomplete67.89 FPS aggregate as well.

No regression caused by the textured floor was demonstrated. Baseline variance,
unlocked clocks and differing resulting cut counts prevent a precise causal
live FPS uplift claim. The low FPS/tail problem is NOT resolved. Retained Quest
runtime samples showed90–97% GPU utilization; the first capture's simulation
worker median was0.39ms (includes more than physics). These observations suggest
rendering/runtime pressure, without isolating a specific GPU pass.

Evidence: `output/measurement/floor-live-regression-20261112/COMPARISON.json`, raw
frames, manifests and logs. Restored the original0.13 textured APK hash above,
normal input and score persistence; diagnostic files removed, TOP5 unchanged,
observer connected and Stream decoded in browser. No source performance fix or
new APK version was introduced by this follow-up.
