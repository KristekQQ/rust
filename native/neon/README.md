# Native Neon Arena for Quest

Samostatná Android/OpenXR/Vulkan aplikace `cz.kristek.neon.nativevr`.
Nepoužívá prohlížeč ani JavaScript za běhu. `native/rim` poskytuje sdílené
vlastnictví OpenXR/Vulkan, vstup obou rukou, haptiku a stereo prezentaci;
Rim si ponechává samostatné APK i svou výchozí Android entry feature.

Obsah je zmrazený export revize `cynatar-arena-50` z webové arény:
601 authoring operací, 10 druhů ovoce ×15 sdílených variant, Cynatar font,
obě katany, galerie s tabulkou hodnot, skříň skóre, TOP5 a prostorový info panel.
Offline JS v export-source se do APK nebalí; assets obsahují jen JSON/glTF/font,
buffery, PNG a licence. Exporter čte pouze zdejší snapshot, ne živý checkout.

Rust řídí minutové kolo: 24 událostí, 140 cílů, nejvýše10 živých originálů.
Používá původní XrPlayer, kontakt/řezy, ComboScore, ComboDisplay, nativní numerické
panely a mesh font. Každý originální řez má10bodů; série a oba zásobníky zachovávají
webová pravidla. Čas i projektily se pozastaví při výpadku hlavy/fokusu.
Restart čistí živé cíle, kusy a efekty, ale zachová uložené skóre a cyklus variant.

Ovládání: A vpravo = nové kolo; levý/pravý trigger uvolní příslušný zásobník.
Ray+trigger nebo dotyk mečem aktivuje START/DALSI KOLO. Y vlevo skrývá info panel.
Obě katany řežou bez držení triggeru; platný řez vyžádá krátkou haptiku ovladače.
Info panel je stereo geometrie na floor/reference anchoru [0.9,0.35,-1]m,
0.66×0.40m; neotáčí se s pohledem. App FPS a aktivní displayHz se měří zvlášť.
Host žádá120Hz, s90Hz fallbackem; není to záruka120FPS.

Původní nativní vzhled rev48: skleněné ovoce opacity.06/variation1, velikost.126,
geometrický neon strength.4/radius.045, matná podlaha, SSR/bloom vypnuté.
Galerie dostane stejnou bootstrap materiálovou úpravu jako webový main.js.
TOP5 se atomicky ukládá do privátního `neon-top5.json` v headsetu. Není sdílené
se scoreboard serverem webové arény. Desktopový editor/preset UI a jeho
srovnávací benchmark zatím nemají samostatné nativní menu; minutový program
je zabalený jako ověřené recepty. Obecné fruit řezy zatím běží přes stejný
synchronní TrackedController jako web; nový GLB Rim snapshot worker je jiná cesta.

Build: `native/neon/build-apk.sh` (release arm64 Rust, lokální debug podpis).
Při aktualizaci existující instalace nastav `NEON_SIGNING_KEY` na její původní
keystore; nový výchozí klíč v izolovaném checkoutu nebude pro aktualizaci přijat.
Název profilu ani APK nepotvrzuje shodu certifikátu — ověř ji před instalací.
Výstup: `output/native-neon-apk/kristek-neon-debug.apk`.
Testy: `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo test --manifest-path native/neon/Cargo.toml --locked --target-dir output/native-neon-target --lib`.
Native replay: sestav binární `neon-preview` do stejného target-dir a nastav
DYLD_LIBRARY_PATH na MoltenVK loader; spusť `--output DIR` nebo `--look-at-info`.
Replay dokládá Vulkan/stereo/transformace, ne fyzické OpenXR FPS nebo komfort.

Lokální build není commit/push ani změna sdíleného webového serveru.

Rev49: náhrobek a kredit CODEX odstraněny; VÝKON zůstává samostatně u nohou.

## Cyber katany (rev50)

Vlastní procedurální design inspirovaný termálními cyberpunkovými čepelemi:
[CDPR Thermal Katana](https://eu.gear.cdprojektred.com/en-as/products/cyberpunk-2077-thermal-errata-katana),
[výrobce oficiální repliky Zing](https://zing.store/products/thermal-katana-cyberpunk-2077-replica).
Převzatý není žádný model ani textura. Jde o vizuální motiv napájeného ostří,
nikoli simulaci tepla nebo novou mechaniku zranění. Pravá má oranžové ostří,
levá tyrkysové; tmavý plášť, kovový výbrus, emitér, žebra a stavové články.
Každý meč: jeden neprůhledný mesh, 364 trojúhelníků, žádná textura.
Řezná osa −Z, edge −Y a endpointy −.1 až −.95 m zůstávají zachovány.
`neon-preview --look-at-swords` vykreslí oba skutečně vybavené meshe
v kontrolní póze; tato póza není nový grip ani animace při hraní.

## Spuštění bez počítače

Nainstalovaná rev50 aplikace (Android versionCode2/versionName0.2.0) se jmenuje
**Apollo Games · Neon Arena**, package `cz.kristek.neon.nativevr`. Má vlastní
Android launcher ikonu a banner s oficiálním logem z apollogames.cz.
V Questu ji hledej v Knihovna → Neznámé zdroje. APK i veškerý herní obsah
jsou uložené v headsetu, nikoli streamované z Macu. Restart APK neodinstaluje;
launch after actual reboot has not yet been physically verified. Připnutí řeší
systémový launcher Questu; dostupnost volby záleží na jeho verzi.
Ruční render launcher assetů na Macu: `bash native/neon/android/artwork/render.sh`.
Zdroj loga je zaznamenaný v `android/artwork/SOURCE.md`; herní meshe/řezy se
launcher aktualizací nemění.

## Přecentrování a výstavní režim

Výchozí `referenceMode: "recenterable"` používá podporovaný OpenXR LOCAL_FLOOR
a přebírá systémové přecentrování (podržení Meta). `"fixed-stage"` zvolí STAGE
pro výstavu ukotvenou v místnosti, která běžné přecentrování nenásleduje.
Nezamyká změny kalibrace místnosti provedené samotným runtime.
Volba je v privátním souboru aplikace `files/neon-settings.json`, například
`{"referenceMode":"fixed-stage"}`. Načítá se při spuštění aplikace; chybějící
nebo neplatná konfigurace ponechá přecentrování zapnuté. Skóre i restart kola
jsou nezávislé. Bez podpory LOCAL_FLOOR host výslovně hlásí STAGE fallback.

## Optional frame profiling

Build the native crate with `--features frame-profiling` to enable the existing
nonblocking GPU timestamp sampler and CPU scopes for OpenXR frame wait, scene
update, swapchain wait and each eye's encoding/submission. `Native profile` logs
include render dimensions, separate end-frame wall time and profiler JSON.
Unsupported GPU timestamps remain unavailable; CPU scopes still work. Windows
cover rendered frames, and GPU pass sums exclude runtime/compositor work. The
default build has profiling disabled; compare final FPS with profiling off too.

## Původní webový vzhled — 2026-11-07

Izolovaná aktualizace versionCode4 / versionName0.4.0 vrací původních 80
zkosených lesklých dlaždic se samostatnými spárami, reflectivity .94,
SSR strength .92 / trace scale .5 a původní receiver bounds. Zachovává
rev50 katany/ovoce/gameplay i recenterable/fixed-stage nastavení. Půlky ovoce
po řezu fyzicky padají a rotují bez okamžitého .75s dissolve; běžný feedback
pool je odstraní po1.5s. Engine shader ani ostatní hry se nemění.

`neon-preview --look-at-floor --output DIR` ověří stereo podlahu/SSR;
`--ssr-off` slouží k párovému porovnání stejné scény. Report obsahuje
`ssrEnabled` a `ssrWork`. Offscreen Vulkan replay není důkaz Quest FPS.

## Kompaktní zásahový burst — 0.5.0

Prstenec a jiskry nyní trvají .28s; popisek skóre si ponechává .65s.
Textura má tenký okraj s omezenou modrou mlhou, šest drobných tyrkysových
jisker místo velkých růžových kosočtverců. Velikost zásahu roste s původní
naměřenou rychlostí, ale vizuální rozměr silných zásahů se měkce omezuje.
Rovina skutečných řezných ploch, alpha blending, depth test, pool, skóre
a pohyb/životnost půlek se nemění.

`neon-preview --look-at-cut --cut-speed 1|2|3 --cut-age .08 --output DIR`
reprodukuje skutečný zásah přes Game/XrPlayer na autorovaném melounu.
`--cut-age .35` zachytí konec burstu; jde o offscreen Vulkan QA, ne Quest FPS.

## Elektrický zásah — 0.6.0

Sdílený Rust generuje jedinou RGBA texturu pro nativní i webovou verzi:
sedm přerušených klikatých oblouků s bílým jádrem, tyrkysovým halem
a krátkými rozvětvenými výboji. Střed zůstává průhledný; žádný plný disk
ani další hladké kruhy. Textura vzniká jednou při vytvoření rendereru,
nikoli během zásahu. Trvání .28s, velikost podle rychlosti, skutečná řezná
rovina, pool, skóre a fragmenty zůstávají stejné jako v 0.5.0.

## Rychlejší elektrický zásah — 0.7.0

Prstenec a jiskry trvají .18s místo .28s (o 36 % kratší). Stejný
normalizovaný průběh rozpínání i útlumu proběhne rychleji v nativní i
webové verzi. Textura a velikost zůstávají stejné, skóre si ponechává .65s
a pohyb/životnost půlek se nemění. Předchozí popisy .28s patří verzím 0.5/0.6.

## Rozvětvený elektrický zásah — 0.9.0

Životnost se vrací na .28s. Čtyři samostatné klikaté výboje naznačují kruh:
proměnlivá šířka bílých jader, vrstvené halo, delší rozvětvené blesky a několik
oddělených jisker. Převládá modrá/tyrkysová, fialová doplňuje část výbojů,
jantarová zvýrazňuje jedinou malou větev. Průhledný střed a větší mezery
zachovávají čitelnost zásahu. Sdílená RGBA textura vzniká jednou při vytvoření
rendereru; shader RGB nepřebarvuje. Velikost, rovina řezu, reakce na rychlost,
skóre a fyzika zůstávají stejné.

## Oprava přenosu materiálu zásahu — 0.10.0

Simulační worker nyní publikuje také dvě ID prstenců. Renderovací zrcadlo
podle nich použije texturovaný FX materiál; dříve přeneslo pouze mesh a
vykreslilo plný tyrkysový prstenec. Gameplay stav, fyzika a animace zůstávají
na workeru. Vyčištění pool zaparkuje, opětovné použití zachová materiál,
odstranění meshe odstraní i jeho značku z dalšího snímku. Spawn jiskry
se nemění. Textura a trvání .28s zůstávají stejné jako v 0.9.0.

`neon-preview --look-at-cut` nyní používá skutečný Simulation → publication
→ App render mirror. Předchozí cut fixture provozovala Game přímo v renderovací
scéně a tuto chybu nemohla zachytit. Experimentální binární transport NWR1/v2
přenáší stejné značky; obě strany musí být sestavené společně, v1 se odmítá.

## Větší bleskový prstenec — 0.11.0

Lineární velikost samotného bleskového prstence je 1.5× větší v nativní i
webové verzi. Čas .28s, barvy, řezná rovina a křivka reakce na rychlost
zůstávají stejné. Popisek, jiskry, ovoce a fyzika nemění velikost ani polohu.

### Native half-resolution edge halos

Native Neon enables `configure_neon_glow_resolution(0.5)`. Only ordinary mesh
halos render at half width/height; real surfaces, cut faces, numeric glyph atlas
and panels remain full resolution. Vulkan postprocess views with sampled
Depth32Float and even dimensions use conservative depth reduction and a
depth-aware MAX composite; unsupported views retain the full-resolution path.
This is a visual approximation: halo energy can be lower at occluding silhouettes.
Each eye keeps its own cached resources and reuses source bind groups. The shared
renderer now defaults to .5 for eligible Vulkan/WebGPU views, including direct
rendering without postprocessing. Empty ordinary halo layers are skipped. WebGL
retains full-resolution ordinary halos. The native log records `Neon halo view`
with the effective scale for the first render of each view.

Compare with `scripts/quest-hardware-probe.py --experiment half-glow` plus its
required device/build arguments. `--view cut --cut-age 0.4` checks separated cut
fragments. Use the whole-pass sum, including `neon.half.depth` and
`neon.half.edges`. Physical Quest 3 evidence is in
`output/measurement/half-glow-20261109/`: 3 paired 600-frame runs reduced frozen
stereo GPU time by 1.6–14.9% (median paired 13.7%); this is not live OpenXR FPS.
