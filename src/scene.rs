//! CPU scene state and simulation. No browser, JavaScript or GPU dependencies.
use glam::{Mat4, Vec3};
use std::collections::BTreeMap;

pub const MAX_LIGHTS: usize = 4;
pub const INVALID_ID: u32 = u32::MAX;

/// World-space normals must remain perpendicular to transformed tangents.
pub(crate) fn normal_matrix(model: Mat4) -> Mat4 {
    model.inverse().transpose()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeshKind {
    Cube,
    Plane,
    Sphere,
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

pub struct SceneObject {
    pub transform: Transform,
    pub mesh: MeshKind,
    pub(crate) render_data: crate::visibility::ObjectRenderData,
}
impl SceneObject {
    fn refresh_render_data(&mut self) {
        self.render_data = crate::visibility::ObjectRenderData::new(self.mesh, self.transform);
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

#[derive(Default)]
pub struct SceneManager {
    objects: BTreeMap<u32, SceneObject>,
    lights: BTreeMap<u32, Light>,
    light_orbits: BTreeMap<u32, LightOrbit>,
    next_id: u32,
    render_revision: u64,
    time: f32,
    rotations: Vec<RotationAction>,
    removals: Vec<ScheduledRemoval>,
}

impl SceneManager {
    /// Fixed spatial demo for flying through and inspecting visibility.
    /// Creation and all transforms remain in Rust, not the JS test page.
    pub fn load_culling_demo(&mut self) {
        self.clear();
        for x in 0..20 {
            for y in 0..25 {
                for z in 0..20 {
                    let position = Vec3::new(
                        (x as f32 - 9.5) * 3.0,
                        (y as f32 - 12.0) * 1.8,
                        (z as f32 - 9.5) * 3.0,
                    );
                    self.add_object(
                        MeshKind::Cube,
                        Transform::new(position, Vec3::ZERO, Vec3::splat(0.65)),
                    );
                }
            }
        }
        for position in [
            Vec3::new(-20.0, 20.0, -20.0),
            Vec3::new(20.0, 20.0, 20.0),
            Vec3::new(-20.0, -20.0, 20.0),
            Vec3::new(20.0, -20.0, -20.0),
        ] {
            self.add_light(position, Vec3::splat(0.35));
        }
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
                render_data: crate::visibility::ObjectRenderData::new(mesh, transform),
            },
        );
        self.render_revision = self.render_revision.wrapping_add(1);
        id
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
        self.objects.clear();
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
    }
    pub fn remove_object(&mut self, id: u32) -> bool {
        if self.objects.remove(&id).is_none() {
            return false;
        }
        self.render_revision = self.render_revision.wrapping_add(1);
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
        self.render_revision = self.render_revision.wrapping_add(1);
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
        self.render_revision = self.render_revision.wrapping_add(1);
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
        self.render_revision = self.render_revision.wrapping_add(1);
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
        self.render_revision = self.render_revision.wrapping_add(1);
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
        if !dt.is_finite() || dt < 0.0 {
            return;
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
            elapsed < rotation.duration
        });
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
    pub fn load_example(&mut self) {
        self.clear();
        self.add_object(
            MeshKind::Plane,
            Transform::new(
                Vec3::new(0.0, -0.6, 0.0),
                Vec3::ZERO,
                Vec3::new(8.0, 1.0, 8.0),
            ),
        );
        self.add_cube(Vec3::ZERO);
        self.add_object(
            MeshKind::Sphere,
            Transform::new(Vec3::new(1.8, 0.4, 0.2), Vec3::ZERO, Vec3::splat(0.6)),
        );
        self.add_light(Vec3::new(2.5, 3.0, 2.5), Vec3::ONE);
        self.add_light(Vec3::new(-2.0, 1.5, -1.5), Vec3::new(0.2, 0.6, 1.0));
    }
}

#[cfg(test)]
mod tests {
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
}
