# Cynatar — extruded glyph meshes

Original font: **Cynatar**, Leonard Posavec / LeoSupply.co.
Source and license: https://www.dafont.com/cynatar.font

**Free for personal use only.** This also applies to these derived glyph meshes.
Commercial use requires a license from the author. This local demonstration is
not covered by the engine's license. Do not publish the font/derived assets as
freely redistributable project assets. The original OTF is not bundled here.

`glyphs.json` contains 185 mapped Unicode characters, original advances and
closed, consistently oriented indexed meshes, extruded 0.13 font units. Front
triangle flags allow the example to use brighter fronts and dimmer colored sides.
Holes remain holes; disconnected strokes remain separate closed components.
Not all Czech diacritics exist in this font. No kerning pairs are baked.

The example uses Rust CPU triangle meshes and `setNeonGlow`, the same geometric
glow as the arena. It uses no DOM text, billboard texture or separate renderer
for the displayed 3D letters.

Regeneration (Blender 5.3 and Python with `shapely==2.1.2`):

```sh
Blender --background --python-exit-code 1 --python scripts/fonts/bake-font.py -- \
  --font /path/to/Cynatar.otf --name Cynatar --license 'Free for personal use only' \
  --author 'Leonard Posavec / LeoSupply.co' --source 'https://www.dafont.com/cynatar.font' \
  --output output/cynatar/raw.json
python scripts/fonts/solidify-font.py output/cynatar/raw.json --output assets/fonts/cynatar/glyphs.json
```

Blender reads the actual OTF curves. The second step unions the filled 2D caps,
removes sub-pixel slivers, separates point pinches by 0.00001 font units,
constrained-triangulates the regions and builds front/back/side surfaces. It
checks that every edge has exactly two opposite uses before writing the asset.

Runtime API and shared glyph lifetime: [FONTS.md](../../../docs/FONTS.md).
