# Ověření prvního nativního Říma — 2026-11-03

Základ: předaný `origin/main` commit `84a44ec3205ac5f9f6ea2edf08c7b7cd1ea38eb8`.
Lokální změny: `codex/native-rim-20261103`, samostatný worktree. Bez commitu,
pushe, nové CI nebo změny existujícího Quest preview serveru.

## Skutečné výsledky

- `cargo test --features native-render --lib`: 441 PASS, 3 ignored. Nově se tím
  kompiluje i sdílený nativní renderer. Při zavádění feature byl opraven dvojitý
  import renderovacích layoutů pouze v testovacím režimu.
- `cargo test --manifest-path native/rim/Cargo.toml --lib`: 2 PASS: asymetrická
  OpenXR projekce/hloubka a původní assety/hra/tracking bez browseru. 19 modelových
  kořenů sdílí 7 originálních GLB assetů.
- Skutečný nativní offscreen replay na Apple M1, Vulkan přes MoltenVK 1.4.2:
  13 014 různých barev, 64 648 rozdílných stereo pixelů, obnova ovladačů po
  výpadku PASS. Obrázky a JSON: `output/native-rim/`. Je to dodaná stopa póz,
  nikoli OpenXR runtime nebo Quest emulátor.
- Release WASM sestaven; 9 browser scénářů PASS na Chrome 156.0.8078.80:
  `rim`, `gltf-publication`, `render-target`, `postprocessing` na obou backendech,
  `webxr-webgpu` na WebGPU. Report: `output/verification/browser-report.json`.
  Finální čistě testovací úprava `cfg` následně znovu sestavila přesně stejné
  WASM bajty (`a5fffa2760fcebd1be02c512afd7110b5902aab58d715e0f2009ac6d6ad36705`).
- Android arm64 release knihovna sestavena, obsahuje `ANativeActivity_onCreate`.
  Podepsané APK obsahuje engine, Khronos loader a původní assety; kontrola
  `apksigner verify` a `zipalign -c -P 16 4` PASS. Přesný finální hash je v
  `output/native-rim-apk/DELIVERY.json`.
- Nezávislá kontrola kódu opravila explicitní registraci importovaných GLB na
  GPU, pořadí skutečného/simulačního času, blokující čtení Android událostí,
  dokončení GPU práce při chybě a přístup k aktivitě po Destroy. Nativní vstup
  zachovává skutečný čas pro rychlost meče, pohyb integruje s omezeným krokem.

## Porovnání webu

Použit stávající `scripts/render-performance.mjs`, zmrazený původní release a
kandidát. Stejný Chrome 156.0.8078.80, `rim-static`, 960×640 DPR 1, bez profileru,
`measurement=minimal`. Kalibrace pevné původní scény 1 s / 2,5 s; potom na každém
backendu 3 páry AB/BA/AB, 8 s warmup + 25 s měření na běh. Bez souběžných buildů
nebo GPU testů v měřicích oknech.

| Backend | CPU mediány baseline / kandidát po párech (ms) | Medián párových CPU poměrů | RAF poměr | Pixely |
|---|---|---|---|---|
| WebGPU | 0,80 / 0,80; 0,70 / 0,80; 0,80 / 0,80 | 1,00 | 1,00 | všechny 3 přesně stejné |
| WebGL | 1,50 / 1,40; 1,40 / 1,10; 1,60 / 1,60 | 0,93 | 1,00 | všechny 3 přesně stejné |

Obě regresní kontroly prošly s výchozí hranicí 5 %. Jeden nepříznivý WebGPU pár
(+14,3 % při rozdílu 0,1 ms) zůstává ve výsledcích. Nízké časy jsou kvantované;
příznivější WebGL čísla nejsou důkazem univerzálního zrychlení. Kadence je
omezena přibližně 60 Hz. Pevná statická scéna neposkytuje přetěžovací osu;
netvrdíme z ní propustnost, výkon aktivního boje, WebXR nebo Quest FPS.

Raw JSON, zachované jednotlivé běhy, screenshoty, kalibrace a přesné příkazy:
`output/perf/native-rim-web-regression/`. Původní snapshot je v integračním
worktree `output/perf/native-rim-baseline-01a1a139`, kandidát v tomto worktree
`output/perf/native-rim-candidate`. Liší se skutečný WASM build; nejde o dvě
měření stejného balíčku.

## Zbývající fyzická kontrola

`adb devices -l` při práci nehlásilo žádné zařízení. APK nebylo instalováno ani
spuštěno na Questu. OpenXR lifecycle, tracking, input bindingy, podlaha/měřítko,
komfort a FPS jsou proto neověřené. Native host konzervativně čeká na GPU před
vrácením swapchain obrazu a zpracovává dávky řezání synchronně; jejich dopad musí
změřit Quest. Foveace je vypnutá. Postup instalace a přejímky je v README.

## Doplnění: první Quest launch a 90Hz/panel aktualizace

Původní APK `c9345ea4…` bylo následně přes ADB nainstalováno a spuštěno na
Questu 3 / Androidu 14. Oculus runtime 209.166.0 přešel do FOCUSED a Honza
potvrdil „Město a meče“. Po tomto kroku bylo ověřeno i Wi-Fi ADB. Jednotlivé
VrApi vzorky běžící původní aplikace hlásily přibližně 72 FPS / 72 Hz; nebyl
to kontrolovaný benchmark. Neznamená to přijetí všech herních funkcí.

Nová hostová aktualizace přidává podporovaný požadavek na 90 Hz a původní
prostorový panel, oddělené měření app FPS / display Hz a toggle Y. Čtyři hostové
testy PASS: projekce, původní assety, skutečná 45 FPS kadence odlišená od 90Hz
údaje a ukotvení/hide/show/tracking. Obě oči byly nativně vykresleny v režimu
pohledu na panel; důkazy `output/native-rim-90-panel-hi/`. Android build a
podpis/alignment PASS. Aktuální stav instalace a přesný hash jsou v
`output/native-rim-90/DELIVERY.json`; 90 Hz a čitelnost na headsetu se nesmějí
považovat za potvrzené před novým nasazením. Webový kód ani balíček tato
aktualizace nemění.


### Fyzická aktualizace a hranice 90 FPS

Verze `6e77185f…` byla přes Wi-Fi nainstalovaná a spuštěná na Questu 3. Runtime
potvrdil přechod na 90 Hz a uživatel potvrdil panel. Při hraní ovšem FPS kolísalo:
posledních přibližně 60 s před dalším zásahem mělo 59 jednosekundových vzorků,
minimum 47,9, medián 85,3, maximum 90,8 FPS; 20 vzorků bylo pod 80 FPS.
Jde o vzorky při volném hraní, nikoli úplné rozdělení snímků nebo kontrolované
srovnání s WebGL2. Samotný úsek okolo 90 FPS tedy nestačí pro přijetí výkonu.
Raw log a souhrn: `output/quest-native-90/baseline-full-play*`.

Následující hostová oprava nahrazuje blokující GPU čekání před každým
`xrReleaseSwapchainImage` neblokujícím poll. Oba oči a konečný COLOR_TARGET
přechod jsou nadále submitnuté na téže Vulkan frontě před release/end-frame;
GPU idle při teardownu a guard při chybě zůstávají. Nezávislá kontrola nenašla
překážku při těchto podmínkách. Toto odpovídá
[Khronos upřesnění](https://github.com/KhronosGroup/OpenXR-Docs/issues/50) a
[Vulkan binding kontraktu](https://raw.githubusercontent.com/KhronosGroup/OpenXR-Docs/main/specification/sources/chapters/extensions/khr/khr_vulkan_enable.adoc).
Rozlišení, scéna a efekty se nemění. APK `2f3b9f20…` se přes ADB nainstalovalo,
ale Wi-Fi se při startu odpojila; dopad na FPS a vizuální stabilitu zatím nemá
potvrzený post-run výsledek. Starší blokující APK je uchované v
`output/quest-native-90/baseline-blocking.apk` pro případné srovnání.


### Potvrzený běh opravy a HP reset připravený k předání

Po obnovení Wi-Fi byla verze `2f3b9f20…` spuštěná na Questu. Uživatel potvrdil
„je to teď lepší“. Posledních přibližně 60 s zachyceného běhu má 59 app vzorků:
87,6–90,7 FPS, medián 90,0; žádný pod 80 FPS. Odpovídající VrApi vzorky 89–91 FPS
při 90 Hz. Úseky mají odlišný pohled/herní stav, takže zlepšení není kontrolovaný
A/B výsledek ani prokázaná převaha nad WebGL2. Raw a souhrn jsou v
`output/quest-native-90/async-play-live*`. Archiv nasazeného APK:
`output/native-rim-90/kristek-rim-90hz-async-debug.apk`.

Na upřesněný požadavek uživatele se po HP <= 0 připravuje úplně nový encounter
na stejných modelech a GPU identitách. Čistí projektily/řezy/debris, obnoví
100 HP a první vlnu, vrátí hráče na bezpečný spawn se zohledněním fyzického
room-scale XZ a zachová yaw. Zdravá hra se pouhým návratem z menu neresetuje.

Ověření HP aktualizace: 5 host testů PASS včetně skutečné smrti z původních
útoků a obnovení scény; 28 engine Rim testů PASS; nový matching release WASM
`f68201b53f7267dbf717602623cdc86784cd3a6dc38ef5361bd03e0794448484` a 4 browser
kontroly (`rim`, `rim-gameplay` na obou backendech) PASS. Engine přidává pouze
read-only `is_dead()`; webové chování smrti a existující server se nemění.
Nezávislá kontrola resetu v daném rozsahu bez nálezů. Podepsané APK
`596c3f43…` je připravené, ale nebylo nasazené: uživatel nyní zkouší webovou
verzi. 120Hz přepnutí není zapnuté; návrh měření a optimalizací je v README.

### Postupné vlny, HP dropy a dokončené stage (lokální balíček)

Nový sdílený gameplay a HUD jsou popsané v README. Kontroly: 32 engine Rim testů,
5 nativních host testů, 33 SDK testů a typecheck PASS. Zdrojové browser kontroly
`rim` a `rim-gameplay` prošly na WebGPU i WebGL2 (4 PASS), včetně skutečného
zásahu/řezu a přechodu na druhou vlnu s jednou dokončenou. Release/minifikovaný
`output/production` prošel oběma backendy stránky `rim` (2 PASS).
Nativní stereo capture `output/native-rim-progression/{left,right}.png` ověřil
zobrazení první vlny a textu `VLNA 1 HOTOVO 0`. Léčení/drop lifecycle je ověřen
Rust testy; fyzické přijetí nového gameplay a čitelnosti HUD zůstává neověřené.
Nezávislá kontrola této změny v daném rozsahu bez nálezů.

Podepsané arm64 APK `output/native-rim-progression/kristek-rim-progression-debug.apk`
má SHA256 `1b4cf0af011a029a0643ebf175eeb8d47cb0aed9c69d53c059d03019acb68bad`;
release build, podpis a alignment PASS. Nová verze není nasazená na headset ani
na existující sdílený webový server, který obsluhuje jiný worktree.
Požadavek zůstává 90 Hz. Předchozí FPS vzorky neprokazují výkon změněné herní zátěže.
Žádný commit, push ani CI pro tuto změnu neproběhl.

Následné předání na výslovný požadavek uživatele: stejné APK `1b4cf0af…`
bylo úspěšně nainstalováno přes Wi-Fi ADB a spuštěno na Questu. Proces 20812
loguje renderované snímky, aktivních 90 Hz a obnovu arény při HP <= 0.
Krátký startup úsek není dlouhodobý benchmark; viz `output/native-rim-progression/device-startup.txt`.
Přijetí nových vln, HP dropů a HUD uživatelem zatím nepotvrzeno.

### 120 Hz, A dash a paralelní výpočet řezu

Native-only aktualizace preferuje 120 Hz, s podporovaným 90Hz fallbackem;
A pravého ovladače nyní spouští původní 5m dash stejně jako WebXR.
Výpočet `rim_cut::split` byl přesunut z XR vlákna na samostatný `rim-cut` worker
s nejvýše dvěma rozpracovanými úlohami, neblokujícím poll/submit a generační
ochranou při obnově Game. Příprava posed mesh chunků, simulace, publikování a
GPU zůstávají na XR vlákně; browser rovněž offloaduje pouze split.
8 native host testů PASS včetně 5m dash/hold, preference frekvence, skutečné
shody split výsledků, backpressure, generačního resetu, chyb a teardown při
plném kanálu. Nezávislá source kontrola bez nálezů. Android arm64 release,
podpis a alignment PASS. APK `6bab4f3a…` archivováno v `output/native-rim-120/`.
Předání a skutečné frekvence se zaznamenají po obnovení Wi-Fi; 120FPS není
samotným nastavením prokázáno. Browser bundle ani sdílený server se nemění.

Následně APK `6bab4f3a…` přes Wi-Fi nainstalováno a spuštěno; OpenXR
potvrdil změnu na 120 Hz a log potvrzuje start workeru. Posledních 15 app vzorků
z prvního zachyceného běhu: 99,7–119,8 FPS, medián 109,6. Nejde o stabilních
120 FPS ani kontrolovaný A/B test. Celý záznam a souhrn: `output/native-rim-120/device-run*`.

### Snapshot worker: skutečná příprava oddělení mimo XR vlákno

Uživatel upřesnil, že požaduje skutečné offloadování geometrii, nikoli kosmetický
pohyb stojícího zasaženého modelu. Pokus o recoil byl stažen a nebyl nasazen.
Nativní CutWork worker nyní dělá celý CPU geometry řetězec popsaný v README;
scéna, GPU upload a původní atomická publikace zůstávají na vlastníkovi.
PreparedVisualMesh zachovává původní triangle-color/normal konverzi a cacheuje
výsledné render vrcholy, takže renderer neprovádí druhou expanzi na XR vlákně.

33 engine Rim testů PASS včetně skutečného nového vlákna, bitové shody obou
výstupních stran proti webové binary cestě a čekání na GPU ack. 8 host testů
PASS včetně skutečných GLB, reálných slash vstupů, thread-ID kontroly, kapacity,
generace a teardown. 4 browser kontroly (`rim`, `rim-gameplay`, oba backendy)
PASS s matching release WASM; oddělené poloviny vizuálně zkontrolovány.
Nezávislá kontrola vlastnictví/transakcí/limitu/render cache bez nálezů.
Podepsané release arm64 APK `13dde842…` nainstalované přes Wi-Fi stream instalací
bez incremental režimu a spuštěné. Záznam nového běhu a přijetí se doplní;
změna nezaručuje stabilních 120FPS. Archiv `output/native-rim-cut-hitch/`.

Nový fyzický běh zaznamenal odeslané snapshot úlohy i přijaté hotové meshe
a OpenXR120Hz. Rezervace úlohy na XR vlákně: {"samples": 50, "min": 0.0, "median": 0.0, "max": 0.01} ms.
Nejde o kontrolovaný A/B test ani důkaz nulové vizuální prodlevy.
Souhrn `output/native-rim-cut-hitch/snapshot-worker-summary.json`; raw záznam vedle.
Release/minifikovaný production balíček stránky rim prošel WebGPU i WebGL2 (2 PASS).

### Propad FPS při pohledu na rozpouštěné kusy

Uživatel potvrdil plynulejší řezy, ale hlásí cca50FPS při pohledu na fragmenty
oproti120FPS jinde. Zachycený úsek potvrzuje58,5 app FPS při CPU práci2,41ms,
runtime GPU přibližně0,93. Ukazuje na GPU zatížení, sám o sobě neurčuje průchod.
Kód nesprávně klasifikoval i neprůhledný prostorový dissolve jako alpha fade:
opaque průchod jej zahodil a plná geometrie se pak kreslila dvěma alfa průchody.

Oprava odděluje cutout dissolve od skutečné alfa průhlednosti. Zachovává obě
strany otevřených řezů, orientaci vnitřních normál ke kameře, stejné pole/rim a
skleněné materiály. 22 visibility testů PASS. Šest browser případů PASS včetně
opraveného affected recheck: rim, rim-gameplay, overlay-selection na obou
backendech. Reálné kusy v prostředku rozpouštění: alfa draws0 (opaque),34 (glass)
na obou backendech; obnovení opaque a mirrored geometrie PASS. První assertion
kontroly obnovení omylem odečítala per-frame čítače; opravena na přímé per-frame
hodnoty, bez změny rendereru mezi běhy. Vizuální kontrola zachovala vnitřní povrchy;
obraz může změnit správná hloubka, překrývání a zapojení FXAA/SSR.
Nezávislá source kontrola bez nálezů. Podepsané release APK `215d8947…` připravené
v `output/native-rim-dissolve/`; fyzický dopad zatím čeká na wake/install.

Následně `215d8947…` úspěšně nainstalováno a spuštěno přes Wi-Fi; SHA256
base.apk přečtená z headsetu přesně odpovídá archivu. Běh potvrzuje120Hz
a38frame s odeslanou cut úlohou. Po vyloučení prvních10s od první app vzorku:
46vzorků,79,8–120,3FPS, medián114,1. Kamera/herní stav se liší od baseline,
takže nejde o kontrolovaný A/B ani důkaz stabilních120FPS či definitivního
odstranění všech propadů. Raw a summary jsou v output/native-rim-dissolve/device-run*.
