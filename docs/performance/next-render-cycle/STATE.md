# Batch 2 — complete locally

Only batches1–2 are complete; stop before batch3. Isolated `codex/next-render-cycle-2`, parent40b2a4ccc. Bounded64-slot/16MiB exact opaque-plan cache reuses prepared rows and skips warm selection/group/basis/payload/record rebuilds. Exact view/input/pipeline and mutable-buffer content epochs protect pass/eye ownership. Transparent ordering remains uncached. [Report](batch-2/REPORT.md), [results](batch-2/RESULTS.json), [manifest](batch-2/MANIFEST.json), [verification](batch-2/VERIFICATION.json).

Release source1452a9f7ca54; WASMaf2905cc7d71. Fresh2048-model stationary Rome,1280×720,DPR1,8s/25s,3AB/BA/AB pairs/backend, profiling off. Medians of run means: WebGPU callback43.24→17.36ms,cadence23.00→35.35FPS; WebGL2 callback47.98→20.82ms,cadence20.75→26.76FPS. All6 pairs exact pixels and identical307 glTF commands/126823 instance events/39067116 triangles per RAF. Warm candidate1hit/RAF,0group/payload/record rebuilds,0uploads; accounted plan102520B. Diagnostic1024-model rebuild phases11.67/13.78→0ms/RAF; lookup remains real CPU work.

139/139 browser and419 native PASS,2 existing manual timings ignored; SDK/harness/XR/types/strict wasm checks PASS;42 performance helper units PASS. One final verify:push and matching receipt reused after measurements. Full distributions/raw hashes checked; all frozen files and served payloads reverified. Local checkpoint only: no push, main merge, deployment, shared8443 restart or physical Quest test. Shared8443 was not touched; its current served bytes were not re-audited in batch2.

Limits: contended GPU environment, static desktop only, no fresh Three comparison. Preserve the slower unpaired WebGL diagnostic39.30→36.60FPS and final candidate24.75FPS row. No general active-game/VR speedup is proven. Remaining observed CPU wall scopes include render.encode.overlays and scene.gpuUpdates; no extra refactor. Next authorized action requires a separate batch3 request for fine motion/VR invalidation.

The completed batch1 state below remains historical; its raw evidence/checkpoint are preserved.

# Batch 1 — verified baseline

Status: COMPLETE; this local checkpoint on `codex/next-render-cycle-1` finishes batch 1. Batches 2–9 remain unstarted and require separate instructions. No push, main merge, deployment or shared-server restart.

Baseline source: GitHub main observed at task start, `b7de3be56ad2a8a79a30e7b4fe927fa6822e33c7`; the requested historical87447ea5 is7 commits older. Isolated checkout `/private/tmp/rust-next-render-cycle-1`. The primary checkout’s dirty work was preserved and is not the measured baseline.

Verified release: source `845a83b79156a8b6d30151f17629b07cc966d0dc152de58b6fed36726d8a0963`; WASM `b5cc8eba529f7c3990b1a4eacbc16c13786ac774b8a366ad15eba6943340987a`. Full bindings/SDK/tool hashes are in [MANIFEST.json](MANIFEST.json). All2461 frozen files rechecked;828 primary requested-file hashes verified. No renderer optimization or new release-WASM build.

Evidence:48 profiler-off primary runs, each8s warmup/25s sample;10 separate profiler captures;5 API counter retries;12 accepted worker replays/72 completed worker jobs. [Report with commands and results](REPORT.md), [derived results](RESULTS.json), [plan](PLAN.md), [journal](EXPERIMENTS.md). Final `npm run verify:push` PASS:419 native tests,137 browser scenarios, release/type/SDK/harness checks. Two native manual timing benchmarks intentionally ignored;42 separate performance helper units PASS. [Verification](VERIFICATION.json).

Receipt mismatch resolved: direct Node and npm serialize the same relevant environment keys in different insertion order. The current cache is order-sensitive; `npm exec --offline -- node scripts/verification-cache.mjs check all` matches the original PASS receipt. No tests/evidence rewritten or repeated; the cache implementation remains unchanged. Hashed environment/order evidence is archived.

Ordinary stationary/camera Rome mostly reaches the60Hz cap.1024-model load reveals expensive glTF queue preparation, movement updates and GPU main-pass intervals. Large stress drift and39–45% background aggregate GPU utilization prevent isolated capacity or small-gain claims. CPU-history freshness, API completeness and unavailable timings are explicit. No physical Quest or fresh Three comparison.

Deployment identity: initially8443 was unreachable from office LAN192.168.11.134. After returning home to192.168.0.17, the same PID12350 serves release `ba920dd931`, WASM `c7091506…`, verified over TLS against its unchanged manifest plus7 actual payload hashes/no-store headers. It is DIFFERENT from this baseline. Nothing was replaced or restarted.

Next: stop. On a separate batch2 request, reuse these frozen workloads and run fresh alternating A/B pairs under recorded conditions. Do not compare a later lone candidate against this drifting series. The full compressed evidence archive and its SHA256 are recorded in MANIFEST.json; exact captures and unfavorable attempts remain retained.
