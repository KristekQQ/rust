# Crystal cave

Original seeded low-poly environment made from actual Blender meshes, with no external assets or textures. Coordinates are meters, Y up, forward -Z. `crystal-cave.blend` is editable; `build.py` regenerates the geometry, GLB, layout and eye-level `interior-preview.png`.

```
/Applications/Blender.app/Contents/MacOS/Blender --background --python assets/models/crystal-cave/build.py
python3 assets/models/crystal-cave/validate_asset.py
```

The flat dry floor is y=0. Walkable center bounds are x[-7,7], z[-31,2]; keep player radius inside those bounds. The entire x[-2,2] central lane stays clear. Nine landing pads and conservative side-object boxes are in `layout.json`. Cave walls and roof enclose the interior; the open entrance is at z=4. Hanging stalactites terminate well above head height. The oversized rear cyan formation sits beside the central lane.

Eight joined flat-shaded material meshes limit draw primitives; cyan/violet crystals and warm lantern glass use emissive PBR materials. There are no exported lights or cameras. Presentation-only lighting and a camera remain in the Blender file; the engine must provide interior fill and local crystal/lantern lights. The Blender preview establishes geometry and composition, not engine appearance or headset performance. `asset-report.json` comes from validation of exported bytes.
