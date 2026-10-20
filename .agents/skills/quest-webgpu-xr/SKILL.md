---
name: quest-webgpu-xr
description: Implement or diagnose direct WebGPU WebXR on Quest, including feature detection, projection texture imports and headset error capture. Use for XR rendering/runtime support, not player equipment or scene placement.
---

# Quest: přímé WebGPU XR

Kořen projektu je o tři adresáře výš. Použij [workflow enginu](../rust-webgpu-project/SKILL.md),
[WEBXR.md](../../../docs/WEBXR.md) a relevantní scénáře z [ověřovací matice](../../../docs/DEVELOPMENT.md).

- Uživatel 2026-10-18 potvrdil funkční WebGPU VR v neon aréně na Quest 3 / Browser 149.1.
  Potvrzení následovalo po zapnutí **WebXR Experiments** a opravě importu barevné XR textury.
  Verze prohlížeče ani desktopový WebGPU nejsou důkazem podpory konkrétní XR relace.
- Uživatel potvrdil baseline `b07ae09b` s opravou SSR artefaktů na Browser 152.1.
  Dokonči efekty v engine-owned per-eye color/depth cílech a obě browserové slices
  předej jedním finálním stereo submit. Depth attach respektuje `ignoreDepthValues`.
  Nový frame graph/profiler sám nepředstavuje nové potvrzení z headsetu;
  pro rozšíření prostředků použij [frame-pipeline](../frame-pipeline/SKILL.md).
- `?backend=auto` používá WebGPU, pokud je dostupné; `?backend=webgl` zůstává kompatibilní cesta.
  Pro headset použij důvěryhodné HTTPS. Nezaměňuj vykreslený desktopový canvas za VR.
- Při chybě nejprve čti čerstvé Quest záznamy `output/diagnostics/browser.jsonl`,
  především `xr-capabilities`, `xr-enter`, `xr-frame`, `xr-start` a `xr-first-frame`.
  Rozlišuj chybějící `XRGPUBinding`, odmítnutou feature `webgpu` a selhání snímku.
  Na stránce je volitelná **Diagnostika WebGPU VR**, která testuje nativní relaci mimo engine.
- Neslibuj, že samotná aktualizace nebo seznam přepínačů podporu vyřeší. Quest 149.1
  bez WebXR Experiments binding nevystavoval. Názvy a dostupnost Layers voleb se liší;
  ověř skutečný stav zařízení, nezapínej nesouvisející XR přepínače odhadem.
- Rust vlastní render/simulaci. `js/webxr.js` předává pózy a frame-scoped nativní textury.
  Adaptér musí být `xrCompatible:true` už před vytvořením zařízení; relace vyžaduje
  `requiredFeatures:['webgpu']`. Použij stejné zařízení pro renderer i `XRGPUBinding`.
  Lokální wgpu patch je popsán v [PATCH.md](../../../third_party/wgpu/PATCH.md).
- WebGPU XR projekce má hloubku 0..1: znovu ji nepřeváděj z OpenGL. Oko vybírá
  `getViewDescriptor()` jako jediný 2D řez texture array. Browserové textury nedrž
  přes snímky a nenič je. Přímá cesta vyžaduje celé řezy. Postprocessing běží přes per-eye mezicíle;
  finální pipeline musí odpovídat skutečnému formátu XR řezu (bez sRGB suffixu).
- Výstupní barvy XR: engine-owned eye stage má sRGB render view a UNORM source
  view. Finální copy čte zakódované bajty z UNORM view, bez další gamma korekce.
  Používej stage i bez efektů; přímý lineární zápis do browser UNORM ztmavoval obraz.
  Browser textury nemají povolené sRGB view reinterpretace. Ověř kvantitativní
  šedé v `webxr-webgpu`, ne jen přítomnost pixelů; podrobnosti ve frame-pipeline.
- Quest 149.1 vrací u barevné XR textury někdy mip/sample/usage = **0/0/0**.
  `src/render/xr_output.rs` nahrazuje pouze tuto přesnou trojici u barevné přílohy
  známým nastavením projekční vrstvy 1/1/RENDER_ATTACHMENT. Zachovej ostatní validaci
  i přísnou kontrolu hloubkové přílohy; neodstraňuj workaround podle desktopového testu.
- Ověř matching WASM a `webxr-webgpu` na WebGPU (reálné GPU pixely obou očí, nulová
  metadata, clipping a lifecycle), při změně společného hostu i `neon-arena-xr` na WebGL.
  Simulace nenahrazuje headset. Uživatelské potvrzení obrazu nepředstavuje FPS benchmark
  ani důkaz shody barev WebGPU/WebGL; barevné cesty dosud nejsou sjednocené.
