# SceneManager a rychlé vykreslování

Pro tento engine volíme Rust scene state → cache transformací a bounds →
CPU frustum culling → dávky podle meshe → instancované vykreslení wgpu.
JS/TS pouze posílá příkazy a čte diagnostiku. GPU vykonává WGSL shadery.

## Co už je implementováno

- `SceneObject` drží cache modelové/normalové matice a world AABB. Cache se
  obnovuje při změně transformace, včetně animací, nikoli při pohybu kamery.
- `visibility.rs` extrahuje šest rovin z view-projection matice. Near rovina
  používá řádek Z, protože clip depth je 0…W. AABB test je konzervativní:
  rotovaný, plochý či částečně viditelný objekt se nesmí chybně vyřadit.
- Culling omezuje pouze vykreslování. Animace a plánované odstranění běží dál.
- Revize geometrického stavu a matice kamery zneplatňují cache extrakce.
  Ve statické scéně se culling ani sestavení dávek neopakuje každý snímek.
  Pohyb světel aktualizuje společné osvětlení bez opakování cullingu objektů.
- Tři opakovaně používané vektory obsahují viditelné instance krychlí, rovin a
  koulí. Každý neprázdný mesh má jeden `draw_indexed` s více instancemi.
- Geometrie, pipeline a bind group se sdílí. Kamera a světla se zapisují
  pro každý vykreslený pohled do společného 208B uniformu. Instance mají 144 B a využívají vertex
  buffer s krokem Instance, takže řešení podporuje WebGPU i WebGL2.
- GPU instance buffery rostou geometricky a používají se znovu. Nezměněná
  dávka se nezapisuje; změna transformace nebo viditelnosti aktualizuje dávku.
- `engine.renderer_stats()` vrací total, visible, culled, drawCalls,
  uploadBytes a prepareMs; `engine.set_frustum_culling(bool)` umožňuje A/B
  porovnání. Draw calls nezahrnují pomocné čáry. prepareMs měří Rust simulaci,
  přípravu/encoding/submission všech průchodů, nikoli GPU execution.

## Ověření a meze měření

Původní verze `tests/render-stress.html` vytvořila 10 000 krychlí, z nichž 100 leželo v záběru.
Na WebGPU i WebGL bylo ověřeno 100 viditelných, 9 900 vyřazených a jeden
objektový draw call. Ve stabilním snímku bez změn se přenáší pouze 208 B.
Po vypnutí cullingu se odešle všech 10 000 instancí; stále jde o jeden draw
call a v dalších stabilních snímcích se instance znovu nepřenášejí.

Aktuální stránka volá JS `createScene` přes SDK v `examples/culling/scene.js`: 20 × 25 × 20
krychlí rozmístěných kolem kamery v prostoru přibližně 57 × 43 × 57 jednotek.
Je určena pro létání a vizuální kontrolu; počet vyřazených krychlí není fixní.
Volná kamera má Q/E pro vertikální pohyb a Shift pro čtyřnásobnou rychlost.
Stránka nabízí i pevnou ukázku a přepínání Free/Orbit. Spouštět přes HTTP,
např. `http://127.0.0.1:8000/tests/render-stress.html`, nikoli file://.

FPS a průměrný interval snímku jsou počítány `FrameMetrics` v Rustu z intervalů
RAF v půlsekundových oknech. Pauza delší než 250 ms resetuje okno, aby přepnutí
do pozadí neznečišťovalo průměr. Jde o frekvenci browser smyčky, nikoli měření
času GPU nebo důkaz fyzické prezentace každého snímku. Statistiku zobrazuje
zátěžový test i hlavní ukázka; JS pouze formátuje hodnoty.

## BVH, octree, kd-tree

BVH není univerzálně nejlepší. Pro pravidelně rozmístěné podobně velké objekty,
voxelový svět nebo statické chunky je přirozenou volbou uniform grid nebo octree.
Pro různě velké a nepravidelně rozložené objekty je obvykle praktičtější BVH.
Kd-tree je relevantní pro statické ray-intersection úlohy, ale není automaticky
rychlejší při výběru celých objektů podle šesti rovin frusta. Náklady údržby,
duplikace přes hranice buněk a rozložení dat v paměti jsou stejně důležité jako
název stromu. Současný engine má zatím flat AABB scan s cache statického pohledu,
nikoli BVH ani octree. Další strukturu vybrat porovnáním pohybující se kamery
ve stejné scéně, včetně nákladů aktualizací a tvorby dávek.

Srovnání: [PBRT BVH](https://www.pbr-book.org/4ed/Primitives_and_Intersection_Acceleration/Bounding_Volume_Hierarchies),
[Babylon.js selection octree](https://github.com/BabylonJS/Documentation/blob/master/content/features/featuresDeepDive/scene/optimizeOctrees.md).

Předchozí renderer měl N objektových draw callů a N zápisů uniformů,
přičemž každý snímek posílal 336 B na každý objekt, i mimo záběr. Pro 10 000
objektů je to 3 360 000 B plus pomocná data. Toto je srovnání práce v kódu,
nikoli změřený násobek FPS. Culling přidává CPU práci a u drobných meshů
nemusí sám zvýšit FPS; instancing odstraňuje hlavní režii počtu GPU volání.

Časy porovnávat v release WASM (`wasm-pack build --target web --release`),
ve stejném prohlížeči, rozlišení a scéně, po zahřátí. Debug WASM, souběžné
karty, throttling a hrubé browser časovače výsledky výrazně ovlivňují.
GPU timestamp queries doplnit podle schopností zařízení pro GPU bottlenecky.

## Další kroky podle profilu skutečné scény

1. **Velká CPU režie extrakce:** husté úložiště objektů a ID → index mapa,
   se zachováním stabilních veřejných ID. BTreeMap se zatím zachovává kvůli
   jednoduchosti a deterministickému pořadí. Současný culling je O(N).
2. **Rozlehlá převážně statická scéna:** BVH nad world bounds nebo prostorové
   chunky; nejprve testovat uzly, pak jejich objekty. Aktualizovat pouze
   změněné bounds. Strom není automaticky výhodný u malé či silně dynamické scény.
3. **Časté změny několika instancí:** stabilní sloty a zápis změněných rozsahů;
   dnešní kompaktní dávky při změně přenášejí celou příslušnou dávku.
4. **GPU čas při mnoha viditelných meshech:** LOD podle velikosti v pixelech,
   batch klíč mesh + material + pipeline, případně řazení opaque zepředu dozadu.
   Transparentní objekty vyžadují vlastní pořadí; zrcadlené transformace vlastní
   variantu orientace/cullingu. Tyto obecné material/LOD větve ještě nejsou API.
5. **CPU culling je skutečně bottleneck:** WebGPU compute nad storage bounds,
   kompakce viditelných ID a nepřímé instance draws. Bez čtení seznamu zpět na
   CPU každý snímek; pro WebGL zachovat CPU cestu. Nezavádět compute pouze
   kvůli názvu: dispatch, buffery a synchronizace mají vlastní režii.
6. **GPU overdraw a zakryté objekty:** Hi-Z occlusion culling až s depth
   pyramidou, časovou konzervativností a profilem, který ospravedlní další passy.

## Zdroje

- [WebGPU Fundamentals: Speed and Optimization](https://webgpufundamentals.org/webgpu/lessons/webgpu-optimization.html)
  vysvětluje omezení zápisů, bind groups, draw calls a vhodnost instancingu.
- [Learn Wgpu: Instancing](https://sotrh.github.io/learn-wgpu/beginner/tutorial7-instancing/)
  ukazuje instanční vertex buffer a více instancí jednoho meshe.
- [wgpu 30 RenderPass](https://docs.rs/wgpu/30.0.1/wgpu/struct.RenderPass.html)
  popisuje instanční rozsahy, nepřímé draws a požadavky jejich API.
- [glam DirectX/WebGPU projekce](https://docs.rs/glam/latest/glam/camera/lh/proj/directx/index.html)
  dokumentuje projekce pro používané clip-space konvence.

## Renderovací cíl oddělený od canvasu

`renderer.rs` obsahuje SceneRenderer, GPU zdroje scény, culling, instance buffery
a cache pipeline podle formátů barevné/hloubkové přílohy. Nepracuje s HTML,
canvasem ani surface. `RenderView` předává view-projection a pozici kamery;
`RenderTarget` předává TextureView pro barvu a hloubku, formáty, rozměry,
viewport a operace Clear/Load. Podporovány jsou single-sample 2D přílohy.
Rozměry viewportu se kontrolují před zahájením render passu, kompatibilitu
GPU příloh kontroluje wgpu. Clear vymaže celou přílohu; při dalším pohledu do
stejné přílohy musí volající zvolit Load pro zachování předchozího obrazu.

`prepare_view` a `render_view` neposouvají simulaci. Host ji aktualizuje jednou
za snímek, pak může připravit/renderovat jednotlivé pohledy. `render_view`
submittuje každý pohled před přípravou dalšího, aby zápisy do sdílených
uniformů a instance bufferů nebyly všechny použity s daty poslední kamery.
Instancing a cache extrakce zůstávají zachovány; pipeline se nevytváří každý
snímek. Plná VR integrace a WebXR binding zatím nejsou implementovány.

`output.rs` je adaptér pro canvas: získá surface frame, řeší resize/ztrátu
surface a prezentuje snímek. `state.rs` propojuje tento adaptér s rendererem.
SDK `engine.set_output_mode("canvas" | "texture")` dovoluje ověřit dva výstupy:
přímo do surface, nebo do samostatné textury s hloubkou a následnou prezentací
fullscreen trojúhelníkem. Druhý režim přidává prezentovací pass a paměť textur;
není automaticky rychlejší. Výchozí režim zůstává přímý. Textury se vytvářejí
při prvním použití a znovu po resize, nikoliv každý snímek.

`tests/render-target.html` testuje přepínání, odmítnutí neplatného režimu,
resize a přítomnost scény. Viditelný obraz a konzoli je nutné ověřit i v
prohlížeči, včetně `?backend=webgl`.

Stíny a odrazy: [aktuální architektura průchodů](RENDER_PASSES.md). InstanceData nyní obsahuje také 16 B materiálu a má 144 B.
