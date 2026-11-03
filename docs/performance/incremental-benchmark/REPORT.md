# Initial benchmark repeatability control — 2026-10-31

A benchmark already exists. This task freezes its incremental-use protocol and measures an A/A negative control, rather than creating another harness or changing the renderer. Both sides use the **same** verified animation-candidate release. There is no optimization, FPS gain or deployment in this control.

## Method and integrity

Moving-camera Rome stress,2048roots,1280×720,DPR1, seed1337, production graphics. Backend-specific calibration2s/5s is pilot-only. Main8s warmup/25s sample,3alternating pairs per backend;12runs total. Both pilots and all main runs use --no-profile --measurement minimal. Separate transient servers and configured headless Chrome; shared8443 untouched. Sources/build fingerprints and actual request URL/hash/byte maps match on both sides. Raw-series mean/median/p95/p99/max were recomputed independently and match reports. All samples assert profiler=false, zero sampler polls, disabled timer instrumentation, no overflow, unchanged driver input hashes and no server errors.

Observed variability applies to this session/workload only. Three run pairs are not a statistical precision guarantee, and individual consecutive frames are not independent experiment replicates. GPU times are unavailable. Per-run host power/thermal endpoints, actual phase/work counters and browser identity are preserved. The desktop is not fully isolated; endpoint snapshots are not continuous thermal monitoring. Neither this control nor simulated XR proves Quest performance.

## Every run

| Backend | Pair | Side | FPS | Mean frame ms | Frame p95/p99/max ms | CPU mean ms | CPU p95/p99/max ms |
|---|---:|---|---:|---:|---|---:|---|
| wg | 1 | baseline | 55.166 | 18.1272 | 33.30/33.40/33.40 | 15.8152 | 17.60/21.00/22.00 |
| wg | 1 | candidate | 55.045 | 18.1668 | 33.30/33.40/50.00 | 15.9300 | 18.30/21.10/25.80 |
| wg | 2 | candidate | 55.086 | 18.1536 | 33.30/33.40/33.50 | 15.9318 | 17.80/21.20/22.50 |
| wg | 2 | baseline | 55.126 | 18.1404 | 33.30/33.40/33.40 | 15.9239 | 17.70/21.10/22.30 |
| wg | 3 | baseline | 54.966 | 18.1932 | 33.30/33.40/33.40 | 16.1041 | 18.90/21.30/22.50 |
| wg | 3 | candidate | 55.082 | 18.1547 | 33.30/33.40/33.50 | 16.0495 | 18.40/21.10/27.30 |
| gl | 1 | baseline | 37.161 | 26.9096 | 33.40/33.40/33.50 | 17.1610 | 19.20/20.90/23.20 |
| gl | 1 | candidate | 37.177 | 26.8985 | 33.40/33.40/33.40 | 17.1922 | 19.00/21.00/21.60 |
| gl | 2 | candidate | 37.161 | 26.9097 | 33.40/33.40/33.50 | 17.2772 | 19.00/21.10/21.80 |
| gl | 2 | baseline | 37.186 | 26.8915 | 33.40/33.40/33.50 | 17.2313 | 19.00/21.00/22.00 |
| gl | 3 | baseline | 37.201 | 26.8806 | 33.40/33.40/33.50 | 17.2189 | 19.10/21.10/21.60 |
| gl | 3 | candidate | 37.201 | 26.8806 | 33.40/33.40/33.50 | 17.2400 | 19.20/21.10/23.70 |

## Apparent change without any runtime change

Positive means the side labelled candidate appears faster, despite identical builds. FPS uses reciprocal **mean** frame interval; CPU reduction uses mean whole callback wall time. Both are predeclared measures, distinct from the maintained driver's median-ratio summary.

| Backend | Pair | Apparent FPS gain % | Apparent CPU wall reduction % |
|---|---:|---:|---:|
| wg | 1 | -0.2178 | -0.7257 |
| wg | 2 | -0.0725 | -0.0497 |
| wg | 3 | +0.2120 | +0.3394 |
| gl | 1 | +0.0412 | -0.1818 |
| gl | 2 | -0.0676 | -0.2665 |
| gl | 3 | +0.0000 | -0.1223 |

This demonstrates why a lone0.1% change is not sufficient evidence here. The observed maxima/ranges are descriptive, **not** a95% interval, universal threshold or assurance that future noise cannot be worse. Small changes can still be useful: prove less work/allocations with correctness, then measure aggregate whole-frame benefit under predeclared repetitions. Do not add individual percentages together. For suspected tiny effects extend sample duration and independent pairs under a fixed budget, preserve all runs and label unresolved outcomes.

## Quality / limits

All6A/A captures independently decoded and compared exact RGBA:0changedpixels. This is this fixed capture state, not blanket gameplay pixel acceptance or resolution of historical A/B/C/D image blockers. No golden/tolerance change. Camera motion/stress state is validated by the maintained runner; work counters and real dt are preserved, not normalized into supposedly identical submitted triangles. No new Three run: pinned Three instancing/common and production profiles remain milestone comparisons per [protocol](PROTOCOL.md). No cold/soak/worker-latency or ordinary-game campaign was rerun here; they are future-candidate regression requirements, not completed tests.

## Reproduction and evidence

See [protocol](PROTOCOL.md). Existing runner command parameters/configuration and hashes are in raw reports. Local evidence is output/incremental-benchmark/{wg-calibration-host,gl-calibration,wg-aa,gl-aa}; each main directory includes report.json, raw.json.gz, requested-file manifests and six PNGs. analyze.py recomputes raw distributions and refuses mismatches. pixel-audit.json contains independent decoded-pixel results. The initial sandbox attempt could not start serveFrozen (unsettled top-level await); its empty output directory/log remain preserved. Host-authorized temporary local-server runs then completed; no fallback backend or sandbox disabling.

Runtime remains59615b78, sourcec8b6ee44; no runtime files changed, no rebuild/native suite needed for protocol/report-only work, no commit/push/merge/deploy/shared-server restart. Next bounded candidate is per-node shared transform publication, separately from XR presentation. The initial control cannot claim sensitivity to tenths of a percent.
