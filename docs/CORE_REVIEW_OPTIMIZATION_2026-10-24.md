# Review follow-up: avoid unused work and bound cut installation

Current continuation: [remaining renderer review](REMAINING_RENDER_REVIEW_2026-10-24.md) implements and measures rigid instancing, primitive culling and UV-interface specialization. Build identities and delivery below describe this earlier milestone.

Local follow-up to `088fdb34` on `codex/core-render-optimization`. The implementation is uncommitted. No main integration or new push is implied. [Versioned measurement samples and build identity](benchmarks/review-followup-2026-10-24.json) allow the event statistics to be recomputed without ignored local artifacts.

## Accepted changes

1. WebGL packed depth is allocated and written only when an active postprocess effect reads depth. FXAA/bloom-only targets have one color attachment. SSR/environment/god rays restore packed depth as needed. Real depth testing and the deferred transparency/rings/glow path remain intact. A non-attachment fallback texture satisfies unused depth bindings without a feedback loop.
2. Cut preparation is capped at 3500 triangles, including inside a single large primitive. Only requested vertices are deformed. Result decoding uses `[f32; 9]`, eliminating the per-vertex heap allocation.
3. Worker acceptance validates framing and enqueues data. Native game updates install at most 9000 vertices per tick into hidden meshes. Renderer synchronization uploads those meshes; the web bridge explicitly acknowledges actual GPU handles. All pieces become visible and begin physics/dissolve in one later game tick. Neither XR eye publishes independently. Reset/error/stale-owner paths remove staged resources.
4. Static and dynamic shadow-caster revisions are separate from general scene revisions. Noncaster motion no longer prepares or redraws occupied maps. Caster motion/pose/material changes, addition/removal and category transitions invalidate the applicable layer; light matrix changes still invalidate. Existing global static revisions retain their other purposes.
5. One opaque glTF queue per pass retains binding state across models while preserving original draw order. Transparent sorting remains separate. Optional diagnostics identify submitted object/node/primitive/view/pass; ordinary execution does not collect this list. Actual glTF `queue.write_buffer` calls are now counted separately from bytes.
6. Cut benchmarks measure worker acceptance, preparation, native staging and stereo frame work separately. The scripted gesture can run paced for timing; the maintained correctness scenario remains deterministic and manually driven.

No resolution, PCF kernel, texture quality, enemy count, geometry detail or PBR feature was reduced.

## Measurements and limits

Apple M1, hardware ANGLE Metal, Chrome 155.0.8059.85. Ordinary benchmark: 960×640/DPR1, two-second warmup, five-second sample, approximately60Hz RAF cap. Cut benchmark: three fresh WebGL simulated-XR pages per version, same tracked fatal-swing scenario. The paced benchmark drains the initial queued simulator burst and waits14ms between gesture frames. These are browser measurements, not Quest acceptance.

| Cut metric | Reference | Accepted follow-up |
|---|---:|---:|
| Worker acceptance maximum | 21.60 ms | 0.20 ms |
| Worker acceptance mean | 4.39 ms /15 calls | 0.072 ms /18 calls |
| Prepare-job maximum | 1.40 ms | 1.90 ms |
| Post-gesture staged-frame CPU maximum | No comparable samples: old result was already installed | 6.60 ms |
| Native staging maximum, observed post-gesture | Not separately instrumented | 3.20 ms |
| Completion from fatal hit, mean | 120.53 ms | 191.83 ms |
| Whole gesture/frame maximum | 18.20 ms | 18.10 ms |

The substantial result is removal of synchronous installation from worker delivery. Installation is spread across frames and completion is approximately71ms later in this scenario. Subset deformation can do more repeated vertex work than a full primitive deformation on these small primitives, explaining why preparation is bounded but not faster in this fixture. Initial gesture-frame peaks around16–18ms remain; this does not establish removal of every hitch. Small sample counts do not establish reliable long-run p99 statistics.

Normal Rome CPU time did not show a reliable additional improvement: baseline repeats averaged1.66–1.95ms on WebGPU and3.90–4.06ms on WebGL2; accepted repeats measured1.71–1.88ms and3.97–4.15ms respectively. RAF stayed near60FPS. This follow-up targets unused attachments, invalidations and task spikes, rather than claiming another large average-frame gain.

The fixed FXAA reference is bit-identical before/after on both backends. At960×640, WebGL eliminates one full-resolution RGBA8 target (2,457,600 bytes of logical texture storage) and its fragment output. GPU milliseconds for this WebGL saving were not established. No asynchronous pass timestamps are summed into a whole-frame duration.

Opaque pipeline transitions per submitted glTF draw fell from approximately0.0198 to0.0054. Submitted draw count remains unchanged. A paused diagnostic snapshot submitted441 glTF draws, including six orbiter models at67 primitives each. The asset has no skins,42 nodes and eight animations. This identifies a concrete rigid-instancing candidate; it does not explain the earlier Three.js draw-count difference by itself. Primitive culling and true instancing were not introduced in this follow-up.

## Rejected uniform batching experiment

A per-model aligned uniform arena reduced approximately400 small writes/frame to about7, while padding increased normal Rome uploads from roughly98KB/frame to107KB/frame. The two experimental runs did not improve CPU frame time (WebGPU approximately2.01–2.08ms; WebGL2 approximately4.08–4.23ms). It was reverted. The accepted WASM is byte-identical to the previously tested queue-only version:

`12c1d5e141b9427b5c42e7cb5ee4f07ebf7b2a1f76250463f815b108f284be53`

The experiment's source/package and raw reports remain under ignored `output/review-optimization/uniform-arena-experiment/` and `output/core-optimization/review-final*`. Write-call diagnostics remain for future designs. No further shader attribute/TBN simplification was made: the nonneutral fallback normal and compiler elimination need an independent pixel/performance study.

## Verification

- Native library:348 passed, two ignored. Includes strict large-primitive batching, exact subset deformation, atomic publication, GPU-ack gating, stale/error/reset cleanup and caster-category revisions.
- Release WASM build, wasm32 Clippy with warnings denied, formatting and type checks passed. SDK/harness fast checks passed.
- Integrated browser coverage:28 applicable scenarios passed across WebGPU/WebGL2, including FXAA/SSR toggles/resize, occupied and empty shadow caches, glTF GPU/CPU pixels, negative/nonuniform transforms, material/lifecycle paths, cutting, effects, render targets, overlay composition and simulated XR. A temporary instrumentation-only failure on frozen SDK handles was corrected with a transport adapter. Final affected checks passed after restoring the accepted WASM and updating timing instrumentation.
- Independent source review covered the shadow/postprocess/cut lifecycle changes. A further narrow review covered the later-rejected uniform arena.
- Production-root checks and LAN served-byte checks are recorded at delivery below. Real Quest comfort/performance and CI have not been measured for this follow-up.

## Reproduction

`node scripts/benchmark-core.mjs <label> <pkg-directory>` with `SCENES=fxaa,rim` and optional `DRAW_DIAGNOSTICS=1`. Timing and pixels are collected before enabling the expensive diagnostic overlay.

`node scripts/benchmark-rim-cuts.mjs <label> <pkg-directory>` runs three sequential WebGL scenarios and stores samples, browser/GPU information and served-file hashes. A sibling `source/` directory supplies the frozen baseline SDK/example modules; tests always use the same current instrumentation. Intentional aborted background decoration loads at dispose are recorded separately, matching the maintained browser runner.

Raw artifacts live under `output/review-optimization/` and `output/core-optimization/review-*`. The reference package is `output/review-optimization/baseline/pkg`. Do not run benchmarks concurrently with other render tests/builds or equate the simulator's manual callback timing with headset cadence.

## Delivery

The accepted release/minified production package is served on HTTPS192.168.0.17:8443. Production-root `rim` passed on WebGPU and WebGL2; `rim-gameplay` passed on WebGL2. LAN-served WASM returned HTTP200 and SHA-256 matched the accepted12c1d5e1 build above. Refresh the page to load it. The Three.js server on8878 is preserved. No commit/push for this follow-up; physical Quest acceptance remains with the player.
