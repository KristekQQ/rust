//! Owned cut snapshots: deformation, clipping and visual mesh assembly can run
//! without borrowing the live game, scene, renderer or graphics queue.
use crate::{
    cutting_mesh::PreparedVisualMesh,
    gltf_loader::{ImportedMesh, ImportedVertex},
    gltf_model::{ModelAsset, ModelPose},
};
use glam::{Mat4, Vec3};
use std::{collections::VecDeque, sync::Arc};

pub(crate) struct CutSource {
    pub asset: Arc<ModelAsset>,
    pub pose: ModelPose,
}
pub struct CutWork {
    pub(crate) token: u32,
    pub(crate) source: Arc<CutSource>,
    pub(crate) root: Mat4,
    pub(crate) normal: Vec3,
    pub(crate) point: Vec3,
    /// (node, primitive, index start, index end); bounded by the game's chunk limit.
    pub(crate) ranges: Vec<(usize, usize, usize, usize)>,
}
pub struct PreparedCut {
    pub(crate) token: u32,
    pub(crate) meshes: VecDeque<(usize, PreparedVisualMesh)>,
}
impl CutWork {
    /// Existing web binary transport; native calls execute on its worker instead.
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let triangles = self.ranges.iter().map(|r| (r.3 - r.2) / 3).sum::<usize>();
        let mut output = Vec::with_capacity(32 + triangles * 108);
        output.extend_from_slice(&self.token.to_le_bytes());
        output.extend_from_slice(&(triangles as u32).to_le_bytes());
        for v in [
            self.normal.x,
            self.normal.y,
            self.normal.z,
            self.point.x,
            self.point.y,
            self.point.z,
        ] {
            output.extend_from_slice(&v.to_le_bytes());
        }
        for &(node, primitive, start, end) in &self.ranges {
            let p = &self.source.asset.primitives[primitive];
            let vertices = crate::gltf_animation::deformed_cut_vertex_subset(
                &self.source.asset,
                &self.source.pose,
                node,
                primitive,
                p.indices[start..end].iter().copied(),
            )?;
            let matrix =
                self.root * crate::gltf_animation::node_render_matrix(&self.source.pose, node);
            let nm = matrix.inverse().transpose();
            let material = p.material.map(|m| &self.source.asset.materials[m]);
            for v in vertices {
                let position = matrix.transform_point3(Vec3::from_array(v.position));
                let normal = nm
                    .transform_vector3(Vec3::from_array(v.normal))
                    .normalize_or_zero();
                let color = Vec3::from_slice(&v.color)
                    * material.map_or(Vec3::ONE, |m| Vec3::from_slice(&m.base_color_factor));
                for f in [
                    position.x, position.y, position.z, normal.x, normal.y, normal.z, color.x,
                    color.y, color.z,
                ] {
                    output.extend_from_slice(&f.to_le_bytes());
                }
            }
        }
        Ok(output)
    }
    /// Native worker entry: all geometry loops run on the caller's worker thread.
    pub fn execute(self) -> Result<PreparedCut, String> {
        let result = crate::rim_cut::split(&self.encode()?)?;
        let counts = [
            u32::from_le_bytes(result[4..8].try_into().unwrap()) as usize,
            u32::from_le_bytes(result[8..12].try_into().unwrap()) as usize,
        ];
        let mut meshes = VecDeque::new();
        let mut byte_offset = 12;
        for (side, count) in counts.into_iter().enumerate() {
            for chunk in result[byte_offset..byte_offset + count * 36].chunks(9000 * 36) {
                let mut min = Vec3::splat(f32::INFINITY);
                let mut max = Vec3::splat(f32::NEG_INFINITY);
                let vertices = chunk
                    .as_chunks::<36>()
                    .0
                    .iter()
                    .map(|bytes| {
                        let f: [f32; 9] = std::array::from_fn(|i| {
                            f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
                        });
                        let p = Vec3::from_slice(&f);
                        min = min.min(p);
                        max = max.max(p);
                        ImportedVertex {
                            position: p.to_array(),
                            normal: [f[3], f[4], f[5]],
                            color: [f[6], f[7], f[8]],
                        }
                    })
                    .collect::<Vec<_>>();
                let render = ImportedMesh {
                    indices: (0..vertices.len() as u32).collect(),
                    vertices,
                    bounds_min: min.to_array(),
                    bounds_max: max.to_array(),
                    warnings: Vec::new(),
                };
                meshes.push_back((side, PreparedVisualMesh::new(render)));
            }
            byte_offset += count * 36;
        }
        Ok(PreparedCut {
            token: self.token,
            meshes,
        })
    }
}
