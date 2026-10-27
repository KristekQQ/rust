# Rome visual comparison — 2026-10-27

Managed archived checkout restored at the same path, preserving saved source and reports. Ignored runtime outputs were absent. Rebuilt release source535ac6bc produces exactly the previously measured WASMdcce4c0c and bindingse248ba20. Five original Three vendor dependencies restored from local authored source after matching every recorded original hash. No renderer/gameplay optimization or stopped restart-button fix is included.

New page `examples/rim-compare/` provides a keyboard-operable image divider, native backend selector, full A/B switch, actual gameplay links and separate historical measurement tables. It displays actual unretouched canvas captures from the maintained performance CLI: same southern-gate camera,1280×720,DPR1, static time-zero characters, no active combat. One1s warmup/1s smoke sample per capture; not a new repeated benchmark. Native GPU and GL paths and the original Three adapter all rendered without errors/fallback. All are approximately60Hz capped at this light viewport; this cannot establish comparative headroom.

Visual inspection: composition and colors are close. Three has visibly finer roof, temple and paving edges in this view; native GPU/GL are similar. Existing Three hardware MSAA versus native FXAA, PBR/shadow differences and independent gameplay remain explicit. A still image cannot establish motion shimmer, input latency or headset comfort. No equal-quality/full-game/Quest performance claim.

Previous26October512-model2880×1920 repeated data is reused with its date and limits. Direct paired nativeGL15.931FPS/308calls versusThree24.754/108,Three wins. Separate self-pairs nativeGL15.221→20.081 andWG37.201→38.907; oneWG pair adverse. These are separate campaigns; do not combine their absolute values into one three-way ranking. See ../2026-10-26-light-classes/REPORT.md. Calls5266→308 belongs to the earlier batching change.

Raw capture reports and SHA256 manifest are retained here. Production acceptance/deployment evidence is appended after verification. No commit/push/CI or new physical-headset acceptance.

## Acceptance and delivery

All three maintained minified production cases PASS: actual Rim WebGPU, actual Rim WebGL2 and Three stress/VR-button control. Package app93b31ba5f9,260 payload files; nine actual HTTPS hashes/TLS/no-store checks PASS on current192.168.0.17:8443. Installed by directory rename; sharedserver79190 and primary Neon unchanged. Browser-only VR button tests do not prove physical Quest entry.

Unified in-app browser verified native selector, A/B switch and divider50 state. Its fresh profile rejects the development TLS authority; no warning was bypassed or trust setting changed. Desktop preview uses the maintained production HTTP server on127.0.0.1:55019, while public Quest links remainHTTPS8443. Initial benchmark fixture server lacked CSS MIME support and was replaced for preview by the standard production server; deployedHTTPS CSS independently returns200 text/css. Correctly styled preview inspected and screenshot saved.
