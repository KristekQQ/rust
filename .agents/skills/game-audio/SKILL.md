---
name: game-audio
description: Add or diagnose game sound in the shared Rust audio journal, web/native output adapters and scene observer. Use for cues, levels, spatial playback and audio regressions; use quest-stream for headset capture permissions and AAC transport.
---

# Game audio

Read [docs/AUDIO.md](../../../docs/AUDIO.md) for architecture and current limits.
Rust owns gameplay cues and policy (`src/audio.rs`, `games/neon-core/src/game.rs`).
The journal is retained, not destructively consumed: native output, web output
and observer must each see the same events. Preserve epoch/id deduplication,
event age, bounded voices/queues, priority/group limits and lifecycle resets.
WebAudio DSP in `js/audio.js` and native PCM/AAudio are output adapters, not
independent gameplay simulations. Do not promise bit-identical spatial sound.

Assets in `assets/audio/neon/` are original generated mono 48 kHz PCM16 WAVs.
When changing the bank, preserve manifest identities and validate native decode
as well as browser load. Keep game master gain separate from viewer/user volume;
avoid summed clipping. Native playback consumes typed snapshots off the sim
thread; do not add JSON parsing or unbounded work to the XR frame path.

Keep DOM controls in the owning menu when a floating panel would cover gameplay
entry controls. Audio needs a user gesture in browsers. Hide/disable output when
appropriate, cancel scheduled voices on disposal, and discard stale events on
resume/reconnect instead of playing a delayed burst.

## Choose checks by changed behavior

- Journal/mixer: `cargo test --lib audio`; shared cues: neon-core tests.
- Web adapter: `npm run test:audio`; maintained browser `--page audio` covers
  real gesture unlock, PCM bank decode, stereo, listener rotation and distance.
- Arena integration: browser `--page neon-arena-entry,neon-native-parity`
  checks mobile/desktop pointer access, XR reentry and shared state.
- Observer transport and LAN access: `npm run test:observer`. For actual playback
  open the requested local or LAN viewer, enable sound, inspect scene updates,
  switch camera and check reconnect. A server status or unit test is not visual
  or audible acceptance. The viewer-only LAN gateway is documented in AUDIO.md;
  use the computer's current LAN IP for another device, never localhost.
- Android output: affected native tests/build, then actual device lifecycle and
  listening when available. Host PCM tests do not test AAudio routing or latency.

Use the repository's maintained browser runner and matching build environment;
do not rewrite freshness receipts to bypass mismatches. The final push gate is
defined by AGENTS.md; avoid repeating full suites during tuning.

## Keep the two sound paths distinct

Observer renders events from its own camera/listener. Quest Stream captures
permitted Android playback separately; observer audio cannot fix silent capture.
Use [quest-stream](../quest-stream/SKILL.md) for RECORD_AUDIO/MediaProjection,
H.264+AAC checks and same-session reconnect. An AAC label proves track selection,
decoded media proves playback, and user listening proves the exercised sound.
Report these separately from latency, headset FPS and spatial-quality claims.
