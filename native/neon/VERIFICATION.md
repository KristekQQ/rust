# Ověření — 2026-11-04

- Základ: izolovaný codex/native-neon-20261104, base84a44ec32 + zmrazené
  necommitnuté změny nativního Říma tohoto chatu. Původní checkouty zachovány.
- Offline authoring export rev48 opakovatelný:607operací,127handles,
  150variant/10druhů,24cues/140targets;2glTFdekorace a Cynatar font.
- 6 nativních Neon testů PASS: kompletní obsah, výchozí galerie/sklo,
  minutový start/pauza, skutečný katana řez+skóre+bank, TOP5 persistence,
  držení meče v tlačítku bez reset smyčky a všechny24cues/140targets.
- 8 testů sdíleného Rim hostu PASS po FrameInput/haptics/host adaptaci.
- Nezávislá Lupa source revize našla3lifecycle/reset chyby; opravené,
  následná kontrola bez zbývajících nálezů v daném rozsahu.
- Native Vulkan/MoltenVK replay PASS:10630barev,93461stereo odlišných pixelů.
  Info replay PASS:28648barev,1784681stereo odlišných pixelů. Obrazy prohlédnuté;
  text čitelný ve renderu. Skutečný App tracking-loss zachoval polohu projektilu
  i round clock. Offscreen hodnoty FPS/Hz nepředstavují měření Questu.
- Android arm64 release APK, podpis a16KiB alignment PASS. Konečná identita
  je v output/native-neon-release/DELIVERY.json.
- Uživatel není doma a bude zavírat notebook; instalace a fyzická akceptace
  byly výslovně odloženy, nic se do headsetu nenahrávalo.
- Nový webový runtime balíček se nepublikoval. Pro nativní port se neměnily
  browser příklady; sdílená SceneManager API změna je přidaný single-panel update.
- Žádný commit/push/CI. Výkon, stereo komfort, haptika a skutečný kompletní
  průchod kolem na Questu zůstávají k ověření doma.

Následná instalace: USB Quest3 serial2G0YC5ZG1L01Q0 autorizovaný; APK687d9271
nainstalováno, tcpip5555 obnoven a ADB Wi-Fi192.168.0.234:5555 ověřeno.
Neon spuštěn přes Wi-Fi, base.apk SHA256 přečten z headsetu přesně souhlasí.
Oculus runtime209.166.0 vytvořil relaciIDLE; při prvním záznamu PowerManager
hlásil Asleep. Aktivní120Hz, skutečný obraz a FPS zatím nejsou potvrzené.
Záznamy output/native-neon-release/device-first-run.txt a device-start-followup.txt.

Po nasazení se relace skutečně rozběhla FOCUSED na120Hz; app snímky jsou
zachycené v device-first-play.txt. Posledních30 app vzorků: {"scope": "latest up to30 one-second startup/play app samples; uncontrolled view/game state; no sustained120 claim", "samples": 13, "appFpsMin": 71.0, "appFpsMedian": 119.9, "appFpsMax": 120.8, "runtimeHz": 120, "focusedSessionObserved": true, "userVisualAcceptance": "pending"}
Po sejmutí přešla relace zpět STOPPING/IDLE. Nejde o kontrolované měření
celého kola ani potvrzení fyzické čitelnosti a ovládání uživatelem.

2026-11-04 rev49: grave/credits removed, one score-display model remains. Re-export 599 operations, 6/6 native tests PASS. Earlier screenshots and device evidence above describe rev48.

Rev49 web: canonical production served on8443, HTTPS scene bytes checked (rev49, no grave/credit). Typecheck PASS via arm64 Node. Browser runner blocked before scenarios by Chrome DevTools startup timeout; no new browser rendering acceptance. Native APK45a32595 ready; Quest Wi-Fi offline, installation pending.

Rev50 cyber swords: closed nondegenerate geometry (364 triangles each), typecheck PASS,
6/6 native tests incl real tracked cut/bank PASS. Native stereo inspection replay
PASS:26323 colors,598580 stereo-different pixels. Parent and independent Oko
reviewed both eyes: readable edges/guard/grip, no obvious mesh clipping; mostly
straight angular sci-fi silhouette. Inspection pose is not gameplay or Quest acceptance.
Canonical minified release rebuilt and HTTPS scene rev50 verified.
Browser launch remains blocked from rev49 attempt; no fresh browser runtime claim.
