//! Conservative render-only bounds. CPU queries/cutting retain exact deformation.
use crate::gltf_animation::{model_bounds, node_render_matrix};
use crate::gltf_model::{ModelAsset, ModelPose};
use glam::{Mat4, Vec3};
use std::collections::HashMap;

const MAX_CACHE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: [f64; 3],
    max: [f64; 3],
}

impl Bounds {
    fn point(point: Vec3) -> Self {
        let point = point.to_array().map(f64::from);
        Self {
            min: point,
            max: point,
        }
    }
    fn union(&mut self, other: Self) {
        for axis in 0..3 {
            self.min[axis] = self.min[axis].min(other.min[axis]);
            self.max[axis] = self.max[axis].max(other.max[axis]);
        }
    }
    fn magnitude(self) -> [f64; 3] {
        std::array::from_fn(|axis| self.min[axis].abs().max(self.max[axis].abs()))
    }
    fn expand(&mut self, padding: [f64; 3]) {
        for (axis, padding) in padding.into_iter().enumerate() {
            self.min[axis] -= padding;
            self.max[axis] += padding;
        }
    }
    fn transform(self, matrix: Mat4) -> Option<Self> {
        if !matrix.is_finite()
            || matrix.w_axis.w != 1.
            || matrix.x_axis.w != 0.
            || matrix.y_axis.w != 0.
            || matrix.z_axis.w != 0.
        {
            return None;
        }
        let matrix = matrix.to_cols_array_2d();
        let mut transformed = Self {
            min: [0.; 3],
            max: [0.; 3],
        };
        let magnitude = self.magnitude();
        for (axis, &translation) in matrix[3].iter().take(3).enumerate() {
            let translation = translation as f64;
            let mut min = translation;
            let mut max = translation;
            let mut absolute = translation.abs();
            for (column, coefficients) in matrix.iter().take(3).enumerate() {
                let coefficient = coefficients[axis] as f64;
                let low = coefficient * self.min[column];
                let high = coefficient * self.max[column];
                min += low.min(high);
                max += low.max(high);
                absolute += coefficient.abs() * magnitude[column];
            }
            // Includes f32 point-transform rounding and f64 interval construction.
            let error = absolute * gamma(8)?;
            transformed.min[axis] = min - error;
            transformed.max[axis] = max + error;
        }
        Some(transformed)
    }
    fn finish(self) -> Option<(Vec3, Vec3)> {
        // One extra representable step makes casts outward on both sides.
        let min = Vec3::from_array(self.min.map(|v| (v as f32).next_down()));
        let max = Vec3::from_array(self.max.map(|v| (v as f32).next_up()));
        (min.is_finite() && max.is_finite()).then_some((min, max))
    }
}

fn gamma(operations: usize) -> Option<f64> {
    let error = operations as f64 * f32::EPSILON as f64;
    (error < 0.5).then(|| error / (1. - error))
}

fn include(bounds: &mut Option<Bounds>, value: Bounds) {
    if let Some(bounds) = bounds {
        bounds.union(value);
    } else {
        *bounds = Some(value);
    }
}

#[derive(Debug)]
struct PrimitiveBounds {
    all: Option<Bounds>,
    rigid: Option<Bounds>,
    joints: Vec<(u16, Bounds)>,
    max_terms: usize,
    max_sum: f64,
    sum_error: f64,
}

/// Bounded immutable cache, shared across transactional pose clones.
#[derive(Debug, Default)]
pub(crate) struct RenderBoundsCache {
    primitives: Vec<Option<PrimitiveBounds>>,
}

impl RenderBoundsCache {
    pub(crate) fn new(asset: &ModelAsset) -> Self {
        Self::with_budget(asset, MAX_CACHE_BYTES)
    }

    fn with_budget(asset: &ModelAsset, budget: usize) -> Self {
        let Some(mut bytes) = asset
            .primitives
            .len()
            .checked_mul(std::mem::size_of::<Option<PrimitiveBounds>>())
        else {
            return Self::default();
        };
        if bytes > budget {
            return Self::default();
        }
        let mut primitives = Vec::new();
        if primitives
            .try_reserve_exact(asset.primitives.len())
            .is_err()
        {
            return Self::default();
        }
        for primitive in &asset.primitives {
            let build = || -> Option<PrimitiveBounds> {
                if !primitive.morph_targets.is_empty() {
                    return None;
                }
                let mut all = None;
                let mut rigid = None;
                let mut joints = HashMap::<u16, Bounds>::new();
                let mut max_terms = 0;
                let mut max_sum: f64 = 1.;
                let mut sum_error: f64 = 0.;
                for vertex in asset.vertices.get(primitive.vertices.clone())? {
                    if vertex
                        .position
                        .iter()
                        .chain(&vertex.normal)
                        .chain(&vertex.tangent)
                        .any(|v| !v.is_finite())
                    {
                        return None;
                    }
                    // Extreme attributes can overflow normal/tangent deformation;
                    // leave the exact path responsible for reporting that error.
                    if vertex
                        .normal
                        .iter()
                        .chain(&vertex.tangent)
                        .any(|v| v.abs() > 1e12)
                    {
                        return None;
                    }
                    let point = Bounds::point(Vec3::from_array(vertex.position));
                    include(&mut all, point);
                    if vertex.influences.is_empty() {
                        include(&mut rigid, point);
                        continue;
                    }
                    let mut sum = 0.;
                    let mut terms: usize = 0;
                    for &(joint, weight) in &vertex.influences {
                        if !weight.is_finite() || weight < 0. {
                            return None;
                        }
                        if weight == 0. {
                            continue;
                        }
                        sum += weight as f64;
                        terms += 1;
                        if let Some(bounds) = joints.get_mut(&joint) {
                            bounds.union(point);
                        } else {
                            // Budget includes a generous allowance for temporary hash table storage.
                            if joints
                                .len()
                                .checked_add(1)?
                                .checked_mul(128)?
                                .checked_add(bytes)?
                                > budget
                            {
                                return None;
                            }
                            joints.try_reserve(1).ok()?;
                            joints.insert(joint, point);
                        }
                    }
                    if (sum - 1.).abs() > 1e-4
                        || gamma(terms.checked_mul(2)?.checked_add(8)?).is_none()
                    {
                        return None;
                    }
                    max_terms = max_terms.max(terms);
                    max_sum = max_sum.max(sum);
                    sum_error = sum_error.max((sum - 1.).abs());
                }
                let mut joint_bounds = Vec::new();
                joint_bounds.try_reserve_exact(joints.len()).ok()?;
                joint_bounds.extend(joints);
                Some(PrimitiveBounds {
                    all,
                    rigid,
                    joints: joint_bounds,
                    max_terms,
                    max_sum,
                    sum_error,
                })
            };
            let cached = build();
            if let Some(cached) = &cached {
                bytes += cached.joints.len() * std::mem::size_of::<(u16, Bounds)>();
            }
            primitives.push(cached);
        }
        Self { primitives }
    }

    fn local_bounds(
        &self,
        asset: &ModelAsset,
        pose: &ModelPose,
        node_index: usize,
        primitive_index: usize,
    ) -> Option<Bounds> {
        let primitive = self.primitives.get(primitive_index)?.as_ref()?;
        let all = primitive.all?;
        let local = if asset.nodes.get(node_index)?.skin.is_none() {
            all
        } else {
            let matrices = pose.skin_matrices.get(node_index)?;
            let mut local = primitive.rigid;
            let mut skinned = None;
            let mut magnitude = [0_f64; 3];
            for &(joint, bounds) in &primitive.joints {
                let matrix = *matrices.get(joint as usize)?;
                // Degenerate/ill-conditioned palettes preserve exact error handling.
                let inverse = matrix.inverse();
                if !inverse.is_finite()
                    || matrix
                        .to_cols_array()
                        .iter()
                        .chain(inverse.to_cols_array().iter())
                        .any(|v| v.abs() > 1e12)
                {
                    return None;
                }
                let transformed = bounds.transform(matrix)?;
                include(&mut skinned, transformed);
                let p = all.magnitude();
                let columns = matrix.to_cols_array_2d();
                for (axis, magnitude) in magnitude.iter_mut().enumerate() {
                    let absolute = (0..3)
                        .map(|col| (columns[col][axis] as f64).abs() * p[col])
                        .sum::<f64>()
                        + (columns[3][axis] as f64).abs();
                    *magnitude = magnitude.max(absolute);
                }
            }
            if let Some(mut skinned) = skinned {
                // Convex enclosure plus error from non-unit float sums and
                // the CPU's weighted-matrix accumulation before transformation.
                let padding =
                    primitive.sum_error + primitive.max_sum * gamma(2 * primitive.max_terms + 16)?;
                skinned.expand(magnitude.map(|value| value * padding));
                include(&mut local, skinned);
            }
            local?
        };
        Some(local)
    }

    /// Conservative bounds in draw coordinates. Unknown inputs disable culling.
    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn primitive_bounds(
        &self,
        asset: &ModelAsset,
        pose: &ModelPose,
        node: usize,
        primitive: usize,
        root: Mat4,
    ) -> Option<(Vec3, Vec3)> {
        let mesh = asset.nodes.get(node)?.mesh?;
        if !asset.meshes.get(mesh)?.primitives.contains(&primitive) {
            return None;
        }
        // node_render_matrix uses direct indexing; malformed poses must fail open.
        pose.skin_world_space.get(node)?;
        pose.world_matrices.get(node)?;
        let render = root * node_render_matrix(pose, node);
        self.local_bounds(asset, pose, node, primitive)?
            .transform(render)?
            .finish()
    }

    /// Returns None when exact deformation is required, never a narrower guess.
    fn cached_bounds(&self, asset: &ModelAsset, pose: &ModelPose) -> Option<(Vec3, Vec3)> {
        let mut result = None;
        for &node_index in &pose.active_nodes {
            let node = asset.nodes.get(node_index)?;
            let Some(mesh) = node.mesh else {
                continue;
            };
            let render = node_render_matrix(pose, node_index);
            for &primitive_index in &asset.meshes.get(mesh)?.primitives {
                let primitive = self.primitives.get(primitive_index)?.as_ref()?;
                if primitive.all.is_none() {
                    continue;
                }
                let local = self.local_bounds(asset, pose, node_index, primitive_index)?;
                include(&mut result, local.transform(render)?);
            }
        }
        result.map_or(Some((Vec3::ZERO, Vec3::ZERO)), Bounds::finish)
    }

    pub(crate) fn bounds(
        &self,
        asset: &ModelAsset,
        pose: &ModelPose,
    ) -> Result<(Vec3, Vec3), String> {
        #[cfg(target_arch = "wasm32")]
        let _scope = crate::render::profiler::cpu_scope("gltf.bounds");
        self.cached_bounds(asset, pose)
            .map(Ok)
            .unwrap_or_else(|| model_bounds(asset, pose))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gltf_animation::recompute_world;
    use crate::gltf_model::*;
    use glam::Quat;
    use std::sync::Arc;

    fn fixture() -> ModelAsset {
        let vertex = |position, influences| ModelVertex {
            position,
            influences,
            normal: [0., 1., 0.],
            tangent: [1., 0., 0., 1.],
            uv0: [0.; 2],
            uv1: [0.; 2],
            color: [1.; 4],
            joints: [0; 4],
            weights: [0.; 4],
            tex_coords: vec![],
        };
        let node = |mesh, skin| ModelNode {
            name: None,
            children: vec![],
            mesh,
            skin,
            camera: None,
            trs: NodeTrs {
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
            matrix: None,
            weights: None,
        };
        ModelAsset {
            vertices: vec![
                vertex([-2., 1., 3.], vec![(0, 0.3), (1, 0.7)]),
                vertex([4., -3., 2.], vec![(0, 0.6), (1, 0.4)]),
                vertex([1., 2., -5.], vec![]),
            ],
            primitives: vec![ModelPrimitive {
                flat_normals: false,
                vertices: 0..3,
                indices: vec![0, 1, 2],
                mode: PrimitiveMode::Triangles,
                material: None,
                morph_targets: vec![],
            }],
            meshes: vec![ModelMesh {
                name: None,
                primitives: vec![0],
                weights: vec![],
            }],
            materials: vec![],
            images: vec![],
            textures: vec![],
            samplers: vec![],
            nodes: vec![node(Some(0), Some(0)), node(None, None), node(None, None)],
            skins: vec![ModelSkin {
                name: None,
                joints: vec![1, 2],
                inverse_bind_matrices: vec![Mat4::IDENTITY; 2],
                skeleton: None,
            }],
            animations: vec![],
            scenes: vec![ModelScene {
                name: None,
                nodes: vec![0, 1, 2],
            }],
            default_scene: Some(0),
            cameras: vec![],
            warnings: vec![],
        }
    }

    fn assert_contains(model: &ModelInstance) {
        let exact = model_bounds(&model.asset, &model.pose).unwrap();
        let cached = model
            .render_bounds_cache
            .cached_bounds(&model.asset, &model.pose)
            .expect("cached runtime path");
        assert!(
            cached.0.cmple(exact.0).all(),
            "cached min {:?} exact {:?}",
            cached.0,
            exact.0
        );
        assert!(
            cached.1.cmpge(exact.1).all(),
            "cached max {:?} exact {:?}",
            cached.1,
            exact.1
        );
        // The public CPU API still returns vertex extrema, without render padding.
        assert_eq!(exact, model_bounds(&model.asset, &model.pose).unwrap());
    }

    #[test]
    fn conservative_skin_bounds_include_mixed_and_uninfluenced_vertices() {
        let mut model = ModelInstance::new(Arc::new(fixture()), None).unwrap();
        assert!(Arc::ptr_eq(
            &model.render_bounds_cache,
            &model.clone().render_bounds_cache
        ));
        for scale in [Vec3::ONE, Vec3::new(-2., 0.3, 3.), Vec3::new(0., 2., -1.)] {
            for step in 0..20 {
                let time = step as f32 * 0.2;
                model.pose.local_trs[0].scale = scale;
                model.pose.local_trs[0].translation = Vec3::new(5., 2., -4.);
                model.pose.local_trs[1].translation = Vec3::new(time, -3., 2.);
                model.pose.local_trs[1].rotation = Quat::from_rotation_y(time);
                model.pose.local_trs[2].rotation = Quat::from_rotation_x(-time);
                model.pose.local_trs[2].scale = Vec3::new(-1., 3., 0.1);
                recompute_world(&model.asset, &mut model.pose).unwrap();
                assert_contains(&model);
            }
        }
    }

    #[test]
    fn rigid_bounds_include_negative_nonuniform_and_singular_transforms() {
        let mut asset = fixture();
        asset.nodes[0].skin = None;
        let mut model = ModelInstance::new(Arc::new(asset), None).unwrap();
        for scale in [Vec3::new(-2., 0.1, 4.), Vec3::new(0., 2., 1.)] {
            model.pose.local_trs[0].scale = scale;
            model.pose.local_trs[0].rotation = Quat::from_rotation_z(0.7);
            recompute_world(&model.asset, &mut model.pose).unwrap();
            assert_contains(&model);
        }
    }

    #[test]
    fn real_fox_all_clips_cached_bounds_contain_exact_poses() {
        let asset = crate::gltf_loader::parse_model(
            include_bytes!("../assets/models/fox/Fox.glb"),
            &[],
            &[],
        )
        .unwrap();
        let mut model = ModelInstance::new(Arc::new(asset), None).unwrap();
        for clip in 0..model.asset.animations.len() {
            let duration = model.asset.animations[clip].duration;
            model.play(clip, false).unwrap();
            for step in 0..7 {
                model.seek(duration * step as f32 / 6.).unwrap();
                assert_contains(&model);
            }
        }
    }

    #[test]
    fn unsupported_weights_morphs_and_budget_use_exact_bounds() {
        for influences in [vec![(0, -0.5), (1, 1.5)], vec![(0, 0.)], vec![(0, 2.)]] {
            let mut asset = fixture();
            asset.vertices[0].influences = influences;
            let model = ModelInstance::new(Arc::new(asset), None).unwrap();
            assert!(model
                .render_bounds_cache
                .cached_bounds(&model.asset, &model.pose)
                .is_none());
            assert_eq!(
                model.render_bounds().unwrap(),
                model_bounds(&model.asset, &model.pose).unwrap()
            );
        }
        let mut asset = fixture();
        asset.primitives[0].morph_targets.push(MorphTarget {
            positions: vec![[100., -30., 5.]; 3],
            normals: vec![],
            tangents: vec![],
        });
        asset.meshes[0].weights = vec![4.];
        let model = ModelInstance::new(Arc::new(asset), None).unwrap();
        assert!(model
            .render_bounds_cache
            .cached_bounds(&model.asset, &model.pose)
            .is_none());
        assert_eq!(
            model.render_bounds().unwrap(),
            model_bounds(&model.asset, &model.pose).unwrap()
        );
        let cache = RenderBoundsCache::with_budget(&model.asset, 0);
        assert!(cache.primitives.is_empty());
        assert_eq!(
            cache.bounds(&model.asset, &model.pose).unwrap(),
            model_bounds(&model.asset, &model.pose).unwrap()
        );
    }

    #[test]
    fn degenerate_and_nonfinite_palette_fallback_preserves_errors() {
        let mut model = ModelInstance::new(Arc::new(fixture()), None).unwrap();
        model.pose.skin_matrices[0][0] = Mat4::ZERO;
        assert!(model
            .render_bounds_cache
            .cached_bounds(&model.asset, &model.pose)
            .is_none());
        assert_eq!(
            model.render_bounds().unwrap(),
            model_bounds(&model.asset, &model.pose).unwrap()
        );
        model.pose.skin_matrices[0][0] = Mat4::from_cols_array(&[f32::INFINITY; 16]);
        assert_eq!(
            model.render_bounds().unwrap_err(),
            model_bounds(&model.asset, &model.pose).unwrap_err()
        );
    }

    #[test]
    fn cache_accepts_many_joint_influences_without_gpu_limits() {
        let mut asset = fixture();
        asset.vertices[0].influences = (0..100).map(|i| (i % 2, 0.01)).collect();
        let model = ModelInstance::new(Arc::new(asset), None).unwrap();
        assert_contains(&model);
    }

    fn assert_primitive_contains(model: &ModelInstance, primitive: usize, root: Mat4) {
        let bounds = model
            .render_bounds_cache
            .primitive_bounds(&model.asset, &model.pose, 0, primitive, root)
            .expect("supported primitive bounds");
        let render = root * node_render_matrix(&model.pose, 0);
        for vertex in
            crate::gltf_animation::deformed_vertices(&model.asset, &model.pose, 0, primitive)
                .unwrap()
        {
            let point = render.transform_point3(Vec3::from_array(vertex.position));
            assert!(
                bounds.0.cmple(point).all() && bounds.1.cmpge(point).all(),
                "primitive {primitive}: {:?} outside {:?}",
                point,
                bounds
            );
        }
    }

    #[test]
    fn primitive_bounds_contain_animated_vertices_in_combined_draw_coordinates() {
        for skin in [false, true] {
            let mut asset = fixture();
            if !skin {
                asset.nodes[0].skin = None;
            }
            // Separate primitive ranges must keep independent spatial bounds.
            asset.primitives[0].vertices = 0..2;
            asset.primitives[0].indices = vec![0, 1];
            let mut second = asset.primitives[0].clone();
            second.vertices = 2..3;
            second.indices = vec![0];
            asset.primitives.push(second);
            asset.meshes[0].primitives.push(1);
            let mut model = ModelInstance::new(Arc::new(asset), None).unwrap();
            for step in 0..20 {
                let t = step as f32 * 0.17;
                model.pose.local_trs[0].scale = Vec3::new(-2., 0.3, 3.);
                // Keep skin palette strictly affine (existing cache contract);
                // rigid nodes also exercise animated mesh rotation.
                model.pose.local_trs[0].rotation = if skin {
                    Quat::IDENTITY
                } else {
                    Quat::from_rotation_y(t)
                };
                model.pose.local_trs[0].translation = Vec3::new(5., -3., 4.);
                model.pose.local_trs[1].translation = Vec3::new(t, 2., -1.);
                model.pose.local_trs[1].rotation = Quat::from_rotation_z(-t);
                model.pose.local_trs[2].scale = Vec3::new(-1., 3., 0.1);
                model.pose.local_trs[2].rotation = Quat::from_rotation_x(t);
                recompute_world(&model.asset, &mut model.pose).unwrap();
                let root = Mat4::from_scale_rotation_translation(
                    Vec3::new(0.2, -4., 1.7),
                    Quat::from_rotation_z(t),
                    Vec3::new(100., -200., 30.),
                );
                assert_primitive_contains(&model, 0, root);
                assert_primitive_contains(&model, 1, root);
            }
        }
    }

    #[test]
    fn primitive_bounds_unknown_morph_budget_and_bad_transforms_fail_open() {
        let mut model = ModelInstance::new(Arc::new(fixture()), None).unwrap();
        let query = |model: &ModelInstance, node, primitive, root| {
            model.render_bounds_cache.primitive_bounds(
                &model.asset,
                &model.pose,
                node,
                primitive,
                root,
            )
        };
        assert!(query(&model, usize::MAX, 0, Mat4::IDENTITY).is_none());
        assert!(query(&model, 0, usize::MAX, Mat4::IDENTITY).is_none());
        assert!(query(&model, 1, 0, Mat4::IDENTITY).is_none());
        assert!(query(&model, 0, 0, Mat4::ZERO).is_none());
        model.pose.skin_matrices[0][0] = Mat4::from_scale(Vec3::new(1e-20, 1., 1.));
        assert!(query(&model, 0, 0, Mat4::IDENTITY).is_none());
        model.pose.world_matrices.clear();
        assert!(query(&model, 0, 0, Mat4::IDENTITY).is_none());

        let mut asset = fixture();
        asset.primitives[0].morph_targets.push(MorphTarget {
            positions: vec![[1., 0., 0.]; 3],
            normals: vec![],
            tangents: vec![],
        });
        let model = ModelInstance::new(Arc::new(asset), None).unwrap();
        assert!(query(&model, 0, 0, Mat4::IDENTITY).is_none());
        let cache = RenderBoundsCache::with_budget(&model.asset, 0);
        assert!(cache
            .primitive_bounds(&model.asset, &model.pose, 0, 0, Mat4::IDENTITY)
            .is_none());
    }
}
