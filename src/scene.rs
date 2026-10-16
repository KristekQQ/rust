//! CPU scene state and simulation. No browser, JavaScript or GPU dependencies.
use glam::{Mat4, Vec3};
use std::collections::BTreeMap;

pub const MAX_LIGHTS: usize = 4;
pub const INVALID_ID: u32 = u32::MAX;

pub struct TriangleObject {
    pub mesh: crate::cutting_mesh::MeshData,
    pub cuttable: bool,
    pub cap_color: Vec3,
}
#[derive(Default)]
pub struct CutReport {
    pub removed: Vec<u32>,
    pub pieces: Vec<u32>,
    pub object_candidates: usize,
    pub bvh_nodes: usize,
    pub triangle_tests: usize,
    piece_directions: Vec<Vec3>,
}
#[derive(PartialEq)]
pub struct CutSnapshot {
    cutter: (u32, [Vec3; 2], Mat4),
    targets: Vec<(u32, Mat4, Vec3)>,
}
/// Owned immutable inputs; safe to retain while the GPU request is pending.
pub struct ResidentCandidate {
    pub id: u32,
    pub mesh: crate::cutting_mesh::MeshData,
    pub a: Vec3,
    pub b: Vec3,
    pub motion: Vec3,
    pub color: Vec3,
    pub transform: Transform,
    pub options: RenderOptions,
    pub direction: Vec3,
    pub angular: [Vec3; 2],
}
impl ResidentCandidate {
    pub fn support_dirs(&self) -> [[f32; 4]; 128] {
        std::array::from_fn(|i| {
            let matrix = Transform::new(
                Vec3::ZERO,
                self.transform.rotation + self.angular[i / 64] * ((i % 64) as f32 / 63.),
                self.transform.scale,
            )
            .model();
            matrix
                .transpose()
                .transform_vector3(Vec3::Y)
                .extend(0.)
                .to_array()
        })
    }
}

struct CutScatter {
    start: Transform,
    pivot: Vec3,
    direction: Vec3,
    angular: Vec3,
    floor_y: f32,
    distance: f32,
    duration: f32,
    elapsed: f32,
}

/// World-space normals must remain perpendicular to transformed tangents.
pub(crate) fn normal_matrix(model: Mat4) -> Mat4 {
    model.inverse().transpose()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeshKind {
    Cube,
    Plane,
    Sphere,
    Imported(u32),
}

#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,
}

impl Transform {
    pub fn new(position: Vec3, rotation: Vec3, scale: Vec3) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }
    pub fn model(&self) -> Mat4 {
        Mat4::from_translation(self.position)
            * Mat4::from_rotation_y(self.rotation.y)
            * Mat4::from_rotation_x(self.rotation.x)
            * Mat4::from_rotation_z(self.rotation.z)
            * Mat4::from_scale(self.scale)
    }
    fn is_valid(&self) -> bool {
        self.position.is_finite()
            && self.rotation.is_finite()
            && self.scale.is_finite()
            && self.scale.abs().min_element() > 0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderOptions {
    pub is_static: bool,
    pub casts_shadow: bool,
    pub receives_shadow: bool,
    pub reflectivity: f32,
}
impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            is_static: false,
            casts_shadow: true,
            receives_shadow: true,
            reflectivity: 0.0,
        }
    }
}

pub struct SceneObject {
    pub transform: Transform,
    pub mesh: MeshKind,
    pub render_options: RenderOptions,
    local_bounds: Option<(Vec3, Vec3)>,
    pub(crate) render_data: crate::visibility::ObjectRenderData,
}
impl SceneObject {
    fn refresh_render_data(&mut self) {
        self.render_data = if let Some((min, max)) = self.local_bounds {
            crate::visibility::ObjectRenderData::with_bounds(self.transform, min, max)
        } else {
            crate::visibility::ObjectRenderData::new(self.mesh, self.transform)
        };
    }
}
#[derive(Clone, Copy)]
pub struct Light {
    pub position: Vec3,
    pub color: Vec3,
}
struct RotationAction {
    id: u32,
    start: f32,
    duration: f32,
    delta: Vec3,
    elapsed: f32,
}
#[derive(Clone, Copy)]
enum RemovalKind {
    Object,
    Light,
}
struct ScheduledRemoval {
    id: u32,
    at: f32,
    kind: RemovalKind,
}
struct LightOrbit {
    radius: f32,
    height: f32,
    speed: f32,
    phase: f32,
    pulse: f32,
    color: Vec3,
}

#[derive(Clone, Copy)]
struct BoxCollider {
    half_extents: Vec3,
    offset: Vec3,
}

#[derive(Default)]
pub struct SceneManager {
    objects: BTreeMap<u32, SceneObject>,
    triangle_meshes: BTreeMap<u32, TriangleObject>,
    cutters: BTreeMap<u32, [Vec3; 2]>,
    cutting_index: crate::spatial::Bvh,
    cutting_swings: BTreeMap<u32, (Vec3, Vec3, f32)>,
    cutting_scatter: BTreeMap<u32, CutScatter>,
    // Current preview: cutter, first dashed-line handle, plane/origin/finite axis.
    cutting_guide: Option<(u32, u32, [[f32; 4]; 3])>,
    models: BTreeMap<u32, crate::gltf_model::ModelInstance>,
    lights: BTreeMap<u32, Light>,
    light_orbits: BTreeMap<u32, LightOrbit>,
    next_id: u32,
    render_revision: u64,
    static_revision: u64,
    spins: BTreeMap<u32, Vec3>,
    pub paused: bool,
    pub mirror_object: Option<u32>,
    time: f32,
    rotations: Vec<RotationAction>,
    removals: Vec<ScheduledRemoval>,
    colliders: BTreeMap<u32, BoxCollider>,
    collision_world: crate::collision::CollisionWorld,
    collision_dirty: bool,
    visibility_index: std::cell::RefCell<crate::visibility::VisibilityIndex>,
}

impl SceneManager {
    pub(crate) fn visibility_index(
        &self,
        collect_stats: bool,
    ) -> std::cell::RefMut<'_, crate::visibility::VisibilityIndex> {
        let mut index = self.visibility_index.borrow_mut();
        index.sync(self, collect_stats);
        index
    }
    pub fn configure_mirror(&mut self, id: u32) -> bool {
        if !self
            .objects
            .get(&id)
            .is_some_and(|o| o.mesh == MeshKind::Plane && o.render_options.reflectivity > 0.0)
        {
            return false;
        }
        self.mirror_object = Some(id);
        self.changed(id);
        true
    }
    pub fn mirror_plane(&self) -> Option<crate::render_math::MirrorPlane> {
        let object = self.mirror_object.and_then(|id| self.objects.get(&id))?;
        if object.render_options.reflectivity <= 0.0 {
            return None;
        }
        let normal = object.render_data.plane_normal();
        crate::render_math::MirrorPlane::new(normal, -normal.dot(object.transform.position))
    }
    pub fn mirror_visible(&self, matrix: Mat4, culling: bool) -> bool {
        let Some(object) = self.mirror_object.and_then(|id| self.objects.get(&id)) else {
            return false;
        };
        !culling
            || object
                .render_data
                .intersects(&crate::visibility::Frustum::from_view_projection(matrix))
    }
    pub fn static_revision(&self) -> u64 {
        self.static_revision
    }
    fn changed(&mut self, id: u32) {
        if self.colliders.contains_key(&id) {
            self.collision_dirty = true;
        }
        self.render_revision = self.render_revision.wrapping_add(1);
        if self
            .objects
            .get(&id)
            .is_some_and(|o| o.render_options.is_static)
        {
            self.static_revision = self.static_revision.wrapping_add(1);
        }
    }
    pub fn set_render_options(&mut self, id: u32, options: RenderOptions) -> bool {
        if !options.reflectivity.is_finite() || !(0.0..=1.0).contains(&options.reflectivity) {
            return false;
        }
        let Some(object) = self.objects.get_mut(&id) else {
            return false;
        };
        let was_static = object.render_options.is_static;
        object.render_options = options;
        self.changed(id);
        if was_static && !options.is_static {
            self.static_revision = self.static_revision.wrapping_add(1);
        }
        true
    }

    /// Explicit local-space box, independent of render geometry or animation bounds.
    pub fn set_box_collider(&mut self, id: u32, half_extents: Vec3, offset: Vec3) -> bool {
        if !self.object_exists(id)
            || !half_extents.is_finite()
            || half_extents.min_element() <= 0.0
            || !offset.is_finite()
        {
            return false;
        }
        self.colliders.insert(
            id,
            BoxCollider {
                half_extents,
                offset,
            },
        );
        self.collision_dirty = true;
        true
    }

    pub fn remove_collider(&mut self, id: u32) -> bool {
        if !self.object_exists(id) {
            return false;
        }
        self.collision_dirty |= self.colliders.remove(&id).is_some();
        true
    }

    /// Cached BVH; movement of a non-solid character never rebuilds the world.
    pub fn collision_world(&mut self) -> &crate::collision::CollisionWorld {
        if self.collision_dirty {
            let colliders = self
                .colliders
                .iter()
                .filter_map(|(id, collider)| {
                    let transform = self.objects.get(id)?.transform;
                    let model = transform.model();
                    let center = model.transform_point3(collider.offset);
                    // Rotated authored boxes use conservative world-aligned bounds.
                    let half = model.x_axis.truncate().abs() * collider.half_extents.x
                        + model.y_axis.truncate().abs() * collider.half_extents.y
                        + model.z_axis.truncate().abs() * collider.half_extents.z;
                    crate::collision::Aabb::new(center - half, center + half)
                        .map(|bounds| crate::collision::Collider { id: *id, bounds })
                })
                .collect();
            self.collision_world.rebuild(colliders);
            self.collision_dirty = false;
        }
        &self.collision_world
    }
    pub fn set_object_spin(&mut self, id: u32, radians_per_second: Vec3) -> bool {
        if !self.object_exists(id) || !radians_per_second.is_finite() {
            return false;
        }
        if radians_per_second == Vec3::ZERO {
            self.spins.remove(&id);
        } else {
            self.spins.insert(id, radians_per_second);
        }
        true
    }
    pub fn render_revision(&self) -> u64 {
        self.render_revision
    }

    // IDs are never reused, including after clear/reset. Old handles stay invalid.
    fn allocate_id(&mut self) -> Option<u32> {
        if self.next_id >= i32::MAX as u32 {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        Some(id)
    }
    pub fn objects(&self) -> &BTreeMap<u32, SceneObject> {
        &self.objects
    }
    pub fn lights(&self) -> &BTreeMap<u32, Light> {
        &self.lights
    }
    pub fn models(&self) -> &BTreeMap<u32, crate::gltf_model::ModelInstance> {
        &self.models
    }
    pub fn model(&self, id: u32) -> Option<&crate::gltf_model::ModelInstance> {
        self.models.get(&id)
    }
    pub fn add_model(
        &mut self,
        asset: std::sync::Arc<crate::gltf_model::ModelAsset>,
    ) -> Result<u32, String> {
        let model = crate::gltf_model::ModelInstance::new(asset, None)?;
        let (min, max) = crate::gltf_animation::model_bounds(&model.asset, &model.pose)?;
        let id = self.add_imported(min, max);
        if id == INVALID_ID {
            return Err("Cannot allocate model root".into());
        }
        self.models.insert(id, model);
        Ok(id)
    }
    fn commit_model(
        &mut self,
        id: u32,
        model: crate::gltf_model::ModelInstance,
    ) -> Result<(), String> {
        let (min, max) = crate::gltf_animation::model_bounds(&model.asset, &model.pose)?;
        let object = self
            .objects
            .get_mut(&id)
            .ok_or("Model root does not exist")?;
        object.local_bounds = Some((min, max));
        object.refresh_render_data();
        self.models.insert(id, model);
        self.changed(id);
        Ok(())
    }
    fn mutate_model(
        &mut self,
        id: u32,
        operation: impl FnOnce(&mut crate::gltf_model::ModelInstance) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut candidate = self
            .models
            .get(&id)
            .ok_or("Model root does not exist")?
            .clone();
        operation(&mut candidate)?;
        candidate.last_error = None;
        self.commit_model(id, candidate)
    }
    pub fn play_model_animation(
        &mut self,
        id: u32,
        clip: usize,
        looping: bool,
        speed: f32,
    ) -> Result<(), String> {
        if !speed.is_finite() {
            return Err("Animation speed must be finite".into());
        }
        self.mutate_model(id, |model| {
            model.play(clip, looping)?;
            model.playback.as_mut().unwrap().speed = speed;
            Ok(())
        })
    }
    pub fn pause_model_animation(&mut self, id: u32, paused: bool) -> Result<(), String> {
        let model = self
            .models
            .get_mut(&id)
            .ok_or("Model root does not exist")?;
        model
            .playback
            .as_mut()
            .ok_or("Model has no active animation")?
            .playing = !paused;
        model.playback_revision = model.playback_revision.wrapping_add(1);
        Ok(())
    }
    pub fn seek_model_animation(&mut self, id: u32, time: f32) -> Result<(), String> {
        self.mutate_model(id, |model| model.seek(time))
    }
    pub fn stop_model_animation(&mut self, id: u32) -> Result<(), String> {
        self.mutate_model(id, |model| model.stop())
    }
    pub fn select_model_scene(&mut self, id: u32, scene: usize) -> Result<(), String> {
        self.mutate_model(id, |model| model.select_scene(scene))
    }
    pub fn set_model_morph_weights(
        &mut self,
        id: u32,
        node: usize,
        weights: &[f32],
    ) -> Result<(), String> {
        self.mutate_model(id, |model| model.set_morph_weights(node, weights))
    }
    pub fn object_exists(&self, id: u32) -> bool {
        self.objects.contains_key(&id)
    }
    pub fn light_exists(&self, id: u32) -> bool {
        self.lights.contains_key(&id)
    }
    pub fn add_object(&mut self, mesh: MeshKind, transform: Transform) -> u32 {
        if !transform.is_valid() {
            return INVALID_ID;
        }
        let Some(id) = self.allocate_id() else {
            return INVALID_ID;
        };
        self.objects.insert(
            id,
            SceneObject {
                mesh,
                transform,
                render_options: RenderOptions::default(),
                local_bounds: None,
                render_data: crate::visibility::ObjectRenderData::new(mesh, transform),
            },
        );
        self.render_revision = self.render_revision.wrapping_add(1);
        id
    }
    pub fn add_imported(&mut self, min: Vec3, max: Vec3) -> u32 {
        if !min.is_finite() || !max.is_finite() || min.cmpgt(max).any() {
            return INVALID_ID;
        }
        let id = self.add_object(
            MeshKind::Imported(0),
            Transform::new(Vec3::ZERO, Vec3::ZERO, Vec3::ONE),
        );
        if let Some(object) = self.objects.get_mut(&id) {
            object.mesh = MeshKind::Imported(id);
            object.local_bounds = Some((min, max));
            object.refresh_render_data();
        }
        id
    }
    pub fn triangle_meshes(&self) -> &BTreeMap<u32, TriangleObject> {
        &self.triangle_meshes
    }
    pub fn add_triangle_mesh(&mut self, mesh: crate::cutting::CutMesh) -> Result<u32, String> {
        self.add_mesh_data(mesh.into())
    }
    fn add_mesh_data(&mut self, mesh: crate::cutting_mesh::MeshData) -> Result<u32, String> {
        let total: usize = self
            .triangle_meshes
            .values()
            .map(|o| o.mesh.triangle_count())
            .sum();
        if total + mesh.triangle_count() > 200_000 {
            return Err("Scene cutting triangle budget exceeded".into());
        }
        let (min, max) = mesh.bounds();
        let id = self.add_imported(min, max);
        if id == INVALID_ID {
            return Err("Object IDs exhausted".into());
        }
        self.triangle_meshes.insert(
            id,
            TriangleObject {
                mesh,
                cuttable: false,
                cap_color: Vec3::new(1., 0.3, 0.12),
            },
        );
        Ok(id)
    }
    pub fn set_cuttable(&mut self, id: u32, enabled: bool, cap_color: Vec3) -> Result<(), String> {
        if !cap_color.is_finite() || cap_color.min_element() < 0. || cap_color.max_element() > 1. {
            return Err("Invalid cap color".into());
        }
        if enabled && self.cutters.contains_key(&id) {
            return Err("Cutter cannot also be cuttable".into());
        }
        let object = self
            .triangle_meshes
            .get_mut(&id)
            .ok_or("Cuttable requires a custom triangle mesh")?;
        object.cuttable = enabled;
        object.cap_color = cap_color;
        self.render_revision = self.render_revision.wrapping_add(1);
        Ok(())
    }
    pub fn is_cuttable(&self, id: u32) -> bool {
        self.triangle_meshes.get(&id).is_some_and(|o| o.cuttable)
    }
    pub fn cutting_guide_uniforms(&self) -> [[f32; 4]; 3] {
        self.cutting_guide
            .filter(|(cutter, guide, _)| {
                self.cutters.contains_key(cutter)
                    && self
                        .objects
                        .get(guide)
                        .is_some_and(|o| o.transform.scale.abs().min_element() > 0.0001)
            })
            .map_or([[0.0; 4]; 3], |(_, _, uniforms)| uniforms)
    }
    pub fn aim_cutter(
        &mut self,
        id: u32,
        guides: &[u32],
        aim: crate::cutting_tool::ToolAim,
    ) -> Result<Vec3, String> {
        let blade = *self
            .cutters
            .get(&id)
            .ok_or("Object is not a registered cutter")?;
        let object = self.objects.get(&id).ok_or("Cutter no longer exists")?;
        if guides.len() > 64
            || guides.iter().any(|&guide| {
                guide == id
                    || !self.object_exists(guide)
                    || self.triangle_meshes.get(&guide).is_some_and(|o| o.cuttable)
            })
        {
            return Err("Invalid cutting guide objects".into());
        }
        let pose = aim.pose(blade, object.transform.scale)?;
        let model = Transform::new(pose.position, pose.rotation, object.transform.scale).model();
        let middle = model.transform_point3((blade[0] + blade[1]) * 0.5);
        let normal = pose.direction.cross(pose.motion).normalize();
        self.cutting_guide = guides.first().map(|&guide| {
            (
                id,
                guide,
                [
                    normal.extend(-normal.dot(middle)).to_array(),
                    middle.extend(aim.floor).to_array(),
                    pose.direction.extend(pose.length * 0.5).to_array(),
                ],
            )
        });
        self.set_object_position(id, pose.position);
        self.set_object_rotation(id, pose.rotation);
        let spacing = pose.length / (guides.len().max(1) as f32);
        for (i, &guide) in guides.iter().enumerate() {
            let along = (i as f32 + 0.5) * spacing - pose.length * 0.5;
            self.set_object_transform(
                guide,
                pose.guide_center + pose.direction * along,
                pose.rotation,
                Vec3::new(0.035, 0.008, spacing * 0.32),
            );
        }
        Ok(pose.motion)
    }
    pub fn set_cutter(&mut self, id: u32, blade: Option<[Vec3; 2]>) -> Result<(), String> {
        if !self.object_exists(id) {
            return Err("Cutter object does not exist".into());
        }
        if let Some([a, b]) = blade {
            if !a.is_finite()
                || !b.is_finite()
                || a.abs().max_element() > 10_000.
                || b.abs().max_element() > 10_000.
                || a.distance(b) < 1e-4
            {
                return Err("Invalid local blade endpoints".into());
            }
            if self.triangle_meshes.get(&id).is_some_and(|o| o.cuttable) {
                return Err("Cuttable object cannot also be cutter".into());
            }
            self.cutters.insert(id, [a, b]);
        } else {
            self.cutters.remove(&id);
        }
        self.render_revision = self.render_revision.wrapping_add(1);
        Ok(())
    }
    /// Explicit straight sweep command. Geometry and new object ownership stay in Rust.
    /// All cuts are prepared first: a topology error never removes any source object.
    pub fn cut_with(
        &mut self,
        cutter: u32,
        motion: Vec3,
        separation: f32,
    ) -> Result<CutReport, String> {
        self.cut_with_geometry(cutter, motion, separation, None)
    }
    pub fn cut_geometry_inputs(
        &self,
        cutter: u32,
        motion: Vec3,
    ) -> Result<Vec<crate::cutting::CutGeometryInput>, String> {
        if !motion.is_finite() || motion.abs().max_element() > 1000. {
            return Err("Invalid cutting sweep".into());
        }
        let blade = self
            .cutters
            .get(&cutter)
            .ok_or("Object is not a registered cutter")?;
        let model = self
            .objects
            .get(&cutter)
            .ok_or("Cutter no longer exists")?
            .transform
            .model();
        let [a, b] = blade.map(|p| model.transform_point3(p));
        let normal = (b - a).cross(motion).normalize_or_zero();
        if normal.length_squared() < 0.5 {
            return Err("Invalid cutting sweep".into());
        }
        let min = a.min(b).min(a + motion).min(b + motion) - Vec3::splat(1e-4);
        let max = a.max(b).max(a + motion).max(b + motion) + Vec3::splat(1e-4);
        let mut inputs = Vec::new();
        for (&id, source) in &self.triangle_meshes {
            if !source.cuttable {
                continue;
            }
            let model = self.objects[&id].transform.model();
            let (lo, hi) = source.mesh.bounds();
            let center = model.transform_point3((lo + hi) * 0.5);
            let half = (hi - lo) * 0.5;
            let radius = model.x_axis.truncate().abs() * half.x
                + model.y_axis.truncate().abs() * half.y
                + model.z_axis.truncate().abs() * half.z;
            if (center + radius).cmpge(min).all()
                && (center - radius).cmple(max).all()
                && normal.dot(center - a).abs() <= normal.abs().dot(radius) + 1e-4
            {
                let inverse = model.inverse();
                if !inverse.is_finite() {
                    return Err("Noninvertible cutting transform".into());
                }
                inputs.push(source.mesh.cpu()?.geometry_input(
                    id,
                    inverse.transform_point3(a),
                    inverse.transform_point3(b),
                    inverse.transform_vector3(motion),
                )?);
            }
        }
        Ok(inputs)
    }
    pub fn cut_snapshot(&self, cutter: u32) -> Result<CutSnapshot, String> {
        let blade = *self
            .cutters
            .get(&cutter)
            .ok_or("Cutter no longer registered")?;
        let model = self
            .objects
            .get(&cutter)
            .ok_or("Cutter no longer exists")?
            .transform
            .model();
        Ok(CutSnapshot {
            cutter: (cutter, blade, model),
            targets: self
                .triangle_meshes
                .iter()
                .filter(|(_, o)| o.cuttable)
                .map(|(&id, o)| (id, self.objects[&id].transform.model(), o.cap_color))
                .collect(),
        })
    }
    pub fn resident_candidates(
        &self,
        cutter: u32,
        motion: Vec3,
        separation: f32,
        scatter: Option<[f32; 3]>,
    ) -> Result<Vec<ResidentCandidate>, String> {
        if !separation.is_finite()
            || !(0.0..=2.0).contains(&separation)
            || !motion.is_finite()
            || motion.abs().max_element() > 1000.
        {
            return Err("Invalid cutting sweep".into());
        }
        let blade = self.cutters.get(&cutter).ok_or("Object is not a cutter")?;
        let matrix = self
            .objects
            .get(&cutter)
            .ok_or("Cutter no longer exists")?
            .transform
            .model();
        let [a, b] = blade.map(|p| matrix.transform_point3(p));
        let normal = (b - a).cross(motion).normalize_or_zero();
        if normal.length_squared() < 0.5 {
            return Err("Cut motion must cross blade".into());
        }
        if let Some([floor, distance, duration]) = scatter {
            if !floor.is_finite()
                || floor.abs() > 10_000.
                || !distance.is_finite()
                || !(0.0..=3.0).contains(&distance)
                || !duration.is_finite()
                || !(0.05..=3.0).contains(&duration)
                || Vec3::new(normal.x, 0., normal.z).length_squared() < 0.5e-6
            {
                return Err("Invalid fragment animation options".into());
            }
        }
        let min = a.min(b).min(a + motion).min(b + motion) - Vec3::splat(1e-4);
        let max = a.max(b).max(a + motion).max(b + motion) + Vec3::splat(1e-4);
        let mut candidates = Vec::new();
        for (&id, source) in &self.triangle_meshes {
            if !source.cuttable {
                continue;
            }
            let object = &self.objects[&id];
            let model = object.transform.model();
            let (lo, hi) = source.mesh.bounds();
            let center = model.transform_point3((lo + hi) * 0.5);
            let half = (hi - lo) * 0.5;
            let radius = model.x_axis.truncate().abs() * half.x
                + model.y_axis.truncate().abs() * half.y
                + model.z_axis.truncate().abs() * half.z;
            if (center + radius).cmpge(min).all()
                && (center - radius).cmple(max).all()
                && normal.dot(center - a).abs() <= normal.abs().dot(radius) + 1e-4
            {
                let inverse = model.inverse();
                if !inverse.is_finite() {
                    return Err("Noninvertible cutting transform".into());
                }
                let direction = normal * model.determinant().signum();
                let angular = std::array::from_fn(|side| {
                    if scatter.is_none() {
                        return Vec3::ZERO;
                    }
                    let dir = if side == 0 { direction } else { -direction };
                    let dir = Vec3::new(dir.x, 0., dir.z).normalize();
                    let sign = if dir.x + dir.z >= 0. { 1. } else { -1. };
                    Vec3::new(dir.z * 1.15, sign * 0.12, -dir.x * 1.15)
                });
                candidates.push(ResidentCandidate {
                    id,
                    mesh: source.mesh.clone(),
                    a: inverse.transform_point3(a),
                    b: inverse.transform_point3(b),
                    motion: inverse.transform_vector3(motion),
                    color: source.cap_color,
                    transform: object.transform,
                    options: object.render_options,
                    direction,
                    angular,
                });
            }
        }
        Ok(candidates)
    }
    /// Atomic scene replacement after every GPU result has been validated.
    pub fn commit_resident(
        &mut self,
        cutter: u32,
        motion: Vec3,
        separation: f32,
        scatter: Option<[f32; 3]>,
        mut pending: Vec<(ResidentCandidate, [crate::cutting_mesh::MeshData; 2])>,
        candidate_count: usize,
    ) -> Result<CutReport, String> {
        let removed: Vec<_> = pending.iter().map(|(c, _)| c.id).collect();
        let retained: usize = self
            .triangle_meshes
            .iter()
            .filter(|(id, _)| !removed.contains(id))
            .map(|(_, o)| o.mesh.triangle_count())
            .sum();
        let total: usize = pending
            .iter()
            .flat_map(|(_, pieces)| pieces)
            .map(|m| m.triangle_count())
            .sum();
        if retained + total > 200_000 {
            return Err("Scene cutting triangle budget exceeded".into());
        }
        if self
            .next_id
            .checked_add((pending.len() * 2) as u32)
            .is_none_or(|id| id == INVALID_ID)
        {
            return Err("Object IDs exhausted".into());
        }
        let cutter_start = self
            .objects
            .get(&cutter)
            .ok_or("Cutter no longer exists")?
            .transform
            .position;
        for (candidate, _) in &mut pending {
            candidate.options = self
                .objects
                .get(&candidate.id)
                .ok_or("Cut target no longer exists")?
                .render_options;
        }
        for &id in &removed {
            self.remove_object(id);
        }
        let mut report = CutReport {
            removed,
            object_candidates: candidate_count,
            ..Default::default()
        };
        for (candidate, pieces) in pending {
            for (side, piece) in pieces.into_iter().enumerate() {
                let direction = if side == 0 {
                    candidate.direction
                } else {
                    -candidate.direction
                };
                let mut transform = candidate.transform;
                transform.position += direction * separation * 0.5;
                let id = self.add_mesh_data(piece)?;
                self.set_object_transform(
                    id,
                    transform.position,
                    transform.rotation,
                    transform.scale,
                );
                self.set_render_options(id, candidate.options);
                self.set_cuttable(id, true, candidate.color)?;
                report.pieces.push(id);
                report.piece_directions.push(direction);
            }
        }
        if let Some([floor, distance, duration]) = scatter {
            self.scatter_cut_pieces(
                &report.pieces,
                &report.piece_directions,
                floor,
                distance,
                duration,
            );
        }
        self.cutting_swings
            .insert(cutter, (cutter_start, cutter_start + motion, 0.));
        Ok(report)
    }
    pub fn cut_with_geometry(
        &mut self,
        cutter: u32,
        motion: Vec3,
        separation: f32,
        geometry: Option<&BTreeMap<u32, crate::cutting::CutGeometry>>,
    ) -> Result<CutReport, String> {
        if !motion.is_finite()
            || motion.abs().max_element() > 1000.
            || !separation.is_finite()
            || !(0.0..=2.0).contains(&separation)
        {
            return Err("Invalid cutting sweep".into());
        }
        let blade = *self
            .cutters
            .get(&cutter)
            .ok_or("Object is not a registered cutter")?;
        let transform = self
            .objects
            .get(&cutter)
            .ok_or("Cutter no longer exists")?
            .transform;
        let [a, b] = blade.map(|p| transform.model().transform_point3(p));
        let normal = (b - a).cross(motion).normalize_or_zero();
        if normal.length_squared() < 0.5 {
            return Err("Cut motion must cross the blade direction".into());
        }
        let min = a.min(b).min(a + motion).min(b + motion) - Vec3::splat(1e-4);
        let max = a.max(b).max(a + motion).max(b + motion) + Vec3::splat(1e-4);
        let entries: Vec<_> = self
            .triangle_meshes
            .iter()
            .filter(|(_, o)| o.cuttable)
            .map(|(&id, o)| {
                let (min, max) = o.mesh.bounds();
                let model = self.objects[&id].transform.model();
                let center = model.transform_point3((min + max) * 0.5);
                let half = (max - min) * 0.5;
                let half_extent = model.x_axis.truncate().abs() * half.x
                    + model.y_axis.truncate().abs() * half.y
                    + model.z_axis.truncate().abs() * half.z;
                (
                    id,
                    crate::spatial::Bounds {
                        center,
                        half_extent,
                    },
                )
            })
            .collect();
        self.cutting_index.update(&entries, false);
        let mut report = CutReport::default();
        let mut candidates = vec![];
        let mut object_tests = 0;
        self.cutting_index.query::<true>(
            &|bounds| {
                (bounds.center + bounds.half_extent).cmpge(min).all()
                    && (bounds.center - bounds.half_extent).cmple(max).all()
            },
            &|_| true,
            &mut candidates,
            &mut report.bvh_nodes,
            &mut object_tests,
        );
        report.object_candidates = candidates.len();
        let mut pending = vec![];
        for id in candidates {
            let source = &self.triangle_meshes[&id];
            let object = &self.objects[&id];
            let inverse = object.transform.model().inverse();
            if !inverse.is_finite() {
                return Err("Noninvertible cutting transform".into());
            }
            if let Some(slice) = source.mesh.cpu()?.slice_prepared(
                inverse.transform_point3(a),
                inverse.transform_point3(b),
                inverse.transform_vector3(motion),
                source.cap_color,
                geometry.and_then(|g| g.get(&id)),
            )? {
                report.bvh_nodes += slice.stats.nodes;
                report.triangle_tests += slice.stats.triangle_tests;
                for piece in slice.pieces {
                    // A box center may be outside its component. A real vertex
                    // furthest from the plane identifies the correct halfspace.
                    let model = object.transform.model();
                    let distance = piece
                        .positions
                        .iter()
                        .map(|&p| normal.dot(model.transform_point3(p) - a))
                        .max_by(|a, b| a.abs().total_cmp(&b.abs()))
                        .ok_or("Empty cut piece")?;
                    let world_sign = distance.signum();
                    let mut transform = object.transform;
                    transform.position += normal * world_sign * separation * 0.5;
                    pending.push((
                        id,
                        piece,
                        transform,
                        object.render_options,
                        source.cap_color,
                        normal * world_sign,
                    ));
                }
                report.removed.push(id);
            }
        }
        if self
            .next_id
            .checked_add(pending.len() as u32)
            .is_none_or(|id| id == INVALID_ID)
        {
            return Err("Object IDs exhausted".into());
        }
        let retained: usize = self
            .triangle_meshes
            .iter()
            .filter(|(id, _)| !report.removed.contains(id))
            .map(|(_, o)| o.mesh.triangle_count())
            .sum();
        if retained + pending.iter().map(|p| p.1.triangles.len()).sum::<usize>() > 200_000 {
            return Err("Scene cutting triangle budget exceeded".into());
        }
        for &id in &report.removed {
            self.remove_object(id);
        }
        for (_, piece, transform, options, cap_color, direction) in pending {
            let id = self.add_triangle_mesh(piece)?;
            self.set_object_transform(id, transform.position, transform.rotation, transform.scale);
            self.set_render_options(id, options);
            self.set_cuttable(id, true, cap_color)?;
            report.pieces.push(id);
            report.piece_directions.push(direction);
        }
        self.cutting_swings.insert(
            cutter,
            (transform.position, transform.position + motion, 0.0),
        );
        Ok(report)
    }
    /// Presentation animation for already cut solids, not a rigid-body solver.
    /// Directions come from the actual split halfspaces, not bounding-box centers.
    fn scatter_cut_pieces(
        &mut self,
        ids: &[u32],
        directions: &[Vec3],
        floor_y: f32,
        distance: f32,
        duration: f32,
    ) {
        for (&id, &normal) in ids.iter().zip(directions) {
            let object = &self.objects[&id];
            let (min, max) = self.triangle_meshes[&id].mesh.bounds();
            let pivot = (min + max) * 0.5;
            let direction = Vec3::new(normal.x, 0.0, normal.z).normalize();
            let sign = if direction.x + direction.z >= 0.0 {
                1.0
            } else {
                -1.0
            };
            self.cutting_scatter.insert(
                id,
                CutScatter {
                    start: object.transform,
                    pivot,
                    direction,
                    angular: Vec3::new(direction.z * 1.15, sign * 0.12, -direction.x * 1.15),
                    floor_y,
                    distance,
                    duration,
                    elapsed: -0.075,
                },
            );
        }
    }
    pub fn cut_with_scatter(
        &mut self,
        cutter: u32,
        motion: Vec3,
        separation: f32,
        floor_y: f32,
        distance: f32,
        duration: f32,
    ) -> Result<CutReport, String> {
        self.cut_with_scatter_geometry(
            cutter,
            motion,
            separation,
            [floor_y, distance, duration],
            None,
        )
    }
    pub fn cut_with_scatter_geometry(
        &mut self,
        cutter: u32,
        motion: Vec3,
        separation: f32,
        scatter: [f32; 3],
        geometry: Option<&BTreeMap<u32, crate::cutting::CutGeometry>>,
    ) -> Result<CutReport, String> {
        let [floor_y, distance, duration] = scatter;
        if !floor_y.is_finite()
            || floor_y.abs() > 10_000.0
            || !distance.is_finite()
            || !(0.0..=3.0).contains(&distance)
            || !duration.is_finite()
            || !(0.05..=3.0).contains(&duration)
        {
            return Err("Invalid fragment animation options".into());
        }
        let blade = self.cutters.get(&cutter).ok_or("Object is not a cutter")?;
        let model = self
            .objects
            .get(&cutter)
            .ok_or("Cutter no longer exists")?
            .transform
            .model();
        let normal = model.transform_vector3(blade[1] - blade[0]).cross(motion);
        let axis = Vec3::new(normal.x, 0.0, normal.z).normalize_or_zero();
        if !axis.is_finite() || axis.length_squared() < 0.5 {
            return Err("Scatter requires a cut with a horizontal normal".into());
        }
        let report = self.cut_with_geometry(cutter, motion, separation, geometry)?;
        self.scatter_cut_pieces(
            &report.pieces,
            &report.piece_directions,
            floor_y,
            distance,
            duration,
        );
        Ok(report)
    }
    pub fn add_cube(&mut self, position: Vec3) -> u32 {
        self.add_object(
            MeshKind::Cube,
            Transform::new(position, Vec3::ZERO, Vec3::ONE),
        )
    }
    pub fn add_plane(&mut self, position: Vec3) -> u32 {
        self.add_object(
            MeshKind::Plane,
            Transform::new(position, Vec3::ZERO, Vec3::ONE),
        )
    }
    pub fn add_sphere(&mut self, position: Vec3) -> u32 {
        self.add_object(
            MeshKind::Sphere,
            Transform::new(position, Vec3::ZERO, Vec3::ONE),
        )
    }
    pub fn clear_scene(&mut self) {
        self.triangle_meshes.clear();
        self.cutters.clear();
        self.cutting_swings.clear();
        self.cutting_scatter.clear();
        self.cutting_guide = None;
        self.cutting_index = crate::spatial::Bvh::default();
        self.colliders.clear();
        self.collision_dirty = true;
        self.objects.clear();
        self.models.clear();
        self.mirror_object = None;
        self.spins.clear();
        self.static_revision = self.static_revision.wrapping_add(1);
        self.render_revision = self.render_revision.wrapping_add(1);
        self.rotations.clear();
        self.removals
            .retain(|r| matches!(r.kind, RemovalKind::Light));
    }
    pub fn clear_lights(&mut self) {
        self.lights.clear();
        self.light_orbits.clear();
        self.removals
            .retain(|r| matches!(r.kind, RemovalKind::Object));
    }
    pub fn clear(&mut self) {
        self.clear_scene();
        self.clear_lights();
        self.paused = false;
    }
    pub fn remove_object(&mut self, id: u32) -> bool {
        let was_static = self
            .objects
            .get(&id)
            .is_some_and(|o| o.render_options.is_static);
        if self.objects.remove(&id).is_none() {
            return false;
        }
        self.collision_dirty |= self.colliders.remove(&id).is_some();
        self.models.remove(&id);
        self.triangle_meshes.remove(&id);
        self.cutters.remove(&id);
        self.cutting_swings.remove(&id);
        self.cutting_scatter.remove(&id);
        if self
            .cutting_guide
            .is_some_and(|(cutter, guide, _)| cutter == id || guide == id)
        {
            self.cutting_guide = None;
        }
        self.render_revision = self.render_revision.wrapping_add(1);
        if was_static {
            self.static_revision = self.static_revision.wrapping_add(1);
        }
        if self.mirror_object == Some(id) {
            self.mirror_object = None;
        }
        self.spins.remove(&id);
        self.rotations.retain(|r| r.id != id);
        self.removals
            .retain(|r| r.id != id || !matches!(r.kind, RemovalKind::Object));
        true
    }
    pub fn set_object_transform(
        &mut self,
        id: u32,
        position: Vec3,
        rotation: Vec3,
        scale: Vec3,
    ) -> bool {
        let transform = Transform::new(position, rotation, scale);
        if !transform.is_valid() {
            return false;
        }
        let Some(object) = self.objects.get_mut(&id) else {
            return false;
        };
        object.transform = transform;
        object.refresh_render_data();
        self.cutting_swings.remove(&id);
        self.cutting_scatter.remove(&id);
        self.changed(id);
        true
    }
    pub fn set_object_position(&mut self, id: u32, position: Vec3) -> bool {
        if !position.is_finite() {
            return false;
        }
        let Some(object) = self.objects.get_mut(&id) else {
            return false;
        };
        object.transform.position = position;
        object.refresh_render_data();
        self.cutting_swings.remove(&id);
        self.cutting_scatter.remove(&id);
        self.changed(id);
        true
    }
    pub fn set_object_rotation(&mut self, id: u32, rotation: Vec3) -> bool {
        if !rotation.is_finite() {
            return false;
        }
        let Some(object) = self.objects.get_mut(&id) else {
            return false;
        };
        object.transform.rotation = rotation;
        object.refresh_render_data();
        self.cutting_scatter.remove(&id);
        self.changed(id);
        true
    }
    pub fn set_object_scale(&mut self, id: u32, scale: Vec3) -> bool {
        if !scale.is_finite() || scale.abs().min_element() <= 0.0 {
            return false;
        }
        let Some(object) = self.objects.get_mut(&id) else {
            return false;
        };
        object.transform.scale = scale;
        object.refresh_render_data();
        self.cutting_scatter.remove(&id);
        self.changed(id);
        true
    }
    pub fn add_light(&mut self, position: Vec3, color: Vec3) -> Option<u32> {
        if self.lights.len() >= MAX_LIGHTS || !position.is_finite() || !color.is_finite() {
            return None;
        }
        let id = self.allocate_id()?;
        self.lights.insert(id, Light { position, color });
        Some(id)
    }
    pub fn remove_light(&mut self, id: u32) -> bool {
        if self.lights.remove(&id).is_none() {
            return false;
        }
        self.light_orbits.remove(&id);
        self.removals
            .retain(|r| r.id != id || !matches!(r.kind, RemovalKind::Light));
        true
    }
    pub fn set_light(&mut self, id: u32, position: Vec3, color: Vec3) -> bool {
        if !position.is_finite() || !color.is_finite() {
            return false;
        }
        let Some(light) = self.lights.get_mut(&id) else {
            return false;
        };
        *light = Light { position, color };
        true
    }
    #[allow(clippy::too_many_arguments)] // Explicit GPU/FFI parameter layout.
    pub fn set_light_orbit(
        &mut self,
        id: u32,
        radius: f32,
        height: f32,
        speed: f32,
        phase: f32,
        pulse: f32,
        color: Vec3,
    ) -> bool {
        if !self.light_exists(id)
            || ![radius, height, speed, phase, pulse]
                .iter()
                .all(|v| v.is_finite())
            || !color.is_finite()
        {
            return false;
        }
        self.light_orbits.insert(
            id,
            LightOrbit {
                radius: radius.max(0.01),
                height,
                speed,
                phase,
                pulse: pulse.clamp(0.0, 1.0),
                color,
            },
        );
        true
    }
    pub fn clear_light_orbit(&mut self, id: u32) -> bool {
        if !self.light_exists(id) {
            return false;
        }
        self.light_orbits.remove(&id);
        true
    }
    pub fn schedule_rotate(
        &mut self,
        id: u32,
        rx_deg: f32,
        ry_deg: f32,
        rz_deg: f32,
        delay: f32,
        duration: f32,
    ) -> bool {
        let degrees = Vec3::new(rx_deg, ry_deg, rz_deg);
        if !self.object_exists(id)
            || !degrees.is_finite()
            || !delay.is_finite()
            || !duration.is_finite()
            || duration <= 0.0
        {
            return false;
        }
        self.rotations.push(RotationAction {
            id,
            start: self.time + delay.max(0.0),
            duration,
            delta: degrees * std::f32::consts::PI / 180.0,
            elapsed: 0.0,
        });
        true
    }
    pub fn schedule_remove_object(&mut self, id: u32, delay: f32) -> bool {
        if !self.object_exists(id) || !delay.is_finite() {
            return false;
        }
        self.removals.push(ScheduledRemoval {
            id,
            at: self.time + delay.max(0.0),
            kind: RemovalKind::Object,
        });
        true
    }
    pub fn schedule_remove_light(&mut self, id: u32, delay: f32) -> bool {
        if !self.light_exists(id) || !delay.is_finite() {
            return false;
        }
        self.removals.push(ScheduledRemoval {
            id,
            at: self.time + delay.max(0.0),
            kind: RemovalKind::Light,
        });
        true
    }
    pub fn update(&mut self, dt: f32) {
        if self.paused || !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let active: Vec<_> = self
            .models
            .iter()
            .filter(|(_, model)| {
                model
                    .playback
                    .as_ref()
                    .is_some_and(|p| p.playing && p.speed != 0.)
            })
            .map(|(&id, _)| id)
            .collect();
        for id in active {
            let mut candidate = self.models[&id].clone();
            let result = match candidate.update(dt) {
                Ok(true) => self.commit_model(id, candidate),
                Ok(false) => Ok(()),
                Err(error) => Err(error),
            };
            if let Err(error) = result {
                if let Some(model) = self.models.get_mut(&id) {
                    model.last_error = Some(error);
                    if let Some(playback) = &mut model.playback {
                        playback.playing = false;
                    }
                }
            }
        }
        self.time += dt;
        let now = self.time;
        let pending = std::mem::take(&mut self.removals);
        for removal in pending {
            if now < removal.at {
                self.removals.push(removal);
                continue;
            }
            match removal.kind {
                RemovalKind::Object => {
                    self.remove_object(removal.id);
                }
                RemovalKind::Light => {
                    self.remove_light(removal.id);
                }
            }
        }
        self.cutting_swings.retain(|id, (start, end, elapsed)| {
            let Some(object) = self.objects.get_mut(id) else {
                return false;
            };
            *elapsed += dt;
            let t = (*elapsed / 0.24).min(1.0);
            object.transform.position = start.lerp(*end, t * t * (3.0 - 2.0 * t));
            object.refresh_render_data();
            self.collision_dirty |= self.colliders.contains_key(id);
            self.render_revision = self.render_revision.wrapping_add(1);
            if object.render_options.is_static {
                self.static_revision = self.static_revision.wrapping_add(1);
            }
            t < 1.0
        });
        self.cutting_scatter.retain(|id, action| {
            let Some(object) = self.objects.get_mut(id) else {
                return false;
            };
            let Some(geometry) = self.triangle_meshes.get(id) else {
                return false;
            };
            action.elapsed += dt;
            if action.elapsed < 0.0 {
                return true;
            }
            let t = (action.elapsed / action.duration).min(1.0);
            let eased = 1.0 - (1.0 - t).powi(3);
            let mut transform = action.start;
            transform.rotation += action.angular * eased;
            transform.position = Vec3::ZERO;
            let pivot = action.start.model().transform_point3(action.pivot);
            transform.position = pivot + action.direction * action.distance * eased
                - transform.model().transform_point3(action.pivot);
            // Support every vertex on the board after rotation; a small hop settles
            // to exactly floor_y. Pivot rotation does not orbit around the old root.
            let bottom = geometry
                .mesh
                .bottom(transform)
                .expect("Validated cutting support");
            transform.position.y +=
                action.floor_y - bottom + (std::f32::consts::PI * t).sin() * 0.18 * (1.0 - t);
            object.transform = transform;
            object.refresh_render_data();
            self.render_revision = self.render_revision.wrapping_add(1);
            if object.render_options.is_static {
                self.static_revision = self.static_revision.wrapping_add(1);
            }
            self.collision_dirty |= self.colliders.contains_key(id);
            t < 1.0
        });
        self.rotations.retain_mut(|rotation| {
            let Some(object) = self.objects.get_mut(&rotation.id) else {
                return false;
            };
            if now < rotation.start {
                return true;
            }
            let elapsed = (now - rotation.start).min(rotation.duration);
            object.transform.rotation +=
                rotation.delta * ((elapsed - rotation.elapsed) / rotation.duration);
            rotation.elapsed = elapsed;
            object.refresh_render_data();
            self.render_revision = self.render_revision.wrapping_add(1);
            if object.render_options.is_static {
                self.static_revision = self.static_revision.wrapping_add(1);
            }
            elapsed < rotation.duration
        });
        for (id, velocity) in &self.spins {
            if let Some(object) = self.objects.get_mut(id) {
                object.transform.rotation += *velocity * dt;
                object.refresh_render_data();
                self.render_revision = self.render_revision.wrapping_add(1);
                if object.render_options.is_static {
                    self.static_revision = self.static_revision.wrapping_add(1);
                }
            }
        }
        for (id, orbit) in &self.light_orbits {
            if let Some(light) = self.lights.get_mut(id) {
                let angle = now * orbit.speed + orbit.phase;
                light.position = Vec3::new(
                    angle.cos() * orbit.radius,
                    orbit.height + (angle * 1.3).sin() * 0.3,
                    angle.sin() * orbit.radius,
                );
                let pulse =
                    1.0 - orbit.pulse * 0.5 + (now * 2.0 + orbit.phase).sin() * orbit.pulse * 0.5;
                light.color = orbit.color * pulse;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn static_revision_invalidates_only_when_static_geometry_changes() {
        use super::*;
        let mut scene = SceneManager::default();
        scene.load_effects_demo();
        let revision = scene.static_revision();
        let geometry = scene.render_revision();
        scene.update(0.5);
        assert_eq!(scene.static_revision(), revision);
        assert_ne!(scene.render_revision(), geometry);
        let id = *scene
            .objects()
            .iter()
            .find(|(_, o)| o.render_options.is_static && o.render_options.casts_shadow)
            .unwrap()
            .0;
        scene.set_object_position(id, Vec3::new(-2.0, 0.0, 0.0));
        assert_ne!(scene.static_revision(), revision);
        let revision = scene.static_revision();
        scene.remove_object(id);
        assert_ne!(scene.static_revision(), revision);
        let geometry = scene.render_revision();
        scene.paused = true;
        scene.update(1.0);
        assert_eq!(scene.render_revision(), geometry);
    }
    #[test]
    fn mirror_follows_its_transform_and_removal() {
        use super::*;
        let mut scene = SceneManager::default();
        scene.load_effects_demo();
        let id = scene.mirror_object.unwrap();
        scene.set_object_position(id, Vec3::new(0.0, 1.0, -5.0));
        let plane = scene.mirror_plane().unwrap();
        assert!((plane.normal.dot(Vec3::new(0.0, 1.0, -5.0)) + plane.distance).abs() < 1e-5);
        assert!(plane.normal.dot(Vec3::Z).abs() > 0.99);
        assert!(!scene.configure_mirror(super::INVALID_ID));
        scene.remove_object(id);
        assert!(scene.mirror_plane().is_none());
    }
    #[test]
    fn normal_transform_preserves_perpendicularity_under_nonuniform_scale() {
        let transform = super::Transform::new(
            glam::Vec3::new(3.0, -2.0, 5.0),
            glam::Vec3::new(0.4, 0.7, -0.2),
            glam::Vec3::new(2.0, 0.5, 3.0),
        );
        let model = transform.model();
        let normal = glam::Vec3::new(1.0, 1.0, 1.0).normalize();
        let tangent = glam::Vec3::new(1.0, -1.0, 0.0);
        let world_normal = super::normal_matrix(model)
            .transform_vector3(normal)
            .normalize();
        let world_tangent = model.transform_vector3(tangent).normalize();
        assert!(world_normal.dot(world_tangent).abs() < 1e-5);
        // The old model-matrix transformation fails this same geometry.
        assert!(
            model
                .transform_vector3(normal)
                .normalize()
                .dot(world_tangent)
                .abs()
                > 0.1
        );
    }
    use super::*;
    #[test]
    fn animation_is_computed_by_rust_and_finishes_exactly() {
        let mut scene = SceneManager::default();
        let id = scene.add_cube(Vec3::ZERO);
        assert!(scene.schedule_rotate(id, 0.0, 180.0, 0.0, 1.0, 2.0));
        scene.update(1.0);
        assert_eq!(scene.objects()[&id].transform.rotation, Vec3::ZERO);
        scene.update(1.0);
        assert!(
            (scene.objects()[&id].transform.rotation.y - std::f32::consts::FRAC_PI_2).abs() < 1e-5
        );
        scene.update(3.0);
        assert!((scene.objects()[&id].transform.rotation.y - std::f32::consts::PI).abs() < 1e-5);
        scene.update(3.0);
        assert!((scene.objects()[&id].transform.rotation.y - std::f32::consts::PI).abs() < 1e-5);
    }
    #[test]
    fn removed_and_reset_handles_never_target_new_objects() {
        let mut scene = SceneManager::default();
        let old = scene.add_cube(Vec3::ZERO);
        scene.schedule_remove_object(old, 2.0);
        scene.remove_object(old);
        let new = scene.add_cube(Vec3::ONE);
        assert_ne!(old, new);
        assert!(!scene.set_object_position(old, Vec3::ZERO));
        scene.update(3.0);
        assert!(scene.object_exists(new));
        scene.clear();
        assert_ne!(new, scene.add_cube(Vec3::ZERO));
    }
    #[test]
    fn lights_enforce_capacity_and_removal_does_not_resurrect_them() {
        let mut scene = SceneManager::default();
        let first = scene.add_light(Vec3::ZERO, Vec3::ONE).unwrap();
        for _ in 1..MAX_LIGHTS {
            assert!(scene.add_light(Vec3::ZERO, Vec3::ONE).is_some());
        }
        assert!(scene.add_light(Vec3::ZERO, Vec3::ONE).is_none());
        scene.remove_light(first);
        assert!(!scene.set_light_orbit(first, 1.0, 1.0, 1.0, 0.0, 0.0, Vec3::ONE));
        assert!(scene.add_light(Vec3::ZERO, Vec3::ONE).is_some());
    }
    #[test]
    fn sample_is_stable_and_invalid_values_do_not_corrupt_simulation() {
        let mut scene = SceneManager::default();
        scene.load_example();
        assert_eq!(scene.objects().len(), 3);
        assert_eq!(scene.lights().len(), 2);
        scene.update(100.0);
        assert_eq!(scene.objects().len(), 3);
        assert_eq!(scene.add_cube(Vec3::splat(f32::NAN)), INVALID_ID);
        let id = *scene.objects().keys().next().unwrap();
        assert!(!scene.set_object_scale(id, Vec3::ZERO));
        scene.update(f32::NAN);
        assert!(scene.objects()[&id].transform.model().is_finite());
    }
    #[test]
    fn scheduled_removals_and_light_orbits_run_without_javascript() {
        let mut scene = SceneManager::default();
        let cube = scene.add_cube(Vec3::ZERO);
        let light = scene.add_light(Vec3::ZERO, Vec3::ONE).unwrap();
        scene.set_light_orbit(light, 2.0, 1.0, 1.0, 0.0, 0.0, Vec3::ONE);
        scene.schedule_remove_object(cube, 1.0);
        scene.schedule_remove_light(light, 2.0);
        scene.update(1.0);
        assert!(!scene.object_exists(cube));
        assert!((scene.lights()[&light].position.x - 2.0_f32 * 1.0_f32.cos()).abs() < 1e-5);
        scene.update(1.0);
        assert!(!scene.light_exists(light));
    }

    #[test]
    fn collider_bvh_tracks_transform_removal_clear_and_only_solid_changes() {
        let mut scene = SceneManager::default();
        let cube = scene.add_cube(Vec3::ZERO);
        let query = crate::collision::Aabb::new(-Vec3::ONE, Vec3::ONE).unwrap();
        assert!(scene.collision_world().query(query, None).is_empty());
        assert!(scene.set_box_collider(cube, Vec3::splat(0.5), Vec3::ZERO));
        assert_eq!(scene.collision_world().query(query, None).len(), 1);
        assert!(!scene.collision_dirty);
        let player = scene.add_cube(Vec3::ZERO);
        scene.set_object_position(player, Vec3::ONE);
        assert!(
            !scene.collision_dirty,
            "Non-solid movement must preserve the cached BVH"
        );
        scene.set_object_transform(cube, Vec3::new(4.0, 0.0, 0.0), Vec3::ZERO, Vec3::splat(2.0));
        assert!(scene.collision_dirty);
        assert!(scene.collision_world().query(query, None).is_empty());
        let far = crate::collision::Aabb::new(Vec3::new(3.5, -0.5, -0.5), Vec3::new(4.5, 0.5, 0.5))
            .unwrap();
        assert_eq!(scene.collision_world().query(far, None).len(), 1);
        assert!(scene.remove_collider(cube));
        assert!(scene.collision_world().query(far, None).is_empty());
        scene.set_box_collider(cube, Vec3::splat(0.5), Vec3::ZERO);
        scene.remove_object(cube);
        assert!(scene.collision_world().query(far, None).is_empty());
        scene.set_box_collider(player, Vec3::ONE, Vec3::ZERO);
        scene.clear();
        assert!(scene.collision_world().query(query, None).is_empty());
        assert!(!scene.set_box_collider(player, Vec3::ONE, Vec3::ZERO));
        let invalid = scene.add_cube(Vec3::ZERO);
        assert!(!scene.set_box_collider(invalid, Vec3::ZERO, Vec3::ZERO));
    }
}
