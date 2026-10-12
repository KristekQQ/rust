#!/usr/bin/env python3
"""Offline geometry-only conversion of the McGuire archive's Sibenik OBJ ZIP.
Usage: python3 scripts/convert-sibenik.py /tmp/sibenik.zip output.glb
No textures, material maps, scene geometry or runtime logic are generated.
"""
import argparse
import hashlib
import json
import math
import struct
import zipfile
from pathlib import Path


def convert(archive, destination):
    with zipfile.ZipFile(archive) as package:
        source = package.read('sibenik.obj').decode('utf-8')
    positions, vertices, normals, indices, lookup = [], [], [], [], {}
    material = ""
    omitted_glass_faces = 0
    for line in source.splitlines():
        fields = line.split()
        if not fields:
            continue
        if fields[0] == 'v':
            positions.append(tuple(map(float, fields[1:4])))
        elif fields[0] == 'usemtl':
            material = fields[1]
        elif fields[0] == 'f':
            # The opaque-only preview must keep clear/stained window openings open.
            if material.startswith('staklo'):
                omitted_glass_faces += 1
                continue
            corners = []
            for token in fields[1:]:
                raw = int(token.split('/')[0])
                idx = raw - 1 if raw > 0 else len(positions) + raw
                if not 0 <= idx < len(positions):
                    raise ValueError('Invalid OBJ position index')
                corners.append(idx)
            if len(corners) not in (3, 4):
                raise ValueError('Source must contain only triangles or quads')
            a, b, c = [positions[i] for i in corners[:3]]
            u = [b[i] - a[i] for i in range(3)]
            v = [c[i] - a[i] for i in range(3)]
            n = (u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0])
            length = math.sqrt(sum(x*x for x in n))
            if length < 1e-12:
                continue
            if len(corners) == 4:
                # This archive includes one concave quad whose internal diagonal
                # is still 0 -> 2. Reject source variants with an invalid fan.
                d = positions[corners[3]]
                u = [c[i] - a[i] for i in range(3)]
                v = [d[i] - a[i] for i in range(3)]
                second = (u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0])
                # Source decimal rounding includes nearly coincident corners.
                if sum(second[i]*n[i] for i in range(3)) < -1e-8:
                    raise ValueError('Source quad requires a different triangulation diagonal')
            n = tuple(x/length for x in n)
            face = []
            for idx in corners:
                key = (idx, n)
                if key not in lookup:
                    lookup[key] = len(vertices)
                    vertices.append(positions[idx])
                    normals.append(n)
                face.append(lookup[key])
            for i in range(1, len(face)-1):
                indices.extend((face[0], face[i], face[i+1]))
    minimum = [min(p[i] for p in vertices) for i in range(3)]
    maximum = [max(p[i] for p in vertices) for i in range(3)]
    points = b''.join(struct.pack('<3f', *p) for p in vertices)
    shading = b''.join(struct.pack('<3f', *n) for n in normals)
    elements = struct.pack(f'<{len(indices)}I', *indices)
    binary = points + shading + elements
    document = {
        'asset': {'version': '2.0', 'generator': 'scripts/convert-sibenik.py',
                  'copyright': 'Marko Dabrovic; corrections Kenzie Lamar; archive Morgan McGuire. CC BY-NC.'},
        'buffers': [{'byteLength': len(binary)}],
        'bufferViews': [
            {'buffer': 0, 'byteOffset': 0, 'byteLength': len(points), 'target': 34962},
            {'buffer': 0, 'byteOffset': len(points), 'byteLength': len(shading), 'target': 34962},
            {'buffer': 0, 'byteOffset': len(points)+len(shading), 'byteLength': len(elements), 'target': 34963}],
        'accessors': [
            {'bufferView': 0, 'componentType': 5126, 'count': len(vertices), 'type': 'VEC3', 'min': minimum, 'max': maximum},
            {'bufferView': 1, 'componentType': 5126, 'count': len(normals), 'type': 'VEC3'},
            {'bufferView': 2, 'componentType': 5125, 'count': len(indices), 'type': 'SCALAR'}],
        'materials': [{'name': 'Neutral geometry preview', 'pbrMetallicRoughness': {'baseColorFactor': [.65,.65,.65,1]}}],
        'meshes': [{'name': 'Sibenik Cathedral geometry', 'primitives': [{'attributes': {'POSITION': 0, 'NORMAL': 1}, 'indices': 2, 'material': 0, 'mode': 4}]}],
        'nodes': [{'mesh': 0}], 'scenes': [{'nodes': [0]}], 'scene': 0}
    header = json.dumps(document, separators=(',', ':')).encode()
    header += b' ' * (-len(header) % 4)
    binary += b'\0' * (-len(binary) % 4)
    glb = struct.pack('<III', 0x46546c67, 2, 28+len(header)+len(binary))
    glb += struct.pack('<II', len(header), 0x4e4f534a) + header
    glb += struct.pack('<II', len(binary), 0x004e4942) + binary
    Path(destination).write_bytes(glb)
    print(json.dumps({'omitted_glass_faces': omitted_glass_faces, 'vertices': len(vertices), 'triangles': len(indices)//3, 'bounds': [minimum, maximum], 'bytes': len(glb), 'sha256': hashlib.sha256(glb).hexdigest()}, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive', type=Path)
    parser.add_argument('destination', type=Path)
    args = parser.parse_args()
    convert(args.archive, args.destination)
