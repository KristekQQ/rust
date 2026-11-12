# Generic scene observer — native Quest to Rust/WASM

The native path now mirrors the engine's supported render publication rather than authoring placeholder swords in JavaScript. A shared model/variant has a stable resource ID; scene objects retain their IDs and exact transforms/model matrices. Cutting removes the source object and introduces the real detached triangle meshes; later removal clears them. The optional game state includes flying object IDs and fragment/cut counts. Rendering does not require a fruit-name switch or duplicate physics/gameplay in the viewer.

## Run

The observer selects `engine.setCameraHandedness('right')` so native/OpenXR world
poses and text have the same left/right orientation on desktop. Other desktop
examples retain the existing left-handed default. The option changes built-in
camera projection and horizontal input in Rust; source objects, wire matrices,
native XR views and imported glTF cameras are unchanged.

```sh
node tools/scene-observer/server.mjs
```

Open `http://127.0.0.1:8791/tools/scene-observer/?source=native`. The native application needs a private `files/neon-observer.json` with `host` (Mac LAN IPv4), `port:8792`, and the receiver's per-session `token`. Optional `scenePort` defaults to `port+1`. Tokens are available only via the loopback `/api/native` endpoint; do not commit tokens/config backups or share the endpoint outside the local machine. The HTTP UI binds loopback, while authenticated UDP8792 and TCP8793 receive from the LAN. HTTP itself is not encrypted.

`connect-quest.py` backs up the installed APK, score/settings/config, installs the selected matching-signature APK, writes the private receiver configuration and starts Neon. Supply `--adb-port`, `--serial`, `--host`, `--apk`, and a new empty `--out`. Read the device's current IP; do not assume a historical endpoint. Installing a build restarts the app; preserve the signing key (`NEON_SIGNING_KEY` when building). The connector checks that diagnostic replay is absent. It does not change Quest Stream or the proximity helper. Existing `tools/quest-stream/keep-awake.py --keep-app cz.kristek.neon.nativevr --follow-app` can keep this development session awake; Guardian/tracking dialogs still need the wearer to complete them.

## Data and ownership

- The existing UDP NPO1 path carries head/grip poses plus clock calibration.
- Generic NSC1 TCP ingress authenticates with `NSC1`+16 token bytes. Each frame is u32LE JSON length, u32LE NWR length, JSON metadata, NWR bytes; one byte `1` ACK follows successful ordered ingest. One authenticated publisher; absolute authentication deadline; receive/cache caps.
- Native opt-in simulation captures an additional complete render publication at most20Hz, sharing immutable geometry. A low-priority worker serializes NWRv4 and owns networking. No observer networking/serialization runs on the XR thread. This extra capture has not been benchmarked as negligible.
- Geometry is sent when new/changed. Current NWR carries full live object state each frame; it is not yet a changed-matrices-only protocol. One native frame is in flight. Newer complete snapshots can replace a pending native snapshot without losing cut geometry; the serializer compares against the last acknowledged publication.
- The relay ingests every native delta in order, retains bounded live assets and emits per-browser coherent snapshots. A slow browser cannot hold up native ingest. Reload/late second viewers receive required current assets before transforms. Native reconnect resets its asset cache and starts a new session.
- The browser uses a matching Rust/WASM decoder. Generic mode creates no local objects in the authoritative ID namespace; this avoids ID collisions. It renders source equipment instead of locally reconstructed swords. The head pose remains metadata; the old pose-only helmet overlay is not injected into the generic scene.

Supported wire content includes primitive objects, triangle meshes, shared variants, panels, numeric/mesh-font tags, lights, transforms/material state and cut meshes. Static glTF assets/poses still require identical preloading; this work does not claim arbitrary glTF/skinning/texture replication. It is reusable within the engine's publication schema, not a universal protocol for unrelated applications.

## Verification

```sh
cargo run --manifest-path native/neon/Cargo.toml --locked --bin observer-scene-fixture --target-dir output/observer-content-test-target -- tests/fixtures/native-scene
node --test tests/scene-observer-transport.test.mjs tests/scene-observer-native.test.mjs tests/scene-observer-scene.test.mjs
VERIFY_WASM_PROFILE=release node scripts/test-browser.mjs --page scene-observer-scene --backend all --capture
```

The fixture is produced by actual Rust shared-mesh cutting, not fabricated JS geometry. Tests cover initial shared asset, two detached cut halves, removal, browser coalescing, late subscriber bootstrap and an unauthenticated peer not claiming publisher ownership. The browser case applies the actual NWRv4 bytes and checks rendered pixels on WebGPU and WebGL. Rust wire tests also cover borrowed-publication equivalence and skipped snapshots. These are distinct from physical Quest/player acceptance and do not measure streaming latency, FPS impact, thermal cost or endurance.

## Delay handling and binary deltas (2026-11-10)

NDP1 is an exact transport envelope around native NWR bytes and negotiated browser packets (`encoding=ndp1`). Full frames establish a numbered base; later frames patch coalesced changed64-byte blocks only when smaller. Source and browser packet sequences remain ordered, with a full fallback for poorly matching data. This reduces unchanged transport bytes; it is not semantic matrix interpolation or a new scene schema. Bases reset on new connections. Native ACK advances the base; malformed sequence/ranges/caps are rejected.

The viewer retains the last rendered scene during brief delays and pauses. It labels delay after250ms and pause after1500ms instead of covering the whole scene at200ms. Initial loading and actual errors remain explicit. Same-session generic reconnect keeps the GPU engine and rebases per-peer publication numbering; a real new source session reinitializes. An authenticated but idle TCP source is retained with TCPkeepalive rather than forcing a5-second scene reconnect. Unauthenticated peers still have an absolute deadline.

A local receiver may start with `OBSERVER_SNAPSHOT=/absolute/path/to/snapshot.packet` to display a previously received coherent scene while the Quest is offline. Its old timestamp must remain unchanged, so the UI labels it paused. The snapshot is replaced when the real source reconnects; it is never presented as live tracking. The snapshot can contain score/state data; keep it in ignored local output.

Additional checks: `node --test tests/scene-observer-byte-delta.test.mjs tests/scene-observer-scene.test.mjs` and the maintained `scene-observer-delivery` browser case cover exact byte roundtrips, malformed/base rejection, pause/no-cover and same-session recovery on both backends. Native byte-delta unit tests passed5/5. Updated APK is built locally; actual new-APK hardware throughput/latency must be measured after the Quest reconnects.

## Rust mirror interpolation

`engine.setSceneInterpolation({durationMs:60,maxDistance:2})` enables presentation-only smoothing; duration0 disables it. The observer enables it by default and adapts the interval to the median source cadence (30–250ms, with pause rejection and hysteresis). At4Hz it uses250ms; ordinary20Hz delivery retains60ms. The engine keeps source world matrices authoritative and interpolates displayed translation/scale and quaternion rotation toward each new target. It does not extrapolate after the target, run remote gameplay locally, or synthesize missing cuts. Removal, ordinary spawns, geometry replacement, teleports, invalid/sheared transforms snap. Explicit cut-child spawn metadata can instead seed a new child from its removed parent's displayed world pose multiplied by the immutable child birth-relative transform (including recentered physics pivots), so the child immediately moves toward its received pose during its first update interval. NWRv4 carries source-authored cut child/parent lineage through the native relay and recordings. The normal viewer applies it automatically; no proximity guesses or fruit-specific setup are used. Legacy v1–v3 packets remain readable and snap ordinary new IDs. Only active changed IDs are visited each render frame. Source material/fade values remain current, and normals/bounds/revisions follow the displayed pose.

This is smoothing toward the latest received pose over a configured duration, not a claim of60 source packets/s or zero extra visual latency. The UI reports the age of the received state. Smoothing adds presentation lag; tune duration against update cadence and jitter. It does not improve the headset's own render FPS, and no physical-device latency/FPS claim is made while the Quest is disconnected.

Acceptance: seven focused Rust interpolation tests and twelve wire tests pass. The maintained `scene-observer-interpolation` case uses an actual Rust motion publication: verifies an intermediate rendered pose, further motion without a second packet, settling, and immediate real cut/removal. `scene-observer-delivery` verifies no full-frame cover on delay/pause and same-session reconnect without recreating the GPU engine. All three observer browser cases pass on WebGPU and WebGL. Viewer smoothing runs in WASM. Automatic native cut provenance requires the updated NWRv4 Arena APK; installation and physical-device acceptance are recorded separately.

## Visible4Hz network test

Open `/tools/scene-observer/network-demo.html` on the same local receiver. This is explicitly a simulated source, independent of the physical Quest and native ingress. Both iframe runtimes receive identical coherent NWR packets, default4Hz plus150ms delay. Left disables interpolation; right uses the same Rust interpolation as the real viewer, with duration matching the selected packet interval. Controls allow4/8/12/20Hz, delay, jitter, interpolation toggle and a2-second delivery stall. The last state remains visible; recovery coalesces pending updates while ingesting all required source deltas.

`native/neon/src/bin/observer-network-demo.rs` generates the deterministic36-second60Hz source recording in `tools/scene-observer/fixtures/network-demo/`. Five staggered ballistic fruit throws and two independently swinging swords use reproducible irregular timings, directions and pauses. The source embeds child/parent identity mappings in each NWRv4 snapshot; no geometry or motion is reconstructed in JavaScript. Four fruit streams are sliced by the real Rust CPU cutter near the apex: the source object is removed, two capped flesh-colored meshes are published, separate/spin, then expire before a new throw. The visible sword sweep is scheduled to the authored cut time (not headset tracking). Rust authors model geometry and poses; the browser changes only delivery timing and forwards snapshots. The test does not claim that the real Quest connection has that latency or throughput. Hidden tabs pause this local demonstration timeline.

Checks: `node --test tests/scene-observer-network-demo.test.mjs` validates timing/order/stall/queue caps; the maintained `scene-observer-network-demo` browser case verifies actual foreground pixels, a visible raw-versus-smooth difference at4Hz+150ms, and coherent recovery after a2-second stall on WebGPU and WebGL.
