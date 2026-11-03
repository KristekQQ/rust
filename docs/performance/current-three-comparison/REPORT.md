# Current native renderer vs pinned Three r170 — 2026-10-31

Fresh direct comparison, not historical numbers and not the preceding moving-camera A/A test. Current local native release WASM59615b78/sourcec8b6ee44; Three r170 uses the existing per-part InstancedMesh reference. Both sides request the same city/drone GLB bytes,2048orbiters in the same positions and Idle@0, same gate camera and1280x720DPR1. Actual asset/module request hashes were checked.

**This is a static limited common base-color profile:** no AA, lighting, shadows, fog, tone mapping or postprocessing on either side. It is not production graphics, active animations/gameplay or VR. The new animation allocation optimization is therefore not the work being stressed. Native per-primitive culling and Three aggregate instance bounds still submit different triangle counts. Exact cross-engine pixel equality is not claimed. Do not turn these results into a universal equal-quality renderer ranking.

## Main result

Each row averages three per-run FPS values (1000/mean RAF interval), not inverse-median cadence. Three is shown separately beside each native campaign, preserving time-local paired observations.

| Campaign | Renderer | FPS mean (run range) | Mean callback wall ms | Actual draws | Submitted triangles |
|---|---|---:|---:|---:|---:|
| wg | native-wg | 44.919 (44.872–44.982) | 2.3596 | 287.0–287.0 | 39,061,900–39,061,900 |
| wg | Three-WebGL2 | 38.597 (38.587–38.602) | 0.7303 | 86.0–86.0 | 42,009,212–42,009,212 |
| gl | native-gl | 39.203 (39.070–39.349) | 3.3367 | 287.0–287.0 | 39,061,900–39,061,900 |
| gl | Three-WebGL2 | 38.606 (38.602–38.616) | 0.7424 | 86.0–86.0 | 42,009,212–42,009,212 |

Native counts are actual glTF command/triangle counters, not the legacy unbatched estimate. Three counts come from renderer.info.render; this profile has no postprocess pass, but counter scopes remain labelled. Do not normalize different triangles into hypothetical equal-work FPS. CPU callback is wall time, including browser/API/scheduling, not pure compute. GPU timings are unavailable in clean profiler-off samples; shorter CPU callback does not automatically imply higher FPS.

## Every pair

| Native backend | Pair | Native FPS | Three FPS | Native callback mean ms | Three callback mean ms |
|---|---:|---:|---:|---:|---:|
| wg | 1 | 44.872 | 38.587 | 2.3487 | 0.7172 |
| wg | 2 | 44.982 | 38.601 | 2.3895 | 0.7359 |
| wg | 3 | 44.902 | 38.602 | 2.3404 | 0.7377 |
| gl | 1 | 39.189 | 38.602 | 3.2079 | 0.7575 |
| gl | 2 | 39.349 | 38.602 | 3.4509 | 0.6950 |
| gl | 3 | 39.070 | 38.616 | 3.3514 | 0.7747 |

## Tails and exact image observations

| Campaign | Pair | Side | Frame p95/p99/max ms | Callback p95/p99/max ms |
|---|---:|---|---|---|
| wg | 1 | native-wg | 33.40/33.40/33.50 | 2.90/3.00/4.60 |
| wg | 1 | Three-WebGL2 | 33.40/33.40/33.50 | 0.90/1.00/1.60 |
| wg | 2 | Three-WebGL2 | 33.40/33.40/33.50 | 0.90/1.00/1.20 |
| wg | 2 | native-wg | 33.40/33.40/33.50 | 2.90/3.00/4.60 |
| wg | 3 | native-wg | 33.40/33.40/33.50 | 2.90/3.00/3.70 |
| wg | 3 | Three-WebGL2 | 33.40/33.40/33.50 | 0.90/1.00/1.20 |
| gl | 1 | native-gl | 33.40/33.40/33.50 | 3.90/4.00/4.60 |
| gl | 1 | Three-WebGL2 | 33.40/33.40/33.50 | 0.90/1.00/1.30 |
| gl | 2 | Three-WebGL2 | 33.40/33.40/33.50 | 0.90/1.00/1.10 |
| gl | 2 | native-gl | 33.40/33.40/33.50 | 3.90/4.00/5.10 |
| gl | 3 | native-gl | 33.40/33.40/33.50 | 3.90/4.00/5.20 |
| gl | 3 | Three-WebGL2 | 33.40/33.40/33.50 | 0.90/1.00/1.50 |

Independently decoded native/Three PNG differences (no tolerance adjustment):

| Campaign | Pair | Changed RGBA pixels | Max channel difference /255 |
|---|---:|---:|---:|
| wg | 1 | 50 | 255 |
| wg | 2 | 50 | 255 |
| wg | 3 | 50 | 255 |
| gl | 1 | 50 | 255 |
| gl | 2 | 50 | 255 |
| gl | 3 | 50 | 255 |

These are cross-engine output differences in an already limited profile, not an accepted native before/after pixel match or a fix for historical native A/B/C/D blockers. Whole gameplay and Quest acceptance remain separate.

## Execution and reproducibility

12clean runs:3alternating pairs for each native backend against ThreeWebGL2.8s warmup/25s sample; native-only2s/5s fixed-load calibration is pilot-only. Actual profiler=false/minimal sampler/zero sampled polls/no recorder overflow, no server errors/input drift; full raw-series distributions independently recomputed. Configured headless Chrome, matched browser version within each campaign, host power/thermal endpoints retained; normal desktop contention remains unisolated. No API observer/GPU timer during clean runs. [Protocol and exact commands](PROTOCOL.md).

Raw local evidence: output/current-three-comparison/{wg-calibration,gl-calibration,wg-three,gl-three}; raw.json.gz/full manifests/PNG/report.json per campaign. analyze.py and pixel-audit.json preserve reduction/image evidence. Initial redundant --candidate-root flag attempt failed CLI validation before browser startup; error log preserved. No runtime/driver source edits, native rebuild/test rerun, deployment/shared8443 restart/commit/push/merge. This report is a new measurement only; it does not replace earlier evidence or claim a production/VR victory.
