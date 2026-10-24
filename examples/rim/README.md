# Aurelia in the Rust/WASM renderer

This playable example uses the actual city exported from the original Aurelia scene:
62,076 city triangles in 19 material batches, four original animated GLB assets, and the original gladii. Import, model state,
animations, PBR and rendering belong to Rust. Runtime has no Three.js dependency.

The six authored glTF cameras preserve source right-handed orientation, 65-degree
vertical FOV and original poses. They intentionally use the viewport aspect ratio.
The default is the original gate view, `[0, 1.7, 42]` facing -Z. Switch views with
the bottom buttons. The default gameplay starts at the gate; the view buttons pause gameplay for an architectural preview.

The default appearance uses the authored linear sky/ground hemisphere colors,
a directional sun, ACES exposure 1.25, 90–245 m fog and a cached 2048² sun
shadow map. Desktop pixel ratio is capped at 1.5, matching the Three reference.
A compact FXAA pass smooths opaque geometry, with specular antialiasing in glTF
materials. XR uses the headset eye dimensions at resolution scale 1.

FXAA and the reference's hardware MSAA are different techniques. Transparent
geometry/glow drawn after the opaque postprocess are not covered by FXAA.
Native HUD, particle presentation and cut-fragment materials still differ from
Three; this is not a claim of identical gameplay code or pixel-for-pixel output.

Source assets, camera metadata, export reproduction and provenance are in
`../../assets/models/rim/SOURCE.md`. The original four model files are copied
byte-for-byte. The city GLB can be downloaded from the viewer.

## Verification

Use a matching release `pkg/`, then the maintained repository runner:

```sh
VERIFY_WASM_PROFILE=release node scripts/test-browser.mjs --page rim,rim-gameplay --backend all --capture
node assets/models/rim/verify-assets.mjs
```

The Verify button checks city/camera pixels, actual keyboard movement, dash/stamina, slow-time jump, sword hit and worker-generated split geometry. `rim-gameplay` exercises the current-frame stereo path, tracked blades, input mapping, movement and session reentry.
WebGPU/WebGL acceptance is desktop evidence, not physical Quest performance.

## Desktop comparison observed during this conversion

At 1280×800 with pixel ratio 1, paused original poses, identical 65-degree gate
camera, four comparable fixed point lights, and AA/shadows/fog/tone mapping off,
Chrome 155 delivered approximately 60 RAF frames/s for Three.js, native WebGPU
and native WebGL. All three submitted 254 color draw calls. Refresh-rate capping
means this does not rank their maximum throughput or prove equal GPU cost.
The WebGPU profiler reported one main-pass GPU sample of 2.59 ms; there was no
matching Three GPU timing, so it is not a cross-renderer speedup measurement.
The comparison used sequential pages, with no concurrent renderer tabs.

The historical timing above predates the appearance and visibility corrections.
It is retained as a record, not a benchmark of the current default configuration.
Current verification covers `gltf-visibility`, `scene-appearance`, `antialias`,
`rim` and `rim-gameplay`; headset timing is measured separately.

## Native gameplay

The example now explicitly configures Aurelia gameplay in Rust (`rim_game`, `rim_movement`). Encounter waves: 6 orbiters, 2 tanks, 2 wraiths, 8 orbiters, 3 tanks, 3 wraiths; the next wave waits for all deaths. A fixed 8/3/3 model pool is reused. Native 100 HP, original .23/.34 m plasma telegraph/beam radii, pooled reflectable bolts, both gladii, ballistic slow-time jumps, dash and real-time stamina run before presentation. The original 1,064 static convex proxies plus the Madmechanic collider are exported offline, including roof planes and arch openings; a cached native BVH handles movement and rays. Desktop uses the original WASD/Shift, Q/E or mouse buttons, drag-look, C dash and Space hold/release jump controls. XR uses original 1.8 m/s left-stick walk, right snap turn, right trigger jump aiming and A dash.

`cut-worker.js` is a message/initialization shim. Its independent instance of the same Rust WASM performs actual plane clipping of posed GLB triangles. Input preparation is batched; complete results replace the original model atomically, then native ballistic motion and dissolve remove the fragments. The clip surfaces remain open; textured GLB fragment materials are represented by their original vertex/base colors. Original `Explode` animation bridges worker preparation. No dead fragment participates in gameplay contacts. `dissolve` mode avoids geometric division. The shared appearance profile now follows the authored Three lighting; the BRDF and fragment-material paths are not identical implementations.

Generate collision data and the original gladius offline with `node assets/models/rim/export-gameplay.mjs`. This conversion imports the original Three app solely for export; it is not a runtime dependency. No push or physical-headset performance acceptance is implied by browser tests.

## Enemy health (shared Three/native contract)

Orbiters have 100 HP, tanks 200 and wraiths 150. Each sword stroke deals 50; a
returned projectile deals 100. Nonfatal hits keep the target alive and never
start a cut or award a kill. Pooled world-space bars show remaining HP in both
desktop and stereo views. Each hand has a contact latch and a 0.28 s real-time
cooldown; desktop swing IDs additionally prevent two hits from one swing.
A stationary blade inside the target cannot repeatedly deal damage when motion
resumes; actual separation rearms it. Spawn resets HP and hit state.

`healthBars` in the scene configuration optionally supplies one background/fill
mesh pair per enemy. Rust owns their pose, fill width and visibility; authoring
creates their static colored geometry. The Three reference uses the same
wave/HP/damage values in its `combat-rules.js`.
