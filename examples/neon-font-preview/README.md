# Neonové číslice 0–9

Samostatná ukázka skutečných Rust 3D číslic: `main.js` nastavuje vlastní engine,
`scene.js` tvoří dvě řady přes `scene.createNumericText`. Nepoužívá běžící kolo
ani načítání scény z arény. Všechny číslice sdílejí tutéž knihovnu jako herní
odpočet a skóre. Jezdec mění pouze glow v této ukázce.

`npm run verify:browser -- --page neon-font-preview --backend all --capture`
ověří deset instancí, deset sdílených meshů a skutečnou změnu pixelů s glow.
