# Ovocná aréna s neonem a lesklou podlahou

Aktuální hra používá [dva rostoucí zásobníky](#dva-zásobníky-prototyp-rostoucích-sérií).
Starší popisy automatických trojic a historie níže popisují zachovaný SDK režim
`legacy`; nejsou pravidly výchozí arény.

## Automatické srovnání odrazů v brýlích

Před vstupem do VR zaškrtni **Srovnávací test před hrou**. Místo hry vznikne
stejná nehybná sada šesti plodů nad podlahou; simulace se pozastaví a automatické
vystřelování neběží. Dívej se stále na odrazy před sebou a nehýbej ovladači.
Test projde bez efektů → glow → SSR 50 % → SSR 100 % → SSR 50 % + glow → bez efektů.
Procenta znamenají poměr šířky i výšky SSR textury, nikoli hlavního obrazu.

Každá fáze zahodí dvě telemetrická okna a změří další tři (180 snímků v okně).
FPS a CPU ms vážíme počtem snímků/dobou; medián a minimum jsou statistiky **oken**,
nikoli 1% low nebo per-frame percentily. Po výstupu z VR uvidíš tabulku a stáhneš
JSON. Stejný souhrn se jednou odešle místnímu `/__diagnostics` jako
`xr-benchmark-result`. Neodesíláme další data za každý snímek.
Poslední směr pohledu každého okna je zaznamenán, pohyb hlavy není uzamčený.

Test trvá zhruba 1–2 minuty; původní SSR/glow obnoví i při předčasném ukončení.
Po třech minutách bez dostatečných vzorků skončí s částečnými výsledky.
Změna XR relace či rozlišení měření zruší. Srovnej zvlášť WebGPU a WebGL,
se stejným pohledem, stavem profilování a po podobném zahřátí zařízení.
Základ zopakovaný na konci ukazuje možný časový drift; neurčuje jeho příčinu.
Pevná scéna izoluje náklady renderování a nenahrazuje FPS celé dynamické hry.

Řadič je volitelný diagnostický nástroj v `performance-test.js`, ne herní simulace
v SDK. `tests/neon-arena-benchmark.html` ověřuje přepínání skutečného SSR,
statistiky ze syntetických paketů, obnovu nastavení a úklid v simulovaném XR.
Nejde o naměřený výkon Questu.

Aktuální ukázka používá čisté černé pozadí. Obsahuje devět druhů ovoce včetně dvojice třešní, banánu, hroznů,
pomeranče a limetky; dvě katany,
60sekundové kolo a skutečné Rust řezy. Přímo před hráčem je velký odpočet z neonových trubic a pod ním skóre. Nízký žulový náhrobek vpravo mimo prostor galerie nese kredit CODEX a živou
diagnostiku FPS, FRAME MS, CPU MS, CULL MS, VISIBLE, CULLED, OBJECTS a DRAWS.

Aplikace neimportuje `environment.js`, nestahuje barevné/emisní stěny cubemapy,
má ale skutečnou kachličkovou podlahu se zapnutým SSR (`strength:.92`).
Osmdesát tmavých kachliček se zkosenými hranami má reflektivitu .94, oddělují je
matné spáry. Podlaha pokrývá 6 × 7,5 m na Y=0; její geometrie je jediný statický mesh.
Odrazy používají aktuální barvu a hloubku každého oka, takže reagují na pohyb ovoce.
Objekty mimo obraz mohou v SSR chybět. Bloom zůstává vypnutý; postprocessing má
dva SSR průchody: trace v poloviční šířce/výšce a depth-aware composite v plném
rozlišení. `configureSSR({resolutionScale:1})` umožňuje porovnání s původní kvalitou;
`.5` je nastavení arény. Ovoce a texty zůstávají v původním rozlišení.
SSR je omezeno na prostředních 20 kachliček (3 × 3,75 m z celkových 6 × 7,5 m).
Každé oko má vlastní ořez výpočtu. Mimo odraznou oblast zůstávají kachličky matné;
při pohledu mimo ni se celý SSR řetězec přeskočí. Hlavní scéna se tím nezmenšuje.
Původní obrazové podklady a offline
build skripty zůstávají uložené v příkladu, ale herní stránka je nepoužívá.

## Neonová záře

Aréna zapíná `engine.setNeonGlow({strength:.4,radius:.045})`. Rust vykresluje
souvislé neonové jádro s měkkým barevným lemem podél hranic svítících pásků
a ostrých hran ovoce a dalších emissive triangle meshes. Koplanární diagonály
triangulace vynechá. Maximum RGB místo sčítání brání světlým boulím ve vrcholech.
Původní mesh zůstává viditelný uvnitř záře. Záře používá
stejné modelové transformace, vzniká i u nových rozříznutých kusů a černé capy
se do ní nezahrnují. Vyšší emise zasažených symbolů zesílí i jejich glow.

Shader nečte žádné textury a při vypnutém postprocessingu sdílí existující color
pass i hloubku pro desktop, WebGL XR a přímé WebGPU XR. Nevyžaduje nové barevné
mezicíle. Jde o geometrickou aproximaci glow, nikoli HDR bloom celého obrazu;
glTF materiály, odrazy a environment probe ji automaticky nepřebírají.
Síla 0–2 a poloměr 0,001–0,05 m jsou příkazy SDK, bez vlastního JS RAF.
`strength:0` uvolní glow buffery a pipelines. Buffery vznikají jednou pro stabilní
ID meshe, cache odstraní zaniklé kusy. `engine.debugStats().neonGlow` ukazuje
aktuální draw calls, segmenty, počet cache položek a bytes bufferů.

Inspirace: [ARM – mobilní glow a alternativy k postprocessingu](https://developer.arm.com/community/arm-community-blogs/b/mobile-graphics-and-gaming-blog/posts/post-processing-effects-on-mobile-optimization-and-alternatives)
a [GPU Gems 2 – předfiltrované čáry](https://developer.nvidia.com/gpugems/gpugems2/part-iii-high-quality-rendering/chapter-22-fast-prefiltered-lines).
Spoje a překryvy čar popisuje také [Matt DesLauriers – Drawing Lines is Hard](https://mattdesl.svbtle.com/drawing-lines-is-hard).
WGSL implementace je vlastní; další segmenty, draw calls a overdraw nejsou zdarma.
Profil `neon-arena-profile` porovnává stejnou šestici reálných cílů s glow a bez něj.
Jde o desktopový stereo profil s readbackem, nikoli benchmark Questu.

Devět druhů ovoce a žolík tvoří deset řádků pod sebou. Každý řádek obsahuje
tři stejné rotující 3D symboly vedle sebe a cenu dokončené trojice. První zásah
rozsvítí, zvětší o 32 % a zlatě orámuje první symbol, druhý druhý a třetí celou trojici.
Dokončený řádek zůstane zvýrazněný do dalšího platného zásahu; jiný druh začíná
zvýrazněním prvního symbolu svého řádku. Fragmenty zvýraznění nemění, restart
vše zhasne. Žolíky na začátku ukazuje řádek žolíka; po určení druhu se postup
přenese na dané ovoce. Rotaci a zvýraznění přes `bindComboIndicator(kind, slot)`
(slot 0–2) vlastní Rust, JS pouze vytvoří a naváže objekty.
Celá výherní tabulka, její rámečky a hodnoty sdílejí sklon −18° kolem X
a otočení +90° kolem Y. Její střed je vlevo na [−2,75;1,2;−0,4]; při pohledu
dopředu je mimo zorné pole, zobrazí se při otočení hlavy doleva.
Získané symboly i jejich samostatné zlaté rámečky používají emisi 8,0 místo
klidové 0,65; zvýraznění stále řídí skutečný postup série v Rustu.
Galerie je viditelná v obou očích po otočení doleva; diagnostický kámen je vpravo
na X=3 m, aby ji nezakrýval. Restart zůstává před hráčem na [0;0,20;−1,45].
Výchozí nástroj nyní posílá až deset plodů najednou každé 3 s; viz Ovocné vlny níže. Rust vlastní fyziku, řezy,
čas i skóre. Objekt DALSI KOLO lze aktivovat mečem nebo namířením a triggerem.
Při odchodu z XR se výstřely zastaví a náhledové ovoce se vrátí.

## Přední ukazatel a historie

`countdown` a `score` jsou centrované číslice z oblých 3D trubic bez podkladové desky.
Oblé křivky mají světlé bílo-modré jádro, modré boky a měkké modré halo.
Technické tvary se zkosenými rohy a rozdělenými tahy ukazuje samostatná
[sada 0–9](../neon-font-preview/), včetně ovládání glow a pohledu zešikma.
Trubice mají střední tloušťku a vlastní neonový shader: analytický světelný profil podle normály a pohledu, modré okraje a jasné jádro s plynulým převodem HDR. Znaky jsou lehce nakloněné doprava. Používají rozšířené sci-fi
proporce; rozestupy a fitting vycházejí ze skutečných mezí glyphů, aby zůstaly
oddělené i v dlouhých skóre. Čas má větší výšku a intenzitu než širší skóre,
aby delší bodové částky nezabíraly celý ukazatel. Rámečky symbolů
sdílejí jednu geometrii kruhové neonové trubice se zaoblenými rohy; jejich
měřítko je jednotné v každé ose.
Principy světelného profilu: [Neon Light Shader](https://shaders.orels.sh/docs/orl-standard/neon-light) a [Three-layer neon glow](https://andrewdetwiler.com/sdf/notes/three-layer-glow).
Vizuální reference: [skutečné skleněné neony AOOS](https://www.aoos.com/products/custom-real-glass-neon-signs).
Číslice 0–9 mají jednou připravenou 3D geometrii a GPU buffery. Odpočet a skóre
je skládají ze sdílených instancí; opakované nuly nekopírují mesh. Při změně čísla
se mění sestava a rozmístění znaků, nikoli GPU geometrie. CPU snapshot panelu
zůstává dostupný pro diagnostiku.
Materiál `-2` je vyhrazený numerickým trubicím (`-1` je běžný bitmapový text).
Jejich kanonický vertex RGB nese zakódovanou jednotkovou osu segmentu, nikoliv
barvu. Vertex shader ji převede do světa, fragment shader promítne pohled do
průřezu trubice. Tím jádro nehasne při pohledu podél tahu; CPU snapshot panelu
nadále vrací modrou diagnostickou barvu. Záře používá stávající průchod a šířku
odvozenou ze skutečného poloměru instance, bez nového full-screen efektu.
Souvislé tahy navazují přes přesné kruhové oblouky s analytickými tečnami.
Samostatné části číslic mají skutečné mezery, aby se světelný profil ve spojích
nelámal přes překrývající se koncové kapsle.
Výchozí odpočet je 30 s; skutečný Rust round clock postupuje jednou za platformní
snímek, nikoli jednou na oko. Vlastní font nevyžaduje síťové načítání.

`engine.xr.configureComboDisplay(world.comboDisplay)` naváže registrované sdílené
assety symbolů a rámečku, timer/score panely a tři panely odměn. Rust vytváří pouze
aktuální trojici na [0;1,42;−2,15] a nejvýše tři historické trojice pod ní. V každém řádku jsou tři širší modré rámečky. Ovoce vzniká pouze pro získané
sloty; zbývající místa jsou prázdná, bez tlumených kopií ovoce. Jiný druh přesune
přerušenou trojici do historie, úspěch ji doplní o `+body`. Historie a její záře
plynule slábnou 2 s, pak Rust odstraní instance. Žolík zachovává pravidla skórování.
Restart maže historii i skóre, `scene.clear()` ruší binding.

SDK drží pouze handles/config; žádný další JS RAF nebo polling skóre. Stav pro
cílené testy je `engine.xr.comboDisplayState`, veřejné nastavení je popsáno v
`js/engine.d.ts`. `src/combo_display.rs` vlastní životnost a fade, `src/xr_round.rs`
události úspěchu/přerušení, `src/info_panel.rs` trubicové číslice a zbývající bitmapové texty. Po vypnutí/zapnutí
glow se fontová cache obnoví bez přegenerování stejného textu každým snímkem.

## Trojice a skóre

Body přidá až třetí po sobě rozkrojený plod stejného druhu. Jiný druh začne
novou sérii 1/3. Po vyplacení trojice se série vrátí na 0/3; šest zásahů
stejného druhu tedy vyplatí dvě trojice. Minutí série nepřerušuje, fragmenty
nepřidávají body. Restart vymaže body i sérii, po konci kola body nepřibývají.

Původní katalog jednotlivých výstřelů má `[ovoce, ovoce, wild, ovoce, ovoce, ovoce]`.
Nový výchozí nástroj Ovocné vlny jej nahrazuje současnými formacemi s mixem po trojicích.
Žolík má vlastní uzavřený neonový mesh třícípé šaškovské koruny a lze ho skutečně
rozkrojit. `setComboTarget(kind, points, {wild:true})` jej označí jako náhradu
jakéhokoliv druhu: ovoce–wild–ovoce i wild–wild–ovoce vyplatí cenu ovoce.
Tři wild vyplatí 120 bodů; po výplatě další trojice začíná samostatně.

Plody mají větší herní měřítko (scale 0,09), startují v `[±0,38;1,15;−1,10]`, mají pomalý
oblouk (vy=1,2 m/s, gravitace 1,6 m/s²), mírně se přibližují a po 1,9 s zmizí
bez odrazů. V dosahu 1,10–1,65 m na výšku zůstávají přes sekundu; Původní režim jednotlivého výstřelu je zachován pro desktopovou ukázku;
nové vlny používají vlastní rozmístění, trajektorie a limit deseti cílů. Jde o návrh hratelnosti,
ne o potvrzený komfort na skutečném headsetu.

Prostřední displej ukazuje body, druh ovoce, postup série a čas; pod ním je
zvýrazněná 3D galerie s hodnotami za dokončenou trojici:

| Ovoce | Body |
|---|---:|
| Třešeň | 10 |
| Limetka | 15 |
| Citron | 20 |
| Pomeranč | 25 |
| Banán | 40 |
| Švestka | 50 |
| Hruška | 60 |
| Hrozny | 100 |
| Meloun | 120 |
| Tři žolíci | 120 |

Jde o vlastní bodovou adaptaci. [Oficiální demo Midnight Fruits 81](https://wwsim0-tst.apollogames.com/casino-games/?backUrl=https%3A%2F%2Fwww.apollogames.com&currency=EUR&gameMode=Demo&gameName=midnightfruits&language=en)
vyplácí za **čtyři** symboly: třešně/citron/pomeranč 1× sázku,
banán/švestka 2×, hrozny/meloun 10×. Zachováváme pořadí těchto skupin,
ale hodnoty rozlišujeme pro každý druh a přidáváme limetku a hrušku.
Konfigurace je `FRUIT_POINTS` v `scene.js`. `setComboTarget(kind, points)`
je opt-in příkaz SDK; evidence původních cílů, postup série a výpočet bodů
zůstávají v Rustu. Restart zachová metadata stále živých cílů a odstraní registrace zaniklých objektů.

Ovoce má skutečně průhledné mezery mezi neonovými hranami. Příkaz
`setColorCutout(.25)` zahodí tmavé plochy před zápisem hloubky; světlé hrany
se kreslí z obou stran. Nejde o černou výplň ani o celoplošný postprocessing.
Uzavřený objem pro fyziku a řezy zůstává zachovaný. Řezné plochy mají barvu
`[0,0,0]`, takže je stejný cutout úplně zahodí i ze zápisu hloubky: půlky
neonového ovoce jsou vizuálně otevřené, bez výplně. Vnější neonový plášť
zachovává průhledné mezery.

Barvy jsou zapečené do trojúhelníků při vytvoření geometrie: třešně mají
červený/růžový plod a plné zelené neonové stopky, ostatní plody vlastní prostorové
akcenty. Nevyžadují další objekty, textury, materiálové průchody ani aktualizaci
barev každý snímek. Hustota katalogu je 5 884 trojúhelníků místo 9 688 (−39 %),
uzavřené objemy i řezy všech devíti druhů zůstávají ověřené. Profil používá šest
různých skutečných neonových cílů včetně třešní a hroznů; snížení počtu trojúhelníků
samo o sobě není měření vyššího FPS na Questu.

Sestava zahrnuje všech sedm ovocných symbolů Midnight Fruits 81: třešně,
limetku, banán, pomeranč, švestku, hrozny a meloun; navíc ponechává citron a hrušku.
[Herní plán Apollo](https://gapagroup.cz/_download/MF_30205_2017_34_110.pdf).
Nové kolo začíná třešněmi. Každý druh má vlastní uzavřenou geometrii;
banán je prohnutý, dvojice třešní má stopky a hrozny tvoří šest bobulí.

Obě katany používají `setCutAssist(.35)`: stačí skutečně přejet alespoň 35 %
průřezu, a to i opačnou stranou čepele. Lehčí režim nepřichytává ovoce na ostří. `setBladeContactResponse(true)`
přidává kontakt plochy/hřbetu i zachycení příchozího ovoce drženou čepelí;
oba směry tahu podél ostří zůstávají řezatelné. Přenos rychlosti je omezený
na 8 m/s, aby tracking nemohl vystřelit plod nepřiměřenou rychlostí. Krájení stále vyžaduje konečný pohyb přes skutečný mesh;
chybějící tracking a skoky pozice se nepočítají jako tah. Rozkrojené kusy
už další zásah nepřijímají, pouze odletí a uklidí je Rust. Výchozí režim enginu
zůstává beze změny; `setCutAssist(0)` asistenci vypne.

Při zásahu příklad explicitně volá `scene.playCutFeedback(pieces)`: prostorový
neonový kruh, šest jisker a bitmapový nápis CISTY REZ. Kruh používá průhlednou
RGBA texturu a rovinu skutečného řezu. Nápis je nad horní mezí plodu; ukotvení
se počítá ze skutečných transformovaných fragmentů. Efekt je ukotvený v místě zásahu, nikoli k hlavě. Při zásahu více plodů
se jejich fragmenty seskupují podle skutečného původního plodu; kruhy
nevznikají v průměrném bodě mezi dvěma zásahy. Rust používá dva opakovaně
využitelné sloty po osmi objektech; efekt zhasne za 0,65 s. Skutečné uzavřené kusy
si ponechají příchozí rychlost, dostanou impuls podél normály řezu a natočí
řezné plochy k hráči. Jejich posun už integruje běžná projectile fyzika
s gravitací 9,81 m/s², odrazem 0,45 a kontakty s ovocem i mečem;
po 1,5 s se odstraní. Opětovné použití světelného efektu nepřeruší jejich fyzikální let. `clearCutFeedback()` uklízí
při restartu a odchodu z XR. Desktopové tlačítko **Ukázat řez** provede skutečný
řez melounem a stejný efekt. `mirrorText` opravuje orientaci nápisu v desktopovém
náhledu oproti XR.

Obě katany mají `setBladeTrail(true)`: tři krátké neonové stopy z historie
skutečné čepele, maximálně šest objektů pro obě ruce. Stopa dohasne za 60 ms;
ztráta trackingu ji okamžitě skryje. Opt-in `haptics:true` u ovladače přidává
krátký impuls při řezu a slabší při začátku kontaktu; držený kontakt
nevibruje stále. Celé plody používají pomalejší oblouk uvedený výše. Efekty i odlet počítá Rust, bez JS RAF,
celoplošného rozmazání, bloom, SSR nebo dalších světel.

## Spuštění a kontroly

- Náhled: `/examples/neon-arena/`, automatický WebGPU.
- Quest: `/examples/neon-arena/?backend=auto` pro experimentální přímý WebGPU XR; při nedostupném `XRGPUBinding` použij `?backend=webgl`. Obojí na důvěryhodném HTTPS; [připojení](../../docs/WEBXR.md).
- `VERIFY_WASM_PROFILE=release node scripts/test-browser.mjs --page neon-arena,neon-arena-xr,neon-arena-profile --backend all --capture`
- Profil používá skutečnou aplikaci a stereo renderer1024×1024 na oko s odečtem
  pixelů; jde o desktopový diagnostický test, nikoli výkon skutečného Questu.

Bitmapové fonty mají větší titulky/skóre, zarovnané zlaté/cyanové řádky a sloučené
tahy. `scene.createTextPanel({title,lines,position,width,height})` vytvoří statický
nápis bez pozadí. `scene.createInfoPanel('diagnostics',options)` obnovuje hodnoty
nejvýše čtyřikrát za sekundu a přestavuje mesh jen při změně obsahu. Font5×7
obsahuje A–Z a číslice, texty příkladu používají zápis bez diakritiky.

Modely vytvoří `node examples/neon-arena/build-monument.mjs`; materiál žuly je
`assets/granite.png`, vygenerovaný vestavěným ImageGen. [Původ assetů](assets/ASSETS.md).

Náhrobek stojí ve VR vpravo na [2.2,0,-4.4] m a má výšku přibližně1.46 m. Diagnostika je přibližně0.66 m nad zemí; texty sledují společné natočení a měřítko kamene.


## Rychlé ladění zásahu

`?backend=webgl&hitSize=0.15` nastavuje velikost lehkého zásahu. Například
`hitSize=0.08` jej dále zmenší; rozsah je 0,01 až 2. Silný útok si ponechává
plnou velikost. Výchozí hodnota je 0,15. Změna odkazu nebo `CUT_EFFECT_SIZE`
nepotřebuje nový WASM build. Velikost roste kvadraticky podle posunu gripu
ovladače / skutečného času: minimum do 0,75 m/s, maximum od 3 m/s. Krátká historie
omezuje třes; otáčení špičky samo rychlost ruky nenavyšuje.

`engine.xr.player` je obecný hráč s inventářem. Aréna mu explicitně vybavuje
obě katany přes `player.equip`; model hráče ani jiné zbraně automaticky nevytváří.


## Nástroj Ovocné vlny: člověk i AI

Panel **Ovocné vlny** před VR nastavuje 1–10 současných skutečně řezatelných
plodů, pauzu 1800–6000 ms, ovoce a formaci (jedna řada, střídavé zahnuté tahy, kříž, déšť).
Výchozí nastavení je 10 menších plodů v mírně zahnutém tahu / 3000 ms.
Čtyři varianty střídají sklon nahoru/dolů i prohnutí; další vlna tedy neopakuje
stejný vodorovný pohyb. Směr řezu se nevynucuje, hráč reaguje na skutečnou polohu cílů. Vše zůstává před hráčem:
X ±0,74 m (ostatní formace ±0,68 m), počáteční Y 1,01–1,75 m, Z přibližně −1,0 až −1,27 m.
Jednotlivé původní cíle mizí za 2,6 s; fyziku a řezy dál počítá Rust.
Další celá vlna čeká, pokud by překročila limit deseti celých plodů.
Fragmenty tento limit nespotřebovávají. Mix střídá druhy po skupinách tří,
aby šlo dál využít existující bodování; nové bonusy za jeden švih nepřidává.
Levý trigger ve VR pošle celou vlnu, pravý a levý meč dál normálně řežou.

Člověk i AI používají společný validovaný dispatcher:

```js
window.arenaTools.call('arena_configure', {count:10, formation:'arc', intervalMs:3000});
window.arenaTools.call('arena_spawn');
window.arenaTools.call('arena_state');
window.arenaTools.call('arena_stop'); // zastaví další vlny, ponechá rozehrané cíle
window.arenaTools.call('arena_start'); // nové 60s kolo, reset skóre a cílů
```

Poslání samostatné vlny funguje i v desktopovém náhledu. Start bodovaného
kola vyžaduje aktivní VR; bez něj vrátí chybu. Stav `presenting` výslovně říká,
zda běží immersive VR. Start VR vyžaduje běžné tlačítko a souhlas prohlížeče.
Srovnávací benchmark blokuje start a posílání vln.

### Skutečné MCP (stdio) a připojený Quest

Není potřeba další Python balíček. Běžný server MCP neaktivuje.
Pro Quest nejprve připrav produkci `npm run build:production`, potom spusť
volitelný server s důvěryhodným certifikátem a vlastním tajným tokenem:

```sh
python3 scripts/arena-tools.py --host 0.0.0.0 --port 8766 \
  --production --directory output/production \
  --cert /absolutni/cesta/server.crt --key /absolutni/cesta/server.key \
  --token VLASTNI_TAJNY_TOKEN
```

Otevři `https://IP_MACU:8766/examples/neon-arena/` v Questu. V panelu
**Připojit AI přes MCP** vlož token a klikni **Připojit**. Stránka příkazy
vyzvedává ze stejného HTTPS původu; nepřipojuje se k localhostu headsetu.
Token se neukládá do URL ani localStorage. Pro lokální HTTP náhled vynech
certifikát, produkční volby a `--host`; výchozí server je jen na loopbacku.

Do MCP klienta nastav command `python3` s argumenty:

```json
["/Users/Honza/WebstormProjects/rust/scripts/arena-tools.py", "--mcp",
 "--url", "https://IP_MACU:8766", "--token", "VLASTNI_TAJNY_TOKEN"]
```

Na jiném počítači uprav absolutní cestu. MCP nástroje jsou `arena_state`,
`arena_configure`, `arena_spawn`, `arena_start`, `arena_stop`. Jejich argumenty
odpovídají dispatcheru; volitelné `session` vybírá konkrétní stránku.
Při více připojených stránkách je session povinná. Bez připojené stránky
nebo při timeoutu se vrací chyba. Token musí znát server, stránka i klient;
TLS ověřuje běžný systémový trust store Pythonu. Neregistrujeme MCP automaticky
v globální konfiguraci Codexu.

Ověření: `node --test tests/arena-wave-unit.test.mjs`,
`python3 tests/arena-tools-unit.py` a udržovaný browser scénář `arena-waves`
(10 cílů v obou očích, Rust řez, životnost, ovládání a timer).
Simulované XR a produkční balíček nepotvrzují pohodlí ani FPS skutečného Questu.

Návrh formací čerpá z [rozhovoru s vývojářem Fruit Ninja VR](https://www.gameanalytics.com/blog/fruit-ninja-dev-talks-porting-to-vr)
a [diskuse hráčů na Redditu](https://www.reddit.com/r/OculusQuest/comments/bvo2fi/fruit_ninja_vr/).
Vývojář zdůrazňuje několik plodů jedním pohodlným tahem; reakce hráčů na
opakování a chaos se liší. Čtyři formace a nastavitelné pauzy jsou náš návrh,
ne prokázané univerzální optimum.


## Paměť vzhledu a vystřelování

Verzovaná [wave-preset.json](wave-preset.json) uchovává přijatou scénu:
10 menších plodů, čtyři střídavé zahnuté tahy a 60sekundové kolo.
Obsahuje množství, tempo, formaci, druhy, velikost, šířku, výšku, hloubku,
prohnutí, sklon, počáteční rychlost vzhůru, gravitaci a životnost cíle.
Parametry se načtou před spuštěním ovládání; fyziku dál integruje Rust.
Čas kola je 60 s podle `minute-scene.json`; společný Rust clock jej měří pro oba režimy.

Panel **Tvar a let ovoce** i MCP `arena_configure` upravují stejné hodnoty.
Platná změna se automaticky uloží jako celý JSON do localStorage pod klíčem
`neon-arena.wave-preset.v1`. Při příštím otevření má přednost před projektovým
JSONem. Načtení nic nevystřelí ani nespustí kolo.

Nástroje a odpovídající tlačítka:

- `arena_save`: výslovně uložit aktuální nastavení.
- `arena_load`: znovu načíst uložené nastavení.
- `arena_reset`: odstranit lokální změny a obnovit projektový JSON.
- `arena_export`: vrátit přenositelný JSON; panel nabízí jeho stažení.
- `arena_import({json:"…"})`: validovat, aplikovat a uložit celý JSON.

Například `arena_configure({curve:.15, width:.68, fruitScale:.05})` změní
prohnutí, poloviční šířku a velikost. Uplatní se u příštích cílů, nikoli
zpětně na už rozletěné plody. Jednorázové overrides `arena_spawn` se neukládají.
Verze formátu je 1; neúplné/neplatné importy zachovají původní platný stav.
Poškozené uložené nastavení zobrazí chybu a použije projektovou předvolbu,
aniž by poškozený obsah potichu přepsalo. Nedostupné úložiště nebo plný limit
nebrání nastavení pro aktuální stránku, ale vrací `saved:false` a varování;
výslovné uložení/import vrací chybu.

LocalStorage patří konkrétnímu prohlížeči **a původu včetně portu**. Desktop,
Quest, HTTP/HTTPS a jiný port mají oddělené nastavení. K přenosu použij JSON.
Uložená data neobsahují MCP token ani herní skóre či pozici rozehraného ovoce.
Uložení v prohlížeči nemění soubor v repozitáři.

Test `arena-presets` ve stejném browser runneru ověřuje skutečný reload,
obnovení parametrů, synchronizaci panelu, import souboru a reset.


## Minuta pro skóre · připravená scéna

[minute-scene.json](minute-scene.json) obsahuje konkrétní 60sekundový sled
24 výstřelů se 140 cíli. Výchozí `program:"minute"` vystřídá volbu levné
nebo hodnotné trojice, dokončení dvojice třetím plodem později mezi rušivými
cíli, malé hodnotné ovoce, pohybující se cíle se společným oknem pro řez,
rychlejší série a finále s deseti cíli. Už první čtyři výstřely střídají
diagonály, zahnutý tah a oddělený vějíř.
Předchozí zahnuté řady zůstávají v `program:"practice"` jako volné vlny.
Oba režimy trvají minutu; režim se ukládá s předvolbou v localStorage.
Starší uložené předvolby bez `program` přejdou na minutovou scénu a zachovají
velikost i ostatní hodnoty.

Ve VR se mění skutečný prostorový nadpis fáze. Časování se řídí uplynulým
časem Rust kola; jediný 100ms autorovací timer nepoužívá čas od otevření stránky.
Pozastavené/zpožděné callbacky nespouštějí zpětně shluk starých výstřelů.
Stop zastaví řadič, restart resetuje cue index i cíle, konec kola/odchod z XR
uvolní timer. Původních celých plodů zůstává nejvýš deset.

U cue s `launch:{apexSeconds,gravity}` znamená `target.position` bod pro řez
u vrcholu dráhy. `planCue` jednou odvodí počáteční polohu pod ním a svislou
rychlost; skutečný let dál integruje Rust. Minutovka používá gravitaci
2,4525 m/s² a vrchol po 0,84 s. Jde o arkádový let zpomalený dvakrát
při zachování stejné prostorové dráhy, nikoli realistickou gravitaci. Vodorovné složky `target.velocity`
řídí nálet; její Y zde nahrazuje vypočtená rychlost. Bez `launch` zůstává
původní význam pozice a rychlosti. Atmosférický odpor není implementován.
Krátké sdílené světelné výstřely jsou dekorace bez fyzikálních proxy a mizí
nativně po 0,24 s; stop/restart/odchod je také vyčistí.

Každý cíl může přepsat `scale`, `velocity` a `lifetime`. Životnost má v tomto režimu strop 2,6 s, aby delší uložená hodnota neblokovala
následující výstřely ani finále.
Šířka, výška a prohnutí z panelu řídí volné vlny. Zásahy se nesimulují ani
nepřidělují automaticky, řezy, fyzika a stávající bodování trojic zůstávají v Rustu.
Mezery a kombinace usnadňují přechod mezi stranami, ale použití konkrétní ruky
či směr řezu nejsou vynucované bodovacím pravidlem.

`arena_preview({seconds:41})` ukáže jednu část scény před VR. Bez `seconds`
postupuje k další odlišné fázi. Náhled nespouští čas kola a ve VR je blokovaný.
`arena_configure({program:"minute"})` nebo volba **Herní scéna** přepíná režim.
Levý trigger během připravené scény nevkládá další ovoce; ve volných vlnách
zůstává ruční výstřel skupiny. Samostatný `arena_spawn` je v běžící připravené
scéně blokovaný, aby nerušil choreografii.

Ověření: unit `arena-minute-unit.test.mjs` kontroluje plán, rozestupy a no-cut
kapacitu. Browser `arena-minute` projde skutečných šedesát sekund v simulovaném
XR, zachytí obě oči v šesti fázích a ověří všechny výstřely, konec, restart,
náhled i stop. Navíc skutečnými řezy sledovaným ovladačem porovnává dvě
pořadí zásahů přes bodování Rustu (110 vs. 2440). Jde o izolovanou kontrolu
bodovacích pobídek, nikoli odehraný lidský výkon v pohyblivé scéně. Pohodlí, subjektivní zábavnost a výkon skutečného Questu vyžadují
hraní na zařízení; tyto testy je nepotvrzují.

## Dva zásobníky: prototyp rostoucích sérií

Aréna používá `ComboDisplayOptions.mode:"chains"`; výchozí SDK režim zůstává
`legacy`. Skóre, dva druhové sloty a jejich čítače drží Rust. Obě ruce přidávají
ovoce podle druhu. Každý řez původního plodu přidá hned 10 jistých bodů.
Růst bonusu série se připisuje hned do hlavního skóre.
Třetí druh nahradí nejdříve založenou sérii; všechny získané body zůstanou. Další stejný plod stáří série neobnovuje. Druhá série
zůstane zachovaná. Žolík doplní kratší obsazenou sérii.

Levý/pravý trigger, SDK `engine.xr.bankChain(hand)`, panel i MCP `arena_bank`
uvolní slot; už získaný bonus se znovu nepřičítá. Méně než tři kusy mají
nulový bonus. Na konci kola se pouze uvolní zbývající sloty. Tabulka vlevo ukazuje bonus za trojici. Hodnota delších sérií roste
podle nativního `FruitChain::value`; herní nápověda tento skrytý růst neprozrazuje.
Každý nasbíraný plod přidá skutečný symbol do vodorovné řady směrem ven: čtyři/pět kusů ukážou
čtyři/pět symbolů. Sdílené meshe se znovu používají; limit je 144 symbolů na
zásobník (minuta má 140 cílů), každý symbol má vlastní sdílený rámeček. Levá řada roste doleva,
pravá doprava; velikost i rozestupy zůstávají stejné, dlouhá řada pokračuje mimo výhled. Ovoce i rámeček společně přiletí z vnější strany
(levá zleva, pravá zprava): 55 % → 135 % → 75 % → 100 % za 0,76 s,
s neonovým zábleskem celého zásobníku až při dosednutí. Prázdné sloty nemají
rámečky; získané rámečky svítí stejně jasně i v klidu po skončení animace.
Čítače jsou drobný šedý debug text
bez glow (`debug-text`, stejná bitmapová abeceda jako FPS), nikoli herní odměna.
Každý zásobník má vlastní pulzující celkovou hodnotu (10 za každý plod + bonus). U řezu vyletí
celková hodnota příslušného zásobníku: `+10`, `+20`, třetí hrozen `+130`.
Nativní `scoreAwards` rozlišuje plody zasažené jedním švihem a nese skutečný
přírůstek i celkovou hodnotu (`totalPoints`). Nahrazená série předá existující
symboly a rámečky odletu pod skóre (cubic ease-out, fade a zmenšení za 0,95 s),
s řadou odletí i kladná celková hodnota (např. pět melounů `+770`), která sdílí easing
a vyblednutí; potom se odstraní symboly, rámečky i dočasný číselný panel. Nejvýš jedna odcházející série;
reset nebo další nahrazení ji uklidí okamžitě. Žádná druhá JS herní smyčka.
Pole `pendingBonus` zůstává pro kompatibilitu: ukazuje bonus držených sérií,
který je již započten do skóre.
`XRRoundState.chains` a `ComboDisplayState.chains` umožňují kontrolu skutečného stavu.
Přepnutí režimu resetuje skóre; rekonfigurace stejného režimu jej zachová.

Ověření `arena-minute` nyní kontroluje aktuální chain režim včetně skutečných
řezů 4/5 kusů, třetího druhu, levého triggeru, panelu, AI toolu a koncového
vyplacení. Původní hromadné legacy pořadí lze spustit samostatně:
`verify:browser -- --page arena-minute --backend webgl --arena-legacy-stress`.
Tento dlouhý diagnostický průběh nyní reprodukuje WASM `unreachable` v
`xr_render_eye`, následovaný `RefCell already borrowed` při úklidu XR. Příčina
rendererového pádu není touto gameplay změnou opravena. Legacy pravidla zvlášť
pokrývá `neon-arena-xr`; oddělení diagnostiky není potvrzení, že chyba zmizela.

Po vypršení kola se stejné tlačítko `START` přesune 0,95 m před aktuální
vodorovný směr hlavy, přibližně na výšku hrudi. Meč i trigger spustí další kolo;
za hry se tlačítko vrátí za hráče, aby nepřekáželo řezání.
