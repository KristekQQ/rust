# Rome headset diagnostics and refresh-independent blade contact

Owned isolated checkout: `rim-publish/rust`, branch
`codex/core-render-optimization`, base `088fdb34` plus preserved follow-ups.
The primary source checkout, other 21 examples and Three.js reference were preserved.

## Reproduced defect

The contact eligibility required both `travel > .018` per frame and speed > .3 m/s.
The first condition imposed minimum speeds of 1.296/1.62/2.16 m/s at 72/90/120 Hz.
A deterministic 1.5 m/s physical trajectory passed at 72 Hz but failed at 90 Hz
(HP stayed 100 instead of 50). Eligibility now uses physical speed > .3 m/s.
Existing continuity, contact latch, cooldown and swept collision remain intact.
The same trajectories now hit and queue death slicing at all three rates.
This proves a source defect; it does not prove it caused the user's specific
previous headset incident, for which Rome had no device logs.

## Runtime diagnostics

Rome now records an early bootstrap identity, initialization/global errors, XR
lifecycle/cadence and bounded native state snapshots to `/__diagnostics`.
Snapshots run from actual XR frames at most once per second, with a bounded
session sample budget. They include build/WASM identity, actual backend, enemy
HP/phase, hit/contact samples, discontinuities, cut queue/staging/publication/error
counts and worker readiness/error/timing counters. Actual viewport sizes and
supported rates are sanitized. Poses, meshes and projectile arrays are omitted.
The native state is still serialized once per snapshot; this is bounded overhead,
not zero cost. Fetch failures cannot interrupt rendering. No profiler is enabled
by this module. Stored records live in `output/diagnostics/browser.jsonl`.

Worker startup readiness, startup failures, message failures and token slots are
observable. This does not add automatic restart after failed WASM initialization;
existing failure cleanup remains. ContactSamples/slowSamples count enemy-hand
samples, not a percentage of rendered frames. maxSpeedForFrame preserves the
fastest sampled hand instead of letting a stationary hand mask it.

## Acceptance

- Native Rome: 26 PASS including the failing-before/passing-after 72/90/120 Hz
  physical trajectories, real geometry transactions and next-wave progression.
- Logger/transport units: 7 PASS; strict wasm32 Clippy, release WASM, typecheck and
  diff checks PASS. Astra independently identified/reviewed the eligibility defect
  and diagnostic naming issues; the latter were corrected.
- Production acceptance covers 10 cases: actual Rome bootstrap/entry/slow90Hz
  strikes/worker/log POST on both backends, tracked gameplay at90/120Hz on both
  backends, desktop Rome on both, suspended-document-RAF negative control and
  direct WebGPU XR platform coverage.
- The first 120Hz fixture reused a fixed 15-frame walking duration and fixed
  .15m-per-frame gesture. Those are different physical motions at120Hz. Walking
  duration and gesture speed were corrected; all four affected90/120Hz cases were
  repeated. Six unrelated passing cases were retained against the identical
  production payload. `final-acceptance.json` names both evidence sources.
- Each opposing group of actual fragment meshes is independently hidden/restored
  at dt0. Both actual eye targets change by thousands of pixels for each group,
  proving visible halves rather than unrelated enemy/camera motion. GPU and GL
  paths use real rendering; only platform poses/session/projection handles are
  simulated. Visual stages were inspected.
- The actual production bootstrap accepted two1.5m/s strikes at90Hz, reached HP0,
  published18 fragment meshes with one commit and zero errors. Each backend's
  production test log was found on disk by runId with its build and cut counters;
  these are headless records, not physical Quest records.

## Delivery

Application **7c37709d6c**, release WASM
`c4301d8142f89cda9472095b128a458372101d646a7b9ed1980e84a5569b526a`
is served on the existing official HTTPS8443 production server. All219 raw files
(including manifest) matched accepted bytes with no-store headers. All22 examples
remain. Previous8657bc564b is preserved under the task output directory.
Evidence: `output/rim-device-cut-2026-10-25/` (regression, first/final affected
browser reports, consolidated acceptance, persisted log proof and delivery).

The user's tab was not manipulated after its earlier security-policy denial;
manual reopening is required to run the new client and logging. Physical Quest
performance, the exact prior incident and complete visual parity with Three.js
remain unverified. No commit, push or CI run was requested/performed here.
