struct Parameters {
    plane: vec4<f32>,
    counts: vec4<u32>, // vertices, candidate edges, unused, unused
}

@group(0) @binding(0) var<storage, read> positions: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> edges: array<vec2<u32>>;
@group(0) @binding(2) var<storage, read_write> results: array<vec4<f32>>;
@group(0) @binding(3) var<uniform> parameters: Parameters;

fn signed_distance(position: vec3<f32>) -> f32 {
    let distance = dot(parameters.plane.xyz, position) + parameters.plane.w;
    return select(distance, 0.0, abs(distance) < 1e-5);
}

@compute @workgroup_size(64)
fn prepare(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(num_workgroups) groups: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    let row = (group.y * groups.x + group.x) * 64u + lane;
    let vertex_count = parameters.counts.x;
    let edge_count = parameters.counts.y;
    if row >= vertex_count + edge_count || row >= arrayLength(&results) {
        return;
    }
    if row < vertex_count {
        if row < arrayLength(&positions) {
            results[row] = vec4<f32>(signed_distance(positions[row].xyz), 0.0, 0.0, 0.0);
        }
        return;
    }

    results[row] = vec4<f32>(0.0, 0.0, 0.0, -1.0);
    let edge_index = row - vertex_count;
    if edge_index >= edge_count || edge_index >= arrayLength(&edges) {
        return;
    }
    let edge = edges[edge_index];
    let i = min(edge.x, edge.y);
    let j = max(edge.x, edge.y);
    if i >= vertex_count || j >= vertex_count || j >= arrayLength(&positions) {
        return;
    }
    let pi = positions[i].xyz;
    let pj = positions[j].xyz;
    // Recompute here: another invocation's distance row is not synchronized.
    let di = signed_distance(pi);
    let dj = signed_distance(pj);
    if (di < 0.0 && dj > 0.0) || (di > 0.0 && dj < 0.0) {
        let t = di / (di - dj);
        results[row] = vec4<f32>(pi + (pj - pi) * t, t);
    }
}
