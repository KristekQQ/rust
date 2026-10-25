//! Generic Rust-owned XR tracking, independent of optional scene-object bindings.
use crate::scene::{CutReport, SceneManager};
use crate::xr_input::{valid_pose, TrackedController};
use glam::{EulerRot, Mat4, Quat, Vec3};
use std::collections::{BTreeSet, VecDeque};

const HIDDEN: Vec3 = Vec3::new(0., -10_000., 0.);

/// Net grip displacement over ~60ms, not the amplified velocity of a remote blade tip.
#[derive(Default)]
pub(crate) struct GripSpeed {
    time: f32,
    samples: VecDeque<(f32, Vec3)>,
}
impl GripSpeed {
    pub(crate) fn sample(&mut self, from: Vec3, to: Vec3, dt: f32) -> f32 {
        if self.samples.is_empty() {
            self.samples.push_back((self.time, from));
        }
        self.time += dt;
        self.samples.push_back((self.time, to));
        while self.samples.len() > 2
            && (self.samples[1].0 <= self.time - 0.06 || self.samples.len() > 10)
        {
            self.samples.pop_front();
        }
        let (start, position) = self.samples[0];
        to.distance(position) / (self.time - start).max(0.001)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TrackedPoseState {
    pub matrix: Mat4,
    /// Net translation in meters per second; head tracking currently reports zero.
    pub speed: f32,
}

#[derive(Default)]
pub struct XrPlayer {
    pub locomotion: crate::xr_locomotion::XrLocomotion,
    pub controllers: [TrackedController; 2],
    head: Option<TrackedPoseState>,
    hands: [Option<TrackedPoseState>; 2],
    head_binding: Option<(u32, Vec3, Vec3, bool)>,
    reference: Mat4,
    hand_speeds: [GripSpeed; 2],
    inventory: BTreeSet<u32>,
}

impl XrPlayer {
    pub fn head(&self) -> Option<TrackedPoseState> {
        self.head
    }

    pub fn hand(&self, hand: usize) -> Option<TrackedPoseState> {
        self.hands.get(hand).copied().flatten()
    }

    pub fn head_object(&self) -> Option<u32> {
        self.head_binding.map(|(id, _, _, _)| id)
    }

    /// Registered scene objects, independent of their optional hand equipment slots.
    pub fn inventory(&self) -> Vec<u32> {
        self.inventory.iter().copied().collect()
    }

    pub fn add_item(&mut self, scene: &SceneManager, id: u32) -> Result<(), String> {
        self.validate_item(scene, id)?;
        self.prune_inventory(scene);
        self.inventory.insert(id);
        Ok(())
    }

    /// Remove ownership and hide held equipment; scene objects remain caller-owned.
    pub fn remove_item(&mut self, scene: &mut SceneManager, id: u32) -> Result<(), String> {
        for controller in &mut self.controllers {
            if controller.object_id() == Some(id) {
                controller.bind(scene, None, false)?;
            }
        }
        self.inventory.remove(&id);
        Ok(())
    }

    pub fn bind_hand(
        &mut self,
        scene: &mut SceneManager,
        hand: usize,
        id: Option<u32>,
        cutting: bool,
    ) -> Result<(), String> {
        if hand >= self.controllers.len() {
            return Err("Tracked hand index must be 0 or 1".into());
        }
        if let Some(id) = id {
            self.validate_item(scene, id)?;
            if self.controllers[1 - hand].object_id() == Some(id) || self.head_object() == Some(id)
            {
                return Err("Tracked object is already equipped in another slot".into());
            }
        }
        // Controller binding validates cutter configuration before changing its old binding.
        self.controllers[hand].bind(scene, id, cutting)?;
        self.prune_inventory(scene);
        if let Some(id) = id {
            self.inventory.insert(id);
        }
        Ok(())
    }

    fn validate_item(&self, scene: &SceneManager, id: u32) -> Result<(), String> {
        if !scene.object_exists(id) {
            return Err("Inventory object does not exist".into());
        }
        let live_count = self
            .inventory
            .iter()
            .filter(|&&id| scene.object_exists(id))
            .count();
        if !self.inventory.contains(&id) && live_count >= 64 {
            return Err("XR player inventory is full (64 objects)".into());
        }
        Ok(())
    }

    fn prune_inventory(&mut self, scene: &SceneManager) {
        self.inventory.retain(|&id| scene.object_exists(id));
    }

    pub fn bind_head(&mut self, scene: &mut SceneManager, id: Option<u32>) -> Result<(), String> {
        self.bind_head_offset(scene, id, Vec3::ZERO, false)
    }

    pub fn set_reference(&mut self, reference: Mat4) {
        if valid_pose(reference) {
            self.reference = reference;
        }
    }

    pub fn bind_head_offset(
        &mut self,
        scene: &mut SceneManager,
        id: Option<u32>,
        offset: Vec3,
        feet: bool,
    ) -> Result<(), String> {
        if !offset.is_finite() || offset.abs().max_element() > 10. {
            return Err("Tracked attachment offset must be finite within 10m".into());
        }
        if id.is_some_and(|id| self.controllers.iter().any(|c| c.object_id() == Some(id))) {
            return Err("Tracked object is already equipped in a hand".into());
        }
        let binding = id
            .map(|id| {
                scene
                    .objects()
                    .get(&id)
                    .map(|object| (id, object.transform.scale, offset, feet))
                    .ok_or_else(|| "Tracked head object does not exist".to_string())
            })
            .transpose()?;
        if let Some((old, _, _, _)) = self.head_binding {
            scene.set_object_position(old, HIDDEN);
        }
        self.head_binding = binding;
        if let Some((id, _, _, _)) = binding {
            scene.set_object_position(id, HIDDEN);
        }
        Ok(())
    }

    pub fn update_head(
        &mut self,
        scene: &mut SceneManager,
        pose: Option<Mat4>,
    ) -> Result<(), String> {
        self.prune_inventory(scene);
        if self
            .head_object()
            .is_some_and(|id| !scene.object_exists(id))
        {
            self.head_binding = None;
        }
        let invalid = pose.is_some_and(|pose| !valid_pose(pose));
        self.head = pose
            .filter(|_| !invalid)
            .map(|matrix| TrackedPoseState { matrix, speed: 0. });
        if let Some((id, scale, offset, feet)) = self.head_binding {
            if let Some(state) = self.head {
                let (y, x, z) = Quat::from_mat4(&state.matrix)
                    .normalize()
                    .to_euler(EulerRot::YXZ);
                let head = state.matrix.w_axis.truncate();
                let (position, rotation) = if feet {
                    let floor = Vec3::new(head.x, self.reference.w_axis.y, head.z);
                    let position = floor + self.reference.transform_vector3(offset);
                    let toward = head - position;
                    (
                        position,
                        Vec3::new(
                            -toward.y.atan2(toward.x.hypot(toward.z)),
                            toward.x.atan2(toward.z),
                            0.,
                        ),
                    )
                } else {
                    (state.matrix.transform_point3(offset), Vec3::new(x, y, z))
                };
                scene.set_object_transform(id, position, rotation, scale);
            } else {
                scene.set_object_position(id, HIDDEN);
            }
        }
        if invalid {
            Err("Tracked pose must be a finite rigid transform within 100m".into())
        } else {
            Ok(())
        }
    }

    pub fn update_hand(
        &mut self,
        scene: &mut SceneManager,
        hand: usize,
        pose: Option<Mat4>,
        pressed: bool,
        dt: f32,
    ) -> Result<CutReport, String> {
        if hand >= self.hands.len() {
            return Err("Tracked hand index must be 0 or 1".into());
        }
        self.prune_inventory(scene);
        if pose.is_some_and(|pose| !valid_pose(pose)) {
            self.hands[hand] = None;
            self.hand_speeds[hand] = GripSpeed::default();
            self.controllers[hand].reset(scene);
            return Err("Tracked pose must be a finite rigid transform within 100m".into());
        }
        let speed = if let Some(matrix) = pose {
            let speed = self.hands[hand].and_then(|previous| {
                let from = previous.matrix.w_axis.truncate();
                let to = matrix.w_axis.truncate();
                let distance = from.distance(to);
                let angle = Quat::from_mat4(&matrix)
                    .angle_between(Quat::from_mat4(&previous.matrix))
                    .abs();
                (dt.is_finite()
                    && dt > 0.
                    && dt <= 0.05
                    && distance <= 1.
                    && distance <= 15. * dt + 0.025
                    && angle <= 1.5
                    && angle <= 45. * dt + 0.03)
                    .then(|| self.hand_speeds[hand].sample(from, to, dt))
            });
            if speed.is_none() {
                self.hand_speeds[hand] = GripSpeed::default();
            }
            let speed = speed.unwrap_or(0.);
            self.hands[hand] = Some(TrackedPoseState { matrix, speed });
            speed
        } else {
            self.hands[hand] = None;
            self.hand_speeds[hand] = GripSpeed::default();
            0.
        };
        self.controllers[hand].update_timed_with_speed(scene, pose, pressed, dt, Some(speed))
    }

    /// Clear poses and motion history while preserving optional object bindings.
    pub fn reset_tracking(&mut self, scene: &mut SceneManager) {
        self.head = None;
        self.hands = [None; 2];
        self.hand_speeds = Default::default();
        if let Some((id, _, _, _)) = self.head_binding {
            scene.set_object_position(id, HIDDEN);
        }
        for controller in &mut self.controllers {
            controller.reset(scene);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xr_player_tracks_unbound_head_and_hands_without_spawning() {
        let mut scene = SceneManager::default();
        let mut player = XrPlayer::default();
        let pose = Mat4::from_translation(Vec3::new(1., 2., 3.));
        player.update_head(&mut scene, Some(pose)).unwrap();
        for hand in 0..2 {
            player
                .update_hand(&mut scene, hand, Some(pose), false, 0.01)
                .unwrap();
            assert_eq!(player.hand(hand).unwrap().matrix, pose);
        }
        assert_eq!(player.head().unwrap().matrix, pose);
        assert!(scene.objects().is_empty());
        assert!(player.hand(2).is_none());
        assert!(player
            .update_hand(&mut scene, 2, Some(pose), false, 0.01)
            .is_err());
    }

    #[test]
    fn feet_attachment_ignores_gaze_but_follows_reference_and_floor() {
        let mut scene = SceneManager::default();
        let mut player = XrPlayer::default();
        let panel = scene.add_cube(Vec3::ZERO);
        let offset = Vec3::new(0.9, 0.35, -1.);
        player
            .bind_head_offset(&mut scene, Some(panel), offset, true)
            .unwrap();
        let head = Vec3::new(2., 1.7, 3.);
        player
            .update_head(&mut scene, Some(Mat4::from_translation(head)))
            .unwrap();
        let position = scene.objects()[&panel].transform.position;
        for yaw in [0.5, 1.5, -1.] {
            player
                .update_head(
                    &mut scene,
                    Some(Mat4::from_rotation_translation(
                        Quat::from_rotation_y(yaw),
                        head,
                    )),
                )
                .unwrap();
            assert!(scene.objects()[&panel]
                .transform
                .position
                .abs_diff_eq(position, 1e-6));
        }
        let reference =
            Mat4::from_rotation_translation(Quat::from_rotation_y(1.), Vec3::new(0., 0.4, 0.));
        player.set_reference(reference);
        player
            .update_head(&mut scene, Some(Mat4::from_translation(head)))
            .unwrap();
        assert!(scene.objects()[&panel].transform.position.abs_diff_eq(
            Vec3::new(head.x, 0.4, head.z) + reference.transform_vector3(offset),
            1e-6
        ));
        player.update_head(&mut scene, None).unwrap();
        assert_eq!(scene.objects()[&panel].transform.position, HIDDEN);
        assert!(player
            .bind_head_offset(&mut scene, Some(panel), Vec3::splat(f32::NAN), true)
            .is_err());
    }

    #[test]
    fn xr_player_bindings_preserve_scale_orientation_and_hide_on_loss() {
        let mut scene = SceneManager::default();
        let mut player = XrPlayer::default();
        let head = scene.add_cube(Vec3::ZERO);
        let hand = scene.add_cube(Vec3::ZERO);
        let scale = Vec3::new(0.2, 0.3, 0.4);
        for id in [head, hand] {
            scene.set_object_scale(id, scale);
        }
        player.bind_head(&mut scene, Some(head)).unwrap();
        player.bind_hand(&mut scene, 0, Some(hand), false).unwrap();
        let pose = Mat4::from_rotation_translation(
            Quat::from_euler(EulerRot::YXZ, 1.1, 0.7, -0.4),
            Vec3::new(1., 2., 3.),
        );
        player.update_head(&mut scene, Some(pose)).unwrap();
        player
            .update_hand(&mut scene, 0, Some(pose), false, 0.01)
            .unwrap();
        for id in [head, hand] {
            let transform = scene.objects()[&id].transform;
            assert_eq!(transform.scale, scale);
            assert!(transform
                .model()
                .abs_diff_eq(pose * Mat4::from_scale(scale), 1e-5));
        }
        player.reset_tracking(&mut scene);
        assert!(player.head().is_none() && player.hand(0).is_none());
        assert_eq!(player.head_object(), Some(head));
        assert_eq!(player.controllers[0].object_id(), Some(hand));
        for id in [head, hand] {
            assert_eq!(scene.objects()[&id].transform.position, HIDDEN);
        }
        player.update_head(&mut scene, Some(pose)).unwrap();
        player.update_head(&mut scene, None).unwrap();
        assert_eq!(scene.objects()[&head].transform.position, HIDDEN);
        player.bind_head(&mut scene, None).unwrap();
        assert!(scene.object_exists(head));
    }

    #[test]
    fn xr_player_hand_speed_uses_grip_time_and_resets_after_loss_or_invalid_motion() {
        let mut scene = SceneManager::default();
        let mut player = XrPlayer::default();
        for i in 0..10 {
            player
                .update_hand(
                    &mut scene,
                    0,
                    Some(Mat4::from_translation(Vec3::X * i as f32 * 0.01)),
                    false,
                    0.01,
                )
                .unwrap();
            if i > 0 {
                assert!((player.hand(0).unwrap().speed - 1.).abs() < 1e-5);
            }
        }
        player
            .update_hand(&mut scene, 0, None, false, 0.01)
            .unwrap();
        assert!(player.hand(0).is_none());
        for (pose, dt) in [
            (Mat4::IDENTITY, 0.01),
            (Mat4::from_translation(Vec3::X), 0.01),
            (Mat4::IDENTITY, f32::NAN),
        ] {
            player
                .update_hand(&mut scene, 0, Some(pose), false, dt)
                .unwrap();
            assert_eq!(player.hand(0).unwrap().speed, 0.);
        }
        assert!(player
            .update_hand(
                &mut scene,
                0,
                Some(Mat4::from_scale(Vec3::splat(2.))),
                false,
                0.01
            )
            .is_err());
        assert!(player.hand(0).is_none());
        assert!(player
            .update_head(
                &mut scene,
                Some(Mat4::from_translation(Vec3::splat(f32::NAN)))
            )
            .is_err());
        assert!(player.head().is_none());
    }

    #[test]
    fn xr_player_inventory_equipment_transfer_and_removal_keep_scene_objects() {
        let mut scene = SceneManager::default();
        let mut player = XrPlayer::default();
        let first = scene.add_cube(Vec3::ZERO);
        let second = scene.add_cube(Vec3::ONE);
        player.add_item(&scene, first).unwrap();
        player.add_item(&scene, first).unwrap();
        player.bind_hand(&mut scene, 0, Some(first), false).unwrap();
        player.bind_hand(&mut scene, 0, None, false).unwrap();
        assert_eq!(player.inventory(), vec![first]);
        assert_eq!(scene.objects()[&first].transform.position, HIDDEN);
        player.bind_hand(&mut scene, 1, Some(first), false).unwrap();
        player
            .bind_hand(&mut scene, 0, Some(second), false)
            .unwrap();
        assert_eq!(player.inventory(), vec![first, second]);
        player.reset_tracking(&mut scene);
        assert_eq!(player.inventory(), vec![first, second]);
        assert_eq!(player.controllers[1].object_id(), Some(first));
        player.remove_item(&mut scene, first).unwrap();
        assert_eq!(player.controllers[1].object_id(), None);
        assert_eq!(player.inventory(), vec![second]);
        assert_eq!(scene.objects()[&first].transform.position, HIDDEN);
        player.bind_hand(&mut scene, 0, None, false).unwrap();
        scene.set_object_position(second, Vec3::ONE);
        player.remove_item(&mut scene, second).unwrap();
        assert_eq!(scene.objects()[&second].transform.position, Vec3::ONE);
        assert!(player.inventory().is_empty());
        assert_eq!(scene.objects().len(), 2);
    }

    #[test]
    fn xr_player_equipment_conflicts_and_failed_validation_are_atomic() {
        let mut scene = SceneManager::default();
        let mut player = XrPlayer::default();
        let first = scene.add_cube(Vec3::ZERO);
        let second = scene.add_cube(Vec3::ZERO);
        let third = scene.add_cube(Vec3::ZERO);
        player.bind_hand(&mut scene, 0, Some(first), false).unwrap();
        player
            .bind_hand(&mut scene, 1, Some(second), false)
            .unwrap();
        player
            .update_hand(&mut scene, 1, Some(Mat4::IDENTITY), false, 0.01)
            .unwrap();
        for (hand, id, cutting) in [
            (1, first, false),
            (2, third, false),
            (1, u32::MAX, false),
            (1, third, true),
        ] {
            assert!(player
                .bind_hand(&mut scene, hand, Some(id), cutting)
                .is_err());
            assert_eq!(player.controllers[0].object_id(), Some(first));
            assert_eq!(player.controllers[1].object_id(), Some(second));
            assert_eq!(player.inventory(), vec![first, second]);
            assert_eq!(scene.objects()[&second].transform.position, Vec3::ZERO);
        }
        assert!(player.bind_head(&mut scene, Some(first)).is_err());
        assert_eq!(player.head_object(), None);
        player.bind_head(&mut scene, Some(third)).unwrap();
        assert!(player.bind_hand(&mut scene, 0, Some(third), false).is_err());
        assert_eq!(player.head_object(), Some(third));
        assert_eq!(player.controllers[0].object_id(), Some(first));
    }

    #[test]
    fn xr_player_inventory_limit_prunes_removed_objects_on_tracking_updates() {
        let mut scene = SceneManager::default();
        let mut player = XrPlayer::default();
        let ids: Vec<_> = (0..65).map(|_| scene.add_cube(Vec3::ZERO)).collect();
        for &id in &ids[..64] {
            player.add_item(&scene, id).unwrap();
        }
        player.add_item(&scene, ids[0]).unwrap();
        assert!(player.add_item(&scene, ids[64]).is_err());
        assert!(player
            .bind_hand(&mut scene, 0, Some(ids[64]), false)
            .is_err());
        assert_eq!(player.controllers[0].object_id(), None);
        assert_eq!(player.inventory().len(), 64);
        scene.remove_object(ids[0]);
        player
            .update_hand(&mut scene, 0, None, false, 0.01)
            .unwrap();
        assert_eq!(player.inventory().len(), 63);
        player
            .bind_hand(&mut scene, 0, Some(ids[64]), false)
            .unwrap();
        assert_eq!(player.inventory().len(), 64);
        assert!(!player.inventory().contains(&ids[0]));
    }
}
