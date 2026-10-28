# Batch 4 — bounded instance resources

Implementation, clean performance series and final diagnostics/gate are complete locally; this containing commit is the local checkpoint. No push, main merge, deployment or shared8443 restart. Only batch4 is authorized.

**Accepted outcome:** repeated in-budget workloads stop recreating instance buffers/material bind groups after warmup. **Not proven:** a general FPS or tail-latency gain. Strict moving-stress pixel repeatability remains open; preserve the failed comparisons below.

## Implementation

See [design](DESIGN.md). Keep existing pass/view/storage-layout ownership and mutable-content/generation guards. Active prefix differs from retained page capacity; inactive streams and standalone buffers can survive temporary layout/admission changes. Standalone growth replaces only its vertex buffer. One64MiB known-buffer /4096handle budget evicts inactive resources globally, using owner access order and slot-local exact/fine invalidation. One-frame pass grace and active/pinned working sets are reported exceptions, never hidden by dropping geometry. No destroy or synchronous GPU wait.

Rc leases follow material buffers/bind groups and instance buffers into prepared rows. Release counters mean last engine ownership, not GPU completion. Clear/final-asset removal clears maps/caches; partial asset removal can retain generic bounded transform/uniform pages. Shader/layout/view generations and mirrors stay separate. Creation counters increment at actual Rust wgpu create calls, not CPU Vec changes. GPU API diagnostics are separate evidence.

## Protocol and identity

[Fixed primary and diagnostic protocols](PROTOCOL.md), [manifest](MANIFEST.json). Desktop1280x720,DPR1, same authored Rome/Neon, seed1337, matched quality.44 clean runs /53290 full RAF rows:8s warmup/25s sample, threeAB/BA/AB pairs per dynamic configuration/backend, one pair per static/Neon control. GPU timing queries and native stage clocks are OFF; GPU times unavailable in these primary runs. Callback wall time includes waits/driver/sampling work and is not pure computation. No owned builds or second GPU runner overlap. Foreign GPU work is preserved and prevents isolated-capacity/small-gain claims.

Baseline8e03df21/sourcee8b39/WASMbdae44; candidateWASMc77e4b. Final SDK JSDoc annotation differs from the measured SDK bytes; canonical minified runtime hashes are identical (exact hashes/proof retained). Rust source/WASM unchanged. Original primary and revised diagnostic drivers are separately archived/hashed.

## Clean measurements

Values are medians of per-run means/quantiles; max is the worst individual callback across the series. Creation counts are per25s sample (all three repetitions agree). B→C means baseline→candidate. Static/Neon rows have only one pair and are regression controls.

| Workload | Backend | Instance buffers / bind groups B→C | CPU p95 ms B→C | CPU p99 ms B→C | Worst callback ms B→C | Cadence FPS B→C |
|---|---|---|---:|---:|---:|---:|
| rim-pool-cycle 64 | webgpu | 816/804→0/0 | 3.50→3.70 | 4.70→4.90 | 10.10→9.30 | 60.00→60.00 |
| rim-pool-cycle 64 | webgl | 816/804→0/0 | 4.10→4.10 | 5.10→5.20 | 9.50→9.40 | 60.00→60.00 |
| rim-pool-cycle 1024 | webgl | 2424/2412→0/0 | 15.20→15.30 | 21.00→19.90 | 70.70→96.40 | 50.93→51.04 |
| rim-pool-cycle 1024 | webgpu | 2424/2412→0/0 | 14.80→15.00 | 18.60→20.00 | 71.40→70.00 | 58.80→58.88 |
| rim-clustered 1024 | webgpu | 268/268→0/0 | 47.60→47.60 | 49.00→48.80 | 58.20→59.80 | 25.88→25.86 |
| rim-clustered 1024 | webgl | 268/268→0/0 | 48.40→48.20 | 50.00→49.50 | 64.20→58.40 | 25.20→25.30 |
| rim-stress 1024 | webgl | 0/0→0/0 | 15.60→15.80 | 17.00→18.00 | 18.20→20.70 | 42.51→42.20 |
| rim-stress 1024 | webgpu | 0/0→0/0 | 15.30→15.20 | 16.90→16.80 | 29.70→18.10 | 59.88→59.96 |
| arena-active 1 | webgpu | 0/0→0/0 | 2.00→2.00 | 2.60→2.60 | 3.80→4.40 | 60.00→60.00 |
| arena-active 1 | webgl | 0/0→0/0 | 2.90→2.80 | 3.30→3.30 | 5.70→3.80 | 60.00→60.00 |

The baseline creates21048 instance buffers and20904 material bind groups over the complete measured windows; candidate0/0 after warmup in the same configurations. This is the instance-pool scope, not all engine/browser GPU allocations. New model/asset/private-uniform creation remains outside this pool. Real spawn/despawn is separately checked; the primary pool cycle keeps loaded assets and changes visibility.

Observed candidate known live capacities plateau at213216B (64roots) and2211456B (1024roots), identical end/peak values across repetitions; no retired leases remain at those boundaries. Default64MiB limit is not exercised by these stress roots; separate low-byte/handle/stream-limit tests exercise pressure/eviction/fallback. No claim of whole-process or physical VRAM plateau: bind-group driver bytes, queue-held backend memory and physical destruction completion are unavailable.

FPS is approximately neutral. Mixed p99 and a worse large-WebGL maximum96.4ms remain visible; no universal stutter improvement is claimed. Small callback differences under desktop GPU contention do not justify an adaptive fallback. The accepted change is bounded lifetime/reuse, not a proven frame-throughput improvement.

Full work-counter boundaries include draws/instances/triangles, upload bytes/calls, rebuilds, buffer/bind creation/release and capacity gauges in [results](RESULTS.json) and compressed raw evidence. Dynamic time-based workloads naturally sample slightly different phases/frame counts; do not infer identical totals from matched inputs.

## Strict image boundary

**Primary strict pixels19/20 exact,1FAIL.** Two live-Neon control pairs have no deterministic image gate. The failing fully-moving1024 WebGPU pair differs in8pixels,max2 channel units. Keep it FAIL; no tolerance/reference change. Additional frozen.375 controls demonstrate unchanged baseline repeat instability (one repeat14906pixels,max85; another restored8pixels,max1), but the failing candidate primary SHA has not been observed in this batch baseline sample. Candidate independent cache-off/culling-off comparison is1/3exact (other2:4pixels,max1). Therefore neither universal stress parity nor unchanged instability frequency is established; causality is unresolved. Deterministic pool fixture and actual-eye independent oracles are separate passing evidence, not substitutes for this limit.

## Separate allocation diagnostics

Twelve final25s instrumented pages reproduce the real API creation reduction. On WebGPU, pool64 creates816buffers/804bind groups and1765824known buffer bytes in baseline; pool1024 creates2424/2412 and25744704B; cluster1024 creates268/268. Candidate0/0 and0new known bytes in every warm window. WebGL reports matching buffer/createData counts (BG API absent); exact bytes include backend-specific padding. Cumulative Rust reasons and bounded API/label totals include cold startup; warm details never exhaust the unchanged50000event cap. Four earlier completed diagnostic pages and one cold-overflow rejection remain preliminary, outside this final series.

Baseline allocations occur in12RAF callbacks per pool cycle series /4 in cluster. Candidate has no allocation-associated callbacks, so their quantiles are **unavailable**, not zero. Global instrumented p99 remains mixed, including a worse clusterGL diagnostic42.2→47.3ms. Callback association includes wrappers, payload/model updates, uploads, driver waits and remaining rendering; it does not isolate allocation cost. Full API draws/instances/triangles and completeness flags are in RESULTS. Profiling/queries stay disabled; no GPU duration is claimed.

## Verification and checkpoint

One complete `VERIFY_WASM_PROFILE=release npm run verify:push` PASS:423native tests (2existing manual timings ignored),143/143browser scenarios, SDK/harness/XR/types/format/native+WASM strict clippy. Release was reused only after exact source/generated-byte checks.35performance-helper units pass separately, including exact GL source ranges/window gating. Targeted motion/light-class/actual-eye/asymmetric/reentry checks pass. Final pool fixture adds actual spawn/despawn, byte/handle pressure, 16B↔112B, low-stream standalone fallback, final-asset removal, clear/reload plateau2944/2944/2944B with live0 after every clear, and active dispose/reinit with exact independent pixels on both backends. Independent reviewer source findings corrected; source review is not GPU evidence.

The project gate does not convert the separate stress-pixel FAIL into PASS. Full manifests/requested bytes/raw distributions and archive readback are audited. Local checkpoint only. No physical Quest/CI/push evidence. Stop after batch4; batch5 requires a separate instruction.


Commands actually executed (exact arguments/timestamps and failed attempts retained in evidence):

```sh
RUSTFLAGS=--cfg=web_sys_unstable_apis cargo check --target wasm32-unknown-unknown --locked --quiet
RUSTFLAGS=--cfg=web_sys_unstable_apis wasm-pack build --target web --release -- --locked
VERIFY_WASM_PROFILE=release node scripts/build-fingerprint.mjs --write
VERIFY_WASM_PROFILE=release node scripts/freeze-render-build.mjs output/next-render-cycle-4/baseline
VERIFY_WASM_PROFILE=release node scripts/freeze-render-build.mjs output/next-render-cycle-4/candidate
node scripts/test-browser.mjs --backend all --page gltf-instance-pool
node scripts/test-browser.mjs --backend all --page gltf-plan-motion,gltf-light-classes,webxr,webxr-webgpu
node --test --test-isolation=none scripts/render-performance.test.mjs
node output/next-render-cycle-4/run-calibration.mjs
node output/next-render-cycle-4/run-primary.mjs
node output/next-render-cycle-4/pixel-oracle.mjs
node output/next-render-cycle-4/run-diagnostics-final.mjs
VERIFY_WASM_PROFILE=release npm run verify:push
```

The output coordinators call the maintained CLI sequentially; they are not a second benchmark implementation. No full gate or GPU runner overlapped primary. The local evidence archive contains full frozen manifests, both generated packages/changed inputs, exact original primary driver and revised diagnostic/final sources, all primary/preliminary/rejected/capture/raw data and final gate logs. Unchanged tracked base assets and historical archives are not duplicated; reconstruction instructions and per-member hashes are inside. Exact archive identity is in MANIFEST.json. Full local frozen roots remain available under the isolated output directory.


Archive:27033390B,506entries (359unique payloads/145exact-content hardlinks),180454130recorded uncompressed bytes. SHA256`cab88de081124ab6cd1accdb33fb678639b641bec33121307926ee6794df3b14`; every payload/link read back. Input audit verified4979frozen files,612actual served payload records,198driver-file hashes and all44raw quantile distributions. Four preserved foreign GPU processes /32–98% aggregate AGX utilization snapshots prevent isolated small-gain claims.
