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
chybí; zatím bez temporal accumulation, roughness a environment fallback.

## God rays / volumetrické světlo

Od 2026-10-12 shader místo radiálního rozmazání integruje rozptýlené světlo
podél kamerového paprsku ve 32 vzorcích. První viditelný povrch z packed depth
omezuje délku integrace (maximum 24 jednotek). Každý vzorek používá statickou
i dynamickou stínovou mapu prvního světla, směrovou HG fázi a exponenciální útlum.
Stabilní prostorový jitter omezuje pravidelné pásy; temporal accumulation není
implementována. Zdroj už nemusí být vidět na obrazovce.

Ukázka používá kamenný průchod a nepravidelné skály místo barevných lamel.
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
