# Neon robot — native Blender remake

The current asset is authored in Blender from new mesh and curve geometry. It
does not import or polish the earlier procedural GLB. The supplied reference
informs the huge oval helmet, asymmetrical brass optics, swept wire crowns,
raised industrial cannon arm, down-pointing energy blade and diagonal lunge.

## Reproduce

```sh
/Applications/Blender.app/Contents/MacOS/Blender --background \
  --python assets/models/neon-robot/build_blender.py
```

- `build_blender.py`: native bpy geometry, PBR materials, pose, studio and animation.
- `neon-robot.blend`: animated assembled character plus studio and a hidden
  `AUTHORING` collection retaining editable individual meshes, curves and bevels.
- `neon-robot.glb`: character-only export; no floor, camera or lights.
- `make_textures.py`: regenerate the deterministic scratch/pitting PNGs in Blender Python.
- `build_stage.py` / `studio-stage.glb`: preview-only PBR floor, outside the character GLB.
- `textures/`: actual 512px violet/brass base-color, roughness and normal PNGs,
  packed in the Blender source and embedded in the GLB.
- `output/neon-robot/blender-reference.png`: 1024px Cycles hero render with
  pink/cyan studio lights, reflective dark floor and compositor Fog Glow.

The rejected non-Blender prototype is archived outside the asset folder under
`output/neon-robot/rejected-procedural/`; the current build uses Blender.

## Asset and animation

The Blender scene is Z up, front -Y. The GLB is Y up, front +Z, with an export
lift calculated from actual vertices so the lowest boot is exactly at Y=0.
Neutral GLB bounds: `[-2.2645, 0, -0.6540]` to `[1.9066, 3.6885, 1.7207]`.

The runtime export contains eleven rigid meshes, 23 nodes, 80 material
primitives, eleven materials, six embedded PNG images and 490,322 triangles.
It is 16,143,772 bytes. All armor, vents, fasteners, iris lamellae, hydraulic
lines, fingers, neon tubes and the closed tapered blade are real geometry.

- `Idle` — 4 seconds: gentle chassis movement, head, arms and reactor.
- `Attack` — 2 seconds: raised blade wind-up/swing with elbow/wrist follow-through.
- `Victory` — 3 seconds: claw raised, small body bounce and head motion.

Controls preserve the real shoulder, elbow and wrist pivots. Every clip returns
to its exact neutral endpoints. The export step normalizes Blender's frame-one
NLA time origin and restores authored neutral node transforms omitted by its
exporter. Each clip explicitly includes translation and rotation for all eleven
controls; this is compatible with engines that reset node transforms on sampling.

Validated: GLB chunk/buffer lengths, all accessor/index ranges, embedded PNGs,
clip coverage, exact durations, closed endpoints, first keys matching neutral
node transforms, ground contact, and GLB world bounds matching the authored
Blender world geometry. Browser rendering and headset acceptance are separate.

The Blender hero render uses studio lights, metallic PBR and optical bloom.
The runtime receives the actual embedded worn-metal textures; its final lighting
and glow are supplied by the engine scene.
