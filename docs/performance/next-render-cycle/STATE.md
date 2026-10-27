# Batch 1 — verified baseline

Status: COMPLETE; this local checkpoint on `codex/next-render-cycle-1` finishes batch 1. Batches 2–9 remain unstarted and require separate instructions. No push, main merge, deployment or shared-server restart.

Baseline source: GitHub main observed at task start, `b7de3be56ad2a8a79a30e7b4fe927fa6822e33c7`; the requested historical87447ea5 is7 commits older. Isolated checkout `/private/tmp/rust-next-render-cycle-1`. The primary checkout’s dirty work was preserved and is not the measured baseline.

Verified release: source `845a83b79156a8b6d30151f17629b07cc966d0dc152de58b6fed36726d8a0963`; WASM `b5cc8eba529f7c3990b1a4eacbc16c13786ac774b8a366ad15eba6943340987a`. Full bindings/SDK/tool hashes are in [MANIFEST.json](MANIFEST.json). All2461 frozen files rechecked;828 primary requested-file hashes verified. No renderer optimization or new release-WASM build.

Evidence:48 profiler-off primary runs, each8s warmup/25s sample;10 separate profiler captures;5 API counter retries;12 accepted worker replays/72 completed worker jobs. [Report with commands and results](REPORT.md), [derived results](RESULTS.json), [plan](PLAN.md), [journal](EXPERIMENTS.md). Final `npm run verify:push` PASS:419 native tests,137 browser scenarios, release/type/SDK/harness checks. Two native manual timing benchmarks intentionally ignored;42 separate performance helper units PASS. [Verification](VERIFICATION.json).

Receipt mismatch resolved: direct Node and npm serialize the same relevant environment keys in different insertion order. The current cache is order-sensitive; `npm exec --offline -- node scripts/verification-cache.mjs check all` matches the original PASS receipt. No tests/evidence rewritten or repeated; the cache implementation remains unchanged. Hashed environment/order evidence is archived.

Ordinary stationary/camera Rome mostly reaches the60Hz cap.1024-model load reveals expensive glTF queue preparation, movement updates and GPU main-pass intervals. Large stress drift and39–45% background aggregate GPU utilization prevent isolated capacity or small-gain claims. CPU-history freshness, API completeness and unavailable timings are explicit. No physical Quest or fresh Three comparison.

Deployment identity: initially8443 was unreachable from office LAN192.168.11.134. After returning home to192.168.0.17, the same PID12350 serves release `ba920dd931`, WASM `c7091506…`, verified over TLS against its unchanged manifest plus7 actual payload hashes/no-store headers. It is DIFFERENT from this baseline. Nothing was replaced or restarted.

Next: stop. On a separate batch2 request, reuse these frozen workloads and run fresh alternating A/B pairs under recorded conditions. Do not compare a later lone candidate against this drifting series. The full compressed evidence archive and its SHA256 are recorded in MANIFEST.json; exact captures and unfavorable attempts remain retained.
