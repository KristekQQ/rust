# Reusable 3D fonts

Rust owns font validation, glyph assets, text layout, transforms and rendering.
The JS SDK exposes IDs and commands. No font layout, per-character object arrays
or animation loop is required in an application.

## Existing text paths

| Path | Purpose | API |
| --- | --- | --- |
| Pixel bitmap | Existing HUD and world-space information panels | `createInfoPanel`, `createTextPanel` |
| Neon tubes | Existing shared curved 3D digits, countdown and score; specialized continuous core shader | `createNumericText`, countdown/score panels |
| General mesh fonts | Imported vector-font geometry and the existing pixel alphabet extruded into solids | `loadFont3D`, `builtinFont3D`, `Font3D.createText` |

The numeric tube material remains available. Neon arena defaults to Cynatar and
offers a selector to restore its original neon digits.
`scene.builtinFont3D('pixel')` reuses `debug_overlay::glyph`; it does not bundle a
second bitmap alphabet. It supports uppercase A–Z, 0–9, space and `./+-`.

## Using an imported font

```js
const font = await engine.scene.loadFont3D(
    new URL('../../assets/fonts/cynatar/glyphs.json', import.meta.url));
const label = font.createText('CYNATAR', {
    position: [0, 1.7, 0], size: 1.05, maxWidth: 6.6,
    align: 'center', letterSpacing: 0,
    color: [.02, .8, 1], sideIntensity: .32,
}).setEmission(4);
engine.setNeonGlow({strength: .4, radius: .045});
label.setText('MODELAR');
label.setOptions({color: [1, .035, .65]});
label.setPosition([1, 1.7, 0]).setRotation([0, .4, 0]);
```

All glyph children inherit the root's position, rotation, scale, emission and
render options. Their IDs are internal; remove the text root, not individual
glyphs. Default alignment is center, baseline Y=0 and front is +Z. The desktop
left-handed orbit view uses `.setScale([-1,1,1])` in the example; apply the actual
world orientation for XR rather than assuming a desktop mirror is correct there.

`size` scales font units. `maxWidth` only shrinks the whole line uniformly;
advances preserve spacing. `letterSpacing` adds font-unit spacing between code
points. `maxWidth: null` removes a previous fit constraint. This implementation
uses one line, with no shaping, kerning pairs, ligature substitution or fallback
font. Unsupported characters throw before replacing the existing text. Empty
text and spaces are supported without visible geometry.

`Font3D.info` reports supported characters, source depth/license and live/cache
counts. `Text3D.info` reports the original text and resulting dimensions/options.
These are explicit diagnostic snapshots, not required per-frame reads.

`registerFont3D(data)` accepts the same baked schema from an already available
object. `loadFont3D` only transports JSON, bounded to 16 MiB, with UTF-8 validation
and cancellation on `scene.clear()`/dispose; Rust parses and validates the asset.
It does not parse TTF/OTF at runtime.

## Native information panels

Bind a font to a single-value countdown, score or numeric information panel:

```js
panel.setFont3D(font, {color: [.02, .8, 1], sideIntensity: .32});
console.log(panel.font3DInfo.text);
panel.setFont3D(null); // restore the original font with the current value
```

Rust updates the bound glyphs when the panel value changes, fits ink into its
existing dimensions and keeps its root ID, transforms and material. No JS polling
is needed. Hidden legacy glyph geometry is not regenerated while bound.
The arena selector applies this to time, total score, combo award and both stock
scores. `?font=neon` selects the original style on load.

## Sharing and lifetime

Each `(font, glyph, color, sideIntensity)` has one shared immutable renderer
asset. Repeated letters use the existing GPU instance pipeline. There are no
per-instance glyph vertex buffers. Layout/material changes invalidate only the
text involved; unchanged frame synchronization skips child updates. The standard
neon mesh-edge shader supplies glow; no font-specific full-screen pass is added.
Color styles can require separate assets; unused style assets are released.

`label.remove()` removes its glyph instances. `font.remove()` prevents new texts
but preserves existing labels until they are removed. `scene.clear()` drops all
texts/fonts; old IDs cannot alias new resources. `engine.dispose()` also cancels
pending transport and rejects old SDK handles.

Measure rather than infer GPU savings:

```js
const stats = engine.debugStats().font3d;
// runtime: fonts, texts, cachedGlyphs, liveGlyphs, layoutRebuilds, syncUpdates
// geometryBytes: shared uploaded glyph geometry
// instanceGeometryBytes: should be zero
const draws = engine.renderer_stats().drawCalls;
```

Draws depend on visible unique glyph/style/pass/winding combinations, not only
text length. Different letters are not automatically one atlas draw. These
counters are not GPU timings or Quest FPS evidence.

## Importing another TTF/OTF

Use a font whose license permits the intended use and preserve its attribution.
Cynatar is **personal use only**, including derived meshes. It is not relicensed
by the engine; do not publish it as a freely reusable font asset. Built-in pixel
and numeric glyphs have no dependency on Cynatar.

The reusable importer runs offline in Blender and Python with `shapely==2.1.2`:

```sh
Blender --background --python-exit-code 1 --python scripts/fonts/bake-font.py -- \
  --font /path/Font.ttf --name 'Font Name' \
  --license 'Original license' --author 'Original author' \
  --source 'Original source URL' --depth .13 \
  --output output/font-import/raw.json
python scripts/fonts/solidify-font.py output/font-import/raw.json \
  --output assets/fonts/font-name/glyphs.json
```

For a large font choose `--characters 'ABCXYZ0123456789 '` explicitly; missing
characters fail instead of using Blender fallback glyphs. The importer reads
Unicode cmap formats 4/12 (BMP, excluding private-use characters) and supports
up to 2048 selected glyphs. It preserves measured advances. It unions filled
outline regions, repairs sub-pixel pinches, constrained-triangulates caps while
preserving holes and extrudes closed, consistently oriented front/back/sides.
No raster tracing or texture-backed text is involved.

Schema (optional `format`/`version` preserve compatibility with the first Cynatar
asset):

```json
{
  "format": "mesh-font", "version": 1,
  "font": "Font Name", "depth": 0.13, "license": "Original license",
  "glyphs": {
    "A": {"advance": 0.6, "positions": [], "indices": [], "front": []}
  }
}
```

The arrays in the example only illustrate the schema. Actual visible glyphs
need finite xyz positions, closed outward triangle indices and one 0/1 front
flag per triangle. Whitespace can use empty arrays with a positive advance.
Rust checks UTF-8 JSON, version, glyph keys, geometry, dimensions and budgets.
Current limits: 64 font handles, 2048 glyphs/font, 16 MiB/font JSON, one million
triangles/font (also subject to per-glyph mesh limits), 1024 text roots and
32768 live glyph instances/scene. Text has at most 1024 code points/8192 UTF-8
bytes; options JSON is bounded to 4096 bytes.

## Verification

During iteration use `cargo test font3d --lib`, SDK tests and the actual font
example on both backends, with a matching WASM build:

```sh
npm run verify:browser -- --page cynatar,neon-font-preview,scene-sdk,lifecycle --backend all --capture
```

The font acceptance checks actual imported geometry, a hole in O, side emission,
glow, both colors, transforms, layout changes, builtin switching, shared GPU
geometry, stable-frame counters, actual draw calls and cleanup. It is desktop
acceptance; use a headset run before claiming Quest appearance or performance.
