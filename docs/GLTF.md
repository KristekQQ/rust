# Import statických glTF modelů

`scene.loadModel()` načte glTF 2.0 (`.gltf`) nebo binární GLB (`.glb`) přes SDK a vrátí běžný handle scény. Fetch zajišťuje JavaScript; parsování, převod geometrie, bounds a její vykreslení řídí Rust.

```js
const arch = await scene.loadModel(new URL(
    '../../assets/models/kenney-nature-bridge-stone-round/bridge_stone_round.glb',
    import.meta.url,
));

arch.setPosition([0, -0.6, 0])
    .setRotation([0, Math.PI / 4, 0]) // radiány
    .setScale([6, 8, 4])
    .setRenderOptions({
        isStatic: true,
        castsShadow: true,
        receivesShadow: true,
    });
```

V `.gltf` lze použít externí buffery, například vedlejší `.bin`; SDK načte všechny URI bufferů relativně k URL modelu. V GLB se použije vložený BIN blok. Celkový přenos modelu a bufferů je omezen na 128 MiB. Načítání je asynchronní, proto ho `await` před použitím handle. `scene.clear()` zruší čekající importy; soubory po HTTP přesměrování používají finální URL. `new URL(..., import.meta.url)` naváže cestu k modulu příkladu, nikoli k aktuální stránce.

## Podporovaná geometrie a materiály

Loader prochází výchozí glTF scénu a sloučí její mesh primitiva do jediné statické geometrie. Node transformace se započítají do vrcholů při importu; pozdější `setPosition`, `setRotation` a `setScale` mění transformaci celého výsledného objektu. Osa a směry scén se nemění. Bounds se počítají z importovaných vrcholů a Rust je aktualizuje při transformaci objektu.

Podporovány jsou pouze neprůhledné primitiva `TRIANGLES`. Pozice musí být `float VEC3`; normály jsou volitelné (při absenci je Rust dopočítá). Barvy vrcholů `COLOR_0` mohou být `float` nebo normalizované `uint8/uint16` VEC3/VEC4. Barva vrcholu se násobí materiálovým `baseColorFactor`. Bez těchto barevných vstupů se použije bílá.

Jde o barvu pro stávající osvětlení enginu, ne o glTF PBR renderer. Metallic/roughness se nepřenáší. Model nesmí odkazovat textury ani obrázky; materiály s `BLEND`, `MASK` nebo `doubleSided` jsou odmítnuty. Požadovaná glTF rozšíření nejsou podporována; Draco, meshopt a quantized geometry jsou odmítnuty.

## Statické vykreslení a efekty

Importovaná geometrie sama o sobě nemá animovaný runtime. Skelety (`skins`) a morph targets jsou odmítnuty. Pokud dokument obsahuje animace, loader je nepřehrává a použije výchozí statickou pózu uzlů. Pro pohyblivé objekty použij transformace scény nebo její Rustem řízenou animaci.

`isStatic: true` označuje objekt pro statické cache stínů a odrazů; nezamyká transformaci. Změna transformace statického objektu invaliduje příslušnou cache. `castsShadow` určuje účast v průchodu casterů; `receivesShadow` řídí vzorkování stínů při osvětlení povrchu. Bounds modelu používá Rust pro frustum culling a vykreslovací průchody včetně stínů a odrazů. Pro god rays model vrhá stín do mapy prvního světla, kterou efekt vzorkuje. Reflektivita patří na povrchy scény přes `reflectivity`; samotný model není obrazovkový SSR ani obecný texturovaný materiál.

Limity parseru jsou 128 MiB vstupu/bufferů, 1 000 000 vrcholů a 3 000 000 indexů. Překročení limitu nebo nepodporovaná data skončí chybou při načtení.

## Příklad a asset

Kompletní scéna s komplexním interiérem katedrály, kaluží, SSR a god rays je v [`examples/postprocessing/`](../examples/postprocessing/); viz zejména [`scene.js`](../examples/postprocessing/scene.js) a [`main.js`](../examples/postprocessing/main.js). Původní malý testovací model je kamenný oblouk [Kenney Nature Kit](../assets/models/kenney-nature-bridge-stone-round/bridge_stone_round.glb). Balík je vydán pod CC0; původ a přesná přiložená licence jsou v [README modelu](../assets/models/kenney-nature-bridge-stone-round/README.md) a [LICENSE.txt](../assets/models/kenney-nature-bridge-stone-round/LICENSE.txt). Kenneyho [stránka Nature Kit](https://kenney.nl/assets/nature-kit) uvádí licenci CC0.

Aktuální postprocessing příklad používá [geometrii katedrály Šibenik](../assets/models/sibenik-cathedral/README.md): 72 513 trojúhelníků bez textur či animací. Tento asset je CC BY-NC, pro nekomerční demonstraci. `engine.setCameraView(position, target)` nastaví volnou kameru pomocí Rust výpočtů; SDK očekává dva Vector3 a odmítá shodný bod nebo svislý směr.
