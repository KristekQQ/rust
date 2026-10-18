# Ovocná aréna s černým pozadím

Aktuální ukázka používá čisté černé pozadí. Obsahuje devět druhů ovoce včetně dvojice třešní, banánu, hroznů,
pomeranče a limetky; dvě katany,
30sekundové kolo a skutečné Rust řezy. Pod nápisem APOLLO GAMES je bitmapový
3D displej se skóre. Nízký žulový náhrobek vpravo, přiblížený pro čtení nese kredit CODEX a živou
diagnostiku FPS, FRAME MS, CPU MS, CULL MS, VISIBLE, CULLED, OBJECTS a DRAWS.

Aplikace neimportuje `environment.js`, nestahuje barevné/emisní stěny cubemapy,
nevytváří odraznou podlahu a nemá zapnutý SSR ani bloom. Postprocess statistiky
mají nula průchodů a nula barevných cílů. Původní obrazové podklady a offline
build skripty zůstávají uložené v příkladu, ale herní stránka je nepoužívá.

Devět druhů ovoce slouží jako desktopový náhled ve dvou řadách. Ve VR se tyto existující objekty
zaparkují mimo scénu a každých 850 ms startuje plod přímo před hráčem ve výšce rukou. Rust vlastní fyziku, řezy,
čas i skóre. Objekt DALSI KOLO lze aktivovat mečem nebo namířením a triggerem.
Při odchodu z XR se výstřely zastaví a náhledové ovoce se vrátí.

Ovoce má skutečně průhledné mezery mezi neonovými hranami. Příkaz
`setColorCutout(.25)` zahodí tmavé plochy před zápisem hloubky; světlé hrany
se kreslí z obou stran. Nejde o černou výplň ani o celoplošný postprocessing.
Uzavřený objem pro fyziku a řezy zůstává zachovaný. Skutečné půlky ukazují
barevné uzavřené řezné plochy; vnější neonový plášť zachovává průhledné mezery.

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
nevibruje stále. Celé plody používají stejnou gravitaci a odraz 0,55. Efekty i odlet počítá Rust, bez JS RAF,
celoplošného rozmazání, bloom, SSR nebo dalších světel.

## Spuštění a kontroly

- Náhled: `/examples/neon-arena/`, automatický WebGPU.
- Quest: `/examples/neon-arena/?backend=webgl` na důvěryhodném HTTPS; [připojení](../../docs/WEBXR.md).
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
