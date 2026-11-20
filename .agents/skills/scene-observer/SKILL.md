---
name: scene-observer
description: Diagnose and optimize a replicated 3D spectator scene with an independent camera. Use for Quest-to-viewer lag, stalls on cuts, reconnects, excess wire data, missing effects or viewer-dependent presentation.
---

# Diagnose the replicated scene

Identify the failing view first. The headset simulation owns the game. The observer receives scene state and renders its own camera. Quest Stream sends display video and has a [separate workflow](../quest-stream/SKILL.md). Confirm the serving checkout, viewer entry point and build used by the reporter.

## Locate the delay

Reproduce the same passage from idle into fruit launch, a burst of cuts and recovery. Compare source sample times, publication/encoding time, transport bytes and gaps, and viewer apply/render time. Keep clock domains explicit. Measure decoded asset bytes separately from bytes on the link. Scene age can include sampling and the playback buffer. A lower buffer trades delay against a greater chance of exhausting the available poses. Compare at equal source cadence and include long gaps and a reconnect.

For the LAN observer, check the viewer JS actually served through the listener and look for the active playback branch. The verified Neon LAN viewer uses a **50 ms** source-timestamp buffer. Eligible fruit and fragments can extrapolate for up to **80 ms** beyond the latest source sample; swords hold their last known pose during starvation. After the cap, fruit also holds until fresh data arrives. These settings are separate; adding them does not give measured end-to-end latency. The headset game has its own frame timing.

The owning checkouts can differ: in the LAN server checkout inspect `tools/scene-observer/viewer.js` and `src/scene_playback.rs`. The legacy/replay [cadence policy](../../../tools/scene-observer/interpolation-policy.mjs) starts at 60 ms and adapts to source cadence. Measure the active branch before changing either value.

## Send changes and preserve continuity

- Reuse known assets and instance IDs. Send transforms and changed semantic state. Timer and score updates previously invalidated and sent unused fallback panel meshes even when the visible digits came from 3D glyph instances. A bound panel reuses the shared glyph assets and publishes the changed instances. An unbound panel still needs its fallback geometry. For a regression check, change the score repeatedly with a warm viewer cache, then join from an empty cache and check both the digits and bytes by asset type. [Measured glyph example](../../../docs/OBSERVER_GLYPH_WIRE.md)
- Inspect the [transport and replay guide](../../../tools/scene-observer/README.md) before changing protocols. Delta decoding depends on a valid base. Exercise a slow viewer and late join; verify the latest state recovers without building a queue of obsolete poses.
- In [mirror interpolation](../../../src/scene_interpolation.rs), smooth the displayed pose while keeping authoritative transforms intact. When a viewer has already displayed the parent, cut lineage and the birth transform let its halves start at that displayed pose. A late join or reconnect can require the current fragment meshes and snap to their current poses. This legacy smoothing path holds at its target. The live buffered viewer follows the timing rules above. Hits, score and rewards always come from the simulation.
- **Billboard cut:** the authoritative sweep determines the line on the original quad. The two bird pieces reuse the same artwork and carry the frozen animation frame, clipping metadata for opposite sides and their own poses. Inspect [the sprite split](../../../src/sprite.rs) and `sprite_cut_fragment_full_frame_clip_and_frozen_plane_roundtrip`. The viewer can draw an arbitrary diagonal cut without receiving a newly baked texture or a triangle mesh for each piece.
- **3D fruit cut:** the current path publishes the new fragment geometry when needed, then its poses. Lineage smooths motion; it cannot reconstruct the cut surface. For a future compact cut message send the parent asset identity/revision, exact local-space sweep, cut options, parent/child IDs and pivot transforms. Replay with the same cutter on the viewer, compare a geometry fingerprint, and fetch the missing mesh once if the parent is unavailable or the result differs. Include reconnect after a second-generation cut and coalesced updates in acceptance. [Design and limits](../../../docs/OBSERVER_GLYPH_WIRE.md)
- Follow short-lived events from producer through receiver. For audio, Neon publishes its journal via `Game::state().audio` and the [viewer](../../../tools/scene-observer/viewer.js) ingests it from frame metadata. Preserve event identity, deduplication and expiry through coalescing and reconnects. Only apply those guarantees to other event types after checking their implementation. [Game audio](../game-audio/SKILL.md)
- Compare headset and observer with the same source pose to diagnose missing assets, floor or lighting. Then switch the observer camera: world state stays shared while billboard direction, projection and apparent reflections can differ with viewpoint.

Run `npm run test:observer` for transport and reconnect behavior. For changed rendering or cuts, use the browser fixtures in the transport guide, including an irregular motion and burst-cut replay. Confirm behavior on the actual LAN for reported network issues. The design target of roughly 100 spectators needs a separate concurrent load test.
