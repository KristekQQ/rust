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
        let radius: f32 = 3.0;
        let yaw: f32 = 0.0;
        let pitch: f32 = 0.0;
        let target = Vec3::ZERO;
        let position = target
            + Vec3::new(radius * yaw.cos() * pitch.cos(), radius * pitch.sin(), radius * yaw.sin() * pitch.cos());
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
        // recalc position
        self.position = self.target
            + Vec3::new(
                self.radius * self.yaw.cos() * self.pitch.cos(),
                self.radius * self.pitch.sin(),
                self.radius * self.yaw.sin() * self.pitch.cos(),
            );
    }

pub fn matrix(&self) -> Mat4 {
        let view = Mat4::look_at_lh(self.position, self.target, Vec3::Y);
        let proj = Mat4::perspective_lh(std::f32::consts::FRAC_PI_4, self.aspect, 0.1, 100.0);
        proj * view
    }
}

use crate::input::camera::CameraController;

impl CameraController for OrbitCamera {
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

