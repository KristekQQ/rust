# glTF modely, PBR a animace

Model načti přes `await scene.loadModel(url)`. JavaScript zajišťuje přenos souborů a drží pouze ID; Rust parsuje glTF/GLB, vlastní hierarchii uzlů, materiály, pózu, animace, bounds i vykreslování. Import nikdy automaticky nezmění kameru.

```js
import { initEngine } from '../js/engine.js';
const engine = await initEngine(document.querySelector('#gpu-canvas'));
const model = await engine.scene.loadModel(new URL('../assets/models/fox/Fox.glb', import.meta.url));
model.setScale([0.035, 0.035, 0.035]);
model.playAnimation('Walk', { loop: true, speed: 1 });
// Všechny následující příkazy mění Rust runtime:
model.pauseAnimation();
model.seekAnimation(0.5); // sekundy
model.pauseAnimation(false);
model.stopAnimation(); // výchozí póza souboru
console.log(model.info); // snímek metadat a přehrávání z Rustu
```

## Import a materiály

`.gltf` a `.glb` mohou obsahovat externí buffery, relativní PNG/JPEG, data URI i vložené bufferView obrázky. URI se řeší vůči finální URL modelu po přesměrování. SDK načítá zdroje postupně; celkový přenos je omezen na 128 MiB. `scene.clear()` zneplatní čekající importy a zruší jejich stahování i čtení streamů. Chyba importu nepřidá částečný model do scény.

PBR používá glTF metallic–roughness: base color, metallic/roughness, normal map, occlusion a emissive včetně faktorů. Base color/emissive jsou sRGB; ostatní mapy jsou lineární. Renderer respektuje UV sady, barvy vrcholů, tangenty, wrap/filter samplery a mipmapy. Podporuje OPAQUE, MASK s alphaCutoff, BLEND a doubleSided. Materiály jednotlivých primitiv zůstávají oddělené. Geometrie zachovává uzly a sdílené mesh reference.

Indexovaná i neindexovaná geometrie podporuje body, čáry, smyčky, stripy, trojúhelníky a fan. Loader čte sparse accessors. Při absenci normál počítá ploché normály z deformované geometrie; normal mapa bez tangentů používá odvození báze ve fragment shaderu. Body a čáry mají neosvětlený materiál dle glTF.

Osvětlení je přímé GGX/Smith/Schlick ze světel enginu a jednoduchá ambientní složka. Zatím není environment IBL ani HDR tonemapping; kovový model proto nemá stejné okolní odrazy jako prohlížeč s HDR panoramatem. `reflectivity` enginu je samostatná možnost pro stávající SSR/rovinný odraz.

## Hierarchie a animace

Každý model má jedno ID scény a uvnitř Rustu vlastní strom uzlů, mesh primitiva, skiny a morph targety. Transformace handle mění kořen celého modelu. Klipy podporují translaci, quaternion rotaci, scale a morph weights; interpolace STEP, LINEAR (rotace SLERP) a CUBICSPLINE. Skinnování používá inverse bind matrices a všechny sady joint/weight vlivů. Nejprve se aplikuje morph, potom skin a transformace uzlu.

`playAnimation(indexOrName, {loop, speed})`, `pauseAnimation(bool)`, `seekAnimation(seconds)` a `stopAnimation()` řídí jeden aktivní klip modelu. Smíchávání více klipů není součástí současného API. Rust aktualizuje animace jednou za frame, bez JS animační smyčky. Změněná póza obnoví bounds i statickou stínovou cache; označení `isStatic` animaci nezamyká.

První portable implementace deformuje vrcholy v Rustu na CPU a uploaduje je jen při změně pózy či kořenové transformace. Stejné buffery pak používá hlavní pohled, odraz i stíny. Statický model se každý frame znovu nedeformuje. Mnoho animovaných postav bude vyžadovat další optimalizaci GPU skinningu; tato verze neslibuje maximální výkon davových scén.

`model.setMorphWeights(nodeIndex, weights)` nastaví váhy bez spuštění klipu. `model.selectScene(index)` přepne glTF scénu. `model.useCamera(nodeIndex)` explicitně zapne perspektivní nebo ortografickou kameru modelu (včetně perspektivy bez zfar). `engine.setCameraView()` nebo `set_camera_mode()` vrátí ovládání běžné kameře. Dostupné uzly, scény a klipy jsou v `model.info`.

## Stíny, odrazy a průhlednost

OPAQUE a MASK se vykreslují do hlavního obrazu, pomocné hloubky, odrazové textury a stínových map. MASK používá stejnou texturu a cutoff i ve stínu. BLEND se řadí od vzdálenějších primitiv, nezapisuje hloubku a nevrhá neprůhledné stíny. V hlavním pohledu se přidává po screen-space efektech; SSR a god rays tedy používají hloubku neprůhledné geometrie. Řazení průhledných trojúhelníků uvnitř jednoho primitiva není řešeno.

## Rozsah kompatibility

Cílem je glTF 2.0 core, nikoli všechna existující rozšíření. Nepodporované `extensionsRequired` skončí s konkrétní chybou; volitelná rozšíření používají základní glTF fallback s upozorněním. Draco, meshopt, KTX2/BasisU, transmission, clearcoat a další rozšířené materiály nejsou v této verzi implementované. Podpora PBR není tvrzení o Khronos certifikaci celého importeru.

Velikosti obrázků a bufferů se kontrolují proti limitům zařízení. Soubor, který překročí parserové/GPU limity, se odmítne s chybou. Rozbalená data všech animačních kanálů mají společný limit 64 MiB; jedna póza včetně kopií skin matic a morph vah má limit 64 MiB. Sdílené zdroje se započítávají za každou rozbalenou kopii. Přesné ochranné limity jsou v Rust loaderu a GPU uploadu.

## Ukázky a ověření

- [Samostatná glTF ukázka](../examples/gltf/): animovaná liška a PBR helma; každá scéna je sestavená v JS.
- [Runtime integrační test](../tests/gltf-full.html): externí PNG, UV, MASK/BLEND, morph, kamery, více scén a skin/klipy skutečného modelu.
- [Původní integrační test](../tests/gltf-sdk.html): import, transformace, stíny, odraz a životní cyklus.
- [Postprocessing](../examples/postprocessing/): katedrála Šibenik, zachovaná kamera a světlo u růžicového okna.

Licenční údaje stažených modelů jsou v jejich asset README. Fox má části CC0 a CC BY 4.0. DamagedHelmet a Šibenik jsou nekomerční demonstrační assety; licence enginu se tím nemění.

Specifikace: [Khronos glTF 2.0](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html).
