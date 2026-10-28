# Batch 4 — complete locally; strict stress-image limit open

Only batches1–4 are authorized. This containing checkpoint is on `codex/next-render-cycle-4`, isolated `/private/tmp/rust-next-render-cycle-4`, parent `8e03df21ddc26f1a36c41bc7a12293d3405b4b22`. No push/main merge/deployment/shared8443 restart; primary checkout foreign work/servers preserved. Stop before batch5.

Release source `f3e185262040df47bd7d3c5c7e2a12a06c3ba761cd935ed44ceb06df5f40b5bf`, WASM `c77e4b1e063052aa22f3935c7ab54c6b6fb3c16051add000a353c9d18f0f2a3d`. Final SDK differs only by type comment from measured SDK; equal canonical runtime hashes and both exact file hashes are retained. [Report](batch-4/REPORT.md), [manifest](batch-4/MANIFEST.json), [results](batch-4/RESULTS.json), [verification](batch-4/VERIFICATION.json), [design](batch-4/DESIGN.md).

One64MiB known-buffer/4096handle budget retains inactive owner-local pages/streams and standalone buffers. Active/pinned working set and one-frame pass grace are explicit exceptions; this is not a whole-VRAM ceiling. Generation/content guards and Rc leases keep prepared/in-flight ownership safe; no destroy/wait. Clear/final-asset removal releases maps/caches; generic partial-asset pages can remain bounded.

44clean8s/25s runs,53290RAF rows. Baseline21048 instance buffers/20904bind groups in measured windows; candidate0/0 after warmup. Known capacity plateaus213216B/2211456B.12separate25s GPU API pages confirm actual warm creation0; GL BG API unavailable. FPS approximately neutral and p99 mixed (largeGL maximum70.7→96.4ms retained). No proven general stutter/FPS gain. Cold/new-model/asset/private-uniform and cut-geometry creation are outside this pool.

Final gate423native/143browser plus SDK/harness/XR/types/strictWASM PASS;2existing manual timings ignored.35helper units PASS. Actual-eye/reentry, spawn/despawn, pressure/stream fallback, clear/reload2944B plateau/live0 and active dispose/reinit exact fixture pixels pass.4979frozen files,612served payload records and198driver-file identities verified. Archive27033390B/506entries SHA256`cab88de081124ab6cd1accdb33fb678639b641bec33121307926ee6794df3b14`; every payload/hardlink read back.

**Strict primary pixels19/20exact,1FAIL**,2liveNeon pairs ungated. Moving1024WG failure8pixels,max2. Extra baseline temporal instability observed, but failing candidate SHA not reproduced in this baseline sample. Candidate independent oracle1/3exact; other2 differ4pixels,max1. Causality/frequency and universal stress parity remain unresolved; project gate PASS does not change this FAIL. No physicalQuest/freshThree/CI evidence.

Next: stop. Batch5 requires a separate request. Previous batch reports remain under [batch3](batch-3/REPORT.md), [batch2](batch-2/REPORT.md), and [batch1](REPORT.md); do not reuse historical timing/device claims as current evidence.
