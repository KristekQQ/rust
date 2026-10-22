# Opička / Monkey

Original procedural model made for this project, without downloaded assets or a
reference image. Uses the repository license. Front is +Z, up is +Y, feet at Y=0.

`monkey.glb` is a self-contained glTF 2.0 asset with smooth geometry, six PBR
materials and the three-second looping `Hello` animation (shoulder and head).
The mesh consists of overlapping closed parts; it is a realtime character study,
not a watertight single surface for printing. No skeletal rig or textures.

Regenerate deterministically with Python 3 (standard library only):

```sh
python3 assets/models/monkey/generate.py
```

Live example: `examples/monkey/`. Model loading, hierarchy, animation and rendering
use the Rust engine. JavaScript only issues SDK commands.
