# Three VR-entry repair — 2026-10-26

Quest user reported no visible VR button. The maintained runner reproduced the defect against production613216e02e: clicking Walk adds body.walking, hiding #intro and its nested #vr button (zero-size rectangle). The button is now in the persistent header, with a44px minimum height and responsive spacing. Only Three HTML/CSS and the native app-build marker payload changed; native WASM and rendering/instancing logic are unchanged.

Targeted minified-production Three acceptance passed: existing eight clone/instance image oracles, refresh/mode/removal/mobile controls, then actual Walk and visible/uncovered VR entry at1280x720,960x540 and390x740. A real automated click reached requestSession with a controlled rejected request. This proves event delivery, not an immersive XR session or physical Quest acceptance.

App d793a99857 is served at192.168.0.17:8443; full254-file manifest matches tested staging, five actual HTTPS decoded payload hashes/no-store/local-CA/IP validation pass. Previous package output/production-before-rim-three-vr-entry-20261026. No performance optimization or fresh FPS claim, commit/push/CI. Compressed before/after/deployment reports accompany this note.
