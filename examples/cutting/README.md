# Cutting example: závěr compute experimentu

Rozhodnutí z 2026-10-16 pro další instance: **pro tuto úlohu ponechat řezání na CPU**.
WebGPU compute jsme skutečně vyzkoušeli jako hybrid i jako GPU-residentní řezání
včetně uzávěrů a vykreslovacích bufferů. V dosavadních lokálních end-to-end
testech bylo optimalizované Rust/WASM CPU rychlejší; compute se pro aktuální
velikost jablek a jednotlivé řezy nevyplatilo.

- `auto` záměrně volí CPU. `gpu` a `gpu-resident` jsou experimentální režimy pro
  porovnání, nikoli doporučená optimalizace. Zachovat je i jejich testy.
- Další instance nemají znovu přesouvat toto řezání na GPU jen na základě
  předpokladu, že GPU musí být rychlejší. Závěr znovu otevřít při požadavku
  uživatele, změně zátěže nebo nových průkazných měřeních.
- Jde pouze o tento compute experiment, ne o WebGPU renderer ani všechny
  compute úlohy. GPU varianta má navíc užší podporu topologie a CPU fallback.
- Quest 3 zatím změřený není. Lokální časy kolísaly; další srovnání musí
  kontrolovat napájení/úsporný režim, zahřátí pipeline, stejnou zátěž a celkovou
  odezvu včetně alokací, čekání a readbacku. FPS celé scény není čas řezu.

Detaily API, omezení a naměřená čísla: [docs/CUTTING.md](../../docs/CUTTING.md).
Historie implementace a ověření: [progress.md](progress.md).
Samostatný example vlastní bootstrap v `main.js` a scénu v `scene.js`; bez WebXR.
