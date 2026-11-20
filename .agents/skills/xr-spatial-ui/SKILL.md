---
name: xr-spatial-ui
description: Build or adjust in-headset VR interfaces such as FPS panels, status displays and menus as spatial scene nodes, including head-following placement and stereo verification. Use for VR presentation, not desktop-only HUDs or tracking backend changes.
---

# Spatial interfaces in VR

Follow the repository [AGENTS.md](../../../AGENTS.md), [XR player workflow](../xr-player-equipment/SKILL.md) and [scene layout conventions](../scene-object-layout/SKILL.md).

Honza's correction: a VR FPS display must be an actual spatial node that moves with the player, not text pinned to a corner of each rendered eye. Do not solve an in-headset UI request by enabling the desktop/screen-space debug overlay unless the user specifically requests that overlay.

## Choose the attachment and physical layout

- Treat the interface as geometry shared by both eyes: author width, height and distance in metres, with a readable face toward the viewer. A DOM panel or a painted eye corner is not a substitute.
- Distinguish world-fixed, head-following and hand/controller-mounted UI. When Honza asks for a panel in front that follows him, use the head binding with a local forward offset; do not silently replace it with a fixed landmark or assume body/yaw-only following.
- For head following, Rust owns `world = headPose * translation(localOffset) * scale`. Use `engine.xr.player.bindHead(panel,{offset:[x,y,z]})`; no JS RAF, polling or per-eye copy. Under the default head frame, -Z is forward and -Y is down. `setPosition` before a head binding is not a persistent attachment offset.
- Debug/FPS panels default to `examples/shared/vr-debug-panel.js`: [0.9,0.35,-1]m from the player's floor anchor, size 0.66 × 0.40m. Bind with `anchor:'feet'`; this follows body translation and locomotion/reference-frame rotation and floor height, ignores ALL head rotation including yaw plus crouching for placement, and faces toward the head. Looking down or sideways must not make the panel chase the face. Sideways glances leave its player-local position fixed; explicit player snap turns rotate the reference frame. Benchmark debug status temporarily replaces the same debug slot. Neon overrides that offset in [panel-layout.js](../../../examples/neon-arena/panel-layout.js) to put its FPS panel left of the player, just below score height. Aim the simulated camera at the scene's configured panel when comparing both eyes. Keep the main view usable and allow hiding the panel; respond to the user's headset feedback about distance, size or placement.
- Use existing native info panels for live FPS/counters. Do not rebuild their text every frame in JS. Distinguish application XR cadence from compositor refresh and CPU submission from GPU time.
- Preserve scale and native slot ownership. Hidden/disabled UI must not stay at the eye origin. Tracking loss or XR exit hides the bound node; reentry restores the binding. Rebinding must not overwrite another head-bound item without an explicit scene decision.

## In-world score entry

Neon uses a controller aim ray to select one to three initials after a qualifying round. Draw the ray before the trigger press so the player can see the intended key. Use the same aim pose for the pointer and the hit test. Exercise selection, a held trigger, confirmation and persistence after restart. The [keyboard](../../../games/neon-core/src/initials.rs), [visible-ray browser check](../../../tests/neon-native-parity.html) and [native persistence test](../../../native/neon/src/tests.rs) cover different parts of this interaction.

## Verify the user-visible behavior

Check both-eye visibility and finite depth, readable orientation, expected local offset during head translation/rotation, locomotion and teleport. Check hide/show, tracking loss and XR reentry. Ensure the screen-space corner overlay is disabled. A pixel difference alone does not prove a correct attachment; combine rendered stereo evidence with native transform assertions.

Use the maintained XR simulator and matching WASM. The forest scenario is [forest-arena-xr.js](../../../tests/forest-arena-xr.js). Simulation verifies transforms and rendering, not headset comfort or readability; request a real headset check only after the working change is delivered.
