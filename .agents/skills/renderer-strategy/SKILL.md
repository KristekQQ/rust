---
name: renderer-strategy
description: Design reusable renderer architecture and scene-preparation policies for this Rust/WASM engine across games, mixed static/dynamic content and desktop/VR. Use for general batching, lifetime, invalidation and cross-renderer optimization decisions.
---

# General renderer strategy

Honza's central project ambition is a world-leading, reusable renderer and engine,
not a renderer tuned only for one example. Treat that ambition as a direction to
prove through transferable improvements at matched quality, not a claim of
universal superiority. Optimize frame latency/tails, CPU/GPU work, memory and
loading/preparation cost together; minimum draw calls alone is not success.

Follow [AGENTS.md](../../../AGENTS.md). Rust owns scene analysis, simulation,
calculations and rendering; the JS SDK expresses commands and semantic intent.
This skill guides architecture, not permission to implement, deploy or push.
Keep discussion-only requests as discussion.

## Prepare from properties, not game names

Inspect the current scene revisions, transform/bounds caches, instancing streams,
pass/view caches and frame graph before adding another planning/cache layer.
Extend existing ownership and invalidation rather than duplicating them.

Infer facts from assets and render state: shared geometry/material identity,
alpha mode, skin/morph use, vertex layout and pipeline requirements. Get semantic
intent such as mobility, destructibility and gameplay lifetime from the scene/API;
do not guess that an object will never change because several frames were stable.
Use independent properties rather than one global static/dynamic switch:

| Change pattern | Reusable preparation policy |
|---|---|
| Stable geometry, material and transform | Retain shared resources and eligible prepared data; use spatially bounded groups when merging unique static geometry. |
| Stable geometry, moving rigid transforms | Retain geometry/material groups; update changed transforms, normal data and bounds. |
| Partially changing or rarely edited content | Track transform, pose, topology, material, visibility, caster/light and lifetime dependencies separately; rebuild the affected portion. |
| Skinning, morphing or deformation | Select supported GPU paths while preserving animation, bounds and CPU query/cutting semantics; retain a correct fallback. |

Transparency/order dependence, shadow participation and pass/view compatibility
are separate axes. A static object can be transparent; a moving object can share
immutable geometry. Static shadows depend on relevant light/caster revisions, not
just a scene label. A change to a static-marked object must still invalidate its
dependents correctly.

## Build and maintain the render plan

Separate asset preparation, change-driven plan updates and view-dependent frame
work. Persistent groups describe compatible resources; each view/pass selects its
visible instance stream. Avoid rebuilding the entire scene or re-uploading stable
buffers for one changed object. Budget/prewarm expensive preparation when useful;
include its cost and retained memory in the experiment rather than hiding it.

- Batch eligible visible opaque parts across object roots by shared geometry,
  material, pipeline/options and other actual compatibility constraints. Preserve
  necessary mirrored-transform/deformation distinctions. Root AABB overlap alone
  is not a universal ban on opaque instancing; coplanar/order-sensitive cases need
  a demonstrated safe policy or fallback. Do not blindly relax every barrier.
- Keep culling and batching complementary. Compact visible instances where it
  pays; bound static merges spatially so useful culling is retained. Compare
  larger batches against extra submitted geometry, not draw count alone.
- Preserve transparent sorting, depth/clipping, material variants and auxiliary
  passes. Do not silently remove effects, reduce geometry, change resolution or
  alter gameplay to obtain a faster result. Expose deliberate quality options.
- Preserve object IDs, collision geometry and independent gameplay lifetimes even
  when rendering shares batches. Spawn, removal, cuts and material/pose changes
  invalidate their dependencies; publish coherent render/simulation changes.
- Share view-independent preparation where valid. Visibility, projections and
  order remain specific to each eye/pass; do not reuse one eye's visible list
  blindly. Preserve existing stereo presentation, resource and uniform ownership.
- Select backend/device policies from capabilities and measured workload costs.
  Keep equivalent scene semantics and correct fallbacks on WebGPU/WebGL2. Do not
  assume the desktop winner is the Quest winner or that a GPU/worker path wins.
- Keep decisions observable: actual submitted calls/instances/triangles, uploads,
  rebuild causes, batch-break reasons, resource use and scoped timings. Distinguish
  estimates from actual commands and requested from visible object counts.

For resource/pass invariants use [frame-pipeline](../frame-pipeline/SKILL.md);
for direct Quest XR support use [quest-webgpu-xr](../quest-webgpu-xr/SKILL.md).

## Learn, prove and generalize

Use relevant techniques from other renderers, including Three.js, as normal
engineering inputs. Inspect the exact implementation/version and distinguish an
engine feature from an explicit scene setup such as authored InstancedMesh groups.
Adapt the mechanism to Rust ownership and general scene properties; avoid branches
keyed to example names, asset names or a benchmark's fixed object count.

For a substantial change, state one bounded hypothesis and its failure modes.
Use independent correctness checks plus repeated matched workload measurements;
retain adverse results. Keep geometry, resolution, effects and gameplay aligned,
or label remaining differences. Actual CPU scopes, GPU samples, application FPS,
presentation cadence and loading cost are different measurements; do not add them
into a fabricated frame cost. Missing data stays unavailable.

At an integrated milestone, check transfer beyond the motivating example with a
relevant second workload and a mutation scenario when lifetime/invalidation is
involved. Choose representative coverage and proportionate checks; this is not a
full-suite requirement after every edit. Use the [verification matrix](../../../docs/DEVELOPMENT.md)
and [performance guidance](../../../docs/RENDER_PERFORMANCE.md), checking current
source and metric definitions before relying on historical notes. Separate desktop,
simulated XR and physical-headset evidence; use matched on-device settings for a
Quest performance claim. Keep a compact accepted/rejected experiment record with
inputs, quality gate, results, limits and the next hypothesis. A completed checklist
or one scene win does not establish a universally optimized engine.
