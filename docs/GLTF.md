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

## Shared instances and GPU deformation

`model.createInstance()` creates a new default pose/root from the same immutable
Rust asset. It does not copy the source transform, animation playback or physics.
Configure each instance normally; removing the source does not invalidate its
instances. Calling it on a removed/disposed handle fails. Repeated `loadModel()`
calls are separate imports; use `createInstance()` to share geometry/materials.

```js
const second = model.createInstance();
second.setPosition([2, 0, 0]).playAnimation('Run');
```

Rigid primitives keep immutable local-space GPU vertices. Compatible repeated
rigid draws use a shared per-view instance page; other draws upload only changed
model/normal uniforms. Skinned primitives use a GPU palette on both backends when
there are at most 128 joints, at most 8 nonzero ordered influences per vertex,
no morph targets, and the actual device supports the required 13 attributes and
8 KiB vertex uniform binding. No influence truncation or renormalization occurs.
Other cases retain exact CPU deformation. Normal inverse-transpose is computed
from the weighted skin matrix; tangents, mirrored winding and singular handling
retain the CPU contract. Exact affine vertex means preserve transparency sorting.

Material/geometry sharing is keyed by live `Arc<ModelAsset>` identity. The weak
cache owns no orphaned assets; the last model releases shared resources. Each
instance keeps its own pose, palette and draw state. Compatible opaque rigid
primitives now also use real `draw_indexed(..., 0..instance_count)` instancing on
both backends. Consecutive roots must share their asset and have pairwise disjoint
conservative bounds; batching preserves each model's primitive order. Overlapping
roots, BLEND and skin/morph paths retain independent draws. This conservative
eligibility avoids changing coplanar depth winners or transparency ordering.

Instance data uses 112 bytes and seven vertex attributes (locations 9–15), gated
on actual device limits. Each view/pipeline has separate cached transform and
material buffers; unchanged material data is not reuploaded with transforms.
Removing models invalidates cached pages, including views not rendered again.
`debugStats().gltf` reports actual instanced draws, instances, primitive culling,
write calls and cached batch bytes. Legacy `renderer_stats().drawCalls` retains
its object-batch estimate; actual commands are `debugStats().mainDrawCalls` and
`debugStats().drawCalls`.

Animated render bounds use cached conservative per-joint/rigid bounds with
outward rounding. Morphs and unsafe/unsupported weights/transforms fall back to
exact deformation. Each draw also uses the cached conservative primitive bounds
against its current eye/shadow/reflection frustum. Unsupported primitive bounds
fail open, without adding exact CPU deformation merely to cull. Public CPU
`model_bounds` and cutting geometry remain exact.
`debugStats().gltf` exposes cumulative uploads/deformed vertices/state changes and
live resource counts; `frameProfile()` adds `gltf.animation`, `gltf.bounds`,
`gltf.update` and fallback `gltf.deform` CPU scopes. Scopes can nest; do not sum
nested scopes as frame time. Physical Quest performance must be measured separately.

## Import a materiály

`.gltf` a `.glb` mohou obsahovat externí buffery, relativní PNG/JPEG, data URI i vložené bufferView obrázky. URI se řeší vůči finální URL modelu po přesměrování. SDK načítá zdroje postupně; celkový přenos je omezen na 128 MiB. `scene.clear()` zneplatní čekající importy a zruší jejich stahování i čtení streamů. Chyba importu nepřidá částečný model do scény.

PBR používá glTF metallic–roughness: base color, metallic/roughness, normal map, occlusion a emissive včetně faktorů. Base color/emissive jsou sRGB; ostatní mapy jsou lineární. Renderer respektuje UV sady, barvy vrcholů, tangenty, wrap/filter samplery a mipmapy. Podporuje OPAQUE, MASK s alphaCutoff, BLEND a doubleSided. Materiály jednotlivých primitiv zůstávají oddělené. Geometrie zachovává uzly a sdílené mesh reference.

Indexovaná i neindexovaná geometrie podporuje body, čáry, smyčky, stripy, trojúhelníky a fan. Loader čte sparse accessors. Při absenci normál počítá ploché normály z deformované geometrie; normal mapa bez tangentů používá odvození báze ve fragment shaderu. Body a čáry mají neosvětlený materiál dle glTF.

Osvětlení je přímé GGX/Smith/Schlick ze světel enginu a jednoduchá ambientní složka. Zatím není environment IBL ani HDR tonemapping; kovový model proto nemá stejné okolní odrazy jako prohlížeč s HDR panoramatem. `reflectivity` enginu je samostatná možnost pro stávající SSR/rovinný odraz.

Shader varianty se sdílejí podle přítomných materiálových map (5bit maska), nikoli
podle ID modelu. Chybějící base/MR/AO/emissive mapy mají přímo konstanty
odpovídající fallback texelům. Without `normalTexture`, keep the shading normal
unchanged; a synthetic 128/255 normal would introduce a small unintended tilt.
Real normal textures retain their original decoding and tangent-space path.
OPAQUE hlavní průchod bez clip roviny neobsahuje dosažitelný `discard`. MASK,
BLEND, stíny a průchody s clip rovinou zachovávají původní testy. `clipped` je
samostatný klíč, nezaměňovat s obráceným windingem (`reflected`). Shader moduly
vznikají líně a sdílejí se mezi pipeline včetně rigidní, skinning a instancované varianty; nemění se každý frame.
Unused per-map UV inputs/varyings are omitted by material class. A missing
normal map also omits UV2/TBN, while retaining world derivatives for flat normals
and specular AA. An independent explicit normal texture with scale 0 checks this
variant on both backends (supplied/missing normals/tangents, back faces, no UV). CPU vertex storage keeps
the shared 96-byte format; this does not claim a packed-geometry bandwidth gain.
Ověření zahrnuje `gltf-full`, `gltf-sdk`, `gltf-material-cache` a `render-mirrored` na obou
backendech. Změna kompilace může mírně změnit zaokrouhlení odlesků; pixelová
shoda není zaručena. Desktop čas neprokazuje zrychlení na Questu.

## Hierarchie a animace

Každý model má jedno ID scény a uvnitř Rustu vlastní strom uzlů, mesh primitiva, skiny a morph targety. Transformace handle mění kořen celého modelu. Klipy podporují translaci, quaternion rotaci, scale a morph weights; interpolace STEP, LINEAR (rotace SLERP) a CUBICSPLINE. Skinnování používá inverse bind matrices a všechny sady joint/weight vlivů. Nejprve se aplikuje morph, potom skin a transformace uzlu.

`playAnimation(indexOrName, {loop, speed})`, `pauseAnimation(bool)`, `seekAnimation(seconds)` a `stopAnimation()` řídí jeden aktivní klip modelu. Smíchávání více klipů není součástí současného API. Rust aktualizuje animace jednou za frame, bez JS animační smyčky. Změněná póza obnoví bounds i statickou stínovou cache; označení `isStatic` animaci nezamyká.

Rigid transforms and eligible skinning use the GPU paths described above. Morph/unsupported deformation keeps the portable CPU fallback and uploads only changed poses. Main, reflection and shadow passes share the same geometry and pose resources; immutable models are not reuploaded each frame.

`model.setMorphWeights(nodeIndex, weights)` nastaví váhy bez spuštění klipu. `model.selectScene(index)` přepne glTF scénu. `model.useCamera(nodeIndex)` explicitně zapne perspektivní nebo ortografickou kameru modelu (včetně perspektivy bez zfar). `engine.setCameraView()` nebo `set_camera_mode()` vrátí ovládání běžné kameře. Dostupné uzly, scény a klipy jsou v `model.info`.

## Ovládání postavy

[Samostatný příklad s liškou](../examples/fox-control/) ukazuje jednorázové
nastavení vstupů a kamery za postavou:

```js
const { fox } = await createScene(engine.scene);
engine.controlCharacter(fox, {
    speed: 2.5,
    runSpeed: 5,
    animations: { idle: 'Survey', walk: 'Walk', run: 'Run' },
    physics: { halfExtents: [0.4, 0.8, 0.8], gravity: 18, jumpSpeed: 7 },
});
```

Rust počítá WASD pohyb po rovině XZ vůči směru kamery, natočení modelu,
Shift pro běh a přepínání klipů. V příkladu kliknutí na canvas zapne fullscreen
a zachytí kurzor; kameru pak otáčí samotný pohyb myši. Escape opustí hru
a pozastaví pohyb, další kliknutí ovládání obnoví. Ostatní příklady bez
zachycení kurzoru zachovávají tažení myší.
Diagonální pohyb má stejnou rychlost; ztráta fokusu uvolní klávesy.
Názvy klipů jsou volitelné, při zadání musí existovat. `modelForwardYaw`
nastaví korekci natočení modelu v radiánech. `engine.characterState()` vrací
aktuální ID, polohu a stav pohybu z Rustu, nebo `null` bez aktivní postavy.
Přepnutí kamery a odstranění postavy ovládání odpojí.

`physics` zapíná kolizní motor se vzpřímeným boxem: poloha modelu je poloha
chodidel, tělo má výšku `2 * halfExtents[1]`. Rust řeší gravitaci, skok na
mezerník pouze ze země, klouzání podél stěn a dopad na plošinu. Vynechání
`physics` ponechá volný pohyb bez kolizí. `grounded`, `verticalVelocity`
a `collisionCandidates` v `characterState()` ukazují skutečný stav motoru.

Pevné objekty nastavuje aplikace výslovně:

```js
const block = scene.createCube([0, 0.3, 3]);
block.setScale([2.6, 0.6, 2]);
block.setCollider(); // Box se škáluje a posouvá spolu s objektem.
```

`setCollider({ halfExtents, offset })` určuje lokální box nezávisle na renderovací
geometrii; `removeCollider()` ho odebere. Kolizní svět používá cachovaný BVH,
obnovovaný při změně pevného objektu. Dotaz vybere okolní boxy a sweep zastaví
pohyb na jejich hraně i při vysoké rychlosti. Příklad má čtyři plošiny a reset na R.
Jde o boxové kolize, nikoli přesné trojúhelníkové kolize glTF; otočené boxy používají
konzervativní světový AABB. Svahy, odtlačování dynamických těles, zotavení po
teleportu dovnitř překážky a prolínání animací nejsou součástí tohoto motoru.

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

### Reusable render preparation storage

The glTF queue retains flat index/value scratch across views and frames. `debugStats().gltf.queueScratchGrowths` is the cumulative count of tracked vectors whose capacity grew during a preparation call, not the total number of allocator calls. `queueScratchBytes` reports live retained capacity for those seven vectors; model-dependent cache clearing after removals releases it. Prepared GPU handle lists, diagnostic records and stable blend sorting can still allocate. See [the scoped Astra audit](ASTRA_RENDER_AUDIT_2026-10-24.md) for measurements and remaining scaling limits.


### Order-independent rigid opaque instances

`model.setRenderOptions({opaqueReorderable: true, ...otherOptions})` explicitly
permits opaque parts of overlapping roots to be grouped by shared asset/part.
This semantic promise is independent of `isStatic`; moving roots retain individual
IDs, transforms, culling, collisions and lifetime. Default `false` preserves the
existing order/AABB policy. As with the existing setter, omitted render options
reset to their documented defaults; pass the complete intended options together.

Opt in only when inter-root equal-depth/coplanar surfaces do not require draw
order. The renderer additionally requires the entire immutable asset to contain
only rigid, non-BLEND draws. Skin/morph and mixed transparent assets retain their
fallback. A non-opted/unsupported root ends the run; the queue never jumps over it.
The first implementation groups at most 128 contiguous roots per scheduling
window, retaining input order between windows, existing auxiliary
shadow/reflection/clipped queues and 1024-instance pages. It extends existing shared resources and per-view
streams, not CPU geometry or gameplay object ownership.

Rigid immutable GPU vertices also omit unused material UV slots from their stored
Float32 stream (56–96 byte stride). Shader locations/types and CPU query/cutting
geometry stay intact; skin and CPU-deformation retain the 96-byte layout. This
storage optimization applies to rigid glTF assets independently of the optional
root-reordering policy.
