# Batch 3 — dependency-scoped opaque preparation

Parent: completed batch 2 checkpoint `1afcd015787a3bff3fb58afd54126a6c7728a512`. Only batch 3 is authorized. No push, merge, deployment or shared-server changes. See [report](REPORT.md) for measurements and [experiment journal](../EXPERIMENTS.md) for rejected attempts.

## A. Immutable compatibility

Uploaded assets intern bounded compatibility-class tokens for primitive ordinal, material/geometry ownership, mirror state and render options. Each draw re-interns only when these dependencies change. A shared token proves the original immutable comparisons; it never grants additional reordering permission. The map is limited to 4,096 entries per asset with weak ownership; unsupported or nonfinite keys use the original comparison.

## B. Dynamic data

Existing per-draw `last_uniform` is the canonical world-data source. Conservative data and basis revisions change with actual rigid-uniform publication. Model publications have a globally ordered renderer-local epoch, which never resets on clear/cache toggle. Updating one model no longer globally drops all prepared plans. Batch 2's exact unchanged-input cache additionally checks individual model data revisions.

Fine groups retain ordered draw identities, resource generations, data/basis revisions and shared prepared rows. Matching groups patch only changed records into their own view/pass CPU stream mirror. Compact records read 16 bytes directly from the Uniform; 112-byte records are constructed only when the actual dirty payload needs them. There is no duplicate full record stored on every draw. A reduced basis-equality proof preserves the literal old affine predicate, IEEE equality and nonfinite behavior; independent native oracle tests cover 20,000 seeded IEEE cases.

The existing stream gives group indices power-of-two byte reservations. Small count changes therefore do not shift every later group. Actual bindings still use current live bytes; 16B/112B transitions, storage growth and offset changes refresh affected records. Empty reservations survive 2→1→2 while other instanced groups remain. Padding counts toward the existing 16 MiB stream admission. This is layout slack, not a new GPU resource pool.

One union dirty span limits upload calls; it can include large unchanged gaps. CPU bytes copied and actual GPU bytes uploaded are reported separately. Dirty full records may be packed again for another changed view. No universal upload reduction or zero dynamic work is claimed.

## C. Fresh view selection

Every exact-cache miss performs the original root/primitive culling, depth grouping, overlap tests and order barriers. Camera movement never rounds matrices or accepts an old visibility list. Unchanged selected membership may reuse prepared rows and world payloads after that fresh selection. Transparent objects retain their original ordered builder.

There are 64 fixed fine view/pass slots and a 16 MiB accounted element-storage limit, separate from the exact cache's 16 MiB limit. No matrix-key map grows with camera motion. Actual content epochs, storage generations, offsets, widths, material uniform contents and pipeline variants guard row reuse between eyes, shadow/reflection passes and the main pass. Removal releases groups owning removed generations; clear/dispose releases relevant ownership. Model Uniforms, class maps, allocator/Rc overhead and existing CPU/GPU stream copies are outside these cache-element accounting limits.

## Measured admission

Views without an instanced group use the original scratch builder. A moving view also falls back when at least 8 currently contributing model roots changed, at least 512 items are currently selected, and those changed roots own at least 75% of selected items. The decision uses only current nonempty model ranges and the fixed per-view publication mark. A single heavy visible or offscreen root cannot trigger it. Conservative no-op pose publications can cause unnecessary fallback; image correctness does not depend on this heuristic.

The thresholds separate observed adverse regimes; they are not universally optimal. Original-builder work remains visible in ordinary preparation counters. Fine counters alone must not imply zero work after fallback.

## Independent verification

Cache-on/culling-on exact pixels are compared with cache-off/culling-off, using the original builder and hardware clipping as an independent visibility reference. Tests inspect first-mutation-frame counters before capture warmups. Coverage includes camera motion, one moving root, animation/morphs, visibility edges, clustering, spawn/removal, ordered barriers, transparency, nonuniform/negative scale, 16B↔112B, lifecycle and asymmetric actual eye attachments. Heavy-root regressions assert that static rows survive both visible and offscreen unrelated updates. A small moving fleet separately verifies admission and actual fallback work. Maintained simulated-XR gameplay replays publish real worker cuts.

Matched baseline/candidate timing uses frozen completed-batch-2 and batch-3 release packages, unchanged quality/content, static/camera/mixed/all-moving and small/native controls on both backends. Profiler-off primary runs and separate profiled/API/worker diagnostics remain distinct. Desktop and simulated XR do not establish physical Quest performance. Stop before batch 4.
