# Řím — nativní Quest / OpenXR prototyp

Tento host používá stejný `webgpu_wasm` Rust engine, jeho `SceneRenderer`, WGSL,
scénu, glTF animace, `XrPlayer`, fyziku a `rim_game`. Obsah odpovídá
`examples/rim/scene.js`: původní město, 14 nepřátel, dva gladii, kolize,
projektily, životy, stíny a FXAA. Je to první nativní prototyp, nikoli ověřená
finální Quest hra.

## Oddělení platforem

- Výchozí webový build zachovává WebGPU/WebGL a browserové adaptéry.
- `native-render` je explicitní Cargo feature; zpřístupňuje společný renderer
  a Vulkan. `cfg` vybírá hostové služby při kompilaci. Do webové frame smyčky
  nepřibyla runtime kontrola Androidu, dynamický renderer ani JNI volání.
- `app.rs` skládá původní Rust renderovací průchody a předává vstup původní hře.
  `content.rs` je nativní recept scény; herní pravidla zůstávají společná.
- `xr.rs` vlastní OpenXR session, action set, stereo swapchain a úzké unsafe
  Vulkan/wgpu propojení. Runtime vlastní obrazy; engine je nesmí zničit.
- `android.rs` obsluhuje NativeActivity, APK assets a neblokující lifecycle.
  Webový SDK, příklady a existující HTTPS server se nemění.

## Lokální nativní kontrola

```sh
cargo test --manifest-path native/rim/Cargo.toml --lib
native/rim/replay.sh
```

Na Macu replay používá Vulkan přes MoltenVK. Nastav `VULKAN_LIB_DIR` na adresář
s `libvulkan.dylib`; zde je lokálně připravený MoltenVK 1.4.2 v
`output/native-tools/`. Na Linuxu je potřeba funkční Vulkan loader/driver.
Výstup `output/native-rim/{left.png,right.png,report.json}` obsahuje skutečně
vykreslené pixely a test ztráty/obnovy ovladačů. Pózy jsou zadané testem:
**nejde o emulaci Quest compositoru ani ověření OpenXR runtime**.
`rim-preview --xr` může použít skutečný desktopový OpenXR runtime, pokud existuje;
na tomto Macu nebyl ověřen.

## Sestavení APK

Předpoklady: Rust dle `rust-toolchain.toml` + `aarch64-linux-android`, JDK,
Android SDK s NDK `27.2.12479018`, build-tools `35.0.0`, platformou `android-35`.
Skript očekává macOS NDK cestu; pro jiný host je nutná úprava linkeru.

```sh
native/rim/build-apk.sh
```

Výstup `output/native-rim-apk/kristek-rim-debug.apk` je arm64 APK s release Rust
kódem a lokálním debug podpisem (manifest je debuggable pro logcat). Loader
Khronos OpenXR 1.1.40 se stáhne z Maven Central a kontroluje pevným SHA-256.
Klíč zůstává v ignorovaném výstupu. Skript ověřuje podpis a ZIP alignment.
Nejde o připravené vydání pro Meta Store.

## Quest

Zapni vývojářský režim, připoj Quest přes USB a potvrď USB debugging v headsetu.
Potom z kořene tohoto worktree:

```sh
SDK="${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}"
"$SDK/platform-tools/adb" devices -l
"$SDK/platform-tools/adb" install -r output/native-rim-apk/kristek-rim-debug.apk
"$SDK/platform-tools/adb" shell am start -n cz.kristek.rim.nativevr/android.app.NativeActivity
"$SDK/platform-tools/adb" logcat -s KristekRim OpenXR-Loader
```

Levá páčka: pohyb; pravá páčka: otočení po 30°. Pravá spoušť: míření/
zpomalení a teleportační paprsek dle původní hry; tlačítko A pravého ovladače: dash (stejně jako web).
Tlačítko **Y na levém ovladači** skrývá/zobrazuje prostorový panel výkonu.
Meče sledují grip obou ovladačů bez držení spouště. Po ztrátě sledování
se vybavení skryje. Hra začíná automaticky.

První fyzická kontrola musí ověřit launch/session, podlahu a měřítko, obě oči,
pohyb hlavy, oba ovladače, ztrátu/obnovu trackingu, boj/kolize, pause/resume
přes systémové menu a ukončení. Odezva, komfort a FPS zůstávají do této kontroly
neověřené. Host odesílá oba obrazy a potřebné Vulkan přechody na frontu sdílenou
s OpenXR; před každým vrácením obrazu již blokujícím způsobem nečeká na GPU.
Pořadí submit/release/end-frame a blokující dokončení při ukončení jsou zachované. Řezání používá původní dávkované
Rust úlohy synchronně v hostu. Výkon vůči webu ani jiné nativní hře se netvrdí.
Foveace není zapnutá.

Zdroje platformních kontraktů:
[Khronos hello_xr](https://github.com/KhronosGroup/OpenXR-SDK-Source/tree/main/src/tests/hello_xr),
[openxrs](https://github.com/Ralith/openxrs),
[wgpu 30.0.1 HAL](https://github.com/gfx-rs/wgpu/tree/v30.0.1/wgpu-hal),
[Meta manifest](https://developers.meta.com/vr/documentation/native/android/mobile-native-manifest/).
Licence původních assetů jsou v `assets/models/rim/SOURCE.md` a `LICENSE-three.txt`;
APK obsahuje jejich kopie a licenci OpenXR loaderu.

## 120 Hz a prostorové údaje

Host zjistí podporované frekvence přes `XR_FB_display_refresh_rate` a při vstupu
relace požádá přednostně o 120 Hz; pokud není podporovaných nebo požadavek selže,
použije dostupných 90 Hz. Aktivní frekvenci čte zpět a
aktualizuje i při události změny frekvence. Nepodporovaný požadavek neskrývá.
120 Hz displeje není důkazem 120 vykreslených FPS.

Panel používá původní engine `InfoPanel` a `XrPlayer` vazbu `anchor:feet` jako
webový panel: offset `[0.9,0.35,-1]` m, velikost 0.66 × 0.40 m. Je vpravo dole
od hráče; pohyb/snap turn sleduje, otočení a sklánění hlavy jeho umístění nemění.
Ztráta trackingu ho skryje. Tlačítkem Y ho lze vypnout včetně diagnostických
počítadel. Text se obnovuje maximálně čtyřikrát za sekundu.

`FPS APP`, `FRAME MS` a `FRAME P95` vycházejí ze skutečných wall-clock intervalů
mezi dokončenými stereo `xrEndFrame`, nikoli z požadovaných Hz nebo
`predictedDisplayTime`. `DISPLAY HZ` uvádí aktivní údaj runtime. `APP+WAIT MS`
je práce celého host frame po `xrWaitFrame`, včetně případného čekání v runtime voláních;
`UPDATE MS` měří aktualizaci scény/herní logiky v hostu. Nejde o čisté GPU časy.
`GPU MS` je proto N/A. Draw calls se nulují jednou před každým stereo frame.

Kontrola panelu na Macu: spusť sestavený `rim-preview --look-at-info --output DIR`
s nastaveným Vulkan loaderem. Tento režim používá 1792×1792 px na oko, dívá se
na panel a skryje meče, aby nezakrývaly text. Hodnoty FPS/HZ zůstávají bez
skutečné OpenXR relace nenaměřené; režim ověřuje prezentaci a stereo.

## Obnova arény při ztrátě HP

Native host při HP <= 0 vytvoří čerstvý encounter na původních scene IDs:
100 HP, první vlna, čisté projektily/řezy/debris, původní bezpečný spawn.
Modely GLB ani GPU assety se znovu nenačítají. Zohlední se aktuální room-scale
XZ posun, zachová se referenční yaw a vyčistí historie mečů, aby po respawnu
nevznikl falešný švih. Nový proces aplikace začíná čerstvě už původně; návrat
z nabídky Questu nezpůsobuje dodatečný reset zdravého hráče.

## Další měření výkonu při 120 Hz

Runtime na tomto Questu nabídl [72,80,90,120] Hz. Přepnutí je technicky možné,
ale 120 FPS potřebuje rozpočet 8,33 ms oproti 11,11 ms při 90 Hz. Host nyní žádá
120 Hz a kvalita scény se nesnižuje. Dosažené app FPS se měří odděleně.

1. Znovu použít engine `render::profiler` pro vzorkované nativní Vulkan GPU
   timestamps a oddělit CPU update/encode, GPU průchody, XR pacing a případné
   GPU čekání. Časy vnořených CPU scopes nesčítat. Změřit i kontrolu bez profileru.
2. Zachovat stejné rozlišení, efekty, scénu a srovnatelnou stopu kamery/boje.
   Současné volné hraní nepotvrzuje poměr výkonu vůči WebGL2.
3. Jediné vlákno `rim-cut` nyní dostává `CutWork`: sdílený immutable asset,
   snímek zásahové pózy a indexové rozsahy. Deformace/transformace vrcholů,
   rozříznutí, dekódování výsledků, CutMesh, bounds/statistiky a příprava
   vykreslovacích dat běží v `CutWork::execute` mimo XR vlákno. Nejvýše dvě úlohy
   jsou rozpracované. Na XR vlákně zůstává rezervace rozsahů, registrace hotových
   meshů (nejvýše 9000 vrcholů za frame), GPU buffer upload a atomické zveřejnění
   po upload ack. Renderer znovu nerozbaluje workerem připravené render meshe.
   Generace transportu a owner/serial chrání novou arénu před starými výsledky.
   Teardown join probíhá pouze při ukončení. Žádný kosmetický recoil se nepřidává.
4. Podle měření řešit dominantní PBR/stíny/postprocessing nebo opakovanou
   přípravu a uploady. Zachovat obě oči, pořadí uniform write/submit, životnosti
   importovaných obrazů a nezměněnou browser cestu.

Přijetí 120 Hz: skutečné 120Hz hlášení runtime, dlouhé úseky aktivního boje
s odpovídající app/GPU kadencí, kontrola p95/p99 a propadlých snímků, vizuální
kontrola obou očí a profiler-off kontrola. Pouhé číslo DISPLAY HZ nestačí.

## Postupné vlny, léčení a počítadlo

Sdílená Rust logika začíná jedním dronem, pokračuje dvěma drony, jedním tankem
a následně střídá smíšené vlny včetně přízraků. Pozdější vlny postupně rostou,
ale mají nejvýše 10 aktivních protivníků. Prostorový HUD ukazuje `VLNA X HOTOVO Y`;
dokončená vlna se započítá po vyřazení všech protivníků včetně doběhnutí animací.

Třetí zabití zaručeně vytvoří zelený léčivý drop, další zabití jej dávají přibližně
ve 30 % případů podle deterministického hashe. Přiblížení do 0,85 m léčí o 25 HP
nejvýše do 100; při plném zdraví drop zůstává. Nejvýše čtyři dropy mají životnost
30 sekund aktivní hry, kontrolu podlahy a přímé dostupnosti. Přechod vlny je zachová,
reset po smrti je odstraní. Web i nativní host používají stejná pravidla.

## Hranice nativního cut workeru

`Game::take_cut_work` přidělí token a omezené indexové rozsahy bez deformace
geometrie. `CutWork::execute` je samostatná CPU úloha nad vlastněnými daty;
`Game::accept_prepared_cut` pouze přijme hotové meshe do existující transakce.
Assety se sdílejí přes Arc, renderer/scéna/Game se workeru nepředávají.
Původní browser byte transport (`take_cut_job` / `accept_cut`) zůstává funkční;
web zatím nadále offloaduje samotný split. Binární a snapshot cesta mají test
bitově shodných výsledných pozic/normál/barev a stejné GPU-ack pravidlo.

Póza modelu v okamžiku zásahu zůstává zachycená pro geometrickou přesnost.
Samotné přesunutí výpočtu negarantuje nulovou vizuální prodlevu do zveřejnění
obou hotových polovin ani vyřešení všech studených GPU pipeline špiček.
Diagnostika `Native phases` odděluje rezervaci úlohy, tick, scene advance a render;
GPU vykreslení není totéž jako tyto CPU časy. Spouští se na úlohách a CPU snímcích
nad 20 ms; `jobs` je počet odeslaných snapshot úloh, nikoli počet bajtů.

## GPU vykreslení rozpouštěných pevných kusů

Záporně zabalený fade znamená prostorové vykrajování (dissolve), nikoli alfa
průhlednost. Přeživší neprůhledné plochy proto zůstávají v hloubkovém průchodu;
nevykreslují se znovu dvěma alfa draw calls. Částečně rozpouštěné importované
meshe používají oboustranný pipeline a stejnou orientaci vnitřních normál ke kameře
jako předchozí překryv. Skutečné sklo a kladný alfa fade si zachovávají alfa cestu.
Dissolve pole, geometrie, rozlišení a nastavení efektů se nemění. Nové hloubkové
zápisy mění překrývání kusů a jejich správné zapojení do depth/postprocess/SSR;
nejde o bitově totožný obraz původního překryvu bez zápisu hloubky.
