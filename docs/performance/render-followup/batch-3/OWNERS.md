# Non-glTF allocation owners

These are source-audited owners; only the measured warm window below proves actual call frequency. Logical `wgpu::Buffer` creations and browser API creations are different instrumentation layers, never additive.

| Owner / trigger | Buffer purpose and lifetime | Decision |
|---|---|---|
| `State::refresh_info_panels` → `InfoPanel::update` → `refresh_info_panel_meshes` | Bitmap vertex36B/index4B records. Text refresh every250ms when rows change; old implementation allocates a replacement pair, despite recurring similar sizes. Live until next replacement/remove/clear. | WebGL2: reuse COPY_DST capacity per live mutable bitmap panel. Full current vertices at original cadence; exact unchanged index prefix can skip index upload. WebGPU defaults to original after adverse timing; immutable Text stays original on both. |
| Static imported/procedural triangle mesh sync | Vertex/index buffers at first source appearance; source identity changes invalidate derived alpha data. New cut halves contain new geometry. | Keep required creation; do not pool unrelated immutable meshes. |
| `InstanceBatch::upload`, imported instance pages | Transform/instance records; capacity growth and pass/eye ownership. | Existing capacity management left intact; not the measured warm owner. |
| `GlowMesh::new` | Two geometry buffers for newly encountered glow geometry or dirty neon-text mesh. Cached until removal/disable/update. | Retain; no evidence this is the ordinary Neon warm churn. Bitmap reuse does not remove necessary glow updates. |
| `DebugHud` | Vertex buffer initialized/grown as required;250ms text updates write existing capacity. | Already reuses capacity; no speculative second pool. |
| Cutting compute/resident paths | Event-specific output, edge, parameter and readback storage; geometry source caches. | Keep; event costs distinct from continuously replaced info-panel buffers. |
| Profiler, frame targets, pipeline initialization | Query/resolve/readback, uniform/staging and render targets. | Initialization/explicit resize/debug-only scope, not warm panel replacement. |

## Actual initial observation (before change)

Fresh25s separate whole-API observation, actual arena-active1280x720:
- WebGPU:89 vertex +89 index `createBuffer`,18,090,720 +3,015,120 bytes. Legacy labels say `glTF vertices`/`glTF u32 indices`; the shared `Mesh::imported` constructor is also used for bitmap panels. No glTF draw work inferred from those names.
- WebGL2:186 createBuffer plus186 bufferData calls describing20,015,556 bytes. These are186 resources, not372. createBuffer itself has no byte size; backend storage/alignment can differ from logical payload.

Separate candidate/reference-labelled observations below establish the dynamic owner directly. Timestamp coincidence is not causal proof of a frame stall.

## Lifetime and bounds

No new global pool. Each live panel owns one vertex and one index buffer. Power-of-two growth, minimum4B, shrink when payload is below one quarter of retained capacity. After publication, capacity is bounded by current live content (at most4× each nonzero payload, minimum4B per buffer), not historical maximum or ever-seen keys. Zero geometry draws zero indices; retained capacity never becomes `index_count`. Empty/removed and changed-size cases are tested. Physical driver residency/deallocation timing is unavailable; reported gauges are engine-owned logical buffers.

Updates happen before rendering a view; the dirty flag clears after publication. Later eyes/passes use identical current bytes. No synchronous GPU wait or private backend interop is introduced. Replacement drops owned handles, allowing wgpu to preserve submitted-command lifetimes. `finish_scene_updates` cleans removed objects. Clear/dispose retains no inactive panel pool.

The original mapped-init path remains independently selectable for diagnostic A/B. CPU bitmap geometry generation and temporary Vertex conversion remain unchanged; removing GPU creation is not a claim of zero CPU allocation or faster FPS.

Gauge caveat: CPU payload, current-scene live capacity and all held index shadows can describe different phases between JS mutation and next renderer cleanup. See REPORT memory section and aligned lifecycle test. Do not use arbitrary mixed-phase samples as an instantaneous bound.
