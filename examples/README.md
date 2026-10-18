# Samostatné příklady pro vlastní aplikaci

Spusť `scripts/run-local.sh` a otevři http://127.0.0.1:8000/examples/.
Rozcestník je statické HTML bez canvasu, importů enginu a GPU práce.
Každý příklad je samostatná stránka:

- `webxr/`: VR pro Meta Quest 3, sledování hlavy a stereo Rust rendering; [HTTPS a USB spuštění](../docs/WEBXR.md).
- `basic/`: první scéna, základní importy a inicializace.
- `cutting/`: pět jablek, režim meče v ruce/plánování, zelená stopa řezu a volně ovladatelná kamera (pravé tažení, kolečko, WASD). Levý klik seká shora dolů; kusy lze řezat znovu. Volitelný hybrid WebGPU compute + CPU; Auto používá naměřenou rychlejší CPU cestu. [Ovládání, API, měření a omezení](../docs/CUTTING.md).
- `shadows/`: dva stíny, statické objekty a Rust animace.
- `mirror/`: rozšíření ukázky stínů o render odrazu do textury.
- `lights/`: světelné orbity počítané Rustem.
- `postprocessing/`: texturovaná kaluž, SSR a god rays; vlastní volitelné UI.
- `culling/`: 10 000 statických krychlí; Rust vybírá viditelné instance přes BVH. F3 přepíná panel FPS/CPU/culling přímo v obrazu.
- `fox-control/`: fullscreen a zachycená myš, WASD pohyb, Shift běh, mezerník skok, R návrat na start. Pevné plošiny a stěny, gravitace a kolize přes BVH jsou v Rustu; Escape hru pozastaví.

Každá složka obsahuje `index.html`, `main.js` a `scene.js`.
V `fox-control` také F3 přepíná Rust-renderovaný informační panel. Výchozí stav
je vypnuto; panel bez zapnutí negeneruje text ani další GPU práci.
`main.js` ukazuje po jednotlivých krocích skutečné importy, získání canvasu,
await inicializaci enginu, nastavení výstupů, sestavení scény a resize.
`scene.js` obsahuje objekty a příkazy pro animace. Rust drží jejich stav,
počítá simulaci, kamery, culling, cache a rendering. Recept se neopakuje každý frame.
Každý `main.js` obsahuje vlastní bootstrap, konfiguraci a přidání své scény.
Můžeš tedy vzít jeden příklad jako samostatný základ a upravovat ho bez toho,
aby závisel na jiném příkladu. Objekty, světla a animace definuje každý příklad
ve vlastním `scene.js`; sdílené je jen volitelné obecné UI a CSS.
Při kopírování uprav cesty k SDK, UI a případným modelům podle umístění své aplikace.

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

Postprocess API a přidání efektu: [POSTPROCESSING](../docs/POSTPROCESSING.md).
