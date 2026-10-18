---
name: scene-object-layout
description: Adjust positions, rotations and scale of 3D objects and attached text in this Rust/WASM engine, especially WebXR scenes. Use for requests such as right/left, closer/farther, lower/higher, or moving a display onto a prop.
---

# Rozmístění objektů ve scéně

Projektový skill; kořen repozitáře je o tři adresáře výš. Respektuj
[AGENTS.md](../../../AGENTS.md) a ověřovací matici v
[DEVELOPMENT.md](../../../docs/DEVELOPMENT.md). Tento postup řeší layout scény;
změnu rendereru nebo SDK potřebuje jen tehdy, když existující příkazy nestačí.

## Souřadnice a záměr

- Při výchozím XR pohledu směrem −Z znamená +X doprava, +Y nahoru a zápornější Z
  dál před hráče. Jednotkou je metr. U jiného natočení odvoď strany od dohodnutého
  pohledu nebo ukotvení hráče, nikoli automaticky od znaménka světového X.
- Desktopový OrbitCamera používá levotočivý pohled; při pohledu −Z může být obraz
  vodorovně obrácený proti XR. Nápisy v příkladu mají korekci orientace pro náhled.
  Umístění „vpravo ve VR“ nepotvrzuj jen desktopovým screenshotem. Skutečný XR
  převod je v [render_math.rs](../../../src/render_math.rs).
- Změň souřadnice objektu, když uživatel žádá přesun objektu. Pohyb/zoom kamery
  neřeší jeho fyzické umístění. „Níže“ mění výšku; menší výška objektu je samostatná
  změna měřítka, pokud ji záměr skutečně vyžaduje.
- `setPosition` a `setScale` nastavují absolutní hodnoty; `setRotation` používá
  radiány. `rotate` je jiný příkaz: přírůstek ve stupních s volitelným časováním.
  Při pochybnosti zkontroluj [typy SDK](../../../js/engine.d.ts).

## Celek a připevněné části

Najdi recept scény v `examples/<name>/scene.js` a jeho volání v `main.js`.
Zjisti, které modely, texty, rámečky a interakční objekty tvoří přesouvaný celek.
Přesun neznamená přepisovat layout ostatních objektů nebo měnit efekty prostředí.

Pro oddělené SDK objekty používej společný autorovací základ: `origin`, natočení
a `scale`; lokální pozice příslušenství převeď jako `world = T + R * (S * local)`.
Popisky na povrchu se musí přesunout, natočit i rozměrově přizpůsobit společně
s podkladem. Ponech malý odstup ve směru normály, aby nápis nezmizel uvnitř modelu
nebo neblikal se souběžnou plochou.

- `setScale` nepřičítá k dosavadnímu měřítku. Korekce textu při vstupu/výstupu XR
  nesmí nechtěně zrušit jeho základní měřítko. Lze měnit autorované `width/height`
  textu a zachovat samostatnou korekci znaménka osy X.
- Ověř pozici interakční plochy, collideru a kurzoru, pokud patří k přesouvanému
  prvku. Nedoplňuj kolize ani vazbu k hlavě, pokud je scéna nemá požadované.
- Polohy pro statický layout spočítej jednou v receptu scény. Runtime sledování,
  simulace a rendering zůstávají v Rustu; nepřidávej synchronizační JS RAF.
- Při zmenšení nebo vzdálení zkontroluj čitelnost bitmapového písma v oku headsetu.
  Pozice odpovídající požadavku sama nezaručuje čitelnost ani pohodlí.

Praktický vzor společného základu a nápisů na kameni je v
[neon-arena/scene.js](../../../examples/neon-arena/scene.js). Jeho konkrétní
souřadnice nejsou výchozí hodnoty pro jiné scény.

## Ověření a předání

Pro změnu pouze v JS receptu použij matching existující WASM, kontrolu typů
a udržovaný browser runner pro dotčený example. Při změně WASM/exportů obnov `pkg/`
podle projektového workflow. Kvůli pouhému přesunu není nutný nový build ani plná
sada testů.

U XR zkontroluj obě oči, stranu a výšku vůči hráči, okluzi, ukotvení při pohybu
hlavy a funkčnost dotčeného ovládání. Vhodný je konkrétní invariant polohy nebo
rozsahu, nikoli test přesného znění komentáře. Pro arénu existuje scénář
[neon-arena-xr.html](../../../tests/neon-arena-xr.html):

```sh
VERIFY_WASM_PROFILE=release node scripts/test-browser.mjs --page neon-arena,neon-arena-xr --backend all --capture
```

Vyber skutečně dotčené scénáře z matice; příkaz výše není univerzální test jiných
her. Reportuj stručně výsledné umístění a skutečné ověření. Simulované XR odliš
od ověření na Questu. Zachovej stav ostatních částí scény a její výkonový profil.
