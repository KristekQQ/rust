# Aurelia source assets

Converted from the local project
`/Users/Honza/Documents/Codex/2026-10-22/ude/outputs/anticke-mesto/dist`.
The source was read only. `source-manifest.json` records SHA-256 hashes of every
conversion input before and after export; both snapshots match.

- `rim-city.glb`: the actual deterministic `createCity(THREE)` geometry, not a
  recreation. It contains 62,076 triangles, 19 meshes and 19 materials; bounds
  are `[-88, -0.6000000238418579, -93]` to `[88, 17.25, 69]` metres, Y up.
  It excludes character roots and decorations. Six named perspective cameras
  preserve Three.js right-handed `lookAt` orientation, 65-degree vertical FOV,
  near 0.08 and far 350. Aspect ratio is intentionally omitted for the viewport.
- `rim-decorations.glb`: original character name plaques (including embedded
  CanvasTexture pixels) and translucent ground shadow discs, at their original
  holder transforms; eight meshes, 136 triangles. No character meshes.
- `blade-wraith.glb`, `core-tank.glb`, `drone-orbiter.glb`, `madmechanic.glb`:
  byte-identical source copies, preserving all original animation clips.
- `layout.json`: original landmarks, exact character holder/root transforms,
  normalized heights, animation names and named camera node indices. All model
  root/holder quaternions are identity; each root scale is uniform. Combine
  holder and root positions when placing a separately loaded original GLB.
- `source-gate.png`: 1280×800 reference rendered from the original city and
  character tree, camera `[0, 1.7, 42]` facing -Z, with source lighting, fog,
  ACES exposure and shadows. Characters are in the initial unadvanced pose.
  The source render settings are recorded in `layout.json`; this is source
  Three.js evidence, not evidence of Rust rendering parity or headset FPS.

All six GLBs are self-contained binary glTF with no external buffers or images.
The runtime example loads these static artifacts using the Rust renderer. It
requires no Three.js; Three.js and Puppeteer are conversion tools only.

## Reproduce and check

Run from the repository root, with its installed Puppeteer and native Node:

```sh
node assets/models/rim/export-source.mjs /path/to/anticke-mesto/dist
node assets/models/rim/verify-assets.mjs
```

The exporter binds an ephemeral loopback HTTP port and opens a headless browser.
It requires permission to launch Chromium and bind localhost. It imports the
source's vendored exporter, loader and Three.js, snapshots input hashes, exports
city and decoration GLBs, copies character files and writes metadata. It rejects
unexpected city geometry or source drift. The checker validates GLB structure,
embedded dependencies, camera projection, city counts, hashes and animation
names. Browser acceptance of the Rust example is a separate integration check.

## Provenance and licensing

The source README describes Aurelia as an authored, stylized Mediterranean
city, rather than an archaeological reconstruction. It says the first three
characters came from `rust/assets/models/neon-enemies`, and Madmechanic was
created as an original Blender model. These are source-project statements;
this conversion does not establish copyright ownership or grant a new license.
No separate redistribution license for the city or characters was found in
that source project. They are reused here at the user's explicit request.

The vendored Three.js r170 conversion code is MIT licensed; its original
copyright and license text are preserved in `LICENSE-three.txt`.
