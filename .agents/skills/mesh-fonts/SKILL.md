---
name: mesh-fonts
description: Add, import or tune reusable 3D mesh fonts and neon lettering in this Rust/WASM engine, including glyph instancing and side emission. Use existing numeric tubes or bitmap panels when they already fit the request.
---

# 3D mesh fonts

Read [FONTS.md](../../../docs/FONTS.md) for the API, portable baked schema,
converter commands, limits and diagnostics. Use the repository's
[rust-webgpu-project](../rust-webgpu-project/SKILL.md) workflow for code and checks.

Inspect the existing font paths before creating another: `src/info_panel.rs`
contains bitmap panels and specialized neon digits; `src/numeric_font.rs` shares
the numeric geometry; `src/font3d.rs` owns imported font assets and general text.
The existing bitmap alphabet is `debug_overlay::glyph`, exposed as the built-in
`pixel` mesh font. Preserve the dedicated numeric core shader and existing callers.

For new TTF/OTF sources, verify the font's license for the intended use. Preserve
source/author/license metadata and attribution; Cynatar and its derived geometry
remain personal-use-only. Use `scripts/fonts/bake-font.py` in Blender, followed by
`scripts/fonts/solidify-font.py` in Python with Shapely. Select supported characters
explicitly for large fonts. Test the converter; do not hand-wave missing glyphs,
holes, outward winding or closed sidewalls.

Applications load a font with `scene.loadFont3D`, then retain only Font3D/Text3D
handles. Rust owns spacing, fit/alignment, glyph caches, transforms and lifetime.
Do not rebuild a JS array of glyph objects or add an application RAF loop for text.
Use one text root for a string and ordinary root transforms/material commands.
Keep glyph children internal; remove the root or font through its public handle.

For existing single-value information panels use `panel.setFont3D(font, options)`;
`null` restores their original font. Keep native countdown/score updates rather
than polling them from JS. Inspect `panel.font3DInfo` only for diagnostics.

For neon, distinguish face emission, dimmer colored side emission and geometric
edge glow. Keep actual side surfaces visible in an oblique/edge-on view; the gray
side material from the initial Cynatar demo was a design choice, not an optimization
requirement. Tune `sideIntensity`, emission and existing `setNeonGlow` before adding
another shader/pass. Preserve holes and a visible sense of depth.

Verify actual Rust-rendered pixels, not DOM text or metadata alone. Cover front,
oblique and side views, glow on/off, unsupported characters, font/style changes and
clear/dispose. Check `debugStats().font3d`: repeated glyphs should share GPU geometry,
`instanceGeometryBytes` should stay zero, and unchanged frames should not rebuild
layouts or resync children. Inspect actual draw calls; different glyph/style/pass
combinations need not collapse into one draw. Desktop results do not prove Quest FPS.
