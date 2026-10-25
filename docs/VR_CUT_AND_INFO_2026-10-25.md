# VR cut scheduling and shared information panels

## Scope and diagnosis

Owned checkout: `/Users/Honza/.codex/worktrees/rim-publish/rust`, branch
`codex/core-render-optimization`, base `088fdb34` plus preserved local follow-ups.
Primary source checkout and the Three.js reference server were preserved.

Rome's cut transport scheduled preparation exclusively with document RAF. An
immersive runtime can stop that clock while XRSession frames continue. Native
phase 3 intentionally waits for queued/pending/staged cuts, so a stopped transport
also prevents the next wave. The previous simulator scenario left document RAF
running and therefore did not cover this failure mode. This is a source diagnosis;
it does not prove the precise cause of the user's earlier physical Quest run.

## Changes

- `WebXRHost` emits a platform `frame` notification after native stereo completion.
  Rome transfers at most one bounded job per XR frame; desktop RAF is cancelled
  during XR and restarted after exit. Geometry slicing remains Rust/WASM in its
  worker. Native simulation/publication remains before both eyes on a later frame.
- Transport slots are correlated by token, expire after 10 seconds and ignore late
  replies. Expiry cannot release another job's slot. Disposal removes listeners.
- Native cut inactivity tracks active real time, independent of slow simulation.
  Preparation/results/staging/new GPU acknowledgements count as progress. Ten
  seconds without progress cancels only that enemy generation, cleans hidden
  geometry and retires its death, allowing subsequent waves. Actual worker errors
  still use the existing global cut-failure cleanup. Successful effects retain
  their original separation/dissolve timing.
- Detailed profiling now defaults off in every example; native FPS/counters remain
  available. `profile=1` opts in. Desktop statistics require `infoDesktop=1`;
  no periodic JS statistics snapshots run by default or during XR. Rome's hidden
  DOM HUD also skips full game-state snapshots during XR.
- All 22 engine examples share the native diagnostics panel and player-floor
  placement, adopting their authored panel where present. One node is reused
  across hide/show/exit/reentry. Other equipment owns its head slot until explicitly
  released. Benchmark completion explicitly restores diagnostics. The Neon Arena
  gravestone and inscription were removed.

## Evidence and limits

Focused native Rome tests: 25 PASS, including six actual worker-geometry results
followed by the next tank wave, lost worker/upload readiness, hidden-mesh cleanup,
and continuously slowed simulation. Transport plus XR-platform unit tests: 15
PASS. SDK: 30 PASS. Typecheck, strict wasm32 Clippy and optimized release build PASS.
Astra performed a scoped independent source review; the slow-time timeout finding
was fixed and re-reviewed.

The initial browser pass retained two test defects (overly broad global RAF-call
assertion and fixture teardown bypassing authored pagehide cleanup). Neither is
reported as product acceptance. Final packaged browser/deployment evidence is
recorded in `output/vr-followup-2026-10-25/` after completion.

The new simulator regression suppresses document RAF while XRSession frames run;
a negative control omits the XR hook. The per-example matrix loads each actual
example HTML/bootstrap and verifies native placement, positive scale, both-eye
panel pixels and lifecycle. Desktop backend checks and simulated XR remain
separate from physical Quest acceptance.

Old physical Quest observations show Neon Arena WebGL cadence around 76–82 FPS
in later windows and WebGPU around 64–66 FPS at resolutionScale .75 / requested
90 Hz. Gameplay, head motion and warmup are not locked, so these are observations,
not a matched backend benchmark, a GPU bottleneck diagnosis or new-build evidence.
No universal backend speed claim, physical-headset acceptance, commit, push or CI
result is established by this follow-up.

## Final delivery

Application `8657bc564b`, release WASM
`1b07f8af648e0eab504da16dd5a1c3a541027fbcaadbfb9db3370486962b42ed`
is served by the existing official Quest production server on HTTPS 8443.
The final production-root run passed **38/38 cases** with stable input identities:
all 22 actual example bootstraps with stereo native panel assertions, shared-panel
lifecycle, real Arena button entry, authored XR gameplay scenes, both desktop
Rome/Arena backends, direct WebGPU XR, and suspended-document-RAF Rome positive
and negative controls. Native Rome wave tests independently verify all six cuts
followed by tank spawn; the browser tracked-blade test verifies one actual worker
cut through visible separation and evaporation, not the complete six-kill wave.

All 218 live raw payloads (including manifest) matched the accepted package with
no-store headers; all 22 scene routes matched. Previous `d51271cfc9` is preserved
under the task output directory. The Three.js reference on 8878 is unchanged.
No source edits followed acceptance; subsequent edits were report/coordination
only. Detailed evidence: `final-browser-report.json`, `delivery.json`, positive
and negative Rome reports and stereo/menu images in the task output directory.
The initial matrix run was interrupted after its fixture and polling issues;
`matrix-initial-observations.json` is explicitly observations, not a PASS receipt.

Remaining limits: headset FPS/hitch improvements need the user's physical Quest
check; initial blade/model-snapshot work and general backend GPU costs were not
eliminated or newly benchmarked. The cut worker computes geometry; fragment motion
remains native deterministic main-instance motion, not a new physics worker.
No commit, push or CI run was requested/performed in this follow-up.
