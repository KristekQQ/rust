# Opička

Samostatný příklad skutečného GLB modelu v Rust/WASM enginu. Vlastní `main.js`
inicializuje engine, `scene.js` načítá model a přidává světla. Rust přehrává
animaci `Hello`; JavaScript nemá animační smyčku.

Otevři `/examples/monkey/` na projektovém HTTP serveru. Tažení myši otáčí kameru,
kolečko přibližuje; tlačítka ukážou přední/zadní pohled a pozastaví mávání.
Odkaz Stáhnout GLB uloží samostatný model, použitelný i v jiných programech.

Model i generátor: [assets/models/monkey](../../assets/models/monkey/README.md).
Jde o originální stylizaci bez referenčního obrázku.

Cílené ověření skutečných pixelů modelu, animované pózy a zadního pohledu:

```sh
VERIFY_WASM_PROFILE=release npm run test:browser -- --page monkey --backend all --capture
```

Vyžaduje matching `pkg/`; pokud není aktuální, použij `verify:browser` se stejnými
parametry a `VERIFY_WASM_PROFILE=release`. Volitelný ověřovací panel až po kliknutí načítá `tests/pixels.js` z repozitáře; pro
samostatnou aplikaci lze ověřovací import a panel odstranit.
