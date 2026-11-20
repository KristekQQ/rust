//! Development-only readback of renderer bounds. Never mutates the scene.
use glam::{Mat4, Vec3, Vec4};

#[cfg(any(target_arch = "wasm32", feature = "native-render"))]
const MAX_ENTRIES: usize = 2000;
const EDGES: [(usize, usize); 12] = [
    (0, 1),
    (2, 3),
    (4, 5),
    (6, 7),
    (0, 2),
    (1, 3),
    (4, 6),
    (5, 7),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

/// Homogeneous clipping keeps lines finite at the eye and inside the viewport.
fn clip_edge(mut a: Vec4, mut b: Vec4) -> Option<[f32; 4]> {
    if !a.is_finite() || !b.is_finite() {
        return None;
    }
    for plane in 0..7 {
        let distance = |p: Vec4| match plane {
            0 => p.w - 1e-5,
            1 => p.z,
            2 => p.w - p.z,
            3 => p.w + p.x,
            4 => p.w - p.x,
            5 => p.w + p.y,
            _ => p.w - p.y,
        };
        let (da, db) = (distance(a), distance(b));
        if da < 0.0 && db < 0.0 {
            return None;
        }
        if (da < 0.0) != (db < 0.0) {
            let point = a.lerp(b, da / (da - db));
            if da < 0.0 {
                a = point;
            } else {
                b = point;
            }
        }
    }
    if a.w <= 0.0 || b.w <= 0.0 {
        return None;
    }
    let result = [
        (a.x / a.w + 1.0) * 0.5,
        (1.0 - a.y / a.w) * 0.5,
        (b.x / b.w + 1.0) * 0.5,
        (1.0 - b.y / b.w) * 0.5,
    ];
    result
        .iter()
        .all(|v| v.is_finite())
        .then(|| result.map(|v| v.clamp(0.0, 1.0)))
}

fn project_bounds(min: Vec3, max: Vec3, view: Mat4) -> Vec<[f32; 4]> {
    if !min.is_finite() || !max.is_finite() || !min.cmple(max).all() {
        return vec![];
    }
    let corners: [Vec4; 8] = std::array::from_fn(|i| {
        view * Vec3::new(
            if i & 1 == 0 { min.x } else { max.x },
            if i & 2 == 0 { min.y } else { max.y },
            if i & 4 == 0 { min.z } else { max.z },
        )
        .extend(1.0)
    });
    EDGES
        .iter()
        .filter_map(|&(a, b)| clip_edge(corners[a], corners[b]))
        .collect()
}

#[cfg(any(target_arch = "wasm32", feature = "native-render"))]
fn rect_edges([x, y, right, bottom]: [f32; 4]) -> Vec<[f32; 4]> {
    vec![
        [x, y, right, y],
        [right, y, right, bottom],
        [right, bottom, x, bottom],
        [x, bottom, x, y],
    ]
}

#[cfg(any(target_arch = "wasm32", feature = "native-render"))]
fn label(edges: &[[f32; 4]]) -> [f32; 2] {
    edges.iter().fold([1.0f32, 1.0f32], |p, e| {
        [p[0].min(e[0]).min(e[2]), p[1].min(e[1]).min(e[3])]
    })
}

#[cfg(any(target_arch = "wasm32", feature = "native-render"))]
pub(crate) fn snapshot(
    scene: &crate::scene::SceneManager,
    view: Mat4,
    glow_eligible: impl Fn(u32) -> bool,
    ssr_rect: Option<[f32; 4]>,
) -> serde_json::Value {
    use serde_json::json;
    let mut entries = Vec::new();
    let truncated = std::cell::Cell::new(false);
    let entry_count = std::cell::Cell::new(0usize);
    let mut primitive_budget = MAX_ENTRIES;
    let mut push = |id: String, kind: &str, layer: &str, edges: Vec<[f32; 4]>, parts: usize| {
        if edges.is_empty() {
            return;
        }
        if entries.len() == MAX_ENTRIES {
            truncated.set(true);
            return;
        }
        entry_count.set(entry_count.get() + 1);
        entries.push(json!({"id":id, "kind":kind, "layer":layer, "label":label(&edges), "edges":edges, "parts":parts}));
    };
    // Reserve this entry even in very large scenes so the SSR diagnostic remains visible.
    if let Some(rect) = ssr_rect {
        push(
            "ssr".into(),
            "effect",
            "SSR trace region (bounding rectangle)",
            rect_edges(rect),
            1,
        );
    }
    for (index, (&id, object)) in scene.objects().iter().enumerate() {
        if index >= MAX_ENTRIES || entry_count.get() >= MAX_ENTRIES {
            truncated.set(true);
            break;
        }
        if object.visual_fade <= 0.0 {
            continue;
        }
        let data = &object.render_data;
        let edges = project_bounds(
            data.center - data.half_extent,
            data.center + data.half_extent,
            view,
        );
        let model = scene.model(id);
        let mut primitives = Vec::new();
        let mut unsupported = false;
        let mut parts = 0;
        if let Some(model) = model {
            for &node in &model.pose.active_nodes {
                let Some(mesh) = model
                    .asset
                    .nodes
                    .get(node)
                    .and_then(|n| n.mesh)
                    .and_then(|m| model.asset.meshes.get(m))
                else {
                    continue;
                };
                for &primitive in &mesh.primitives {
                    parts += 1;
                    if primitive_budget == 0 {
                        truncated.set(true);
                        unsupported = true;
                        break;
                    }
                    primitive_budget -= 1;
                    if let Some((min, max)) = model.render_bounds_cache.primitive_bounds(
                        &model.asset,
                        &model.pose,
                        node,
                        primitive,
                        data.model(),
                    ) {
                        primitives.push((node, primitive, project_bounds(min, max, view)));
                    } else {
                        unsupported = true;
                    }
                }
            }
        } else {
            parts = 1;
        }
        let kind = if model.is_some() {
            "model"
        } else if scene.info_panels.contains_key(&id) {
            "panel"
        } else if data.instance.material[3] < -0.5 || scene.is_text_object(id) {
            "glyph"
        } else {
            "object"
        };
        let layer = if unsupported {
            "world AABB (some primitive bounds unavailable)"
        } else {
            "world AABB"
        };
        push(id.to_string(), kind, layer, edges.clone(), parts);
        for (node, primitive, edges) in primitives {
            push(
                id.to_string(),
                "primitive",
                &format!("node {node} / primitive {primitive} AABB"),
                edges,
                1,
            );
        }
        if glow_eligible(id) && !edges.is_empty() {
            let [x, y] = label(&edges);
            let [right, bottom] = edges.iter().fold([0.0f32; 2], |p, e| {
                [p[0].max(e[0]).max(e[2]), p[1].max(e[1]).max(e[3])]
            });
            // Deliberate visual offset only. This is not an estimate of halo extent.
            let rect = [
                (x - 0.0015).max(0.0),
                (y - 0.0015).max(0.0),
                (right + 0.0015).min(1.0),
                (bottom + 0.0015).min(1.0),
            ];
            push(
                id.to_string(),
                "effect",
                "glow eligible (offset guide; not halo extent)",
                rect_edges(rect),
                1,
            );
        }
    }
    json!({"entries":entries, "truncated":truncated.get()})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clips_near_eye_and_viewport_without_nonfinite_coordinates() {
        assert!(clip_edge(Vec4::new(0., 0., -2., -1.), Vec4::new(0., 0., -1., -0.5)).is_none());
        let edge = clip_edge(Vec4::new(-4., 0., -1., -1.), Vec4::new(0.5, 0., 0.5, 1.)).unwrap();
        assert!(edge
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
        assert_eq!(
            clip_edge(Vec4::new(-2., 0., 0.5, 1.), Vec4::new(2., 0., 0.5, 1.)),
            Some([0., 0.5, 1., 0.5])
        );
    }
    #[test]
    fn omits_boxes_entirely_behind_near_plane() {
        assert!(project_bounds(
            Vec3::new(-1., -1., -2.),
            Vec3::new(1., 1., -1.),
            Mat4::IDENTITY
        )
        .is_empty());
        assert_eq!(
            project_bounds(Vec3::ZERO, Vec3::splat(0.5), Mat4::IDENTITY).len(),
            12
        );
    }
}
