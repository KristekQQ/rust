# Texturové cíle, stíny a rovinné odrazy

Engine drží scénu, simulaci, matice, culling a plán průchodů v Rustu. JavaScript
pouze posílá příkazy. Ukázka: `tests/render-effects.html` přes lokální HTTP server,
případně `?backend=webgl`. Hlavní stránka nadále načítá původní malou scénu.

## Moduly a průchody

| Modul | Odpovědnost |
| --- | --- |
| `scene.rs` | transformace, render options, statická revize, simulace, identita zrcadla |
| `render_math.rs` | normalizovaná rovina, odražený pohled, světelný pohled; nativní testy |
| `visibility.rs` | konzervativní AABB/frustum test a filtry pro každý průchod |
| `render/target.rs` | vypůjčené color/depth a samostatné depth cíle, viewport, Clear/Load |
| `render/textures.rs` | vlastnictví a tvorba opakovaně použitelných barevných/hloubkových textur |
| `render/renderer.rs` | GPU meshe, čtyři nezávislé cache dávek, barevné a hloubkové vykreslení |
| `render/effects.rs` | prostředky efektů, pořadí průchodů a invalidace texturových cache |
| `render/output.rs` | browser surface, resize a prezentace; používá stejný ColorTarget jako odraz |

Pořadí je statická stínová mapa → dynamická stínová mapa → odraz → hlavní
kamera. Každý pomocný průchod se spouští pouze při invalidaci. Simulace se
posouvá jednou před vykreslením, bez ohledu na počet kamer. Pomocné průchody
mají vlastní RenderQueue a instance buffery; nesdílejí seznam viditelných
objektů hlavní kamery. Caster mimo hlavní frustum tedy může pořád vrhat stín.
Reflexní průchod vynechává samotné aktivní zrcadlo.

Pipeline se cachují podle formátů příloh a orientace pohledu; hloubkové
pipeline podle formátu. Textury se vytvářejí při prvním použití/resize.
Renderer submittuje každý průchod před zápisem uniformů dalšího pohledu:
wgpu queue.write_buffer nemá snapshot uniformů pro každý dříve vytvořený
command buffer. Toto pořadí chrání data kamer. Dalším výkonovým krokem podle
profilování může být vlastní uniform buffer pro každý průchod a společný
encoder/submit; nyní nejsou tvrzeny absolutní výkonnostní výsledky pro jiné GPU.

## Cache a stíny

Statická mapa: klíč světelného pohledu + statická revize + stav cullingu.
Dynamická mapa: světelný pohled + revize geometrie + culling. Odraz: odražený
pohled + geometrie + světla + culling + stav stínů; resize také ruší jeho cache.
Zrcadlo mimo frustum hlavní kamery odraz neobnovuje. Při neměnné scéně a
pohledu se pomocné průchody vynechají, zatímco hlavní kamera stále vykresluje.

`isStatic` je příznak pro cache, nikoliv zákaz změny. Přesun, změna materiálu,
odstranění nebo animace statického objektu invalidují statickou revizi. Pohyb
běžného dynamického objektu ji nemění. Propojení obou stínových map používá
minimum porovnání v každém texelu, následované 3×3 PCF; tím se vrhající objekty
kombinují před filtrováním. Depth bias při rasterizaci a malý bias podle
normály omezují self-shadow acne.

Současná implementace má **jedno stínující světlo (první světlo scény)**,
perspektivní mapu s omezeným záběrem kolem středu scény (FOV 1,5 rad,
near 0,1, far 30), nikoliv úplný 360° point-light shadow. Mimo tento záběr je
povrch osvětlen bez stínu tohoto světla. Každá mapa má 1024² texelů Depth32Float;
dvě mapy využívají přibližně 8 MiB. Nejde o cascaded shadow maps ani cube mapy.
Další světla mají běžné osvětlení. Toto omezení je vhodné pro ukázku a budoucí
spot/directional rozšíření potřebuje vlastní nastavení projekce.

## Rovinné zrcadlo

Jeden aktivní Plane objekt se registruje jako zrcadlo. Rovina se počítá z jeho
cached normal matrix a pozice, takže sleduje přesun i rotaci. Odražený
view-projection je původní matice × world reflection matrix. Ta obrací
orientaci, proto odraz používá opačný culling trojúhelníků. Fragmentový clip
plane odstraňuje geometrii za zrcadlem; jde o konzervativní frustum culling a
fragmentové oříznutí, nikoliv oblique near-plane clipping. Odraz se nerekurzuje.

Odraz je RGBA8 sRGB + hloubka, poloviční rozlišení canvasu s delší stranou
omezenou na 1024. Zachovává poměr stran; materiál používá projekční UV.
Podporovány jsou neprůhledné objekty, jedna rovina a single-sample přílohy.
Více zrcadel, environment cubemaps, průhlednost, roughness mip chain a VR/WebXR
binding zatím nejsou implementovány.

## JS/TS příkazy

```js
const engine = await initEngine(canvas);
const cube = engine.scene.createCube([0, 0, 0]);
cube.setRenderOptions({ isStatic: false, castsShadow: true, receivesShadow: true });
cube.spin([0, 0.8, 0]); // radiány za sekundu; simulace probíhá v Rustu

const mirror = engine.scene.createPlane([0, 0.5, -3]);
mirror.setRotation([Math.PI / 2, 0, 0]).setScale([5.5, 1, 3]);
mirror.setRenderOptions({ isStatic: true, castsShadow: false, receivesShadow: false, reflectivity: 0.94 });
engine.scene.setMirror(mirror); // rovina z transformace; výměna aktivního zrcadla
engine.set_render_effects(true, true); // stíny, odraz
engine.set_simulation_paused(true); // kamera se může dál pohybovat
```

`setRenderOptions` nastaví celý balík voleb; vynechané hodnoty mají výchozí
hodnoty false/true/true/0. Odrazivost přijímají pouze Plane objekty, vizuálně ji
používá aktivní zrcadlo. `spin([0,0,0])` zastaví průběžnou rotaci.
`engine.scene.loadEffectsDemo()` vytvoří v Rustu 9 statických objektů a 2
rotující objekty. `engine.effects_stats()` vrací počty aktualizací map, počty
viditelných objektů v průchodech, počet provedených průchodů a rozlišení.
Počet průchodů nezahrnuje závěrečný prezentovací blit při output mode texture.
`prepareMs` nyní zahrnuje CPU simulaci, přípravu/encoding/submission všech
průchodů, nikoliv GPU execution. FPS je stále interval browser RAF.

Nízké Rust rozhraní používá `RenderView`, `RenderTarget`, `DepthRenderTarget`,
`ColorTarget` a `DepthTarget`. `SceneRenderer::render_depth_view` přijme vlastní
pohled a hloubkový cíl spolu s filtrem RenderPassKind. Vlastní další barevný cíl
lze předat `render_view`; canvas není nutný pro renderovací kód. Simulaci
posouvejte zvlášť přes `advance_scene`, jednou za snímek. Vlastní přílohy musí
odpovídat deklarovaným formátům/rozměrům, vzorkování musí být single-sample.

## Ověření

Nativní testy kontrolují odraz, rovinu po změně transformace, konzervativní
frusta, castery mimo hlavní kameru, rozdělení statických/dynamických casterů,
invalidaci statické revize, pause a Rust/WGSL layout materiálu (InstanceData
144 B, material na offsetu 128). `tests/render-effects.html` testuje cache v
klidu, obnovování za pohybu, vypnutí efektů, resize a výstup přes texturu.
Skutečný obraz a GPU konzoli je nutné kontrolovat v browseru; statistiky samy
nejsou důkazem správných stínů/odrazu.

Zdroje návrhu: [oficiální WebGPU shadow mapping sample](https://webgpu.github.io/webgpu-samples/samples/shadowMapping/),
[wgpu depth sample type](https://docs.rs/wgpu/30.0.1/wgpu/enum.TextureSampleType.html).
