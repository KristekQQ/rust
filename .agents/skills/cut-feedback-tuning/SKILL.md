---
name: cut-feedback-tuning
description: Tune the native pooled cut-impact ring, alpha texture and speed response in neon-arena, including Quest reports that slow and fast hits look alike. Use for hit effects, not general XR inventory or scene layout.
---

# Ladění efektu řezu

[Projektové workflow](../rust-webgpu-project/SKILL.md) popisuje build a ověření.
Aktuální hra je [neon-arena](../../../examples/neon-arena/README.md), nativní
animace je v `src/cut_feedback.rs`; textura/pipeline v `src/render/cut_feedback*`.

- Nejprve pojmenuj měněnou vlastnost: velikost, práh rychlosti, průhlednost,
  životnost nebo rovina řezu. Neměň současně několik vlastností bez důvodu.
- Pro velikost lehkého zásahu už existuje `?hitSize=0.15` a `CUT_EFFECT_SIZE`
  v receptu arény. Menší hodnota potlačuje lehké zásahy; maximální silný útok
  zůstává výrazný. Tato konfigurace nepotřebuje WASM build.
- V současné křivce je minimum do 0,75 m/s a maximum od 3 m/s gripu;
  mezi nimi je kvadratický růst. Při změně hodnot uprav také příslušné důkazy,
  ne pouze očekávání bez kontroly chování. Čísla jsou nastavení této hry,
  nikoli obecná fyzikální konstanta nebo pravidlo pro každou zbraň.
- Rychlost měří `XrPlayer` z gripu A→B / čas XR, ne ze vzdálené špičky meče.
  Při hlášení příliš velkého efektu zkontroluj nejprve skutečné měření a jeho
  předání do fragmentů. Neposouvej opakovaně práh naslepo.
- Efekt patří do skutečné řezné roviny a středu řezných ploch. Při šikmém řezu
  neotáčej kruh ke kameře a nepřidávej další prstence místo opravy roviny.
- RGBA textura má skutečný alpha blending, test hloubky a nezapisuje hloubku.
  Uzavřený pomocný mesh smí prolnout jen jednu plochu. Zachovej půlení meshe,
  průhledný plášť ovoce, pevný pool a transformace bez přestavby geometrie za snímek.

## Krátká iterace

Čistá změna konfigurace: odpovídající URL + dotčený browser scénář; bez Rust buildu.
Nativní změna: nejmenší užitečný test, jeden matching release WASM build a cílený
runner pro dotčenou stránku. V průběhu ladění nespouštěj celý předpushový gate.
Nepřidávej pomocníka pro prostou změnu čísla. Plné ověření patří až před push.

`neon-arena-xr` porovnává stejný plod a dráhu při různém čase, velikost při stejném
stáří efektu a samostatně malý i velký zásah. Pouhý poměr dvou neviditelných efektů
nestačí. Při změně průhlednosti zachovej pixelový důkaz kompozice s geometrií
za efektem. První kompilaci pipeline v testu zahřívej se zastaveným časem efektu.
Důkaz ve skutečné aréně má přednost před samotným testem skalárního násobiče.
