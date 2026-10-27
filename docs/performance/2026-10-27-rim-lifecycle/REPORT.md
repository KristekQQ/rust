# Rim disposed-runtime restoration — 2026-10-27

The reported launch failure remains underspecified: clarification of scene/backend and page versus XR entry is pending. This fixes one independently reproduced lifecycle defect, not a claim that every possible Quest startup failure is resolved.

Rim disposed its WASM engine on pagehide, but later blur, key-up, visibility and XR-end events still sent commands. A restored document retained that disposed runtime. Before the fix, the maintained runner reproduced four disposed-engine errors and timed out waiting for restoration.

`examples/rim/main.js` now guards callbacks after disposal and reloads a persisted restored document. The SDK continues rejecting stale handles. `scripts/test-browser.mjs` extends rim-refresh with simulated pagehide/pageshow and post-disposal events, followed by an actual Play click and active-game assertion. This is simulated browser lifecycle wiring, not proof of real BFCache admission or headset XR entry.

Typecheck and both minified production rim-refresh cases pass (WebGPU and WebGL). Diff whitespace check passes. The renderer/WASM is unchanged from the measured actual-light shader release; this fix makes no new FPS claim.

Live app `7ee4df8ebe`, WASM `dcce4c0c`, 254 verified payload records. Installed only the owned fallback production package using directory renames, preserving the prior app at `output/production-before-rim-lifecycle-20261027`. Existing shared HTTPS server was not restarted; primary Neon work was untouched. Six actual HTTPS payloads match their accepted hashes with TLS and no-store checks. Evidence: [before regression](reproduce-report.json), [fixed cases](fixed-browser-report.json), [deployment](deployment.json).

Same URLs on home LAN 192.168.0.17:8443. No commit, push, CI or physical Quest acceptance.
