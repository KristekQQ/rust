# KAIRO 09 — outdoor cyberpunk alley

Original procedural architecture created for this repository. No third-party meshes,
textures, fonts or image files are embedded. Lettering is converted Blender Bfont
geometry; all surfaces use self-contained PBR materials. The editable Blender scene
retains material-joined geometry, presentation lights and two cameras.

Created and rendered with Blender 5.3.0 Beta (2026-10-15 build).

Rebuild from the repository root:

```sh
/Applications/Blender.app/Contents/MacOS/Blender --background --factory-startup --python assets/models/cyberpunk-alley/build_blender.py
```

This regenerates `cyberpunk-alley.blend`, `cyberpunk-alley.glb`, `layout.json`, and
`output/cyberpunk-alley/{hero.png,street.png,asset-report.json}`. Run `python3 assets/models/cyberpunk-alley/validate_asset.py` to reproduce the
layout, GLB budget and self-containment checks (`output/cyberpunk-alley/layout-checks.json`).

The GLB excludes
presentation cameras/lights; engine integration supplies lighting. Joining geometry
by material bounds render primitives. No external texture downloads are needed.

Coordinates use metres, X right, Y up, arrival looking toward -Z. Floor surface is
Y=0; navigation rectangle is `[-14.5,14.5,-31.4,7.4]` in minX,maxX,minZ,maxZ order.
`layout.json` exposes `spawn`, `landingPads` (`id`, `position`, `radius`), `obstacles`
(`id`, `center`, `halfExtents`) and `modelSpawns` (`id`, `position`). The obstacle
AABBs describe all ground building masses, kiosks, utility blocks and the single
branch barricade. Overhead architecture is excluded from ground collision. Flat
landing rings are decorative and do not provide elevated teleport surfaces.

Nine landing decorations form a connected graph with hops <=10m. Keep the central
X[-2,2], Z[1,-20] lane clear. At Z=-25 a 3.6 x1.4 x0.9m barricade splits routes;
side aisles reconnect at the final landing near Z=-30. Monster placements are open
ground suggestions, independent of the geometry. Gameplay/nav integration belongs
to the example, not the asset. Blender renders establish geometry appearance only,
not engine lighting parity or headset performance.
