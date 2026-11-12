# Shared audio

Rust owns Neon sound events, cue choice, variants, positions, gain and voice policy.
`src/audio.rs` provides `AudioScene`, `AudioEvent`, `AudioSnapshot`, `AudioPolicy`.
The game appends a maximum of 64 events, retained for one simulation second.
Each round starts a new epoch; IDs increase within an epoch. Overflow is counted
in `AudioScene::dropped`. Journals travel with the existing state publication;
no scene-wire schema change or second gameplay simulation is needed.

## Outputs

- `js/audio.js`: reusable `AudioClip`, `AudioSource`, `AudioListener`, `AudioMixer`
  output adapter. Web Audio handles mixing/spatialization outside rendering.
  Rust-authored policy supplies group limits and total voice count. The master
  policy gain is multiplied by independent user volume. Output unlock occurs
  inside the VR-entry/button gesture; loading and hidden/muted output consume
  event IDs without replaying a backlog when enabled.
- `src/audio_mixer.rs`: preloaded PCM clips, fixed 24 voice capacity, mono-to-stereo
  equal-power pan and distance attenuation, bounded output. Native AAudio lives
  on a dedicated blocking-write worker with bounded nonblocking commands, not
  on the XR/render thread. Android usage is GAME for playback capture eligibility.
  The desktop native preview currently has no audio device adapter.
- Observer: the same event journal is deduplicated and scheduled with scene
  interpolation delay. Effective event age includes live transport age. Cold
  joins/reconnects baseline past events; new rounds can play their first cue.
  Recorded replay uses recording-relative timing, not current wall-clock age.
  Listener follows the viewer camera; live XR listener follows the head.

These are shared gameplay semantics with platform DSP adapters, not bit-identical
web/native output. Browser compressor and native peak limiter differ. Mono stereo
panning supplies left/right placement and attenuation; it is not binaural HRTF,
so no claim of reliable front/back or elevation localization is made.

## Neon cues

Start/end, last three seconds, hand swings, successful cutting sweeps, combo and
banking use the same `games/neon-core` implementation on web and native. A sweep
that cuts several targets emits one impact to avoid overload. Impact originates
near the blade, not a fabricated contact point. Optional spawn/miss are quiet-bank
assets only; no extra signal flood is enabled. PCM assets are embedded in native
and decoded once in web, shared across sources. No streaming music asset is added.
Web sources support explicit looping; durable loop replication is not part of
this one-shot Neon journal and must be added before authoring replicated ambience.

## Verification

`node --test tests/audio-unit.test.mjs`: dedupe, coalescing, age, resume, policy,
voice limits and invalid data. `cargo test --lib audio`: journal and PCM tests.
`cargo test --manifest-path games/neon-core/Cargo.toml`: shared gameplay regression.
Maintained browser runner `--page audio,neon-native-parity --backend webgl` checks
real user-gesture unlock, bank decode, rendered stereo PCM and shared game events.
`native/neon` Android target build verifies AAudio bindings/linking; real headset
latency, routing, suspend/resume and listening remain separate acceptance.

Quest Stream captures system playback independently of observer event playback.
Its capture permission, AAC transport and browser decoding checks belong to
`tools/quest-stream`; a silent capture cannot be repaired by observer sounds.

## Acceptance on 2026-11-12

The signed Neon and Quest Stream APKs were installed on a physical Quest 3 with
matching SHA-256 identities. The user reported hearing game and observer effects
and accepted the result for integration. The live receiver displayed H.264 + AAC,
1280×720 video, running/unmuted. This confirms the exercised setup, not measured
capture latency, spatial accuracy, Quest FPS impact or long-session stability.
Android labels RECORD_AUDIO as microphone permission even for playback capture;
Quest Stream exclusively configures AudioPlaybackCapture and never a microphone
source. Denial keeps the capture video-only. Closing its panel leaves the service
running; stopping/restarting capture changes the private viewer session URL.

## Observer on another device

Keep the observer server bound to loopback. To share its native scene with a
phone on the same trusted LAN, run the viewer-only gateway with the computer's
current LAN address and the existing observer port:

```sh
node tools/scene-observer/lan-viewer.mjs --host 192.168.0.17 --port 8894 --upstream http://127.0.0.1:8891
```

Open the private URL printed by the gateway on each device. It authenticates a
viewer cookie and uses WebGL for HTTP access; tap **Zapnout zvuk** on each device.
The gateway excludes publishing, recording downloads and native connection
secrets. It serves only the observer's runtime files. This is unencrypted HTTP
for a trusted local network; do not expose the port to the Internet. Restarting
the gateway creates a new URL. The computer must remain running.

`npm run test:observer` includes authentication, origin checks, runtime-file
allowlisting, native-token redaction and viewer-only WebSocket checks.
