# Followup4 — complete, local checkpoint only

Only batch4 was authorized. Checkout `/private/tmp/rust-render-followup-2`, branch `codex/render-followup-4`, base `237279b6a39d7843fa47258faf9d09ae6dfb34d3`. [Report](batch-4/REPORT.md), [three-pair results](batch-4/RESULTS.md), [manifest](batch-4/MANIFEST.json), [verification](batch-4/VERIFICATION.json), [commands](batch-4/COMMANDS.md).

Union remains the default on BOTH APIs. Explicit bounded coalesced/full-active experiments, exact-byte diagnostic counters and regression fixtures are retained; no new production FPS gain claimed. Sparse2048 traffic1,081,616→1,072B/RAF costs1→67writes. All three long WebGL2 sparse FPS pairs regress; WebGPU quiet gains are small/mixed and do not improve every tail/workload. No magic threshold or new mapping/pool introduced. Original→probe-union controls have mixed signs, so zero instrumentation overhead is not proven.

218 completed runs/208768RAF (198 profiler-off runs including40short pilots);102 exact independently checked deterministic PNG pairs,0new failure,6authored-active equality-unavailable. Two partial pool sides/6333RAF excluded from completed-campaign counts. Preclosure, home and control campaigns remain separate. Detailed20run-side frame context is exact; clean sparse profiler-frame alignment is unavailable, not invented. No clean GPU timing; diagnostic queries separate.

Twelve actual worker-cut simulated-XR replays pass; three publication events per policy/API are not a latency-gain proof. Both120s pool soaks keep622512B accounted live instance capacity,0warm buffer/bind-group creates/evictions; range scratch32B WG/512B GL, explicit clear→0 verified. Driver residency and physical Quest unverified.

Release WASM `7829658f7705608ce2e4b445b36c375c010bec516f000f778414e8b61a9c0d63`, source `a4c47fa969c154620f71df649b3ca9ecc80a3c0d2ced9799720263e8972f4118`, bindings `62a99f26d857b4330be39924125fc98ea2ab016481d8c8a71c2b532d97eb00ea`. Probe/final executable behavior matches; final SDK has a JSDoc-only correction, both byte identities recorded. Final gate431native(2ignored)/163browser(62WG,101GL),SDK32,XR71,harness14Node+13Python PASS; targeted54performance helper tests PASS. Gate attempt1 rejected JSDoc; corrected attempt2 and current receipt pass. No changed tolerance/golden.

Prior A/B/C/D image cases remain separately open. No push/merge/deploy/shared-server restart/CI/Quest/freshThree. Batch5 NOT started. Next authorized batch must reread this state; possible exact payload sharing is only an unimplemented hypothesis.

## Historical followup3 — complete, local checkpoint only

2026-10-29. Isolated `/private/tmp/rust-render-followup-2`, branch `codex/render-followup-3`, base `bcd774eb5761c201777c9b8892017e1be494b9af`. This checkpoint contains only owned batch3 work. [Report](batch-3/REPORT.md), [owners](batch-3/OWNERS.md), [runs](batch-3/RUNS.md), [manifest](batch-3/MANIFEST.json), [verification](batch-3/VERIFICATION.json), [commands](batch-3/COMMANDS.md).

Proven warm allocation owner: changing Diagnostics bitmap panel, not glTF/leak. Per-live mutable panel COPY_DST capacity plus exact unchanged index-prefix reuse; immutable Text remains exactly-sized original. **WebGL2 default accepted:186→0actual createBuffer/25s**, same current geometry/cadence/content. **WebGPU default stays original:** explicit reuse reduced allocations but worsened panel preparation/event callback tails. No FPS gain or global perfect optimization claimed.

Final release WASM `bb3e3bf93d1483ef2ee010a5887008ecf8f48748fea90ee62d905a6c25869290`; source `d44d29f28fd00eaf08f2551fe4c2673059dad1211e8ba39381741b3c517fabdc`; bindings `33f7c4ef0074300d54b7b2b64478efb764d400cc01ee28084e37a0cbc7e93d64`. Frozen/runtime/current source/SDK/pkg agree; not a production-serving claim.

48clean runs/54064RAF rows,12exact independently checked PNGpairs/12authored-Neon equality-unavailable/0new deterministic failure. Final both builds30Hz vs V1both60Hz during battery change; nonisolated host. GLtails mixed, one9.2ms outlier retained. Two120s eventsoaks keep same peak2,359,296B logical GPU capacity and192,528B CPUshadow each30swindow. Arbitrary samples can mix mutation/publication phases; failed ratio checks preserved. Additional aligned20cycles/API pass100checks/20zero-storage clears. Actual driver residency unavailable.

Fresh full gate:429native(2ignored),153browser(57WG/96GL),SDK32,XR71,harness14Node+13Python PASS; matching receipt PASS. Focused exact bitmap59WG/72GLstates includes asymmetric simulatedXR. PriorA/B/C/D image issues remain separate/open. No push/merge/deploy/sharedrestart/CI/physicalQuest/freshThree. Batch3 stops; batch4 requires separate authorization.

## Prior batch2 checkpoint (historical evidence)

2026-10-29. Isolated `codex/render-followup-2`, base `0b7af5b75`. [Report](batch-2/REPORT.md), [commands](batch-2/COMMANDS.md), [manifest](batch-2/MANIFEST.json), [verification](batch-2/VERIFICATION.json). Original B8/f14c4beb preserved.

Proven redundant opaque primitive scans; diagnostic immutable guard removes them (WG2048 130287→0 primitive visits/call). Automatic default rejected on BOTH APIs: lower CPU wall time did not consistently improve whole-frame stress cadence/tails. Large dynamic WG workload improved in these runs, but has a strict image failure. Original selection remains default; `setOverlaySelectionReference(false)` is experimental opt-in only. No production FPS gain claimed.

Final release WASM `d0677791488085cb26b7423a4c062fb4411044e90f55d958193558bd7c5f0798`, source `7cd160e788ece6dfd78b6d913957ba758cb19dc8ad4abae0d52c6114d263ca4f`, bindings `504f0f506ebe4e1839bf3dfc4b169109b35b8ba8c490fdf1c99f5665827080fe`. Current isolated pkg/source/SDK/tests match. One fresh full gate:428native(2ignored),151browser(56WG/95GL),SDK32,XR71,harness14Node+13Python PASS. New exact overlay states21WG/25GL plus per-eye BLEND/asymmetric simulatedXR PASS. Full-cut regressions cover delivered default; no complete worker-cut/fade/dissolve pixel oracle for experimental opt-in.

90clean runs/102137RAF rows, three alternating pairs/config:32exact/1FAIL/12unavailable. Six original→restored-default controls exact, small mixed timing differences. Profiling verified off; clean GPU times unavailable. Nonisolated desktop, no physical Quest. CaseD clustered1024WG:four pixels/max1; both variants also occur in original runtime. Mechanism unresolved, FAIL retained independently of A/B/C. No tolerance/golden change.

Local checkpoint/evidence only. No push/merge/deploy/shared restart. Historical batch2 stopped before the separately authorized batch3. Prior batch1 [report](REPORT.md): A unresolved/not reproduced, B and C separate/open.
