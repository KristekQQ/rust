# Šibenik Cathedral — geometry-only demonstration asset

Original model by **Marko Dabrovic**; holes corrected by **Kenzie Lamar** at Vicarious Visions. Downloaded from [Morgan McGuire's Computer Graphics Archive](https://casual-effects.com/data/).

- Official archive: https://casual-effects.com/g3d/data10/research/model/sibenik/sibenik.zip
- Official metadata: https://casual-effects.com/g3d/data10/research/model/sibenik/info.js
- The metadata identifies this asset as **CC BY-NC**, without a version. Treat this demonstration asset as **noncommercial**; do not include it in a commercial game without separate permission. This restriction belongs to this asset, not to the engine.
- Original attribution is preserved in `SOURCE-COPYRIGHT.txt`; original metadata is preserved in `SOURCE-METADATA.txt`. No license version or broader permission is inferred.
- Original ZIP SHA-256: `b7980306d30d4cc93deb3596082cfa63831b3902e51892e386f5e3721ed06e5c`.

## Conversion

`python3 scripts/convert-sibenik.py /tmp/sibenik.zip assets/models/sibenik-cathedral/sibenik-geometry.glb`

The original OBJ positions and face winding are retained. Quads are split into two triangles along their internal 0–2 diagonal (checked during conversion); 11 zero-area triangles are omitted. Face normals are computed and vertices split at normal discontinuities. All UVs, texture references, original materials and image data are discarded. The 2,760 faces assigned to `staklo*` (clear/stained glass) are omitted so the opaque-only preview preserves light-transmitting window apertures; stone frames and tracery remain. A uniform neutral color previews the construction under the engine's existing lighting. The neutral stone material uses metallic=0 and roughness=0.95 for the PBR renderer; no original texture maps or animation are added.

Result: **72,513 triangles, 145,857 vertices**, 4,371,796 bytes. Source coordinates and scale are retained; the JavaScript example applies the placement and uniform scale through the SDK.

GLB SHA-256: `099d80d2ef252316297313be8664c81c0277f35516acae588bb285ba057257c4`.
