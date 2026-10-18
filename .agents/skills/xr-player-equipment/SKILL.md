---
name: xr-player-equipment
description: Extend the Rust-owned XRPlayer, head and hand tracking, inventory and generic equipment bindings. Use for shared XR player architecture rather than scene positioning or weapon-specific visuals.
---

# XR hráč a vybavení

Kořen projektu je o tři adresáře výš. Čti pouze dotčené části
[WEBXR.md](../../../docs/WEBXR.md#obecný-hráč-xrplayer) a
[projektového workflow](../rust-webgpu-project/SKILL.md).

- `src/xr_player.rs::XrPlayer` je obecný lokální hráč: hlava, ruce, inventář a vazby
  na existující objekty. SDK jej zpřístupňuje jako `engine.xr.player` / `XRPlayer`.
  Nevytváří automaticky tělo, meče ani herní scénu.
- Hra vybavuje libovolný `SceneObject` přes `player.equip(hand, item, options)`.
  Řezání je explicitní schopnost nástroje (`setCutter`, případně `cutting:true`),
  ne výchozí vlastnost člověka. Konkrétní meče patří do receptu arény.
- Současné konečné řezy a kontakty obsluhují `xr_input.rs`, `blade_physics.rs`
  a `scene.rs`. Samostatná třída Weapon zatím není zavedena; nepředstírej hotový
  přesun tohoto chování. Samotná úprava hráče není důvod přepisovat řezání.
- Rust počítá matice a rychlost; JS předává platformové pózy, události a ID.
  Snapshot `player.state` je diagnostika na vyžádání, bez další JS simulační smyčky.
- Rychlost gripu používá posun ovladače / skutečný čas XR snímků, s krátkou
  historií proti třesu. Špička nástroje není rychlost ruky. Výpadek trackingu,
  neplatný pohyb a nové navázání nesmí vytvořit umělý rychlý švih.
- Zachovej měřítko vybavení, jediného vlastníka transformace na slot,
  atomické odmítnutí neplatné vazby a kontrolu příslušnosti SDK handle.
  `unequip` a `removeItem` nemažou geometrii. Ztráta trackingu objekty skryje;
  odchod z XR zachová inventář, disposal zneplatní SDK.

## Ověření změny

Při zásahu do nativního hráče ověř `cargo test xr_player --lib`; změny předávání
pohybu také přes `cargo test xr_input --lib`. SDK hranici kontrolují
`tests/sdk-unit.test.mjs` a `tests/xr-simulator-unit.test.mjs`.
Po změně WASM obnov matching `pkg/` podle projektového workflow a vyber skutečně
dotčené browser scénáře `webxr-contract`, `webxr-simulation`, `neon-arena-xr`.
Ověř libovolný předmět bez řezání, prázdnou ruku, oba sloty, výpadek/reentry
trackingu a původní herní interakci. Simulace nepotvrzuje skutečný headset.
