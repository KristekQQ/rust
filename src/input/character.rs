//! Native third-person movement and chase camera, independent of browser listeners.
use super::camera::CameraController;
use crate::scene::SceneManager;
use glam::{Mat4, Vec3};
use std::collections::HashSet;

const DISTANCE: f32 = 7.0;
const TARGET_HEIGHT: f32 = 1.2;
const MAX_DT: f32 = 0.1;
const SENSITIVITY: f32 = 0.002;
const PHYSICS_STEP: f32 = 1.0 / 120.0;

#[derive(Clone, Copy)]
pub struct CharacterPhysics {
    pub half_extents: Vec3,
    pub gravity: f32,
    pub jump_speed: f32,
}
impl CharacterPhysics {
    fn valid(self) -> bool {
        self.half_extents.is_finite()
            && self.half_extents.min_element() > 0.0
            && self.gravity.is_finite()
            && self.gravity > 0.0
            && self.jump_speed.is_finite()
            && self.jump_speed > 0.0
    }
}

pub struct CharacterController {
    id: u32,
    root: Vec3,
    speed: f32,
    run_speed: f32,
    model_yaw: f32,
    clips: [Option<usize>; 3],
    animation_state: Option<usize>,
    owned_playback: Option<(usize, u64)>,
    yaw: f32,
    pitch: f32,
    aspect: f32,
    pressed: HashSet<String>,
    moving: bool,
    running: bool,
    physics: Option<CharacterPhysics>,
    vertical_velocity: f32,
    grounded: bool,
    jump_requested: bool,
    collision_candidates: usize,
}

impl CharacterController {
    pub fn new(
        id: u32,
        position: Vec3,
        speed: f32,
        run_speed: f32,
        model_yaw: f32,
        clips: [Option<usize>; 3],
        aspect: f32,
    ) -> Option<Self> {
        if !position.is_finite()
            || !speed.is_finite()
            || speed <= 0.0
            || !run_speed.is_finite()
            || run_speed <= 0.0
            || !model_yaw.is_finite()
        {
            return None;
        }
        Some(Self {
            id,
            root: position,
            speed,
            run_speed,
            model_yaw,
            clips,
            animation_state: None,
            owned_playback: None,
            yaw: 0.0,
            pitch: 0.35,
            aspect,
            pressed: HashSet::new(),
            moving: false,
            running: false,
            physics: None,
            vertical_velocity: 0.0,
            grounded: false,
            jump_requested: false,
            collision_candidates: 0,
        })
    }

    pub fn set_aspect(&mut self, aspect: f32) {
        self.aspect = aspect;
    }

    pub fn state(&self) -> (u32, Vec3, bool, bool) {
        (self.id, self.root, self.moving, self.running)
    }

    pub fn set_physics(&mut self, physics: CharacterPhysics) -> bool {
        if !physics.valid() {
            return false;
        }
        self.physics = Some(physics);
        self.vertical_velocity = 0.0;
        self.grounded = false;
        self.jump_requested = false;
        true
    }

    pub fn physics_state(&self) -> (bool, f32, usize) {
        (
            self.grounded,
            self.vertical_velocity,
            self.collision_candidates,
        )
    }

    /// False means the target was removed; the owner must detach and clear all inputs.
    pub fn update_scene(&mut self, scene: &mut SceneManager, dt: f32) -> bool {
        let Some(object) = scene.objects().get(&self.id) else {
            self.clear_input();
            return false;
        };
        let transform = object.transform;
        if transform.position != self.root {
            // Teleports are commands from the application, not accumulated physics.
            self.vertical_velocity = 0.0;
            self.grounded = false;
            self.jump_requested = false;
        }
        // Scene commands remain authoritative, including teleports while paused.
        self.root = transform.position;
        if scene.paused {
            self.clear_input();
            // Pause suppresses movement and animation mutations, but still follows the root.
            return true;
        }
        if !dt.is_finite() || dt < 0.0 {
            return true;
        }
        let dt = dt.min(MAX_DT);
        let axis = |positive: &str, negative: &str| {
            i32::from(self.pressed.contains(positive)) as f32
                - i32::from(self.pressed.contains(negative)) as f32
        };
        let forward = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos());
        let right = Vec3::Y.cross(forward);
        let direction =
            (forward * axis("KeyW", "KeyS") + right * axis("KeyD", "KeyA")).normalize_or_zero();
        self.moving = direction != Vec3::ZERO && dt > 0.0;
        self.running = self.moving
            && (self.pressed.contains("ShiftLeft") || self.pressed.contains("ShiftRight"));
        let mut rotation = transform.rotation;
        let mut position = self.root;
        if self.moving {
            let speed = if self.running {
                self.run_speed
            } else {
                self.speed
            };
            if self.physics.is_none() {
                position += direction * speed * dt;
            }
            rotation.y = direction.x.atan2(direction.z) + self.model_yaw;
        }
        if let Some(physics) = self.physics {
            let world = scene.collision_world();
            let support =
                world.move_character(position, physics.half_extents, Vec3::ZERO, Some(self.id));
            self.grounded = support.grounded;
            self.collision_candidates = support.candidates;
            if self.jump_requested && self.grounded && dt > 0.0 {
                self.vertical_velocity = physics.jump_speed;
                self.grounded = false;
            }
            if dt > 0.0 {
                self.jump_requested = false;
            }
            let speed = if self.running {
                self.run_speed
            } else {
                self.speed
            };
            let steps = (dt / PHYSICS_STEP).ceil() as usize;
            let step = if steps > 0 { dt / steps as f32 } else { 0.0 };
            for _ in 0..steps {
                self.vertical_velocity -= physics.gravity * step;
                let delta = direction * speed * step + Vec3::Y * self.vertical_velocity * step;
                let motion =
                    world.move_character(position, physics.half_extents, delta, Some(self.id));
                position = motion.position;
                self.grounded = motion.grounded;
                self.collision_candidates = self.collision_candidates.max(motion.candidates);
                if (motion.grounded && self.vertical_velocity < 0.0)
                    || (motion.hit_ceiling && self.vertical_velocity > 0.0)
                {
                    self.vertical_velocity = 0.0;
                }
            }
            self.moving = (position - self.root).with_y(0.0).length_squared() > 1e-10;
            self.running &= self.moving;
        }
        // One mutation for position + facing; keep the authored scale and X/Z rotation.
        if (position != transform.position || rotation != transform.rotation)
            && !scene.set_object_transform(self.id, position, rotation, transform.scale)
        {
            self.clear_input();
            return true;
        }
        self.root = position;
        let state = if self.running {
            2
        } else if self.moving {
            1
        } else {
            0
        };
        if self.animation_state != Some(state) {
            if let Some(clip) = self.clips[state] {
                // Optional clips may be absent/invalid; never retry/restart every frame.
                if scene.play_model_animation(self.id, clip, true, 1.0).is_ok() {
                    self.owned_playback = scene
                        .model(self.id)
                        .map(|model| (clip, model.playback_revision));
                }
            } else if self.owns_playback(scene) {
                let _ = scene.stop_model_animation(self.id);
                self.owned_playback = None;
            }
            self.animation_state = Some(state);
        }
        true
    }

    fn owns_playback(&self, scene: &SceneManager) -> bool {
        self.owned_playback.is_some_and(|(clip, revision)| {
            scene.model(self.id).is_some_and(|model| {
                model.playback_revision == revision
                    && model
                        .playback
                        .as_ref()
                        .is_some_and(|playback| playback.clip == clip)
            })
        })
    }

    /// Consumed once after detaching. Missing/replaced playback is left alone.
    /// While paused, freeze owned playback without changing the pose or scene revisions.
    /// Otherwise select idle, or stop if idle is missing/invalid.
    pub fn finish(self, scene: &mut SceneManager) {
        if !self.owns_playback(scene) {
            return;
        }
        if scene.paused {
            let _ = scene.pause_model_animation(self.id, true);
        } else if self.owned_playback.map(|(clip, _)| clip) != self.clips[0] {
            if let Some(idle) = self.clips[0] {
                if scene.play_model_animation(self.id, idle, true, 1.0).is_ok() {
                    return;
                }
            }
            let _ = scene.stop_model_animation(self.id);
        }
    }

    fn target(&self) -> Vec3 {
        self.root + Vec3::Y * TARGET_HEIGHT
    }
}

impl CameraController for CharacterController {
    fn key_down(&mut self, code: String) {
        if code == "Space" && self.physics.is_some() {
            if self.pressed.insert(code) {
                self.jump_requested = true;
            }
            return;
        }
        if matches!(
            code.as_str(),
            "KeyW" | "KeyA" | "KeyS" | "KeyD" | "ShiftLeft" | "ShiftRight"
        ) {
            self.pressed.insert(code);
        }
    }

    fn key_up(&mut self, code: String) {
        self.pressed.remove(&code);
    }

    fn mouse_move(&mut self, dx: f32, dy: f32) {
        if dx.is_finite() && dy.is_finite() {
            self.yaw = (self.yaw + dx * SENSITIVITY).rem_euclid(std::f32::consts::TAU);
            self.pitch = (self.pitch + dy * SENSITIVITY).clamp(0.08, 1.2);
        }
    }

    fn clear_input(&mut self) {
        self.pressed.clear();
        self.moving = false;
        self.running = false;
        self.jump_requested = false;
    }

    fn update(&mut self, _dt: f32) {
        // Movement requires SceneManager and is deliberately handled by update_scene only.
    }

    fn matrix(&self) -> Mat4 {
        let view = glam::camera::lh::view::look_at_mat4(self.position(), self.target(), Vec3::Y);
        let proj = glam::camera::lh::proj::directx::perspective(
            std::f32::consts::FRAC_PI_4,
            self.aspect,
            0.1,
            100.0,
        );
        proj * view
    }

    fn position(&self) -> Vec3 {
        self.target()
            + DISTANCE
                * Vec3::new(
                    -self.yaw.sin() * self.pitch.cos(),
                    self.pitch.sin(),
                    -self.yaw.cos() * self.pitch.cos(),
                )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::active_camera::{ActiveCamera, CameraType};

    fn setup() -> (SceneManager, ActiveCamera, u32) {
        let mut scene = SceneManager::default();
        let id = scene.add_cube(Vec3::ZERO);
        let mut camera = ActiveCamera::new(1.0);
        assert!(camera.follow_object(id, Vec3::ZERO, 2.0, 6.0, 0.0, [None; 3]));
        (scene, camera, id)
    }

    #[test]
    fn forward_is_positive_z_and_diagonals_have_equal_speed() {
        let (mut scene, mut camera, id) = setup();
        assert!(camera.position().z < 0.0);
        camera.key_down("KeyW".into());
        camera.update_scene(&mut scene, 0.1);
        let straight = scene.objects()[&id].transform.position;
        assert!((straight - Vec3::Z * 0.2).length() < 1e-6);
        let (mut scene, mut camera, id) = setup();
        camera.key_down("KeyW".into());
        camera.key_down("KeyD".into());
        camera.update_scene(&mut scene, 0.1);
        let diagonal = scene.objects()[&id].transform.position;
        assert!((diagonal.length() - straight.length()).abs() < 1e-6);
        assert!((diagonal.x - diagonal.z).abs() < 1e-6 && diagonal.x > 0.0);
        assert!(
            (scene.objects()[&id].transform.rotation.y - std::f32::consts::FRAC_PI_4).abs() < 1e-6
        );
    }

    #[test]
    fn opposing_keys_cancel_and_shift_only_runs_when_moving() {
        let (mut scene, mut camera, id) = setup();
        camera.key_down("ShiftRight".into());
        camera.update_scene(&mut scene, 0.1);
        assert_eq!(
            camera.character_state(),
            Some((id, Vec3::ZERO, false, false))
        );
        camera.key_down("KeyW".into());
        camera.key_down("KeyS".into());
        camera.update_scene(&mut scene, 0.1);
        assert_eq!(scene.objects()[&id].transform.position, Vec3::ZERO);
        camera.key_up("KeyS".into());
        camera.update_scene(&mut scene, 0.1);
        assert!((scene.objects()[&id].transform.position.z - 0.6).abs() < 1e-6);
        assert!(camera.character_state().unwrap().3);
        camera.key_up("ShiftRight".into());
        camera.update_scene(&mut scene, 0.1);
        assert!((scene.objects()[&id].transform.position.z - 0.8).abs() < 1e-6);
    }

    #[test]
    fn blur_clear_stops_motion_and_update_alone_never_moves_root() {
        let (mut scene, mut camera, id) = setup();
        camera.key_down("KeyW".into());
        camera.update(0.1);
        assert_eq!(camera.character_state().unwrap().1, Vec3::ZERO);
        camera.update_scene(&mut scene, 0.1);
        let position = scene.objects()[&id].transform.position;
        camera.clear_input();
        let revision = scene.render_revision();
        camera.update_scene(&mut scene, 0.1);
        assert_eq!(camera.character_state(), Some((id, position, false, false)));
        assert_eq!(scene.render_revision(), revision);
    }

    #[test]
    fn mouse_changes_camera_relative_movement_and_limits_pitch() {
        let (mut scene, mut camera, id) = setup();
        camera.mouse_move(std::f32::consts::FRAC_PI_2 / SENSITIVITY, 0.0);
        assert!(camera.position().x < 0.0);
        camera.key_down("KeyW".into());
        camera.update_scene(&mut scene, 0.1);
        assert!((scene.objects()[&id].transform.position - Vec3::X * 0.2).length() < 1e-6);
        for dy in [1e6, -1e6] {
            camera.mouse_move(0.0, dy);
            assert!(camera.matrix().is_finite());
            let root = camera.character_state().unwrap().1;
            let target = root + Vec3::Y * TARGET_HEIGHT;
            assert!(camera.position().y > target.y);
            let clip = camera.matrix() * target.extend(1.0);
            assert!(clip.x.abs() < 1e-5 && clip.y.abs() < 1e-5 && clip.w > 0.0);
        }
        let before = camera.matrix();
        camera.mouse_move(f32::NAN, f32::INFINITY);
        assert_eq!(camera.matrix(), before);
    }

    #[test]
    fn pause_clears_keys_and_does_not_mutate_scene() {
        let (mut scene, mut camera, id) = setup();
        camera.key_down("KeyW".into());
        camera.update_scene(&mut scene, 0.1);
        let position = scene.objects()[&id].transform.position;
        scene.paused = true;
        let revision = scene.render_revision();
        camera.update_scene(&mut scene, 0.1);
        camera.key_down("KeyW".into());
        camera.update_scene(&mut scene, 0.1);
        assert_eq!(scene.render_revision(), revision);
        assert_eq!(camera.character_state(), Some((id, position, false, false)));
        let teleported = Vec3::new(5.0, 2.0, -4.0);
        assert!(scene.set_object_position(id, teleported));
        let revision = scene.render_revision();
        camera.update_scene(&mut scene, 0.1);
        assert_eq!(
            camera.character_state(),
            Some((id, teleported, false, false))
        );
        assert_eq!(scene.render_revision(), revision);
        scene.paused = false;
        camera.update_scene(&mut scene, 0.1);
        assert_eq!(scene.objects()[&id].transform.position, teleported);
    }

    #[test]
    fn removal_or_clear_detaches_without_transferring_held_keys() {
        for clear in [false, true] {
            let (mut scene, mut camera, id) = setup();
            camera.key_down("KeyW".into());
            if clear {
                scene.clear();
            } else {
                assert!(scene.remove_object(id));
            }
            camera.update_scene(&mut scene, 0.1);
            assert!(camera.character_state().is_none());
            let before = camera.position();
            camera.update(0.1);
            assert_eq!(camera.position(), before);
            let replacement = scene.add_cube(Vec3::ZERO);
            assert!(camera.follow_object(replacement, Vec3::ZERO, 2.0, 6.0, 0.0, [None; 3]));
            camera.update_scene(&mut scene, 0.1);
            assert_eq!(scene.objects()[&replacement].transform.position, Vec3::ZERO);
        }
    }

    #[test]
    fn scene_position_is_authoritative_and_movement_preserves_scale() {
        let (mut scene, mut camera, id) = setup();
        let rotation = Vec3::new(0.2, 0.0, -0.3);
        let scale = Vec3::new(0.01, 0.02, 0.03);
        let root = Vec3::new(4.0, 0.5, 3.0);
        assert!(scene.set_object_transform(id, root, rotation, scale));
        // An outdated initial camera position must never reposition the scene object.
        assert!(camera.follow_object(id, Vec3::ZERO, 2.0, 6.0, 0.4, [None; 3]));
        let revision = scene.render_revision();
        camera.update_scene(&mut scene, 0.1);
        assert_eq!(scene.objects()[&id].transform.position, root);
        assert_eq!(camera.character_state().unwrap().1, root);
        assert_eq!(scene.render_revision(), revision);
        camera.update_scene(&mut scene, 0.1);
        assert_eq!(scene.render_revision(), revision);
        camera.key_down("KeyD".into());
        camera.update_scene(&mut scene, 0.1);
        let transform = scene.objects()[&id].transform;
        assert_eq!(transform.scale, scale);
        assert_eq!(transform.rotation.x, rotation.x);
        assert_eq!(transform.rotation.z, rotation.z);
        assert!((transform.rotation.y - std::f32::consts::FRAC_PI_2 - 0.4).abs() < 1e-6);
        camera.clear_input();
        let external = Vec3::new(1.0, 2.0, 3.0);
        assert!(scene.set_object_position(id, external));
        camera.update_scene(&mut scene, 0.1);
        assert_eq!(camera.character_state().unwrap().1, external);
    }

    #[test]
    fn suspended_tab_dt_is_clamped_and_invalid_dt_is_ignored() {
        let (mut scene, mut camera, id) = setup();
        camera.key_down("KeyW".into());
        camera.update_scene(&mut scene, 60.0);
        assert!((scene.objects()[&id].transform.position.z - 0.2).abs() < 1e-6);
        let revision = scene.render_revision();
        for dt in [f32::NAN, f32::INFINITY, -1.0] {
            camera.update_scene(&mut scene, dt);
        }
        assert_eq!(scene.render_revision(), revision);
    }

    #[test]
    fn invalid_options_preserve_controller_and_explicit_views_detach() {
        let (mut scene, mut camera, id) = setup();
        let before = camera.matrix();
        assert!(!camera.follow_object(id, Vec3::splat(f32::NAN), 2.0, 6.0, 0.0, [None; 3]));
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(!camera.follow_object(id, Vec3::ZERO, bad, 6.0, 0.0, [None; 3]));
            assert!(!camera.follow_object(id, Vec3::ZERO, 2.0, bad, 0.0, [None; 3]));
        }
        assert!(!camera.follow_object(id, Vec3::ZERO, 2.0, 6.0, f32::NAN, [None; 3]));
        assert_eq!(camera.matrix(), before);
        assert!(!camera.set_view(Vec3::ZERO, Vec3::ZERO));
        assert!(camera.character_state().is_some());
        camera.key_down("KeyW".into());
        camera.set_type(CameraType::Free);
        assert!(camera.character_state().is_none());
        let free_position = camera.position();
        camera.update(0.1);
        assert_eq!(camera.position(), free_position);
        assert!(camera.follow_object(id, Vec3::ZERO, 2.0, 6.0, 0.0, [None; 3]));
        assert!(camera.set_view(Vec3::new(3.0, 2.0, -3.0), Vec3::ZERO));
        assert!(camera.character_state().is_none());
        let revision = scene.render_revision();
        camera.update_scene(&mut scene, 0.1);
        assert_eq!(scene.render_revision(), revision);
    }

    #[test]
    fn animation_clips_change_only_on_idle_walk_run_transitions() {
        use crate::gltf_model::AnimationClip;
        let mut asset = crate::gltf_loader::parse_model(
            include_bytes!("../../tests/fixtures/gltf/triangle.gltf"),
            &[include_bytes!("../../tests/fixtures/gltf/triangle.bin").to_vec()],
            &[],
        )
        .unwrap();
        asset.animations = (0..3)
            .map(|_| AnimationClip {
                name: None,
                channels: vec![],
                duration: 2.0,
            })
            .collect();
        let mut scene = SceneManager::default();
        let id = scene.add_model(std::sync::Arc::new(asset)).unwrap();
        let mut camera = ActiveCamera::new(1.0);
        assert!(camera.follow_object(id, Vec3::ZERO, 2.0, 6.0, 0.0, [Some(0), Some(1), Some(2)]));
        camera.update_scene(&mut scene, 0.05);
        assert_eq!(scene.model(id).unwrap().playback.as_ref().unwrap().clip, 0);
        scene.update(0.05);
        let revision = scene.render_revision();
        camera.update_scene(&mut scene, 0.05);
        assert_eq!(scene.render_revision(), revision);
        assert_eq!(
            scene.model(id).unwrap().playback.as_ref().unwrap().time,
            0.05
        );
        camera.key_down("KeyW".into());
        camera.update_scene(&mut scene, 0.05);
        assert_eq!(scene.model(id).unwrap().playback.as_ref().unwrap().clip, 1);
        scene.update(0.05);
        camera.update_scene(&mut scene, 0.05);
        assert_eq!(
            scene.model(id).unwrap().playback.as_ref().unwrap().time,
            0.05
        );
        camera.key_down("ShiftLeft".into());
        camera.update_scene(&mut scene, 0.05);
        assert_eq!(scene.model(id).unwrap().playback.as_ref().unwrap().clip, 2);
        camera.clear_input();
        camera.update_scene(&mut scene, 0.05);
        assert_eq!(scene.model(id).unwrap().playback.as_ref().unwrap().clip, 0);
    }

    fn fox_asset() -> std::sync::Arc<crate::gltf_model::ModelAsset> {
        let asset = crate::gltf_loader::parse_model(
            include_bytes!("../../assets/models/fox/Fox.glb"),
            &[],
            &[],
        )
        .unwrap();
        let names: Vec<_> = asset
            .animations
            .iter()
            .map(|clip| clip.name.as_deref())
            .collect();
        assert_eq!(names, vec![Some("Survey"), Some("Walk"), Some("Run")]);
        std::sync::Arc::new(asset)
    }

    fn start_fox(
        asset: std::sync::Arc<crate::gltf_model::ModelAsset>,
        running: bool,
        idle: Option<usize>,
    ) -> (SceneManager, ActiveCamera, u32) {
        let mut scene = SceneManager::default();
        let id = scene.add_model(asset).unwrap();
        let mut camera = ActiveCamera::new(1.0);
        assert!(camera.follow_object(id, Vec3::ZERO, 2.0, 6.0, 0.0, [idle, Some(1), Some(2)]));
        camera.key_down("KeyW".into());
        if running {
            camera.key_down("ShiftLeft".into());
        }
        camera.update_scene(&mut scene, 0.05);
        assert_eq!(
            scene.model(id).unwrap().playback.as_ref().unwrap().clip,
            if running { 2 } else { 1 }
        );
        (scene, camera, id)
    }

    #[test]
    fn fox_detach_finishes_walk_run_once_for_mode_and_view_switches() {
        let asset = fox_asset();
        for running in [false, true] {
            for view in [false, true] {
                for idle in [Some(0), None, Some(usize::MAX)] {
                    let (mut scene, mut camera, id) = start_fox(asset.clone(), running, idle);
                    if view {
                        assert!(camera.set_view(Vec3::new(3.0, 2.0, -3.0), Vec3::ZERO));
                    } else {
                        camera.set_type(CameraType::Orbit);
                    }
                    assert!(camera.character_state().is_none());
                    camera.update_scene(&mut scene, 0.05);
                    let model = scene.model(id).unwrap();
                    if idle == Some(0) {
                        assert_eq!(model.playback.as_ref().unwrap().clip, 0);
                    } else {
                        assert!(model.playback.is_none());
                    }
                    let revision = scene.render_revision();
                    let model_revision = model.revision;
                    for _ in 0..3 {
                        camera.update_scene(&mut scene, 0.05);
                    }
                    assert_eq!(scene.render_revision(), revision);
                    assert_eq!(scene.model(id).unwrap().revision, model_revision);
                }
            }
        }
    }

    #[test]
    fn fox_repeated_switches_and_rebinds_finish_before_new_owner() {
        let (mut scene, mut camera, id) = start_fox(fox_asset(), true, Some(0));
        camera.set_type(CameraType::Free);
        assert!(camera.follow_object(id, Vec3::ZERO, 2.0, 6.0, 0.0, [Some(0), Some(1), Some(2)]));
        camera.set_type(CameraType::Orbit);
        camera.set_type(CameraType::Free);
        assert!(camera.follow_object(id, Vec3::ZERO, 2.0, 6.0, 0.0, [Some(0), Some(1), Some(2)]));
        // Replacing a live controller also queues its completion, without dropping older work.
        assert!(camera.follow_object(id, Vec3::ZERO, 2.0, 6.0, 0.0, [Some(0), Some(1), Some(2)]));
        camera.key_down("KeyW".into());
        camera.key_down("ShiftLeft".into());
        camera.update_scene(&mut scene, 0.05);
        assert_eq!(scene.model(id).unwrap().playback.as_ref().unwrap().clip, 2);
        scene.update(0.05);
        camera.update_scene(&mut scene, 0.05);
        assert_eq!(
            scene.model(id).unwrap().playback.as_ref().unwrap().time,
            0.05
        );
    }

    #[test]
    fn fox_detach_while_paused_freezes_pose_and_never_resumes_stale_playback() {
        let (mut scene, mut camera, id) = start_fox(fox_asset(), true, Some(0));
        scene.update(0.05);
        scene.paused = true;
        camera.set_type(CameraType::Free);
        let revision = scene.render_revision();
        let model_revision = scene.model(id).unwrap().revision;
        let transform = scene.objects()[&id].transform;
        let time = scene.model(id).unwrap().playback.as_ref().unwrap().time;
        camera.update_scene(&mut scene, 0.05);
        assert!(!scene.model(id).unwrap().playback.as_ref().unwrap().playing);
        assert_eq!(scene.render_revision(), revision);
        assert_eq!(scene.model(id).unwrap().revision, model_revision);
        assert_eq!(scene.objects()[&id].transform.position, transform.position);
        assert_eq!(scene.objects()[&id].transform.rotation, transform.rotation);
        scene.paused = false;
        scene.update(0.05);
        camera.update_scene(&mut scene, 0.05);
        assert_eq!(
            scene.model(id).unwrap().playback.as_ref().unwrap().time,
            time
        );
        assert_eq!(scene.render_revision(), revision);
    }

    #[test]
    fn fox_pending_finish_ignores_removed_target_and_external_playback() {
        let asset = fox_asset();
        for removed in [false, true] {
            let (mut scene, mut camera, id) = start_fox(asset.clone(), true, Some(0));
            camera.set_type(CameraType::Orbit);
            if removed {
                assert!(scene.remove_object(id));
            } else {
                // A scene command supersedes the clip last selected by this controller.
                scene.play_model_animation(id, 1, true, 1.0).unwrap();
            }
            let revision = scene.render_revision();
            camera.update_scene(&mut scene, 0.05);
            assert_eq!(scene.render_revision(), revision);
            if !removed {
                assert_eq!(scene.model(id).unwrap().playback.as_ref().unwrap().clip, 1);
            }
        }
    }

    #[test]
    fn fox_pending_finish_preserves_external_same_clip_speed() {
        let (mut scene, mut camera, id) = start_fox(fox_asset(), true, Some(0));
        camera.set_type(CameraType::Orbit);
        scene.play_model_animation(id, 2, true, 0.5).unwrap();
        let revision = scene.render_revision();
        let playback_revision = scene.model(id).unwrap().playback_revision;
        for _ in 0..2 {
            camera.update_scene(&mut scene, 0.05);
        }
        let model = scene.model(id).unwrap();
        let playback = model.playback.as_ref().unwrap();
        assert_eq!(playback.clip, 2);
        assert_eq!(playback.speed, 0.5);
        assert!(playback.playing);
        assert_eq!(model.playback_revision, playback_revision);
        assert_eq!(scene.render_revision(), revision);
    }

    #[test]
    fn fox_pending_finish_preserves_external_pause_command_authority() {
        let asset = fox_asset();
        for paused in [false, true] {
            let (mut scene, mut camera, id) = start_fox(asset.clone(), true, Some(0));
            scene.paused = true;
            camera.set_type(CameraType::Free);
            scene.pause_model_animation(id, paused).unwrap();
            let revision = scene.render_revision();
            let playback_revision = scene.model(id).unwrap().playback_revision;
            camera.update_scene(&mut scene, 0.05);
            let model = scene.model(id).unwrap();
            let playback = model.playback.as_ref().unwrap();
            assert_eq!(playback.clip, 2);
            assert_eq!(playback.playing, !paused);
            assert_eq!(model.playback_revision, playback_revision);
            assert_eq!(scene.render_revision(), revision);
            scene.paused = false;
            camera.update_scene(&mut scene, 0.05);
            assert_eq!(
                scene.model(id).unwrap().playback_revision,
                playback_revision
            );
            assert_eq!(scene.render_revision(), revision);
        }
    }

    fn physics_setup() -> (SceneManager, ActiveCamera, u32, u32) {
        let mut scene = SceneManager::default();
        let floor = scene.add_cube(Vec3::new(0.0, -0.5, 0.0));
        scene.set_object_scale(floor, Vec3::new(20.0, 1.0, 20.0));
        scene.set_box_collider(floor, Vec3::splat(0.5), Vec3::ZERO);
        let id = scene.add_cube(Vec3::ZERO);
        let mut camera = ActiveCamera::new(1.0);
        camera.follow_object(id, Vec3::ZERO, 2.5, 5.0, 0.0, [None; 3]);
        assert!(camera.set_character_physics(CharacterPhysics {
            half_extents: Vec3::new(0.4, 0.8, 0.8),
            gravity: 18.0,
            jump_speed: 7.0,
        }));
        (scene, camera, id, floor)
    }

    fn physics_tick(scene: &mut SceneManager, camera: &mut ActiveCamera, frames: usize) {
        for _ in 0..frames {
            camera.update_scene(scene, 1.0 / 60.0);
        }
    }

    #[test]
    fn physics_blocks_walk_then_jumps_and_lands_without_held_key_bouncing() {
        let (mut scene, mut camera, id, _) = physics_setup();
        let platform = scene.add_cube(Vec3::new(0.0, 0.3, 3.0));
        scene.set_object_scale(platform, Vec3::new(2.6, 0.6, 2.0));
        scene.set_box_collider(platform, Vec3::splat(0.5), Vec3::ZERO);
        camera.key_down("KeyW".into());
        physics_tick(&mut scene, &mut camera, 60);
        assert!((scene.objects()[&id].transform.position.z - 1.2).abs() < 1e-4);
        assert!(camera.character_physics_state().unwrap().0);
        camera.key_down("Space".into());
        physics_tick(&mut scene, &mut camera, 12);
        assert!(scene.objects()[&id].transform.position.y > 0.5);
        assert!(!camera.character_physics_state().unwrap().0);
        physics_tick(&mut scene, &mut camera, 18);
        camera.key_up("KeyW".into());
        physics_tick(&mut scene, &mut camera, 60);
        assert!((scene.objects()[&id].transform.position.y - 0.6).abs() < 1e-4);
        camera.key_down("Space".into());
        physics_tick(&mut scene, &mut camera, 30);
        assert!((scene.objects()[&id].transform.position.y - 0.6).abs() < 1e-4);
        assert!(camera.character_physics_state().unwrap().0);
        camera.key_up("Space".into());
        camera.key_down("Space".into());
        physics_tick(&mut scene, &mut camera, 12);
        assert!(scene.objects()[&id].transform.position.y > 1.1);
    }

    #[test]
    fn physics_pause_teleport_and_removed_floor_do_not_keep_stale_motion_or_support() {
        let (mut scene, mut camera, id, floor) = physics_setup();
        physics_tick(&mut scene, &mut camera, 1);
        camera.key_down("Space".into());
        physics_tick(&mut scene, &mut camera, 12);
        let before = camera.character_state();
        let velocity = camera.character_physics_state().unwrap().1;
        scene.paused = true;
        physics_tick(&mut scene, &mut camera, 20);
        assert_eq!(camera.character_state().unwrap().1, before.unwrap().1);
        assert_eq!(camera.character_physics_state().unwrap().1, velocity);
        scene.set_object_position(id, Vec3::ZERO);
        scene.paused = false;
        physics_tick(&mut scene, &mut camera, 1);
        assert!(camera.character_physics_state().unwrap().0);
        assert_eq!(camera.character_physics_state().unwrap().1, 0.0);
        assert!(!camera.set_character_physics(CharacterPhysics {
            half_extents: Vec3::ZERO,
            gravity: 18.0,
            jump_speed: 7.0,
        }));
        scene.remove_collider(floor);
        physics_tick(&mut scene, &mut camera, 8);
        assert!(scene.objects()[&id].transform.position.y < 0.0);
        assert!(!camera.character_physics_state().unwrap().0);
    }

    #[test]
    fn physics_zero_delta_does_not_consume_a_queued_jump() {
        let (mut scene, mut camera, id, _) = physics_setup();
        physics_tick(&mut scene, &mut camera, 1);
        camera.key_down("Space".into());
        camera.update_scene(&mut scene, 0.0);
        assert_eq!(scene.objects()[&id].transform.position, Vec3::ZERO);
        physics_tick(&mut scene, &mut camera, 1);
        assert!(scene.objects()[&id].transform.position.y > 0.0);
    }
}
