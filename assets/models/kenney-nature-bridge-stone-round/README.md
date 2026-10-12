# Kenney Nature Kit: bridge_stoneRound

Originální statický model kamenného obloukového mostu vhodný jako geometrie pro průchod světelných paprsků.

- Primární zdroj: https://kenney.nl/assets/nature-kit
- Oficiální archiv (stažený z tlačítka Download na stránce výše): https://kenney.nl/media/pages/assets/nature-kit/37ac38a37b-1677698939/kenney_nature-kit.zip
- Soubor v archivu: `Models/GLTF format/bridge_stoneRound.glb`
- Licence: CC0 1.0; přesné znění dodaného souboru je v [LICENSE.txt](LICENSE.txt). Oficiální stránka označuje balík CC0.
- Formát: glTF 2.0 binary (`.glb`), 22,552 bajtů.
- SHA-256 modelu: `fec03f019a27a9bccc7d68f367791db04175bb541080866972c1b96a744b065c`
- Geometrie a BIN data jsou původní. Při rozšíření parseru byl opraven scene root z node 1 na jeho identický rodič node 0; původní export porušuje glTF požadavek na kořen bez rodiče. Původní soubor je zachován v `bridge_stone_round.original.glb` (SHA-256 `0b2254c5eb59a5fac2c7185cdc9af8efb941a31f0c0fb5a3fabaeb60c0108b41`).

## Struktura modelu

- 2 nodes, 1 mesh, 2 TRIANGLES primitives, 2 materiály.
- Materiály `stone` a `stoneDark` mají `baseColorFactor`; model neobsahuje ani neodkazuje textury či obrázky.
- Bez skinů, animací a morph targets. GLB deklaruje `KHR_materials_unlit`; barvy jsou zároveň uloženy v základních PBR faktorech.
- Rozměry z pozice accessorů: 1.04 × 0.45 × 1.04 jednotek (X × Y × Z), před node transformacemi.
