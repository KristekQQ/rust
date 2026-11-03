# Current pinned Three comparison — 2026-10-31

Question: where does pinned Three r170 stand against the current local native release on shared Rome content? This is a fresh direct comparison, separate from the preceding same-build A/A control and historical F5/closeout results.

Native frozen runtime: output/animation-workspace/candidate-runtime, WASM59615b78, sourcec8b6ee44 (full hashes in runtime manifest). Three uses that same frozen root's vendored r170 (three.module.js SHA256ce1fa418de16a19495a9f72495580e3015d7745c296d3ce0485897f902ddedfb) and existing per-part InstancedMesh factory. No runtime changes, cloned-per-enemy slowdown or custom benchmark renderer.

Existing maintained scripts/render-performance.mjs, family rim-common,2048orbiters plus same city, Idle@0, shared stress positions/root scale, gate camera65deg/.08/350,1280x720,DPR1. AA/lights/shadows/fog/tone mapping/postprocessing disabled on both sides; sRGB output and original textures/material base colors remain. This is an explicitly limited base-color common profile, not production-quality/gameplay/VR equivalence. It is static; preceding A/A used a moving camera and cannot be compared directly to these FPS values.

Native per-primitive/depth-bounded culling and Three aggregate InstancedMesh culling can submit different triangles. Keep actual work and image differences; do not normalize ratios into supposedly identical GPU work. Native dynamic gameplay/cutting is not active in this profile. No Quest performance claim.

For each native backend: fixed-load native-only2s/5s calibration pilot; same saved configuration reused in3alternating native/Three pairs with8s warmup/25s sample. --no-profile --measurement minimal. Serial headless browsers and ephemeral local servers; live8443 unchanged. GPU times unavailable in these clean runs. Record browser/power/thermal endpoints, actual served manifests and immutable raw series. Desktop contention is not fully isolated; small deltas have that limit.

Primary comparison: mean RAF interval/cadence under actual render load; whole callback wall time separately, plus p95/p99/max. Do not sum CPU/GPU or reinterpret CPU callback as pure compute. Native draw/triangle counts are command counters with documented scope, not legacy render_stats estimates; Three uses renderer.info. Exact cross-engine output is not an acceptance claim; independently decode PNGs and preserve all differences without changing tolerances. Historical native image blockers remain open.

Commands (fresh outputs each time, configured Node on PATH):

```sh
node scripts/render-performance.mjs calibrate --baseline-root output/animation-workspace/candidate-runtime --family rim-common --backend webgpu --load 2048 --max-load 2048 --width 1280 --height 720 --warmup 2000 --sample 5000 --no-profile --measurement minimal --max-time 180 --out output/NEW-wg-calibration
node scripts/render-performance.mjs compare --baseline-root output/animation-workspace/candidate-runtime --three-root output/animation-workspace/candidate-runtime --config output/NEW-wg-calibration/config.json --max-load 2048 --warmup 8000 --sample 25000 --pairs 3 --no-profile --measurement minimal --pixel-channel 0 --pixel-fraction 0 --max-time 900 --out output/NEW-wg-three
# Repeat serially with WebGL2 calibration (--backend webgl) and its own config/output.
```

--three-root itself selects the candidate; passing --candidate-root too is invalid. Initial redundant-flag attempt was rejected before any browser run; its error log is preserved separately. This task authorizes measurement/report only, no commit/push/deploy/shared-server restart.
