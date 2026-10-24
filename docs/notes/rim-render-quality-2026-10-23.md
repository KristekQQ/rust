# Rome rendering quality: observations and proposed improvements

Date: 2026-10-23 (Europe/Prague). Status: notes only, explicitly requested by Honza. No runtime code, scene settings, WASM package or production output changed. No new performance benchmark or headset acceptance performed.

## Main finding

The current Rome port preserves the original geometry and models, but its presentation does not preserve the original quality settings. The visible difference has several concrete causes. It is not evidence that Rust/WASM or WebGL2 intrinsically produces a worse image.

Separate two kinds of work: fixing this example's setup (resolution and enabled capabilities) and extending the shared renderer (AA, lighting/output controls). Do not solve all differences by changing shaders globally or by adding glow.

## Observed on the current Mac preview

Read-only browser inspection of `http://127.0.0.1:8880/`, with both applications using WebGL2:

| Canvas | CSS size | Drawing-buffer size | Horizontal pixel ratio |
|---|---:|---:|---:|
| Original Three.js | 376 × 471 | 564 × 706 | 1.5 |
| Rust/WASM | 376 × 471.5 | 376 × 472 | 1.0 |

The Three canvas contains 398,184 pixels; native contains 177,472: approximately **2.24× the pixels in this captured layout**, or **2.25× at exactly equal CSS dimensions**. This directly explains a substantial difference in sharpness. It does not establish which pipeline has better throughput at equal resolution. These are measurements of the narrow embedded desktop preview, not standalone full-window measurements or Quest eye buffers. The source resizing policies apply to standalone desktop pages as well, subject to device pixel ratio and layout.

The screenshot also shows crowded desktop controls at this narrow width. That presentation issue should be assessed separately from the rendered 3D image.

## Confirmed implementation differences

| Area | Original Rome / Three.js r170 | Current native Rome | Consequence / evidence boundary |
|---|---|---|---|
| Desktop resolution | `setPixelRatio(min(devicePixelRatio, 1.5))` | Resize to canvas CSS width/height | Directly measured lower native pixel density above. |
| Antialiasing | Requests `antialias:true` on context creation | glTF pipelines use default single-sample multisampling | More edge aliasing expected; actual Three sample allocation was not queried in this inspection. |
| Lighting | Warm directional sun + sky/ground hemisphere light | Four distant point lights approximate directions; fixed diffuse ambient in PBR shader | Different shading, sky/ground fill and material response. This was an approximation during the port. |
| Shadows | Enabled 2048² sun shadow, PCF soft filtering, cached static map | Example calls `set_render_effects(false,false)` | Reduced shape/contact cues. The native engine already has shadow support; its use in this example is separate from sun-light support. |
| Output | ACES filmic tone mapping, exposure 1.25, sRGB output | No equivalent ACES/exposure setup in this scene | Different highlight compression and contrast. No gamma-conversion bug has been established. |
| Distance | Fog from 90 to 245 metres | No corresponding scene fog | Different distant depth cues; not the main explanation for nearby jagged edges. |
| HUD / impacts | Textured spatial HUD, original plasma and wound presentation | Primitive bar/block HUD and different attack effects | Some perceived quality loss is scene/UI authoring, independent of core renderer sharpness. |
| Cut fragments | Original material treatment and attempted cut caps | Vertex/base-colour fragments and open cut surfaces | A separate known visual limitation; not caused by canvas resolution. |

Source pointers, relative to this repository:

- `examples/rim/main.js:9–12`: effects disabled and CSS-sized drawing buffer.
- `examples/rim/scene.js:27–33`: documented point-light approximation.
- `src/render/gltf_gpu.rs:826`: glTF pipeline multisample state.
- `src/render/gltf_pbr.wgsl:140–156`: existing metallic/roughness direct-light PBR and fixed ambient contribution. PBR already exists; do not describe the renderer as lacking PBR entirely.
- `js/webxr.js:268–273`: native WebGL XR context/layer explicitly request antialiasing off.
- `examples/rim/README.md`: existing documented visual-parity limits.
- Reference application: `/Users/Honza/Documents/Codex/2026-10-22/ude/outputs/anticke-mesto/dist/main.js:13–21`.

## Proposed order, with no implementation authorization implied

1. **Make pixel density explicit.** Add an eventual desktop resolution policy with render scale and maximum pixel budget. Start comparison at Three's observed 1.5 scale, then evaluate native Retina scale if affordable. Show actual drawing-buffer dimensions in diagnostics. A 1.5 scale costs roughly 2.25× pixels, not 1.5×; this is not a free optimisation. Preserve CSS size and pointer-coordinate mapping.
2. **Add/test suitable AA.** Evaluate MSAA for thin architectural edges and moving weapons. Candidate comparisons are 1×/2×/4× where supported, including proper resolve and consistent colour/depth attachments. MSAA requires renderer/target integration, not merely changing a JS flag. Check postprocessing and both XR eyes. Temporal approaches require motion/history handling and should not be the first unexplained addition in VR.
3. **Restore the reference lighting intent.** Expose a directional sun and controllable sky/ground fill, with an explicit scene lighting configuration. Reuse cached static shadows for the city; update actual dynamic casters only when needed. Match the reference before introducing extra artistic lighting. Four distant points are an approximation, not exact hemisphere/sun parity.
4. **Calibrate colour and output.** Validate linear working values, sRGB colour textures/output, non-colour normal/roughness data, exposure and an ACES-like display transform. Use grey ramps and known material swatches. Do not infer a gamma defect merely from a different image; do not compensate for an incorrect conversion by changing light intensity. Preserve existing XR sRGB conversion contracts.
5. **Restore scene presentation.** Match fog deliberately, replace placeholder HUD geometry with a readable spatial panel, and preserve intended weapon/projectile colours and emissive appearance. Handle cut-material/cap quality separately, with its own performance budget. These can matter visually as much as shader work.
6. **Only then assess further material features.** Environment lighting, roughness-aware reflections, filtering/mipmap/anisotropy quality and contact occlusion can be useful. The original scene inspected here does not configure an environment map, so IBL is not a confirmed missing cause of this particular difference. Texture filtering is not the first target for this largely material-colour city.

For VR, use a separate quality profile. Do not multiply headset eye dimensions by the desktop DPR policy. Compare actual XR eye framebuffer dimensions, foveation and antialiasing, then test moving thin geometry and UI readability on the headset. The current Rome request uses `resolutionScale:1`; that setting alone does not establish equal physical eye-buffer resolution or AA across the two implementations.

## Inspiration from Three.js

Follow the design principles rather than copying its JavaScript runtime into the native engine: explicit pixel dimensions, configurable AA, consistent linear colour workflow, well-defined light types and material/output controls. Keep simulation, shading and rendering in Rust; let the SDK expose authored settings.

Primary references checked today (current documentation; the local reference application remains r170):

- [Responsive rendering and drawing-buffer pixel budgets](https://threejs.org/manual/pages/responsive.html): distinguish CSS dimensions from internal pixels; control the actual pixel workload.
- [WebGLRenderer options](https://threejs.org/docs/pages/WebGLRenderer): context AA, output colour space and shadow-update controls.
- [Colour management](https://threejs.org/manual/pages/color-management.html): linear lighting, annotated inputs and display conversion.

## Acceptance plan for any future implementation

First capture both versions separately at identical camera, animation pose, drawing-buffer dimensions and enabled features. Compare edges, highlights, shadowed surfaces and HUD at native image size. Keep resolution and quality fixed while testing an optimisation. Record CPU submission and GPU/completion costs separately.

The earlier gameplay benchmark intentionally forced DPR 1 and disabled Three AA, shadows, fog and tone mapping to approximate native settings. It does **not** describe the quality of the default pages Honza is viewing now. Its desktop timings also do not prove headset performance.

Then test WebGL2 and WebGPU separately, and finally the actual Quest stereo path. Check both-eye colour, resolve/depth correctness and movement stability. Select the headset quality setting from measured frame budget and user feedback; do not claim that all proposed quality features will be affordable together.

Open questions: actual Three AA sample allocation on this Mac/Quest; perceived headset difference; equal-quality throughput; whether any remaining material mismatch is a renderer defect rather than different authoring. No runtime bug or performance improvement is claimed by these notes.


## Follow-up: concrete Three.js r170 techniques, source audit

Honza requested a deeper comparison with Three.js and use of project skills. `rust-webgpu-project` and `frame-pipeline` were applied. The last explicit scope was notes only; runtime changes require clarification of the newer request. The following observations come from both the local r170 bundle and the versioned upstream source, not assumed behaviour of the latest Three release.

### Sky/ground fill instead of four approximate direct lights

Three's hemisphere contribution interpolates authored ground and sky colours using the surface normal's alignment with a hemisphere direction. The reference Rome combines that with one directional sun. Native Rome instead runs its four-light direct metallic/roughness loop, plus fixed diffuse ambient.

Candidate: represent the intended sun and hemisphere explicitly. A hemisphere fill is a small diffuse-light calculation; it need not execute a full direct specular-light evaluation for each approximation light. This could improve lighting fidelity and reduce some shader work simultaneously. That is a structural opportunity, **not a measured GPU speedup**. Preserve the source's intended intensities and colour management rather than copying numeric intensities between incompatible formulas.

Pointers: local `vendor/three.module.js:14072`; native `src/render/gltf_pbr.wgsl:142–149`. [Versioned Three light shader](https://github.com/mrdoob/three.js/blob/r170/src/renderers/shaders/ShaderChunk/lights_pars_begin.glsl.js).

### Filter rapidly changing specular highlights

Three adds a screen-space normal-variation contribution to material roughness. Our inspected native shader clamps the authored roughness but does not include that corresponding derivative-based contribution. Candidate benefit: calmer small highlights on moving metal, especially relevant to head movement in VR. A/B testing must establish whether this is visible on the Rome models; it is **not yet proven to cause Honza's reported appearance difference**.

This complements geometric-edge AA; it does not replace it. Do not copy the numeric minimum roughness blindly: Three's source explains its particular minimum in terms of an environment-map mip level, while our current material/environment path differs.

Pointers: local `vendor/three.module.js:14084`; native `src/render/gltf_pbr.wgsl:139`. [Versioned Three physical material shader](https://github.com/mrdoob/three.js/blob/r170/src/renderers/shaders/ShaderChunk/lights_physical_fragment.glsl.js).

### Avoid redundant state work, but measure below our backend abstraction

Three tracks current GL state and avoids repeating unchanged program/state changes. Native glTF drawing requests pipeline, material bindings and buffers per primitive. Candidate: inspect actual backend calls, then group compatible opaque work or avoid redundant requests where worthwhile. Our wgpu layer may already suppress some redundancy, so the source pattern alone does not establish a bug or a missing optimisation. Transparency sorting and per-eye state contracts must remain intact.

Our renderer already has material shader/pipeline caching, no-op transform protection, per-view caches and cached static-shadow paths. Preserve and exercise those mechanisms; do not claim to introduce caching as though none existed.

Pointers: local `vendor/three.module.js:23302` and native `src/render/gltf_gpu.rs:848–916`. [Versioned Three GL state cache](https://github.com/mrdoob/three.js/blob/r170/src/renderers/webgl/WebGLState.js).

### Texture sharpening is not a Rome fix

Read the actual GLB JSON chunks: `rim-city`, `madmechanic`, `drone-orbiter`, `core-tank`, `blade-wraith`, and `gladius` all contain **zero images and zero textures**. Their material/geometry appearance will not be repaired by adding anisotropic filtering. Native glTF upload already creates mip levels and averages sRGB colour in linear space before encoding; the audit did not identify a missing-mipmap or encoded-space averaging defect in this path.

Textured assets in other examples deserve their own filtering acceptance scene. Do not extrapolate the texture-free Rome result to the entire engine.

### Quality contract for future ports

Record the source's real framebuffer size, AA, light types, exposure/output transform, shadows, fog, material features and HUD/effect presentation before transferring the scene. Track every unsupported feature and every deliberate approximation. Geometry parity alone must not be reported as equivalent visual quality.

First target: equal-quality Rome at a fixed camera and pose, then efficiency at those same settings. Extra bloom, higher pixel counts or arbitrary brighter lights are not substitutes for identifying the specific missing component. No runtime fixes, shader edits, builds or new headset measurements were performed for this follow-up.


## Implementation follow-up (same day, explicitly authorized)

The later request authorized implementation and subagents. The Rome profile now
uses the authored hemisphere/sun, ACES exposure, fog, cached 2048-square shadows,
desktop DPR cap 1.5, specular AA and opaque FXAA. Three remains the original
reference. Native glTF submissions filter empty visibility entries; inactive Rome
pooled animations pause, preserving visible death animation and atomic cut swaps.

Two integration defects were caught and fixed: WebGL2 must not build an unused
transparent MRT pipeline, and specialized glTF shadow shaders must remain valid
on both Naga and Dawn. The browser runner now captures uncaught GPU validation
events, which Dawn may otherwise only print as warnings. The orthographic shadow
regression checks both a local cast shadow and preserved direct ground lighting.

These changes retain Rust/WASM. See `examples/rim/README.md` and
`output/verification/rim-quality/` for current verification/artifacts. FXAA differs
from Three MSAA; transparent overlays, HUD and fragment materials retain stated
differences. Browser/simulated XR checks do not establish physical Quest quality
or performance. No commit or push was requested. Earlier audit numbers above
remain historical, not timings of this new profile.
