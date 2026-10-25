# Neon District — cyberpunková VR aréna

Stejný example `/examples/monster-room/` nyní obsahuje venkovní městskou ulici pro budoucí hack-and-slash. Prostředí je skutečný samostatný GLB vytvořený v Blenderu. Ve VR má hráč dva meče. Rust řídí přibližování jednoho protivníka, zásah švihem a smrtící animaci Explode; teprve po jejím dokončení nastupuje další ze tří druhů. Druhy se opakují. Jedna rána stačí. Monstra na dosah přehrávají útok; zdraví hráče zatím není zavedeno.

## Prostor a ovládání

Ulice ústí do širší bojové křižovatky. Boční cesty vedou kolem překážek a vracejí se do zadní části arény. Svítící kruhy označují orientační místa; teleport není omezený jen na ně. Průchozí podlaha je na y=0. Lávky, balkony a střechy jsou zatím kulisa, nejsou cílem výškového teleportu.

- Meč v každé ruce: švih čepelí přes tělo monstra. Pouhé držení v těle zásah nevyvolá.
- Levý analog: chůze 2,5 m/s podle směru hlavy.
- Pravý analog: otočení o 30°.
- Držet pravé A: zpomalit svět na 15 % a zamířit elipsou; pustit A: teleport až 10 m.
- B, výpadek trackingu nebo ukončení relace zruší zamíření.

## Asset a ověření

`assets/models/cyberpunk-alley/` obsahuje GLB, editovatelný `.blend`, Blender builder a `layout.json`. Navigační půdorysy odpovídají autorem zadané pevné architektuře. Runtime rendering, animace, pohyb a teleport zůstávají v Rustu; JS pouze načte assety a předá konfiguraci.

`npm run serve:quest` připraví release/minifikovaný balíček. V existujícím HTTPS serveru otevři `/examples/monster-room/`; `?backend=webgl` vybírá kompatibilní cestu.

Browser runner: `VERIFY_WASM_PROFILE=release npm run test:browser -- --page monster-room,monster-room-xr --backend all --capture`. Ověřuje načtení GLB, různé pohledy, průchodnost všech orientačních míst a skutečné přesuny teleportem po obou větvích. Simulace není měření výkonu ani komfortu v Questu.

## Quest quality revision 2

The scene disables four dynamic shadow maps and requests runtime-recommended XR resolution (`resolutionScale:1`, formerly 0.75). This increases eye pixel count; headset FPS must be measured, not inferred from desktop tests. Browser preview respects device pixel ratio (capped at 2). Bloom and geometry remain. This is not MSAA/FXAA; fine edges can still alias. Optional same-origin diagnostics report actual FPS, eye dimensions, CPU submission and render counters under revision `neon-district-2`. Use `?profile=1` only for detailed profiling. Other examples retain their default XR scale.

Rigid glTF parts keep their vertex buffers on the GPU; node animation updates transform uniforms. Original enemy GLBs and all animation clips are retained. Diagnostic revision `neon-district-3-rigid` identifies this renderer change. Desktop upload and timing evidence does not establish Quest FPS.

FPS je standardně vidět přímo v headsetu na prostorovém panelu 0,66 × 0,40 m, 0,9 m vpravo, 0,35 m nad podlahou a 1 m před nohama. Sleduje hráče a teleport přes nativní `bindHead(...,{anchor:'feet'})`; při otočení ani sklopení hlavy neujíždí, drží místo vůči referenční matici hráče. Nativní počitadla se obnovují 4× za sekundu, bez JS RAF a bez screen-space debug overlaye. Před vstupem lze panel vypnout přepínačem. Revize `neon-district-4-spatial-fps`.

## Melee revision 5

Native encounter: `src/melee_encounter.rs`; explicit SDK command `scene.configureMelee`, diagnostic `scene.meleeState`. Only a tracked moving blade can defeat the torso capsule. Tracking gaps, session changes, snap turns and teleports clear sweep history. Hidden waiting models are paused; navigation caches its next waypoint. No mesh slicing of the animated GLB is performed: death uses its original Explode clip.

Focused acceptance: `cargo test melee_encounter --lib`; release WASM, typecheck, `test:browser -- --page monster-combat,monster-room,monster-room-xr --backend all --capture`. Simulated XR is separate from physical Quest acceptance.
