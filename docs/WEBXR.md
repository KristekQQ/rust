# WebXR na Meta Quest 3

Pro WebXR je potřeba bezpečný kontext: důvěryhodné HTTPS, nebo `localhost`.
Běžné `http://192.168.…` v headsetu nestačí. Lokální vývoj může používat vlastní certifikát. Meta popisuje ruční potvrzení
jeho varování v Quest Browser; obecně to nenahrazuje důvěryhodnou certifikační
cestu v každém prohlížeči. Stav WebXR je potřeba ověřit přímo na headsetu.

## Lokální HTTPS přes Wi-Fi (bez veřejného serveru)

Quest a Mac musí být ve stejné nebo vzájemně dostupné lokální síti. Spusť:

```sh
scripts/run-quest.sh
# Jiná síťová karta / port:
QUEST_HOST=192.168.0.17 QUEST_PORT=8443 scripts/run-quest.sh
```

Skript vytvoří lokální CA a serverový certifikát pro aktuální LAN IP pomocí
OpenSSL. CA se při dalších spuštěních zachová, serverový certifikát má platnost
90 dní a obnoví se při spuštění. Klíče jsou v `~/.local/share/rust-webxr/tls/`,
mimo repozitář. Skript neinstaluje certifikáty do systémového úložiště důvěry
Macu ani Questu. Nepoužívá tunel ani veřejnou službu.

V Quest Browser otevři vypsanou adresu `https://<Mac-LAN-IP>:8443/examples/webxr/`.
Meta v [postupu lokálního testování](https://developers.meta.com/vr/documentation/iwsdk/guides/02-testing-experience/)
popisuje ruční potvrzení varování vývojového certifikátu a následný vstup do XR.
Tento krok provádí uživatel na headsetu. Pokud WebXR zůstane nedostupné,
je potřeba řešit důvěru certifikátu v konkrétní verzi Quest Browser.

Veřejný certifikát CA je `~/.local/share/rust-webxr/tls/rootCA.crt`; lze ho
přenést do zařízení, pokud jeho systém umožňuje instalaci vlastní CA.
Soubory `*.key` nikam nekopíruj. Důvěra na Macu se automaticky nepřenáší do Questu.
Po změně IP znovu spusť skript. Firewall musí povolit lokální spojení na port 8443.
Server zpřístupní vývojový repozitář pouze na zadané síťové adrese; po práci jej
ukonči Ctrl+C. Na routeru není potřeba přesměrovávat porty.

Vlastní již důvěryhodný certifikát lze dodat také explicitně:

```sh
scripts/run-quest.sh /cesta/certificate.pem /cesta/private-key.pem
```

## Quest na jiné síti: veřejné HTTPS

Pro rychlý vzdálený test slouží `scripts/share-quest.sh`. Vyžaduje matching
`pkg/` a [cloudflared](https://developers.cloudflare.com/tunnel/get-started/quick-tunnels/)
z oficiální distribuce Cloudflare. Skript hledá `cloudflared` v PATH nebo
v ignorovaném `output/tools/cloudflared`; alternativně nastav `CLOUDFLARED_BIN`.

```sh
scripts/share-quest.sh
```

V Quest Browser otevři vypsanou adresu `https://….trycloudflare.com` a stiskni
**Vstoupit do VR**. Certifikát poskytuje Cloudflare, headset nepotřebuje lokální CA.
Mac musí zůstat zapnutý, připojený k internetu a skript musí běžet. Ctrl+C ukončí
server i tunel. Adresa je dočasná a při dalším spuštění se změní.

Skript kopíruje jen WASM, JS moduly, soubory WebXR ukázky a tento návod do
samostatného dočasného adresáře; zdrojový repozitář nesdílí. Kopie zůstává stejná
po dobu běhu: po změně aplikace skript znovu spusť. Každý se znalostí adresy má
k ukázce přístup. Quick Tunnel je určený pro vývoj, bez záruky dostupnosti.

## Nejrychlejší vývoj přes USB

Zapni vývojářský režim a USB debugging Questu, připoj USB a povol přístup
k zařízení. S nainstalovaným Android Platform Tools:

```sh
# V kořeni tohoto repozitáře; vytvoří pkg/ a spustí HTTP na Macu.
scripts/run-local.sh 8000
# V druhém terminálu:
adb devices
adb reverse tcp:8000 tcp:8000
```

V **Meta Quest Browser** otevři `http://localhost:8000/examples/webxr/`.
ADB přesměruje localhost headsetu na Mac; localhost je potenciálně důvěryhodný
původ i přes HTTP. USB musí zůstat připojené. Pokud už matching `pkg/` existuje,
server bez opakovaného buildu spustí `python3 scripts/serve-local.py 8000`.

## Engine a ukázka

Otevři `examples/webxr/` v Meta Quest Browser a stiskni **Vstoupit do VR**.
Ukázka má vlastní `main.js` a `scene.js`; podlaha je y=0, jednotka je metr.
Používá `immersive-vr`, `local-floor` (s fallbackem na `local` a výškou 1,6 m),
polohu a orientaci hlavy a samostatné pohledy očí. Ukončení systémovým menu
vrátí desktopový náhled a dovolí opakovaný vstup. Ztráta trackingové polohy
vynechá obraz; při chybě rendereru se relace ukončí a UI zobrazí chybu.

```js
import { initEngine } from '../../js/engine.js';
const engine = await initEngine(canvas, { backend: 'webgl' });
// Sestav vlastní scénu přes engine.scene.
button.onclick = () => engine.xr.enter().catch(error => console.log(error));
// engine.xr.exit(); engine.dispose();
```

`engine.xr.isSupported()` vrací dostupnost; `isPresenting` stav relace.
Události `start`, `end`, `error` a `lastError` umožňují vlastní UI.
`engine.dispose()` ukončí i XR a zruší jeho snímky; staré handles zůstávají neplatné.
JS platformový adaptér předává XR matice do WASM a obraz canvasu do
`XRWebGLLayer`. Rust převádí rozsah hloubky OpenGL na wgpu, počítá pohled,
culling, scénu i simulaci. Simulace postupuje jednou za XR snímek.

## Přímé WebGPU XR a kompatibilní WebGL

Neon aréna na `/examples/neon-arena/?backend=auto` umí přímý WebGPU výstup,
pokud prohlížeč poskytuje `XRGPUBinding`. Meta uvádí experimentální WebGPU XR
od [Browser 146.0](https://developers.meta.com/vr/downloads/package/browser/146.0/).
Pokud XR adaptér není dostupný, desktopový náhled použije běžný adaptér a
VR nabídne cestu WebGL. Dostupnost závisí na konkrétním headsetu a prohlížeči; existence desktopového
WebGPU sama o sobě nestačí. Při chybě použij odkaz `?backend=webgl`.

SDK před inicializací označí canvas pro XR, Rust požádá o adaptér s
`xrCompatible: true` a použije stejné GPU zařízení pro renderer i XR binding.
Relace vyžaduje feature `webgpu`. Rust vykresluje každé oko přímo do příslušného
2D řezu browserových color/depth texture arrays; canvas se pro oči nezvětšuje,
neprobíhá upload canvasu, blit ani CPU readback. WebGPU projekce už používá
hloubku 0..1, proto se znovu nepřevádí z OpenGL. Cizí textury se drží jen při
vykreslení oka, engine je neničí. Zatím podporujeme celé samostatné řezy očí,
`rgba8unorm`/`bgra8unorm` a `depth32float`. Zapnutý postprocessing používá
mezicíle pro každé oko a finální pipeline odpovídající formátu XR řezu; SSR má
kameru a hloubku právě vykreslovaného oka. Jiný než celý viewport řezu vyvolá explicitní chybu.
Okluze následných vrstev používá interní depth mezicíle. Postprocess cesta po
overlays předává skutečnou hloubku externímu XR attachmentu depth-only GPU průchodem
(`textureLoad` → `frag_depth`), protože browserové attachmenty nepotřebují COPY_DST.
Kompozitor tak nedostává nevyplněnou hloubku ani při `usesDepthValues:true`.
Nativní GPU test porovnává obsah obou depth slices s přímým renderem stejné scény.

`wgpu 30.0.1` nemá volbu `xrCompatible` ve veřejném request API. Proto je
v `third_party/wgpu` lokální kopie se dvěma úzkými změnami popsanými v
[PATCH.md](../third_party/wgpu/PATCH.md); běžný konstruktor zůstává stejný.
Otisk buildu zahrnuje i tuto závislost.

Při `backend=webgl` zůstává původní cesta: každé oko se vykreslí přes Rust/wgpu
a zkopíruje přes canvas do WebGL kontextu XR kompozitoru. Každé oko má vlastní
přenosovou texturu, jejíž úložiště se mění jen při změně rozměrů. JS nečte
pixely zpět na CPU. Přímé WebGPU odstraňuje tyto přenosy, ale skutečný přínos FPS
je nutné změřit na headsetu; desktopová simulace to nedokazuje.

Ověření: `VERIFY_WASM_PROFILE=release node scripts/test-browser.mjs --page webxr-webgpu --backend webgpu --capture`
používá simulovaný XR runtime a skutečné GPU textury/Rust renderer. Kontroluje
oba řezy očí, rozdílný stereo obraz, near clipping, adaptér, velikost canvasu,
GPU validační chyby, exit/reentry a disposal. Skutečný Quest tím ověřen není.

Výchozí rozlišení vrstvy je 75 % doporučeného. Grip ovladače a trigger jsou
podporované. Ruce bez ovladačů, teleportace a AR/passthrough nejsou součástí ukázky.
Desktopový test s náhradou XR rozhraní ověřuje stereo pixely a životní cyklus,
ale nepotvrzuje tracking, pohodlí, výkon ani skutečný výstup v headsetu.

Specifikace: [WebXR Device API](https://www.w3.org/TR/webxr/),
[Meta WebXR overview](https://developers.meta.com/vr/documentation/web/webxr-overview/).

## Jablka, meč a kontrola VR

Samostatná ukázka `examples/webxr/` obsahuje vlastní geometrii jablek,
dvě katany a dvě odpalovací místa u země. V XR se automaticky vystřelí jablko každé
2 sekundy během 30sekundového kola, levý trigger vystřelí další. V náhledu funguje tlačítko **Vystřelit jablko**.
Na každý grip je navázaná katana; oběma rukama lze sekat včetně obloukového
švihu zápěstím. Trigger se v této ukázce držet nemusí. Výchozí ovladače odpovídají WebXR handedness `left`/`right`,
polohy se berou z `gripSpace`, nikoli ze zaměřovacího paprsku.
Čepel vede podél gripové osy −Z, její plochá strana má normálu X (směr hřbetu
pravé ruky). Model je vůči původní geometrii otočený o 90° kolem rukojeti;
řezné konce zůstávají na stejné ose jako viditelná čepel.

```js
engine.xr.bindController('right', sword, {cutting: true, requireTrigger: false}); // sword.setCutter(...) předem
engine.xr.bindController('left', leftSword, {cutting: true, requireTrigger: false});
engine.xr.addEventListener('select', event => {
    if (event.detail.handedness === 'left') shootApple();
});
engine.xr.addEventListener('cut', event => {
    // detail: skutečné odstraněné IDs a handles nových kusů, plus cutting statistiky.
    console.log(event.detail.removed, event.detail.pieces);
});
// engine.xr.bindController('right', null); // odpojit vazbu
```

Ve VR jsou dvě tabulky: **SCENA** (FPS, čas snímku, CPU, objekty, draw calls)
a **SKORE** (počet rozkrojených plodů, zbývající čas a stav kola). Kolo trvá
30 sekund. Čas i skóre spravuje Rust, počítá úspěšná rozdělení (včetně dalšího dělení kusů), nikoli počet vzniklých fragmentů.
Po konci se střelba i skóre zastaví. Do prostorového tlačítka **DALSI KOLO / MECEM RESTART** udeř libovolným
mečem. Kontakt funguje i při rychlém švihu bez triggeru; při držení meče uvnitř
se restart neopakuje. Funguje také namíření ovladačem a stisk triggeru.
Zaměřovací bod potvrzuje zásah paprsku.
Stejné tlačítko umožňuje restart i během kola; skóre se vynuluje.
Tabule jsou skutečné mesh objekty v metrech, ukotvené ve scéně před hráčem.
Text má stereoskopickou hloubku, reaguje na pohyb hlavy a zakrývají jej bližší
objekty. Původní obrazový HUD je v této ukázce vypnutý.

Rust zpracovává transformaci gripu, pohyb meče, geometrii řezů a balistiku.
Řezání zůstává CPU. U fyzické čepele se počítá podíl skutečné plochy průřezu,
kterou prošlo ostří: od 75 % se ovoce rozdělí, mezi 50 a 75 % se zaklíní
na čepeli a mělký ukončený zásah pod 50 % se odrazí. Pokračující švih má čas
proniknout hlouběji; zaklíněné ovoce lze dalším pohybem doříznout nebo uvolnit.
Drobné chvění ruky ani otočení zápěstí zaklínění neruší; pro uvolnění
je potřeba ostří souvisle vytáhnout přibližně o 2 cm. Rychlý švih, který
rovnou přesáhne 75 %, ovoce ihned rozdělí bez zastávky.
Obecné řezání bez fyziky čepele dál vyžaduje celý průřez. Řezný pás je sjednocení konečných
plošek mezi skutečnými vzorky čepele, nikoli nekonečná řezná rovina nebo plná
konvexní obálka. Rotace zápěstí je podporovaná. Stále jde o přibližně planární
řez: výrazně prostorově zkroucený pohyb se nespojuje do falešného řezu.
Translace cíle se započítává; rotace povrchu během starších vzorků není přesná
spojitá kolize. Po dosažení prahu vzniknou dvě uzavřené skutečné části původního
meshe. `meshInfo.cutProgress` zpřístupňuje pokrytí 0–1 a `jammedBy` ID čepele
(případně `null`). Výpadek trackingu,
teleport a prudká změna směru začínají nový tah.
Rychlost švihu se kontroluje podle času XR snímku, nikoli pevným úhlem na
snímek. Mezi platnými polohami se grip lineárně interpoluje a rotace vzorkuje
po malých krocích pomocí quaternion slerp; současně se interpoluje translace ovoce.
Tím se kontroluje i zakřivená dráha špičky při rychlém švihu a vynechaném snímku.
Výpadek delší než 50 ms záměrně přeruší tah, aby se přes něj neodhadoval zásah.
Pohyb ovoce se vyhodnocuje i proti stojící čepeli. Řezná rovina se odhaduje
z plochy celého nasbíraného pásu, aby malé pootočení zápěstí nezahodilo přímý švih.
Řez začíná úzkou spárou v původním povrchu. Každá půlka má vlastní těžiště,
dědí pohyb a rotaci ovoce a plynule se rozevírá podle směru řezu.
Sourozenecké půlky se navzájem neodstrkují nepřesnými kulovými kolizními
obaly původního celého ovoce. Fragmenty padají a po 4 sekundách zmizí.
Oběma meči lze dál řezat půlky, čtvrtky a menší části v různých rovinách.
Tentýž průchod čepele nespouští opakované dělení nově vzniklého kusu; druhý
meč může navázat dalším skutečným zásahem. Drobné zbytky se už dále nedělí
a úklid i limity počtu částí brání zahlcení scény. Potomci sdílejí původní
čtyřsekundovou dobu do odstranění, takže další řez životnost neprodlužuje. Volitelná `createScene(scene, {practice:true})` přidá
stolek a dvě stojící jablka pro izolovaný regresní test.

## Přehratelná simulace bez headsetu

Otevři [VR simulaci](../tests/webxr-simulation.html?backend=webgl&slow=1).
Spouští přímo produkční `examples/webxr/main.js`; nahrazené jsou pouze WebXR
zařízení a hodiny intervalů. Jablka automaticky vystřelují, skutečný Rust počítá
jejich let/spin. Virtuální pravý grip provede 70° oblouk bez stisku triggeru.
Scénář obsahuje švih vedle, dva zásahy pohybujících se jablek a výpadek trackingu.
Testuje také řez oběma rukama bez triggerů a pixely informačního panelu v obou očích.
Rychlé řezy a konečný dosah ověřuje na pohybujícím se rotujícím jablku při
72/90/120 Hz a s rozestupem 36 Hz; dlouhý rychlý oblouk kontroluje pro obě ruce.
Stránka ukáže stereosnímky před zásahem a po něm. Test kontroluje skutečné
uzavřené fragmenty, zachování objemu a zastavení střelby po ukončení/reentry.
Volba pomalého přehrávání jen zpomaluje zobrazení; simulované kroky jsou 72 Hz.
Tyto hodnoty nejsou naměřená FPS Questu.

Spuštění přes udržovaný runner:

```sh
npm run verify:browser -- --page webxr-simulation,webxr,webxr-preview --backend webgl --capture
```

Automatická kontrola zahrnuje:

- `cargo test xr_input --lib`: skutečný vícesnímkový split, objem, orientaci,
  měřítko, gravitaci/zděděnou rychlost, úklid, ztrátu/rebind/odstranění objektu.
- `webxr` v udržovaném browser runneru: skutečné stereo pixely a paralaxu,
  produkční geometrii jablka, rozdělení přes emulované grip snímky, uzavřenost a
  zachování objemu, Rust let jablka, stisk triggeru, obnovení trackingu a úklid.
- `webxr-preview`: načtení vlastní ukázky, skutečné pixely a tlačítkový výstřel.
  `lifecycle` kontroluje navazující dispose/reinit; původní `cutting` regresi řezání.

Přejímka přímo na Questu (automatická náhrada XR API ji nepotvrzuje):

1. Otevři lokální HTTPS a vstup do VR. Podlaha musí odpovídat skutečné podlaze,
   obraz při otáčení a pohybu hlavy musí držet polohu, oči nesmí být prohozené.
2. Ověř měřítko a polohu katan v obou rukou. Přímý i obloukový švih přes
   letící jablko bez držení triggeru vytvoří dva skutečné kusy s vnitřními plochami.
3. Levý trigger mimo tlačítko vystřelí jablko ze země, také běží automatická střelba.
   Vyzkoušej zásah letícího jablka, pád kusů a jejich zmizení.
4. Zakryj/odlož ovladač, obnov tracking a vystup/vstup znovu. Návrat nesmí
   způsobit falešný řez ani zdvojit interval vystřelování.
5. **Informace o scéně v obrazu VR** jsou zapnuté ve výchozím stavu. Sleduj plynulost při
   několika jablkách a samotném řezu, zaznamenej poklesy a teplotu/délku běhu.
   Desktopová FPS nelze vydávat za výkon Questu.

Obecné testování dalších VR her, reporty a adaptéry: [VR_TESTING.md](VR_TESTING.md).

Volitelný časovač pro další hry: `engine.xr.startRound(30)` v aktivní XR session;
`engine.xr.roundState` vrací čas a skóre, událost `roundend` přijde jednou při nule.
Bez spuštění kola zůstává obecné XR řezání bez časového omezení.

Prostorové tabule vytvoří přímo scéna: `scene.createInfoPanel('scene', {position:[-.68,2.05,-2.3],width:1.2,height:.72})`
a `scene.createInfoPanel('round', {...})`. Rust spravuje textovou geometrii i obnovu
hodnot; handle podporuje běžné přesuny, rotaci, měřítko a odstranění objektu.

## Ostrá hrana a plocha meče

V ukázce je aktivní `sword.setBladePhysics({edge:[0,-1,0],halfWidth:.0225,halfThickness:.004})`.
Zlatá linka označuje jedinou ostrou hranu. Řez vyžaduje relativní pohyb ovoce
proti této hraně; tupý hřbet a plocha neřežou. Při plném průchodu stále vznikají
skutečné uzavřené mesh fragmenty. Částečný dotek sám nevytvoří dva kusy.

Meč je kinematické těleso. Ovoce reaguje odrazem, třením a gravitací, na vodorovné
ploše může zůstat podepřené a s mečem se posouvat. Při ztrátě podpory spadne.
Podpíranému ovoci se obnovuje existující čas úklidu, aby nezmizelo v ruce.
Kontakty využívají uloženou obálku ze skutečných vertexů a orientovaných bounds,
nepřepočítávají celou geometrii každý snímek. Je to omezená kontaktní aproximace,
nikoli přesná rigid-body kolize všech trojúhelníků. Řezání používá skutečný mesh.
Bez opt-in `setBladePhysics` zůstává původní obecný cutter beze změny.

Regrese obsahují ostrou/plochou/tupou stranu, rychlé průchody, pětisekundovou
podporu včetně expirace, přenášení třením, ztrátu trackingu a stisk 3D tlačítka
oběma ovladači. Každé úspěšné rozdělení přidá bod; samotné vytvoření dvou fragmentů nepřidává dva body.


## Obecný hráč: XRPlayer

`engine.xr.player` je jedna enginem vlastněná instance `XRPlayer` (v Rustu
`xr_player::XrPlayer`). Hlava a obě ruce se sledují i bez geometrie. Hráč sám
nevytváří tělo, zbraně ani objekty scény. Konkrétní hra vybavuje libovolné objekty:

```js
const player = engine.xr.player;
player.addItem(flashlight);                 // existující SceneObject
player.equip('right', flashlight);           // bez řezání
player.equip('left', sword, {cutting:true, requireTrigger:false, haptics:true});
player.unequip('right');                     // ponechá předmět v inventáři
player.removeItem(flashlight);               // odpojí a vyřadí; nemaže geometrii
const {head, leftHand, rightHand, inventory} = player.state;
```

Snapshot obsahuje `tracked`, polohu, quaternion, matici, rychlost gripu v m/s
(z posunu a skutečného času) a volitelný připojený objekt. Rychlost hlavy je zatím
0. Při ztrátě trackingu jsou pozice/matice `null`, rychlost 0 a připojené objekty
skryté. Opuštění XR zachová inventář i vybavení; `engine.dispose()` zneplatní SDK.
Snapshot je diagnostika na vyžádání, nikoli další JS simulační smyčka.

`player.bindHead(object)` volitelně připojí existující objekt k hlavě. Měřítko
objektů zůstává zachované. Jeden objekt smí ovládat jen jeden slot. Inventář má
limit 64 živých objektů a předměty se automaticky registrují při vybavení.
`bindController` zůstává kompatibilní zkratka přes tentýž nativní hráčský stav.
Neon aréna vybavuje své meče v `examples/neon-arena/main.js`; základní hráč nemá
žádné znalosti o ovoci nebo zbraních. Hand tracking prstů ani fyzický avatar těla
zatím nejsou součástí této abstrakce.

## Potvrzený postup a diagnostika na Questu

Uživatel 2026-10-18 potvrdil skutečné WebGPU VR v neon aréně na Questu 3
s Browser 149.1. V lokálním logu po opravě následovaly `xr-start` a
`xr-first-frame` s `renderer:BrowserWebGpu`. Nejde o měření FPS ani barevné shody.

V tomto buildu byl nutný **WebXR Experiments** v `chrome://flags` a restart
prohlížeče. Před zapnutím nebyl `XRGPUBinding` dostupný, přestože běžné WebGPU,
VR i projekční vrstvy fungovaly. Názvy dalších přepínačů Layers se liší mezi
buildy; samotná aktualizace, seznam flagů ani existence bindingu nejsou zárukou
funkční relace. Starší Browser 144.2 vracel na povinnou feature `webgpu`
`NotSupportedError`. Feature se nesmí odebrat jen proto, aby požadavek prošel.
[Meta dokumentuje WebXR Experiments pro experimentální WebGPU vrstvy](https://developers.meta.com/vr/downloads/package/browser/152.1/?view=full_width);
tento návod pro novější build není automaticky popisem každé starší verze.

Volitelná diagnostika arény zapisuje přes místní `scripts/serve-local.py` do
ignorovaného `output/diagnostics/browser.jsonl`. `xr-capabilities` oddělí chybějící
API od nedostupného immersive VR; `xr-enter`/`xr-frame` obsahují chybu a při
selhání importu i metadata obou příloh. `xr-start` znamená vytvořenou relaci,
`xr-first-frame` úspěšně odeslaný první stereo snímek. **Diagnostika WebGPU VR**
zkusí nativní relaci a projekční vrstvu nezávisle na enginu a relaci pak ukončí.
Odmítnutí oprávnění zůstává opakovatelné, nepodporovaná GPU relace se v dané
instanci dál nenabízí.

### Quest 149: nulová metadata barevné projekční textury

Na skutečném Questu 3 / Browser 149.1 po zapnutí WebXR Experiments vznikla
GPU relace, ale první snímek selhal na `Invalid XR projection attachment`.
Barevná textura měla platné rozměry 1260×1320×2 a formát `rgba8unorm`, ale
`mipLevelCount=0`, `sampleCount=0`, `usage=0`. Hloubková textura vracela 1/1/16.
Adaptér proto pouze u barevné XR textury s touto přesnou trojicí nahradí
nuly vlastnostmi explicitně vytvořené projekční vrstvy: jeden mip, jeden sample,
RENDER_ATTACHMENT. Ostatní kontroly i hloubková textura zůstávají přísné.
Browser test `webxr-webgpu` simuluje stejné nulové gettery na skutečné GPU
textuře a čte oba vykreslené řezy. Po této opravě uživatel potvrdil funkční obraz na Questu;
změny rendereru vyžadují vlastní následné ověření.

Při `ignoreDepthValues === true` kompozitor hloubku nečte; postprocess vynechá její
přenos do externího attachmentu (interní hloubka pro SSR a overlays zůstává).
Chybějící nebo false hodnota zachová depth resolve. XR host po zahřátí pošle
nejvýše tři události `performance`: skutečnou kadenci XR callbacků, průměrný
CPU submission čas a rozměry očí. Nejde o GPU timestamp ani čas kompozitoru.

### Quest Browser 152.1: dokončené stereo před kompozitorem

Uživatel po aktualizaci hlásí artefakty pouze se SSR; nativní depth a odstranění
packed MRT je neodstranily. Log potvrzuje načtený aktuální renderer, platné
slices a `ignoreDepthValues:true`; konkrétní browserová regrese není potvrzená.
Proto při zapnutém postprocessingu engine dokončí každé oko do vlastního
trvalého barevného/depth cíle a teprve potom jediným závěrečným `queue.submit`
vykreslí obě browserové slices. Efekty a overlays už nekombinují browserovou
barvu s interní hloubkou. Finální kopie používá `textureLoad` bez změny barev,
bez CPU readbacku a bez požadavku COPY_DST. Ignorovanou browserovou hloubku
ani nepřipojuje; požadovanou přenáší z dokončené per-eye hloubky.

Přímá cesta bez efektů zůstává zachovaná. Dva vlastní per-eye cíle přidávají
paměť a jeden jednoduchý průchod na oko; jde o workaround předání obrazu,
nikoli prokázané zrychlení. Uživatel 2026-10-19 potvrdil, že tato varianta
odstranila artefakty SSR na Questu 3 / Browser 152.1. Přesný mechanismus chyby
prohlížeče ani výkonová shoda WebGPU/WebGL tím potvrzené nejsou. `xr-first-frame`
obsahuje `presentation:stereo-final-submit-v1`, aréna hlásí
`revision:xr-stereo-present-1`. GPU test sleduje skutečné browserové attachments
a příkazy: oba slices musí dostat jediný finální stereo submit, přitom stále
procházejí SSR/glow/stereo/depth testy a žádné stabilní alokace GPU prostředků.
Pravidelný test používá i nečtvercové oko 192×208.

Podklad: [WebXR WebGPU binding – synchronizace posledního submitu](https://github.com/immersive-web/webxr-webgpu-binding/blob/main/index.bs).
Více submitů je podle specifikace platných; tento workaround omezuje aplikační
závislost na této cestě, sám o sobě nedokazuje chybu prohlížeče.
