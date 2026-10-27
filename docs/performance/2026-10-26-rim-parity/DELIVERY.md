# Verification and local delivery — 2026-10-26

- Retained release WASM: `a54c09c90f591f16399196a0806fb0945161e045f4340d1bf3bd714f16df9317`; source fingerprint `b6bc0aeef47435635542d7a1c4ee970c177034663cbfe68011a080807e96966b`. Branch `codex/wasm-render-performance`, HEAD `64bfda680483e71f6aeb6bf902e63b29b3e90791`, owned isolated checkout. Changes remain uncommitted and unpushed; no CI run or physical Quest acceptance is claimed.
- Strict WASM Clippy (`-D warnings`), TypeScript check and 32 performance-driver/PCF numeric unit tests passed. [Logs](evidence/checks/). The separate normal-map oracle covers 32 states on each actual backend; the rejected integer-power experiment has additional 44-state/numeric checks.
- Browser-tested the isolated **release/minified production** package: 18 cases passed across actual WebGPU/WebGL, covering scene appearance, shadow cache, glTF full/PBR/skinning, SDK actual planar reflection, material-cache stability, instancing, mirrored transforms, Rome gameplay/cuts, simulated direct WebGPU XR and Neon Arena entry. [Report](evidence/production/browser-report.json).
- Four additional actual Rome entry/normal-refresh cases passed on both backends. [Entry report](evidence/production/rim-entry-report.json). These are browser and simulated-XR checks, not a physical headset session.
- All six result selectors, original before/after images, overlay alignment, slider behavior and mobile no-overflow passed. [UI checks](view-verification.json), [desktop](report-desktop.png), [mobile](report-mobile.png). A collector variable-name error and mobile select overflow were caught and corrected before delivery; raw benchmark data was unchanged. The final pixel text explicitly counts pixels **above tolerance**, not all changed pixels.

## Live endpoints

- [Native WebGPU Rome](https://192.168.11.134:8443/examples/rim/?backend=webgpu)
- [Native WebGL2 Rome](https://192.168.11.134:8443/examples/rim/?backend=webgl)
- [Unchanged original Three.js Rome](https://192.168.11.134:8878/examples/rim/)

`npm run serve:quest` prepared and serves `output/production/`: application `ee1b8d0017`, release WASM `a54c09c9`. Its complete file manifest matches the browser-verified staging package. Five actual HTTPS payloads (HTML, Rome main, SDK, bindings, WASM) have matching hashes and `Cache-Control: no-store`; HTML exposes the correct engine/application markers. Certificate chain and current IP were verified against the existing local development CA, without changing system or headset trust. [Deployment check](evidence/production/deployment-check.json), [production manifest](evidence/production/production-manifest.json).

The first restart on old `192.168.0.17` failed because this IP is no longer assigned to the Mac. The active LAN is `192.168.11.134`. The user explicitly approved replacing the server on that address with the verified isolated package. Main-checkout sources and its previous production package were preserved. The previous owned `.17` package is also backed up locally at `output/production-before-a54-20261026/`.

Original Three's server was restarted only to load the certificate for the current IP. All 30 original application files match the frozen reference, and the actual HTTPS entry bytes match too. [Three live check](evidence/production/three-live-check.json). No URL version parameter is needed; normal refresh uses no-store responses.

The compact experiment archive is 18.8 MB; its 1,836 included file hashes and all 24 copied final timing inputs passed [integrity verification](evidence/evidence-integrity.json). Complete probe images are additionally backed up locally; omitted passing-image hashes are listed in the archive manifest.

Desktop timings do not establish Quest FPS. The direct Three comparison still differs in AA/shaders/shadows and is not a proof of equal graphics. See [results](REPORT.md) and [independent review](REVIEW.md).
