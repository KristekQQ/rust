# Společný tester VR

Tester má dvě vrstvy: kontrakt enginu a samostatné herní scénáře. Žádná budoucí
hra nemá kopírovat vlastní náhradu `navigator.xr`. Vše používá společný
`tests/xr/simulator.js`. Simulované jsou headset, ovladače a čas; skutečný
Rust/WASM renderer, scéna, fyzika a geometrie zůstávají aktivní.

## Použití

```sh
npm run verify:vr
npm run test:xr
# Také pomalé přehrávání (ověří skutečné čekání oddělené od virtuálních hodin):
npm run test:browser -- --page webxr-contract,webxr-simulation --backend webgl --xr-slow
# Pouze existující matching WASM, bez buildu:
npm run test:browser -- --page webxr-contract,webxr-simulation --backend webgl --capture
```

[Obecný tester](../tests/webxr-contract.html) nabízí virtuální profily stojícího,
sedícího a prostorového uživatele s kroky 72/90/120 Hz a různými rozestupy očí.
Nejde o naměřené FPS konkrétního zařízení. [Jablka](../tests/webxr-simulation.html)
jsou první herní adaptér; jejich pravidla nejsou uvnitř XR platformy.
Vše je lokální, stejný server jako ostatní testy. HTTPS pro Quest popisuje
[WEBXR.md](WEBXR.md).

## Co se ověřuje společně

- Oddělený obraz očí, IPD a šest stupňů volnosti hlavy se skutečnými pixely.
- Jeden krok Rust simulace na XR snímek, různé časové kroky a lokální podlaha.
- Levý/pravý grip a orientace, nezávislá ztráta trackingu, odpojení a návrat.
- Hrana stisku triggeru, držení, uvolnění, obnova bez falešné akce.
- Skrytá relace, systémové překrytí a přenastavení referenčního prostoru.
- Zamítnutý vstup, ukončení, návrat rozměrů canvasu, opakovaný vstup a dispose.

Použití squeeze, páček/axes, haptiky, ručního trackingu nebo teleportace se
musí explicitně přidat mezi herní požadavky. Simulátor umí dodat obecné buttons
a axes, současný SDK však veřejně zpracovává grip a trigger. Test nesmí vydávat
pouhou přítomnost dat za funkční herní ovládání.

## Další hra

1. Vytvoř `tests/xr/scenarios/<hra>.js` s funkcí, která přijme `{simulator,test}`.
   Importuj skutečný bootstrap hry, spusť XR a používej `simulator.frame(...)`.
2. Pomocí `test.assert(condition, 'stálý název kontroly')` kontroluj skutečný
   stav hry. U viditelných změn přidej skutečné framebuffer pixely. Na konci
   ukonči relaci a zavolej `engine.dispose()` i při chybě.
3. Přidej malou HTML stránku s canvasem a DOM potřebným pro bootstrap hry.
   Obal běh pomocí `new XRTestRun(id, simulator, publishXRReport).execute(...)`.
4. Zaregistruj stránku v `scripts/test-browser.mjs` s `backends:['webgl']` a
   `xrReport:true`. Registr zahrne scénář i do plného předpushového gate.
5. Herní úspěch definuj před implementací: očekávaný zásah, blízké minutí,
   opakování, přerušení trackingu a úklid. Pro každou hru napiš také krátkou
   přejímku na skutečném headsetu.

```js
const simulator = installXRSimulator({profile:{id:'my-case',hz:90}});
const test = new XRTestRun('my-game', simulator, publishXRReport);
try {
  await test.execute(async test => {
    const app = await import('../../../examples/my-game/main.js');
    try {
      await app.engine.xr.enter();
      simulator.frame({
        head:{position:[0,1.6,0],orientation:[0,0,0,1]},
        controllers:{right:{position:[0.3,1.2,-0.5],pressed:false},left:null}
      });
      test.assert(app.gameState().score === 0, 'no accidental action');
    } finally { app.engine.dispose(); }
  });
} finally { simulator.dispose(); }
```

`frame()` používá úplné matice nebo pozici + quaternion. Neuvedená pole zachová;
`controllers.left=null` odpojí levý zdroj a `tracked:false` ztratí jen jeho pose.
`setVisibility()` a `resetReferenceSpace()` vyvolají odpovídající XR události.
`denyNextRequest()` / `deferNextRequest()` umožní testovat start a jeho zrušení.
Pro zpomalení zobrazení používej `await simulator.wait(ms)`; globální setTimeout
a setInterval jsou součástí virtuálních hodin a čekání na ně by zastavilo replay.
Instalaci vždy páruj s `dispose()`, který obnoví globální API, časovače a zdroje.
Nevkládej tester do produkčního SDK ani runtime hry.

## Důkazy a opakování

Každá kontrola má jméno, PASS/FAIL a počet pozorování. Stránka poskytuje stažení
JSON reportu včetně syntetických vstupů. Runner ukládá reporty a stopy pod
`output/verification/*-report.json` a `*-trace.json`; při `--capture` i snímky.
Report rozlišuje simulované XR od skutečného Rust rendereru.

`simulator.recording()` vrátí verzovanou JSON stopu normalizovaných vstupů.
`await simulator.replay(trace, {afterFrame,slow:true})` ji přehraje v aktivní
čisté testovací relaci. Stav aplikace musí adaptér před přehráním obnovit;
přehrání vstupů samo o sobě nenahrazuje kontrolu očekávaného výsledku.
Stopa zahrnuje i viditelnost a reset prostoru. U více relací vyžaduje replay
`onSession(index)`, ve kterém adaptér ukončí starou a vytvoří novou relaci.
Změna renderovacího profilu nebo režimu podlahy se odmítne. Obecný kontrakt
kontroluje opakování šestiosé stopy porovnáním skutečných pixelů 1:1.

## Hranice

Nejde o certifikaci WebXR ani emulaci skutečného Quest compositoru. Syntetické
stopy nepotvrdí tracking hardware, komfort, skutečnou latenci, výkon/teplotu,
permission UI, důvěru certifikátu ani fyzický prostor. Tyto věci se musí ověřit
na headsetu. Podporované lifecycle události vycházejí ze
[specifikace WebXR](https://www.w3.org/TR/webxr/).
