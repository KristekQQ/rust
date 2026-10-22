"""Convert Blender's planar font caps to manifold extruded glyphs.

Requires shapely==2.1.2. Repairs overlapping/touching original font contours in
2D, then constrained-triangulates actual filled regions, preserving holes.
Usage: python solidify-font.py RAW.json --output assets/fonts/name/glyphs.json
"""
from collections import Counter
import argparse
import json
from pathlib import Path

from shapely import constrained_delaunay_triangles, orient_polygons, set_precision
from shapely.geometry import Polygon
from shapely.ops import unary_union

parser = argparse.ArgumentParser()
parser.add_argument('input')
parser.add_argument('--output', required=True)
options = parser.parse_args()
data = json.loads(Path(options.input).read_text())
half_depth = data['depth'] / 2
total = 0
for char, glyph in data['glyphs'].items():
    source = glyph['positions']
    points = [source[i:i+3] for i in range(0, len(source), 3)]
    caps = []
    for i in range(0, len(glyph['indices']), 3):
        if glyph['front'][i//3]:
            face = Polygon([points[j][:2] for j in glyph['indices'][i:i+3]])
            if face.area > 1e-10:
                caps.append(face)
    if not caps:
        continue
    # A tiny expansion separates zero-width pinches where outline holes touch
    # the exterior; these are valid 2D glyphs but non-manifold after extrusion.
    shape = unary_union(caps).simplify(0.00005, preserve_topology=True).buffer(0.00001, join_style=2)
    shape = set_precision(shape, 0.000001)
    shape = orient_polygons(shape)
    polygons = [shape] if shape.geom_type == 'Polygon' else list(shape.geoms)
    vertices, indices, front = [], [], []
    for polygon in polygons:
        if polygon.geom_type != 'Polygon' or polygon.area < 1e-8:
            continue
        # Each disconnected component owns its own topology, even at touching points.
        lookup = {}

        def vertex(x, y, z):
            key = (round(x, 6), round(y, 6), z)
            if key not in lookup:
                lookup[key] = len(vertices) // 3
                vertices.extend(key)
            return lookup[key]

        def triangle(a, b, c, bright):
            indices.extend([a, b, c])
            front.append(bright)

        for face in constrained_delaunay_triangles(polygon).geoms:
            coords = list(face.exterior.coords)[:3]
            (ax, ay), (bx, by), (cx, cy) = coords
            if (bx-ax)*(cy-ay)-(by-ay)*(cx-ax) < 0:
                coords.reverse()
            top = [vertex(x, y, half_depth) for x, y in coords]
            bottom = [vertex(x, y, -half_depth) for x, y in coords]
            triangle(*top, 1)
            triangle(*reversed(bottom), 0)
        for ring in [polygon.exterior, *polygon.interiors]:
            for (ax, ay), (bx, by) in zip(ring.coords, list(ring.coords)[1:]):
                a, b = vertex(ax, ay, -half_depth), vertex(bx, by, -half_depth)
                c, d = vertex(bx, by, half_depth), vertex(ax, ay, half_depth)
                triangle(a, b, c, 0)
                triangle(a, c, d, 0)
    counts = Counter()
    directed = Counter()
    for a, b, c in zip(*[iter(indices)]*3):
        for edge in [(a, b), (b, c), (c, a)]:
            counts[tuple(sorted(edge))] += 1
            directed[edge] += 1
    if any(count != 2 for count in counts.values()) or any(directed[(b, a)] != count for (a, b), count in directed.items()):
        raise RuntimeError(f'Non-manifold or inconsistent winding: {char!r}')
    glyph.update(positions=vertices, indices=indices, front=front)
    total += len(indices)//3

target = Path(options.output).resolve()
target.parent.mkdir(parents=True, exist_ok=True)
target.write_text(json.dumps(data, ensure_ascii=False, separators=(',', ':')))
print(f'{len(data["glyphs"])} glyphs, {total:,} triangles, closed oriented geometry: {target}')
