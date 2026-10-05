# Projektová paměť

Technická fakta o tomto repozitáři, ověřená z kódu a místního běhu 2026-10-05.
Nejde o archiv konverzací ani osobních údajů. Při změně projektu ověřit a
aktualizovat dotčené údaje; historická poznámka není důkaz aktuální funkčnosti.

## Co je to za projekt

`webgpu_wasm` je prototyp interaktivní 3D scény v prohlížeči. Rust 2021 se
překládá do WebAssembly (`cdylib`), `wasm-bindgen` propojuje Rust s JavaScriptem,
`wgpu` 30.0.1 poskytuje grafickou vrstvu a `glam` 0.34.0 matematiku.
Rust 1.99.0 je připnutý pro projekt v `rust-toolchain.toml`; wasm-pack byl
aktualizován na 0.15.0 a Puppeteer na 25.12.0. Manifest povoluje
backendy WebGPU i WebGL; skutečnou dostupnost je nutné ověřit v prohlížeči.

Aktuální zdrojový kód obsahuje krychle, roviny a koule, transformace objektů,
mřížku, až čtyři světla, oběh a pulzování světel, plánované rotace a odstraňování.
`SceneManager::load_example` v Rustu definuje pevnou ukázkovou scénu
(rovina, krychle, koule a dvě statická světla). JS ji pouze požádá načíst. Rust inicializuje prázdnou scénu; automatické přidávání,
rotace a mizení ukázkových objektů byly na přání uživatele odstraněny. Přepíná Orbit/Free kameru a zobrazení mřížky, reaguje na změnu velikosti
okna a nabízí JavaScriptové API přes `window.engine` i `window.scene`.

Ovládání podle kódu: tažení levým tlačítkem na canvasu mění směr pohledu,
W/A/S/D posouvají volnou kameru nebo cíl orbitální kamery. Orbit používá
Equal/NumpadAdd pro přiblížení a Minus/NumpadSubtract pro oddálení.

## Mapa zdrojů

| Soubor | Úloha |
| --- | --- |
| `src/lib.rs` | Kamera dostupná nativně, renderer a web pouze pro `wasm32` |
| `src/web.rs` | Exporty do JS, inicializace canvasu `gpu-canvas`, animační smyčka a vstup |
| `src/scene.rs` | CPU SceneManager: objekty, světla, transformace, časování, animace, ID a validace |
| `src/render/state.rs` | GPU zařízení a zdroje, synchronizace stavu SceneManageru, resize a render |
| `src/render/data.rs` | Vrcholy, geometrie, GPU uniformy, `MAX_LIGHTS = 4` |
| `src/render/pipeline.rs`, `depth.rs` | Renderovací pipeline a depth buffer |
| `src/shader.wgsl` | Transformace vrcholů, ambientní/difuzní/speculární osvětlení, `LIGHT_COUNT = 4` |
| `src/input/` | Free/Orbit kamery, klávesnice a pointer události |
| `index.html` | Hlavní stránka a ovládací panel |
| `js/engine.js` | Inicializace a JS API k Rust enginu |
| `js/scene.js` | Příkaz pro načtení ukázky definované v Rustu |
| `js/engine.d.ts` | Typy JS/TS SDK a objektových/světelných handle |
| `js/app.js` | UI, `window.engine` a kompatibilní `window.scene` |
| `pkg/` | Generované a Git ignorované JS/WASM výstupy |
| `cube.html` | Samostatná JS/WebGL krychle |
| `check_webgpu.js` | Detekce dostupnosti grafických API v headless Chromium |
| `screenshot_render.js` | Screenshot samostatné `cube.html` |
| `render_webgpu.js` | Samostatný JS WebGPU/WebGL trojúhelník |
| `scripts/dump_sources.sh` | Generuje `all_sources.txt` z manifestu, Rustu a shaderů |

## Vývoj a hranice ověřování

```sh
cargo test
RUSTFLAGS=--cfg=web_sys_unstable_apis cargo check --target wasm32-unknown-unknown
RUSTFLAGS=--cfg=web_sys_unstable_apis wasm-pack build --target web
python3 -m http.server
```

Otevřít `http://localhost:8000/index.html`. Native testy pokrývají kamery, ale vynechávají renderer
a webové exporty, proto samy nepotvrzují překlad WASM ani vykreslení. `npm i` připraví
Puppeteer a pngjs pro existující pomocné skripty; ty nenahrazují ověření hlavní
stránky. Před commitem/pushem platí pořadí `cargo test`, poté `scripts/pre-push`.

Rust uniformy a WGSL musí mít shodné rozložení a počet světel. Transformace
objektů používají radiány, `schedule_rotate` přijímá stupně a delay/duration v
sekundách. ID objektů a světel se nyní nepoužívají znovu, ani po resetu scény.
Neplatný/starý handle nemůže ovládat nově vytvořený objekt.

## Historie průzkumu a navazující opravy

- README původně popisovalo pouhé vyčištění canvasu a nesprávně tvrdilo,
  že výchozí Cargo config používá vendor. Popis byl aktualizován.
- Původní patch `GPUAdapter.prototype` byl odstraněn. Stránka nyní umožňuje
  vynutit WebGL přes `?backend=webgl` a vypisuje aktivní backend nebo chybu.
- Inicializace je explicitní: `await init(...)`, poté `await start()`.
  Async automatický WASM start nezaručoval připravenost GPU při sestavování
  JS dema. Při migraci se to projevilo zobrazením pouze výchozí krychle.
- JS a WASM se načítají se stejným parametrem `v` odvozeným z času načtení
  stránky, aby prohlížeč při vývoji nepoužíval staré generované soubory.
- Kamerové klávesy se čistí při změně režimu i ztrátě fokusu okna.
  Odstranění objektu/světla ruší jeho čekající akce před opětovným využitím
  slotu. Zápis světla už neobnovuje smazaný slot.
- Renderer využívá sRGB view pro shodné barvy obou backendů, řeší dočasné
  stavy povrchu bez zastavení smyčky a při ztrátě vytváří nový povrch.
  Nucená ztráta zařízení nebyla testována.
- Offline skripty obnovují `stable-x86_64-unknown-linux-gnu`; nalezený lokální
  symlink `stable-offline` míří do `/workspace/rust/...`. Nejde o připravený
  macOS toolchain. `pack-all` navíc regeneruje vendor a balí Cargo/Rustup cache;
  nepoužívat ho jako běžný krok k prohlédnutí projektu.
- Při zahájení průzkumu už byly upravené README, hlavní HTML, část rendereru,
  shader a webové exporty, smazaný `render_base64.txt` a další lokální cache.
  Tyto existující změny nebyly součástí přidání této dokumentace.
- Navazující ověření po opravách: `cargo test` prošel se třemi testy
  (uvolnění kláves při přepnutí/clear-input a rozsah projekce), kontrola
  WASM a sestavení přes wasm-pack prošly. `scripts/pre-push` obnovil dump.
- Hlavní `index.html` bylo přes browser MCP skutečně vykresleno s WebGPU
  i vynuceným WebGL. Ověřené jsou kamery, pointer drag, klávesový vstup,
  přepínání mřížky, stop přidávání a resize na 640 × 480 s obnovou rozměrů.
  Automatické demo zobrazovalo a odstraňovalo objekty. Poslední build
  nepřidal nové konzolové chyby.
- Spuštění: `./scripts/run-local.sh [port]` sestaví vývojový `pkg/` a spustí
  server na 127.0.0.1. Produkční sestavení lze dělat bez `--dev`.

## Aktuální rozdělení odpovědností

Uživatel upřesnil, že scene manager, logika a výpočty musí být v Rustu.
JavaScript/TypeScript je rozhraní kvůli znalostem programátorů. `src/scene.rs`
je samostatný SceneManager bez GPU/browser závislostí: vlastní data, validaci,
stabilní ID, matice transformací, animace a plánování i ukázkovou scénu.
Renderer pouze synchronizuje GPU zdroje a vykresluje stav manageru.
SDK v JS ukládá jen handle s ID a bindingem; nepřepočítává ani neduplikuje stav.
API: engine.scene.createCube/createPlane/createSphere/createLight,
objekt.setPosition/setRotation/setScale/rotate/removeAfter/remove,
scene.clear/loadExample a počty čtené přímo z Rustu.

Ověření: osm Rust testů (kamery a SceneManager) prošlo, WASM build prošel,
TypeScript příklad/deklarace se překládají přes npm run typecheck.
Skutečný integrační test tests/scene-sdk.html přes browser MCP prošel:
prázdný start, ukázka z Rustu, JS příkazy, Rustem časované odstranění,
stabilní ID, odmítnutí starého handle, světla a vyčištění.
Ukázka nepřidává objekty automaticky. Historie výše může popisovat dřívější demo.
