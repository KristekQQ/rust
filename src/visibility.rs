//! CPU visibility and render extraction; shared by native tests and WASM.
use crate::scene::{MeshKind, SceneManager, Transform};
use glam::{Mat4, Vec3, Vec4};

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct InstanceData {
    pub model: [[f32; 4]; 4],
    pub normal_matrix: [[f32; 4]; 4],
    pub material: [f32; 4],
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
        let model = transform.model();
        let local = match mesh {
            MeshKind::Cube => Vec3::splat(0.5),
            MeshKind::Plane => Vec3::new(0.5, 0.0, 0.5),
            MeshKind::Sphere => Vec3::ONE,
        };
        Self {
            instance: InstanceData {
                model: model.to_cols_array_2d(),
                normal_matrix: crate::scene::normal_matrix(model).to_cols_array_2d(),
                material: [0.0, 1.0, 0.0, 0.0],
            },
            center: transform.position,
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
}
impl RenderPassKind {
    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Default)]
pub struct RenderQueue {
    pub batches: [Vec<InstanceData>; 3],
    pub total: usize,
    pub visible: usize,
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
        self.total = 0;
        self.visible = 0;
        let frustum = Frustum::from_view_projection(view_projection);
        for (id, object) in scene.objects() {
            let options = object.render_options;
            let included = match pass {
                RenderPassKind::Main => true,
                RenderPassKind::Reflection => Some(*id) != scene.mirror_object,
                RenderPassKind::ShadowStatic => options.casts_shadow && options.is_static,
                RenderPassKind::ShadowDynamic => options.casts_shadow && !options.is_static,
            };
            if !included {
                continue;
            }
            self.total += 1;
            let data = &object.render_data;
            if culling && !frustum.intersects_aabb(data.center, data.half_extent) {
                continue;
            }
            let index = match object.mesh {
                MeshKind::Cube => 0,
                MeshKind::Plane => 1,
                MeshKind::Sphere => 2,
            };
            let mut instance = data.instance;
            instance.material = [
                if Some(*id) == scene.mirror_object {
                    options.reflectivity
                } else {
                    0.0
                },
                if options.receives_shadow { 1.0 } else { 0.0 },
                0.0,
                0.0,
            ];
            self.batches[index].push(instance);
            self.visible += 1;
        }
    }
    pub fn draw_calls(&self) -> usize {
        self.batches
            .iter()
            .filter(|batch| !batch.is_empty())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn camera() -> Mat4 {
        glam::camera::lh::proj::directx::perspective(1.0, 1.0, 0.1, 10.0)
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
        assert_eq!(std::mem::size_of::<InstanceData>(), 144);
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
}
