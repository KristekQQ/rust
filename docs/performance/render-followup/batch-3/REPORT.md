# Followup3 — bitmap panel allocation work

Status: complete, local checkpoint only. WebGL2 reuse accepted for less resource churn; WebGPU automatic enablement rejected. No push, merge, deployment or shared-server change.

## Cause and minimal change

The actual warm Neon owner is the **Diagnostics bitmap panel**, not glTF draws, sword trails or a demonstrated leak. Its rows update at the existing250ms interval; when text changes, `Mesh::imported` used to replace both vertex and index GPU buffers. Source labels incorrectly suggested glTF. Fresh original25s traces found178WG/186GL creations; labelled control confirmed174WG creations for87updates and190GL for95updates. Different update counts reflect actual changing row values; this is not throttling. See [owners](OWNERS.md).

The new path keeps bounded COPY_DST buffers per live mutable panel, grows/shrinks capacity, writes current vertices and uses the actual index count. Index upload is skipped only if current indices exactly match a prefix of bytes previously uploaded to that same surviving buffer; replacement invalidates that proof. CPU index shadow is bounded by the live index capacity and removed with the mesh. Immutable Text retains the original exact-sized upload; no speculative global pool. Geometry generation, text cadence, alpha, effects, simulation and cuts are preserved.

**Backend decision:** WebGL reuse is the candidate default. WebGPU retains original mapped-init buffers by default: the first candidate reduced actual allocations but raised panel preparation cost. `setPanelBufferReuse(true)` is an explicit diagnostic experiment there, not a proven general optimization. The final measurement table must distinguish this opt-in from shipped default.

## Evidence boundaries

- Baseline base/checkpoint:`bcd774eb5761c201777c9b8892017e1be494b9af`, release WASM`d0677791488085cb26b7423a4c062fb4411044e90f55d958193558bd7c5f0798`.
- Initial experiment WASM`3b0bb9e4064c0358e257134e2e8d9466853bbe68c01353e266acb652715b3268`.
- Final safe-default WASM`bb3e3bf93d1483ef2ee010a5887008ecf8f48748fea90ee62d905a6c25869290`.
- Frozen runtime roots and actually served resource/harness hashes recorded by the maintained runner. No production byte match or headset deployment claimed.
- AppleM1/16GiB,macOS26.6.1,Chrome156.0.8078.48,Node24.22.1,Rust1.99.0. Headless with sandbox. Battery power, other browser/Codex processes; no isolated GPU/thermal clock guarantee.
-8s warmup/25s samples,3 alternating pairs per scene/API,1280x720,DPR1,profile-off verified. Calibration1..16 real bitmap panels remained60Hz limited. These are latency/allocation tests, **not throughput or higher-FPS proof**.
- Actual authored Neon text includes timings and is not a deterministic pixel oracle. Independent bitmap/cut fixture uses identical inputs and exact pixels without golden/tolerance changes. Real cuts in this fixture are synchronous native cuts; the full worker regression suite is separate.
- CPU callback/scopes are wall times, not pure CPU execution. Scope histories are sparse bounded observations, not complete invocation distributions. Clean GPU times unavailable; separate profiled queries report only documented pass/RAF command intervals. No mechanical CPU+GPU sum.

## Correctness

59exact bitmap states on WebGPU,72onWebGL, with foreground checks, grow/shrink/empty, same-size zero-create/one-write assertions,20repeated cycles,3clear/reload cycles, effects, and asymmetric simulatedXR eyes180×190/164×178 plus exit/reentry. The fixture measured stable same-size capacity147456B for109704B payload, then zero live buffers after clear. This is accounted GPU capacity, not physical residency. Other publication tests cover renderer/cutting/XR regressions; their fresh result is recorded separately.

V1 completed24clean runs/36033RAF rows:6event-image pairs exact;6Neon pairs equality unavailable. Event fixture3200→768logical creates for1600updates, but WG callback p99 worsened in all3pairs. Reject automatic WG enablement; do not hide this behind fewer allocations. GL scope and event tails improved or stayed similar. Final refined results below. The final matrix transitioned to30Hz for BOTH unchanged baseline and candidate while battery fell below20%; macOS lowpowermode0, exact browser/OS cause unisolated. Do not compare60HzV1 cadence directly against30Hzfinal.

Historical image issues A/B/C/D remain separate/open; these new exact checks do not close them. No physical Quest FPS/comfort/context-loss acceptance.

## Final paired results and decision

[Every run](RUNS.md), [machine-readable summary](RESULTS.json), [audit](AUDIT.json), [commands](COMMANDS.md), [identities](MANIFEST.json). Across V1 and final: **48 clean runs, 54,064 RAF rows**, 12 exact event-fixture image pairs independently rechecked as byte-identical PNGs. Twelve authored-Neon pairs have equality unavailable, not PASS. No new failed deterministic pair. Warmup8s/sample25s, three alternating pairs per workload/backend, profiling verified off. The final runs are around30Hz on BOTH builds; V1 around60Hz. These capped measurements cannot establish throughput gains.

| Final25s diagnostic, ordinary Neon | Original | Reuse | Meaning |
|---|---:|---:|---|
| WebGL2 actual createBuffer |186|0|Accepted default removes recurring replacement work.|
| WebGL2 dirty panel updates |93|92|Text depends on actual changing timing rows; cadence unchanged.|
| WebGL2 backend allocated storage |19,922,316B|0B|Does not include retained live storage.|
| WebGL2 current vertex writes |mapped initialization|92calls /16,901,856B|92 identical index uploads omitted.|
| WebGPU actual createBuffer |180|0|Experimental opt-in only; not delivered default.|
| WebGPU dirty panel updates |90|90|All updates retained.|
| WebGPU initial payload / vertex writes |21,240,072B|18,218,448B|Lower allocation/upload does not imply faster preparation.|

These are separate instrumented observations, not clean timing. Actual submitted commands/content unchanged within each API: GL49draws/57instances/17,275triangles per observed callback; WG48/56/17,274. Logical creates and API creates are corroborating layers, never added together. API initialization+first1s label inventories are retained separately in AUDIT; they are not a pure cold-initialization measurement.

Deterministic16-panel/cut fixture: GL original3200creates/1600updates versus768creates/1616updates over25s. The one-event boundary difference is retained, not hidden; all scheduled events run once. Required recreation/shrink/growth still allocates. Original created118,395,648B; candidate29,295,360B. Candidate wrote106,959,168B in1968calls and reused1072 unchanged index uploads. It does not pool removed panels indefinitely.

| Final event fixture, callback p95 / p99 / max (ms) | Original | Reuse |
|---|---|---|
| WebGL2 pair0 |2.9 /3.6 /4.1|2.7 /3.4 /3.6|
| WebGL2 pair1 |3.1 /3.6 /4.3|2.7 /3.2 /4.2|
| WebGL2 pair2 |2.9 /3.5 /3.9|3.0 /3.6 /4.3|
| WebGPU pair0 |2.7 /3.4 /3.7|3.5 /4.1 /4.8|
| WebGPU pair1 |2.9 /3.5 /5.2|3.5 /4.6 /5.5|
| WebGPU pair2 |2.8 /3.3 /3.9|3.3 /3.8 /4.7|

GL is accepted as a bounded reduction of repeated resource work, **not proven higher FPS or universally lower tails**. One ordinary GL candidate run had a9.2ms callback versus4.4ms baseline maximum; retained. V1 and final consistently show higher WG preparation/tails, so WG automatic enablement is rejected. Delivered WG still allocates original mapped-init panels. No effects or updates were removed to get zero creates.

## CPU / GPU diagnostics

Separate instrumented16-panel histories: median `panel.prepare` GL1.0→0.7ms, WG1.1→1.9ms. Sparse32-entry histories cannot represent all calls or total-frame savings. CPU bitmap geometry itself is unchanged. Commands originate outside engine RAF, so `panel.geometry` histories can be reset before export; a separate4s/8s diagnostic records512 actual text commands per variant,32 event callbacks. Mean command wall time WG0.04375→0.03926ms, GL0.03223→0.03789ms; quantized median0/p95≈0.1ms retained. This includes SDK/validation/native text geometry, not subsequent GPU upload, and is not clean benchmark evidence.

Clean GPU timings are unavailable. Separate valid async GL RAF queries (8samples) median1.543499→1.551166ms; WG pass samples(7/8) main0.287121→0.284246ms andFXAA0.536199→0.484909ms. Limited histories and nonisolated host prevent a GPU speedup claim; no CPU+GPU sums. API-observer allocation/callback overlap is recorded, but coincidence is not treated as causal proof.

## Memory and lifecycle

120s event soak per API, explicit reuse (experimental onWG): GL3602/WG3601frames, each7696text commands,61successful cuts,541spawns,663removals. In each of four30s windows, peak accounted panel capacity stayed2,359,296B, CPU index shadow192,528B,32live buffers. This is an engine-owned plateau, not physical GPU residency or JS-heap proof. Measurement arrays themselves intentionally retain observations.

A naive asynchronous capacity/payload-ratio assertion failed29GL/26WG samples; **the false results remain in AUDIT**. Every such sample had CPU payload0. `payloadBytes` describes current CPU geometry, `capacityBytes/liveBuffers` filter current scene IDs, while index shadow includes meshes awaiting next `finish_scene_updates`. Sampling between JS mutation and publication therefore mixes lifecycle phases. It cannot prove a per-publication bound. The dedicated exact fixture checks after-render bounds and clear, and an additional aligned20-cycle check per API passed100post-publication observations and20zero-capacity/zero-shadow clears per API, recorded in VERIFICATION. No production fence/readback was added. Gauges should be interpreted with these scopes; a future diagnostic revision may expose pending-cleanup bytes separately.

## Delivery boundaries

Source changes are confined to bitmap panel reuse/diagnostics and their SDK/harness/tests. Existing immutable geometry, instance pools, glow/HUD, new cut geometry and renderer pass order remain intact. Necessary allocation owners and reasons are in OWNERS. Independent review corrected missing zero-create assertions and skipped-index upload accounting before the final build.

Full fresh gate, exact source/pkg identity, archive integrity and local checkpoint are recorded in VERIFICATION/STATE. Retain all failed tooling attempts and adverse timings. Prior image problemsA/B/C/D stay open. No physical Quest, fresh Three comparison, CI, push, merge, deployment or shared-server restart. Followup4 has not started.

## Fresh verification result

`VERIFY_WASM_PROFILE=release npm run verify:push` exit0:429native passed(2ignored),153browser passed(57WG/96GL),SDK32,XRplatform71,harness14Node+13Python; format/clippy/WASM/types pass. Full browser started2026-10-29T14:30:13Z, finished14:42:15Z. Source dump generated. Matching-input receipt check PASS; final source/pkg/WASM/bindings match frozen final. The additional aligned lifecycle probe passed on both backends after this gate without source changes. Diagnostic scripts/evidence are archived; historical failure screenshots in the verification directory retain their provenance and are not new gate failures. See VERIFICATION.json for hashes/counts.

Evidence archive80,369,191bytes, SHA256 `7cd1df5db6a03aca79d5e082c8b0ecd37961ec73703fdee9132d7b9547b0e980`; all1124unique objects/2762aliases verified and restored to independent files in a new directory. Restored baseline/V1/finalWASM hashes match frozen manifests. See MANIFEST archive record; no historical report archive recursion.
