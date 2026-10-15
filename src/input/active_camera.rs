use crate::input::camera::{Camera, CameraController};
use crate::input::character::CharacterController;
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
    character: Option<CharacterController>,
    pending_characters: Vec<CharacterController>,
    aspect: f32,
}

impl ActiveCamera {
    pub fn new(aspect: f32) -> Self {
        Self {
            free: Camera::new(aspect),
            orbit: OrbitCamera::new(aspect),
            active: CameraType::Orbit,
            character: None,
            pending_characters: Vec::new(),
            aspect,
        }
    }

    pub fn set_type(&mut self, ty: CameraType) {
        self.clear_input();
        self.detach_character();
        self.active = ty;
    }

    fn detach_character(&mut self) {
        if let Some(character) = self.character.take() {
            self.pending_characters.push(character);
        }
    }

    pub fn set_view(&mut self, position: Vec3, target: Vec3) -> bool {
        if !self.free.set_view(position, target) {
            return false;
        }
        self.set_type(CameraType::Free);
        true
    }

    pub fn set_aspect(&mut self, aspect: f32) {
        self.aspect = aspect;
        self.free.set_aspect(aspect);
        self.orbit.set_aspect(aspect);
        if let Some(character) = &mut self.character {
            character.set_aspect(aspect);
        }
    }

    /// Clips are ordered idle, walk, run. Invalid options leave the current view intact.
    pub fn follow_object(
        &mut self,
        id: u32,
        position: Vec3,
        speed: f32,
        run_speed: f32,
        model_yaw: f32,
        clips: [Option<usize>; 3],
    ) -> bool {
        let Some(character) = CharacterController::new(
            id,
            position,
            speed,
            run_speed,
            model_yaw,
            clips,
            self.aspect,
        ) else {
            return false;
        };
        self.clear_input();
        self.detach_character();
        self.character = Some(character);
        true
    }

    /// Call before reading matrix() each frame; update() alone never moves a character.
    pub fn update_scene(&mut self, scene: &mut crate::scene::SceneManager, dt: f32) {
        // Complete old ownership first, so a rebind wins even within the same frame.
        for character in self.pending_characters.drain(..) {
            character.finish(scene);
        }
        if let Some(character) = &mut self.character {
            if !character.update_scene(scene, dt) {
                self.clear_input();
                self.character = None;
            }
        }
    }

    pub fn character_state(&self) -> Option<(u32, Vec3, bool, bool)> {
        self.character.as_ref().map(CharacterController::state)
    }

    pub fn set_character_physics(
        &mut self,
        physics: crate::input::character::CharacterPhysics,
    ) -> bool {
        self.character
            .as_mut()
            .is_some_and(|character| character.set_physics(physics))
    }

    pub fn character_physics_state(&self) -> Option<(bool, f32, usize)> {
        self.character
            .as_ref()
            .map(CharacterController::physics_state)
    }

    fn active_mut(&mut self) -> &mut dyn CameraController {
        if let Some(character) = &mut self.character {
            return character;
        }
        match self.active {
            CameraType::Free => &mut self.free,
            CameraType::Orbit => &mut self.orbit,
        }
    }

    fn active_ref(&self) -> &dyn CameraController {
        if let Some(character) = &self.character {
            return character;
        }
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
        if let Some(character) = &mut self.character {
            character.clear_input();
        }
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
    fn explicit_free_view_projects_target_and_rejects_invalid_input() {
        let mut camera = ActiveCamera::new(1.0);
        let position = Vec3::new(12.0, 3.0, 0.0);
        let target = Vec3::new(-8.0, 5.0, 0.0);
        assert!(camera.set_view(position, target));
        assert_eq!(camera.position(), position);
        let clip = camera.matrix() * target.extend(1.0);
        assert!(clip.x.abs() < 1e-5 && clip.y.abs() < 1e-5 && clip.w > 0.0);
        let previous = camera.matrix();
        for bad in [position, Vec3::splat(f32::NAN), position + Vec3::Y] {
            assert!(!camera.set_view(position, bad));
            assert_eq!(camera.matrix(), previous);
        }
    }

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
