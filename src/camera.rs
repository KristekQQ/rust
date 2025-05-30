use glam::{Mat4, Vec3};
use std::collections::HashSet;

pub struct Camera {
    pub position: Vec3,
    yaw: f32,
    pitch: f32,
    pub speed: f32,
    pub sensitivity: f32,
    pressed: HashSet<String>,
    aspect: f32,
}

impl Camera {
    pub fn new(aspect: f32) -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 2.0),
            yaw: -std::f32::consts::FRAC_PI_2,
            pitch: 0.0,
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
        let forward = self.forward();
        let right = Vec3::Y.cross(forward).normalize();
        if self.pressed.contains("KeyW") {
            self.position += forward * self.speed * dt;
        }
        if self.pressed.contains("KeyS") {
            self.position -= forward * self.speed * dt;
        }
        if self.pressed.contains("KeyA") {
            self.position -= right * self.speed * dt;
        }
        if self.pressed.contains("KeyD") {
            self.position += right * self.speed * dt;
        }
    }

    pub fn matrix(&self) -> Mat4 {
        let view = Mat4::look_at_lh(self.position, self.position + self.forward(), Vec3::Y);
        let proj = Mat4::perspective_lh(std::f32::consts::FRAC_PI_4, self.aspect, 0.1, 100.0);
        proj * view
    }

    fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize()
    }
}
