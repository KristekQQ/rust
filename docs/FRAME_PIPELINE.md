# Frame pipeline: plánování a rozšiřování

Rust vlastní plán, prostředky i skutečné GPU příkazy. JS poskytuje tenké příkazy
a diagnostiku `engine.frameProfile()` a `engine.postprocessing.graph()`.
Veřejné API není editor libovolných WGSL pipeline. Graf sám nespouští shader,
nepřenáší simulaci automaticky na GPU a nemění její časový krok.

## Mapa implementace

| Soubor | Odpovědnost |
|---|---|
| `src/frame_graph.rs` | GPU-independent `Graph`, validace, pruning, pořadí a sloty |
| `src/postprocess_plan.rs` | `PostprocessPlan`, deklarace aktivního řetězce efektů |
| `src/render/graph_resources.rs` | Skutečné alokace transient slotů přes `GraphResources` |
| `src/render/postprocess.rs` | Cache targetů, bind groups a encoding efektů |
| `src/render/renderer.rs`, `state.rs` | Aktualizace scény, render, overlays a XR předání |
| `src/render/profiler.rs` | CPU scopes a volitelné asynchronní GPU timestampy |
| `src/web.rs`, `js/engine.js`, `js/engine.d.ts` | JSON diagnostika a typované SDK |

## Jeden snímek

Simulace a vstupy upraví Rust scénu. V XR proběhnou i změny od ovladačů a řezání;
`finish_scene_updates()` potom publikuje mesh/pose změny jednou před prvním okem.
Další oko používá stejný publikovaný stav. Finalizace neposouvá simulaci znovu.
Desktop ji volá po své aktualizaci scény.

Scénové průchody zachovávají své nezávislé visibility/cache rozhodování.
Postprocessing používá deklarovaný řetězec; výsledné transparentní objekty,
cut rings, glow a helpers sdílejí finální overlay pass při zachovaném pořadí.
Graf postprocessingu neznamená, že všechny engine průchody již mají jeden encoder.

WebGPU XR dokončí každé oko do vlastního color/depth cíle a následně odešle
obě browserové slices **jediným finálním stereo submit**. Tento prezentační krok
zachovej i při slučování encoderů. Nesdílej pozdě přepsaný uniform buffer mezi
očima v jednom batchi: každé oko potřebuje vlastní buffer/range, nebo původní
bezpečné pořadí zápisu a submitu. Také cesta bez efektů používá stage: browser
UNORM textura nesmí dostat nekódované lineární RGB.
Při stagingu validuj browser metadata přes `XrAttachments::inspect()`; GPU views
vytváří až `new()` ve finální prezentaci. Nevytvářej je pouze kvůli zjištění
rozměrů. Validace a skutečné importování sdílejí stejný kontrakt.

Engine-owned eye textura má sRGB render view a explicitně povolený UNORM view.
Scéna, PBR, SSR a overlay se skládají v lineární barvě; hardware zakóduje výsledek
při sRGB zápisu. Finální shader čte UNORM view (`textureLoad`) a kopíruje již
zakódované hodnoty do browserového UNORM cíle. Nečti zde sRGB view (znovu by
barvu dekódoval) a nepřidávej další gamma převod. Browserové XR textury mají
prázdné `viewFormats`; jejich sRGB reinterpretaci nepovolujeme odhadem.
U SSR nepřibyl pass, bez efektů je nyní fullscreen předání na oko potřebné také.
`webxr-webgpu` ověřuje lineární šedé .02/.18/.5 proti přesným sRGB hodnotám
v obou očích s efekty i bez nich; headsetové potvrzení barvy zůstává samostatné.

`PostProcessorViewUniforms` používá oddělené dynamické rozsahy: desktop `0`,
levé oko `1`, pravé oko `2`. Toto oddělení platí pro postprocess uniforms;
ostatní renderer/effects buffery tím automaticky nezískávají per-eye rozsahy.
Přesunutí více očí do společného encoderu vyžaduje samostatné posouzení všech
takových zápisů, nikoli pouze změnu počtu submitů.

Uživatel potvrdil baseline `b07ae09b` na Questu 3 / Browser 152.1; nejde o potvrzení
nových změn grafu ani prokázaný výkonový zisk. Podrobnosti: [WEBXR.md](WEBXR.md).

## Odesílání příkazů jednoho pohledu

`Effects::render` / `SceneRenderer::render_view` vrací hlavní command buffer;
shadow/reflection/environment se dál odesílají ihned před přepsáním sdílených
uniformů. `PostProcessor::render` vrací počet passů a volitelný command buffer,
`draw_postprocess_overlays` volitelný overlay buffer. `State::render_to` tyto
výsledky lokálně seřadí **main → postprocess → overlays** a odešle jedním
`queue.submit` pro daný pohled. Postprocess frame counter se zvýší až potom.
Při chybě se lokální neodeslané buffery zahodí; nic se neukládá do dalšího snímku.

Batch musí být odeslán před depth resolve, HUD, finální prezentací i přípravou
dalšího oka. Nesbírej auxiliary passy kolem celého `render_view`: přepisují stejné
uniformy. V běžné XR aréně se SSR, bez využití compositor depth a bez debug HUD,
jsou nyní dvě odeslání očí + jedno finální stereo předání místo šesti + jednoho.
Přibyde-li skutečný shadow/reflection pass, depth resolve, HUD či profiler readback,
má vlastní odeslání. Počet render passů ani ray-march kroků se tím nesnižuje;
headsetový FPS přínos je nutné měřit. `webxr-webgpu` kontroluje skutečné submitty
včetně jejich command bufferů a pass labels, nikoli jen deklarovaný graf.

`State::render_to` je společný pro wgpu WebGPU i WebGL, takže tato úprava
odesílání platí pro oba backendy. Číslo 7 → 3 je ověřené pro uvedenou WebGPU XR
cestu s finálním stereo předáním; není měřením počtu odeslání WebGL ani důkazem
příčiny rozdílu FPS mezi backendy.

Statická scéna může držet geometrii, bindings a plán bez opakované přípravy;
shadow mapy a environment výsledky lze obnovovat podle změn jejich závislostí.
Pohyb kamery/hlavy však mění pohled i nad statickou geometrií, takže SSR a výstup
každého oka nelze automaticky převzít z minulého snímku. Další reuse (například
render bundles na podporovaném backendu) vybírej podle měření a konkrétních
závislostí, ne pouze podle označení scény jako statické.

## Kontrakt grafu

`Graph::resource(name, desc, lifetime)` vrací `ResourceId`.
`ResourceDesc` je `Texture { width, height, layers, samples, format, usage }`
nebo `Buffer { size, usage }`; v této verzi jde o 2D textury s jedním mipem.
`Lifetime` rozlišuje `Transient`, `Persistent`, `External`.

`Graph::pass(name, reads, writes, side_effect)` deklaruje pass a vrací `PassId`.
Logický resource má nejvýše jednoho writera v celém grafu. Pro další verzi dat,
oko nebo samostatně plánovaný subresource vytvoř další handle. Čtení i zápis
stejného handle v jednom passu je odmítnut; in-place storage není podporované.

`compile(outputs)` validuje i nepoužité větve: rozměry/velikost, neznámé handles,
chybějící inicializaci transient dat, více writerů a cykly. Passy mohou být
deklarované mimo pořadí. External/persistent jsou inicializované callerem;
pokud mají v grafu writera, čtení na něm závisí. Označení `Persistent` samo
nezakládá cache ani alokaci.

Kořeny jsou požadované outputs a passy s `side_effect=true`; jejich závislosti
zůstanou, ostatní passy se odstraní. Side effect použij například pro skutečný
externí zápis, který nemá požadovaný output. Neoznačuj jím každý pass, zrušil by pruning.

Výsledek obsahuje `ordered_passes`, `resource_slots`, `slots`,
`resource_lifetimes` a `pruned_pass_count`. Životnosti jsou **inkluzivní** indexy
ve výsledném pořadí. Output žije až do konce grafu. Slot lze sdílet pouze při
stejném descriptoru a `previous.last_use < next.first_use`; použití ve stejném
passu se překrývá. Rozdílné usage, format, velikost, layers nebo samples nealiasují.
External/persistent i nepoužité resources mají `resource_slots[id.0] == None`.
Názvy a JSON plán poskytuje `graph.debug_summary(&compiled)`.

## Skutečné alokace a invalidace

`GraphResources::new(device, &compiled)` vytvoří jednu Texture + TextureView
nebo Buffer na fyzický transient slot. `view(id)`, `texture(id)`, `buffer(id)`
vracejí prostředek podle logického handle; externí/persistent data dodává caller.
Usage flags se nepřidávají automaticky. Alokátor kontroluje základní descriptor
a limity zařízení; konkrétní podporu kombinace formátu/usage validuje wgpu.
`allocated_bytes_estimate()` počítá přibližný payload bez driver alignmentu.

Drž plán a alokátor s target cache, nealokuj je v každém stabilním snímku.
Přepočítej je při resize, změně formátu, layers/samples, aktivních efektů,
jejich pořadí nebo režimu měnícího rozměry/závislosti, například half-resolution SSR.
Změna síly efektu obvykle mění uniform, nikoli topologii nebo descriptor.
Rebuild bind groups po změně alokací; při dispose uvolni jejich vlastnictví.

`PostprocessPlan::new(effects, width, height, format, uniform_bytes)` deklaruje
scene color/depth a per-view uniforms jako external, mezivýsledky jako transient.
Half-trace rozměry zaokrouhluje nahoru. Encoder ověřuje shodu trace/compose kroků
s kompilovaným pořadím; plán není pouze dekorativní diagnostika.

## Upload instancí běžných meshů

`renderer.rs::ImportedInstances` sdílí instanční buffery v rámci jednoho pass cache.
Importované triangle meshe mají vlastní byte ranges; color, shadow, cut rings a
neon glow bindují právě tento rozsah. Nejde o slučování draw calls ani změnu
pořadí transparentních objektů. glTF používá vlastní GPU pose data a do těchto
instančních stránek se neposílá.

Běžná stránka pojme nejvýše 4096 instancí; větší skupina jednoho meshe zůstává
souvislá v samostatném bufferu. Každá změněná stránka znamená jeden `write_buffer`
místo mnoha malých zápisů po objektech. Shodná data se znovu neposílají. Cena je
přenos celé změněné stránky, i když se změnila jen její část. Stránky a CPU scratch
se znovu používají; po odstranění objektů se odstraní jejich ranges i neodkazované
koncové stránky a prázdná scéna pool uvolní. `debugStats().instanceUploadPages`
ukazuje počet skutečně vlastněných bufferů ve všech pass cache.

Zachovej byte offsety ve všech čtyřech čtecích cestách. Pořadí uniform zápisů a
submitů mezi passy/očima tato optimalizace nemění. `webxr-webgpu` ověřuje skutečné
GPU uploady více meshů a uvolnění všech stránek po clear; sdílené cesty ověřují
neon arena, glTF, shadow/reflection a mirrored pixelové scénáře na obou backendech.

## Malý příklad: nový storage buffer

Následující nativní test lze vložit do existujícího `mod tests` v
`src/frame_graph.rs` a spustit `cargo test buffer_extension --lib`.
Ukazuje producer, consumer a exportovaný výsledek; GPU shader se zde nespouští.

```rust
#[test]
fn buffer_extension() {
    use crate::frame_graph::{Graph, Lifetime, ResourceDesc};
    let mut graph = Graph::default();
    let scratch = graph.resource(
        "particles.filtered",
        ResourceDesc::Buffer {
            size: 1024,
            usage: wgpu::BufferUsages::STORAGE,
        },
        Lifetime::Transient,
    );
    let exported = graph.resource(
        "particles.result",
        ResourceDesc::Buffer {
            size: 1024,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        },
        Lifetime::Transient,
    );
    let producer = graph.pass("filter", &[], &[scratch], false).unwrap();
    let consumer = graph.pass("summarize", &[scratch], &[exported], false).unwrap();
    let plan = graph.compile(&[exported]).unwrap();
    assert_eq!(plan.ordered_passes, [producer, consumer]);
    assert_ne!(plan.resource_slots[scratch.0], plan.resource_slots[exported.0]);
    assert_eq!(plan.resource_lifetimes[exported.0].unwrap().last_use, 1);
}
```

Pro runtime přidej Rust handler a WGSL pipeline, vytvoř `GraphResources` a bind
groups nad `resources.buffer(scratch)?` / `resources.buffer(exported)?`.
Handler musí podle `plan.ordered_passes` skutečně zakódovat producer před consumer;
každý deklarovaný zápis musí naplnit data, která consumer čte. Graf nedoplňuje
binding layout, shader, dispatch ani kopie. Chceš-li výsledek zachovat přes další
snímky, použij caller-owned persistent buffer a deklaruj jeho novou verzi.
Pro CPU čtení přidej explicitní kopii do MAP_READ/COPY_DST readback bufferu;
`COPY_SRC` na exported samo readback nespouští. Pro upload přidej `COPY_DST`
a deklaruj příslušný vstup; storage binding musí odpovídat WGSL přístupu.

## Profil a ověření

`initEngine(canvas, {profiling:true})` zapne diagnostický profiler před vytvořením
GPU zařízení. Výchozí `false` nevyžaduje timestamp feature, nevytváří query/readback
prostředky a CPU scopes nečtou hodiny. `engine.frameProfile().enabled` rozlišuje
vypnuté měření od nepodporovaných GPU timestampů. V neon aréně použij `?profile=1`;
pro měření FPS vždy porovnej také běh bez profileru.

`engine.frameProfile()` odděluje CPU scope časy (`lastMs`, `meanMs`, `p95Ms`)
od `gpu.passesMs`. CPU zahrnuje práci hosta/encoding, nikoli dokončení GPU. CPU scopes mají statické
názvy a po první registraci znovu používají předalokované 60prvkové historie;
nevytvářej dynamické názvy podle ID objektů.
Volitelná device feature `TIMESTAMP_QUERY` se žádá jen tam, kde ji adapter nabízí.
GPU collector používá tři asynchronní readback sloty: vzorek prvního snímku a
potom každého 30. snímku, bez blokujícího čekání; při obsazeném ringu vzorek přeskočí.
`supported`, `available`, `sampleFrame`, `ageFrames`, `skippedSamples` a `error`
umožňují rozlišit chybějící podporu, čekající nebo starý výsledek. Timestamp není
čas XR kompozitoru; opakované pass labels se ve vzorku sčítají.

Pro změny plánu použij cílené `cargo test frame_graph --lib` nebo
`cargo test postprocess_plan --lib`. Nový buffer testuj i v runtime: obsah,
správné pořadí, bind groups, chybové usage a cleanup; native test nezakóduje GPU pass.
Scénář `frame-graph` na WebGPU spouští skutečný storage-buffer producer/consumer,
ověřuje oba 64prvkové exporty GPU readbackem a tři fyzické sloty pro čtyři
logické buffery: překrývající se buffery odděluje, nepřekrývající sdílí a první
export zůstává platný až do konce. WebGL compute cestu tímto scénářem nenahrazuje.
Po integraci rendereru ověř matching WASM a dotčené `postprocessing`,
`webxr-webgpu` scénáře (včetně SSR pixelů) a sdílené cesty na WebGL podle [DEVELOPMENT.md](DEVELOPMENT.md).
Sleduj skutečné pixely obou očí, efekt on/off, resize a stabilní počet alokací.
Nedostupné timestampy jsou podporovaný fallback, nikoli nulový GPU čas.
Před požadovaným commitem/pushem platí jeden `npm run verify:push` gate;
simulované XR ani desktop WebGPU nenahrazují novou headset kontrolu.


Ověření implementace 2026-10-19: cílené native testy grafu a postprocess plánu,
WASM check/Clippy, TypeScript, 20 SDK a 13 XR unit testů prošly. Třináct
browserových scénářů na WebGPU/WebGL pokrylo storage buffery, SSR v obou očích,
stabilní alokace, neon arénu, glTF, render effects a postprocessing.
Po odstranění opakovaných CPU-profiler alokací znovu prošly `webxr-webgpu`
a `scene-sdk` na obou dostupných backendech (tři scénáře). Nejde o celý push gate
ani o headset benchmark; novou verzi musí uživatel ověřit v Questu.
