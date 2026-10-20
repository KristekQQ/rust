//! Conservative triangle filtering in the original GPU vertex/index address space.
use crate::gltf_loader::ImportedMesh;

pub(crate) fn alpha_indices(mesh: &ImportedMesh, opaque_above: f32) -> Option<Vec<u32>> {
    if !opaque_above.is_finite() || !mesh.indices.len().is_multiple_of(3) {
        return None;
    }
    let mut indices = Vec::new();
    for triangle in mesh.indices.as_chunks::<3>().0 {
        let colors = [
            mesh.vertices.get(triangle[0] as usize)?.color,
            mesh.vertices.get(triangle[1] as usize)?.color,
            mesh.vertices.get(triangle[2] as usize)?.color,
        ];
        // A common channel above the cutoff stays opaque under perspective
        // interpolation. Different bright channels alone do not prove opacity.
        let opaque = (0..3).any(|channel| {
            colors
                .iter()
                .all(|color| color[channel].is_finite() && color[channel] >= opaque_above)
        });
        if !opaque {
            indices.extend_from_slice(triangle);
        }
    }
    Some(indices)
}

/// Union in the shared face-expanded index space: any variant may need a facet.
pub(crate) fn variant_alpha_indices(
    colors: &[[f32; 4]],
    triangles: usize,
    threshold: f32,
) -> Vec<u32> {
    let mut needed = vec![false; triangles];
    for colors in colors.chunks_exact(triangles) {
        for (triangle, color) in colors.iter().enumerate() {
            needed[triangle] |= color[..3].iter().all(|channel| *channel < threshold);
        }
    }
    needed
        .iter()
        .enumerate()
        .filter(|(_, needed)| **needed)
        .flat_map(|(triangle, _)| {
            [
                triangle as u32 * 3,
                triangle as u32 * 3 + 1,
                triangle as u32 * 3 + 2,
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gltf_loader::ImportedVertex;

    #[test]
    fn shared_variants_alpha_subset_is_conservative_union() {
        let table = [
            [0.9, 0., 0., 1.],
            [0.1, 0., 0., 1.],
            [0.1, 0., 0., 1.],
            [0.9, 0., 0., 1.],
        ];
        assert_eq!(
            variant_alpha_indices(&table, 2, 0.25),
            vec![0, 1, 2, 3, 4, 5]
        );
        assert_eq!(variant_alpha_indices(&table, 2, 0.05), Vec::<u32>::new());
        let same = [
            [0.9, 0., 0., 1.],
            [0.1, 0., 0., 1.],
            [0.9, 0., 0., 1.],
            [0.2, 0., 0., 1.],
        ];
        assert_eq!(variant_alpha_indices(&same, 2, 0.25), vec![3, 4, 5]);
    }
    #[test]
    fn alpha_subset_preserves_gradient_cutoff_and_original_indices() {
        let colors = [
            [1., 0., 0.],
            [0., 1., 0.],
            [0., 0., 1.],
            [0.5, 0., 0.],
            [0.5, 0.1, 0.],
            [0.5, 0., 0.1],
            [0.49, 0., 0.],
        ];
        let mesh = ImportedMesh {
            vertices: colors
                .into_iter()
                .map(|color| ImportedVertex {
                    color,
                    position: [0.; 3],
                    normal: [0.; 3],
                })
                .collect(),
            indices: vec![2, 0, 1, 5, 3, 4, 3, 6, 5],
            bounds_min: [0.; 3],
            bounds_max: [0.; 3],
            warnings: vec![],
        };
        assert_eq!(alpha_indices(&mesh, 0.5), Some(vec![2, 0, 1, 3, 6, 5]));
        assert_eq!(alpha_indices(&mesh, 0.), Some(vec![]));
        assert_eq!(alpha_indices(&mesh, f32::NAN), None);
        let mut invalid = mesh;
        invalid.indices.push(0);
        assert_eq!(alpha_indices(&invalid, 0.5), None);
        invalid.indices = vec![0, 1, 99];
        assert_eq!(alpha_indices(&invalid, 0.5), None);
    }
}
