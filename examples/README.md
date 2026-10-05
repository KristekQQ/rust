# Samostatné příklady pro vlastní aplikaci

Spusť `scripts/run-local.sh` a otevři http://127.0.0.1:8000/examples/.
Rozcestník je statické HTML bez canvasu, importů enginu a GPU práce.
Každý příklad je samostatná stránka:

- `basic/`: první scéna, základní importy a inicializace.
- `shadows/`: dva stíny, statické objekty a Rust animace.
- `mirror/`: rozšíření ukázky stínů o render odrazu do textury.
- `lights/`: světelné orbity počítané Rustem.
- `culling/`: 10 000 krychlí vytvořených jednou; Rust vybírá viditelné instance.

Každá složka obsahuje `index.html`, `main.js` a `scene.js`.
`main.js` ukazuje po jednotlivých krocích skutečné importy, získání canvasu,
await inicializaci enginu, nastavení výstupů, sestavení scény a resize.
`scene.js` obsahuje objekty a příkazy pro animace. Rust drží jejich stav,
počítá simulaci, kamery, culling, cache a rendering. Recept se neopakuje každý frame.

## Minimální vlastní aplikace

HTML potřebuje pouze canvas a vstupní modul:

```html
<canvas id="gpu-canvas"></canvas>
<script type="module" src="main.js"></script>
```

V `main.js` jsou skutečné importy (cesty platí při kopii uvnitř `examples/`):

```js
import { initEngine } from '../../js/engine.js';
import { createScene } from './scene.js';

const canvas = document.getElementById('gpu-canvas');
const engine = await initEngine(canvas);
createScene(engine.scene);
```

Vlastní `scene.js` může začít takto:

```js
export function createScene(scene) {
    scene.clear();
    const cube = scene.createCube([0, 0, 0]);
    cube.spin([0, 1, 0]); // Jednorázový příkaz; další snímky počítá Rust.
    scene.createLight({ position: [2, 3, 2], color: [1, 1, 1] });
    return { cube }; // Handle s ID může aplikace použít pro další příkazy.
}
```

## UI je samostatný nástroj

`shared/tools.js` poskytuje `createTools(container, engine, options)`.
Obsahuje výhradně DOM panel, události ovládání a čtení statistik.
Není součástí rendereru ani SDK a engine jej neimportuje. Ve vlastní hře můžeš
odstranit import a volání `createTools`, HTML `<aside>` a jeho cleanup.
Scéna dál funguje. Herní UI si vytvoříš vlastním modulem; mění engine přes SDK.

Žádné `scene.loadExample()` metody nejsou potřeba: tvoje aplikace importuje
konkrétní `createScene` a volá ji sama. SDK nezná ukázky.

„Pozastavit simulaci“ zastaví pohyb, ale hlavní pohled dál renderuje.
„Ukončit render“ naviguje na statický rozcestník a zruší stránku s WASM/GPU.
Otevírej příklady ve stejné kartě, pokud nechceš více běžících enginů.
