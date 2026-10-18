# Postprocessing v Rustu a WebGPU / WebGL

Scéna, depth a přenosná RGB depth kopie se jednou vykreslí do offscreen cíle. `PostProcessor` poté spustí
zapnuté efekty v pořadí registru `EFFECTS`, střídá dva barevné ping-pong cíle
a poslední průchod zapisuje do výstupu. Není tu další JS frame loop.

```js
engine.postprocessing.enable('ssr', { strength: 0.85 });
engine.postprocessing.enable('godRays', { strength: 0.65 });
engine.postprocessing.disable('godRays');
```

Síla je 0…1. Neplatné hodnoty a neznámé efekty se odmítají. Statistika vrací
naposledy submitované `passes`, rozlišení, počet barevných `targets` a monotonní
`frames` (počet submitů řetězce, nikoli dokončení GPU). Vypnutí posledního efektu
uvolní čtyři barevné cíle, jejich depth a bind groups. Pipelines a případná malá
maskovací textura se ponechávají pro další použití. Resize obnovuje cíle a bind
groups společně; při stabilním rozlišení se GPU prostředky každý frame netvoří.

## SSR a kaluž

Objekt nastaví `setRenderOptions({ reflectivity: 0.85 })`. Reflektivita je
metadata v alpha pouze při offscreen renderu; běžný i finální obraz mají alpha 1.
Planar mirror má zvláštní příznak a dál vyžaduje rovinu. SSR z hloubky rekonstruuje
pozici a normálu, sleduje odražený paprsek (64 kroků, 5 zpřesnění), odmítá self-hit
a tlumí odraz na hranici obrazu. Nejde o FidelityFX ani hardwarový ray tracing.

`examples/postprocessing/puddle-mask.svg` je skutečná šedotónová textura:
černá = suchý povrch, bílá = mokrý; šedá = měkký okraj. `texture.js` ji jednou
načte a pošle pixely do Rustu:

```js
engine.postprocessing.setReflectionMask({
    width: 256, height: 256, pixels, // Uint8Array, jeden kanál na pixel
    center: [0, 0], size: [9, 9],   // střed XZ a rozměry ve světových jednotkách
});
```

Maska se promítá na světové XZ, násobí reflektivitu a v SSR průchodu ztmavuje
mokrou část. Je to jedna globální maska pro horizontální povrchy, ne obecný
texturový materiál ani vodní simulace; při vypnutí SSR se tento mokrý vzhled
vypne také. Maximální maska je 1024×1024. Depth je v druhém MRT attachmentu zabalena do RGB8, aby se dala číst běžným samplerem ve WebGL i WebGPU. Normály jsou rekonstruované z depth,
nikoli z normal mapy. Objekty mimo obraz nebo za viditelným povrchem v odrazu
chybí; bez temporal accumulation a roughness. Volitelný environment fallback popisuje další odstavec.

## God rays / volumetrické světlo

Od 2026-10-12 shader místo radiálního rozmazání integruje rozptýlené světlo
podél kamerového paprsku ve 32 vzorcích. První viditelný povrch z packed depth
omezuje délku integrace (maximum 24 jednotek). Každý vzorek používá statickou
i dynamickou stínovou mapu prvního světla, směrovou HG fázi a exponenciální útlum.
Stabilní prostorový jitter omezuje pravidelné pásy; temporal accumulation není
implementována. Zdroj už nemusí být vidět na obrazovce.

Ukázka používá skutečný interiér katedrály Šibenik (72 513 trojúhelníků),
bez textur a animací. Geometry-only GLB vzniká offline převodem staženého OBJ;
provenance a nekomerční licence CC BY-NC jsou v assets/models/sibenik-cathedral.
Stínové mapy se sdílejí s rendererem; další stínové průchody ani kopie nejsou
potřeba. `enable('godRays', { strength })` a pořadí řetězce zůstávají stejné.

Jde o omezený objemový single-scattering efekt, ne úplný atmosférický model:
jedno světlo, homogenní médium, stávající perspektivní stínový kužel místo 360°
point shadows; bez vícečetného rozptylu a bez zeslabení původní povrchové barvy
mlhou. Při vypnutých stínech se rozptyl počítá jako nezakryté světlo v kuželu.
Ve vysoce kontrastních hranách jsou možné šum / aliasing; není to záruka VR výkonu.

## Přidání dalšího efektu

1. Přidej WGSL fragmentový entrypoint (samostatný soubor je možný).
2. Do `EFFECTS` v `src/render/postprocess.rs` přidej descriptor: jméno, shader,
   entrypoint. Společná infrastruktura vytvoří pipeline/uniform/bind groups
   a zařadí průchod; renderovací smyčka se nemění.
3. Rozšiř `PostEffectName` v `js/engine.d.ts` a případné UI. Další WASM export
   pro efekt není potřeba, protože příkaz je obecný.

Společné vstupy: předchozí barva, původní packed depth, uniformy kamery/světla a maska.
Speciální efekty vyžadující historii, HDR nebo více vstupů potřebují rozšířit
kontrakt; současný registr není libovolný render graph. Efekty zachovávají
metadata alpha mezi průchody a finální průchod vynutí alpha 1.

Ukázka a integrační kontroly: `/examples/postprocessing/`, `?backend=webgl`,
`tests/scene-sdk.html`, `tests/render-target.html`, `tests/render-effects.html`.

Podklady: [GPUOpen SSR](https://gpuopen.com/manuals/fidelityfx_sdk/techniques/stochastic-screen-space-reflections/)
, [GPU Gems: objemové světlo](https://developer.nvidia.com/gpugems/gpugems/part-vi-beyond-triangles/chapter-39-volume-rendering-techniques)
a [GPU Gems: light scattering postprocess](https://developer.nvidia.com/gpugems/gpugems3/part-ii-light-and-shadows/chapter-13-volumetric-light-scattering-post-process).

Pomocný grid a značky světel se vykreslí až po řetězci jako depth-tested overlay.
Neovlivňují SSR, god rays ani packed-depth vstup.

## Viditelné paprsky z otevřeného okna

Geometry-only převod vynechává `staklo*` výplně oken, které by jinak vrhaly
neprůhledný stín. Kamenné rámy a rozeta jsou běžné statické castery.
První světlo ukázky je venku před velkou rozetou; výchozí kamera hledí do svazků.
Jde zatím o bodové světlo a jeho perspektivní stínový kužel, ne o implementaci
směrového slunce. Žádná malovaná textura paprsků se nepoužívá.

`engine.postprocessing.configureGodRays({ density: 0.055, exposure: 40 })`
nastaví hustotu média (0–1) a jas objemového světla (0–100), nezávisle na osvětlení
povrchů. Rust validuje a zapisuje hodnoty do existujících uniformů; změna těchto
voleb nevytváří další renderovací cíle. Výchozí hodnoty enginu zůstávají 0.035/2.5.
Ukázkové UI nabízí checkbox efektu, jas a tlačítko návratu k pohledu na paprsky.

Vývojový server `scripts/serve-local.py` nepoužívá cache modulů; `run-local.sh`
ho spustí po WASM buildu. Staré otevřené stránky je při změně SDK třeba obnovit.

## Statický environment probe a emise

`postprocessing.configureEnvironment({enabled:true,center:[0,2,0],halfExtents:[14,10,14]})`
zachytí šest 256² sRGB stran skutečné statické scény; statická revize nebo světla
cache invalidují. Capture má samostatnou viditelnost, vynechá dynamické objekty,
aktivní zrcadlo a pomocné overlaye. SSR používá box projected cubemapu při miss
a na okrajích. Jde o lokální aproximaci, nikoli přesný rovinný odraz.
Vypnutí SSR vypne také jeho environment fallback. Probe nevytváří skybox.

`SceneObject.setEmission(0..8)` nastaví přídavnou vertex-color emisi primitiv a
triangle meshes; glTF nadále používá vlastní materiály. Emise nevrhá světlo.
`postprocessing.enable('bloom',{strength:.38})` přidá omezený LDR jasový blur,
nikoli HDR pipeline. Ukázka: [neon-arena](../examples/neon-arena/README.md).
Sdílený probe je nezávislý na XR oku, SSR má matice a hloubku aktuálního oka;
fyzický Quest a jeho výkon nejsou desktopovým testem prokázány.

## Obrazová cubemapa se samostatnou emisí

`setEnvironmentCubeMap({size,color,emission,intensity:1.2})` nahraje dvě sady
RGBA8 sRGB pixelů v pořadí +X, −X, +Y, −Y, +Z, −Z. Každá sada má
`6 * size * size * 4` bajtů; velikost je 1–2048. `setEnvironmentIntensity(0..8)`
mění intenzitu bez uploadu. `setEnvironmentCubeMap(null)` mapy uvolní.

Automatický první průchod kreslí okolí jen do pixelů bez geometrie a funguje
samostatně bez SSR/bloom. SSR stejnou cubemapu použije jako odraz mimo obraz;
při chybějící obrazové mapě dál může používat původní statický probe. Emise se
přičítá ve WGSL před bloomem. Mezicíle jsou LDR, takže nadbytečná radiance se
ořízne; nejde o HDR pipeline ani skutečnou geometrii okolí. Směrový sampling
sdílí desktop a per-eye WebXR cestu; desktopový test nepotvrzuje headset.

`examples/neon-arena/environment.js` načte PNG jednou přes DOM canvas a předá
pixely SDK. Scéna i GPU sampling běží v Rustu. Zdrojové obrázky, emisní mapa a
offline převod na šest stěn jsou součástí samostatné ukázky.

### Rozpočet SSR ve VR

Desktop zachovává 64 lineárních kroků a 5 zpřesnění. Per-eye XR používá 16 kroků
a 3 zpřesnění; kvadraticky rostoucí rozestupy udrží původní dosah (poslední vzorek
je ve vzdálenosti původního kroku64). Blízké vzorky jsou hustší. Znamená to menší
přesnost odrazů zejména tenké/vzdálené geometrie, nikoli vypnutí dynamického SSR.
Bloom a emise zůstávají zapnuté. XR pixelový test izoluje odraz pohyblivého jablka
ve spodní podlaze, aby změna rozpočtu nemohla potichu ponechat jen statický cube.

`tests/neon-arena-profile.html` měří čtyři varianty skutečné aplikace v udržovaném
XR simulátoru (1024×1024 pro každé oko). Součástí času je synchronní odečet pixelů.
Spuštění: `VERIFY_WASM_PROFILE=release node scripts/test-browser.mjs --page neon-arena-profile --backend webgl`.
Výsledek je desktopový profil s explicitními mezemi, ne měření FPS Questu.

### Bližší obrazové okolí

Volitelné `projection: {center:[0,1.65,0],halfExtents:[7,10.35,7]}` v
`setEnvironmentCubeMap` nastavuje pozici capture a půlrozměry konečného boxu.
WGSL používá průsečík výstupního paprsku s boxem pro pozadí i odrazy; tím obraz
reaguje na posun hlavy. Mimo objem, při chybějící projekci nebo `projection:null`
zůstává nekonečný směrový sampling. Hodnoty musí být konečné, |center|≤1e6,
0<halfExtents≤1e6. Nejde o skutečnou hloubkovou mapu ani přesnou paralaxu složité scény.
