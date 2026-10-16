use glam::{Mat4, Vec3};
use std::collections::HashSet;

pub struct OrbitCamera {
    pub position: Vec3,
    target: Vec3,
    radius: f32,
    yaw: f32,
    pitch: f32,
    pub speed: f32,
    pub sensitivity: f32,
    pressed: HashSet<String>,
    aspect: f32,
}

impl OrbitCamera {
    pub fn new(aspect: f32) -> Self {
        let radius: f32 = 8.0;
        let yaw: f32 = std::f32::consts::FRAC_PI_4;
        let pitch: f32 = 0.35;
        let target = Vec3::ZERO;
        let position = target
            + Vec3::new(
                radius * yaw.cos() * pitch.cos(),
                radius * pitch.sin(),
                radius * yaw.sin() * pitch.cos(),
            );
        Self {
            position,
            target,
            radius,
            yaw,
            pitch,
            speed: 2.0,
            sensitivity: 0.002,
            pressed: HashSet::new(),
            aspect,
        }
    }

    pub fn set_aspect(&mut self, aspect: f32) {
        self.aspect = aspect;
    }

    /// Seed an orbit view from a camera location and its look target.
    pub fn set_view(&mut self, position: Vec3, target: Vec3) -> bool {
        let offset = position - target;
        let radius = offset.length();
        if !position.is_finite()
            || !target.is_finite()
            || !offset.is_finite()
            || !radius.is_finite()
            || radius < 1e-4
        {
            return false;
        }
        let direction = offset / radius;
        if direction.x * direction.x + direction.z * direction.z < 1e-4 {
            return false;
        }
        let yaw = direction.z.atan2(direction.x);
        let pitch = direction.y.asin();
        self.target = target;
        self.radius = radius;
        self.yaw = yaw;
        self.pitch = pitch;
        self.position = self.orbit_position();
        self.pressed.clear();
        true
    }

    pub fn target(&self) -> Vec3 {
        self.target
    }

    pub fn adjust(&mut self, dx: f32, dy: f32, zoom: f32) -> bool {
        if !dx.is_finite() || !dy.is_finite() || !zoom.is_finite() {
            return false;
        }
        let yaw = self.yaw - dx * 0.005;
        let pitch = (self.pitch + dy * 0.005).clamp(0.08, 1.45);
        let radius = (self.radius * (zoom * 0.001).exp()).clamp(2.0, 45.0);
        if !yaw.is_finite() || !radius.is_finite() {
            return false;
        }
        self.yaw = yaw;
        self.pitch = pitch;
        self.radius = radius;
        self.position = self.orbit_position();
        true
    }

    fn orbit_position(&self) -> Vec3 {
        self.target
            + Vec3::new(
                self.radius * self.yaw.cos() * self.pitch.cos(),
                self.radius * self.pitch.sin(),
                self.radius * self.yaw.sin() * self.pitch.cos(),
            )
    }

    pub fn key_down(&mut self, code: String) {
        self.pressed.insert(code);
    }

    pub fn key_up(&mut self, code: String) {
        self.pressed.remove(&code);
    }

    pub fn mouse_move(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * self.sensitivity;
        self.pitch = (self.pitch + dy * self.sensitivity).clamp(-1.54, 1.54);
    }

    pub fn update(&mut self, dt: f32) {
        // Directions relative to the current camera orientation
        let forward = (self.target - self.position).normalize();
        let right = Vec3::Y.cross(forward).normalize();

        if self.pressed.contains("KeyW") {
            self.target += forward * self.speed * dt;
        }
        if self.pressed.contains("KeyS") {
            self.target -= forward * self.speed * dt;
        }
        if self.pressed.contains("KeyA") {
            self.target -= right * self.speed * dt;
        }
        if self.pressed.contains("KeyD") {
            self.target += right * self.speed * dt;
        }
        if self.pressed.contains("Equal") || self.pressed.contains("NumpadAdd") {
            self.radius = (self.radius - self.speed * dt).max(0.5);
        }
        if self.pressed.contains("Minus") || self.pressed.contains("NumpadSubtract") {
            self.radius += self.speed * dt;
        }
        if self.pressed.contains("KeyQ") {
            self.pitch = (self.pitch - self.speed * dt * 0.25).clamp(0.08, 1.45);
        }
        if self.pressed.contains("KeyE") {
            self.pitch = (self.pitch + self.speed * dt * 0.25).clamp(0.08, 1.45);
        }
        // recalc position
        self.position = self.target
            + Vec3::new(
                self.radius * self.yaw.cos() * self.pitch.cos(),
                self.radius * self.pitch.sin(),
                self.radius * self.yaw.sin() * self.pitch.cos(),
            );
    }

    pub fn matrix(&self) -> Mat4 {
        let view = glam::camera::lh::view::look_at_mat4(self.position, self.target, Vec3::Y);
        let proj = glam::camera::lh::proj::directx::perspective(
            std::f32::consts::FRAC_PI_4,
            self.aspect,
            0.1,
            100.0,
        );
        proj * view
    }
}

use crate::input::camera::CameraController;
impl CameraController for OrbitCamera {
    fn clear_input(&mut self) {
        self.pressed.clear();
    }
    fn key_down(&mut self, code: String) {
        OrbitCamera::key_down(self, code);
    }
    fn key_up(&mut self, code: String) {
        OrbitCamera::key_up(self, code);
    }
    fn mouse_move(&mut self, dx: f32, dy: f32) {
        OrbitCamera::mouse_move(self, dx, dy);
    }
    fn update(&mut self, dt: f32) {
        OrbitCamera::update(self, dt);
    }
    fn matrix(&self) -> Mat4 {
        OrbitCamera::matrix(self)
    }
    fn position(&self) -> Vec3 {
        self.position
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_matches_nonvertical_view_and_rejects_invalid_atomically() {
        let mut camera = OrbitCamera::new(1.0);
        let position = Vec3::new(12.0, 3.0, 7.0);
        let target = Vec3::new(-2.0, -1.0, 1.0);
        assert!(camera.set_view(position, target));
        assert!(camera.position.distance(position) < 1e-5);
        assert_eq!(camera.target(), target);
        let before = (camera.position, camera.target());
        for (p, t) in [
            (position, position),
            (Vec3::splat(f32::NAN), target),
            (Vec3::ZERO, Vec3::Y),
        ] {
            assert!(!camera.set_view(p, t));
            assert_eq!((camera.position, camera.target()), before);
        }
        assert!(!camera.adjust(0.0, f32::INFINITY, 0.0));
        assert_eq!((camera.position, camera.target()), before);
    }

    #[test]
    fn adjustment_rotates_and_clamps_pitch_and_zoom() {
        let mut camera = OrbitCamera::new(1.0);
        let target = camera.target();
        let before = camera.position;
        assert!(camera.adjust(100.0, -10_000.0, -100_000.0));
        assert!((camera.position.distance(target) - 2.0).abs() < 1e-5);
        assert_ne!(camera.position, before);
        assert!(((camera.position.y - target.y) / 2.0 - 0.08_f32.sin()).abs() < 1e-5);
        assert!(camera.adjust(0.0, 10_000.0, 100_000.0));
        assert!((camera.position.distance(target) - 45.0).abs() < 1e-4);
        assert!(((camera.position.y - target.y) / 45.0 - 1.45_f32.sin()).abs() < 1e-5);
    }
}
