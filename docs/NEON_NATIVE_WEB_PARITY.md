# Neon Arena native/web parity

Honza's corrected goal is the same **gameplay, fun and functionality** in the
browser as the installed native game. This delivery is not a claim of FPS gain.
The proposed native hybrid experiment was stopped before any source edits.

Shared Rust game: `games/neon-core/{content,game,scores}`. Native compatibility
modules reexport it. Browser CPU adapter: `games/neon-browser-worker`; JS only
loads assets, forwards tracked inputs and applies ordered binary publications.
The game program/assets are the native rev51 export in
`examples/neon-arena/parallel-assets/`. Default web entry is `index.html` with
`parallel-main.js`; previous browser UI remains in `legacy.html`.

Same cyber katanas, glass fruit variants, minute program (24 cues /140 targets),
physical cuts, hit points and stock banking, cut feedback/dissolve, A restart,
Y info toggle, feet-anchored stereo diagnostics, saved TOP5. Browser storage uses
the existing notebook scoreboard API and a local fallback. Native private TOP5
and browser/server TOP5 are separate stores; no native score migration occurred.
Desktop text facing differs from immersive views; Rust updates complete text
roots before font layout. Both backends use the same game and input path.

Validation uses `neon-native-parity` on both WebGL2 and WebGPU, plus
`neon-native-parity-xr` for simulated stereo tracking, panels, recentering and
restart. The shared game checks cover a complete minute, cuts, score banking,
restart and TOP5. `neon-arena-entry` checks the actual default page and VR entry;
`neon-arena-legacy` retains SDK/editor renderer coverage. These browser and
simulated XR checks do not establish physical Quest FPS or player comfort.

Build with `npm run build:production` and deliver to Quest with
`npm run serve:quest`. The package contains the engine, separate game worker,
native-exported scene/assets and optional observer publisher runtime. Use the
browser runner's `--production-root output/production` option to verify packaged
bytes. For a separately served Neon checkout, `QUEST_SHARED_ROOT` selects the
canonical production root and `QUEST_SCORE_FILE` selects the existing scoreboard;
without this override, serving uses this checkout's own production package.

Publishing this integration does not install or replace the headset application.
Physical acceptance must identify the installed APK and actual device evidence;
the user's earlier stutter/feel concern remains separate from browser parity.
