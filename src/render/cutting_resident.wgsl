// Five storage bindings, fixed request budgets, no CPU geometry round trip.
struct Vertex { position: vec4<f32>, normal: vec4<f32>, color: vec4<f32> }
struct Parameters {
    plane: vec4<f32>, a: vec4<f32>, blade: vec4<f32>, motion: vec4<f32>, color: vec4<f32>,
    counts: vec4<u32>, directions: array<vec4<f32>, 128>,
}
struct Metadata {
    count: atomic<u32>, segments: atomic<u32>, status: atomic<u32>, sides: atomic<u32>,
    lo: vec4<f32>, hi: vec4<f32>, volume: vec4<f32>, samples: array<f32, 64>,
}
// Segment region, undirected edge hash, directed point hash (per half).
// Segment region is reused for ordered loop indices after constructing the hashes.
struct Edge { a: vec4<f32>, b: vec4<f32>, count: u32, next: u32, other: u32, pad: u32 }
@group(0) @binding(0) var<storage, read> source: array<Vertex>;
@group(0) @binding(1) var<storage, read_write> output: array<Vertex>;
@group(0) @binding(2) var<storage, read_write> scratch: array<Edge>;
@group(0) @binding(3) var<storage, read_write> metadata: array<Metadata>;
@group(0) @binding(4) var<storage, read_write> packed: array<u32>;
@group(0) @binding(5) var<uniform> p: Parameters;
const EPS: f32 = 0.00001;
const BIG: f32 = 3.402823e38;

fn fail(h: u32, code: u32) { atomicMax(&metadata[h].status, code); }
fn finite(v: vec3<f32>) -> bool { return all(abs(v) <= vec3<f32>(BIG)); }
fn equal(a: vec3<f32>, b: vec3<f32>) -> bool { return all(a == b); }
fn less(a: vec3<f32>, b: vec3<f32>) -> bool {
    if a.x != b.x { return a.x < b.x; }
    if a.y != b.y { return a.y < b.y; }
    return a.z < b.z;
}
fn distance(v: vec3<f32>) -> f32 { return dot(p.plane.xyz, v) - p.plane.w; }
fn classified(v: vec3<f32>) -> f32 {
    let d = distance(v);
    return select(d, 0.0, abs(d) < EPS);
}
fn uncertain(v: vec3<f32>) -> bool {
    let d = distance(v);
    // Conservative dot/subtraction rounding bound; do not invent near-plane topology.
    let error = (dot(abs(p.plane.xyz), abs(v)) + abs(p.plane.w)) * 0.0000005;
    return !finite(v) || !(abs(d) <= BIG) ||
        (abs(d) < EPS && (d != 0.0 || error > EPS * 0.25)) || abs(abs(d) - EPS) <= error;
}
fn interpolate(v0: Vertex, v1: Vertex) -> Vertex {
    var a = v0; var b = v1;
    if less(b.position.xyz, a.position.xyz) { a = v1; b = v0; }
    let da = classified(a.position.xyz); let db = classified(b.position.xyz);
    let t = da / (da - db);
    return Vertex(vec4<f32>(a.position.xyz + (b.position.xyz - a.position.xyz) * t, 1.0),
        a.normal + (b.normal - a.normal) * t, a.color + (b.color - a.color) * t);
}
fn scratch_base(h: u32) -> u32 { return h * (p.counts.z + 2u * p.counts.w); }
fn triangle(h: u32, a: Vertex, b: Vertex, c: Vertex) {
    if !finite(a.position.xyz) || !finite(b.position.xyz) || !finite(c.position.xyz) ||
        !finite(a.normal.xyz) || !finite(b.normal.xyz) || !finite(c.normal.xyz) ||
        !finite(a.color.xyz) || !finite(b.color.xyz) || !finite(c.color.xyz) {
        fail(h, 4u); return;
    }
    let i = atomicAdd(&metadata[h].count, 3u);
    if i + 3u > p.counts.y { fail(h, 3u); return; }
    let base = h * p.counts.y + i;
    output[base] = a; output[base + 1u] = b; output[base + 2u] = c;
}
fn segment(h: u32, a: vec3<f32>, b: vec3<f32>) {
    if equal(a, b) { fail(h, 2u); return; }
    let i = atomicAdd(&metadata[h].segments, 1u);
    if i >= p.counts.z { fail(h, 3u); return; }
    scratch[scratch_base(h) + i] = Edge(vec4<f32>(a, 0.0), vec4<f32>(b, 0.0), 0u, 0u, 0u, 0u);
}

@compute @workgroup_size(64)
fn clip(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= p.counts.x { return; }
    let start = gid.x * 3u;
    var v: array<Vertex, 3>;
    var d: array<f32, 3>;
    var sides = 0u;
    for (var k = 0u; k < 3u; k++) {
        v[k] = source[start + k];
        if uncertain(v[k].position.xyz) || !finite(v[k].normal.xyz) || !finite(v[k].color.xyz) {
            fail(0u, 4u); fail(1u, 4u); return;
        }
        d[k] = classified(v[k].position.xyz);
        if d[k] > EPS { sides |= 1u; }
        if d[k] < -EPS { sides |= 2u; }
    }
    atomicOr(&metadata[0].sides, sides); atomicOr(&metadata[1].sides, sides);
    let coplanar = d[0] == 0.0 && d[1] == 0.0 && d[2] == 0.0;
    let owner = select(0u, 1u, dot(cross(v[1].position.xyz - v[0].position.xyz,
        v[2].position.xyz - v[0].position.xyz), p.plane.xyz) > 0.0);
    for (var h = 0u; h < 2u; h++) {
        if coplanar && owner != h { continue; }
        let sign = select(1.0, -1.0, h == 1u);
        var poly: array<Vertex, 4>;
        var on: array<bool, 4>;
        var n = 0u;
        for (var k = 0u; k < 3u; k++) {
            let j = (k + 1u) % 3u;
            if d[k] * sign >= 0.0 { poly[n] = v[k]; on[n] = d[k] == 0.0; n++; }
            if d[k] * d[j] < 0.0 {
                let vertex = interpolate(v[k], v[j]);
                let error = (dot(abs(p.plane.xyz), abs(vertex.position.xyz)) + abs(p.plane.w)) * 0.0000005;
                if error > EPS * 0.25 || abs(distance(vertex.position.xyz)) > EPS * 0.25 {
                    fail(0u, 4u); fail(1u, 4u); return;
                }
                poly[n] = vertex; on[n] = true; n++;
            }
        }
        if n < 3u { continue; }
        for (var k = 1u; k + 1u < n; k++) { triangle(h, poly[0], poly[k], poly[k + 1u]); }
        for (var k = 0u; k < n; k++) {
            let j = (k + 1u) % n;
            if on[k] && on[j] { segment(h, poly[k].position.xyz, poly[j].position.xyz); }
        }
    }
}

fn point_hash(a: vec3<f32>) -> u32 {
    // Floating equality equates -0/+0, so their hashes must agree as well.
    let bits = bitcast<vec3<u32>>(select(a, vec3<f32>(0.0), a == vec3<f32>(0.0)));
    var h = (bits.x * 0x9e3779b9u) ^ (bits.y * 0x85ebca6bu) ^ (bits.z * 0xc2b2ae35u);
    h ^= h >> 16u; h *= 0x7feb352du; h ^= h >> 15u;
    return h;
}
fn inside_sweep(a: vec3<f32>) -> bool {
    let q = a - p.a.xyz; let u = p.blade.xyz; let v = p.motion.xyz;
    let uu = dot(u, u); let vv = dot(v, v); let uv = dot(u, v);
    let det = uu * vv - uv * uv;
    let s = (dot(q, u) * vv - dot(q, v) * uv) / det;
    let t = (dot(q, v) * uu - dot(q, u) * uv) / det;
    return s >= -EPS && s <= 1.0 + EPS && t >= -EPS && t <= 1.0 + EPS;
}
fn point_lookup(base: u32, a: vec3<f32>) -> u32 {
    let mask = p.counts.w - 1u;
    var slot = point_hash(a) & mask;
    for (var probe = 0u; probe < p.counts.w; probe++) {
        let index = base + slot;
        if scratch[index].count == 0u { return 0xffffffffu; }
        if equal(scratch[index].a.xyz, a) { return index; }
        slot = (slot + 1u) & mask;
    }
    return 0xffffffffu;
}

@compute @workgroup_size(64)
fn close_boundary(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    if lane != 0u { return; }
    let h = group.x;
    if atomicLoad(&metadata[h].status) != 0u { return; }
    if atomicLoad(&metadata[h].sides) != 3u { fail(h, 1u); return; }
    let base = scratch_base(h); let edges = base + p.counts.z; let points = edges + p.counts.w;
    let mask = p.counts.w - 1u;
    let count = atomicLoad(&metadata[h].segments);
    // Non-atomic hash: exactly one invocation owns this half in this phase.
    for (var i = 0u; i < count; i++) {
        let original = scratch[base + i];
        var a = original.a; var b = original.b;
        if less(b.xyz, a.xyz) { a = original.b; b = original.a; }
        var slot = (point_hash(a.xyz) ^ (point_hash(b.xyz) * 0x9e3779b9u)) & mask;
        var inserted = false;
        for (var probe = 0u; probe < p.counts.w; probe++) {
            let index = edges + slot;
            if scratch[index].count == 0u {
                // Preserve original orientation separately from the canonical key.
                scratch[index] = Edge(a, b, 1u, select(0u, 1u, equal(a.xyz, original.a.xyz)), 0u, 0u);
                inserted = true; break;
            }
            if equal(scratch[index].a.xyz, a.xyz) && equal(scratch[index].b.xyz, b.xyz) {
                if scratch[index].count != 1u || scratch[index].next == select(0u, 1u, equal(a.xyz, original.a.xyz)) {
                    fail(h, 2u); return;
                }
                scratch[index].count = 2u; inserted = true; break;
            }
            slot = (slot + 1u) & mask;
        }
        if !inserted { fail(h, 3u); return; }
    }
    var boundary_count = 0u; var first = 0xffffffffu;
    for (var i = 0u; i < p.counts.w; i++) {
        let e = scratch[edges + i];
        if e.count != 1u { continue; }
        // Reverse the exposed surface edge so cap winding closes the surface.
        let a = select(e.a, e.b, e.next == 1u); let b = select(e.b, e.a, e.next == 1u);
        if !inside_sweep(a.xyz) || !inside_sweep(b.xyz) { fail(h, 1u); return; }
        var slot = point_hash(a.xyz) & mask;
        var inserted = false;
        for (var probe = 0u; probe < p.counts.w; probe++) {
            let index = points + slot;
            if scratch[index].count == 0u {
                scratch[index] = Edge(a, b, 1u, 0u, 0u, 0u);
                if first == 0xffffffffu { first = index; }
                boundary_count++; inserted = true; break;
            }
            if equal(scratch[index].a.xyz, a.xyz) { fail(h, 2u); return; }
            slot = (slot + 1u) & mask;
        }
        if !inserted { fail(h, 3u); return; }
    }
    if boundary_count < 3u { fail(h, 1u); return; }
    // Traverse precisely one loop and mark each point. Additional contours fail.
    var current = first;
    let origin = scratch[first].a.xyz;
    var sum = vec3<f32>(0.0); var compensation = vec3<f32>(0.0);
    for (var i = 0u; i < boundary_count; i++) {
        if current == 0xffffffffu || scratch[current].other != 0u { fail(h, 2u); return; }
        scratch[current].other = 1u;
        scratch[base + i].next = current;
        let y = (scratch[current].a.xyz - origin) - compensation;
        let next_sum = sum + y; compensation = (next_sum - sum) - y; sum = next_sum;
        current = point_lookup(points, scratch[current].b.xyz);
    }
    if current != first { fail(h, 2u); return; }
    let center = origin + sum / f32(boundary_count);
    let outward = p.plane.xyz * select(-1.0, 1.0, h == 1u);
    for (var i = 0u; i < boundary_count; i++) {
        let e = scratch[scratch[base + i].next];
        let area = dot(cross(e.b.xyz - e.a.xyz, center - e.a.xyz), outward);
        if !(area > 0.00000001) { fail(h, 5u); return; }
    }
    if atomicLoad(&metadata[h].count) + boundary_count * 3u > p.counts.y { fail(h, 3u); return; }
    for (var i = 0u; i < boundary_count; i++) {
        let e = scratch[scratch[base + i].next];
        let normal = vec4<f32>(outward, 0.0);
        triangle(h, Vertex(vec4<f32>(center, 1.0), normal, p.color),
            Vertex(vec4<f32>(e.a.xyz, 1.0), normal, p.color),
            Vertex(vec4<f32>(e.b.xyz, 1.0), normal, p.color));
    }
}

var<workgroup> measure_lo: array<vec3<f32>, 64>;
var<workgroup> measure_hi: array<vec3<f32>, 64>;
var<workgroup> measure_volume: array<f32, 64>;
@compute @workgroup_size(64)
fn measure(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    let h = group.x;
    let count = min(atomicLoad(&metadata[h].count), p.counts.y); let base = h * p.counts.y;
    var lo = vec3<f32>(BIG); var hi = vec3<f32>(-BIG);
    for (var i = lane; i < count; i += 64u) {
        let v = output[base + i].position.xyz;
        if !finite(v) { fail(h, 4u); }
        lo = min(lo, v); hi = max(hi, v);
    }
    measure_lo[lane] = lo; measure_hi[lane] = hi; workgroupBarrier();
    for (var stride = 32u; stride > 0u; stride /= 2u) {
        if lane < stride {
            measure_lo[lane] = min(measure_lo[lane], measure_lo[lane + stride]);
            measure_hi[lane] = max(measure_hi[lane], measure_hi[lane + stride]);
        }
        workgroupBarrier();
    }
    lo = measure_lo[0]; hi = measure_hi[0];
    var sum = 0.0; var compensation = 0.0;
    for (var i = lane * 3u; i < count; i += 64u * 3u) {
        let a = output[base + i].position.xyz - lo;
        let b = output[base + i + 1u].position.xyz - lo;
        let c = output[base + i + 2u].position.xyz - lo;
        let value = dot(a, cross(b, c)) / 6.0;
        let y = value - compensation; let t = sum + y;
        compensation = (t - sum) - y; sum = t;
    }
    measure_volume[lane] = sum; workgroupBarrier();
    for (var stride = 32u; stride > 0u; stride /= 2u) {
        if lane < stride { measure_volume[lane] += measure_volume[lane + stride]; }
        workgroupBarrier();
    }
    if lane == 0u {
        sum = measure_volume[0];
        metadata[h].lo = vec4<f32>(lo, 0.0); metadata[h].hi = vec4<f32>(hi, 0.0);
        metadata[h].volume = vec4<f32>(sum, 0.0, 0.0, 0.0);
        if !(sum > 0.0 && sum <= BIG) && atomicLoad(&metadata[h].status) == 0u { fail(h, 4u); }
    }
}

var<workgroup> reduction: array<f32, 64>;
@compute @workgroup_size(64)
fn support(@builtin(workgroup_id) group: vec3<u32>, @builtin(local_invocation_index) lane: u32) {
    let h = group.x / 64u; let sample = group.x % 64u;
    if p.a.w == 1.0 && sample != 0u { return; }
    var value = BIG;
    let count = min(atomicLoad(&metadata[h].count), p.counts.y);
    if atomicLoad(&metadata[h].status) == 0u {
        for (var i = lane; i < count; i += 64u) {
            value = min(value, dot(p.directions[group.x].xyz, output[h * p.counts.y + i].position.xyz));
        }
    }
    reduction[lane] = value; workgroupBarrier();
    for (var stride = 32u; stride > 0u; stride /= 2u) {
        if lane < stride { reduction[lane] = min(reduction[lane], reduction[lane + stride]); }
        workgroupBarrier();
    }
    if lane == 0u {
        if p.a.w == 1.0 {
            for (var i = 0u; i < 64u; i++) { metadata[h].samples[i] = reduction[0]; }
        } else { metadata[h].samples[sample] = reduction[0]; }
    }
}

@compute @workgroup_size(64)
fn pack(@builtin(global_invocation_id) gid: vec3<u32>) {
    let h = gid.y; let i = gid.x;
    if atomicLoad(&metadata[h].status) != 0u || i >= min(atomicLoad(&metadata[h].count), p.counts.y) { return; }
    let v = output[h * p.counts.y + i];
    let base = h * p.counts.y * 10u;
    let pos = bitcast<vec3<u32>>(v.position.xyz); let color = bitcast<vec3<u32>>(v.color.xyz);
    let normal = bitcast<vec3<u32>>(v.normal.xyz);
    for (var k = 0u; k < 3u; k++) {
        packed[base + i * 9u + k] = pos[k];
        packed[base + i * 9u + 3u + k] = color[k];
        packed[base + i * 9u + 6u + k] = normal[k];
    }
    packed[base + p.counts.y * 9u + i] = i;
}
