# Řezání triangulovaných těles

## Rozhodnutí: pro aktuální řezání preferovat CPU

WebGPU compute řezání byl experiment (hybrid i GPU-residentní). Dosavadní lokální
testy ukázaly lepší celkovou odezvu optimalizované CPU cesty, proto `auto` zůstává
CPU a GPU režimy slouží jen explicitnímu porovnávání. Další instance mají tento
závěr respektovat, nikoli předpokládat zrychlení pouhým přesunem na GPU.
Nové měření má smysl při změně zátěže nebo požadavku uživatele. Quest 3 zatím
změřený není; nejde o obecný závěr proti compute ani WebGPU vykreslování.
Stručné předání: [README příkladu](../examples/cutting/README.md).

## GPU-residentní cesta a Quest 3

`await scene.cutWithAsync(sword, {motion, execution: 'gpu-resident', scatter})`
používá GPU pro clipping, hash hran/obrysů, uzávěry, objem/bounds, podporu animace
i sestavení vertex/index bufferů. Renderer používá výsledné buffery přímo ve
stávajících průchodech. Opakovaný řez čte residentní GPU zdroj. Úspěšný řez
nevrací geometrii CPU: mapuje pouze 640 B metadat na kandidáta; report uvádí
`metadataReadbackBytes` a `geometryReadbackBytes: 0`.

CPU nadále vlastní příkazy, ID, transformace, transakční změnu scény a výběr
kandidátů podle bounds. Animace používá 64 GPU podpůrných vzorků na polovinu
s konzervativní interpolací: dosednutí na konci je přesné, průběh není bitově
shodný s CPU a může mít malou kladnou vůli nad stolem. `meshInfo` používá
metadata; po libovolné rotaci/měřítku mimo tuto trajektorii je `bottom`
konzervativní podpora lokálního AABB, nikoli skrytý synchronní CPU přepočet.
`resident` a `cpuMaterialized` umožňují kontrolovat skutečnou rezidenci.

GPU varianta nyní podporuje jedno souvislé uzavřené genus-zero těleso,
jediný obrys řezu s bezpečným středovým vějířem, nejvýše 20k vstupních
trojúhelníků, nejvýše 4096 hran v pracovním obrysu a 20k výstupních trojúhelníků
na kus (stejný limit jako CPU fallback). Nejednoznačná přesnost, více obrysů,
nevyhovující vějíř nebo překročení limitů použijí CPU **celého příkazu před
změnou scény**, s `execution: 'cpu'` a `fallbackReason`. Póly těsně u roviny
mohou proto fallback použít. CPU podporuje širší konkávní případy. GPU uzávěry
mají jiné vnitřní triangulování, ale musí zachovat povrch, objem a ploché normály.
Chyba GPU validace/mapování není prohlašována za úspěch. `auto` zatím zůstává CPU.
První GPU řez navíc zahrnuje kompilaci pěti pipeline; mediány po zahřátí nejsou
údajem o tomto prvním seku. Pracovní buffery mají omezenou kapacitu a
objem/bounds se redukují paralelně; bez otáčení se podpora nepočítá 64× stejně.

CPU cesta navíc hashuje jen hrany roviny, nikoli všechny ponechané hrany slupky.
BVH nových kusů se staví líně až při jejich dalším skutečném řezu; původní mesh
má BVH připravené. Nativní release benchmark prvního řezu 19k jablka: přibližně
11,0 → 4,7 ms. Část práce se přesouvá na případný další řez; nejde o záruku
stejného násobku pro celou sekvenci řezání.

Pouze explicitní CPU fallback nebo `meshSnapshot` mohou líně přehrát historii
řezů na CPU. Tento snapshot je **CPU reference**, ne důkaz skutečné GPU geometrie.
`await piece.gpuMeshSnapshot()` naproti tomu diagnosticky přečte skutečné GPU
vertexy; normální render/řez tento úplný readback nepoužívá.

Pro reprodukovatelné měření: `VERIFY_WASM_PROFILE=release npm run verify:browser -- --page cutting-resident,cutting-compute --backend webgpu`.
Profil se zapisuje do fingerprintu; při následném `test:browser` nastav stejné
prostředí. Výchozí `dev` je pro vývoj, nikoli srovnávání produkčního výkonu.
GPU limity se ověřují na běžícím device, bez závislosti na user-agentu.
Quest 3 je cílová platforma, ale desktopové časy nejsou headsetové výsledky.
Pro headset je nutné bezpečné HTTPS/localhost prostředí a skutečné měření
v Quest Browseru; WebXR ani připojení headsetu tato změna nepřidává.
Meta doporučuje [detekci schopností za běhu a test na headsetu](https://developers.meta.com/vr/documentation/web/browser-specs/).

Lokální závěrečný release/WASM běh (medián 4 měřených vzorků po zahřátí):
12 096 tri CPU ~10,0 ms, residentní GPU s přípravou animace ~20,4 ms, bez ní
~57,3 ms. U 1 088 tri ~9,5 / 15,5 / 25,2 ms. Velký rozptyl podle pořadí,
alokací a čekání ve sdílené render queue znamená, že to není izolovaný
benchmark kernelu; první GPU sek v example měl ~322 ms včetně kompilace.
GPU tudíž zatím nelze prohlásit za rychlejší než CPU. Report:
`output/verification/browser-report.json` (release, 10/10 PASS WebGPU/WebGL).

## CPU a původní hybridní WebGPU compute

Výchozí `scene.cutWith(...)` zůstává synchronní CPU/WASM cesta. Nová
`await scene.cutWithAsync(sword, {motion, execution: 'auto'|'cpu'|'gpu', scatter})`
má stejné nahrazování objektů, BVH, uzávěry, normály a animaci. `auto` nyní
záměrně volí optimalizované CPU: naměřený hybrid s readbackem je pomalejší.
V ukázce lze explicitně vybrat **WebGPU compute + CPU**; na WebGL je tato
volba nedostupná a programový požadavek `gpu` vrátí chybu, ne tichý fallback.

WGSL compute skutečně počítá vzdálenosti vrcholů od roviny a kanonické průsečíky
hran z kandidátů trojúhelníkového BVH. Dávka používá existující device, jeden
submit a jeden asynchronní readback. Pozice se cachují podle neměnných ID;
odstraněné meshe se z cache uvolňují. Uzávěry, souvislé komponenty, kontrola
objemu a nové BVH zůstávají v Rustu na CPU. Nejde o plně GPU-residentní slicing.
U vrcholů blízko roviny CPU opraví pouze nejednoznačné hodnoty a přilehlé hrany.
Capy nesou explicitní normálu řezné roviny, aby drobné GPU zaokrouhlení neměnilo
jejich stínování. Při větší nekonzistenci se použije CPU referenční geometrie.

Při čekání na GPU není držen borrow scény. Změna cíle, registrace/polohy čepele,
clear nebo dispose zneplatní pozdní výsledek; nesouvisející animace jej neruší.
Současně může běžet jediný async řez. UI zůstává po dobu readbacku obsazené;
reset je možný a staré kusy se po něm nesmějí znovu objevit.
Výsledek přidává `execution`, `gpuMeshes`, `fallbackReason`, `totalMs`,
`computeMs` (dispatch + čekání + readback, nikoliv GPU timestamp) a `cpuMs`.
Explicitní `object.meshSnapshot` slouží diagnostice geometrie/normál, ne pollingu.

Měření nativního release benchmarku (9 vzorků, medián): jablko 19 000 tri
24,74 → 10,66 ms (~2,3×), 8 064 tri 10,31 → 4,59 ms po zrychlení map hran,
komponent a přemapování vrcholů. Není to měření browserového FPS. První skutečný
browserový dev/WASM test ukázal pomalejší hybrid; závěrečný běh po odstranění
zbytečné CPU přípravy má 12 096 tri CPU ~147 ms, hybrid ~325 ms; malé jablko
1 088 tri CPU ~41 ms, hybrid ~384 ms. Celkový dispatch/čekání/readback zde převažuje.
Časy jsou lokální, bez záruky pro jiné GPU; automatický režim proto GPU nevnucuje.
Tyto starší časy jsou z dev profilu; nová residentní cesta výše ponechává na GPU i topologii a výsledky.

Ověření: `cargo test cutting`; ignorovaný měřicí test
`cargo test --release cutting_bench::profile_cpu_cutting -- --ignored --exact --nocapture --test-threads=1`;
browser `cutting-compute` na WebGPU/WebGL porovnává topologii, pozice, normály,
barvy, objemy, opakované/pólové řezy, zrcadlené měřítko a zrušení požadavků.

Zdroje: [WebGPU mapování bufferů](https://www.w3.org/TR/webgpu/#buffer-mapping),
[wgpu Buffer / asynchronní mapování](https://docs.rs/wgpu/30.0.1/wgpu/struct.Buffer.html).

## Ukázka

Ukázka: `examples/cutting/`. Pět nezávisle řezatelných jablek na dřevěném prkénku.
Meč seká shora dolů. Výchozí režim **Meč v ruce** má barevný model ruky,
prstů a rukojeti jako součást stejného meshe: vše se při seku pohybuje spolu.
Pohyb kurzoru nad stolem zaměřuje meč, levý klik seká. **Plán řezu** vybírá
směr a místo tažením, puštěním seká. **Kamera** umožní otáčení levým tažením
i na dotykovém zařízení. Neplatný paprsek/nestolní cíl neprovádí řez na starém cíli.

Pravé tažení nebo Shift+tažení vždy otáčí kameru, kolečko a tlačítka ± přibližují.
WASD posouvá orbit cíl, Q/E mění výškový úhel. V / tlačítko obnoví pohled.
Resize zachovává uživatelskou kameru. H přepíná meč/plán, C přepíná kameru/meč,
mezerník seká, R/tlačítko obnoví všech pět jablek, F přepíná fullscreen.
F3 zapíná/vypíná Rust informační panel scény (výchozí stav vypnuto).
Směr, X/Z posun a náklon lze nastavit posuvníky. Na úzké obrazovce je nastavení
v rozbalovací sekci, aby zůstala vidět scéna.

Zelená přerušovaná čára je skutečná 3D geometrie nad prkénkem, ne DOM overlay.
Na řezatelných meshech ji doplňuje souvislý zelený pás přímo na jejich viditelném
povrchu. WGSL používá stejnou rovinu, délku čepele a výškový rozsah tahu;
nejde o rentgenový overlay, skryté povrchy zůstávají zakryté. Lze ji vypnout.
Rust počítá pozici čepele, tah i čáru
společně; náklon je proto zahrnut ve všech třech. Délka čáry odpovídá čepeli.
Rust dělí skutečný mesh, animuje otevření kusů dužinou nahoru a dosednutí
na prkénko. Jde o řízenou animaci, ne o rigid-body fyzikální simulaci.
Stůl/prkénko ani vodítka nejsou řezatelné. Stopka/list každého jablka zmizí až
při prvním řezu tohoto konkrétního jablka. Reset jablek nezmění pohled kamery.

```js
const target = scene.createMesh({positions, indices, color: [0.1, 0.7, 0.8]});
target.setCuttable(true, {capColor: [1, 0.3, 0.1]});
const sword = scene.createCube().setScale([0.05, 0.05, 5]);
sword.setCutter({base: [0, 0, -0.5], tip: [0, 0, 0.5]});
sword.setPosition([0, 3, 0]);
const result = scene.cutWith(sword, {motion: [0, -6, 0], separation: 0.12});
// result.removed: stará ID; result.pieces: nové SceneObject handles.
// Kusy dědí transformaci, render options, povolení řezání a barvu průřezu.
// result.objectCandidates / bvhNodes / triangleTests: skutečná práce BVH.
```

Volitelně přidej do `cutWith` `scatter: {floorY: 0.2, distance: 0.8, duration: 0.6}`.
Parametry se ověřují před rozdělením. Rust po zásahu kusy pootočí kolem vlastního
středu a odsune podél původních poloprostorů; minimum jejich transformovaných
vrcholů nakonec přesně leží na `floorY`. Nový polohovací/rotační/scale příkaz,
odstranění nebo další řez jejich animaci přeruší. `set_simulation_paused` pozastaví
meč i kusy. Dosah: `distance` 0–3 jednotky, `duration` 0,05–3 sekundy.
`engine.screenToPlane([u,v], height)` promítá normalizované souřadnice canvasu
(levý horní roh 0/0) do vodorovné světové roviny podle skutečné aktuální kamery.
WASM musí mít vykreslený alespoň jeden frame po změně kamery/resize.
SDK drží pouze příkazy/handles; projekce, support bod a průběh pohybu jsou v Rustu.

### Zaměřování a kamera

```js
engine.setOrbitView([7.5, 8.5, 12], [-0.8, 0.65, -0.6]);
engine.adjustOrbit(dxPixels, dyPixels, wheelDelta);
const center = engine.screenToPlane([u, v], 1.15);
const motion = scene.aimCutter(sword, {
    center, yaw: 0, tilt: 0.2, height: 3.6, floorY: 0.2,
    guides: dashedLineHandles,
});
scene.cutWith(sword, {motion, scatter: {floorY: 0.205}});
```

`aimCutter` vyžaduje vodorovnou lokální Z čepel; úhly jsou radiány, náklon ±0,53.
Vodítka jsou existující neřezatelné objekty se stejným bindingem, maximálně 64.
Chybná ID/parametry nezmění původní pose. Výsledný tah je spočten v Rustu,
včetně lokální výšky ostří a zastavení u podložky. Neprovádí se žádný řez.
`worldToScreen(point)` vrací normalizované souřadnice viditelného bodu nebo null.
`cameraViewState()` vrací polohu a orbit cíl, dotazy nevlastní simulaci.

`createMesh` volitelně přijímá `triangleColors`, ploché lineární RGB na každý
trojúhelník (počet hodnot odpovídá `indices.length`). Umožňuje jeden pohybující
se barevný mesh ruky/meče bez JS animace nebo připojování dekorací každý frame.
`smooth: true` při vytvoření spočítá plošně vážené jednotkové normály sdílených
vrcholů. Jablka tuto volbu používají; výchozí mesh zůstává plochý. Řezy interpolují
původní normálové pole slupky bez nového přepočtu/normalizace vrcholů, WGSL
normalizuje až fragment. Nové uzávěry mají samostatné ploché normály, takže
rozhraní slupka/dužina zůstává ostré i po opakovaných řezech.

`positions` je ploché pole xyz, `indices` sdílené indexy trojúhelníků s vnějším
vinutím. `createMesh` vyžaduje uzavřenou orientovanou manifold síť bez degenerovaných
trojúhelníků; ověřuje hrany a kladný objem. Vstup musí být bez samoprůniků
(test všech párů trojúhelníků se neprovádí). Mesh může být konkávní. Každá
samostatná souvislá část dostane vlastní ID. `meshInfo` vrací lokální objem,
počet vrcholů/trojúhelníků, `closed`, `cuttable`, aktuální `position`/`rotation`
a světové minimum `bottom`. Barvy jsou lineární RGB 0–1.
Primitiva a glTF zatím nelze označit jako řezatelná; lze je použít jako čepel.

Čepel je lokální úsečka konkrétního objektu, transformovaná aktuální maticí.
`motion` je světový posun. Úsečka a posun vytvoří konečný rovnoběžník;
celý průřez cíle musí ležet uvnitř. Dotyk, minutí a krátká čepel objekt
nemění. `separation` (0–2 světové jednotky) odsune kusy na obě strany roviny.
Nejsou to rigid-body síly. Meč dokončí vizuální tah za 0,24 s v Rust simulaci;
geometrie se dělí při příkazu. Další polohovací příkaz může tah přerušit.
`setCuttable(false)` chrání existující mesh, `setCutter(null)` odregistruje čepel.
Meč nesmí být zároveň řezatelný.

Všechny zásahy se připraví před změnou scény. Chyba topologie nebo překročení
rozpočtu zachová zdroje. Úspěch odstraní původní ID a jejich naplánované akce;
staré handles se stanou neplatnými. Fyzikální box collidery ani pohybové akce se
na kusy nepřenášejí. `remove`, plánované odstranění, `clear` a `dispose` uvolní
CPU mesh i následně GPU buffery.

## Algoritmus a současné limity

Dvouúrovňové BVH: refit světových AABB řezatelných objektů a persistentní lokální
BVH trojúhelníků. Přesný clipping se provádí pro trojúhelníky u roviny; zbytek
se jen přiřadí k poloprostoru. Mesh se při řezu musí projít a výsledky zapsat,
takže BVH neznamená sublineární cenu celé operace. Průsečíky sdílených hran
mají společná ID. Z otevřených hran vzniknou smyčky; průřez se uzavře fanem
z bodu v jádře polygonu, jinak ear clippingem. Následuje rozklad na komponenty,
kontrola orientovaných hran a zachování objemu.

Podporované jsou jednoduché oddělené průřezové smyčky; vnořené smyčky (otvory,
např. průřez torusem) se odmítnou. Neřeší se částečné zářezy, zakřivená dráha,
skinning, UV ani automatické dělení při běžném kontaktu. Číselná tolerance je
`1e-5` lokální jednotky; extrémní měřítka a téměř degenerované řezy mohou být
odmítnuty. Limit je 20 000 trojúhelníků na kus, 200 000 na scénu, 4 096 bodů
na obrys průřezu a 64 výsledných komponent na jedno těleso.

CPU implementace v Rust/WASM běží stejně na WebGPU i WebGL. Compute shader
není potřeba pro tuto první verzi: proměnlivá topologie, smyčky a nové ID se
lépe kontrolují na CPU a odpadají GPU readbacky. Pozdější WebXR může dodat
transformaci čepele a tah; dosavadní API nezávisí na ovladači ani XR session.

Návrh vychází z [PBRT: BVH](https://www.pbr-book.org/4ed/Primitives_and_Intersection_Acceleration/Bounding_Volume_Hierarchies),
[CGAL: clipping uzavřených objemů](https://doc.cgal.org/latest/PMP_Boolean_operations/group__PMP__clip__grp.html)
a [Mapbox Earcut: triangulace polygonů](https://github.com/mapbox/earcut).
Jde o vlastní omezenou implementaci, tyto knihovny se neimportují.

## Ověření

`cargo test cutting` pokrývá objem, uzavřenost, konkávní průřezy, komponenty,
opakované řezy, transformace, konečnou čepel, opt-in, BVH a cleanup.
`npm run verify:browser -- --page cutting,scene-sdk` sestaví matching WASM a
ověří oba backendy. Cutting používá skutečnou myš, tlačítka a klávesnici,
kontroluje dvě/čtyři části, dosednutí, opakované řezy a světlou dužinu v canvasu.
Testuje posuvníky včetně náklonu, mezerník s focusem na posuvníku, reset/minutí,
kliknutí na různá jablka, režim ruky/plánu/kamery, pravé i levé otáčení,
kolečko, WASD, resize/home, zmizení skutečných zelených pixelů po vypnutí vodítka
a další dotyk během tahu. `render_game_to_text` vrací kusy se skutečnými Rust
metadaty, původní jablka, kameru, nastavení a stav tahu. Nativní regrese pokrývá
jemný řez u zaobleného pólu, který vzniká při reálné projekci kurzoru.
