# Sdílená geometrie a varianty plošek

`Scene.createMeshVariants` registruje neměnnou geometrii s více tabulkami RGB
barev trojúhelníků. `MeshVariants.createInstance(index)` vytvoří samostatný scénový
objekt: má vlastní transformaci, materiál, kolize a řezatelnost. Rust vlastní
prostředky i stav. JavaScript drží pouze handle a předává příkazy.

```js
const asset = scene.createMeshVariants({
    positions, indices,
    variants: [firstTriangleColors, secondTriangleColors],
    smooth: false,
});
const first = asset.createInstance(0).setPosition([-1, 1, -2]);
const second = asset.createInstance(1).setPosition([1, 1, -2]);
asset.remove(); // Zakáže nové instance, existující objekty dál fungují.
first.remove();
second.remove(); // Poslední reference umožní uvolnit GPU prostředky.
```

Každá varianta obsahuje `indices.length` čísel: RGB na každý trojúhelník.
Pozice, indexy a způsob vyhlazení jsou společné. Současná neonová cesta vyžaduje
shodné barvy jasných trojúhelníků (`max RGB >= .25`) ve všech variantách; jinak
registraci odmítne. Tím zachovává jednu sdílenou geometrii i barvu glow hran.
Pro libovolné změny tvaru nebo jasných hran použij samostatný mesh.

## GPU a průchody

Každý asset má jeden vertex/index buffer a malou 2D `RGBA32Float` texturu:
jeden texel na měnící se trojúhelník a variantu. Trojúhelníky se shodným RGB
ve všech variantách zůstávají přímo ve vertex bufferu, bez texture lookupu.
Vertex shader čte proměnlivé RGB přes `textureLoad` podle kompaktního slotu
a čísla varianty. Není potřeba sampler, filtrování,
UV souřadnice, mipmapy ani textura obrázku ovoce. Float32 zachovává původní
barvy i jemný jas použitý jako klíč krytí plošky; 8bitová kvantizace by vzory změnila.
WGSL operace: [textureLoad](https://www.w3.org/TR/WGSL/#textureload).

`InstanceData` zachovává 176 bajtů. `surface_params.xy` zůstává variation/coverage;
`z` je číslo varianty, `w` počet proměnlivých slotů jedné varianty (nula vypíná lookup). Obyčejné objekty mají tuto cestu vypnutou.
Pouze sdílený GPU vertex buffer kóduje proměnlivý slot jako záporné `color.r`
(`-(slot+1)`). Main i shadow shader tento marker dekódují před použitím RGB.
Původní CPU barvy, řezání, glow a alpha subset zůstávají nezměněné; nikdy do nich
marker nepřenášej.
Hlavní a glow průchod seskupují viditelné instance podle asset handle. To snižuje
počet draw calls i kopírování geometrie. Počet vykreslených trojúhelníků všech
viditelných instancí a počet pixelů se samotným instancováním nesnižují.

Průhledné instance zůstávají seřazené podle vzdálenosti a vykreslují zadní/přední
plochy zvlášť. Neslučuj je napříč tímto pořadím. Sdílený alpha index buffer obsahuje
sjednocení potřebných indexů všech variant při daném prahu; vynechání trojúhelníku
musí být bezpečné pro každou variantu. Nevytvářej nové passy ani queue submitty.

CPU řezání používá přesnou geometrii a barvy zvolené varianty. Výsledné půlky mají
novou topologii a přecházejí na samostatné meshe. Odstranění původního objektu
nesmí zrušit asset ostatním instancím. `scene.clear()` ruší assety i objekty;
staré handles pak nejsou platné.

## Aréna a ověření

Aréna registruje deset druhů, každý s patnácti vybranými vzory. Tabulka má třicet
samostatných objektů, hlavní a glow geometrie se seskupuje po druzích.
Čtyřicet rámečků sdílí další dvě geometrie: celkem sedmdesát objektů ve dvanácti
mesh skupinách. Stejné
assety používají střely a náhledový řez. Vzory se za běhu nemění ani nepřenahrávají.

`engine.debugStats().sharedMeshes` hlásí assety, živé instance, GPU meshe, bajty
barevných textur, společné geometrie a hypotetických samostatných kopií geometrie. Ověřuj rozdíl oproti reálnému počtu objektů a skutečným draw calls;
nižší počet bufferů sám nedokazuje vyšší FPS. Pixelový test arény porovnává sdílené
varianty s původním barevným meshem, rozdílnost průhlednosti i uvolnění assetu po
poslední instanci. XR test musí zachovat skutečné řezy a barvy uzávěrů. Desktop a
simulované XR nenahrazují měření výkonu v Questu.
