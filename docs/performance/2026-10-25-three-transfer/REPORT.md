# Přenos postupů z Three.js do Rust/WASM — 25.10.2026

Změny: přímé složení transformační matice YXZ a využití již publikované matice objektu v glTF přípravě. Bez změny kvality, shaderu, geometrie, počtu objektů, simulace nebo XR prezentace. Další kroky Three.js nejsou automaticky pokládány za vhodné či ověřené.

## Co jsme převzali a proč

Lokální Three r170 `Matrix4.compose` zapisuje výslednou matici přímo; `Object3D.updateMatrixWorld` používá připravenou lokální matici. U nás se tvořilo/násobilo pět dílčích matic a glTF publikace navíc znovu počítala root, přestože už je uložen v `ObjectRenderData`. Přeneseny jsou tyto principy, nikoli Three renderer. Zachována naše Euler konvence T·Ry·Rx·Rz·S, obecné glTF normal matrices i existující invalidace.

[Podrobný deník a zdrojový audit](../THREE_TRANSFER_2026-10-25.md). Archiv obsahuje přesný použitý Three r170 zdroj s MIT licencí. Příprava samostatných objektů, hledání instancovaných skupin, SDK přechody a materiály nejsou mezi oběma enginy totožné; nelze tím vysvětlit každou milisekundu rozdílu.

## Identita a postup

Vlastní větev `codex/wasm-render-performance`, základ `64bfda680`; navazuje na předchozí overlap optimalizaci. Baseline `6f9a3bba`, přímé skládání `6236f6b4`, kombinace s cache `292f27c1`. Úplné hashe a manifesty jsou v archivu. Desktop Apple M1/Chrome, headless ANGLE/Metal; fyzický Quest neověřený. Externí GPU aktivita ani teploty nejsou potvrzené. 960 × 640, DPR 1, AA vypnuté, seed 1337, stejné assety/kamera a elapsed-time pohyb.

Hlavní test: 6144 viditelných objektů, 12288 trojúhelníků, 6 skutečných příkazů; 3 střídavé páry AB/BA/AB, 8s zahřátí a 25s měření, profiler prokazatelně vypnutý. Kalibrace proběhla jen na 6f9; shodné zátěže pro oba buildy. CPU je součet aplikačních RAF callbacků, nikoli jen čisté GPU/encoding. RAF kadence není compositor/input-to-display.

[Skutečně spuštěné příkazy a stavy](commands.json) · [Předem zaznamenaný plán](plan.json) · [Ověření](validation.json) · [Hashy archivu](archive-manifest.json)

## Hlavní párové měření

| Backend/verze/pár | CPU medián/p95/p99 ms | RAF p95/p99 ms | RAF překročení počet / součet nad budget ms |
|---|---:|---:|---:|
| wg baseline 0 | 15.60/18.50/21.80 | 16.80/16.80 | 11/183.37 |
| wg candidate 0 | 15.10/18.80/24.80 | 16.80/33.30 | 27/450.40 |
| wg candidate 1 | 14.60/17.20/18.40 | 16.70/16.80 | 1/16.63 |
| wg baseline 1 | 15.40/17.80/18.50 | 16.80/16.80 | 1/16.63 |
| wg baseline 2 | 15.60/18.50/21.60 | 16.80/33.30 | 16/266.83 |
| wg candidate 2 | 14.90/17.60/20.20 | 16.80/16.80 | 7/116.83 |

wg: medián párových poměrů CPU 0.955128, úspora 4.49%; pixelové páry [True, True, True]. [Data](paired-wg/summary.json).

| Backend/verze/pár | CPU medián/p95/p99 ms | RAF p95/p99 ms | RAF překročení počet / součet nad budget ms |
|---|---:|---:|---:|
| gl baseline 0 | 15.60/19.00/23.00 | 16.80/33.20 | 16/266.53 |
| gl candidate 0 | 15.70/21.30/26.40 | 16.80/33.40 | 53/916.67 |
| gl candidate 1 | 14.70/17.10/19.70 | 16.70/16.80 | 4/66.83 |
| gl baseline 1 | 15.60/18.30/20.50 | 16.80/16.80 | 12/200.00 |
| gl baseline 2 | 15.50/17.70/18.30 | 16.80/16.80 | 1/16.73 |
| gl candidate 2 | 14.70/17.30/20.30 | 16.80/16.80 | 3/50.00 |

gl: medián párových poměrů CPU 0.948387, úspora 5.16%; pixelové páry [True, True, True]. [Data](paired-gl/summary.json).


Změny ponecháváme pro doloženou úsporu přípravy: všechny tři WebGPU páry a dva ze tří WebGL párů mají nižší CPU medián. První WebGL pár se zhoršil o 0,1 ms. Špičky nejsou ve všech bězích lepší a RAF mediány zůstaly kolem 16,7 ms; nejde o obecné zrychlení FPS.

Rozpočet 16,667 ms, tolerance 0,5 ms. Překročení je součet času nad budget u intervalů nad tolerancí; zaokrouhlení kolem16,67ms samo není regrese. Plné průměry/maxima, raw časování a cold-ready rozsah jsou v reportech. První direct-only pilot a malé kontroly jsou oddělené a nezakládají samostatné procento pro každou změnu.

## Samostatný profil a prostředky

Profil zapnutý, 2s/8s. CPU scopes jsou vzorky přibližně 10Hz, nikoli histogram každého snímku. Diagnostika API na samostatné stránce cold-init+1s. GPU WebGPU je pojmenovaný pass; GL je asynchronní RAF command interval, může zahrnovat CPU/driver mezery; není to celé GPU frame. Nesčítat CPU+GPU ani vnořené scopes.

| Backend/verze | CPU ms | glTF update / scene.gpuUpdates ms | GPU scope ms | draw/instance/tris za RAF | upload calls / uniform+palette B za RAF | nové instance pages |
|---|---:|---:|---:|---:|---:|---:|
| wg before | 18.30 | 4.20/9.00 | 0.31 | 6.00/6144.00/12288.00 | 6.00/688128.00 | 0.00 |
| wg after | 17.50 | 4.20/8.40 | 0.25 | 6.00/6144.00/12288.00 | 6.00/688128.00 | 0.00 |
| gl before | 18.80 | 4.40/9.40 | 1.01 | 6.00/6144.00/12288.00 | 6.00/688128.00 | 0.00 |
| gl after | 17.60 | 4.40/8.70 | 1.01 | 6.00/6144.00/12288.00 | 6.00/688128.00 | 0.00 |

Upload bytes jsou glTF uniform+palette požadavky, nikoli všechny render uploady nebo fyzický sběrnicový provoz. Gauges retinované kapacity nejsou počty alokací. API příkazy/vytvoření/změny stavu jsou v `apiOperations` jednotlivých reportů; nejsou to warm allocation totals. GPU page churn se touto změnou neopravoval.

## Přímé Three.js kontroly

Oba backendové testy výše jsou náš renderer před/po. Následuje zvlášť náš WebGL2 vs lokální Three r170 WebGL2: 3 střídavé páry, 4s/10s, bez profileru. 6144 objektů/12288 trojúhelníků, shodná geometrie/grid/pohyb/rozlišení. Materiály,osvětlení,handedness a část přípravy se liší: pixel gate je vypnutý, žádná deklarovaná ekvivalence celé hry.

| Three reprezentace | Rust CPU mediány párů ms | Three CPU mediány párů ms | Rust/Three medián párového poměru | skutečné commands Rust/Three |
|---|---:|---:|---:|---:|
| objects | 14.90/14.70/14.80 | 6.60/6.80/6.80 | 2.18× | 6.00/6144.00 |
| instanced | 14.50/14.50/14.60 | 1.70/1.70/1.70 | 8.53× | 6.00/6.00 |

Instanced Three má šest předem připravených skupin a vynechává naše dynamické hledání skupin. Objects varianta vytváří samostatné root/mesh dvojice a6144commands, Rust stále6. Tyto kontroly ukazují zbývající CPU rozdíl; neprokazují univerzální náskok ani nutnost opustit Rust/WASM. Three WebGPU a celý shodný gameplay se v této sérii neměřily.

## Kontroly dynamiky a skutečných scén

Jedna krátká 2s/8s dvojice na workload/backendu,profiler off. Malé rozdíly orientační,scény na60Hz neprokazují throughput.

| Workload | CPU před/po ms | RAF p95/p99 po ms | pixelová kontrola / přesné |
|---|---:|---:|---:|
| dynamic-wg | 4.10/4.20 | 16.80/16.80 | PASS / True |
| rim-static-wg | 1.00/0.90 | 16.80/16.80 | PASS / True |
| arena-static-wg | 0.80/0.70 | 16.70/16.80 | PASS / True |
| dynamic-gl | 4.40/4.40 | 16.70/16.80 | PASS / True |
| rim-static-gl | 1.50/1.50 | 16.70/16.80 | PASS / False |
| arena-static-gl | 1.20/1.20 | 16.80/16.80 | PASS / True |

Řím WebGL2 prošel předem stanovenou pixelovou tolerancí, ale není bitově totožný: maximální rozdíl jednoho kanálu je1/255, žádný pixel nepřekračuje toleranci8. Z16 stejnorendererových párů je15 přesných; všech6 hlavních zátěžových párů je přesných. Golden images se neměnily.

## Ověření a další krok

- 15 unikátních cílených native testů, včetně 4096 numerických případů a aktuálnosti cache na skutečném Fox GLB.
- 14 maintained browser/backend případů na konečném 292f release PASS, včetně nezávislé unbatched pixel reference, mirrored/nonuniform/skin/culling/shadow a skutečných protilehlých půlek při řezu v simulovaném XR.
- Strict wasm32 Clippy PASS; independent Astra source audit bez scoped blockeru.
- Předešlé smoke a functional testy nejsou důkazem fyzického Quest výkonu. Nový push gate/commit/push/nasazení na 8443 se neprováděl.
- Další ověřitelné kandidáty: opakované přípravy matrix/inverse/determinant na primitiva stejného uzlu; oddělení pose změn od root/options změn; již doložený instance-page allocation churn. Využití root normal nebo obecná změna normál se v této iteraci nepřidávala. Změřit zvlášť se stejným protokolem a zachovat shear/singular fallback.
