# Neon Arena VR entry and spatial information — 2026-10-24

User confirmed Quest and reported the information panel covering VR entry even after hiding its text. The shared DOM diagnostics host remained present after collapsing its content and inherited the arena's generic `aside` layout. The arena now uses the core thin SDK directly, creates no desktop diagnostics host, and scopes its menu CSS. The large VR entry with a headset icon and immediate status/error text is at the top of the menu.

The existing authored native diagnostics node is parked before/after XR and bound to the shared right-foot reference offset `[0.9,0.35,-1]`, size `0.66 x 0.40m`, inside XR. A preparation checkbox controls its visibility. Rust still owns following, orientation, live metrics and both-eye rendering; no JS simulation/RAF or Rust changes were introduced. The node is excluded from desktop-label mirroring: visual inspection caught negative scale captured by the initial binding, now fixed with an explicit positive-scale regression. Native screen-space debug overlay remains off.

## Evidence

- Typecheck and diff checks pass. Existing release WASM reused unchanged: `43c94a078d074a1a359f3c75a208d5febfb6f11f798f6001a22345a3c8c5cdc5`.
- Final candidate: **5/5 maintained browser cases PASS**: actual entry GL, Arena desktop on WebGPU and WebGL, Arena full simulated XR GL and direct WebGPU XR. Entry test clones the real packaged HTML/CSS and imports the real main module. It performs actual browser clicks rather than calling enter directly, checks desktop/mobile hit targets, no residual DOM stats host, headset icon, native positive scale/anchor, actual both-eye panel pixel contribution, hide/show, gaze, tracking loss/recovery, exit and a second actual click. **25 entry assertions passed**.
- Inspected actual menu and stereo panel captures. Earlier Arena regression expected the old fixed gravestone location; changed to assert the new intended player-foot anchor. Initial report retained at `output/arena-vr-info/first-arena-regression.json`.
- Astra scoped source review found no remaining issue after visibility text and mirroring corrections. Source review and simulated XR are distinct from real Quest acceptance.
- Production application `d51271cfc9` on existing HTTPS8443. Changed payloads: Arena HTML/main/scene plus the baked application identifier in Rome HTML. Exact served bytes of those files, manifest and unchanged WASM verified with the local CA and no-store headers. All22 scenes retained; previous production stored under `output/arena-vr-info/previous-production-bdd7929e03`.
- Quest diagnostic capture now includes ready state and entry-click events. Available device logs contained older Quest capability records, no fresh entry failure tied to this release. Physical Quest acceptance remains pending; user browser tab was not accessed or refreshed. No commit/push/CI.

Local evidence: `output/arena-vr-info/{delivery,final-browser-report}.json`, `neon-arena-entry-before-vr.png`, `neon-arena-entry-spatial.png`.
