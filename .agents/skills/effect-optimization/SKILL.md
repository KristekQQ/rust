---
name: effect-optimization
description: Create or optimize graphical effects in this engine using cheaper, visually convincing approximations. Use when designing or tuning glow, particles, reflections, shadows or similar effects, especially when Honza asks to fake an effect or improve its performance while preserving its appearance.
---

# Cheaper effects with the same perceived appearance

Honza prefers standard graphics tricks that convincingly reproduce the intended
look with less work. Whenever creating or tuning an effect, consider a cheaper
visual approximation before assuming an expensive exact simulation is needed.
Choose by what a player can actually see during play. This preference does not
authorize unrelated rewrites, deployment or changes to gameplay/physics.

## Choose the approximation

Inspect the current implementation and dominant cost first: overlapping pixels,
shader arithmetic, geometry, passes, uploads or CPU preparation. Reuse existing
capabilities and established techniques; do not add a cache or lookup texture
unless it removes relevant work. A texture lookup is not automatically cheaper.

- Stable shape: consider baked masks/distance fields, atlases or impostors.
  Motion alone does not invalidate local-space data; changing viewpoint or
  topology may. Keep appropriate side-view and mutation fallbacks.
- Soft, spatially smooth effect: consider lower-resolution offscreen rendering
  followed by depth-aware upsampling. Keep sharp geometry and UI separate.
- Repeated or distant detail: consider sharing, instancing or a suitable LOD.
  Reuse across frames only when changes and camera motion are handled correctly.

Keep implementation reusable, following
[renderer-strategy](../renderer-strategy/SKILL.md). Use
[frame-pipeline](../frame-pipeline/SKILL.md) for pass/resource integration.

## Preserve what the player notices

Compare the baseline and candidate at matched scene state, camera and output
resolution. Explicitly label the effect approximation; pixel identity is not
required for an intentionally approximate effect. Still check sharp features,
brightness/color, foreground occlusion, both eyes, close/oblique views and motion.
For destructible objects include changed topology and separated rotating pieces.
Look for shimmer, popping, ghosting and halo leakage that still images can miss.

Keep a baseline switch/fallback during evaluation. Prefer one bounded experiment
and proportionate checks over an unbounded search. Enable the candidate within
the authorized task when appearance and total cost justify it; retain adverse
results and state unsupported backends or unverified conditions.

Measure total affected work, including new passes, allocations, uploads and
composition, rather than just a faster shader. Use
[quest-hardware-profiling](../quest-hardware-profiling/SKILL.md) for physical Quest
evidence. GPU time, live FPS and wearer feedback are different evidence: record
each accurately without translating one into another.

## Existing examples and terminology

- **Glyph boundary-distance atlas:** prepared textures replace many halo edge
  primitives while the real 3D digit cores remain. Oblique views use the original
  path. This is not a general billboard replacement for rotating cut fruit.
- **Half-resolution rendering + depth-aware upsampling:** ordinary soft halos
  render at half width and height, then are composed with depth checks. The halo
  target has one quarter as many pixels; that does not imply four times the FPS.
  The shared renderer selects this by default for eligible Vulkan/WebGPU views,
  with or without postprocessing; WebGL retains full-resolution ordinary halos.
  Sharp surfaces, glyphs and panels retain full resolution. Font prewarming stays
  an explicit loading-time operation; unsupported sources fall back. Conservative
  depth handling can narrow halo silhouettes.

For the accepted glow approach, its device evidence and wearer feedback, see
[the glow documentation](../../../docs/POSTPROCESSING.md#levnější-měkká-záře-half-resolution-rendering--depth-aware-upsampling).

Locate the current owning checkout before using these implementations; canonical
root and installed builds may differ. The existing hardware runner supports
`--experiment half-glow` and `--view cut --cut-age 0.4`. Check actual backend
support instead of assuming native and web have both been enabled.

Reference: [NVIDIA GPU Gems 3, High-Speed Off-Screen Particles](https://developer.nvidia.com/gpugems/gpugems3/part-iv-image-effects/chapter-23-high-speed-screen-particles)
describes the lower-resolution effect/depth/composition approach and its artifacts.
