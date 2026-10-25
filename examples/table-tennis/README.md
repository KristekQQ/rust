# Stolní tenis: fyzika a zápas

Samostatná Rust/WASM ukázka s vlastním `main.js`, `scene.js` a `index.html`.
Otevři `/examples/table-tennis/`; `?backend=webgl` vybere WebGL, jinak náhled
žádá WebGPU. Pro Quest použij standardní produkční balíček `npm run serve:quest`.

Výchozí **volný trénink** umožňuje pustit míček na pálku, opakovaně ho odrážet,
zkoušet naklonění, přizvednutí a boční švih. Pravý trigger / mezerník / **Pustit
míček** umístí nový míček 42 cm svisle nad aktuální plochu pálky bez počáteční
rychlosti. Nastav pálku vodorovně a nech jej dopadnout. Pravidla zápasu zde
nezastavují opakované odrazy. Na PC je pálka v tréninku vodorovná a pohybuje se myší.

Levý trigger / **Přepnout na zápas** zapne původní bodování do 11 s rozdílem dvou.
V zápase zůstává stejná fyzika. Počítač může natočit a pohnout vlastní pálkou;
nesmí přepisovat rychlost míčku. **R / Nový zápas** resetuje aktuální režim.
**FPS / scéna** nebo F3 ovládá společný prostorový panel vpravo u nohou.

## Nativní fyzika

- `src/table_tennis_contact.rs`: CCD koule proti konečnému válci, včetně zaoblené
  kontaktní oblasti obvodu a rotující hrany. Póza plochy používá gripSpace: shaft
  −Z, normála +X, střed plochy 13,5 cm od středu rukojeti.
- Rychlost povrchu je posun středu plus `omega × kontakt`. Restituce působí na
  relativní normálovou rychlost; Coulombovo tření přenáší impuls mezi posunem a
  spinem. Míček je tenká dutá koule, `I/m = 2 R² / 3`. Oddalující se kontakt
  nedostane nový impuls. Žádné automatické míření ani minimální rychlost odpalu.
- `src/table_tennis.rs`: krok 240 Hz, gravitace 9,81 m/s², kvadratický odpor,
  aproximace Magnusovy síly, útlum spinu, opakované kontakty, pravidla a AI.
  Mezi kroky se interpoluje úchop a rotace; zbytkový čas se zachová i při 72/90 Hz.
- Ve VR se hra počítá až po přijetí aktuálních póz rukou a před vykreslením obou
  očí. Ztráta trackingu, opětovné získání pózy a teleport nevytvoří umělý švih.
- Míček má průměr 40 mm. Jeho skutečná nativní rotace pohybuje barevnými značkami
  na meshi. JS drží ID a předává příkazy; nepočítá odrazy ani spin.

Kontaktní parametry: restituce pálky .85, tření .22; stůl .89/.16; podlaha .55/.16.
Restituce pomalého dosednutí pod .25 m/s je nulová pro klidový kontakt.
Odpor `a = −.14 |v| v`, Magnus `a = .0008 (omega × v)`, útlum spinu .35/s.
Jde o viditelné výchozí parametry, nikoli měření konkrétní gumy a míčku.
Skutečná aerodynamika závisí na rychlosti i spinu, včetně oblasti změny vztlaku;
konstantní koeficienty jsou aproximace. Net-let, pružná deformace sítě, kolize
rukojeti a detailní boční geometrie desky nejsou modelované.

Rozměry a kontrola odrazu vycházejí z [ITTF Laws](https://db.ittf.com/sites/default/files/public/2023-06/2023_ITTF_Statutes_tracked_changes_version.pdf):
stůl 2,74 × 1,525 m ve výšce .76 m; míček 40 mm/2,7 g; odraz přibližně 23 cm
po pádu 30 cm. Přehled skutečné závislosti aerodynamiky viz
[Ito & Kamijima, 2025](https://doi.org/10.11308/tjsst.17.25).

## Ověření

```sh
cargo test --locked table_tennis --lib
npm run verify:browser -- --page table-tennis,table-tennis-xr --backend all --capture
```

Native testy kontrolují restituci, směr, posun/rotaci pálky, spin a energii,
rychlý průlet, hranu, ztrátu trackingu, krok při 72/90/120 Hz, pravidla a drop.
XR scénář používá společný simulátor a skutečný Rust renderer: aktuální grip,
pravý trigger, svislý pád a opakované odrazy, sklon, silnější přizvednutí,
pixely obou očí a prostorové FPS. Výkon a pocit na skutečném headsetu se
ověřují samostatně; simulace nenahrazuje lidskou kontrolu pálky.
