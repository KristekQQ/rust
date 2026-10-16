Original prompt: Jablko na stole, řezání mečem shora dolů s nastavitelnou orientací; kusy se mají rozevřít jako základ Fruit Ninja. Kamera zůstává šikmo na stůl.

## Aktuální rozhodnutí pro další instance (2026-10-16)

- Compute řezání byl experiment. Pro současná jablka/jednotlivé řezy vyšlo v lokálních testech lépe optimalizované CPU; `auto` ponechat CPU, GPU režimy jen explicitně pro experimenty/porovnání. Nepovažovat další přesun na GPU automaticky za zrychlení. Quest 3 není změřený; nejde o závěr proti WebGPU rendereru nebo compute obecně. Podrobnosti a podmínky nového měření: [README.md](README.md).
- Závěr je i ve verzovaném projektovém skillu `.agents/skills/rust-webgpu-project/SKILL.md` (dřívější osobní kopie přesunuta do repozitáře). Pouze dokumentace/komentář, bez změny výpočetních cest.

## 2026-10-16: residentní GPU cutting, CPU optimalizace, směr Quest 3

- Nový explicitní `execution: 'gpu-resident'` a volba v example. GPU provádí clipping, hash obrysu, uzávěry, bounds/objem, support a vertex/index buffery; renderer je převezme přímo. Opakovaný řez používá GPU zdroj. Úspěch mapuje jen 640 B metadat/kandidát, nikoli geometrii. Scéna/IDs/transformace zůstávají Rust CPU. Auto nadále CPU.
- MeshData odděluje metadata od geometrie a líné CPU reference historie. Metadata/HUD/animace CPU geometrii nevytvářejí ani po libovolné rotaci/měřítku; mimo support trajektorii vrací bottom konzervativní AABB podporu. CPU fallback nebo explicitní meshSnapshot mohou historii přehrát; gpuMeshSnapshot čte skutečné GPU vertexy jen pro diagnostiku.
- GPU má omezené pracovní buffery (4096 hran), paralelní měření a nepočítá statický support 64×. Jedna souvislá genus-zero komponenta a bezpečný středový vějíř; ostatní/nejisté případy explicitně atomicky CPU fallback. Kus musí dodržet i CPU limit20k tri. Mezipolohy animace jsou konzervativně aproximované, konečné dosednutí přesné.
- CPU hashuje jen hrany roviny; BVH výstupních kusů je líné až pro další řez. Native release 19k jablko první slice ~11,0 → 4,7 ms; část práce může následovat při opakovaném řezu. Release WASM profil podporuje VERIFY_WASM_PROFILE=release a fingerprint odmítá jiný profil.
- Dvě vymezená review: opraveno převzetí aktuálních render options při commitu a odstraněn CPU replay z běžných metadata dotazů; optimalizační delta bez konkrétních nálezů. Dawn na rozdíl od Naga vyžadoval závorky v hash výrazu; opraveno a počáteční GPU chyby jsou zachyceny.
- Ověřeno: 133 native PASS +1 ignored explicitní benchmark (samostatně PASS), SDK14/14, harness6/6 (dev i release), fmt/native Clippy, WASM check/Clippy/release build a typecheck. Finální ustálený cutting/cutting-compute/cutting-resident/scene-sdk/lifecycle WebGPU+WebGL 10/10 PASS bez console/HTTP chyb; skutečná GPU uzavřenost/winding/normály/objem, opakování, zrcadlení, dosednutí skutečných vertexů, CPU pokračování, unsupported/póly, clear/move/concurrency/dispose. Prohlédnut skutečný residentní example s dužinou a stíny. Report output/verification/browser-report.json.
- Release browser medians po zahřátí: 12096tri CPU~10,0ms / GPU resident s animací~20,4ms / bez~57,3ms; 1088tri~9,5/15,5/25,2ms. Rozptyl ovlivňuje pořadí/alokace/shared render queue; první GPU example sek~322ms včetně kompilace. Nejde o důkaz zrychlení GPU ani o Quest benchmark. Starší dev čísla nelze přímo porovnávat.
- No-store server8000 HTTP200; aktualizovaný example otevřen v in-app browseru (nová karta, stará nenalezena). Bez commitu/pushe. Další krok: skutečné měření Quest3 v secure contextu, teprve pak volba/předzahřátí/pooling GPU; WebXR zatím nepřidáno. Poslední tento dokumentační zápis nemění runtime důkazy, ale zneplatní obsahový receipt příštího gate.

- Scéna a interakce se mění z technického dema na jablko/prkénko.
- Výpočty průsečíků, projekce kurzoru a animace kusů patří Rustu.
- Ověření: existující browser runner projektu; skutečné tahy, klávesy a canvas pixely. Druhý automatizační stack se nespouští.
- Implementováno: vlastní apple mesh, stopka/list, prkénko/stůl; šikmá kamera; svislý sek s natočením/posunem/náklonem. Rust projekce kurzoru a rozevření/dosednutí kolem pivotu kusu.
- Ověřeno: 116 native testů, 13 SDK testů, WASM check/Clippy/build a první browser průchod WebGPU/WebGL (2/4 kusy, objem, dužina, landing, swipe, tilt/reset/miss).
- Review odhalilo vícedotykové rušení aktivního gesta. Opraveno primární pointer kontrolou a ID filtrem při ztrátě capture; regrese PASS. Space funguje i s focusem na slideru.
- Finální průchod: typecheck a cutting + scene-sdk na WebGPU/WebGL 4/4 PASS, bez console/HTTP chyb. Prohlédnuty screenshoty celého jablka i čtyř otevřených kusů. Otevřený in-app tab obnoven a ponechán s novým jablkem.
- Zbývající hranice: řízená animace bez rigid-body kolizí, bez dužinových textur/semínek a bez WebXR. Žádný commit/push.

## 2026-10-16: více jablek, kamera, meč v ruce a stopa řezu

- Pět jablek s nezávislými handles/dekoracemi; větší prkénko. Reset plodů zachovává pohled.
- Tři režimy: meč v ruce (barevná ruka/prsty/rukojeť v jednom meshi), plánování tahem a kamera. RMB/Shift drag, wheel a WASD nepřebírají LMB řezání; přidán home/zoom a ovládání na dotyku.
- Rust aimCutter z jedné roviny spočítá čepel, konečný tah a 24 zelených 3D pomlček na prkénku. Native test ověřuje rovinu i při náklonu. Kamera seed/adjust, worldToScreen a triangleColors jsou nové tenké SDK příkazy.
- Review: ošetřeny neplatné projekce kurzoru, které původně mohly seknout na starém místě. Browser regrese pro off-board klik. Reálný kurzor odhalil příliš přísný limit plochy u apple-pole skinny triangles; opraveno se samostatnou native regresí.
- Native základ 121/121 PASS, nové/cutting testy 17/17 PASS, SDK 14/14 PASS. WASM check/Clippy/build, native Clippy a typecheck PASS. Finální cutting WebGPU/WebGL 2/2 PASS a scene-sdk 2/2 PASS (skutečný mouse/keyboard input, různé plody, guide pixels, volume/caps/landing, camera/resize/home). Prohlédnuty screenshoty ruky, meče, vodítka a více rozřezaných jablek.
- Report cutting: output/verification/cutting-browser-report.json; SDK: output/verification/browser-report.json. Širší cache receipt se při úpravě dokumentace neuložil; runtime scénáře samy PASS, kompletní push gate se bez požadavku na commit/push nespouštěl.
- Lokální no-store server běží na 127.0.0.1:8000; HTTP 200 ověřeno. Otevření in-app odkazu předáno aplikaci, kvůli staré chybové kartě neprohlašovat aktuální in-app render za ověřený; akceptace běžela v reálném Chrome na obou backendech.

## 2026-10-16: F3, povrchová stopa a hladká slupka

- F3 přepíná stávající Rust HUD, bez JS animační smyčky.
- Apple mesh explicitně volí smooth normály. Rust drží normály rohů odděleně od topologie, interpoluje je při řezech; nové capy zůstávají ploché. Normalizace původních vrcholů jednou, po řezu až ve fragmentu, aby nedocházelo ke změně odlesku.
- Povrchový pás je shaderový průnik konečné roviny meče s viditelnými řezatelnými meshi, doplňuje pomlčky na prkénku. Sdílí tilt/pose z Rust aimCutter, nevstupuje do stínů/odrazu, respektuje hloubku a checkbox.
- Přidány nativní regrese normál/interpolace/transformací a guide lifecycle; browser test měří skutečné F3 glyphy a pixely slupky přebarvené vodítkem (rovný i nakloněný řez).
- Nezávislé review potvrdilo riziko normalizace nových cut rohů před GPU interpolací; opraveno a regrese kontroluje i skutečně emitované renderové normály.
- Ověřeno: native 127/127, SDK 14/14, fmt/native Clippy, WASM check/Clippy/build a typecheck PASS. Cutting + scene-sdk + render-effects na WebGPU i WebGL 6/6 PASS, bez console/HTTP chyb. Prohlédnuty oba backendy, F3, hladká slupka, povrchová stopa a vícenásobné řezy s rovnými capy. Report: output/verification/browser-report.json. Dokumentační zápis po testech nemění runtime důkazy; nejde o plný push gate. Bez commitu/pushe.

## 2026-10-16: draw calls ve F3

- Společný Rust HUD nyní uvádí DRAWS celkem / MAIN hlavní pohled. Počítají se přímo draw/draw_indexed (včetně glTF primitiv, průhlednosti, pomůcek); celkem navíc skutečně vykreslené stíny/odraz, postprocessing, HUD a prezentovací blit. Cachované průchody nepřidávají nic. SDK debugStats poskytuje drawCalls/mainDrawCalls.
- Ověřeno: native HUD 2/2, fmt, WASM check/Clippy/build a typecheck PASS; cutting + culling na WebGPU/WebGL PASS (output/verification/draw-calls-initial-report.json). V tomto prvním reportu selhal pouze nový effects assertion, protože test po vypnutí pomůcek ještě četl předchozí frame; opraveno čekáním. Dotčený render-effects + gltf-full poté 4/4 PASS na obou backendech (output/verification/browser-report.json). Regrese kontrolují instancing, přesné +1 za HUD/blit/posteffect, cache a tři OPAQUE/MASK/BLEND primitiva. Nový řádek vizuálně prohlédnut na screenshotu cutting-hud-BrowserWebGpu.png. Bez commitu/pushe.

## 2026-10-16: skutečný compute, parita a měřená rychlost

- Nový Rust/wgpu + WGSL compute počítá vzdálenosti a BVH-kandidátní průsečíky, dávkuje jeden submit/readback, cachuje neměnné pozice. Uzávěry/komponenty/validace/BVH zůstávají společné CPU. Async příkaz drží jen vlastněné snapshoty přes await; sleduje čepel/cuttable data, ne nesouvisející animace. Clear/dispose/move/změna blade zneplatní staré výsledky, souběžné async řezy se odmítají.
- Porovnání skutečných GPU a CPU dat odhalilo citlivost geometricky dopočtených cap normál na drobné zaokrouhlení; capy nyní nesou explicitní normálu řezné roviny. Vrcholy blízko roviny a incidentní hrany se opravují CPU aritmetikou, nikoli náhradou celého meshe. Parita kontroluje indexy, pozice, normály, barvy a objemy včetně pólů, opakovaných řezů a zrcadleného/neuniformního měřítka.
- CPU urychlení bez změny výsledků: ploché hash mapy místo stromů pro neřazené účetnictví hran, žádná alokace Vec na každou hranu, husté remap/candidate tabulky. Native release (9 vzorků): jablko 19k tri 24,744 → 10,661 ms (~2,3×); 8k tri 10,305 → 4,585 ms. Ignorovaný explicitní benchmark src/cutting_bench.rs, ne runtime scéna.
- Skutečný finální browser dev/WASM: 1088 tri CPU ~40,8 ms vs hybrid ~384,2 ms; 12096 tri CPU ~146,7 ms vs hybrid ~324,7 ms. Měří se i čekání/readback, ne GPU timestamp. Auto proto záměrně CPU (a nepřipravuje zbytečné GPU vstupy); v menu lze explicitně zvolit WebGPU compute + CPU. WebGL nabízí CPU a programové vynucení GPU odmítne. Další zrychlení potřebuje GPU-residentní topologii/výsledky, ne pouze přesun malého kernelu.
- Ověřeno: native 129 PASS + 1 ručně spuštěný/standardně ignored benchmark; SDK 14/14; fmt/native Clippy; WASM check/Clippy/build a typecheck PASS. Finální cutting + cutting-compute + scene-sdk + lifecycle na WebGPU/WebGL 8/8 PASS bez console/HTTP chyb; skutečný compute režim v příkladu, pólová parita, concurrency a cancellation. Prohlédnut screenshot output/verification/cutting-compute-BrowserWebGpu.png. Finální report output/verification/browser-report.json; první timing run zachován jako cutting-compute-first-report.json. První browser startup jednou timeoutoval při startu Chrome; opakované spuštění a finální akceptace prošly. Bez commitu/pushe.
