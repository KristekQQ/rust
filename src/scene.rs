//! CPU scene state and simulation. No browser, JavaScript or GPU dependencies.
use glam::{Mat4, Vec3};
use std::collections::BTreeMap;

#[path = "cut_feedback.rs"]
pub(crate) mod cut_feedback;

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

#[derive(Clone, Copy)]
struct BladeCutState {
    progress: f32,
    cutter: u32,
    jammed: bool,
    withdrawal: f32,
    edge_velocity: Vec3,
}

#[derive(Clone, Copy)]
struct CutFragment {
    cut_speed: f32,
    family: u64,
    blade_grace_until: f32,
    radius: f32,
    generation: u8,
    expires_at: f32,
    birth_cutter: Option<u32>,
    parent: u32,
    parent_offset: Vec3,
}

const MAX_CUT_GENERATION: u8 = 4;
const MAX_FAMILY_PIECES: usize = 32;
const MIN_CUT_VOLUME: f64 = 2e-6;
const MIN_CUT_EXTENT: f32 = 0.012;

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
    pub emission: f32,
    pub color_cutout: f32,
    cut_assist: f32,
    blade_contact_response: bool,
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
    feedback: cut_feedback::Feedback,
    objects: BTreeMap<u32, SceneObject>,
    triangle_meshes: BTreeMap<u32, TriangleObject>,
    pub(crate) info_panels: BTreeMap<u32, crate::info_panel::InfoPanel>,
    cutters: BTreeMap<u32, [Vec3; 2]>,
    blade_physics: BTreeMap<u32, crate::blade_physics::BladePhysics>,
    blade_cut_states: BTreeMap<u32, BladeCutState>,
    blade_body_radii: BTreeMap<u32, (Vec3, f32)>,
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
    projectiles: BTreeMap<u32, (Vec3, f32, f32)>,
    projectile_scratch: Vec<crate::projectile::Body>,
    cut_fragments: BTreeMap<u32, CutFragment>,
    projectile_order: Vec<usize>,
    projectile_pairs: Vec<(usize, usize)>,
    pub projectile_contacts: u32,
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
    /// Vertex-color emission for primitive/triangle meshes; glTF owns its materials.
    pub fn set_emission(&mut self, id: u32, intensity: f32) -> bool {
        if !intensity.is_finite() || !(0.0..=8.0).contains(&intensity) {
            return false;
        }
        let Some(object) = self.objects.get_mut(&id) else {
            return false;
        };
        object.emission = intensity;
        self.changed(id);
        true
    }
    /// Discard dark vertex-color fragments; zero preserves the opaque surface.
    pub fn set_color_cutout(&mut self, id: u32, threshold: f32) -> bool {
        if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
            return false;
        }
        let Some(object) = self.objects.get_mut(&id) else {
            return false;
        };
        object.color_cutout = threshold;
        self.changed(id);
        true
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
    pub fn set_projectile(&mut self, id: u32, velocity: Vec3, gravity: f32, bounce: f32) -> bool {
        if !self.object_exists(id)
            || !velocity.is_finite()
            || !gravity.is_finite()
            || !bounce.is_finite()
            || gravity < 0.0
            || !(0.0..=1.0).contains(&bounce)
        {
            return false;
        }
        if gravity == 0.0 && velocity == Vec3::ZERO {
            self.blade_cut_states.remove(&id);
            self.projectiles.remove(&id);
        } else {
            self.projectiles.insert(id, (velocity, gravity, bounce));
        }
        true
    }
    #[cfg(test)]
    pub(crate) fn projectile_velocity(&self, id: u32) -> Option<Vec3> {
        self.projectiles.get(&id).map(|state| state.0)
    }
    fn update_projectiles(&mut self, dt: f32) {
        if self.projectiles.is_empty() {
            return;
        }
        let mut bodies = std::mem::take(&mut self.projectile_scratch);
        bodies.clear();
        for (&id, &(velocity, gravity, bounce)) in &self.projectiles {
            if self.blade_cut_states.get(&id).is_some_and(|s| s.jammed) {
                continue;
            }
            if let Some(object) = self.objects.get(&id) {
                bodies.push(crate::projectile::Body {
                    id,
                    family: self
                        .cut_fragments
                        .get(&id)
                        .map_or(0, |fragment| fragment.family),
                    velocity,
                    gravity,
                    bounce,
                    position: object.transform.position,
                    radius: self.cut_fragments.get(&id).map_or(
                        object.transform.scale.abs().max_element() * 1.15,
                        |fragment| fragment.radius,
                    ),
                });
            }
        }
        let contacts = crate::projectile::simulate(
            &mut bodies,
            &mut self.projectile_order,
            &mut self.projectile_pairs,
            dt,
        );
        self.projectile_contacts = self.projectile_contacts.saturating_add(contacts);
        for body in &bodies {
            self.projectiles.get_mut(&body.id).unwrap().0 = body.velocity;
            let object = self.objects.get_mut(&body.id).unwrap();
            object.transform.position = body.position;
            object.refresh_render_data();
            self.collision_dirty |= self.colliders.contains_key(&body.id);
            self.render_revision = self.render_revision.wrapping_add(1);
            if object.render_options.is_static {
                self.static_revision = self.static_revision.wrapping_add(1);
            }
        }
        self.projectile_scratch = bodies;
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
                emission: 0.0,
                color_cutout: 0.0,
                cut_assist: 0.0,
                blade_contact_response: false,
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
    /// An ordinary scene object: app selects its transform and lifetime.
    pub fn add_info_panel(&mut self, kind: &str, width: f32, height: f32) -> Result<u32, String> {
        let panel = crate::info_panel::InfoPanel::new(
            crate::info_panel::PanelKind::parse(kind)?,
            width,
            height,
        )?;
        self.insert_info_panel(panel)
    }
    pub fn add_text_panel(
        &mut self,
        title: &str,
        lines: &[String],
        width: f32,
        height: f32,
    ) -> Result<u32, String> {
        self.insert_info_panel(crate::info_panel::InfoPanel::text(
            title, lines, width, height,
        )?)
    }
    fn insert_info_panel(&mut self, panel: crate::info_panel::InfoPanel) -> Result<u32, String> {
        let id = self.add_imported(
            Vec3::from_array(panel.mesh.bounds_min),
            Vec3::from_array(panel.mesh.bounds_max),
        );
        if id == INVALID_ID {
            return Err("Object IDs exhausted".into());
        }
        self.set_render_options(
            id,
            RenderOptions {
                casts_shadow: false,
                receives_shadow: false,
                ..Default::default()
            },
        );
        self.info_panels.insert(id, panel);
        Ok(id)
    }
    pub fn update_info_panels(
        &mut self,
        kind: crate::info_panel::PanelKind,
        rows: &[(&'static str, String)],
    ) {
        // Text never changes the backing bounds or object transform. Existing culling,
        // instance and shadow caches therefore remain valid; only mesh buffers change.
        for panel in self.info_panels.values_mut().filter(|p| p.kind == kind) {
            panel.update(rows.to_vec());
        }
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
        if !enabled {
            self.blade_cut_states.remove(&id);
        }
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
            if let Some(config) = self.blade_physics(id) {
                config.validate([a, b])?;
            }
            self.cutters.insert(id, [a, b]);
        } else {
            self.reset_blade_trail(id);
            self.cutters.remove(&id);
            self.blade_physics.remove(&id);
            self.release_blade_cuts(id, false);
        }
        self.render_revision = self.render_revision.wrapping_add(1);
        Ok(())
    }
    /// Tracked finite-ribbon coverage required to complete a cut; zero restores physical mode.
    pub fn set_cut_assist(&mut self, id: u32, coverage: f32) -> Result<(), String> {
        if !coverage.is_finite() || !(0.0..=1.0).contains(&coverage) {
            return Err("Cut assist coverage must be finite and in 0..1".into());
        }
        let object = self
            .objects
            .get_mut(&id)
            .ok_or("Cutter object does not exist")?;
        object.cut_assist = coverage;
        self.release_blade_cuts(id, false);
        Ok(())
    }
    pub(crate) fn cut_assist(&self, id: u32) -> f32 {
        self.objects.get(&id).map_or(0., |object| object.cut_assist)
    }
    /// Enable rigid projectile contacts during assisted cutting. Requires blade physics.
    pub fn set_blade_contact_response(&mut self, id: u32, enabled: bool) -> Result<(), String> {
        if enabled && !self.blade_physics.contains_key(&id) {
            return Err("Blade contact response requires configured blade physics".into());
        }
        self.objects
            .get_mut(&id)
            .ok_or("Cutter object does not exist")?
            .blade_contact_response = enabled;
        self.release_blade_cuts(id, false);
        Ok(())
    }
    pub(crate) fn blade_contact_response(&self, id: u32) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|object| object.blade_contact_response)
    }
    pub fn set_blade_physics(
        &mut self,
        id: u32,
        config: Option<crate::blade_physics::BladePhysics>,
    ) -> Result<(), String> {
        let blade = self
            .tracked_blade(id)
            .ok_or("Blade physics requires a registered cutter")?;
        if let Some(config) = config {
            config.validate(blade)?;
            self.blade_physics.insert(id, config);
        } else {
            self.blade_physics.remove(&id);
            self.release_blade_cuts(id, false);
        }
        Ok(())
    }
    pub fn blade_physics(&self, id: u32) -> Option<crate::blade_physics::BladePhysics> {
        self.blade_physics.get(&id).copied()
    }
    /// Fraction of the actual planar cap covered by the finite sharp ribbon.
    pub fn blade_cut_state(&self, id: u32) -> (f32, Option<u32>) {
        self.blade_cut_states
            .get(&id)
            .map_or((0., None), |s| (s.progress, s.jammed.then_some(s.cutter)))
    }
    pub(crate) fn release_blade_cuts(&mut self, cutter: u32, rebound: bool) {
        let released: Vec<_> = self
            .blade_cut_states
            .iter()
            .filter(|(_, s)| s.cutter == cutter)
            .map(|(&id, &s)| (id, s))
            .collect();
        for (id, state) in released {
            if rebound && !state.jammed && state.progress > 1e-4 && state.progress < 0.5 {
                if let Some(projectile) = self.projectiles.get_mut(&id) {
                    projectile.0 += state.edge_velocity.normalize_or_zero()
                        * state.edge_velocity.length().clamp(0.4, 4.)
                        * (1. + projectile.2);
                    self.projectile_contacts = self.projectile_contacts.saturating_add(1);
                }
            }
            self.blade_cut_states.remove(&id);
        }
    }
    pub(crate) fn release_unjammed_blade_cuts(&mut self, cutter: u32, rebound: bool) {
        let jammed: Vec<_> = self
            .blade_cut_states
            .iter()
            .filter(|(_, s)| s.cutter == cutter && s.jammed)
            .map(|(&id, &s)| (id, s))
            .collect();
        self.blade_cut_states
            .retain(|_, s| s.cutter != cutter || !s.jammed);
        self.release_blade_cuts(cutter, rebound);
        self.blade_cut_states.extend(jammed);
    }
    pub(crate) fn settle_shallow_blade_cuts(
        &mut self,
        cutter: u32,
        previous_targets: &BTreeMap<u32, Vec3>,
    ) -> bool {
        let moving = self.blade_cut_states.iter().any(|(&id, s)| {
            s.cutter == cutter
                && !s.jammed
                && self
                    .objects
                    .get(&id)
                    .zip(previous_targets.get(&id))
                    .is_some_and(|(object, old)| {
                        object.transform.position.distance_squared(*old) > 1e-10
                    })
        });
        if moving {
            return false;
        }
        let shallow = self
            .blade_cut_states
            .values()
            .any(|s| s.cutter == cutter && !s.jammed);
        self.release_unjammed_blade_cuts(cutter, true);
        shallow
    }
    pub(crate) fn follow_blade_jams(&mut self, cutter: u32, from: Mat4, to: Mat4, dt: f32) {
        let Some(config) = self.blade_physics(cutter) else {
            return;
        };
        let edge = to.transform_vector3(config.edge).normalize();
        let delta_pose = to * from.inverse();
        let ids: Vec<_> = self
            .blade_cut_states
            .iter()
            .filter(|(_, s)| s.cutter == cutter && s.jammed)
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            let Some(object) = self.objects.get(&id) else {
                continue;
            };
            let position = object.transform.position;
            let delta = delta_pose.transform_point3(position) - position;
            // Tangential/rotation following retains 75% relative sharp advance.
            // Accumulate net pullback: submillimetre tracking reversals must not
            // release a joint, but a sustained slow 2cm withdrawal must.
            let advance = (to.w_axis.truncate() - from.w_axis.truncate()).dot(edge);
            let state = self.blade_cut_states.get_mut(&id).unwrap();
            state.withdrawal = (state.withdrawal - advance).max(0.);
            if state.withdrawal >= 0.02 - 1e-6 {
                self.blade_cut_states.remove(&id);
                continue;
            }
            // Apply the same relative slip in both directions. Clamping reverse
            // slip would ratchet reversible tracking jitter into a completed cut.
            let translation = delta - edge * advance * 0.75;
            let rotation = glam::Quat::from_mat4(&delta_pose)
                * glam::Quat::from_euler(
                    glam::EulerRot::YXZ,
                    object.transform.rotation.y,
                    object.transform.rotation.x,
                    object.transform.rotation.z,
                );
            let (y, x, z) = rotation.to_euler(glam::EulerRot::YXZ);
            let scale = object.transform.scale;
            self.set_object_transform(id, position + translation, Vec3::new(x, y, z), scale);
            if let Some(projectile) = self.projectiles.get_mut(&id) {
                projectile.0 = translation / dt;
            }
            for removal in &mut self.removals {
                if removal.id == id
                    && !self.cut_fragments.contains_key(&id)
                    && matches!(removal.kind, RemovalKind::Object)
                {
                    removal.at = self.time + 3.;
                }
            }
        }
    }
    pub(crate) fn blade_contacts(
        &mut self,
        id: u32,
        from: glam::Mat4,
        to: glam::Mat4,
        previous_targets: &BTreeMap<u32, Vec3>,
        dt: f32,
    ) {
        let Some(config) = self.blade_physics(id) else {
            return;
        };
        let Some(blade) = self.tracked_blade(id) else {
            return;
        };
        let scale = self.objects[&id].transform.scale;
        let config = config.scaled(scale, blade);
        let scaled_blade = blade.map(|p| p * scale);
        for (&target, state) in &mut self.blade_cut_states {
            if state.cutter != id {
                continue;
            }
            let Some(object) = self.objects.get(&target) else {
                continue;
            };
            let axis = scaled_blade[1] - scaled_blade[0];
            let local = to.inverse().transform_point3(object.transform.position);
            let station =
                ((local - scaled_blade[0]).dot(axis) / axis.length_squared()).clamp(0., 1.);
            let material = scaled_blade[0] + axis * station + config.edge * config.half_width;
            let target_motion = previous_targets
                .get(&target)
                .map_or(Vec3::ZERO, |old| object.transform.position - *old);
            let velocity =
                (to.transform_point3(material) - from.transform_point3(material) - target_motion)
                    / dt;
            if velocity.length_squared() > 1e-8 {
                state.edge_velocity = velocity;
            }
        }
        let mut updates = Vec::new();
        let assisted_contacts = self.cut_assist(id) > 0. && self.blade_contact_response(id);
        for (&target, &(velocity, _, bounce)) in &self.projectiles {
            if target == id
                || self
                    .cut_fragments
                    .get(&target)
                    .is_some_and(|f| self.time < f.blade_grace_until)
                || self.blade_cut_states.get(&target).is_some_and(|s| s.jammed)
            {
                continue;
            }
            let Some(mesh) = self.triangle_meshes.get(&target) else {
                continue;
            };
            let object = &self.objects[&target];
            let (min, max) = mesh.mesh.bounds();
            let local_center = (min + max) * 0.5;
            let center = object.transform.model().transform_point3(local_center);
            // Cache the enclosing radius of actual vertices around the authored
            // bounds center. Lazy GPU-resident meshes keep the conservative
            // bounds fallback without forcing geometry readback/materialization.
            let radius = self
                .blade_body_radii
                .entry(target)
                .or_insert_with(|| {
                    let scale = object.transform.scale;
                    let radius = mesh.mesh.ready_cpu().map_or_else(
                        || ((max - min) * 0.5 * scale.abs()).length(),
                        |cpu| {
                            cpu.positions
                                .iter()
                                .map(|p| ((*p - local_center) * scale).length_squared())
                                .fold(0., f32::max)
                                .sqrt()
                        },
                    );
                    (scale, radius)
                })
                .1;
            let old = previous_targets
                .get(&target)
                .copied()
                .unwrap_or(object.transform.position);
            if old.distance(object.transform.position) > 0.75 {
                continue;
            }
            let old_center = center + old - object.transform.position;
            if let Some((resolved, velocity)) = crate::blade_physics::contact(
                config,
                blade.map(|p| p * scale),
                from,
                to,
                old_center,
                center,
                radius,
                Some((
                    glam::Quat::from_euler(
                        glam::EulerRot::YXZ,
                        object.transform.rotation.y,
                        object.transform.rotation.x,
                        object.transform.rotation.z,
                    ),
                    (max - min) * 0.5 * object.transform.scale.abs(),
                )),
                velocity,
                bounce,
                dt,
                assisted_contacts,
                self.tracked_cut_eligible(target, id),
            ) {
                let velocity = if assisted_contacts {
                    velocity.clamp_length_max(8.)
                } else {
                    velocity
                };
                updates.push((target, resolved - center, velocity));
            }
        }
        for (target, delta, velocity) in updates {
            // Keep scheduled fruit alive while held; finished fragments retain
            // their original expiry. Release restarts the existing 3s cleanup.
            if !self.cut_fragments.contains_key(&target)
                && self
                    .triangle_meshes
                    .get(&target)
                    .is_some_and(|mesh| mesh.cuttable)
            {
                for removal in &mut self.removals {
                    if removal.id == target && matches!(removal.kind, RemovalKind::Object) {
                        removal.at = self.time + 3.;
                    }
                }
            }
            self.projectiles.get_mut(&target).unwrap().0 = velocity;
            let position = self.objects[&target].transform.position + delta;
            self.set_object_position(target, position);
            self.projectile_contacts = self.projectile_contacts.saturating_add(1);
        }
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
    pub(crate) fn tracked_blade(&self, id: u32) -> Option<[Vec3; 2]> {
        self.cutters.get(&id).copied()
    }

    /// Tracked cutters are posed externally, never by the presentation swing.
    /// Fragments inherit source velocity, fall under gravity and expire after 4s.
    pub(crate) fn cut_tracked_with(
        &mut self,
        cutter: u32,
        ribbons: &BTreeMap<u32, Vec<[Vec3; 2]>>,
    ) -> Result<CutReport, String> {
        let report = self.cut_with_regions(cutter, Vec3::X, 0.0015, None, Some(ribbons))?;
        self.cutting_swings.remove(&cutter);
        for &id in &report.pieces {
            let velocity = self.projectiles.get(&id).map_or(Vec3::ZERO, |body| body.0);
            self.set_projectile(id, velocity, 9.81, 0.0);
            let expires_at = self.cut_fragments[&id].expires_at;
            self.schedule_remove_object(id, expires_at - self.time);
        }
        Ok(report)
    }
    /// Shared early gate: rejected descendants never allocate or clip a ribbon.
    pub(crate) fn tracked_cut_eligible(&self, target: u32, cutter: u32) -> bool {
        self.is_cuttable(target)
            && self.cut_fragments.get(&target).is_none_or(|fragment| {
                fragment.birth_cutter != Some(cutter)
                    && fragment.generation < MAX_CUT_GENERATION
                    && self.time < fragment.expires_at
                    && self
                        .cut_fragments
                        .values()
                        .filter(|f| f.family == fragment.family)
                        .count()
                        < MAX_FAMILY_PIECES
            })
    }

    /// A newly split child follows the parent's already sampled translation.
    /// Only an actual parent sample may seed the other controller's same-frame sweep.
    pub(crate) fn tracked_previous_position(
        &self,
        target: u32,
        previous: &BTreeMap<u32, Vec3>,
    ) -> Option<Vec3> {
        if let Some(&position) = previous.get(&target) {
            return Some(position);
        }
        let fragment = self.cut_fragments.get(&target)?;
        self.tracked_previous_position(fragment.parent, previous)
            .map(|position| position + fragment.parent_offset)
    }

    /// A birth blade must leave the body (or start a distinct stroke) before it
    /// can cut its own children. Other controllers can cut them immediately.
    pub(crate) fn release_fragment_blade(
        &mut self,
        cutter: u32,
        endpoints: [Vec3; 2],
        fresh: bool,
    ) {
        let blade_margin = self.blade_physics.get(&cutter).map_or(0.015, |config| {
            let scaled =
                config.scaled(self.objects[&cutter].transform.scale, self.cutters[&cutter]);
            scaled.half_width + scaled.half_thickness + 0.002
        });
        for (&id, fragment) in &mut self.cut_fragments {
            if fragment.birth_cutter != Some(cutter) {
                continue;
            }
            let object = &self.objects[&id];
            let inverse = object.transform.model().inverse();
            let points = endpoints.map(|p| inverse.transform_point3(p));
            let (min, max) = self.triangle_meshes[&id].mesh.bounds();
            // Conservative local AABB gate includes the blade edge/body width.
            let margin = Vec3::splat(blade_margin) / object.transform.scale.abs();
            let outside = points[0].max(points[1]).cmplt(min - margin).any()
                || points[0].min(points[1]).cmpgt(max + margin).any();
            if fresh || outside {
                fragment.birth_cutter = None;
            }
        }
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
        let materials: BTreeMap<_, _> = removed
            .iter()
            .map(|id| {
                let object = &self.objects[id];
                (*id, (object.emission, object.color_cutout))
            })
            .collect();
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
                let (emission, cutoff) = materials[&candidate.id];
                self.set_emission(id, emission);
                self.set_color_cutout(id, cutoff);
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
        self.cut_with_regions(cutter, motion, separation, geometry, None)
    }

    fn cut_with_regions(
        &mut self,
        cutter: u32,
        motion: Vec3,
        separation: f32,
        geometry: Option<&BTreeMap<u32, crate::cutting::CutGeometry>>,
        ribbons: Option<&BTreeMap<u32, Vec<[Vec3; 2]>>>,
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
        if ribbons.is_none() && normal.length_squared() < 0.5 {
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
                ribbons.is_some()
                    || ((bounds.center + bounds.half_extent).cmpge(min).all()
                        && (bounds.center - bounds.half_extent).cmple(max).all())
            },
            &|id| {
                ribbons.is_none_or(|histories| {
                    self.tracked_cut_eligible(id, cutter)
                        && histories.get(&id).is_some_and(|h| h.len() >= 2)
                })
            },
            &mut candidates,
            &mut report.bvh_nodes,
            &mut object_tests,
        );
        report.object_candidates = candidates.len();
        let mut pending = vec![];
        let mut family_delta = BTreeMap::<u64, isize>::new();
        for id in candidates {
            let source = &self.triangle_meshes[&id];
            let object = &self.objects[&id];
            let inverse = object.transform.model().inverse();
            if !inverse.is_finite() {
                return Err("Noninvertible cutting transform".into());
            }
            let (slice, normal, a) = if let Some(ribbons) = ribbons {
                let Some(history) = ribbons.get(&id) else {
                    continue;
                };
                // Translate historical blade positions with the moving target.
                // Rotation uses the current real mesh, not a frozen projectile.
                let mut history = history.as_slice();
                let (origin, normal) = loop {
                    // Fit the actual swept surface. Crossing the first and
                    // last blade axes instead chooses the plane of even tiny
                    // wrist yaw, which can be perpendicular to the stroke.
                    let normals = || {
                        history.windows(2).flat_map(|pair| {
                            let [a, b] = pair[0];
                            let [c, d] = pair[1];
                            [(b - a).cross(d - a), (d - a).cross(c - a)]
                        })
                    };
                    let Some(seed) =
                        normals().max_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
                    else {
                        break (Vec3::ZERO, Vec3::ZERO);
                    };
                    if seed.length_squared() < 1e-10 {
                        break (Vec3::ZERO, Vec3::ZERO);
                    }
                    // Midpoint rotations sweep opposite-facing triangles;
                    // align their normals before area-weighted averaging.
                    let normal = normals()
                        .map(|n| if n.dot(seed) < 0. { -n } else { n })
                        .sum::<Vec3>()
                        .normalize();
                    let origin = history.iter().flatten().copied().sum::<Vec3>()
                        / (history.len() * 2) as f32;
                    // Short nearly planar arcs allow at most 3cm of projection
                    // in world space. Discard older samples when normal target
                    // drift exceeds that bound; never enlarge the finite ribbon.
                    if history
                        .iter()
                        .flatten()
                        .all(|p| normal.dot(*p - origin).abs() <= 0.03)
                    {
                        break (origin, normal);
                    }
                    history = &history[1..];
                };
                if normal.length_squared() < 0.5 {
                    continue;
                }
                let samples: Vec<_> = history
                    .iter()
                    .map(|pair| {
                        pair.map(|p| {
                            let projected = p - normal * normal.dot(p - origin);
                            inverse.transform_point3(projected + object.transform.position)
                        })
                    })
                    .collect();
                let assist = self.cut_assist(cutter);
                let physical = assist == 0. && self.blade_physics.contains_key(&cutter);
                let (progress, slice) = if physical || assist > 0. {
                    source.mesh.cpu()?.slice_ribbon_progress(
                        &samples,
                        source.cap_color,
                        if assist > 0. { assist } else { 0.75 },
                    )?
                } else {
                    (
                        0.,
                        source
                            .mesh
                            .cpu()?
                            .slice_ribbon(&samples, source.cap_color)?,
                    )
                };
                if physical && progress > 0. {
                    let previous = self.blade_cut_states.get(&id).copied();
                    if previous.is_none_or(|s| !s.jammed || s.cutter == cutter) {
                        self.blade_cut_states.insert(
                            id,
                            BladeCutState {
                                progress,
                                cutter,
                                jammed: (progress + 2e-5 >= 0.5
                                    || previous.is_some_and(|s| s.jammed))
                                    && progress < 0.75,
                                withdrawal: previous.map_or(0., |s| s.withdrawal),
                                edge_velocity: previous.map_or(Vec3::ZERO, |s| s.edge_velocity),
                            },
                        );
                    }
                }
                let Some(slice) = slice else {
                    continue;
                };
                let origin = samples[0][0];
                let axis = samples[0][1] - origin;
                let local_normal = samples
                    .iter()
                    .flatten()
                    .map(|p| axis.cross(*p - origin))
                    .find(|n| n.length_squared() > 1e-10)
                    .unwrap()
                    .normalize();
                let normal = inverse
                    .transpose()
                    .transform_vector3(local_normal)
                    .normalize();
                (
                    Some(slice),
                    normal,
                    object.transform.model().transform_point3(origin),
                )
            } else {
                (
                    source.mesh.cpu()?.slice_prepared(
                        inverse.transform_point3(a),
                        inverse.transform_point3(b),
                        inverse.transform_vector3(motion),
                        source.cap_color,
                        geometry.and_then(|g| g.get(&id)),
                    )?,
                    normal,
                    a,
                )
            };
            if let Some(slice) = slice {
                if ribbons.is_some() {
                    let family = self
                        .cut_fragments
                        .get(&id)
                        .map_or(u64::from(id) + 1, |f| f.family);
                    let live = self
                        .cut_fragments
                        .values()
                        .filter(|f| f.family == family)
                        .count();
                    let retained = live as isize + family_delta.get(&family).copied().unwrap_or(0)
                        - isize::from(self.cut_fragments.contains_key(&id));
                    if retained + slice.pieces.len() as isize > MAX_FAMILY_PIECES as isize {
                        continue; // Keep the complete source solid; never drop excess pieces.
                    }
                    *family_delta.entry(family).or_default() += slice.pieces.len() as isize
                        - isize::from(self.cut_fragments.contains_key(&id));
                }
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
                        self.projectiles.get(&id).copied(),
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
        let mut fragment_motion = BTreeMap::new();
        if ribbons.is_some() {
            for &source in &report.removed {
                let object = &self.objects[&source];
                // The original projectile velocity belongs to its authored pivot.
                // Preserve the instantaneous rigid velocity at each new center.
                let center = object.transform.position;
                let spin = self.spins.get(&source).copied().unwrap_or(Vec3::ZERO);
                // spins stores YXZ Euler rates; convert their instantaneous axes to world space.
                let yaw = glam::Quat::from_rotation_y(object.transform.rotation.y);
                let pitch = glam::Quat::from_rotation_x(object.transform.rotation.x);
                let omega =
                    Vec3::Y * spin.y + yaw * Vec3::X * spin.x + (yaw * pitch) * Vec3::Z * spin.z;
                let edge_speed = self
                    .blade_cut_states
                    .get(&source)
                    .map_or(0., |state| state.edge_velocity.length());
                let speed = (0.08 + edge_speed * 0.02).min(0.24);
                let mut mass = 0.;
                let mut direction_sum = Vec3::ZERO;
                for piece in pending.iter().filter(|p| p.0 == source) {
                    let volume = piece.1.volume() as f32;
                    mass += volume;
                    direction_sum += piece.5 * volume;
                }
                let ancestor = self.cut_fragments.get(&source).copied();
                fragment_motion.insert(
                    source,
                    (center, spin, omega, speed, direction_sum / mass, ancestor),
                );
            }
        }
        let materials: BTreeMap<_, _> = report
            .removed
            .iter()
            .map(|id| {
                let object = &self.objects[id];
                (*id, (object.emission, object.color_cutout))
            })
            .collect();
        for &id in &report.removed {
            self.remove_object(id);
        }
        for (source, mut piece, mut transform, options, cap_color, direction, mut projectile) in
            pending
        {
            let mut fragment = None;
            let mut inherited_spin = None;
            if let Some(&(source_center, spin, omega, speed, mean_direction, ancestor)) =
                fragment_motion.get(&source)
            {
                let center = piece.recenter();
                // Rebase the pivot without changing any world vertex, apart from the tiny kerf.
                transform.position +=
                    transform.model().transform_vector3(center) - mean_direction * separation * 0.5;
                let offset = transform.position
                    - (direction - mean_direction) * separation * 0.5
                    - source_center;
                let velocity = projectile.map_or(Vec3::ZERO, |body| body.0)
                    + omega.cross(offset)
                    + (direction - mean_direction) * speed;
                projectile = Some((velocity, 9.81, 0.));
                inherited_spin = Some(spin);
                let radius = piece
                    .positions
                    .iter()
                    .map(|p| (*p * transform.scale).length())
                    .fold(0., f32::max);
                fragment = Some(CutFragment {
                    cut_speed: 0.,
                    family: ancestor.map_or(u64::from(source) + 1, |f| f.family),
                    blade_grace_until: self.time + 0.2,
                    radius,
                    generation: ancestor.map_or(1, |f| f.generation + 1),
                    expires_at: ancestor.map_or(self.time + 4., |f| f.expires_at),
                    birth_cutter: Some(cutter),
                    parent: source,
                    parent_offset: transform.position - source_center,
                });
            }
            let eligible = fragment.is_none_or(|f| {
                let (min, max) = piece.bounds();
                f.generation < MAX_CUT_GENERATION
                    && piece.volume() * f64::from(transform.scale.element_product().abs())
                        >= MIN_CUT_VOLUME
                    && ((max - min) * transform.scale.abs()).min_element() >= MIN_CUT_EXTENT
            });
            let id = self.add_triangle_mesh(piece)?;
            self.set_object_transform(id, transform.position, transform.rotation, transform.scale);
            self.set_render_options(id, options);
            let (emission, cutoff) = materials[&source];
            self.set_emission(id, emission);
            self.set_color_cutout(id, cutoff);
            self.set_cuttable(id, eligible, cap_color)?;
            if let Some((velocity, gravity, bounce)) = projectile {
                self.set_projectile(id, velocity, gravity, bounce);
            }
            if let Some(fragment) = fragment {
                self.cut_fragments.insert(id, fragment);
            }
            if let Some(spin) = inherited_spin {
                self.set_object_spin(id, spin);
            }
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
        self.feedback = cut_feedback::Feedback::default();
        self.projectiles.clear();
        self.projectile_scratch.clear();
        self.cut_fragments.clear();
        self.projectile_order.clear();
        self.projectile_pairs.clear();
        self.projectile_contacts = 0;
        self.triangle_meshes.clear();
        self.info_panels.clear();
        self.cutters.clear();
        self.blade_physics.clear();
        self.blade_cut_states.clear();
        self.blade_body_radii.clear();
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
        self.projectiles.clear();
        self.paused = false;
    }
    pub fn remove_object(&mut self, id: u32) -> bool {
        self.remove_blade_trail_binding(id);
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
        self.info_panels.remove(&id);
        self.cutters.remove(&id);
        self.blade_physics.remove(&id);
        self.release_blade_cuts(id, false);
        self.blade_cut_states.remove(&id);
        self.blade_body_radii.remove(&id);
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
        self.projectiles.remove(&id);
        self.cut_fragments.remove(&id);
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
        if self
            .objects
            .get(&id)
            .is_some_and(|o| o.transform.position.distance(position) > 0.75)
        {
            self.release_blade_cuts(id, false);
            self.blade_cut_states.remove(&id);
        }
        let transform = Transform::new(position, rotation, scale);
        if !transform.is_valid() {
            return false;
        }
        let Some(object) = self.objects.get_mut(&id) else {
            return false;
        };
        if object.transform.scale != scale {
            self.blade_body_radii.remove(&id);
        }
        object.transform = transform;
        object.refresh_render_data();
        self.cutting_swings.remove(&id);
        self.cutting_scatter.remove(&id);
        self.changed(id);
        true
    }
    pub fn set_object_position(&mut self, id: u32, position: Vec3) -> bool {
        if self
            .objects
            .get(&id)
            .is_some_and(|o| o.transform.position.distance(position) > 0.75)
        {
            self.release_blade_cuts(id, false);
            self.blade_cut_states.remove(&id);
        }
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
        if object.transform.scale != scale {
            self.blade_body_radii.remove(&id);
        }
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
        self.update_cut_feedback(dt);
        self.time += dt;
        let now = self.time;
        let pending = std::mem::take(&mut self.removals);
        for removal in pending {
            if now < removal.at
                || (!self.cut_fragments.contains_key(&removal.id)
                    && self
                        .blade_cut_states
                        .get(&removal.id)
                        .is_some_and(|s| s.jammed))
            {
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
            if self.blade_cut_states.get(id).is_some_and(|s| s.jammed) {
                continue;
            }
            if let Some(object) = self.objects.get_mut(id) {
                object.transform.rotation += *velocity * dt;
                // Active projectiles refresh once after both rotation and motion.
                if self.projectiles.contains_key(id) {
                    continue;
                }
                object.refresh_render_data();
                self.render_revision = self.render_revision.wrapping_add(1);
                if object.render_options.is_static {
                    self.static_revision = self.static_revision.wrapping_add(1);
                }
            }
        }
        self.update_projectiles(dt);
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
    fn apple() -> crate::cutting::CutMesh {
        let (segments, rings) = (20, 12);
        let mut positions = vec![Vec3::new(0., 0.2, 0.), Vec3::new(0., 2.12, 0.)];
        for j in 1..rings {
            let theta = j as f32 / rings as f32 * std::f32::consts::PI;
            for i in 0..segments {
                let phi = i as f32 / segments as f32 * std::f32::consts::TAU;
                let radius =
                    theta.sin() * 1.1 * (1. - 0.1 * theta.cos()) * (1. + 0.045 * (phi * 5.).cos());
                let y = 0.2 + 1.1 * (1. - theta.cos())
                    - 0.28 * (-((std::f32::consts::PI - theta) / 0.38).powi(2)).exp();
                positions.push(Vec3::new(phi.cos() * radius, y, phi.sin() * radius));
            }
        }
        let mut indices = vec![];
        for i in 0..segments {
            let next = (i + 1) % segments;
            indices.extend([0, 2 + i, 2 + next]);
            let top = 2 + (rings - 2) * segments;
            indices.extend([1, top + next, top + i]);
            for j in 0..rings - 2 {
                let b = 2 + j * segments + i;
                let bn = 2 + j * segments + next;
                indices.extend([b, b + segments, bn + segments, b, bn + segments, bn]);
            }
        }
        crate::cutting::CutMesh::new(positions, &indices, Vec3::X).unwrap()
    }

    #[test]
    fn xr_input_cut_fragments_keep_world_geometry_and_rigid_motion() {
        use super::*;
        let mut scene = SceneManager::default();
        let mesh = apple();
        let source_volume = mesh.volume();
        let source_centroid = mesh.centroid();
        assert!(
            source_centroid.y > 0.8,
            "fixture must have an off-origin center"
        );
        let source = scene.add_triangle_mesh(mesh).unwrap();
        let root = Vec3::new(0., 3., 0.);
        let scale = Vec3::new(0.15, 0.18, 0.12);
        scene.set_object_transform(source, root, Vec3::ZERO, scale);
        scene.set_cuttable(source, true, Vec3::ONE).unwrap();
        let velocity = Vec3::new(0.3, 0.6, -0.2);
        let spin = Vec3::new(0., 0., 0.8);
        scene.set_projectile(source, velocity, 9.81, 0.);
        scene.set_object_spin(source, spin);
        let cutter = scene.add_cube(Vec3::ZERO);
        scene
            .set_cutter(cutter, Some([Vec3::ZERO, Vec3::Z]))
            .unwrap();
        // Target-relative ribbon fully crosses an intentionally unequal split.
        let x = 0.03;
        let history = vec![
            [Vec3::new(x, 0.5, -0.4), Vec3::new(x, 0.5, 0.4)],
            [Vec3::new(x, -0.1, -0.4), Vec3::new(x, -0.1, 0.4)],
        ];
        let before: Vec<_> = scene.triangle_meshes[&source]
            .mesh
            .cpu()
            .unwrap()
            .positions
            .iter()
            .map(|&p| root + p * scale)
            .collect();
        let report = scene
            .cut_tracked_with(cutter, &BTreeMap::from([(source, history)]))
            .unwrap();
        assert_eq!(report.pieces.len(), 2);
        let mut weighted_velocity = Vec3::ZERO;
        let mut weighted_position = Vec3::ZERO;
        let mut volume = 0.;
        let mut initial = Vec::new();
        for (&id, direction) in report.pieces.iter().zip(&report.piece_directions) {
            let mesh = scene.triangle_meshes[&id].mesh.cpu().unwrap();
            mesh.validate().unwrap();
            assert!(mesh.centroid().length() < 1e-5);
            let mass = mesh.volume();
            volume += mass;
            weighted_velocity += scene.projectiles[&id].0 * mass as f32;
            weighted_position += scene.objects[&id].transform.position * mass as f32;
            assert_eq!(scene.spins[&id], spin);
            assert!(scene.triangle_meshes[&id].cuttable);
            // Every retained original vertex stays within the 1.5mm kerf.
            let model = scene.objects[&id].transform.model();
            for vertex in &before {
                if direction.dot(*vertex - (root + Vec3::X * x)) > 0.002 {
                    assert!(mesh
                        .positions
                        .iter()
                        .any(|&p| model.transform_point3(p).distance(*vertex) < 0.0016));
                }
            }
            initial.push(scene.objects[&id].transform.position);
        }
        assert!((volume - source_volume).abs() < 1e-5);
        let com = root + source_centroid * scale;
        assert!((weighted_position / volume as f32 - com).length() < 1e-5);
        let expected = velocity + spin.cross(com - root);
        assert!((weighted_velocity / volume as f32 - expected).length() < 1e-5);
        let axis = report.piece_directions[1];
        let initial_gap = (initial[1] - initial[0]).dot(axis);
        scene.update(1. / 90.);
        for (&id, position) in report.pieces.iter().zip(&initial) {
            assert!(scene.objects[&id].transform.position.distance(*position) < 0.02);
        }
        assert_eq!(scene.projectile_contacts, 0);
        for _ in 0..17 {
            scene.update(1. / 90.);
        }
        let gap = (scene.objects[&report.pieces[1]].transform.position
            - scene.objects[&report.pieces[0]].transform.position)
            .dot(axis);
        assert!(
            gap > initial_gap + 0.015 && gap < initial_gap + 0.09,
            "opening {gap} from {initial_gap}"
        );
        for &id in &report.pieces {
            let mesh = scene.triangle_meshes[&id].mesh.cpu().unwrap();
            assert!(
                scene.objects[&id]
                    .transform
                    .model()
                    .transform_point3(mesh.centroid())
                    .distance(scene.objects[&id].transform.position)
                    < 1e-5
            );
        }
        for _ in 0..390 {
            scene.update(1. / 90.);
        }
        assert!(report.pieces.iter().all(|id| !scene.object_exists(*id)));
        assert!(scene.cut_fragments.is_empty());
        scene.clear_scene();
        assert!(scene.cut_fragments.is_empty());
    }

    fn recut_ribbon(scene: &SceneManager, target: u32, normal: Vec3) -> Vec<[Vec3; 2]> {
        let mesh = scene.triangle_meshes[&target].mesh.cpu().unwrap();
        let object = &scene.objects[&target];
        let center =
            object.transform.model().transform_point3(mesh.centroid()) - object.transform.position;
        let axis = if normal.dot(Vec3::Z).abs() < 0.8 {
            Vec3::Z
        } else {
            Vec3::X
        };
        let across = normal.cross(axis);
        vec![
            [center - across - axis, center - across + axis],
            [center + across - axis, center + across + axis],
        ]
    }

    #[test]
    fn xr_input_two_swords_recut_apple_closed_halves_quarters_and_bounded_descendants() {
        let mut scene = SceneManager::default();
        let source = scene.add_triangle_mesh(apple()).unwrap();
        scene.set_object_transform(source, Vec3::Y * 2., Vec3::ZERO, Vec3::splat(0.15));
        scene.set_cuttable(source, true, Vec3::ONE).unwrap();
        scene.set_projectile(source, Vec3::new(0.3, 1., -0.2), 9.81, 0.);
        scene.set_object_spin(source, Vec3::new(0., 0.4, 0.2));
        let volume = scene.triangle_meshes[&source].mesh.volume();
        let mut swords = Vec::new();
        for _ in 0..2 {
            let sword = scene.add_cube(Vec3::ZERO);
            scene.set_cutter(sword, Some([-Vec3::Z, Vec3::Z])).unwrap();
            swords.push(sword);
        }
        let family = u64::from(source) + 1;
        let mut pieces = vec![source];
        for generation in 1..=4 {
            let cutter = swords[(generation - 1) % 2];
            let normal = [Vec3::X, Vec3::Y, Vec3::Z, Vec3::X][generation - 1];
            let histories = pieces
                .iter()
                .map(|&id| (id, recut_ribbon(&scene, id, normal)))
                .collect();
            let old_velocity: BTreeMap<_, _> = pieces
                .iter()
                .map(|&id| (id, scene.projectiles[&id].0))
                .collect();
            let old_positions: BTreeMap<_, _> = pieces
                .iter()
                .map(|&id| (id, scene.objects[&id].transform.position))
                .collect();
            let report = scene.cut_tracked_with(cutter, &histories).unwrap();
            let mut removed = report.removed.clone();
            let mut expected = pieces.clone();
            removed.sort_unstable();
            expected.sort_unstable();
            assert_eq!(removed, expected, "generation={generation}");
            assert_eq!(report.pieces.len(), pieces.len() * 2);
            // Reusing stale removed IDs, by either hand in the same frame, is harmless.
            assert!(scene
                .cut_tracked_with(swords[1], &histories)
                .unwrap()
                .pieces
                .is_empty());
            pieces = report.pieces;
            let total: f64 = pieces
                .iter()
                .map(|id| scene.triangle_meshes[id].mesh.volume())
                .sum();
            assert!((total - volume).abs() < volume * 1e-4);
            for &id in &pieces {
                let fragment = scene.cut_fragments[&id];
                let mesh = scene.triangle_meshes[&id].mesh.cpu().unwrap();
                mesh.validate().unwrap();
                assert!(mesh.centroid().length() < 1e-5);
                assert_eq!(fragment.family, family);
                assert_eq!(fragment.generation, generation as u8);
                assert_eq!(fragment.expires_at, 4.);
                assert_eq!(scene.removals.iter().find(|r| r.id == id).unwrap().at, 4.);
                assert!(!scene.tracked_cut_eligible(id, cutter));
                assert_eq!(
                    scene.tracked_cut_eligible(id, swords[generation % 2]),
                    generation < 4
                );
                let previous = scene.tracked_previous_position(id, &old_positions).unwrap();
                assert!(previous.distance(scene.objects[&id].transform.position) < 1e-5);
                assert!((scene.projectiles[&id].0 - old_velocity[&fragment.parent]).length() < 0.3);
            }
            // Holding the birth blade through every descendant cannot recursively chip it.
            let held = pieces
                .iter()
                .map(|&id| (id, recut_ribbon(&scene, id, Vec3::Z)))
                .collect();
            let report = scene.cut_tracked_with(cutter, &held).unwrap();
            assert!(report.pieces.is_empty() && report.object_candidates == 0);
            assert!(pieces.len() <= MAX_FAMILY_PIECES);
            if generation == 1 {
                scene.update(0.2); // Recutting later must not renew the original expiry.
            }
        }
        assert!(pieces.iter().all(|id| !scene.is_cuttable(*id)));
        scene.blade_cut_states.insert(
            pieces[0],
            BladeCutState {
                progress: 0.6,
                cutter: swords[0],
                jammed: true,
                withdrawal: 0.,
                edge_velocity: Vec3::ZERO,
            },
        );
        scene.update(3.7);
        assert!(pieces.iter().all(|id| scene.object_exists(*id)));
        scene.update(0.11);
        assert!(pieces.iter().all(|id| !scene.object_exists(*id)));
        assert!(scene.cut_fragments.is_empty());
        assert!(scene.removals.is_empty());
    }

    #[test]
    fn xr_input_family_budget_counts_all_pending_splits_without_dropping_solids() {
        let mut scene = SceneManager::default();
        let source = scene.add_triangle_mesh(apple()).unwrap();
        scene.set_object_scale(source, Vec3::splat(0.13));
        scene.set_cuttable(source, true, Vec3::ONE).unwrap();
        let mut swords = Vec::new();
        for _ in 0..2 {
            let sword = scene.add_cube(Vec3::ZERO);
            scene.set_cutter(sword, Some([-Vec3::Z, Vec3::Z])).unwrap();
            swords.push(sword);
        }
        let history = recut_ribbon(&scene, source, Vec3::X);
        let halves = scene
            .cut_tracked_with(swords[0], &BTreeMap::from([(source, history)]))
            .unwrap()
            .pieces;
        let metadata = scene.cut_fragments[&halves[0]];
        // Simulate a disconnected lineage near its live-piece budget. These
        // retained solids cannot be candidates, but still consume the budget.
        for _ in 0..29 {
            let id = scene.add_triangle_mesh(apple()).unwrap();
            scene.cut_fragments.insert(id, metadata);
            scene.schedule_remove_object(id, 4.);
        }
        let before: f64 = scene
            .triangle_meshes
            .values()
            .map(|mesh| mesh.mesh.volume())
            .sum();
        let histories = halves
            .iter()
            .map(|&id| (id, recut_ribbon(&scene, id, Vec3::Y)))
            .collect();
        let report = scene.cut_tracked_with(swords[1], &histories).unwrap();
        assert_eq!(report.removed.len(), 1);
        assert_eq!(report.pieces.len(), 2);
        assert_eq!(scene.cut_fragments.len(), MAX_FAMILY_PIECES);
        assert_eq!(
            halves.iter().filter(|id| scene.object_exists(**id)).count(),
            1
        );
        let after: f64 = scene
            .triangle_meshes
            .values()
            .map(|mesh| mesh.mesh.volume())
            .sum();
        assert!((after - before).abs() < before * 1e-5);
        assert!(
            scene
                .cut_tracked_with(swords[1], &histories)
                .unwrap()
                .object_candidates
                == 0
        );
        scene.update(4.01);
        assert!(scene.cut_fragments.is_empty());
    }

    #[test]
    fn xr_input_fragment_immunity_releases_only_birth_blade_and_tiny_pieces_stay_solid() {
        let mut scene = SceneManager::default();
        let source = scene.add_triangle_mesh(apple()).unwrap();
        scene.set_object_scale(source, Vec3::splat(0.006));
        scene.set_cuttable(source, true, Vec3::ONE).unwrap();
        let cutter = scene.add_cube(Vec3::ZERO);
        scene.set_cutter(cutter, Some([-Vec3::Z, Vec3::Z])).unwrap();
        let history = recut_ribbon(&scene, source, Vec3::X);
        let report = scene
            .cut_tracked_with(cutter, &BTreeMap::from([(source, history)]))
            .unwrap();
        assert_eq!(report.pieces.len(), 2);
        for &id in &report.pieces {
            assert!(!scene.is_cuttable(id));
            assert!(scene.triangle_meshes[&id].mesh.volume() > 0.);
        }
        let id = report.pieces[0];
        let center = scene.objects[&id].transform.position;
        scene.release_fragment_blade(cutter + 1, [Vec3::splat(5.); 2], true);
        assert_eq!(scene.cut_fragments[&id].birth_cutter, Some(cutter));
        scene.release_fragment_blade(cutter, [center; 2], false);
        assert_eq!(scene.cut_fragments[&id].birth_cutter, Some(cutter));
        scene.release_fragment_blade(cutter, [Vec3::splat(5.); 2], false);
        assert_eq!(scene.cut_fragments[&id].birth_cutter, None);
        scene.clear_scene();
        assert!(scene.cut_fragments.is_empty());
    }

    #[test]
    fn clearing_scene_and_parking_release_projectile_work() {
        use super::*;
        let mut scene = SceneManager::default();
        let id = scene.add_sphere(Vec3::ZERO);
        assert!(scene.set_projectile(id, Vec3::Y, 12.0, 0.0));
        assert!(scene.set_projectile(id, Vec3::ZERO, 0.0, 0.0));
        assert!(scene.projectiles.is_empty());
        assert!(scene.object_exists(id));
        scene.set_projectile(id, Vec3::Y, 12.0, 0.0);
        scene.update(1.0 / 60.0);
        scene.clear_scene();
        assert!(scene.projectiles.is_empty());
        assert!(scene.projectile_scratch.is_empty());
    }
    #[test]
    #[ignore = "manual release performance comparison"]
    fn benchmark_projectile_solver() {
        use super::*;
        for count in [8, 48, 256] {
            let mut samples = Vec::new();
            for _ in 0..5 {
                let mut scene = SceneManager::default();
                for i in 0..count {
                    let id = scene.add_sphere(Vec3::new(
                        (i % 16) as f32 * 1.3,
                        6.0 + (i / 16) as f32 * 1.1,
                        0.0,
                    ));
                    scene.set_object_scale(id, Vec3::splat(0.38));
                    scene.set_projectile(
                        id,
                        Vec3::new(if i % 2 == 0 { 0.3 } else { -0.3 }, 3.0, 0.0),
                        12.0,
                        0.5,
                    );
                }
                scene.update_projectiles(1.0 / 60.0);
                let start = std::time::Instant::now();
                for _ in 0..240 {
                    scene.update_projectiles(1.0 / 60.0);
                }
                samples.push(start.elapsed().as_secs_f64() * 1e6 / 240.0);
                std::hint::black_box(scene.projectile_contacts);
            }
            samples.sort_by(f64::total_cmp);
            eprintln!("projectiles={count} median_us_per_frame={:.3}", samples[2]);
        }
    }
    #[test]
    fn projectile_collision_stops_fast_crossing_and_separates_overlap() {
        use super::*;
        let mut scene = SceneManager::default();
        let a = scene.add_sphere(Vec3::new(-1.0, 4.0, 0.0));
        let b = scene.add_sphere(Vec3::new(1.0, 4.0, 0.0));
        scene.set_object_scale(a, Vec3::splat(0.38));
        scene.set_object_scale(b, Vec3::splat(0.38));
        scene.set_projectile(a, Vec3::X * 25.0, 0.0, 0.0);
        scene.set_projectile(b, -Vec3::X * 25.0, 0.0, 0.0);
        scene.update(0.1);
        assert!(scene.projectiles[&a].0.x < 0.0);
        assert!(scene.projectiles[&b].0.x > 0.0);
        assert!(scene.projectile_contacts > 0);
        assert!(scene.objects[&a].transform.position.x < scene.objects[&b].transform.position.x);
        assert!(
            (scene.objects[&a].transform.position - scene.objects[&b].transform.position).length()
                >= 0.874 - 1e-4
        );
        scene.set_object_position(a, Vec3::new(0.0, 4.0, 0.0));
        scene.set_object_position(b, Vec3::new(0.0, 4.0, 0.0));
        scene.set_projectile(a, Vec3::ZERO, 12.0, 0.0);
        scene.set_projectile(b, Vec3::ZERO, 12.0, 0.0);
        scene.update(1.0 / 60.0);
        assert!(
            (scene.objects[&a].transform.position - scene.objects[&b].transform.position).length()
                >= 0.874 - 1e-4
        );
    }
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

#[cfg(test)]
mod info_panel_tests {
    use super::*;
    #[test]
    fn text_panel_preserves_transform_and_static_text_during_info_updates() {
        let mut scene = SceneManager::default();
        assert!(scene.add_text_panel("TITLE", &[], f32::NAN, 1.).is_err());
        assert!(scene.objects().is_empty());
        let id = scene
            .add_text_panel("TITLE", &["LINE".into()], 1., 0.5)
            .unwrap();
        scene.set_object_position(id, Vec3::new(1., 2., -3.));
        let original = scene.info_panels[&id].mesh.indices.clone();
        scene.update_info_panels(crate::info_panel::PanelKind::Text, &[("FPS", "90".into())]);
        scene.update_info_panels(
            crate::info_panel::PanelKind::Diagnostics,
            &[("FPS", "90".into())],
        );
        assert_eq!(scene.info_panels[&id].mesh.indices, original);
        assert_eq!(
            scene.objects[&id].transform.position,
            Vec3::new(1., 2., -3.)
        );
        assert!(scene.remove_object(id));
        assert!(scene.info_panels.is_empty());
    }
    #[test]
    fn info_panel_stable_handle_bounds_cache_and_removal() {
        let mut scene = SceneManager::default();
        assert!(scene.info_panels.is_empty());
        let id = scene.add_info_panel("round", 1.2, 0.7).unwrap();
        scene.set_object_transform(id, Vec3::new(1., 2., -3.), Vec3::ZERO, Vec3::ONE);
        let bounds = scene.objects[&id].local_bounds;
        let revision = scene.render_revision();
        let static_revision = scene.static_revision();
        scene.update_info_panels(
            crate::info_panel::PanelKind::Round,
            &[("ROZKROJENO", "222".into())],
        );
        assert_eq!(scene.objects[&id].local_bounds, bounds);
        assert_eq!(
            scene.objects[&id].transform.position,
            Vec3::new(1., 2., -3.)
        );
        assert_eq!(scene.render_revision(), revision);
        assert_eq!(scene.static_revision(), static_revision);
        assert!(!scene.is_cuttable(id));
        assert!(!scene.objects[&id].render_options.casts_shadow);
        assert!(!scene.objects[&id].render_options.receives_shadow);
        assert!(scene.remove_object(id));
        assert!(scene.info_panels.is_empty());
        let next = scene.add_info_panel("scene", 1., 1.).unwrap();
        assert_ne!(next, id);
        scene.clear_scene();
        assert!(scene.info_panels.is_empty());
        assert!(!scene.object_exists(next));
    }
}
