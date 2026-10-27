# Zrychlení Rust/WASM rendereru — 25. 10. 2026

Změna zrychluje přípravu velkých kompatibilních skupin glTF objektů. Zachovává počet objektů, rozlišení, shadery, stíny i pořadí kreslení. Neřeší celý renderer ani neprokazuje vyšší výkon na fyzickém Questu.

## Identita a podmínky

- Vlastní větev `codex/wasm-render-performance`, základ `64bfda680483e71f6aeb6bf902e63b29b3e90791`; změny této úlohy nejsou commitnuté ani pushnuté.
- Originální release WASM `c4301d8142f89cda9472095b128a458372101d646a7b9ed1980e84a5569b526a`; finální release `6f9a3bba80c0681fbf4908ac4dbda7fd5088ed72aa6c4f8523b3a2d5a68518b9`.
- Apple M1, Chrome155, hardwarové ANGLE/Metal, desktop headless, 960×640, DPR1, seed1337. Běhy a buildy agenta sériově; nepřítomnost jiné GPU aktivity nebyla potvrzena.
- Primární protokol: tři páry AB/BA/AB, 8s zahřátí +25s měření, profiler prokazatelně vypnutý. Doplňkové rodiny/kapacita mají kratší protokol uvedený v reportech; neslouží jako jistý důkaz malých zrychlení.
- Původní produkční balíček na8443 a reference na8878 zůstaly zachované. Tato úloha nenasadila nový build na8443.
- [Přesné spuštěné příkazy](commands.json), [hashovaný seznam archivu](archive-manifest.json), [protokol](../../RENDER_PERFORMANCE.md), [deník včetně zamítnutých pokusů](../EXPERIMENTS_2026-10-25.md).

## Primární výsledek — 6144 viditelných objektů

Šest skutečných instancovaných příkazů a 12 288 trojúhelníků na aplikační snímek. U původního buildu se trojúhelníky určují z ověřené geometrie a skutečně odeslaných instancí; nový přímý čítač ještě neobsahuje. Před/po stejná geometrie, kamera, měřítko i animace. Porovnání AABB nyní používá omezený přesný strom; po osmi vyčerpáních rozpočtu v jedné skupině přepne na původní lineární dotaz.



### WebGPU

| Běh | CPU medián/p95/p99/max ms | RAF medián/p95/p99 ms | RAF překročení: počet / součet nad rozpočet ms |
|---|---:|---:|---:|
| WebGPU baseline pár0 | 29.30/33.50/37.50/41.10 | 33.30/33.40/33.50 | 670 / 11266.23 |
| WebGPU candidate pár0 | 15.30/17.70/19.10/26.50 | 16.70/16.70/16.80 | 1 / 16.63 |
| WebGPU candidate pár1 | 15.40/18.50/21.00/24.20 | 16.70/16.80/16.80 | 11 / 183.67 |
| WebGPU baseline pár1 | 29.50/32.90/35.30/38.10 | 33.30/33.40/33.50 | 669 / 11199.40 |
| WebGPU baseline pár2 | 29.90/32.80/34.20/37.70 | 33.30/33.40/33.40 | 680 / 11365.97 |
| WebGPU candidate pár2 | 15.60/20.00/23.00/30.90 | 16.70/16.80/33.30 | 29 / 482.97 |

WebGPU: medián poměrů CPU 0.52; úspora 47.80 %. [final-paired-off-wg](final-paired-off-wg/summary.json). Obraz: [True, True, True].

Studená stránka baseline: ready 742–773ms (n=3, obsahuje setup/network, OS/driver cache nejsou vyčištěné).
Studená stránka candidate: ready 477–531ms (n=3, obsahuje setup/network, OS/driver cache nejsou vyčištěné).

### WebGL2

| Běh | CPU medián/p95/p99/max ms | RAF medián/p95/p99 ms | RAF překročení: počet / součet nad rozpočet ms |
|---|---:|---:|---:|
| WebGL2 baseline pár0 | 32.50/36.70/40.50/45.90 | 33.30/33.40/50.00 | 730 / 12548.53 |
| WebGL2 candidate pár0 | 16.80/20.50/22.90/30.10 | 16.70/33.30/33.40 | 76 / 1266.63 |
| WebGL2 candidate pár1 | 16.70/20.50/24.10/31.40 | 16.70/33.30/33.40 | 78 / 1299.90 |
| WebGL2 baseline pár1 | 32.70/37.00/40.20/44.20 | 33.30/49.90/50.00 | 720 / 12631.80 |
| WebGL2 baseline pár2 | 31.90/36.20/38.40/41.80 | 33.30/33.40/50.00 | 725 / 12299.17 |
| WebGL2 candidate pár2 | 17.00/22.00/27.20/46.80 | 16.70/33.30/33.40 | 117 / 1983.90 |

WebGL2: medián poměrů CPU 0.52; úspora 48.31 %. [final-paired-off-gl](final-paired-off-gl/summary.json). Obraz: [True, True, True].

Studená stránka baseline: ready 970–1219ms (n=3, obsahuje setup/network, OS/driver cache nejsou vyčištěné).
Studená stránka candidate: ready 651–707ms (n=3, obsahuje setup/network, OS/driver cache nejsou vyčištěné).
Překročení používá předem určený rozpočet16,667ms a toleranci0,5ms. Číslo je součet času nad rozpočet u intervalů překračujících toleranci, ne doba všech pomalých snímků. RAF je aplikační kadence, ne přímé měření prezentace v headsetu. Celé distribuce včetně průměru a přesných překročení jsou v JSON; jednotlivé snímky v raw.json.gz.

## Profil a prostředky

Krátký profil1s/8s je samostatné měření se zapnutým profilerem. GPU WebGPU = asynchronní pojmenovaný pass; GL = EXT interval příkazů mezi RAF callbacky, který může zahrnovat mezery CPU/driveru. Tyto scope nejsou vzájemně zaměnitelné ani celé GPU frame.

| Backend/verze | CPU ms | glTF queue ms | scene.gpuUpdates ms | GPU scope medián ms | skutečné glTF draws / instance / tris | glTF upload volání / uniform+palette B za snímek | nové stránky během měření | scratch B |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| WebGPU baseline | 32.80 | 17.70 | 9.00 | 0.35 | 6.00 / 6144.00 / — | 6.00 / 688128.00 | — | 569408 |
| WebGPU candidate | 18.60 | 3.50 | 9.10 | 0.25 | 6.00 / 6144.00 / 12288.00 | 6.00 / 688128.00 | 0.00 | 1065152 |
| WebGL2 baseline | 33.70 | 17.90 | 9.30 | 1.36 | 6.00 / 6144.00 / — | 6.00 / 688128.00 | — | 598080 |
| WebGL2 candidate | 18.70 | 3.40 | 9.40 | 0.78 | 6.00 / 6144.00 / 12288.00 | 6.00 / 688128.00 | 0.00 | 1151168 |

Starší baseline nové čítače neobsahuje: „—“ znamená nedostupné. `renderer_stats().drawCalls` je starší odhad počtu objektových batchů; zde se používají skutečné glTF/draw debug čítače. Aktivní/retinované stránky jsou poslední gauge na pass/view klíč; scratch je retinovaná kapacita, nikoli všechny alokace. GPU page pooling se v této změně neopravoval.

Samostatná instrumentovaná stránka zaznamenává skutečná API vytvoření bufferů/textur/bind groups/pipeline, uploady, průchody, submissions a změny stavu. Jde o cold-init+1s, nikoli důkaz nulových warm alokací. Kompletní počty jsou `apiOperations` v každém summary. Profilerové fáze se mohou vnořovat: nesčítat `gltf.update` se `scene.gpuUpdates`.

## Kapacita

Krátký průzkumný párový sweep0,5s/2,5s, stejné původně kalibrované loady, profiler off. Nejvyšší úspěšný **testovaný** počet není globální maximum. Kritérium: RAF medián≤budget+0,5ms, p95≤2×budget+0,5ms, podíl vynechaných intervalů≤10%, CPU p95≤budget+0,5ms.

| Backend | Původní nejvyšší úspěšný test | Finální nejvyšší úspěšný test | Nejvyšší zkoušený počet |
|---|---:|---:|---:|
| WebGPU | 3840 | 4096 | 6144 |
| WebGL2 | 3584 | 4096 | 6144 |

## Ostatní workloady

Pevný baseline load, jeden doplňkový pár; rigid/skinning/morph/fragment/effects/heterogeneous/dynamic2s/5s, skutečné hry2s/8s. Zapnutý profiler. Malé rozdíly jsou orientační. PBR/fragment overdraw je označená syntetická diagnostika. Aktivní hry mají časované vstupy, ale capture neresetuje nativní herní stav: pixelová shoda je nedostupná, ne PASS.

| Backend / workload | Load | CPU medián před→po ms | GPU scope: medián před→po ms | RAF p95/p99 po ms | RAF překročení po počet/ms | Obraz |
|---|---:|---:|---:|---:|---:|---|
| WebGPU rigid | 64 | 1.10→1.10 | `scene.main` 5.01→4.89 | 16.70/16.80 | 0/0.00 | PASS |
| WebGPU skin | 32 | 2.20→2.30 | `scene.main` 0.61→0.64 | 16.80/16.80 | 0/0.00 | PASS |
| WebGPU morph | 32 | 2.30→2.30 | `scene.main` 1.24→1.79 | 16.80/16.80 | 0/0.00 | PASS |
| WebGPU fragment | 64 | 0.60→0.60 | `scene.main` 6.96→6.79 | 16.80/16.80 | 0/0.00 | PASS |
| WebGPU effects | 32 | 0.70→0.70 | `final.postprocess.overlays` 7.75→7.37 | 16.80/16.80 | 0/0.00 | PASS |
| WebGPU batching-heterogeneous | 256 | 3.00→3.10 | `scene.main` 0.13→1.03 | 16.80/16.80 | 0/0.00 | PASS |
| WebGPU dynamic | 512 | 4.00→4.30 | `scene.main` 0.12→0.12 | 16.70/16.80 | 0/0.00 | PASS |
| WebGPU rim-static | 1 | 0.80→0.80 | `scene.main` 2.01→2.90 | 16.80/16.80 | 0/0.00 | PASS |
| WebGPU arena-static | 1 | 0.70→0.60 | `final.postprocess.overlays` 1.54→1.74 | 16.70/16.80 | 0/0.00 | PASS |
| WebGPU rim-active | 1 | 2.40→2.30 | `scene.main` 2.71→3.19 | 16.70/16.80 | 0/0.00 | nedostupné |
| WebGPU arena-active | 1 | 0.90→1.00 | `final.postprocess.overlays` 3.09→3.78 | 16.70/16.80 | 0/0.00 | nedostupné |
| WebGL2 rigid | 64 | 1.40→1.30 | `GL RAF interval` 5.03→7.20 | 16.70/16.80 | 0/0.00 | PASS |
| WebGL2 skin | 32 | 2.60→2.80 | `GL RAF interval` 2.31→1.64 | 16.80/16.80 | 0/0.00 | PASS |
| WebGL2 morph | 32 | 3.40→3.40 | `GL RAF interval` 1.63→1.26 | 16.80/16.80 | 0/0.00 | PASS |
| WebGL2 fragment | 64 | 2.00→2.00 | `GL RAF interval` 8.24→8.51 | 16.80/16.80 | 0/0.00 | PASS |
| WebGL2 effects | 64 | 2.30→2.30 | `GL RAF interval` 60.69→59.85 | 33.40/33.40 | 80/1333.07 | PASS |
| WebGL2 batching-heterogeneous | 256 | 3.80→4.40 | `GL RAF interval` 3.68→3.39 | 16.70/16.80 | 0/0.00 | PASS |
| WebGL2 dynamic | 512 | 4.40→4.80 | `GL RAF interval` 0.76→2.31 | 16.70/16.80 | 0/0.00 | PASS |
| WebGL2 rim-static | 1 | 1.60→1.60 | `GL RAF interval` 6.03→6.32 | 16.80/16.80 | 0/0.00 | PASS |
| WebGL2 arena-static | 1 | 1.40→1.30 | `GL RAF interval` 3.80→3.66 | 16.70/16.80 | 0/0.00 | PASS |
| WebGL2 rim-active | 1 | 3.50→3.50 | `GL RAF interval` 9.05→7.59 | 16.70/16.80 | 0/0.00 | nedostupné |
| WebGL2 arena-active | 1 | 1.90→1.90 | `GL RAF interval` 5.40→5.89 | 16.80/16.80 | 0/0.00 | nedostupné |

| Backend/workload/verze | glTF draw / instances / triangles za snímek | glTF upload calls / uniform+palette B za snímek | celý renderer upload B, medián vzorků10Hz | nové stránky / buffery / bindgroups v měření |
|---|---:|---:|---:|---:|
| WebGPU rigid baseline | 1.00/64.00/— | 1.00/7168.00 | 7520.00 | —/—/— |
| WebGPU rigid candidate | 1.00/64.00/988928.00 | 1.00/7168.00 | 7520.00 | 0.00/0.00/0.00 |
| WebGPU skin baseline | 32.00/32.00/— | 32.00/49152.00 | 49504.00 | —/—/— |
| WebGPU skin candidate | 32.00/32.00/18432.00 | 32.00/49152.00 | 49504.00 | 0.00/0.00/0.00 |
| WebGPU morph baseline | 96.00/96.00/— | 96.00/0.00 | 37216.00 | —/—/— |
| WebGPU morph candidate | 96.00/96.00/192.00 | 96.00/0.00 | 37216.00 | 0.00/0.00/0.00 |
| WebGPU fragment baseline | 0.00/0.00/— | 0.00/0.00 | 352.00 | —/—/— |
| WebGPU fragment candidate | 0.00/0.00/0.00 | 0.00/0.00 | 352.00 | 0.00/0.00/0.00 |
| WebGPU effects baseline | 0.00/0.00/— | 0.00/0.00 | 352.00 | —/—/— |
| WebGPU effects candidate | 0.00/0.00/0.00 | 0.00/0.00 | 352.00 | 0.00/0.00/0.00 |
| WebGPU batching-heterogeneous baseline | 256.00/256.00/— | 256.00/61440.00 | 61792.00 | —/—/— |
| WebGPU batching-heterogeneous candidate | 256.00/256.00/512.00 | 256.00/61440.00 | 61792.00 | 0.00/0.00/0.00 |
| WebGPU dynamic baseline | 41.62/458.88/— | 42.03/56368.11 | 52880.00 | —/—/— |
| WebGPU dynamic candidate | 41.27/458.90/917.79 | 41.67/56321.54 | 52880.00 | 123.00/246.00/123.00 |
| WebGPU rim-static baseline | 39.00/39.00/— | 0.00/0.00 | 352.00 | —/—/— |
| WebGPU rim-static candidate | 39.00/39.00/67292.00 | 0.00/0.00 | 352.00 | 0.00/0.00/0.00 |
| WebGPU arena-static baseline | 0.00/0.00/— | 0.00/0.00 | 352.00 | —/—/— |
| WebGPU arena-static candidate | 0.00/0.00/0.00 | 0.00/0.00 | 352.00 | 0.00/0.00/0.00 |
| WebGPU rim-active baseline | 168.16/371.52/— | 135.59/46508.37 | 49328.00 | —/—/— |
| WebGPU rim-active candidate | 168.14/371.50/169088.88 | 135.28/46433.53 | 49328.00 | 3.00/6.00/3.00 |
| WebGPU arena-active baseline | 0.00/0.00/— | 0.00/0.00 | 4224.00 | —/—/— |
| WebGPU arena-active candidate | 0.00/0.00/0.00 | 0.00/0.00 | 4224.00 | 0.00/0.00/0.00 |
| WebGL2 rigid baseline | 1.00/64.00/— | 1.00/7168.00 | 7520.00 | —/—/— |
| WebGL2 rigid candidate | 1.00/64.00/988928.00 | 1.00/7168.00 | 7520.00 | 0.00/0.00/0.00 |
| WebGL2 skin baseline | 32.00/32.00/— | 32.00/49152.00 | 49504.00 | —/—/— |
| WebGL2 skin candidate | 32.00/32.00/18432.00 | 32.00/49152.00 | 49504.00 | 0.00/0.00/0.00 |
| WebGL2 morph baseline | 96.00/96.00/— | 96.00/0.00 | 37216.00 | —/—/— |
| WebGL2 morph candidate | 96.00/96.00/192.00 | 96.00/0.00 | 37216.00 | 0.00/0.00/0.00 |
| WebGL2 fragment baseline | 0.00/0.00/— | 0.00/0.00 | 352.00 | —/—/— |
| WebGL2 fragment candidate | 0.00/0.00/0.00 | 0.00/0.00 | 352.00 | 0.00/0.00/0.00 |
| WebGL2 effects baseline | 0.00/0.00/— | 0.00/0.00 | 352.00 | —/—/— |
| WebGL2 effects candidate | 0.00/0.00/0.00 | 0.00/0.00 | 352.00 | 0.00/0.00/0.00 |
| WebGL2 batching-heterogeneous baseline | 256.00/256.00/— | 256.00/61440.00 | 61792.00 | —/—/— |
| WebGL2 batching-heterogeneous candidate | 256.00/256.00/512.00 | 256.00/61440.00 | 61792.00 | 0.00/0.00/0.00 |
| WebGL2 dynamic baseline | 41.73/458.90/— | 42.14/56383.63 | 52880.00 | —/—/— |
| WebGL2 dynamic candidate | 41.78/458.89/917.77 | 42.19/56388.47 | 52880.00 | 123.00/246.00/123.00 |
| WebGL2 rim-static baseline | 39.00/39.00/— | 0.00/0.00 | 352.00 | —/—/— |
| WebGL2 rim-static candidate | 39.00/39.00/67292.00 | 0.00/0.00 | 352.00 | 0.00/0.00/0.00 |
| WebGL2 arena-static baseline | 0.00/0.00/— | 0.00/0.00 | 352.00 | —/—/— |
| WebGL2 arena-static candidate | 0.00/0.00/0.00 | 0.00/0.00 | 352.00 | 0.00/0.00/0.00 |
| WebGL2 rim-active baseline | 168.16/371.51/— | 135.36/46453.69 | 49328.00 | —/—/— |
| WebGL2 rim-active candidate | 168.18/371.52/169094.12 | 135.36/46453.95 | 49328.00 | 3.00/6.00/3.00 |
| WebGL2 arena-active baseline | 0.00/0.00/— | 0.00/0.00 | 4224.00 | —/—/— |
| WebGL2 arena-active candidate | 0.00/0.00/0.00 | 0.00/0.00 | 4224.00 | 0.00/0.00/0.00 |

GPU sloupec vybírá nejdelší jednotlivý pojmenovaný scope z baseline, nikoli celý frame ani součet passů; všechny passy včetně postprocess jsou zachované v JSON. Uniform+palette byte sloupec používá uniformUploadBytes; palette je jeho podmnožina a nesčítá se znovu. Nezahrnuje průběžný CPU morph vertex upload. Proto je vedle uveden skutečný per-frame renderer.uploadBytes, medián vzorků přibližně10Hz (požadované uploady, nikoli fyzický přenos po sběrnici). Morph zde deformuje384vertexů×96B; nulový uniform čítač neznamená nulové uploady. glTF čítače nepokrývají primitivní/neon/SSR kreslení. Celé API počty jsou samostatně v JSON. Tvrzení o animaci nebo GPU geometrii vycházejí z reálně vykreslených assetů a čítačů; u skin/morph se nevydává deklarovaný počet rootů za skutečný počet trojúhelníků.

## Delší dynamický běh a řezy

Dynamický soak:2s zahřátí+120s, původní kalibrace stejného harnessu, finální release. WebGPU běh má profiler zapnutý; WebGL2 dlouhý běh jej výslovně vypíná. Tyto dva běhy nejsou párovým srovnáním backendů. Dvanáctitrojúhelníkové uzavřené řezané testovací těleso slouží k opakovanému spawn/cut/despawn; samostatný herní cut benchmark níže ověřuje skutečné půlky nepřítele.

| Backend | Load | CPU medián/p99/max ms | RAF p95/p99/max ms | cuts / spawns / explicit removes | root culling min/max |
|---|---:|---:|---:|---:|---:|
| WebGPU | 512 | 4.20/5.50/6.30 | 16.70/16.80/16.80 | 61/61/183 | 0/153 |
| WebGL2 | 512 | 4.40/6.60/8.30 | 16.80/16.80/16.80 | 61/61/183 | 0/153 |

Top-level culling může předcházet glTF primitive testu; nulová delta glTF cull neznamená nulové root culling. `removes` počítá explicitní odstranění cutteru+dvou půlek; původní target odstraňuje nativní cut a počítá jej událost cuts.

Každá následující sada má tři skutečné běhy simulovaného XR. Latence je od prvního neprázdného požadavku na řez po konec prvního simulačního XR snímku s kompletními půlkami; nezahrnuje potvrzenou prezentaci na fyzickém displeji. Měří se handler onmessage na hlavním vlákně, serializace požadavku a nativní instalace. Doba samotného výpočtu uvnitř workeru není samostatně instrumentovaná; nesmí se zaměnit za krátký onmessage handler.

| Backend/verze | request→publish medián/p95/max ms | hlavní onmessage max ms | transport prepare max ms | stav |
|---|---:|---:|---:|---|
| WebGPU baseline | 160.80/163.60/163.60 | 0.20 | 1.10 | PASS |
| WebGPU candidate | 159.90/163.60/163.60 | 0.30 | 1.10 | PASS |
| WebGL2 baseline | 158.90/159.90/159.90 | 0.30 | 1.10 | PASS |
| WebGL2 candidate | 160.40/162.30/162.30 | 0.20 | 1.10 | PASS |

## Three.js r170

Samostatná geometrická reference: shodné vektory kamery/FOV (nativní LH a Three RH konvence se liší),4vertexy/6indexů na quad, +Z normály, počet objektů, elapsed-time rotace ve shodném pořadíYXZ, viewport a AAfalse. PBR/osvětlení, materiálový systém a část přípravy se liší. Pixel gate je výslovně vypnutý. Není to důkaz výkonu shodné celé hry. Starší `three-geometry-gl` běh používal odlišné pořadí Euler rotací a je pouze historický, nikoli finální odpovídající pohyb.

| Reprezentace / verze | CPU mediány 3 párů ms | skutečné draw commands / tris |
|---|---:|---:|
| instanced Rust final | 15.80/15.80/15.80 | 6.00/12288.00 |
| instanced Three.js | 1.50/1.50/1.70 | 6.00/12288.00 |
| objects Rust final | 15.60/15.50/15.50 | 6.00/12288.00 |
| objects Three.js | 7.10/7.00/7.00 | 6144.00/12288.00 |

Instanced varianta má předem vytvořené skupiny1024 objektů a šest příkazů, zatímco objects vytváří6144 nezávislých root/mesh dvojic a6144 příkazů. První vynechává dynamické hledání skupin; druhá zvyšuje náklad odesílání. Obě hranice jsou užitečné pro profilování, žádná sama neurčuje univerzálního vítěze. GPU čas v těchto profiler-off kontrolách je nedostupný.

## Korektnost, zamítnuté varianty a další krok

- 7 nativních testů nezávislého pairwise oracle;14 skutečných udržovaných browser/backend testů včetně unbatched obrazové reference, overlap/pořadí/mirror/nonuniform, skin/culling, viditelných půlek/evaporace a simulovaného stereoXR. Strict WASM Clippy a SDK typecheck PASS. Nativní testy samy WASM render neověřují.
- První spatial výhra zakryla regresi nepříznivého pořadí: diagnostická varianta f69de1cf měla asi28→55ms. Zachována a odmítnuta. Samostatné counterfix/purefallback varianty32,9/30,9ms proti28,3ms také nebyly přijaty. Finální adaptivní fallback prošel trojím adversarial párem; nepříznivé malé delty až~3% jsou otevřeně zachované.
- Úplně překryté drobné quady měly pouze4viditelnépixely a oba buildy selhaly foreground gate. Nejde o platný výkonový výsledek;97%částečný překryv kontrolu obsahu splnil.
- Stride96B, CPU morph fallback, per-primitive inverse/determinant/bounds a GPU page churn zůstávají. WebGL XR druhý kontext/upload/blit, foveace/eye targets a absent-normal shader jsou auditované, ale neoptimalizované samostatné kandidáty. Předchozí zamítnutá uniform arena se neopakovala.
- Další experimenty podle důkazů: dynamický WebGPU soak vytvořil2648stránek/5296bufferů/2648bindgroups za120s při stabilním scratch a skutečném root cullingu0–153. Ověřit oddělení aktivního počtu od omezeného retinovaného poolu při zachování pass/view vlastnictví. Pro CPU throughput v nyní dominantním `scene.gpuUpdates` oddělit přechod SDK→WASM, world transforms a opakované per-node inverse/bounds; ověřit per-node cache a dávkový vstup na víceprimitivových animovaných assetech. Zachovat stejný pohyb, data a neporušit rollback. Nezačínat shaderovým přepisem, když hlavní pass zde stojí řádově0,25ms.
- Fyzický Quest3: výkon, teplotní drift, eye rozměry/Hz/foveace a input-to-display latence **neověřené**. Desktop/simulované XR tyto závěry nenahrazuje.

Úplná data a nepříznivé běhy zůstávají v archivu; počet úspěšných testů není důkaz „vše dokonale optimalizováno“.

## Dodatečné kontroly nejasných výsledků

- WebGPU dynamická zátěž, tři páry2s/10s bez profileru: CPU mediány původní 4.40/4.40/4.20ms, finální 4.10/4.30/3.50ms; medián poměrů 0.93. Přesný obraz: [True, True, True]. [final-dynamic-paired-off-wg](final-dynamic-paired-off-wg/summary.json). Předem podmíněná kontrola následuje po krátkém nepříznivém rozdílu nad5%.
- WebGL2 dynamická zátěž, tři páry2s/10s bez profileru: CPU mediány původní 4.50/4.30/4.20ms, finální 4.50/4.40/3.90ms; medián poměrů 1.00. Přesný obraz: [True, True, True]. [final-dynamic-paired-off-gl](final-dynamic-paired-off-gl/summary.json). Předem podmíněná kontrola následuje po krátkém nepříznivém rozdílu nad5%.
- Efekty64 bez profileru, jeden2s/5s pár: CPU 0.50→0.60ms; RAFp99 16.80→16.80ms. GPU timing nedostupný záměrně; výsledek odděluje reálné renderování od neúspěšného native timestamp readbacku. [final-effects-off-compare-wg](final-effects-off-compare-wg/summary.json).
- Čerstvý proces přes benchmark-core.mjs spustil skutečný smoke finálního release,64 viditelných objektů, WebGPU/profiler off; manifest a skutečné příkazy uložené. Hlavní kalibrace a třípárová regression kontrola jsou rovněž samostatné nové Node/browser procesy. Schema skillu validováno dostupným quick_validate.py. Viz fresh-workflow.json.gz a validation-commands.json.gz.
