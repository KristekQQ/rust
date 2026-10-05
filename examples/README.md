# JavaScript examples

Otevři http://127.0.0.1:8000/examples/ při spuštěném `scripts/run-local.sh`.
Galerie nabízí základní scénu, dva barevné stíny, zrcadlo, orbitující světla
a 10 000 krychlí pro frustum culling. Přepínač „Hlavní výstup přes texturu“
ukazuje offscreen render a následnou prezentaci v canvasu.

Všechny runtime scény jsou definované v `scenes.js` prostřednictvím JS/TS SDK.
Rust udržuje objekty a jejich ID, počítá transformace, animace, světelné orbity,
kamery, frusta, cache a renderovací průchody. JS nevykresluje ani nepočítá
simulaci. `spin` a `orbit` předají parametry jednou; běh řídí Rust.

```js
import { initEngine } from '../js/engine.js';
import { mirror } from './scenes.js';
const engine = await initEngine(document.getElementById('gpu-canvas'));
mirror(engine.scene);
engine.set_render_effects(true, true);
```

V galerii běží pouze jeden canvas a jeden engine. Přepnutí scény vymaže
předchozí objekty; kapacita dávkových bufferů může zůstat pro další použití.
„Pozastavit simulaci“ zastaví animace, hlavní pohled se dál vykresluje.
„Ukončit render“ znovu načte prázdnou galerii bez inicializace WASM/GPU.
Zavření karty rovněž ukončí její render. Neotevírej více galerií při běžném
prohlížení. Testovací tlačítko ověřuje všech pět scén v jednom engine.

`Scene.loadExample/loadEffectsDemo/loadCullingDemo` zůstávají jako pohodlné
JS recepty pro starší stránky. Rust už nemá runtime exporty pro tyto ukázky;
`src/test_scenes.rs` obsahuje výhradně nativní testovací fixtures.
