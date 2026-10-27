import * as THREE from './vendor/three.module.js';

// Prepared inputs are immutable. Attribute versions invalidate edited geometry;
// callers should prepare loaded originals before interaction, never on first hit.
const prepared = new WeakMap();
const attributeVersion = attribute => attribute?.isInterleavedBufferAttribute ? attribute.data.version : attribute?.version;
export function prepareGeometry(geometry) {
  const position = geometry.getAttribute('position');
  if (!position) return null;
  const normal = geometry.getAttribute('normal'), uv = geometry.getAttribute('uv'), index = geometry.index;
  const old = prepared.get(geometry);
  if (old && old.position === position && old.normal === normal && old.uv === uv && old.index === index
    && old.versions[0] === attributeVersion(position) && old.versions[1] === attributeVersion(normal)
    && old.versions[2] === attributeVersion(uv) && old.versions[3] === attributeVersion(index)
    && old.groups.length === geometry.groups.length && old.groups.every((g,i) =>
      g.start === geometry.groups[i].start && g.count === geometry.groups[i].count && g.materialIndex === geometry.groups[i].materialIndex)) return old;
  const vertices = Array.from({length: position.count}, (_, i) => ({
    p: new THREE.Vector3().fromBufferAttribute(position, i), d: 0,
    n: normal ? new THREE.Vector3().fromBufferAttribute(normal, i) : null,
    uv: uv ? new THREE.Vector2().fromBufferAttribute(uv, i) : null
  }));
  const count = index ? index.count : position.count, triangles = [];
  for (let offset = 0; offset + 2 < count; offset += 3) {
    triangles.push({vertices: [0, 1, 2].map(j => vertices[index ? index.getX(offset + j) : offset + j]),
      material: geometry.groups.find(g => offset >= g.start && offset < g.start + g.count)?.materialIndex ?? 0});
  }
  const value = {position, normal, uv, index, vertices, triangles,
    bounds: (geometry.boundingBox || new THREE.Box3().setFromBufferAttribute(position)).clone(),
    groups: geometry.groups.map(g => ({...g})),
    versions: [position, normal, uv, index].map(attributeVersion)};
  prepared.set(geometry, value);
  return value;
}

// Called only after a hit. All points and the cutting plane are mesh-local.
// Input geometry remains shared; only geometries listed in owned may be disposed.
export function splitGeometry(geometry, plane, capMaterialIndex = 0) {
  const position = geometry.getAttribute('position');
  if (!position || !position.count) return {positive: geometry, negative: null, owned: [], capTriangles: 0};
  const cut = plane.clone().normalize();
  const input = prepareGeometry(geometry);
  const bounds = input.bounds;
  const scale = bounds.getSize(new THREE.Vector3()).length();
  const epsilon = Math.max(scale * 1e-7, 1e-9);
  const {x: nx, y: ny, z: nz} = cut.normal;
  const minDistance = nx * (nx >= 0 ? bounds.min.x : bounds.max.x)
    + ny * (ny >= 0 ? bounds.min.y : bounds.max.y)
    + nz * (nz >= 0 ? bounds.min.z : bounds.max.z) + cut.constant;
  const maxDistance = nx * (nx >= 0 ? bounds.max.x : bounds.min.x)
    + ny * (ny >= 0 ? bounds.max.y : bounds.min.y)
    + nz * (nz >= 0 ? bounds.max.z : bounds.min.z) + cut.constant;
  if (minDistance >= -epsilon) return {positive: geometry, negative: null, owned: [], capTriangles: 0};
  if (maxDistance < -epsilon) return {positive: null, negative: geometry, owned: [], capTriangles: 0};
  let hasPositive = false, hasNegative = false;
  for (let i = 0; i < position.count; i++) {
    const d = nx * position.getX(i) + ny * position.getY(i) + nz * position.getZ(i) + cut.constant;
    hasPositive ||= d > epsilon;
    hasNegative ||= d < -epsilon;
    input.vertices[i].d = Math.abs(d) <= epsilon ? 0 : d;
  }
  if (!hasNegative) return {positive: geometry, negative: null, owned: [], capTriangles: 0};
  if (!hasPositive) return {positive: null, negative: geometry, owned: [], capTriangles: 0};

  const normal = geometry.getAttribute('normal'), uv = geometry.getAttribute('uv');
  const sides = [[], []], segments = [], planeEdges = [];
  let intersections;
  const intersection = (a, b) => {
    const cached = intersections?.find(record => (record.a === a && record.b === b) || (record.a === b && record.b === a));
    if (cached) return cached.vertex;
    const t = a.d / (a.d - b.d);
    const vertex = {p: a.p.clone().lerp(b.p, t), d: 0,
      n: normal ? a.n.clone().lerp(b.n, t).normalize() : null,
      uv: uv ? a.uv.clone().lerp(b.uv, t) : null};
    intersections.push({a, b, vertex});
    return vertex;
  };
  const clip = (vertices, sign) => {
    const result = [];
    for (let j = 0; j < vertices.length; j++) {
      const a = vertices[j], b = vertices[(j + 1) % vertices.length];
      const insideA = sign * a.d >= 0, insideB = sign * b.d >= 0;
      if (insideA) result.push(a);
      if (insideA !== insideB) result.push(intersection(a, b));
    }
    return result;
  };
  const areaA = new THREE.Vector3(), areaB = new THREE.Vector3();
  for (const {vertices, material} of input.triangles) {
    intersections = [];
    const plus = vertices.some(v => v.d > 0), minus = vertices.some(v => v.d < 0);
    for (let side = 0; side < 2; side++) {
      if (side === 1 && !minus) continue;
      if (side === 0 && !plus && minus) continue;
      const polygon = plus && minus ? clip(vertices, side === 0 ? 1 : -1) : vertices;
      for (let j = 1; j + 1 < polygon.length; j++) {
        const tri = [polygon[0], polygon[j], polygon[j + 1]];
        if (areaA.subVectors(tri[1].p, tri[0].p).cross(areaB.subVectors(tri[2].p, tri[0].p)).lengthSq() > epsilon ** 4)
          sides[side].push({vertices: tri, material});
      }
    }
    if (plus !== minus) {
      const onPlane = vertices.filter(vertex => vertex.d === 0);
      if (onPlane.length === 2) planeEdges.push({points: onPlane.map(vertex => vertex.p), sign: plus ? 1 : -1});
    }
    if (plus && minus) {
      const points = [];
      for (let j = 0; j < 3; j++) {
        const a = vertices[j], b = vertices[(j + 1) % 3];
        if (a.d === 0) points.push(a.p);
        if (a.d * b.d < 0) points.push(intersection(a, b).p);
      }
      const unique = points.filter((p, i) => !points.slice(0, i).some(q => p.distanceToSquared(q) <= epsilon ** 2));
      if (unique.length === 2) {
        const direction = vertices[1].p.clone().sub(vertices[0].p).cross(vertices[2].p.clone().sub(vertices[0].p)).cross(cut.normal);
        if (unique[1].clone().sub(unique[0]).dot(direction) < 0) unique.reverse();
        segments.push(unique);
      }
    }
  }

  const n = cut.normal;
  const axis = Math.abs(n.x) < .8 ? new THREE.Vector3(1, 0, 0) : new THREE.Vector3(0, 1, 0);
  const u = axis.cross(n).normalize(), v = n.clone().cross(u);
  const weld = Math.max(scale * 1e-6, 1e-8), nodes = [], buckets = new Map(), edges = [];
  // Numeric spatial buckets avoid 27 string allocations per endpoint. Bucket
  // coordinates are checked too: integer-hash collisions never change welding.
  const bucketHash = (x, y, z) => Math.imul(x, 73856093) ^ Math.imul(y, 19349663) ^ Math.imul(z, 83492791);
  const pointNodes = new WeakMap();
  const node = p => {
    const known = pointNodes.get(p);
    if (known !== undefined) return known;
    const x = Math.floor(p.x / weld), y = Math.floor(p.y / weld), z = Math.floor(p.z / weld);
    for (let dx = -1; dx <= 1; dx++) for (let dy = -1; dy <= 1; dy++) for (let dz = -1; dz <= 1; dz++) {
      const bx = x + dx, by = y + dy, bz = z + dz;
      const bucket = buckets.get(bucketHash(bx, by, bz));
      if (!bucket) continue;
      for (const cell of bucket) if (cell.x === bx && cell.y === by && cell.z === bz)
        for (const id of cell.ids) if (nodes[id].p.distanceToSquared(p) <= weld * weld) { pointNodes.set(p, id); return id; }
    }
    const id = nodes.length, key = bucketHash(x, y, z);
    nodes.push({p: p.clone(), edges: []});
    pointNodes.set(p, id);
    let bucket = buckets.get(key);
    if (!bucket) { bucket = []; buckets.set(key, bucket); }
    let cell = bucket.find(cell => cell.x === x && cell.y === y && cell.z === z);
    if (!cell) { cell = {x, y, z, ids: []}; bucket.push(cell); }
    cell.ids.push(id);
    return id;
  };
  // An existing mesh edge can lie exactly in the plane. Include it only
  // when its incident triangles occupy opposite sides (not for tangency).
  const coincident = new Map();
  for (const {points, sign} of planeEdges) {
    const a = node(points[0]), b = node(points[1]);
    const key = a < b ? `${a}:${b}` : `${b}:${a}`;
    const record = coincident.get(key) || {points, positive: false, negative: false};
    record.positive ||= sign === 1; record.negative ||= sign === -1;
    coincident.set(key, record);
  }
  for (const record of coincident.values()) if (record.positive && record.negative) segments.push(record.points);
  const edgeKeys = new Set();
  for (const [a, b] of segments) {
    const ia = node(a), ib = node(b), key = ia < ib ? `${ia}:${ib}` : `${ib}:${ia}`;
    if (ia === ib || edgeKeys.has(key)) continue;
    edgeKeys.add(key);
    const id = edges.length;
    edges.push({a: ia, b: ib, used: false});
    nodes[ia].edges.push(id); nodes[ib].edges.push(id);
  }
  const contours = [];
  for (const first of edges) {
    if (first.used) continue;
    const ids = [first.a]; let current = first.a, edge = first;
    for (let steps = 0; steps <= edges.length; steps++) {
      edge.used = true;
      current = edge.a === current ? edge.b : edge.a;
      if (current === ids[0]) break;
      ids.push(current);
      const next = nodes[current].edges.find(id => !edges[id].used);
      if (next === undefined) break;
      edge = edges[next];
    }
    if (current === ids[0] && ids.length >= 3) {
      const points = ids.map(id => nodes[id].p);
      const flat = points.map(p => new THREE.Vector2(p.dot(u), p.dot(v)));
      contours.push({points, flat, area: Math.abs(THREE.ShapeUtils.area(flat)), winding: Math.sign(THREE.ShapeUtils.area(flat)), parent: -1, depth: 0});
    }
  }
  const contains = (polygon, p) => {
    let inside = false;
    for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
      const a = polygon[i], b = polygon[j];
      if ((a.y > p.y) !== (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x) inside = !inside;
    }
    return inside;
  };
  for (let i = 0; i < contours.length; i++) {
    let smallest = Infinity;
    for (let j = 0; j < contours.length; j++) if (contours[j].area > contours[i].area && contours[j].area < smallest && contains(contours[j].flat, contours[i].flat[0])) {
      contours[i].parent = j; smallest = contours[j].area;
    }
  }
  for (const contour of contours) {
    let child = contour, parent = contour.parent;
    while (parent !== -1) { if (child.winding !== contours[parent].winding) contour.depth++; child = contours[parent]; parent = child.parent; }
  }
  let capTriangles = 0;
  for (let i = 0; i < contours.length; i++) {
    const contour = contours[i];
    if (contour.depth % 2) continue;
    const holes = contours.filter(c => c.parent === i && c.depth % 2 && c.winding !== contour.winding);
    const points = contour.points.concat(...holes.map(c => c.points));
    const flat = contour.flat.concat(...holes.map(c => c.flat));
    // triangulateShape mutates contours only if the final point duplicates the first.
    const triangles = THREE.ShapeUtils.triangulateShape(contour.flat.slice(), holes.map(c => c.flat.slice()));
    for (const triangle of triangles) {
      for (let side = 0; side < 2; side++) {
        const ids = triangle.slice();
        const a = flat[ids[0]], b = flat[ids[1]], c = flat[ids[2]];
        const cross = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        if ((cross > 0) !== (side === 1)) [ids[1], ids[2]] = [ids[2], ids[1]];
        sides[side].push({material: capMaterialIndex, vertices: ids.map(id => ({p: points[id], n: n.clone().multiplyScalar(side === 0 ? -1 : 1), uv: flat[id]}))});
      }
      capTriangles += 2;
    }
  }
  const build = triangles => {
    const result = new THREE.BufferGeometry(), positions = new Float32Array(triangles.length * 9),
      normals = new Float32Array(triangles.length * 9), uvs = uv ? new Float32Array(triangles.length * 6) : null;
    let pi = 0, ti = 0;
    let previousMaterial = -1;
    for (const triangle of triangles) {
      if (triangle.material !== previousMaterial) {
        result.addGroup(pi / 3, 0, triangle.material);
        previousMaterial = triangle.material;
      }
      result.groups[result.groups.length - 1].count += 3;
      const faceNormal = !normal ? triangle.vertices[1].p.clone().sub(triangle.vertices[0].p).cross(triangle.vertices[2].p.clone().sub(triangle.vertices[0].p)).normalize() : null;
      for (const vertex of triangle.vertices) {
        positions[pi] = vertex.p.x; positions[pi + 1] = vertex.p.y; positions[pi + 2] = vertex.p.z;
        const vn = vertex.n || faceNormal;
        normals[pi] = vn.x; normals[pi + 1] = vn.y; normals[pi + 2] = vn.z; pi += 3;
        if (uv) { uvs[ti++] = vertex.uv.x; uvs[ti++] = vertex.uv.y; }
      }
    }
    result.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    result.setAttribute('normal', new THREE.BufferAttribute(normals, 3));
    if (uv) result.setAttribute('uv', new THREE.BufferAttribute(uvs, 2));
    result.computeBoundingBox(); result.computeBoundingSphere();
    return result;
  };
  const positive = build(sides[0]), negative = build(sides[1]);
  return {positive, negative, owned: [positive, negative], capTriangles};
}

// GLTF material primitives are separate open Mesh patches. Merge siblings in
// their shared local coordinates before slicing so cap contours cross seams.
export function mergePrimitives(meshes) {
  const geometry = new THREE.BufferGeometry(), materials = [];
  const positions = [], normals = [], uvs = [];
  const hasUV = meshes.some(mesh => mesh.geometry.getAttribute('uv'));
  for (const mesh of meshes) {
    mesh.updateMatrix();
    if (!mesh.matrix.equals(new THREE.Matrix4())) throw new Error('mergePrimitives requires identity local transforms');
    const source = mesh.geometry, position = source.getAttribute('position');
    if (!position) continue;
    const normal = source.getAttribute('normal'), uv = source.getAttribute('uv');
    const sourceMaterials = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
    const materialMap = sourceMaterials.map(material => {
      let index = materials.indexOf(material);
      if (index === -1) { index = materials.length; materials.push(material); }
      return index;
    });
    const count = source.index ? source.index.count : position.count;
    let previous = -1;
    for (let offset = 0; offset + 2 < count; offset += 3) {
      const group = source.groups.find(g => offset >= g.start && offset < g.start + g.count);
      const material = materialMap[group?.materialIndex ?? 0] ?? materialMap[0];
      if (material !== previous) { geometry.addGroup(positions.length / 3, 0, material); previous = material; }
      geometry.groups[geometry.groups.length - 1].count += 3;
      const ids = [0, 1, 2].map(j => source.index ? source.index.getX(offset + j) : offset + j);
      const face = normal ? null : new THREE.Vector3().fromBufferAttribute(position, ids[1]).sub(new THREE.Vector3().fromBufferAttribute(position, ids[0])).cross(new THREE.Vector3().fromBufferAttribute(position, ids[2]).sub(new THREE.Vector3().fromBufferAttribute(position, ids[0]))).normalize();
      for (const id of ids) {
        positions.push(position.getX(id), position.getY(id), position.getZ(id));
        normals.push(normal ? normal.getX(id) : face.x, normal ? normal.getY(id) : face.y, normal ? normal.getZ(id) : face.z);
        if (hasUV) uvs.push(uv ? uv.getX(id) : 0, uv ? uv.getY(id) : 0);
      }
    }
  }
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
  geometry.setAttribute('normal', new THREE.Float32BufferAttribute(normals, 3));
  if (hasUV) geometry.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2));
  geometry.computeBoundingBox(); geometry.computeBoundingSphere();
  return {geometry, materials};
}
