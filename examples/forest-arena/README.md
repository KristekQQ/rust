# Hvozd obrů — lesní VR aréna

Samostatná přírodní aréna na `/examples/forest-arena/`: 100 × 120 m, přibližně 10,7× větší obdélníková navigační oblast než 29 × 38,8 m ulice Neon District (plocha před odečtením překážek). Lesní mýtiny, kamenné ruiny a vzdálené orientační body tvoří prostor pro budoucí souboje.

Levý analog ovládá chůzi 2,5 m/s podle hlavy, pravý otáčí o 30°. Držení pravého A zpomalí animace na 15 % a zobrazí cílovou elipsu, puštění teleportuje až 10 m. B zamíření zruší. Průchozí povrch je rovina y=0; kameny, kmeny a ruiny mají explicitní navigační půdorysy. Vyvýšené dekorace nejsou choditelná patra.

Tři originální druhy (golem, stromový strážce, houbový tvor) mají klidové animace. Tato verze nemá AI, útoky, zásahy ani zabíjení. Runtime pohybu, animací a vykreslování zůstává v Rustu. JS načítá modely a předává konfiguraci; nemá vlastní simulační smyčku.

`assets/models/forest-arena/` obsahuje editovatelný Blender zdroj, reprodukovatelný builder, GLB a `layout.json`. Ukázka potřebuje sdílený `js/`, matching `pkg/` a tento asset adresář; lze ji kopírovat nezávisle na ostatních arénách.

Cílená kontrola: `VERIFY_WASM_PROFILE=release npm run test:browser -- --page forest-arena,forest-arena-xr --backend all --capture`. Kontroluje velikost, propojené průchozí trasy, skutečné pixely tří pohledů, stereo, analogový pohyb, teleportové přesuny, zpomalení animací a zrušení po ztrátě trackingu. Pro balíček přidej `--production-root output/production`.

Pro Quest použij `npm run serve:quest`, poté `/examples/forest-arena/` na existujícím HTTPS serveru. Automatické desktopové a simulované XR ověření nenahrazuje komfort a výkon na fyzickém headsetu.

## FPS and hand tracking diagnosis

Revision `forest-arena-spatial-fps-2` uses a real native `scene` information panel (0.66 × 0.40m), attached beside the right foot at floor offset [0.9,0.35,-1]m via `anchor:'feet'`. It follows body translation and locomotion/reference-frame rotation through Rust transforms; looking sideways/up/down leaves its local position fixed, and crouching does not pull it toward the face; both eyes see the same 3D object at finite depth. Screen-space debug HUD stays off. The checkbox detaches/hides or rebinds the panel; tracking loss and XR exit hide it without destroying the binding. This placement is an initial layout, not a claim of verified headset comfort.

The native panel updates FPS/frame/CPU/draw counters without a JS animation loop. Existing bounded 180-frame XR samples after warmup are sent to same-origin diagnostics; the last headset sample remains on the page after exit. `?profile=1` enables optional detailed profiling. No geometry, shadows, resolution or hand update settings changed. Application FPS is not the compositor refresh rate.
