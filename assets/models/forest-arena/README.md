# Woodland arena assets

Original geometry generated with Blender; no external models or textures.

- `forest-arena.blend` is the editable environment with presentation lighting and scenic/overview cameras.
- `forest-arena.glb` contains the complete 100 × 120 m woodland, flat floor, paths, layered branching trees, meadow geometry, ruined arches and rock perimeter.
- `golem`, `treant`, `mushroom` each have editable `.blend` sources and individual `.glb` files. Each GLB contains one complete looping `Idle` animation.
- `layout.json` uses engine meters, Y up and -Z forward, bounds `[-50,50,-100,20]` and authored spawn `[0,0,0]`. Ground top is y=0. Obstacles conservatively bound actual blocking geometry. Overhead branches/arch lintels and low meadow decoration do not block walking.
- `modelSpawns[].slug` selects an individual creature GLB. Landing pads form connected routes with steps below 7 m.

Rebuild from this repository root:

```sh
/Applications/Blender.app/Contents/MacOS/Blender --background --python assets/models/forest-arena/build_blender.py
python3 assets/models/forest-arena/validate.py
```

The fixed seed is 7319. Reports and rendered previews are generated under `output/forest-arena-assets/`. Presentation lights and cameras are excluded from GLB. Blender previews establish geometry and composition; runtime/browser and headset acceptance are separate checks.
