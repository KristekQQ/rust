# Neon enemies

Three native Blender modular reference interpretations: Drone Orbiter, Blade Wraith and Core Tank. Models include curved armor, smooth round surfaces, sharp machined edges, true nested articulation and authored action phases.

Open this folder through the repository HTTP server. `main.js` is a thin UI/SDK command layer; Rust owns glTF parsing, hierarchy, TRS animation sampling and rendering. The 200ms DOM timer only reads native playback metadata to display a timeline; removing it does not affect model animation.

Select a model and energy color, then inspect:

- **Idle**: continuous patrol/stabilization.
- **Run**: continuous in-place locomotion; Wraith running, Drone flight, Tank hover movement.
- **Attack2 / Attack3 / Attack4**: overhead cleave, thrust and spin for Wraith; single/paired plasma shots, three paired volleys and charged salvo for Drone/Tank.
- **Attack**: model-specific anticipation, dive/sword sweeps/cannon recoil, recovery; one-shot by default.
- **Explode**: impact, delayed part separation, debris settles and stays separated.
- **Assemble**: reverse destruction, returning to neutral.

The repeat checkbox enables action looping. Timeline seeking pauses the native clip for inspection. Speed and same-model color changes preserve time; switching model resets to Idle. Completed actions remain at their final pose; Replay starts them again. Camera presets show 3D/front/rear; action views widen to fit moving blades and debris. Download exports the current portable color GLB.

Source assets/native Blender files are in `assets/models/neon-enemies/`. The floor is separate. Projectiles, Explode and Assemble are authored demonstration clips, not arena damage/collision gameplay. Movement through the world is supplied by the host game. Color variants are separate identical-geometry GLBs loaded one at a time; no GPU instancing or measured headset budget is claimed.

Targeted browser acceptance: `node scripts/test-browser.mjs --page neon-enemies --backend all --capture`. Checks load every color and compare rendered pixels, action phases, all eight native clips, one-shot endpoint hold, assembly/reset, playback clock and rear geometry. Desktop acceptance does not establish Quest performance.
