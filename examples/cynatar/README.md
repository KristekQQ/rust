# Cynatar a vestavěný Pixel · Rust 3D fonty

Samostatná ukázka obecného fontového API. `main.js` inicializuje engine a DOM,
`scene.js` zavolá `loadFont3D`/`builtinFont3D` a vytváří pouze textové rooty.
JavaScript nevlastní layout, glyph cache ani pole objektů pro jednotlivá písmena.
Rust sdílí glyph geometrii a používá stávající GPU instancování a neonový shader.

Text lze upravovat, přepnout mezi importovaným Cynatarem a původní pixelovou
abecedou enginu, změnit barvu/emisi/glow a natočit kameru. Nepodporované znaky
vyvolají chybu a předchozí text zůstane zachovaný. Cynatar je pouze pro osobní
použití; licence pixelového fontu není závislá na Cynataru.

[API, převod dalších TTF/OTF, limity a optimalizace](../../docs/FONTS.md).
Původní trubicové neonové číslice jsou dál v [neon-font-preview](../neon-font-preview/).

`?verify=1` otevře testovací tlačítko. Ověření kontroluje skutečné fontové pixely,
otvor O, boční emisi, barvy a glow, změny kořene, původní vestavěnou abecedu,
32 opakovaných písmen v jediném draw, nulovou GPU geometrii u glyph instancí,
stabilní cache při nezměněném textu a uvolnění zdrojů. Použij udržovaný runner:

```sh
npm run verify:browser -- --page cynatar,neon-font-preview,scene-sdk,lifecycle --backend all --capture
```

Desktopová automatika neověřuje Quest compositor, komfort ani FPS v headsetu.
