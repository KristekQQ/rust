# Ovocná aréna s neonem a lesklou podlahou

Aktuální ukázka používá čisté černé pozadí. Obsahuje devět druhů ovoce včetně dvojice třešní, banánu, hroznů,
pomeranče a limetky; dvě katany,
30sekundové kolo a skutečné Rust řezy. Přímo před hráčem je velký odpočet z neonových trubic a pod ním skóre. Nízký žulový náhrobek vpravo mimo prostor galerie nese kredit CODEX a živou
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
Každých 550 ms startuje plod přímo před hráčem ve výšce rukou. Rust vlastní fyziku, řezy,
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

Každá vlna je `[ovoce, ovoce, wild, ovoce, ovoce, ovoce]` po 550 ms,
od třešní až po meloun. Všech devět druhů se vystřídá za 29,7 s během 30s kola při průběžném zasahování cílů.
Žolík má vlastní uzavřený neonový mesh třícípé šaškovské koruny a lze ho skutečně
rozkrojit. `setComboTarget(kind, points, {wild:true})` jej označí jako náhradu
jakéhokoliv druhu: ovoce–wild–ovoce i wild–wild–ovoce vyplatí cenu ovoce.
Tři wild vyplatí 120 bodů; po výplatě další trojice začíná samostatně.

Plody mají poloviční původní měřítko (scale 0,105), startují v `[±0,38;1,15;−1,10]`, mají pomalý
oblouk (vy=1,2 m/s, gravitace 1,6 m/s²), mírně se přibližují a po 1,9 s zmizí
bez odrazů. V dosahu 1,10–1,65 m na výšku zůstávají přes sekundu; nejvýš tři
současné cíle brání zahlcení i při ručním vystřelování. Jde o návrh hratelnosti,
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
