//! CPU visibility and render extraction; shared by native tests and WASM.
use crate::scene::{MeshKind, SceneManager, Transform};
use glam::{Mat4, Vec3, Vec4};

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceData {
    pub model: [[f32; 4]; 4],
    pub normal_matrix: [[f32; 4]; 4],
    pub material: [f32; 4],
    pub emission: [f32; 4],
    pub surface_params: [f32; 4],
}

pub(crate) struct ObjectRenderData {
    instance: InstanceData,
    center: Vec3,
    half_extent: Vec3,
}
impl ObjectRenderData {
    pub(crate) fn plane_normal(&self) -> Vec3 {
        Mat4::from_cols_array_2d(&self.instance.normal_matrix)
            .transform_vector3(Vec3::Y)
            .normalize()
    }
    pub(crate) fn intersects(&self, frustum: &Frustum) -> bool {
        frustum.intersects_aabb(self.center, self.half_extent)
    }
    pub(crate) fn new(mesh: MeshKind, transform: Transform) -> Self {
        let local = match mesh {
            MeshKind::Cube => Vec3::splat(0.5),
            MeshKind::Plane => Vec3::new(0.5, 0.0, 0.5),
            MeshKind::Sphere => Vec3::ONE,
            MeshKind::Imported(_) => Vec3::ZERO,
        };
        Self::with_bounds(transform, -local, local)
    }
    pub(crate) fn with_bounds(transform: Transform, min: Vec3, max: Vec3) -> Self {
        let model = transform.model();
        let local = (max - min) * 0.5;
        Self {
            instance: InstanceData {
                model: model.to_cols_array_2d(),
                normal_matrix: crate::scene::normal_matrix(model).to_cols_array_2d(),
                material: [0.0, 1.0, 0.0, 0.0],
                emission: [0.0, 0.0, 1.0, 8.0],
                surface_params: [0.0, -1.0, 0.0, 0.0],
            },
            center: model.transform_point3((max + min) * 0.5),
            half_extent: model.x_axis.truncate().abs() * local.x
                + model.y_axis.truncate().abs() * local.y
                + model.z_axis.truncate().abs() * local.z,
        }
    }
}

pub struct Frustum {
    planes: [Vec4; 6],
    tolerance: [f32; 6],
}
impl Frustum {
    pub fn from_view_projection(matrix: Mat4) -> Self {
        let rows = matrix.transpose();
        let (x, y, z, w) = (rows.x_axis, rows.y_axis, rows.z_axis, rows.w_axis);
        // WebGPU clip volume: -w <= x,y <= w; 0 <= z <= w.
        let planes = [w + x, w - x, w + y, w - y, z, w - z];
        Self {
            tolerance: planes.map(|p| 1e-5 * p.truncate().length()),
            planes,
        }
    }
    pub fn intersects_aabb(&self, center: Vec3, half_extent: Vec3) -> bool {
        self.planes
            .iter()
            .zip(self.tolerance)
            .all(|(plane, tolerance)| {
                let normal = plane.truncate();
                let distance = normal.dot(center) + plane.w;
                let radius = normal.abs().dot(half_extent);
                // Conservative tolerance, scaled for unnormalized planes.
                distance + radius >= -tolerance
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderPassKind {
    Main,
    Reflection,
    ShadowStatic,
    ShadowDynamic,
    Environment,
}
impl RenderPassKind {
    pub fn index(self) -> usize {
        self as usize
    }
}

/// Query work is per extraction; rebuild counters include only tracked synchronizations.
#[derive(Clone, Copy, Default, Debug)]
pub struct CullingStats {
    pub nodes_visited: usize,
    pub object_tests: usize,
    pub static_rebuilds: usize,
    pub dynamic_rebuilds: usize,
}

#[derive(Default)]
pub(crate) struct VisibilityIndex {
    static_tree: crate::spatial::Bvh,
    dynamic_tree: crate::spatial::Bvh,
    static_revision: Option<u64>,
    render_revision: Option<u64>,
    totals: [usize; 5],
    static_rebuilds: usize,
    dynamic_rebuilds: usize,
}
impl VisibilityIndex {
    pub(crate) fn sync(&mut self, scene: &SceneManager, collect_stats: bool) {
        if self.render_revision == Some(scene.render_revision()) {
            return;
        }
        let rebuild_static = self.static_revision != Some(scene.static_revision());
        let mut statics = Vec::new();
        let mut dynamics = Vec::new();
        self.totals = [0; 5];
        for (&id, object) in scene.objects() {
            for pass in [
                RenderPassKind::Main,
                RenderPassKind::Reflection,
                RenderPassKind::ShadowStatic,
                RenderPassKind::ShadowDynamic,
                RenderPassKind::Environment,
            ] {
                self.totals[pass.index()] += usize::from(included(scene, id, object, pass));
            }
            let data = &object.render_data;
            let entry = (
                id,
                crate::spatial::Bounds {
                    center: data.center,
                    half_extent: data.half_extent,
                },
            );
            if object.render_options.is_static {
                if rebuild_static {
                    statics.push(entry);
                }
            } else {
                dynamics.push(entry);
            }
        }
        if rebuild_static {
            self.static_tree.update(&statics, true);
            if collect_stats {
                self.static_rebuilds += 1;
            }
            self.static_revision = Some(scene.static_revision());
        }
        let rebuilt = self.dynamic_tree.update(&dynamics, false);
        if collect_stats && rebuilt {
            self.dynamic_rebuilds += 1;
        }
        self.render_revision = Some(scene.render_revision());
    }
    fn query<const TRACK: bool>(
        &self,
        scene: &SceneManager,
        frustum: &Frustum,
        pass: RenderPassKind,
        output: &mut Vec<u32>,
    ) -> CullingStats {
        let mut stats = CullingStats {
            static_rebuilds: self.static_rebuilds,
            dynamic_rebuilds: self.dynamic_rebuilds,
            ..Default::default()
        };
        let intersects =
            |b: crate::spatial::Bounds| frustum.intersects_aabb(b.center, b.half_extent);
        let eligible = |id| included(scene, id, &scene.objects()[&id], pass);
        if pass != RenderPassKind::ShadowDynamic {
            self.static_tree.query::<TRACK>(
                &intersects,
                &eligible,
                output,
                &mut stats.nodes_visited,
                &mut stats.object_tests,
            );
        }
        if pass != RenderPassKind::ShadowStatic && pass != RenderPassKind::Environment {
            self.dynamic_tree.query::<TRACK>(
                &intersects,
                &eligible,
                output,
                &mut stats.nodes_visited,
                &mut stats.object_tests,
            );
        }
        output.sort_unstable();
        stats
    }
}

fn included(
    scene: &SceneManager,
    id: u32,
    object: &crate::scene::SceneObject,
    pass: RenderPassKind,
) -> bool {
    if scene.is_numeric_panel(id) {
        return false;
    }
    let options = object.render_options;
    match pass {
        RenderPassKind::Main => true,
        RenderPassKind::Environment => options.is_static && Some(id) != scene.mirror_object,
        RenderPassKind::Reflection => Some(id) != scene.mirror_object,
        RenderPassKind::ShadowStatic => options.casts_shadow && options.is_static,
        RenderPassKind::ShadowDynamic => options.casts_shadow && !options.is_static,
    }
}

#[derive(Default)]
pub struct RenderQueue {
    pub batches: [Vec<InstanceData>; 3],
    pub imported: std::collections::BTreeMap<u32, Vec<InstanceData>>,
    pub total: usize,
    pub visible: usize,
    pub culling_stats: CullingStats,
    pub collect_stats: bool,
}
impl RenderQueue {
    pub fn extract(&mut self, scene: &SceneManager, view_projection: Mat4, culling: bool) {
        self.extract_pass(scene, view_projection, culling, RenderPassKind::Main);
    }
    pub fn extract_pass(
        &mut self,
        scene: &SceneManager,
        view_projection: Mat4,
        culling: bool,
        pass: RenderPassKind,
    ) {
        for batch in &mut self.batches {
            batch.clear();
        }
        self.imported.retain(|id, _| scene.mesh_exists(*id));
        for batch in self.imported.values_mut() {
            batch.clear();
        }
        self.total = 0;
        self.visible = 0;
        let frustum = Frustum::from_view_projection(view_projection);
        let mut candidates = Vec::new();
        self.culling_stats = CullingStats::default();
        if culling {
            let index = scene.visibility_index(self.collect_stats);
            self.total = if pass == RenderPassKind::Reflection {
                index.totals[RenderPassKind::Main.index()]
                    - usize::from(
                        scene
                            .mirror_object
                            .is_some_and(|id| scene.object_exists(id)),
                    )
            } else {
                index.totals[pass.index()]
            };
            self.culling_stats = if self.collect_stats {
                index.query::<true>(scene, &frustum, pass, &mut candidates)
            } else {
                index.query::<false>(scene, &frustum, pass, &mut candidates)
            };
        } else {
            candidates.extend(
                scene
                    .objects()
                    .iter()
                    .filter_map(|(&id, object)| included(scene, id, object, pass).then_some(id)),
            );
            self.total = candidates.len();
        }
        for id in &candidates {
            let object = &scene.objects()[id];
            let options = object.render_options;
            let data = &object.render_data;
            let index = match object.mesh {
                MeshKind::Cube => 0,
                MeshKind::Plane => 1,
                MeshKind::Sphere => 2,
                MeshKind::Imported(_) => 3,
            };
            let mut instance = data.instance;
            instance.emission[0] = object.emission;
            instance.emission[1] = object.color_cutout;
            instance.emission[2] = object.opacity;
            instance.emission[3] = object.opaque_above;
            instance.surface_params[0] = object.opacity_variation;
            instance.surface_params[1] = object.opacity_coverage;
            if let Some(&(handle, variant)) = scene.variant_instances.get(id) {
                instance.surface_params[2] = variant as f32;
                instance.surface_params[3] =
                    scene.mesh_variants[&handle].varying_triangles.len() as f32;
            }
            instance.material = [
                options.reflectivity,
                if options.receives_shadow { 1.0 } else { 0.0 },
                if Some(*id) == scene.mirror_object {
                    1.0
                } else {
                    0.0
                },
                // Negative material flag selects text; numeric tubes use their own profile.
                if scene.is_numeric_glyph(*id) {
                    -2.0
                } else if scene.is_text_object(*id) {
                    -1.0
                } else if scene.is_cuttable(*id) {
                    1.0
                } else {
                    0.0
                },
            ];
            if let MeshKind::Imported(mesh_id) = object.mesh {
                self.imported.entry(mesh_id).or_default().push(instance);
            } else {
                self.batches[index].push(instance);
            }
            self.visible += 1;
        }
    }
    pub fn draw_calls(&self) -> usize {
        self.batches
            .iter()
            .filter(|batch| !batch.is_empty())
            .count()
            + self
                .imported
                .values()
                .filter(|batch| !batch.is_empty())
                .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assert_brute_force(scene: &SceneManager, matrix: Mat4, pass: RenderPassKind, culling: bool) {
        let frustum = Frustum::from_view_projection(matrix);
        let mut expected = RenderQueue::default();
        for (&id, object) in scene.objects() {
            let options = object.render_options;
            let eligible = !scene.is_numeric_panel(id)
                && match pass {
                    RenderPassKind::Main => true,
                    RenderPassKind::Environment => {
                        options.is_static && Some(id) != scene.mirror_object
                    }
                    RenderPassKind::Reflection => Some(id) != scene.mirror_object,
                    RenderPassKind::ShadowStatic => options.is_static && options.casts_shadow,
                    RenderPassKind::ShadowDynamic => !options.is_static && options.casts_shadow,
                };
            if !eligible {
                continue;
            }
            expected.total += 1;
            if culling && !object.render_data.intersects(&frustum) {
                continue;
            }
            expected.visible += 1;
            let mut instance = object.render_data.instance;
            let options = object.render_options;
            instance.material = [
                options.reflectivity,
                if options.receives_shadow { 1.0 } else { 0.0 },
                if Some(id) == scene.mirror_object {
                    1.0
                } else {
                    0.0
                },
                if scene.is_numeric_glyph(id) {
                    -2.0
                } else if scene.is_text_object(id) {
                    -1.0
                } else if scene.is_cuttable(id) {
                    1.0
                } else {
                    0.0
                },
            ];
            match object.mesh {
                MeshKind::Cube => expected.batches[0].push(instance),
                MeshKind::Plane => expected.batches[1].push(instance),
                MeshKind::Sphere => expected.batches[2].push(instance),
                MeshKind::Imported(mesh) => {
                    expected.imported.entry(mesh).or_default().push(instance)
                }
            }
        }
        for track in [false, true] {
            let mut actual = RenderQueue {
                collect_stats: track,
                ..Default::default()
            };
            actual.extract_pass(scene, matrix, culling, pass);
            assert_eq!(
                (actual.total, actual.visible),
                (expected.total, expected.visible)
            );
            for (a, b) in actual.batches.iter().zip(&expected.batches) {
                assert_instances(a, b);
            }
            assert_eq!(
                actual.imported.values().filter(|v| !v.is_empty()).count(),
                expected.imported.len()
            );
            for (id, batch) in &expected.imported {
                assert_instances(&actual.imported[id], batch);
            }
            if !track || !culling {
                assert_eq!(
                    (
                        actual.culling_stats.nodes_visited,
                        actual.culling_stats.object_tests
                    ),
                    (0, 0)
                );
            }
        }
    }
    fn assert_instances(actual: &[InstanceData], expected: &[InstanceData]) {
        assert_eq!(actual.len(), expected.len());
        for (a, b) in actual.iter().zip(expected) {
            assert_eq!(a.model, b.model);
            assert_eq!(a.normal_matrix, b.normal_matrix);
            assert_eq!(a.material, b.material);
        }
    }
    fn compare_passes(scene: &SceneManager) {
        let reflected_view = Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0))
            * Mat4::from_translation(Vec3::new(-2.0, -1.0, 0.0));
        for matrix in [
            camera(),
            camera() * reflected_view,
            crate::render_math::shadow_view_projection(Vec3::new(-3.0, 7.0, 4.0)),
            camera() * Mat4::from_translation(Vec3::new(-200.0, 0.0, 0.0)),
        ] {
            for pass in [
                RenderPassKind::Main,
                RenderPassKind::Reflection,
                RenderPassKind::ShadowStatic,
                RenderPassKind::ShadowDynamic,
                RenderPassKind::Environment,
            ] {
                for culling in [false, true] {
                    assert_brute_force(scene, matrix, pass, culling);
                }
            }
        }
    }
    #[test]
    fn environment_includes_static_neon_without_shadow_and_excludes_dynamic() {
        let mut scene = SceneManager::default();
        let neon = scene.add_cube(Vec3::ZERO);
        scene.set_render_options(
            neon,
            crate::scene::RenderOptions {
                is_static: true,
                casts_shadow: false,
                ..Default::default()
            },
        );
        let dynamic = scene.add_cube(Vec3::ZERO);
        let revision = scene.static_revision();
        assert!(scene.set_emission(neon, 1.5));
        assert!(scene.static_revision() > revision);
        assert!(!scene.set_emission(neon, f32::NAN));
        assert!(!scene.set_emission(neon, -1.0));
        let mut queue = RenderQueue::default();
        queue.extract_pass(&scene, Mat4::IDENTITY, false, RenderPassKind::Environment);
        assert_eq!(queue.visible, 1);
        assert_eq!(queue.batches[0][0].emission[0], 1.5);
        assert!(scene.set_emission(dynamic, 1.0));
        assert_eq!(scene.static_revision(), revision + 1);
    }
    #[test]
    fn bvh_matches_brute_force_across_passes_mutations_and_planar_imports() {
        let mut scene = SceneManager::default();
        let floor = scene.add_plane(Vec3::new(0.0, 0.0, 2.0));
        scene.set_render_options(
            floor,
            crate::scene::RenderOptions {
                reflectivity: 0.8,
                is_static: true,
                ..Default::default()
            },
        );
        scene.configure_mirror(floor);
        for i in 0..180 {
            let mesh = [MeshKind::Cube, MeshKind::Plane, MeshKind::Sphere][i % 3];
            let id = scene.add_object(
                mesh,
                Transform::new(
                    Vec3::new(
                        (i % 19) as f32 - 9.0,
                        (i % 7) as f32 - 3.0,
                        (i % 23) as f32 - 5.0,
                    ),
                    Vec3::new(0.3, 0.7, -0.2),
                    Vec3::new(-1.5, 0.2, 2.0),
                ),
            );
            scene.set_render_options(
                id,
                crate::scene::RenderOptions {
                    is_static: i % 2 == 0,
                    casts_shadow: i % 5 != 0,
                    receives_shadow: i % 3 == 0,
                    reflectivity: 0.25,
                },
            );
        }
        let imported = scene.add_imported(Vec3::new(2.0, 0.0, 1.0), Vec3::new(4.0, 0.0, 3.0));
        // Multiple object IDs share one imported batch; spatial traversal order
        // must not reorder its instances or change mirrored winding matrices.
        scene.add_object(
            MeshKind::Imported(imported),
            Transform::new(
                Vec3::new(0.0, 0.0, 2.0),
                Vec3::ZERO,
                Vec3::new(-1.0, 1.0, 1.0),
            ),
        );
        scene.add_object(
            MeshKind::Imported(imported),
            Transform::new(Vec3::new(1.0, 0.0, 3.0), Vec3::ZERO, Vec3::ONE),
        );
        scene.set_object_transform(
            imported,
            Vec3::new(-2.0, 0.0, 1.0),
            Vec3::new(0.3, 0.2, 0.8),
            Vec3::new(-2.0, 1.0, 0.5),
        );
        compare_passes(&scene);
        scene.set_object_position(imported, Vec3::new(100.0, 0.0, 0.0));
        compare_passes(&scene);
        scene.set_render_options(
            imported,
            crate::scene::RenderOptions {
                is_static: true,
                casts_shadow: false,
                receives_shadow: false,
                reflectivity: 0.6,
            },
        );
        compare_passes(&scene);
        scene.set_render_options(imported, crate::scene::RenderOptions::default());
        scene.mirror_object = None;
        compare_passes(&scene);
        scene.remove_object(imported);
        scene.remove_object(floor);
        compare_passes(&scene);
        scene.clear_scene();
        compare_passes(&scene);
        scene.add_cube(Vec3::new(0.0, 0.0, 2.0));
        compare_passes(&scene);
    }
    #[test]
    fn sparse_bvh_prunes_objects_and_refits_without_static_rebuilds() {
        let mut scene = SceneManager::default();
        scene.load_culling_demo();
        let static_id = *scene.objects().keys().next().unwrap();
        scene.set_render_options(
            static_id,
            crate::scene::RenderOptions {
                is_static: true,
                ..Default::default()
            },
        );
        let dynamic_id = *scene.objects().keys().nth(1).unwrap();
        let mut queue = RenderQueue {
            collect_stats: true,
            ..Default::default()
        };
        queue.extract(&scene, camera(), true);
        let stats = queue.culling_stats;
        assert!(stats.object_tests < 1000, "{:?}", stats);
        assert!(stats.nodes_visited > 0);
        for matrix in [
            camera(),
            camera() * Mat4::from_translation(Vec3::new(-100.0, 0.0, 0.0)),
        ] {
            queue.extract(&scene, matrix, true);
            assert_eq!(queue.culling_stats.static_rebuilds, stats.static_rebuilds);
            assert_eq!(queue.culling_stats.dynamic_rebuilds, stats.dynamic_rebuilds);
        }
        scene.set_object_position(dynamic_id, Vec3::new(0.0, 0.0, 2.0));
        queue.extract(&scene, camera(), true);
        assert_eq!(queue.culling_stats.static_rebuilds, stats.static_rebuilds);
        assert_eq!(queue.culling_stats.dynamic_rebuilds, stats.dynamic_rebuilds);
        assert_brute_force(&scene, camera(), RenderPassKind::Main, true);
        scene.set_object_position(static_id, Vec3::new(0.0, 0.0, 2.0));
        queue.extract(&scene, camera(), true);
        assert_eq!(
            queue.culling_stats.static_rebuilds,
            stats.static_rebuilds + 1
        );
        scene.remove_object(dynamic_id);
        queue.extract(&scene, camera(), true);
        assert_eq!(
            queue.culling_stats.dynamic_rebuilds,
            stats.dynamic_rebuilds + 1
        );
        queue.extract(&scene, camera(), false);
        assert_eq!((queue.total, queue.visible), (9999, 9999));
        assert_eq!(
            (
                queue.culling_stats.nodes_visited,
                queue.culling_stats.object_tests
            ),
            (0, 0)
        );
    }
    #[test]
    fn disabled_diagnostics_do_not_count_rebuilds_or_queries() {
        let mut scene = SceneManager::default();
        let id = scene.add_cube(Vec3::ZERO);
        scene.set_render_options(
            id,
            crate::scene::RenderOptions {
                is_static: true,
                ..Default::default()
            },
        );
        let mut queue = RenderQueue::default();
        queue.extract(&scene, camera(), true);
        scene.set_object_position(id, Vec3::X);
        queue.extract(&scene, camera(), true);
        {
            let index = scene.visibility_index(false);
            assert_eq!((index.static_rebuilds, index.dynamic_rebuilds), (0, 0));
        }
        assert_eq!(
            (
                queue.culling_stats.nodes_visited,
                queue.culling_stats.object_tests
            ),
            (0, 0)
        );
        queue.collect_stats = true;
        scene.set_object_position(id, Vec3::ZERO);
        queue.extract(&scene, camera(), true);
        assert_eq!(queue.culling_stats.static_rebuilds, 1);
        queue.collect_stats = false;
        scene.set_object_position(id, Vec3::X);
        queue.extract(&scene, camera(), true);
        assert_eq!(scene.visibility_index(false).static_rebuilds, 1);
    }
    fn camera() -> Mat4 {
        glam::camera::lh::proj::directx::perspective(1.0, 1.0, 0.1, 10.0)
    }
    #[test]
    fn ssr_reflectivity_is_independent_of_planar_mirror_registration() {
        let mut scene = SceneManager::default();
        let floor = scene.add_plane(Vec3::ZERO);
        let cube = scene.add_cube(Vec3::ZERO);
        for id in [floor, cube] {
            assert!(scene.set_render_options(
                id,
                crate::scene::RenderOptions {
                    reflectivity: 0.85,
                    ..Default::default()
                }
            ));
        }
        let mut queue = RenderQueue::default();
        queue.extract(&scene, Mat4::IDENTITY, false);
        for batch in &queue.batches[..2] {
            assert_eq!(batch[0].material[0], 0.85);
            assert_eq!(batch[0].material[2], 0.0);
        }
        assert!(scene.configure_mirror(floor));
        queue.extract(&scene, Mat4::IDENTITY, false);
        assert_eq!(queue.batches[1][0].material[2], 1.0);
        assert_eq!(queue.batches[0][0].material[2], 0.0);
    }
    #[test]
    fn pass_filtering_partitions_casters_and_excludes_mirror_feedback() {
        let mut scene = SceneManager::default();
        scene.load_effects_demo();
        let mut queue = RenderQueue::default();
        queue.extract_pass(&scene, Mat4::IDENTITY, false, RenderPassKind::ShadowStatic);
        assert_eq!(queue.visible, 7);
        queue.extract_pass(&scene, Mat4::IDENTITY, false, RenderPassKind::ShadowDynamic);
        assert_eq!(queue.visible, 2);
        queue.extract_pass(&scene, Mat4::IDENTITY, false, RenderPassKind::Reflection);
        assert_eq!(queue.visible, 10);
        queue.extract(&scene, Mat4::IDENTITY, false);
        assert_eq!(queue.visible, 11);
        // A caster outside the main frustum remains eligible in the light frustum.
        let id = scene.add_cube(Vec3::new(5.0, 0.0, 0.0));
        let main = Frustum::from_view_projection(camera());
        assert!(!scene.objects()[&id].render_data.intersects(&main));
        let light = Frustum::from_view_projection(crate::render_math::shadow_view_projection(
            Vec3::new(-3.0, 7.0, 4.0),
        ));
        assert!(scene.objects()[&id].render_data.intersects(&light));
    }
    #[test]
    fn spatial_demo_spans_the_world_and_camera_can_leave_it() {
        let mut scene = SceneManager::default();
        scene.load_culling_demo();
        assert_eq!(scene.objects().len(), 10000);
        assert_eq!(scene.lights().len(), 4);
        let mut queue = RenderQueue::default();
        queue.extract(&scene, camera(), true);
        assert!(queue.visible > 0 && queue.visible < queue.total);
        let outside = glam::camera::lh::view::look_at_mat4(
            Vec3::new(100.0, 0.0, 0.0),
            Vec3::new(200.0, 0.0, 0.0),
            Vec3::Y,
        );
        queue.extract(&scene, camera() * outside, true);
        assert_eq!(queue.visible, 0);
        queue.extract(&scene, camera() * outside, false);
        assert_eq!((queue.visible, queue.draw_calls()), (10000, 1));
    }
    #[test]
    fn moving_camera_and_intersecting_bounds_do_not_lose_visible_objects() {
        let projection = camera();
        let view = glam::camera::lh::view::look_at_mat4(
            Vec3::new(5.0, 2.0, -3.0),
            Vec3::new(5.0, 2.0, 0.0),
            Vec3::Y,
        );
        let f = Frustum::from_view_projection(projection * view);
        assert!(f.intersects_aabb(Vec3::new(5.0, 2.0, 0.0), Vec3::splat(0.1)));
        assert!(!f.intersects_aabb(Vec3::new(5.0, 2.0, -5.0), Vec3::splat(0.1)));
        // The center is outside, but the large object's bounds intersect the view.
        assert!(Frustum::from_view_projection(projection)
            .intersects_aabb(Vec3::new(2.0, 0.0, 1.0), Vec3::new(2.0, 0.1, 0.1),));
    }
    #[test]
    fn animation_refreshes_cached_instance_and_normal_matrices() {
        let mut scene = SceneManager::default();
        let id = scene.add_cube(Vec3::new(0.0, 0.0, 2.0));
        scene.set_object_scale(id, Vec3::new(2.0, 0.5, 1.0));
        scene.schedule_rotate(id, 0.0, 90.0, 0.0, 0.0, 1.0);
        scene.update(0.5);
        let object = &scene.objects()[&id];
        assert_eq!(
            object.render_data.instance.model,
            object.transform.model().to_cols_array_2d()
        );
        assert_eq!(
            object.render_data.instance.normal_matrix,
            crate::scene::normal_matrix(object.transform.model()).to_cols_array_2d()
        );
        assert_eq!(std::mem::size_of::<InstanceData>(), 176);
        assert_eq!(std::mem::offset_of!(InstanceData, surface_params), 160);
    }
    #[test]
    fn render_revision_invalidates_on_commands_and_animation_but_not_idle_frames() {
        let mut scene = SceneManager::default();
        let id = scene.add_cube(Vec3::ZERO);
        let mut previous = scene.render_revision();
        scene.update(1.0);
        assert_eq!(previous, scene.render_revision());
        scene.set_object_position(id, Vec3::ONE);
        assert_ne!(previous, scene.render_revision());
        previous = scene.render_revision();
        scene.set_object_scale(id, Vec3::splat(2.0));
        assert_ne!(previous, scene.render_revision());
        previous = scene.render_revision();
        scene.set_object_rotation(id, Vec3::ONE);
        assert_ne!(previous, scene.render_revision());
        previous = scene.render_revision();
        scene.set_object_transform(id, Vec3::ZERO, Vec3::ZERO, Vec3::ONE);
        assert_ne!(previous, scene.render_revision());
        previous = scene.render_revision();
        scene.schedule_rotate(id, 0.0, 90.0, 0.0, 0.0, 1.0);
        scene.update(0.5);
        assert_ne!(previous, scene.render_revision());
        previous = scene.render_revision();
        scene.remove_object(id);
        assert_ne!(previous, scene.render_revision());
        previous = scene.render_revision();
        scene.clear_scene();
        assert_ne!(previous, scene.render_revision());
    }
    #[test]
    fn clips_all_six_planes_using_webgpu_depth_range() {
        let f = Frustum::from_view_projection(camera());
        let e = Vec3::splat(0.01);
        assert!(f.intersects_aabb(Vec3::new(0.0, 0.0, 1.0), e));
        for p in [
            Vec3::new(-3.0, 0.0, 1.0),
            Vec3::new(3.0, 0.0, 1.0),
            Vec3::new(0.0, -3.0, 1.0),
            Vec3::new(0.0, 3.0, 1.0),
            Vec3::new(0.0, 0.0, 0.05),
            Vec3::new(0.0, 0.0, 11.0),
            Vec3::new(0.0, 0.0, -1.0),
        ] {
            assert!(!f.intersects_aabb(p, e), "{p:?}");
        }
        assert!(f.intersects_aabb(Vec3::new(0.0, 0.0, 0.1), e));
        assert!(f.intersects_aabb(Vec3::new(0.0, 0.0, 10.0), e));
    }
    #[test]
    fn bounds_are_conservative_for_rotated_scaled_and_flat_geometry() {
        for mesh in [MeshKind::Cube, MeshKind::Plane, MeshKind::Sphere] {
            let t = Transform::new(
                Vec3::new(2.0, 0.0, 2.0),
                Vec3::new(0.4, 0.7, 0.3),
                Vec3::new(-3.0, 0.4, 2.0),
            );
            let data = ObjectRenderData::new(mesh, t);
            let ext = match mesh {
                MeshKind::Cube => Vec3::splat(0.5),
                MeshKind::Plane => Vec3::new(0.5, 0.0, 0.5),
                MeshKind::Sphere => Vec3::ONE,
                MeshKind::Imported(_) => Vec3::ZERO,
            };
            for x in [-1.0, 1.0] {
                for y in [-1.0, 1.0] {
                    for z in [-1.0, 1.0] {
                        let point = t.model().transform_point3(Vec3::new(x, y, z) * ext);
                        assert!(
                            ((point - data.center).abs() - data.half_extent).max_element() < 1e-5
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn imported_bounds_follow_transforms_and_pass_filters() {
        let mut scene = SceneManager::default();
        let id = scene.add_imported(Vec3::new(99.9, -0.1, 0.9), Vec3::new(100.1, 0.1, 1.1));
        let mut q = RenderQueue::default();
        q.extract(&scene, camera(), true);
        assert_eq!(q.visible, 0);
        scene.set_object_position(id, Vec3::new(-100.0, 0.0, 0.0));
        q.extract(&scene, camera(), true);
        assert_eq!((q.visible, q.draw_calls()), (1, 1));
        assert_eq!(q.imported[&id].len(), 1);
        q.extract_pass(&scene, camera(), true, RenderPassKind::ShadowStatic);
        assert_eq!(q.visible, 0);
        q.extract_pass(&scene, camera(), true, RenderPassKind::ShadowDynamic);
        assert_eq!(q.visible, 1);
        scene.remove_object(id);
        q.extract(&scene, camera(), true);
        assert!(q.imported.is_empty());
    }
    #[test]
    fn non_origin_bounds_contain_rotated_negative_scaled_corners() {
        let min = Vec3::new(2.0, -1.0, 3.0);
        let max = Vec3::new(4.0, 2.0, 7.0);
        let t = Transform::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.4, 0.8, 0.2),
            Vec3::new(-2.0, 3.0, 0.5),
        );
        let data = ObjectRenderData::with_bounds(t, min, max);
        for x in [min.x, max.x] {
            for y in [min.y, max.y] {
                for z in [min.z, max.z] {
                    let point = t.model().transform_point3(Vec3::new(x, y, z));
                    assert!(((point - data.center).abs() - data.half_extent).max_element() < 1e-5);
                }
            }
        }
    }
    #[test]
    fn culling_never_stops_simulation_and_reuses_queue_capacity() {
        let mut scene = SceneManager::default();
        let id = scene.add_cube(Vec3::new(100.0, 0.0, 1.0));
        let mut q = RenderQueue::default();
        q.extract(&scene, camera(), true);
        assert_eq!((q.total, q.visible, q.draw_calls()), (1, 0, 0));
        scene.set_object_position(id, Vec3::new(0.0, 0.0, 1.0));
        q.extract(&scene, camera(), true);
        assert_eq!((q.visible, q.draw_calls()), (1, 1));
        let capacity = q.batches[0].capacity();
        scene.set_object_scale(id, Vec3::splat(2.0));
        scene.schedule_remove_object(id, 0.1);
        scene.update(0.2);
        q.extract(&scene, camera(), true);
        assert_eq!((q.total, q.visible), (0, 0));
        assert_eq!(capacity, q.batches[0].capacity());
    }
    #[test]
    fn numeric_glyph_material_stays_distinct_from_bitmap_and_lit_meshes() {
        let mut scene = SceneManager::default();
        let numeric = scene.add_numeric_text("0", 0.5, 0.5).unwrap();
        let bitmap = scene.add_info_panel("scene", 0.5, 0.5).unwrap();
        scene.add_cube(Vec3::ZERO);
        scene.set_object_position(numeric, Vec3::new(0., 0., 0.5));
        scene.set_object_position(bitmap, Vec3::new(0., 0., 0.5));
        scene.sync_numeric_font();
        let children: Vec<_> = scene
            .objects()
            .iter()
            .filter_map(|(&id, object)| scene.is_numeric_glyph(id).then_some((id, object.mesh)))
            .collect();
        assert_eq!(children.len(), 1);
        let MeshKind::Imported(handle) = children[0].1 else {
            panic!("numeric glyph needs shared imported mesh")
        };
        for culling in [false, true] {
            for pass in [RenderPassKind::Main, RenderPassKind::Reflection] {
                let mut queue = RenderQueue::default();
                queue.extract_pass(&scene, Mat4::IDENTITY, culling, pass);
                assert_eq!(queue.imported[&handle][0].material[3], -2.);
                assert_eq!(queue.imported[&bitmap][0].material[3], -1.);
                assert_eq!(queue.batches[0][0].material[3], 0.);
                assert!(!queue.imported.contains_key(&numeric));
                assert_brute_force(&scene, Mat4::IDENTITY, pass, culling);
            }
        }
        compare_passes(&scene);
    }
    #[test]
    fn info_panel_unlit_flag_survives_all_visibility_paths() {
        let mut scene = SceneManager::default();
        let panel = scene.add_info_panel("scene", 0.5, 0.5).unwrap();
        let cube = scene.add_cube(Vec3::ZERO);
        scene.set_object_position(panel, Vec3::new(0., 0., 0.5));
        for culling in [false, true] {
            for pass in [RenderPassKind::Main, RenderPassKind::Reflection] {
                let mut queue = RenderQueue::default();
                queue.extract_pass(&scene, Mat4::IDENTITY, culling, pass);
                assert_eq!(queue.imported[&panel][0].material, [0., 0., 0., -1.]);
                assert_eq!(queue.batches[0][0].material[3], 0.);
                assert_brute_force(&scene, Mat4::IDENTITY, pass, culling);
            }
        }
        assert!(!scene.is_cuttable(panel));
        assert!(scene.object_exists(cube));
        compare_passes(&scene);
    }
}
