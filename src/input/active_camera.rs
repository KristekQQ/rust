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
        self.clear_input();
        self.active = ty;
    }

    pub fn set_aspect(&mut self, aspect: f32) {
        self.free.set_aspect(aspect);
        self.orbit.set_aspect(aspect);
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
    fn clear_input(&mut self) {
        self.free.clear_input();
        self.orbit.clear_input();
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_camera_releases_held_keys() {
        let mut camera = ActiveCamera::new(16.0 / 9.0);
        camera.set_type(CameraType::Free);
        camera.key_down("KeyW".into());
        camera.update(0.5);
        let position = camera.position();
        camera.set_type(CameraType::Orbit);
        camera.key_up("KeyW".into());
        camera.set_type(CameraType::Free);
        camera.update(0.5);
        assert_eq!(camera.position(), position);
    }

    #[test]
    fn clearing_input_stops_movement_in_both_modes() {
        for mode in [CameraType::Free, CameraType::Orbit] {
            let mut camera = ActiveCamera::new(16.0 / 9.0);
            camera.set_type(mode);
            camera.key_down("KeyW".into());
            let before = camera.position();
            camera.update(0.25);
            assert_ne!(camera.position(), before);
            camera.clear_input();
            let stopped = camera.position();
            camera.update(1.0);
            assert_eq!(camera.position(), stopped);
            assert!(camera.matrix().is_finite());
        }
    }

    #[test]
    fn camera_projects_target_in_webgpu_depth_range() {
        let camera = ActiveCamera::new(16.0 / 9.0);
        let target = camera.matrix().project_point3(Vec3::ZERO);
        assert!(target.x.abs() < 1e-5 && target.y.abs() < 1e-5);
        assert!((0.0..1.0).contains(&target.z));
    }
}
