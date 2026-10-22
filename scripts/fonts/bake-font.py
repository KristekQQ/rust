"""Blender background script: bake authorized TTF/OTF outlines to mesh-font caps.

Blender --background --python bake-font.py -- --font FONT --output RAW.json --license LICENSE
Solidify the intermediate JSON with solidify-font.py. No font license is implied.
"""
import argparse
import json
from pathlib import Path
import struct
import sys
import unicodedata

import bpy
import bmesh

args = argparse.ArgumentParser()
args.add_argument('--font', required=True)
args.add_argument('--output', required=True, help='Intermediate outline mesh JSON, outside versioned assets')
args.add_argument('--name', help='Display name; defaults to source filename stem')
args.add_argument('--license', required=True, help='Original font license label, preserved in the asset')
args.add_argument('--author', default='')
args.add_argument('--source', default='')
args.add_argument('--depth', type=float, default=0.13)
args.add_argument('--characters', help='Optional exact character subset; rejects missing characters')
options = args.parse_args(sys.argv[sys.argv.index('--') + 1:])
font_path = Path(options.font).resolve()
if not 0.001 <= options.depth <= 1:
    raise ValueError('Font depth must be in 0.001..1 font units')
font = bpy.data.fonts.load(str(font_path))


def characters(path):
    """Read Unicode BMP mappings directly from the OpenType cmap format 4 table."""
    data = path.read_bytes()
    count = struct.unpack_from('>H', data, 4)[0]
    tables = {data[12+i*16:16+i*16]: struct.unpack_from('>II', data, 20+i*16) for i in range(count)}
    base, _ = tables[b'cmap']
    result = set()
    for i in range(struct.unpack_from('>H', data, base+2)[0]):
        platform, encoding, offset = struct.unpack_from('>HHI', data, base+4+i*8)
        if platform not in (0, 3):
            continue
        start = base + offset
        fmt = struct.unpack_from('>H', data, start)[0]
        if fmt == 12:
            groups = struct.unpack_from('>I', data, start+12)[0]
            for group in range(groups):
                first, last, first_glyph = struct.unpack_from('>III', data, start+16+12*group)
                for code in range(max(32, first), min(last+1, 0xFFFF)):
                    if not 0xE000 <= code <= 0xF8FF and unicodedata.category(chr(code)) != 'Cc' and first_glyph + code - first > 0:
                        result.add(chr(code))
            continue
        if fmt != 4:
            continue
        segments = struct.unpack_from('>H', data, start+6)[0] // 2
        ends = struct.unpack_from('>'+str(segments)+'H', data, start+14)
        starts = struct.unpack_from('>'+str(segments)+'H', data, start+16+2*segments)
        deltas = struct.unpack_from('>'+str(segments)+'h', data, start+16+4*segments)
        offsets_base = start + 16 + 6*segments
        ranges = struct.unpack_from('>'+str(segments)+'H', data, offsets_base)
        for segment, (first, last) in enumerate(zip(starts, ends)):
            for code in range(first, min(last+1, 0xFFFF)):
                if ranges[segment]:
                    address = offsets_base + segment*2 + ranges[segment] + (code-first)*2
                    glyph = struct.unpack_from('>H', data, address)[0]
                    if glyph:
                        glyph = (glyph + deltas[segment]) & 65535
                else:
                    glyph = (code + deltas[segment]) & 65535
                if glyph and code >= 32 and not 0xE000 <= code <= 0xF8FF and unicodedata.category(chr(code)) != 'Cc':
                    result.add(chr(code))
    return sorted(result, key=ord)


bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
glyphs = {}
available = characters(font_path)
selected = sorted(set(options.characters), key=ord) if options.characters is not None else available
missing = set(selected) - set(available)
if missing:
    raise ValueError(f'Characters absent from source font: {sorted(missing)!r}')
if not selected or len(selected) > 2048:
    raise ValueError('Select 1..2048 characters with --characters for large fonts')
for char in selected:
    curve = bpy.data.curves.new('Glyph', 'FONT')
    curve.font = font
    curve.body = char
    curve.size = 1
    curve.resolution_u = 6
    curve.extrude = options.depth / 2
    curve.fill_mode = 'BOTH'
    obj = bpy.data.objects.new('Glyph', curve)
    bpy.context.collection.objects.link(obj)
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.context.view_layer.update()
    # Measure the advance from a sentinel pair, preserving original side bearings.
    curve.body = char + '|'
    bpy.context.view_layer.update()
    pair_right = max(corner[0] for corner in obj.bound_box)
    curve.body = '|'
    bpy.context.view_layer.update()
    advance = pair_right - max(corner[0] for corner in obj.bound_box)
    curve.body = char
    bpy.context.view_layer.update()
    if char.isspace():
        glyphs[char] = {'advance': round(max(advance, 0.1), 6), 'positions': [], 'indices': [], 'front': []}
        bpy.data.objects.remove(obj, do_unlink=True)
        bpy.data.curves.remove(curve)
        continue
    bpy.ops.object.convert(target='MESH')
    mesh = obj.data
    bm = bmesh.new()
    bm.from_mesh(mesh)
    bmesh.ops.remove_doubles(bm, verts=list(bm.verts), dist=0.000001)
    bmesh.ops.dissolve_degenerate(bm, edges=list(bm.edges), dist=0.00000001)
    bmesh.ops.triangulate(bm, faces=list(bm.faces))
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
    # Text caps can contain zero-area tessellation slivers; remove only those.
    bmesh.ops.delete(bm, geom=[f for f in bm.faces if f.calc_area() < 1e-12], context='FACES_ONLY')
    bm.to_mesh(mesh)
    bm.free()
    mesh.update()
    if mesh.polygons:
        positions = [round(float(c), 6) for vertex in mesh.vertices for c in vertex.co]
        indices = [index for face in mesh.polygons for index in face.vertices]
        front = [1 if face.normal.z > 0.9 else 0 for face in mesh.polygons]
        glyphs[char] = {'advance': round(max(advance, 0.1), 6), 'positions': positions,
                        'indices': indices, 'front': front}
        if len(indices) > 60000:
            raise RuntimeError(f'Glyph {char!r} exceeds engine mesh limit')
    bpy.data.objects.remove(obj, do_unlink=True)
    bpy.data.meshes.remove(mesh)

output = Path(options.output).resolve()
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps({'format':'mesh-font','version':1,'font':options.name or font_path.stem,
    'author':options.author,'source':options.source,'license':options.license,
    'depth':options.depth,'glyphs':glyphs}, ensure_ascii=False, separators=(',', ':')))
print(f'Baked {len(glyphs)} glyphs into {output} ({output.stat().st_size:,} bytes)')
