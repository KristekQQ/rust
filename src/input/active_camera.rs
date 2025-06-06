use crate::input::camera::{Camera, CameraController};
use crate::input::orbit_camera::OrbitCamera;
use glam::{Mat4, Vec3};

#[derive(Copy, Clone)]
pub enum CameraType {
    Free,
    Orbit,
}

pub struct ActiveCamera {
    free: Camera,
    orbit: OrbitCamera,
    active: CameraType,
}

impl ActiveCamera {
    pub fn new(aspect: f32) -> Self {
        Self {
            free: Camera::new(aspect),
            orbit: OrbitCamera::new(aspect),
            active: CameraType::Orbit,
        }
    }

    pub fn set_type(&mut self, ty: CameraType) {
        self.active = ty;
    }

    fn active_mut(&mut self) -> &mut dyn CameraController {
        match self.active {
            CameraType::Free => &mut self.free,
            CameraType::Orbit => &mut self.orbit,
        }
    }

    fn active_ref(&self) -> &dyn CameraController {
        match self.active {
            CameraType::Free => &self.free,
            CameraType::Orbit => &self.orbit,
        }
    }
}

impl CameraController for ActiveCamera {
    fn key_down(&mut self, code: String) {
        self.active_mut().key_down(code);
    }

    fn key_up(&mut self, code: String) {
        self.active_mut().key_up(code);
    }

    fn mouse_move(&mut self, dx: f32, dy: f32) {
        self.active_mut().mouse_move(dx, dy);
    }

    fn update(&mut self, dt: f32) {
        self.active_mut().update(dt);
    }

    fn matrix(&self) -> Mat4 {
        self.active_ref().matrix()
    }

    fn position(&self) -> Vec3 {
        self.active_ref().position()
    }
}
