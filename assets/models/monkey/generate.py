#!/usr/bin/env python3
"""Generate an original stylized monkey as portable glTF 2.0, no dependencies.

Y is up, face points +Z, feet rest at Y=0. Run from any working directory.
Parts are intersecting smooth closed meshes, suitable for realtime display.
"""
import json
import math
from pathlib import Path
import struct

blob = bytearray()
doc = {"asset": {"version": "2.0", "generator": "Modelar procedural monkey"},
       "scene": 0, "scenes": [{"nodes": [0]}], "nodes": [{"name": "Monkey", "children": []}],
       "meshes": [], "materials": [], "accessors": [], "bufferViews": []}


def accessor(values, kind, components, bounds=False):
    while len(blob) % 4:
        blob.append(0)
    offset = len(blob)
    flat = [v for row in values for v in row] if components > 1 else values
    blob.extend(struct.pack("<" + ("f" if kind == 5126 else "H") * len(flat), *flat))
    view = len(doc["bufferViews"])
    doc["bufferViews"].append({"buffer": 0, "byteOffset": offset, "byteLength": len(blob) - offset})
    item = {"bufferView": view, "componentType": kind, "count": len(values),
            "type": {1: "SCALAR", 3: "VEC3", 4: "VEC4"}[components]}
    if bounds:
        rows = values if components > 1 else [[v] for v in values]
        item["min"] = [min(r[c] for r in rows) for c in range(components)]
        item["max"] = [max(r[c] for r in rows) for c in range(components)]
    doc["accessors"].append(item)
    return len(doc["accessors"]) - 1


def geometry(positions, normals, indices):
    return {"attributes": {"POSITION": accessor(positions, 5126, 3, True),
                           "NORMAL": accessor(normals, 5126, 3)},
            "indices": accessor(indices, 5123, 1)}


def sphere():
    positions, indices = [], []
    rows, cols = 20, 32
    for j in range(rows + 1):
        t = math.pi * j / rows
        for i in range(cols + 1):
            p = math.tau * i / cols
            positions.append([math.sin(t) * math.cos(p), math.cos(t), math.sin(t) * math.sin(p)])
    for j in range(rows):
        for i in range(cols):
            a = j * (cols + 1) + i
            b = a + cols + 1
            if j > 0:
                indices.extend([a, a + 1, b])
            if j < rows - 1:
                indices.extend([a + 1, b + 1, b])
    return geometry(positions, positions, indices)


unit = sphere()
palette = {
    "fur": [0.26, 0.095, 0.037, 1], "face": [0.72, 0.43, 0.22, 1],
    "belly": [0.52, 0.27, 0.105, 1], "ear": [0.52, 0.22, 0.15, 1],
    "dark": [0.025, 0.012, 0.009, 1], "white": [0.98, 0.91, 0.73, 1],
}
meshes = {}
for name, color in palette.items():
    material = len(doc["materials"])
    doc["materials"].append({"name": name, "pbrMetallicRoughness": {
        "baseColorFactor": color, "metallicFactor": 0, "roughnessFactor": 0.72 if name == "dark" else 0.92}})
    meshes[name] = len(doc["meshes"])
    doc["meshes"].append({"name": name, "primitives": [{**unit, "material": material}]})


def part(name, position, scale, material="fur", parent=0, rotation=None):
    node = {"name": name, "mesh": meshes[material], "translation": position, "scale": scale}
    if rotation:
        node["rotation"] = rotation
    index = len(doc["nodes"])
    doc["nodes"].append(node)
    doc["nodes"][parent].setdefault("children", []).append(index)
    return index


def group(name, position, parent=0):
    index = len(doc["nodes"])
    doc["nodes"].append({"name": name, "translation": position, "children": []})
    doc["nodes"][parent].setdefault("children", []).append(index)
    return index


def limb(name, a, b, radius, material="fur", parent=0):
    delta = [b[i] - a[i] for i in range(3)]
    length = math.sqrt(sum(v * v for v in delta))
    direction = [v / length for v in delta]
    # Quaternion from +Y to the segment direction.
    q = [direction[2], 0, -direction[0], 1 + direction[1]]
    norm = math.sqrt(sum(v * v for v in q))
    q = [v / norm for v in q] if norm > 1e-8 else [1, 0, 0, 0]
    return part(name, [(a[i] + b[i]) / 2 for i in range(3)],
                [radius, length / 2 + radius * 0.35, radius], material, parent, q)


part("Body", [0, 0.94, 0], [0.57, 0.75, 0.39])
part("Belly", [0, 0.91, 0.31], [0.40, 0.52, 0.13], "belly")
for side in [-1, 1]:
    part("Haunch", [side * 0.48, 0.43, 0], [0.32, 0.40, 0.32])
    part("Foot", [side * 0.43, 0.14, 0.37], [0.27, 0.14, 0.40], "face")
    for digit in range(3):
        part("Toe", [side * 0.43 + (digit - 1) * 0.13, 0.12, 0.68], [0.075, 0.09, 0.14], "face")

head = group("Head pivot", [0, 1.87, 0.01])
part("Head", [0, 0, 0], [0.68, 0.64, 0.51], parent=head)
for side in [-1, 1]:
    part("Ear", [side * 0.74, 0.03, -0.03], [0.32, 0.36, 0.17], parent=head)
    part("Inner ear", [side * 0.77, 0.04, 0.10], [0.22, 0.25, 0.075], "ear", head)
    part("Face lobe", [side * 0.235, 0.03, 0.39], [0.33, 0.40, 0.16], "face", head)
    part("Eye white", [side * 0.245, 0.13, 0.531], [0.155, 0.19, 0.067], "white", head)
    part("Eye", [side * 0.235, 0.115, 0.585], [0.086, 0.115, 0.045], "dark", head)
    part("Eye glint", [side * 0.235 - 0.026, 0.16, 0.619], [0.027, 0.035, 0.013], "white", head)
    part("Brow", [side * 0.245, 0.35, 0.44], [0.19, 0.075, 0.08], parent=head)
part("Muzzle", [0, -0.23, 0.49], [0.37, 0.23, 0.23], "face", head)
part("Nose", [0, -0.115, 0.69], [0.12, 0.075, 0.05], "fur", head)
for side in [-1, 1]:
    part("Nostril", [side * 0.047, -0.12, 0.732], [0.021, 0.017, 0.01], "dark", head)
# A small curved smile, built from a continuous sequence of rounded segments.
for i in range(12):
    x1, x2 = -0.19 + i * 0.38 / 12, -0.19 + (i + 1) * 0.38 / 12
    limb("Smile", [x1, -0.355 + 1.6 * x1 * x1, 0.678],
         [x2, -0.355 + 1.6 * x2 * x2, 0.678], 0.012, "dark", head)
for x, y, radius in [(-0.20, 0.55, 0.19), (0, 0.63, 0.18), (0.17, 0.56, 0.15)]:
    part("Hair tuft", [x, y, -0.03], [radius, radius * 1.4, radius * 0.75], parent=head)

limb("Resting upper arm", [-0.46, 1.30, 0], [-0.77, 0.90, 0.12], 0.18)
limb("Resting forearm", [-0.77, 0.90, 0.12], [-0.68, 0.47, 0.46], 0.15)
part("Resting hand", [-0.68, 0.43, 0.48], [0.19, 0.18, 0.14], "face")
for i in range(4):
    part("Resting finger", [-0.81 + i * 0.085, 0.31, 0.52], [0.046, 0.13, 0.065], "face")
arm = group("Waving shoulder", [0.48, 1.32, 0])
limb("Raised upper arm", [0, 0, 0], [0.38, 0.29, 0.02], 0.17, parent=arm)
limb("Raised forearm", [0.38, 0.29, 0.02], [0.48, 0.75, 0.06], 0.14, parent=arm)
part("Waving palm", [0.48, 0.87, 0.07], [0.18, 0.20, 0.11], "face", arm)
for i in range(4):
    limb("Waving finger", [0.35 + i * 0.087, 0.98, 0.07],
         [0.30 + i * 0.12, 1.20 - abs(i - 1.5) * 0.05, 0.07], 0.043, "face", arm)
limb("Thumb", [0.33, 0.84, 0.07], [0.20, 0.97, 0.07], 0.060, "face", arm)

# Rounded tail segments follow a smooth curl behind the body, visible in side views.
points = [[0, 0.50, -0.26], [0.15, 0.35, -0.62], [0.45, 0.20, -0.86], [0.8, 0.20, -0.91]]
for i in range(49):
    t = i / 48 * math.pi * 1.75
    r = 0.43 * (1 - 0.60 * i / 48)
    points.append([0.80 + r * math.sin(t), 0.63 - r * math.cos(t), -0.91])
positions, normals, indices = [], [], []
for i, point in enumerate(points):
    a, b = points[max(0, i - 1)], points[min(len(points) - 1, i + 1)]
    tangent = [b[c] - a[c] for c in range(3)]
    length = math.sqrt(sum(v * v for v in tangent))
    tangent = [v / length for v in tangent]
    u = [tangent[1], -tangent[0], 0]
    length = math.sqrt(sum(v * v for v in u))
    u = [v / length for v in u]
    v = [tangent[1] * u[2] - tangent[2] * u[1],
         tangent[2] * u[0] - tangent[0] * u[2],
         tangent[0] * u[1] - tangent[1] * u[0]]
    for j in range(12):
        angle = j / 12 * math.tau
        normal = [u[c] * math.cos(angle) + v[c] * math.sin(angle) for c in range(3)]
        radius = 0.08 - 0.025 * i / len(points)
        positions.append([point[c] + radius * normal[c] for c in range(3)])
        normals.append(normal)
        if i < len(points) - 1:
            p, q = i * 12 + j, i * 12 + (j + 1) % 12
            indices.extend([p, q, p + 12, q, q + 12, p + 12])
for i, normal in [(0, [0, 0, 1]), (len(points) - 1, [0, 0, -1])]:
    center = len(positions)
    positions.append(points[i])
    normals.append(normal)
    for j in range(12):
        a, b = i * 12 + j, i * 12 + (j + 1) % 12
        indices.extend([center, b, a] if i == 0 else [center, a, b])
tail_mesh = len(doc["meshes"])
doc["meshes"].append({"name": "Curled tail", "primitives": [{
    **geometry(positions, normals, indices), "material": meshes["fur"]}]})
tail = group("Tail", [0, 0, 0])
doc["nodes"][tail]["mesh"] = tail_mesh

times = accessor([0, 0.5, 1, 1.5, 2, 2.5, 3], 5126, 1, True)
samplers, channels = [], []
for node, angles, axis in [
        (arm, [0, 0.23, -0.17, 0.23, -0.17, 0.10, 0], 2),
        (head, [0, -0.08, -0.13, 0, 0.10, 0.06, 0], 1)]:
    rotations = []
    for angle in angles:
        q = [0, 0, 0, math.cos(angle / 2)]
        q[axis] = math.sin(angle / 2)
        rotations.append(q)
    samplers.append({"input": times, "output": accessor(rotations, 5126, 4), "interpolation": "LINEAR"})
    channels.append({"sampler": len(samplers) - 1, "target": {"node": node, "path": "rotation"}})
doc["animations"] = [{"name": "Hello", "samplers": samplers, "channels": channels}]
while len(blob) % 4:
    blob.append(0)
doc["buffers"] = [{"byteLength": len(blob)}]
encoded = json.dumps(doc, separators=(",", ":")).encode()
encoded += b" " * (-len(encoded) % 4)
payload = struct.pack("<III", 0x46546C67, 2, 28 + len(encoded) + len(blob))
payload += struct.pack("<II", len(encoded), 0x4E4F534A) + encoded
payload += struct.pack("<II", len(blob), 0x004E4942) + blob
target = Path(__file__).with_name("monkey.glb")
target.write_bytes(payload)
print(f"{target.name}: {len(payload):,} bytes, {len(doc['nodes'])} nodes, Hello animation")
