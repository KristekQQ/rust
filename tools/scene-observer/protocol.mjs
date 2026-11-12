// NWR1/v1 layout mirrors src/scene_wire.rs. No simulation runs in this relay.
export const CACHE_LIMIT = 32 * 1024 * 1024;
export const RECORDING_LIMIT = 128 * 1024 * 1024;
const MAX_ITEMS = 200000, MAX_ARRAY = 4000000;
const utf8 = new TextEncoder(), decoder = new TextDecoder('utf-8', { fatal: true });
const fail = (why) => { throw new Error(why); };
function bytes(value) {
  if (!(value instanceof Uint8Array)) fail('Expected Uint8Array');
  return value;
}
function join(parts) {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let offset = 0;
  for (const part of parts) { out.set(part, offset); offset += part.length; }
  return out;
}
function u32(n) { const b = new Uint8Array(4); new DataView(b.buffer).setUint32(0, n, true); return b; }
class Reader {
  constructor(b) { this.b = bytes(b); this.v = new DataView(b.buffer, b.byteOffset, b.length); this.o = 0; }
  skip(n) { if (!Number.isSafeInteger(n) || n < 0 || this.o + n > this.b.length) fail('Truncated wire'); this.o += n; }
  byte() { this.skip(1); return this.v.getUint8(this.o - 1); }
  bool() { const v = this.byte(); if (v > 1) fail('Invalid wire bool'); return v === 1; }
  uint() { this.skip(4); return this.v.getUint32(this.o - 4, true); }
  id() { const n = this.uint(); if (n === 0xffffffff) fail('Invalid wire ID'); return n; }
  count(max = MAX_ITEMS) { const n = this.uint(); if (n > max) fail('Wire count budget'); return n; }
  floats(n) { this.skip(n * 4); for (let p = this.o - n * 4; p < this.o; p += 4) if (!Number.isFinite(this.v.getFloat32(p, true))) fail('Nonfinite wire float'); }
  bounds() { const start = this.o; this.floats(6); for (let i = 0; i < 3; i++) if (this.v.getFloat32(start + i * 4, true) > this.v.getFloat32(start + 12 + i * 4, true)) fail('Invalid wire bounds'); }
  mesh() {
    this.bounds(); const vertices = this.count(MAX_ARRAY); this.floats(vertices * 9);
    const n = this.count(MAX_ARRAY); if (n % 3) fail('Invalid triangle indices');
    for (let i = 0; i < n; i++) if (this.uint() >= vertices) fail('Wire index outside vertices');
    return n;
  }
  glow() { if (this.bool()) { const vertices = this.count(MAX_ARRAY); this.floats(vertices * 9); const n = this.count(MAX_ARRAY); if (n % 3) fail('Invalid glow indices'); for (let i = 0; i < n; i++) if (this.uint() >= vertices) fail('Wire glow index outside vertices'); } }
  meshData() { this.bounds(); const triangles = this.count(MAX_ARRAY); this.count(MAX_ARRAY); this.skip(8); if (!Number.isFinite(this.v.getFloat64(this.o - 8, true)) || this.v.getFloat64(this.o - 8, true) < 0) fail('Nonfinite volume'); const indices = this.mesh(); this.glow(); return {triangles, indices}; }
  colors() { const n = this.count(MAX_ARRAY); this.floats(n * 4); return n; }
  ordered(fn, max = MAX_ITEMS) { const n = this.count(max); let prev = -1; for (let i = 0; i < n; i++) { const id = this.id(); if (id <= prev) fail('Unordered/duplicate wire ID'); prev = id; fn(id); } }
}
function parse(wire, objectIds = null) {
  const r = new Reader(wire);
  if (wire.length > CACHE_LIMIT) fail('Wire/cache budget exceeded');
  if (decoder.decode(wire.subarray(0, 4)) !== 'NWR1') fail('Invalid NWR1 magic');
  r.skip(4); const version = r.uint(); if (![1,2,3,4].includes(version)) fail('Unsupported NWR1 version');
  r.skip(8); const sequence = r.v.getBigUint64(8, true); if (!sequence) fail('Invalid sequence');
  r.skip(version === 1 ? 40 : 48);
  const tables = [new Map(), new Map(), new Map()];
  for (let table = 0; table < 3; table++) {
    const n = r.count();
    for (let i = 0; i < n; i++) {
      const start = r.o, id = r.id(); if (tables[table].has(id)) fail('Duplicate asset');
      if (table === 0) r.meshData();
      if (table === 1) {
        const source = r.meshData(), indices = r.mesh(), colors = r.colors(), originals = r.colors();
        const varying = r.count(MAX_ARRAY);
        for (let j = 0; j < varying; j++) if (r.uint() >= source.triangles) fail('Variant index outside source');
        const variants = r.count();
        if (!variants || !source.triangles || originals !== variants * source.triangles || colors !== variants * varying || indices !== source.indices) fail('Invalid variant shape');
      }
      if (table === 2) { if (r.byte() > 9) fail('Invalid panel kind'); r.floats(2); for (const offset of [r.o - 8, r.o - 4]) { const size = r.v.getFloat32(offset, true); if (size < .01 || size > 100) fail('Invalid panel dimensions'); } r.skip(8); r.bool(); r.mesh(); r.glow(); }
      tables[table].set(id, wire.slice(start, r.o));
    }
  }
  const tailOffset = r.o, live = [new Set(), new Set(), new Set()];
  const objectSet = new Set();
  const objects = [], triangles = [], variants = [], projected = [new Set(), new Set(), new Set()];
  const selected = id => objectIds !== null && objectIds.has(id);
  r.ordered(id => {
    objectSet.add(id);
    const start = r.o - 4;
    const tag = r.byte(); if (tag > 3) fail('Invalid object kind'); r.uint(); r.floats(9);
    r.bool(); r.bool(); r.bool(); r.floats(1); r.bool(); r.floats(8); r.bool(); r.bool();
    if (r.bool()) r.bounds();
    r.floats(44 + 6);
    if (selected(id)) objects.push(wire.subarray(start, r.o));
  });
  const instances = [];
  r.ordered(id => {
    const start = r.o - 4;
    r.bool(); r.floats(3);
    if (r.bool()) { const variant = r.id(); instances.push(variant); r.uint(); if (selected(id)) projected[1].add(variant); }
    else { live[0].add(id); if (selected(id)) projected[0].add(id); }
    if (selected(id)) triangles.push(wire.subarray(start, r.o));
  });
  r.ordered(id => { const start = r.o - 4; r.bool(); live[1].add(id); if (projected[1].has(id)) variants.push(wire.subarray(start, r.o)); });
  r.ordered(id => { r.skip(8); live[2].add(id); });
  for (const id of instances) if (!live[1].has(id)) fail('Missing instance variant');
  const lightOffset = r.o;
  r.ordered(() => r.floats(6), 4);
  const lightEnd = r.o;
  if (r.bool()) for (let i = 0; i < 10; i++) if (!live[1].has(r.id())) fail('Missing numeric asset');
  for (let i = 0; i < (version >= 3 ? 5 : 4); i++) r.ordered(() => {});
  const lineage = [];
  if (version >= 4) r.ordered(child => {
    const start = r.o - 4, parent = r.id(), matrixStart = r.o;
    r.floats(16);
    const m = Array.from({length:16}, (_,i) => r.v.getFloat32(matrixStart + i*4,true));
    const determinant = m[0]*(m[5]*m[10]-m[9]*m[6])-m[4]*(m[1]*m[10]-m[9]*m[2])+m[8]*(m[1]*m[6]-m[5]*m[2]);
    if (m[3]!==0 || m[7]!==0 || m[11]!==0 || m[15]!==1 || !Number.isFinite(determinant) || Math.abs(determinant)<1e-10) fail('Invalid wire birth transform');
    if (child === parent || !objectSet.has(child) || objectSet.has(parent)) fail('Invalid wire cut lineage');
    if (objectIds === null || selected(child)) lineage.push(wire.subarray(start,r.o));
  });
  if (r.o !== wire.length) fail('Trailing wire bytes');
  const header = wire.slice(0, version === 1 ? 56 : 64);
  let tail;
  if (objectIds !== null) {
    const view = new DataView(header.buffer);
    if (!objectIds.has(view.getUint32(48, true))) view.setUint32(48, 0xffffffff, true);
    if (version >= 2) for (const offset of [56,60]) if (!objectIds.has(view.getUint32(offset,true))) view.setUint32(offset,0xffffffff,true);
    tail = join([u32(objects.length), ...objects, u32(triangles.length), ...triangles,
      u32(variants.length), ...variants, u32(0), wire.subarray(lightOffset, lightEnd),
      new Uint8Array(1), u32(0), u32(0), u32(0), u32(0), ...(version >= 3 ? [u32(0)] : []),
      ...(version >= 4 ? [u32(lineage.length), ...lineage] : [])]);
  } else tail = wire.slice(tailOffset);
  return { sequence, tables, live, projected: objectIds !== null ? projected : live, header, tail };
}
export class WireCache {
  constructor({ maxBytes = CACHE_LIMIT, objectIds = null } = {}) {
    if (objectIds !== null && (!Array.isArray(objectIds) || objectIds.some(id => !Number.isInteger(id) || id < 0 || id >= 0xffffffff))) fail('Invalid objectIds');
    this.objectIds = objectIds === null ? null : new Set(objectIds);
    this.maxBytes = Math.min(maxBytes, CACHE_LIMIT); this.assets = [new Map(), new Map(), new Map()];
    this.sourceAssets = this.assets; this.sequence = 0n; this.frame = null; this.byteLength = 0;
  }
  ingest(wire) {
    const frame = parse(bytes(wire), this.objectIds);
    if (frame.sequence <= this.sequence) fail('Out-of-order source sequence');
    const sourceAssets = frame.live.map((ids, table) => {
      const map = new Map();
      for (const id of ids) { const value = frame.tables[table].get(id) ?? this.sourceAssets[table].get(id); if (!value) fail('Missing live asset'); map.set(id, value); }
      return map;
    });
    const assets = frame.projected.map((ids, table) => new Map([...ids].map(id => [id, sourceAssets[table].get(id)])));
    const size = frame.header.length + 12 + frame.tail.length + sourceAssets.reduce((n, map) => n + [...map.values()].reduce((sum, b) => sum + b.length, 0), 0);
    if (size > this.maxBytes) fail('Wire/cache budget exceeded');
    this.sourceAssets = sourceAssets; this.assets = assets; this.frame = frame; this.sequence = frame.sequence; this.byteLength = size;
    // Do not retain the source asset tables (including removed assets) in the latest frame.
    this.frame.tables = null; this.frame.live = null; this.frame.projected = null;
    return this.sequence;
  }
  packetFor(peer = { sequence: 0, known: [new Map(), new Map(), new Map()] }) {
    if (!this.frame) return null;
    if (!Number.isSafeInteger(peer.sequence) || peer.sequence < 0 || peer.sequence >= Number.MAX_SAFE_INTEGER || !Array.isArray(peer.known) || peer.known.length !== 3 || peer.known.some(m => !(m instanceof Map))) fail('Invalid peer');
    const header = this.frame.header.slice(), parts = [header];
    new DataView(header.buffer).setBigUint64(8, BigInt(peer.sequence + 1), true);
    const known = [];
    for (let i = 0; i < 3; i++) {
      const changed = [...this.assets[i]].filter(([id, b]) => peer.known[i].get(id) !== b).sort((a,b) => a[0] - b[0]);
      parts.push(u32(changed.length), ...changed.map(([,b]) => b)); known.push(new Map(this.assets[i]));
    }
    parts.push(this.frame.tail);
    const out = join(parts); peer.sequence++; peer.known = known;
    return out;
  }
}
function checkMeta(meta) {
  if (!meta || typeof meta.session !== 'string' || !meta.session.length || meta.session.length > 256 || !Number.isFinite(meta.sampledAt) || meta.sampledAt < 0 || !['demo', 'live'].includes(meta.mode) || !Object.hasOwn(meta, 'state')) fail('Invalid observer metadata');
}
export function encodePacket(meta, wire) {
  checkMeta(meta); bytes(wire); const json = utf8.encode(JSON.stringify(meta));
  if (json.length > 65536 || wire.length > CACHE_LIMIT) fail('Packet budget exceeded');
  return join([u32(json.length), json, wire]);
}
export function decodePacket(packet) {
  const r = new Reader(packet); if (packet.length > CACHE_LIMIT + 65540) fail('Packet budget exceeded');
  const n = r.count(65536); r.skip(n);
  const meta = JSON.parse(decoder.decode(packet.subarray(4, 4 + n))); checkMeta(meta);
  const wire = packet.subarray(4 + n); if (wire.length < 68 || decoder.decode(wire.subarray(0,4)) !== 'NWR1' || ![1,2,3,4].includes(new DataView(wire.buffer, wire.byteOffset, wire.length).getUint32(4, true))) fail('Invalid wire packet');
  return {meta, wire};
}
export function encodeRecording(packets) {
  let size = 4; const parts = [utf8.encode('NSR1')];
  for (const packet of packets) { bytes(packet); size += packet.length + 4; if (size > RECORDING_LIMIT) fail('Recording budget exceeded'); decodePacket(packet); parts.push(u32(packet.length), packet); }
  return join(parts);
}
export function decodeRecording(recording) {
  const r = new Reader(recording); if (recording.length > RECORDING_LIMIT) fail('Recording budget exceeded');
  r.skip(4); if (decoder.decode(recording.subarray(0,4)) !== 'NSR1') fail('Invalid recording magic');
  const packets = [];
  while (r.o < recording.length) { const n = r.uint(), start = r.o; r.skip(n); const packet = recording.slice(start, r.o); decodePacket(packet); packets.push(packet); }
  return packets;
}
