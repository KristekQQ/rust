//! Rust-owned grip tracking and bounded, finite CPU blade sweeps.
//!
//! Blade samples form a finite piecewise planar ribbon in each target's local
//! translation frame. Target translation is included while a full cross-section
//! accumulates; its current orientation is used for the final mesh split. Twisted strokes, teleports and tracking loss never become giant
//! inferred cutting planes.
use crate::scene::{CutReport, SceneManager};
use crate::xr_player::GripSpeed;
use glam::{EulerRot, Mat4, Quat, Vec3};
use std::collections::BTreeMap;

const MAX_RIBBON_SAMPLES: usize = 256;

const HIDDEN: Vec3 = Vec3::new(0., -10_000., 0.);

#[derive(Clone, Copy)]
struct Sample {
    pose: Mat4,
    endpoints: [Vec3; 2],
    orientation: Quat,
}

#[derive(Default)]
pub struct TrackedController {
    id: Option<u32>,
    cutting: bool,
    scale: Vec3,
    previous: Option<Sample>,
    grip_speed: GripSpeed,
    anchor: Option<Sample>,
    direction: Vec3,
    frames: u32,
    ribbons: BTreeMap<u32, Vec<[Vec3; 2]>>,
    target_positions: BTreeMap<u32, Vec3>,
}

impl TrackedController {
    /// The scene object bound to this controller, if any.
    pub fn object_id(&self) -> Option<u32> {
        self.id
    }

    pub fn bind(
        &mut self,
        scene: &mut SceneManager,
        id: Option<u32>,
        cutting: bool,
    ) -> Result<(), String> {
        let scale = if let Some(id) = id {
            let object = scene
                .objects()
                .get(&id)
                .ok_or("Tracked object does not exist")?;
            if cutting && scene.tracked_blade(id).is_none() {
                return Err("Tracked cutting object must be a registered cutter".into());
            }
            object.transform.scale
        } else {
            Vec3::ONE
        };
        self.reset(scene);
        self.id = id;
        self.cutting = cutting;
        self.scale = scale;
        if let Some(id) = id {
            scene.set_object_position(id, HIDDEN);
        }
        Ok(())
    }

    /// Keep the binding, but hide it and discard all tracking/stroke history.
    pub fn reset(&mut self, scene: &mut SceneManager) {
        if let Some(id) = self.id {
            scene.release_blade_cuts(id, false);
            scene.reset_blade_trail(id);
            scene.set_object_position(id, HIDDEN);
        }
        self.ribbons.clear();
        self.target_positions.clear();
        self.previous = None;
        self.grip_speed = GripSpeed::default();
        self.anchor = None;
        self.direction = Vec3::ZERO;
        self.frames = 0;
    }

    pub fn update(
        &mut self,
        scene: &mut SceneManager,
        pose: Option<Mat4>,
        pressed: bool,
    ) -> Result<CutReport, String> {
        self.update_timed(scene, pose, pressed, 1. / 90.)
    }

    /// The elapsed XR frame time bounds plausible motion before interpolation.
    pub fn update_timed(
        &mut self,
        scene: &mut SceneManager,
        pose: Option<Mat4>,
        pressed: bool,
        dt: f32,
    ) -> Result<CutReport, String> {
        self.update_timed_with_speed(scene, pose, pressed, dt, None)
    }

    /// Player-owned tracking supplies its measured grip speed; standalone callers
    /// retain the controller's original measurement through `update_timed`.
    pub(crate) fn update_timed_with_speed(
        &mut self,
        scene: &mut SceneManager,
        pose: Option<Mat4>,
        pressed: bool,
        dt: f32,
        speed: Option<f32>,
    ) -> Result<CutReport, String> {
        let Some(id) = self.id else {
            return Ok(CutReport::default());
        };
        if !scene.object_exists(id) {
            self.reset(scene);
            self.id = None;
            self.cutting = false;
            return Ok(CutReport::default());
        }
        let Some(pose) = pose else {
            self.reset(scene);
            return Ok(CutReport::default());
        };
        if !valid_pose(pose) {
            self.reset(scene);
            return Err("Tracked pose must be a finite rigid transform within 100m".into());
        }
        let orientation = Quat::from_mat4(&pose).normalize();
        let blade = scene.tracked_blade(id).unwrap_or([Vec3::ZERO, -Vec3::Z]);
        let sample = Sample {
            pose,
            orientation,
            endpoints: blade.map(|p| pose.transform_point3(p * self.scale)),
        };
        self.apply(scene, id, sample);
        scene.sample_blade_trail(id, sample.endpoints, dt);
        scene.release_fragment_blade(id, sample.endpoints, !pressed);
        let target_positions = std::mem::replace(
            &mut self.target_positions,
            scene
                .triangle_meshes()
                .iter()
                .map(|(&id, _)| (id, scene.objects()[&id].transform.position))
                .collect(),
        );
        let previous = self.previous.replace(sample);
        if !self.cutting || !pressed || previous.is_none() {
            self.grip_speed = GripSpeed::default();
            scene.release_blade_cuts(id, !pressed);
            self.anchor = if self.cutting && pressed {
                Some(sample)
            } else {
                None
            };
            self.direction = Vec3::ZERO;
            self.frames = 0;
            self.ribbons.clear();
            if self.cutting && pressed {
                self.record(scene, sample);
            }
            return Ok(CutReport::default());
        }
        let previous = previous.unwrap();
        let translation = sample
            .pose
            .w_axis
            .truncate()
            .distance(previous.pose.w_axis.truncate());
        let angle = sample.orientation.angle_between(previous.orientation).abs();
        // Grip speed, rather than tip distance, allows fast wrist rotation.
        // Hard caps and a bounded gap still reject teleport/reacquisition chords.
        if !dt.is_finite()
            || dt <= 0.
            || dt > 0.05
            || translation > 1.
            || angle > 1.5
            || translation > 15. * dt + 0.025
            || angle > 45. * dt + 0.03
        {
            self.grip_speed = GripSpeed::default();
            scene.release_blade_cuts(id, false);
            self.anchor = Some(sample);
            self.direction = Vec3::ZERO;
            self.frames = 0;
            self.ribbons.clear();
            self.record(scene, sample);
            return Ok(CutReport::default());
        }
        let blade_speed = speed.unwrap_or_else(|| {
            self.grip_speed.sample(
                previous.pose.w_axis.truncate(),
                sample.pose.w_axis.truncate(),
                dt,
            )
        });
        let assisted = scene.cut_assist(id) > 0.;
        let contact_response = scene.blade_contact_response(id);
        if assisted && translation < 1e-6 && angle < 1e-6 {
            if contact_response {
                scene.blade_contacts(id, previous.pose, sample.pose, &target_positions, dt);
            }
            self.ribbons.clear();
            self.record(scene, sample);
            return Ok(CutReport::default());
        }
        if !assisted {
            scene.follow_blade_jams(id, previous.pose, sample.pose, dt);
        }
        let jam_rotation = sample.orientation * previous.orientation.conjugate();
        for (&target, history) in &mut self.ribbons {
            if scene.blade_cut_state(target).1 == Some(id) {
                for pair in history {
                    *pair = pair.map(|p| jam_rotation * p);
                }
            }
        }
        self.target_positions = scene
            .triangle_meshes()
            .iter()
            .map(|(&id, _)| (id, scene.objects()[&id].transform.position))
            .collect();
        // Do not smooth target teleports into apparently continuous motion.
        self.ribbons.retain(|id, _| {
            scene
                .tracked_previous_position(*id, &target_positions)
                .zip(self.target_positions.get(id))
                .is_some_and(|(a, b)| a.distance(*b) <= 0.75)
        });
        if scene
            .triangle_meshes()
            .keys()
            .any(|&target| scene.blade_cut_state(target).1 == Some(id))
        {
            self.frames = 0;
        }
        self.frames += 1;
        let subdivisions = ((translation / 0.035).max(angle / 0.05).ceil() as u32).clamp(1, 64);
        let mut last = previous;
        let mut report = CutReport::default();
        let mut prefix_queries = 0;
        for index in 1..=subdivisions {
            let t = index as f32 / subdivisions as f32;
            let orientation = previous
                .orientation
                .slerp(sample.orientation, t)
                .normalize();
            let position = previous
                .pose
                .w_axis
                .truncate()
                .lerp(sample.pose.w_axis.truncate(), t);
            let pose = Mat4::from_rotation_translation(orientation, position);
            let current = Sample {
                pose,
                orientation,
                endpoints: blade.map(|p| pose.transform_point3(p * self.scale)),
            };
            let midpoint_step = midpoint(current) - midpoint(last);
            let endpoint_step = current.endpoints[1] - last.endpoints[1];
            let step = if midpoint_step.length_squared() >= 1e-8 {
                midpoint_step
            } else if endpoint_step.length_squared() >= 1e-8 {
                endpoint_step
            } else {
                current.endpoints[0] - last.endpoints[0]
            };
            let changed_direction = step.length_squared() > 1e-6
                && self.direction.length_squared() > 0.5
                && step.normalize().dot(self.direction) < 0.7;
            if let Some(anchor) = self.anchor {
                // A continuous wrist arc has no cumulative angle limit: the
                // per-frame angular guard already validates each real interval.
                if midpoint(current).distance(midpoint(anchor)) > 2.
                    || self.frames > 90
                    || changed_direction
                {
                    // A complete finite hit in this prefix must survive the
                    // continuity reset. Bound mesh queries to three per frame.
                    if prefix_queries < 2 {
                        let prefix = scene.cut_tracked_with(id, &self.ribbons);
                        if prefix.is_err() {
                            self.anchor = Some(sample);
                            self.frames = 0;
                            self.ribbons.clear();
                            self.record(scene, sample);
                        }
                        append_report(&mut report, prefix?);
                        prefix_queries += 1;
                    }
                    scene.release_unjammed_blade_cuts(id, true);
                    scene.release_fragment_blade(id, current.endpoints, changed_direction);
                    self.anchor = Some(last);
                    self.direction = Vec3::ZERO;
                    self.frames = 0;
                    self.ribbons
                        .retain(|target, _| scene.blade_cut_state(*target).1 == Some(id));
                    self.record_interpolated(
                        scene,
                        last,
                        &target_positions,
                        (index - 1) as f32 / subdivisions as f32,
                        None,
                    );
                }
            } else {
                self.anchor = Some(last);
            }
            if step.length_squared() > 1e-6 {
                self.direction = step.normalize();
            }
            // Every blade pose shares the corresponding target translation in time.
            self.record_interpolated(scene, current, &target_positions, t, Some(last));
            last = current;
        }
        // Usually one mesh query per frame, with at most two extra prefix
        // queries when a continuity boundary would discard a completed hit.
        let result = scene.cut_tracked_with(id, &self.ribbons);
        if result.as_ref().is_ok_and(|r| !r.removed.is_empty()) || result.is_err() {
            self.anchor = Some(sample);
            self.frames = 0;
            if result.is_err() {
                self.ribbons.clear();
            } else {
                // A cut replaces only its own source. Other fruit may still be
                // halfway through this very same stroke, including wedged pieces.
                self.ribbons
                    .retain(|target, _| scene.tracked_cut_eligible(*target, id));
            }
            self.record(scene, sample);
        }
        append_report(&mut report, result?);
        let settled = !assisted
            && translation < 1e-5
            && angle < 1e-5
            && scene.settle_shallow_blade_cuts(id, &target_positions);
        if !assisted || contact_response {
            scene.blade_contacts(id, previous.pose, sample.pose, &target_positions, dt);
        }
        if settled {
            self.ribbons
                .retain(|target, _| scene.blade_cut_state(*target).1 == Some(id));
            self.record(scene, sample);
        }
        scene.set_cut_feedback_speed(&report.pieces, blade_speed);
        Ok(report)
    }

    fn record(&mut self, scene: &SceneManager, sample: Sample) {
        self.record_interpolated(scene, sample, &BTreeMap::new(), 1., None);
    }

    fn record_interpolated(
        &mut self,
        scene: &SceneManager,
        sample: Sample,
        previous_targets: &BTreeMap<u32, Vec3>,
        t: f32,
        previous_sample: Option<Sample>,
    ) {
        self.ribbons.retain(|id, _| {
            self.id
                .is_some_and(|cutter| scene.tracked_cut_eligible(*id, cutter))
        });
        for (&id, mesh) in scene.triangle_meshes() {
            if !self
                .id
                .is_some_and(|cutter| scene.tracked_cut_eligible(id, cutter))
            {
                continue;
            }
            let position = scene.objects()[&id].transform.position;
            // A new or teleported target has no valid intermediate trajectory.
            let previous = scene
                .tracked_previous_position(id, previous_targets)
                .filter(|old| old.distance(position) <= 0.75);
            if t < 1. && previous.is_none() {
                continue;
            }
            let position = previous.map_or(position, |old| old.lerp(position, t));
            let config = self.id.and_then(|id| {
                scene
                    .blade_physics(id)
                    .map(|c| c.scaled(self.scale, scene.tracked_blade(id).unwrap()))
            });
            let edge_offset = config.map_or(Vec3::ZERO, |c| {
                sample.pose.transform_vector3(c.edge) * c.half_width
            });
            let points = sample.endpoints.map(|p| p + edge_offset - position);
            let history = self.ribbons.entry(id).or_default();
            if let Some(config) = config {
                if let Some(last) = history.last() {
                    let axis = points[1] - points[0];
                    // Gate at the blade station nearest the actual mesh center:
                    // wrist rotations move different stations in different directions.
                    let (min, max) = mesh.mesh.bounds();
                    let center = scene.objects()[&id]
                        .transform
                        .model()
                        .transform_point3((min + max) * 0.5)
                        - position;
                    let station =
                        ((center - points[0]).dot(axis) / axis.length_squared()).clamp(0., 1.);
                    let delta = last[0].lerp(last[1], station);
                    let delta = points[0].lerp(points[1], station) - delta;
                    let assisted_contacts = self.id.is_some_and(|cutter| {
                        scene.cut_assist(cutter) > 0. && scene.blade_contact_response(cutter)
                    });
                    // Target motion cannot turn a flat controller stroke into an edge cut.
                    let delta = if assisted_contacts {
                        previous_sample.map_or(Vec3::ZERO, |previous| {
                            let old_edge =
                                previous.pose.transform_vector3(config.edge) * config.half_width;
                            sample.endpoints[0].lerp(sample.endpoints[1], station) + edge_offset
                                - previous.endpoints[0].lerp(previous.endpoints[1], station)
                                - old_edge
                        })
                    } else {
                        delta
                    };
                    if (assisted_contacts
                        || self.id.is_none_or(|cutter| scene.cut_assist(cutter) == 0.))
                        && (assisted_contacts || delta.length_squared() > 1e-10)
                        && !(if assisted_contacts {
                            config.assisted_sharp(sample.pose, axis, delta)
                        } else {
                            config.sharp(sample.pose, axis, delta)
                        })
                    {
                        // An established joint keeps its real finite ribbon during
                        // tracking reversals; only sharp advance can extend it.
                        if self
                            .id
                            .is_some_and(|cutter| scene.blade_cut_state(id).1 == Some(cutter))
                        {
                            continue;
                        }
                        // A flat/back interval cannot bridge into a later edge cut.
                        history.clear();
                    }
                }
                // Retain only nearby finite candidates before any mesh cutting.
                let (min, max) = mesh.mesh.bounds();
                let object = &scene.objects()[&id];
                let center =
                    object.transform.model().transform_point3((min + max) * 0.5) - position;
                let radius = ((max - min) * 0.5 * object.transform.scale.abs()).length();
                let axis = points[1] - points[0];
                let closest = points[0]
                    + axis * ((center - points[0]).dot(axis) / axis.length_squared()).clamp(0., 1.);
                if center.distance(closest) > radius + 0.12 && history.len() < 2 {
                    history.clear();
                    continue;
                }
                // Once a real nearby edge sweep has started, retain its exit
                // samples until the finite mesh query below. A fast stroke can
                // enter and leave the broad phase within this same XR frame;
                // clearing on exit would discard its completed cross-section.
            }
            if history
                .last()
                .is_some_and(|last| last.iter().zip(points).any(|(a, b)| a.distance(b) > 0.75))
            {
                history.clear();
            }
            if history.last().is_some_and(|last| {
                last.iter()
                    .zip(points)
                    .all(|(a, b)| a.distance_squared(b) < 1e-10)
            }) {
                continue;
            }
            if history.len() >= MAX_RIBBON_SAMPLES {
                history.drain(..MAX_RIBBON_SAMPLES / 2);
            }
            history.push(points);
        }
    }

    fn apply(&self, scene: &mut SceneManager, id: u32, sample: Sample) {
        let (y, x, z) = sample.orientation.to_euler(EulerRot::YXZ);
        scene.set_object_transform(
            id,
            sample.pose.w_axis.truncate(),
            Vec3::new(x, y, z),
            self.scale,
        );
    }
}

fn append_report(report: &mut CutReport, next: CutReport) {
    report.removed.extend(next.removed);
    report.pieces.extend(next.pieces);
    report.object_candidates += next.object_candidates;
    report.bvh_nodes += next.bvh_nodes;
    report.triangle_tests += next.triangle_tests;
}

fn midpoint(sample: Sample) -> Vec3 {
    (sample.endpoints[0] + sample.endpoints[1]) * 0.5
}

pub(crate) fn valid_pose(pose: Mat4) -> bool {
    if !pose.is_finite() || pose.w_axis.truncate().abs().max_element() > 100. {
        return false;
    }
    let axes = [
        pose.x_axis.truncate(),
        pose.y_axis.truncate(),
        pose.z_axis.truncate(),
    ];
    pose.x_axis.w.abs() < 1e-4
        && pose.y_axis.w.abs() < 1e-4
        && pose.z_axis.w.abs() < 1e-4
        && (pose.w_axis.w - 1.).abs() < 1e-4
        && axes.iter().all(|a| (a.length_squared() - 1.).abs() < 0.002)
        && axes[0].dot(axes[1]).abs() < 0.001
        && axes[0].dot(axes[2]).abs() < 0.001
        && axes[1].dot(axes[2]).abs() < 0.001
        && (pose.determinant() - 1.).abs() < 0.003
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (SceneManager, TrackedController, u32, u32) {
        let mut scene = SceneManager::default();
        let positions = vec![
            Vec3::new(-1., -1., -1.),
            Vec3::new(1., -1., -1.),
            Vec3::new(1., 1., -1.),
            Vec3::new(-1., 1., -1.),
            Vec3::new(-1., -1., 1.),
            Vec3::new(1., -1., 1.),
            Vec3::new(1., 1., 1.),
            Vec3::new(-1., 1., 1.),
        ];
        let mesh = crate::cutting::CutMesh::new(
            positions,
            &[
                0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2, 0, 4, 7, 0,
                7, 3, 1, 2, 6, 1, 6, 5,
            ],
            Vec3::ONE,
        )
        .unwrap();
        let target = scene.add_triangle_mesh(mesh).unwrap();
        scene.set_object_scale(target, Vec3::splat(0.2));
        scene.set_object_position(target, Vec3::Y * 2.);
        scene.set_cuttable(target, true, Vec3::X).unwrap();
        scene.set_projectile(target, Vec3::X, 0., 0.);
        let sword = scene.add_cube(Vec3::ZERO);
        scene
            .set_cutter(sword, Some([Vec3::ZERO, -Vec3::Z]))
            .unwrap();
        let mut controller = TrackedController::default();
        controller.bind(&mut scene, Some(sword), true).unwrap();
        (scene, controller, target, sword)
    }
    fn physical_setup() -> (SceneManager, TrackedController, u32, u32) {
        let (mut scene, controller, target, sword) = setup();
        scene.set_object_position(target, Vec3::new(0., 1., -0.5));
        scene.set_projectile(target, Vec3::ZERO, 9.81, 0.2);
        scene
            .set_blade_physics(
                sword,
                Some(crate::blade_physics::BladePhysics {
                    edge: Vec3::X,
                    half_width: 0.0225,
                    half_thickness: 0.004,
                }),
            )
            .unwrap();
        (scene, controller, target, sword)
    }
    fn edge_pose(percentage: f32) -> Mat4 {
        Mat4::from_translation(Vec3::Y + Vec3::X * (percentage * 0.4 - 0.2 - 0.0225))
    }
    #[test]
    fn grip_speed_uses_distance_over_real_time_and_rejects_alternating_jitter() {
        for hz in [72., 90., 120.] {
            for speed in [0.2, 1., 3.6] {
                let mut meter = GripSpeed::default();
                let dt = 1. / hz;
                for i in 0..120 {
                    let from = Vec3::X * (i as f32 * dt * speed);
                    let to = from + Vec3::X * (dt * speed);
                    assert!((meter.sample(from, to, dt) - speed).abs() < 0.001);
                }
            }
            let mut meter = GripSpeed::default();
            let mut previous = Vec3::ZERO;
            for i in 0..120 {
                let position = Vec3::X * if i % 2 == 0 { 0.001 } else { -0.001 };
                let speed = meter.sample(previous, position, 1. / hz);
                if i > 10 {
                    assert!(
                        speed < 0.05,
                        "tracking jitter must not imply a strong attack"
                    );
                }
                previous = position;
            }
        }
    }
    #[test]
    fn xr_input_cut_of_one_body_preserves_other_body_jam_ribbon() {
        let (mut scene, mut controller, first, sword) = physical_setup();
        let mesh = scene.triangle_meshes()[&first].mesh.cpu().unwrap();
        let second_mesh = crate::cutting::CutMesh::new(
            mesh.positions.clone(),
            &mesh
                .triangles
                .iter()
                .flat_map(|t| t.indices)
                .collect::<Vec<_>>(),
            Vec3::ONE,
        )
        .unwrap();
        let second = scene.add_triangle_mesh(second_mesh).unwrap();
        scene.set_object_scale(second, Vec3::splat(0.2));
        scene.set_object_position(second, Vec3::new(0.08, 1., -0.5));
        scene.set_cuttable(second, true, Vec3::X).unwrap();
        scene.set_projectile(second, Vec3::ZERO, 9.81, 0.2);
        controller
            .update(&mut scene, Some(edge_pose(-0.2)), true)
            .unwrap();
        let cut = controller
            .update_timed(&mut scene, Some(edge_pose(0.8)), true, 1. / 30.)
            .unwrap();
        assert_eq!(cut.removed, vec![first]);
        assert_eq!(scene.blade_cut_state(second).1, Some(sword));
        let cut = controller
            .update(&mut scene, Some(edge_pose(1.1)), true)
            .unwrap();
        assert_eq!(
            cut.removed,
            vec![second],
            "completing one cut must retain the other body's partial cut"
        );
    }

    #[test]
    fn xr_input_physical_slow_progress_jam_hold_continue_and_release() {
        let (mut scene, mut controller, target, sword) = physical_setup();
        controller
            .update(&mut scene, Some(edge_pose(-0.2)), true)
            .unwrap();
        for p in [0.1, 0.3, 0.6] {
            let report = controller
                .update(&mut scene, Some(edge_pose(p)), true)
                .unwrap();
            assert!(report.removed.is_empty());
            let (actual, owner) = scene.blade_cut_state(target);
            assert!((actual - p).abs() < 0.001, "p={p}, actual={actual}");
            assert_eq!(owner, if p >= 0.5 { Some(sword) } else { None });
        }
        let position = scene.objects()[&target].transform.position;
        scene.schedule_remove_object(target, 0.1);
        for _ in 0..300 {
            scene.update(1. / 90.);
            controller
                .update(&mut scene, Some(edge_pose(0.6)), true)
                .unwrap();
        }
        assert!(
            scene.objects()[&target]
                .transform
                .position
                .distance(position)
                < 1e-5
        );
        assert_eq!(scene.blade_cut_state(target).1, Some(sword));
        let report = controller
            .update(&mut scene, Some(edge_pose(0.81)), true)
            .unwrap();
        assert_eq!(report.removed, vec![target]);
        assert_eq!(report.pieces.len(), 2);
        assert_eq!(scene.blade_cut_state(target), (0., None));

        let (mut scene, mut controller, target, sword) = physical_setup();
        controller
            .update(&mut scene, Some(edge_pose(-0.2)), true)
            .unwrap();
        controller
            .update(&mut scene, Some(edge_pose(0.25)), true)
            .unwrap();
        assert!((scene.blade_cut_state(target).0 - 0.25).abs() < 1e-4);
        let before = scene.projectile_contacts;
        controller
            .update(&mut scene, Some(edge_pose(0.25)), true)
            .unwrap();
        assert!(scene.projectile_contacts > before);
        assert_eq!(scene.blade_cut_state(target), (0., None));
        controller.reset(&mut scene);
        scene.set_object_position(target, Vec3::new(0., 1., -0.5));
        scene.set_projectile(target, Vec3::ZERO, 9.81, 0.2);
        controller
            .update(&mut scene, Some(edge_pose(-0.2)), true)
            .unwrap();
        controller
            .update(&mut scene, Some(edge_pose(0.2)), true)
            .unwrap();
        controller
            .update(&mut scene, Some(edge_pose(0.6)), true)
            .unwrap();
        assert_eq!(scene.blade_cut_state(target).1, Some(sword));
        controller.update(&mut scene, None, true).unwrap();
        assert_eq!(scene.blade_cut_state(target).1, None);
        let y = scene.objects()[&target].transform.position.y;
        scene.update(0.1);
        assert!(scene.objects()[&target].transform.position.y < y);
    }

    #[test]
    fn xr_input_physical_stationary_blade_preserves_moving_target_prefix() {
        let (mut scene, mut controller, target, _) = physical_setup();
        let pose = Mat4::from_translation(Vec3::Y);
        scene.set_object_position(target, Vec3::new(0.3, 1., -0.5));
        controller.update(&mut scene, Some(pose), true).unwrap();
        for x in [0.22, 0.18, 0.14, 0.10] {
            scene.set_object_position(target, Vec3::new(x, 1., -0.5));
            assert!(controller
                .update(&mut scene, Some(pose), true)
                .unwrap()
                .removed
                .is_empty());
            assert!(scene.blade_cut_state(target).0 > 0.);
        }
        assert!((scene.blade_cut_state(target).0 - 0.30625).abs() < 1e-4);
        assert_eq!(
            scene.projectile_contacts, 0,
            "moving prefixes must not rebound"
        );
    }

    #[test]
    fn xr_input_physical_jam_tangent_rotation_other_hand_finishes() {
        let (mut scene, mut controller, target, sword) = physical_setup();
        for p in [-0.2, 0.2, 0.6] {
            controller
                .update(&mut scene, Some(edge_pose(p)), true)
                .unwrap();
        }
        let pose = Mat4::from_translation(edge_pose(0.6).w_axis.truncate() + Vec3::Y * 0.04);
        let old = scene.objects()[&target].transform.position;
        controller.update(&mut scene, Some(pose), true).unwrap();
        assert_eq!(scene.blade_cut_state(target).1, Some(sword));
        assert!(
            scene.objects()[&target]
                .transform
                .position
                .distance(old + Vec3::Y * 0.04)
                < 1e-4
        );
        let rotated =
            Mat4::from_rotation_translation(Quat::from_rotation_x(0.06), pose.w_axis.truncate());
        let old_model = scene.objects()[&target].transform.model();
        controller.update(&mut scene, Some(rotated), true).unwrap();
        assert_eq!(scene.blade_cut_state(target).1, Some(sword));
        let expected = rotated * pose.inverse() * old_model;
        assert!(scene.objects()[&target]
            .transform
            .model()
            .abs_diff_eq(expected, 1e-4));
        // A second independent sharp ribbon can complete the shared real mesh.
        let second = scene.add_cube(Vec3::ZERO);
        scene
            .set_cutter(second, Some([Vec3::ZERO, -Vec3::Z]))
            .unwrap();
        scene
            .set_blade_physics(second, scene.blade_physics(sword))
            .unwrap();
        let mut other = TrackedController::default();
        other.bind(&mut scene, Some(second), true).unwrap();
        let target_model = scene.objects()[&target].transform.model();
        let origin = target_model.transform_point3(Vec3::ZERO);
        let rotation = Quat::from_rotation_x(0.06);
        let mut removed = Vec::new();
        for x in [-0.3, -0.14, 0.02, 0.18] {
            let pose = Mat4::from_rotation_translation(
                rotation,
                origin + Vec3::X * x + rotation * Vec3::Z * 0.5,
            );
            removed.extend(other.update(&mut scene, Some(pose), true).unwrap().removed);
        }
        assert_eq!(removed, vec![target]);
        assert_eq!(scene.blade_cut_state(target), (0., None));
    }

    #[test]
    fn xr_input_actual_apple_jam_survives_tracking_jitter_and_wrist_rotation() {
        for hz in [72., 90.] {
            let (mut scene, mut controller, old_target, sword) = physical_setup();
            scene.remove_object(old_target);
            let mesh = apple();
            let (mut lo, mut hi) = (-1.5, 1.5);
            for _ in 0..22 {
                let x = (lo + hi) * 0.5;
                let ribbon = [
                    [Vec3::new(-2., 1.1, -2.), Vec3::new(-2., 1.1, 2.)],
                    [Vec3::new(x, 1.1, -2.), Vec3::new(x, 1.1, 2.)],
                ];
                if mesh
                    .slice_ribbon_progress(&ribbon, Vec3::X, 0.75)
                    .unwrap()
                    .0
                    < 0.6
                {
                    lo = x;
                } else {
                    hi = x;
                }
            }
            let target = scene.add_triangle_mesh(mesh).unwrap();
            let origin = Vec3::new(-0.43, 1.2, -1.2);
            scene.set_object_scale(target, Vec3::splat(0.13));
            scene.set_object_position(target, origin);
            scene.set_cuttable(target, true, Vec3::X).unwrap();
            scene.set_projectile(target, Vec3::ZERO, 9.81, 0.);
            let grip = origin + Vec3::new(hi * 0.13 - 0.0225, 1.1 * 0.13, 0.5);
            for offset in [-0.3, -0.15, 0.] {
                controller
                    .update_timed(
                        &mut scene,
                        Some(Mat4::from_translation(grip + Vec3::X * offset)),
                        true,
                        1. / hz,
                    )
                    .unwrap();
            }
            let (progress, owner) = scene.blade_cut_state(target);
            assert!((progress - 0.6).abs() < 1e-4, "{progress}");
            assert_eq!(owner, Some(sword));
            let mut last = Mat4::IDENTITY;
            for frame in 1..=(hz as usize * 2) {
                let t = frame as f32 / hz;
                let jitter = Vec3::new((t * 73.).sin(), (t * 51.).sin(), (t * 67.).sin()) * 0.0007;
                let rotation = Quat::from_rotation_x((t * 3.).sin() * 0.015)
                    * Quat::from_rotation_z((t * 5.).sin() * 0.008);
                last = Mat4::from_rotation_translation(
                    rotation,
                    grip + jitter + Vec3::Y * (0.015 * t),
                );
                scene.update(1. / hz);
                let report = controller
                    .update_timed(&mut scene, Some(last), true, 1. / hz)
                    .unwrap();
                assert!(
                    report.removed.is_empty(),
                    "jitter completed fruit at {hz}Hz frame {frame}"
                );
                assert_eq!(
                    scene.blade_cut_state(target).1,
                    Some(sword),
                    "lost joint at {hz}Hz frame {frame}"
                );
            }
            assert!(scene.objects()[&target].transform.position.y > origin.y + 0.02);
            let mut removed = Vec::new();
            for i in 1..=12 {
                let pose = Mat4::from_rotation_translation(
                    Quat::from_mat4(&last),
                    last.w_axis.truncate() + last.transform_vector3(Vec3::X) * (i as f32 * 0.015),
                );
                removed.extend(
                    controller
                        .update_timed(&mut scene, Some(pose), true, 1. / hz)
                        .unwrap()
                        .removed,
                );
            }
            assert_eq!(removed, vec![target], "continued sharp cut at {hz}Hz");
        }
    }

    #[test]
    fn xr_input_physical_slow_pullback_releases_joint() {
        let (mut scene, mut controller, target, sword) = physical_setup();
        for p in [-0.2, 0.2, 0.6] {
            controller
                .update(&mut scene, Some(edge_pose(p)), true)
                .unwrap();
        }
        assert_eq!(scene.blade_cut_state(target).1, Some(sword));
        for step in 1..=40 {
            controller
                .update(
                    &mut scene,
                    Some(edge_pose(0.6 - step as f32 * 0.00125)),
                    true,
                )
                .unwrap();
            assert_eq!(
                scene.blade_cut_state(target).1,
                if step < 40 { Some(sword) } else { None },
                "withdrawal step {step}"
            );
        }
    }

    #[test]
    fn xr_input_actual_apple_area_progress_finite_union_and_closed_volume() {
        let mesh = apple();
        let ribbon = |x: f32| {
            [
                [Vec3::new(-2., -1., 0.), Vec3::new(-2., 3., 0.)],
                [Vec3::new(x, -1., 0.), Vec3::new(x, 3., 0.)],
            ]
        };
        for expected in [0.25, 0.5, 0.6, 0.74, 0.75, 0.9] {
            let (mut lo, mut hi) = (-1.5, 1.5);
            for _ in 0..22 {
                let mid = (lo + hi) * 0.5;
                let p = mesh
                    .slice_ribbon_progress(&ribbon(mid), Vec3::X, 0.75)
                    .unwrap()
                    .0;
                if p < expected {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let samples = ribbon(hi);
            let (actual, cut) = mesh
                .slice_ribbon_progress(
                    &[samples[0], samples[1], samples[0], samples[1]],
                    Vec3::X,
                    0.75,
                )
                .unwrap();
            assert!(
                (actual - expected).abs() < 2e-5,
                "expected={expected} actual={actual}"
            );
            assert_eq!(cut.is_some(), expected >= 0.75);
            assert!(mesh.slice_ribbon(&samples, Vec3::X).unwrap().is_none());
            if let Some(cut) = cut {
                assert!(
                    (cut.pieces
                        .iter()
                        .map(crate::cutting::CutMesh::volume)
                        .sum::<f64>()
                        - mesh.volume())
                    .abs()
                        < 1e-5
                );
                for piece in cut.pieces {
                    piece.validate().unwrap();
                }
            }
        }
        let miss = [
            [Vec3::new(-2., 2.3, 0.), Vec3::new(-2., 3., 0.)],
            [Vec3::new(2., 2.3, 0.), Vec3::new(2., 3., 0.)],
        ];
        assert_eq!(
            mesh.slice_ribbon_progress(&miss, Vec3::X, 0.75).unwrap().0,
            0.
        );
        let grazing = miss.map(|pair| pair.map(|p| p - Vec3::Y * 0.2));
        let (area, cut) = mesh.slice_ribbon_progress(&grazing, Vec3::X, 0.75).unwrap();
        assert!(area < 0.05);
        assert!(cut.is_none());
    }

    #[test]
    fn xr_input_cut_assist_short_backhand_is_real_closed_cut_without_contacts() {
        for (assisted, contacts) in [(false, false), (true, false), (true, true)] {
            for direction in [-1., 1.] {
                let (mut scene, mut controller, target, sword) = physical_setup();
                scene.set_projectile(target, Vec3::ZERO, 0., 0.);
                if assisted {
                    scene.set_cut_assist(sword, 0.35).unwrap();
                }
                scene.set_blade_contact_response(sword, contacts).unwrap();
                let volume = scene.triangle_meshes()[&target]
                    .mesh
                    .cpu()
                    .unwrap()
                    .volume();
                let mut pieces = vec![];
                for frame in 0..=5 {
                    // Only half of the actual 40cm cross-section is swept.
                    let x = direction * (-0.22 + frame as f32 * 0.04);
                    let pose = Mat4::from_translation(Vec3::new(x, 1., 0.));
                    pieces.extend(
                        controller
                            .update_timed(&mut scene, Some(pose), true, 1. / 90.)
                            .unwrap()
                            .pieces,
                    );
                }
                assert_eq!(
                    !scene.object_exists(target),
                    assisted,
                    "assist={assisted}, direction={direction}"
                );
                if assisted {
                    assert_eq!(scene.projectile_contacts, 0);
                    assert_eq!(scene.blade_cut_state(target).1, None);
                    assert_eq!(pieces.len(), 2);
                    let total: f64 = pieces
                        .iter()
                        .map(|id| {
                            let mesh = scene.triangle_meshes()[id].mesh.cpu().unwrap();
                            mesh.validate().unwrap();
                            mesh.volume()
                        })
                        .sum();
                    assert!((total - volume).abs() < 1e-5);
                    let moving = scene.play_cut_feedback(&pieces).unwrap();
                    let moving_radius = scene.objects()[&moving[0]].transform.scale.x;
                    scene.set_cut_feedback_speed(&pieces, 0.);
                    let base = scene.play_cut_feedback(&pieces).unwrap();
                    let base_radius = scene.objects()[&base[0]].transform.scale.x;
                    assert!(
                        (moving_radius / base_radius - 16.666667).abs() < 1e-4,
                        "actual 3.6m/s tracked stroke must visibly exceed a stationary hit"
                    );
                }
            }
        }
    }
    #[test]
    fn xr_input_assisted_contacts_flat_strike_bounces_without_cut_or_jam() {
        let (mut scene, mut controller, target, sword) = physical_setup();
        scene.set_cut_assist(sword, 0.35).unwrap();
        scene.set_blade_contact_response(sword, true).unwrap();
        scene.set_projectile(target, Vec3::ZERO, 9.81, 0.5);
        for y in [0.65, 0.95] {
            let report = controller
                .update_timed(
                    &mut scene,
                    Some(Mat4::from_translation(Vec3::Y * y)),
                    true,
                    0.025,
                )
                .unwrap();
            assert!(report.pieces.is_empty());
        }
        assert!(scene.object_exists(target));
        assert_eq!(scene.blade_cut_state(target), (0., None));
        assert!(scene.projectile_contacts > 0);
        let velocity = scene.projectile_velocity(target).unwrap();
        assert!(
            velocity.y > 0. && velocity.length() <= 8.001,
            "{velocity:?}"
        );
    }

    #[test]
    fn xr_input_assisted_contacts_moving_edge_bounces_noncuttable_debris() {
        for direction in [-1., 1.] {
            let (mut scene, mut controller, target, sword) = physical_setup();
            scene.set_cuttable(target, false, Vec3::X).unwrap();
            scene.set_cut_assist(sword, 0.35).unwrap();
            scene.set_blade_contact_response(sword, true).unwrap();
            scene.set_projectile(target, Vec3::ZERO, 9.81, 0.5);
            for x in [-0.4 * direction, 0.] {
                let pose = Mat4::from_translation(Vec3::new(x, 1., 0.));
                let report = controller
                    .update_timed(&mut scene, Some(pose), true, 0.03)
                    .unwrap();
                assert!(report.pieces.is_empty());
            }
            assert!(scene.object_exists(target));
            assert_eq!(scene.blade_cut_state(target), (0., None));
            assert!(scene.projectile_contacts > 0);
            assert!(scene.objects()[&target].transform.position.x * direction > 0.1);
            let velocity = scene.projectile_velocity(target).unwrap();
            assert!(
                velocity.x * direction > 0. && velocity.length() <= 8.001,
                "{velocity:?}"
            );
        }
    }

    #[test]
    fn xr_input_assisted_contacts_stationary_blade_bounces_incoming_flat_and_edge() {
        for (direction, cuttable) in [(Vec3::X, true), (Vec3::Y, true), (Vec3::Y, false)] {
            let (mut scene, mut controller, target, sword) = physical_setup();
            scene.set_cuttable(target, cuttable, Vec3::X).unwrap();
            scene.set_cut_assist(sword, 0.35).unwrap();
            scene.set_blade_contact_response(sword, true).unwrap();
            scene.set_projectile(target, -direction * 2., 0., 0.5);
            let origin = Vec3::new(0., 1., -0.5);
            scene.set_object_position(target, origin + direction * 0.3);
            let pose = Mat4::from_translation(Vec3::Y);
            controller
                .update_timed(&mut scene, Some(pose), true, 0.025)
                .unwrap();
            scene.set_object_position(target, origin + direction * 0.1);
            let report = controller
                .update_timed(&mut scene, Some(pose), true, 0.025)
                .unwrap();
            assert!(report.pieces.is_empty());
            assert!(scene.object_exists(target));
            assert_eq!(scene.blade_cut_state(target), (0., None));
            assert!(scene.projectile_contacts > 0);
            assert!(scene.projectile_velocity(target).unwrap().dot(direction) > 0.);
        }
    }

    #[test]
    fn xr_input_assisted_contacts_stationary_support_settles_and_releases() {
        let (mut scene, mut controller, target, sword) = physical_setup();
        scene.set_cut_assist(sword, 0.35).unwrap();
        scene.set_blade_contact_response(sword, true).unwrap();
        scene.set_projectile(target, Vec3::ZERO, 9.81, 0.5);
        scene.set_object_position(target, Vec3::new(0., 1.4, -0.5));
        let pose = Mat4::from_translation(Vec3::Y);
        for _ in 0..300 {
            scene.update(1. / 90.);
            let report = controller
                .update_timed(&mut scene, Some(pose), true, 1. / 90.)
                .unwrap();
            assert!(report.pieces.is_empty());
        }
        let y = scene.objects()[&target].transform.position.y;
        assert!((y - 1.204).abs() < 0.01, "support height: {y}");
        assert!(scene.projectile_velocity(target).unwrap().length() < 0.1);
        assert!(scene.projectile_contacts > 100);
        controller
            .update_timed(&mut scene, None, true, 1. / 90.)
            .unwrap();
        for _ in 0..20 {
            scene.update(1. / 90.);
        }
        assert!(scene.objects()[&target].transform.position.y < y - 0.2);
    }

    #[test]
    fn xr_input_assisted_contacts_tracking_guards_and_miss_have_no_impulse() {
        let (mut scene, mut controller, target, sword) = physical_setup();
        assert!(!scene.blade_contact_response(sword));
        scene.set_cut_assist(sword, 0.35).unwrap();
        scene.set_blade_contact_response(sword, true).unwrap();
        let velocity = Vec3::X;
        scene.set_projectile(target, velocity, 0., 0.5);
        for pose in [
            Some(Vec3::Y * 0.7),
            None,
            Some(Vec3::Y),
            Some(Vec3::Y * 3.),
            Some(Vec3::Y * 3.1),
        ] {
            let report = controller
                .update_timed(&mut scene, pose.map(Mat4::from_translation), true, 0.025)
                .unwrap();
            assert!(report.pieces.is_empty());
            assert_eq!(scene.projectile_velocity(target), Some(velocity));
            assert_eq!(scene.projectile_contacts, 0);
        }
        let unconfigured = scene.add_cube(Vec3::ZERO);
        assert!(scene
            .set_blade_contact_response(unconfigured, true)
            .is_err());
    }

    #[test]
    fn xr_input_cut_assist_tiny_positive_threshold_still_requires_real_intersection() {
        let (mut scene, mut controller, target, sword) = physical_setup();
        scene.set_projectile(target, Vec3::ZERO, 0., 0.);
        scene.set_cut_assist(sword, 1e-6).unwrap();
        // The blade crosses the target's infinite cutting plane but its finite
        // tip stays 1cm beyond the actual closed mesh.
        for frame in 0..=12 {
            let pose = Mat4::from_translation(Vec3::new(-0.24 + frame as f32 * 0.04, 1., 0.71));
            assert!(controller
                .update_timed(&mut scene, Some(pose), true, 1. / 90.)
                .unwrap()
                .removed
                .is_empty());
        }
        assert!(scene.object_exists(target));
        assert_eq!(scene.projectile_contacts, 0);
    }
    #[test]
    fn xr_input_cut_assist_rejects_miss_stationary_and_reacquisition() {
        let (mut scene, mut controller, target, sword) = physical_setup();
        scene.set_projectile(target, Vec3::ZERO, 0., 0.);
        for coverage in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert!(scene.set_cut_assist(sword, coverage).is_err());
        }
        assert_eq!(scene.cut_assist(sword), 0.);
        scene.set_cut_assist(sword, 0.35).unwrap();
        for x in [
            -0.22, -0.18, -0.14, -0.1, -0.06, -0.02, 0.02, 0.06, 0.1, 0.14, 0.18, 0.22,
        ] {
            let pose = Mat4::from_translation(Vec3::new(x, 2., 0.));
            assert!(controller
                .update_timed(&mut scene, Some(pose), true, 1. / 90.)
                .unwrap()
                .removed
                .is_empty());
        }
        controller
            .update_timed(&mut scene, None, true, 1. / 90.)
            .unwrap();
        let pose = Mat4::from_translation(Vec3::Y);
        for _ in 0..4 {
            assert!(controller
                .update_timed(&mut scene, Some(pose), true, 1. / 90.)
                .unwrap()
                .removed
                .is_empty());
        }
        assert!(scene.object_exists(target));
        assert_eq!(scene.projectile_contacts, 0);
        scene.set_cut_assist(sword, 0.).unwrap();
        assert_eq!(scene.cut_assist(sword), 0.);
    }
    #[test]
    fn xr_input_sharp_edge_cuts_but_flat_and_back_same_path_do_not() {
        for (edge, should_cut) in [(Vec3::X, true), (Vec3::Y, false), (-Vec3::X, false)] {
            let (mut scene, mut controller, target, sword) = setup();
            scene.set_object_position(target, Vec3::new(0., 0., -0.5));
            scene.set_projectile(target, Vec3::ZERO, 0., 0.);
            scene
                .set_blade_physics(
                    sword,
                    Some(crate::blade_physics::BladePhysics {
                        edge,
                        half_width: 0.0225,
                        half_thickness: 0.004,
                    }),
                )
                .unwrap();
            let mut removed = Vec::new();
            for frame in 0..=24 {
                let pose = Mat4::from_translation(Vec3::new(-0.48 + frame as f32 * 0.04, 0., 0.));
                removed.extend(
                    controller
                        .update_timed(&mut scene, Some(pose), true, 1. / 90.)
                        .unwrap()
                        .removed,
                );
            }
            assert_eq!(removed.contains(&target), should_cut, "edge={edge:?}");
        }
    }
    #[test]
    fn xr_input_flat_support_preserves_scheduled_fruit_past_three_seconds_and_loss_releases() {
        let (mut scene, mut controller, old_target, sword) = setup();
        scene.remove_object(old_target);
        let target = scene.add_triangle_mesh(apple()).unwrap();
        scene.set_object_scale(target, Vec3::splat(0.13));
        scene.set_cuttable(target, true, Vec3::X).unwrap();
        scene.set_object_position(target, Vec3::new(0., 1.4, -0.5));
        scene.set_projectile(target, Vec3::ZERO, 9.81, 0.);
        scene.schedule_remove_object(target, 3.);
        scene
            .set_blade_physics(
                sword,
                Some(crate::blade_physics::BladePhysics {
                    edge: Vec3::X,
                    half_width: 0.0225,
                    half_thickness: 0.004,
                }),
            )
            .unwrap();
        for _ in 0..450 {
            scene.update(1. / 90.);
            controller
                .update_timed(
                    &mut scene,
                    Some(Mat4::from_translation(Vec3::Y)),
                    true,
                    1. / 90.,
                )
                .unwrap();
        }
        assert!(scene.object_exists(target));
        let y = scene.objects()[&target].transform.position.y;
        let bottom = scene.triangle_meshes()[&target]
            .mesh
            .bottom(scene.objects()[&target].transform)
            .unwrap();
        let gap = bottom - 1.004;
        assert!((0.0..0.02).contains(&gap), "support gap={gap}, y={y}");
        controller
            .update_timed(&mut scene, None, true, 1. / 90.)
            .unwrap();
        for _ in 0..45 {
            scene.update(1. / 90.);
        }
        assert!(scene.objects()[&target].transform.position.y < y - 0.5);
        for _ in 0..240 {
            scene.update(1. / 90.);
        }
        assert!(!scene.object_exists(target));
    }
    #[test]
    fn xr_input_actual_apple_flat_carries_across_width_and_along_axis_without_cut() {
        for carry_direction in [Vec3::X, Vec3::Z] {
            let mut scene = SceneManager::default();
            let target = scene.add_triangle_mesh(apple()).unwrap();
            scene.set_object_scale(target, Vec3::splat(0.13));
            scene.set_object_position(target, Vec3::new(0., 1.35, -1.1));
            scene.set_cuttable(target, true, Vec3::X).unwrap();
            scene.set_projectile(target, Vec3::ZERO, 8., 0.);
            scene.schedule_remove_object(target, 3.);
            let sword = scene.add_cube(Vec3::ZERO);
            scene
                .set_cutter(sword, Some([-Vec3::Z * 0.1, -Vec3::Z * 0.95]))
                .unwrap();
            scene
                .set_blade_physics(
                    sword,
                    Some(crate::blade_physics::BladePhysics {
                        edge: -Vec3::Y,
                        half_width: 0.0225,
                        half_thickness: 0.004,
                    }),
                )
                .unwrap();
            let mut controller = TrackedController::default();
            controller.bind(&mut scene, Some(sword), true).unwrap();
            for frame in 0..450 {
                scene.update(1. / 90.);
                let offset = if frame < 270 {
                    0.
                } else {
                    (frame - 270) as f32 / 180. * 0.06
                };
                let pose = Mat4::from_rotation_translation(
                    Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                    Vec3::new(0., 1., -0.45) + carry_direction * offset,
                );
                let report = controller
                    .update_timed(&mut scene, Some(pose), true, 1. / 90.)
                    .unwrap();
                assert!(
                    report.removed.is_empty(),
                    "carried fruit cut frame={frame}, direction={carry_direction:?}"
                );
            }
            assert!(scene.object_exists(target));
            let position = scene.objects()[&target].transform.position;
            let moved = (position - Vec3::new(0., 1.35, -1.1)).dot(carry_direction);
            assert!(
                moved > 0.025,
                "direction={carry_direction:?}, carried={moved}, position={position:?}"
            );
            let bottom = scene.triangle_meshes()[&target]
                .mesh
                .bottom(scene.objects()[&target].transform)
                .unwrap();
            assert!((1.0..1.025).contains(&bottom), "bottom={bottom}");
        }
    }
    #[test]
    fn xr_input_apple_support_proxy_refreshes_after_scale_change() {
        let (mut scene, mut controller, old_target, sword) = setup();
        scene.remove_object(old_target);
        let target = scene.add_triangle_mesh(apple()).unwrap();
        scene.set_cuttable(target, true, Vec3::X).unwrap();
        scene.set_object_position(target, Vec3::new(0., 1.4, -0.5));
        scene.set_projectile(target, Vec3::ZERO, 9.81, 0.);
        scene
            .set_blade_physics(
                sword,
                Some(crate::blade_physics::BladePhysics {
                    edge: Vec3::X,
                    half_width: 0.0225,
                    half_thickness: 0.004,
                }),
            )
            .unwrap();
        for scale in [0.13, 0.065, 0.16] {
            scene.set_object_scale(target, Vec3::splat(scale));
            for _ in 0..90 {
                scene.update(1. / 90.);
                controller
                    .update_timed(
                        &mut scene,
                        Some(Mat4::from_translation(Vec3::Y)),
                        true,
                        1. / 90.,
                    )
                    .unwrap();
            }
            let bottom = scene.triangle_meshes()[&target]
                .mesh
                .bottom(scene.objects()[&target].transform)
                .unwrap();
            assert!(
                (0.0..0.02).contains(&(bottom - 1.004)),
                "scale={scale}, bottom={bottom}"
            );
        }
    }
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
    fn xr_input_wrist_arc_splits_real_moving_spinning_apple_and_finite_miss() {
        for (moving, offset, reset, depth) in [
            (false, 0., false, 0.),
            (true, 0., false, 0.),
            (true, 0.5, false, 0.),
            (true, 0., true, 0.),
            (false, 0., false, -0.6),
        ] {
            let mut scene = SceneManager::default();
            let target = scene.add_triangle_mesh(apple()).unwrap();
            let volume = scene.triangle_meshes()[&target].mesh.volume();
            scene.set_object_scale(target, Vec3::splat(0.13));
            scene.set_object_position(target, Vec3::new(-0.65, 0.13, -1.2));
            scene.set_cuttable(target, true, Vec3::ONE).unwrap();
            if moving {
                scene.set_projectile(target, Vec3::new(0.4, 4.6, 0.), 8., 0.);
                scene.set_object_spin(target, Vec3::new(0.5, 1.2, 0.3));
                for _ in 0..42 {
                    scene.update(0.01);
                }
            } else {
                scene.set_object_position(target, Vec3::new(-0.43, 1.45, -1.2 + depth));
            }
            let sword = scene.add_cube(Vec3::ZERO);
            scene
                .set_cutter(sword, Some([-Vec3::Z * 0.1, -Vec3::Z * 0.95]))
                .unwrap();
            let mut c = TrackedController::default();
            c.bind(&mut scene, Some(sword), true).unwrap();
            let mut pieces = vec![];
            for i in 0..=24 {
                if moving && i > 0 {
                    scene.update(1. / 72.);
                }
                if reset && i > 0 {
                    c.reset(&mut scene);
                }
                let angle = (35. - 70. * i as f32 / 24.).to_radians();
                let grip = Mat4::from_rotation_translation(
                    Quat::from_rotation_x(angle),
                    Vec3::new(-0.43 + offset, 1.35, -0.45),
                );
                pieces.extend(c.update(&mut scene, Some(grip), true).unwrap().pieces);
            }
            if offset > 0. || reset || depth != 0. {
                assert!(pieces.is_empty() && scene.object_exists(target));
            } else {
                assert_eq!(pieces.len(), 2, "moving={moving}");
                let after: f64 = pieces
                    .iter()
                    .map(|id| scene.triangle_meshes()[id].mesh.volume())
                    .sum();
                assert!((after - volume).abs() < volume * 1e-4);
                for id in pieces {
                    assert!(scene.triangle_meshes()[&id].cuttable);
                }
            }
        }
    }

    #[test]
    fn xr_input_opt_in_fast_apple_profile_regression() {
        for hz in [72., 90., 120., 36.] {
            for miss in [false, true] {
                let mut scene = SceneManager::default();
                let target = scene.add_triangle_mesh(apple()).unwrap();
                scene.set_object_scale(target, Vec3::splat(0.13));
                scene.set_object_position(target, Vec3::new(0., 1.2, -1.1));
                scene.set_cuttable(target, true, Vec3::ONE).unwrap();
                scene.set_projectile(target, Vec3::new(0.2, 0.8, 0.), 0., 0.);
                scene.set_object_spin(target, Vec3::new(0.5, 1.2, 0.3));
                let sword = scene.add_cube(Vec3::ZERO);
                scene
                    .set_cutter(sword, Some([-Vec3::Z * 0.1, -Vec3::Z * 0.95]))
                    .unwrap();
                scene
                    .set_blade_physics(
                        sword,
                        Some(crate::blade_physics::BladePhysics {
                            edge: -Vec3::Y,
                            half_width: 0.0225,
                            half_thickness: 0.004,
                        }),
                    )
                    .unwrap();
                let mut c = TrackedController::default();
                c.bind(&mut scene, Some(sword), true).unwrap();
                let steps = ((std::f32::consts::PI / 3.) / (40. / hz)).ceil() as usize;
                let mut pieces = Vec::new();
                for i in 0..=steps {
                    scene.update(1. / hz);
                    let angle = std::f32::consts::FRAC_PI_6 * (1. - 2. * i as f32 / steps as f32);
                    let pose = Mat4::from_rotation_translation(
                        Quat::from_rotation_x(angle),
                        Vec3::new(if miss { 0.6 } else { 0. }, 1.35, -0.45),
                    );
                    let r = c
                        .update_timed(&mut scene, Some(pose), true, 1. / hz)
                        .unwrap();
                    pieces.extend(r.pieces);
                }
                assert_eq!(
                    pieces.len(),
                    if miss { 0 } else { 2 },
                    "hz={hz} miss={miss}"
                );
            }
        }
    }
    #[test]
    fn xr_input_fast_real_apple_arcs_and_translation_at_quest_rates() {
        for hz in [72., 90., 120., 36.] {
            for rotation in [false, true] {
                for miss in [false, true] {
                    for moving in [false, true] {
                        let mut scene = SceneManager::default();
                        let target = scene.add_triangle_mesh(apple()).unwrap();
                        scene.set_object_scale(target, Vec3::splat(0.13));
                        scene.set_object_position(target, Vec3::new(0., 1.35, -1.2));
                        scene.set_cuttable(target, true, Vec3::ONE).unwrap();
                        if moving {
                            scene.set_projectile(target, Vec3::new(0., 0.6, 0.), 0., 0.);
                            scene.set_object_spin(target, Vec3::new(0.5, 1.2, 0.3));
                        }
                        let sword = scene.add_cube(Vec3::ZERO);
                        scene
                            .set_cutter(sword, Some([-Vec3::Z * 0.1, -Vec3::Z * 0.95]))
                            .unwrap();
                        let mut c = TrackedController::default();
                        c.bind(&mut scene, Some(sword), true).unwrap();
                        let steps = if rotation {
                            ((std::f32::consts::PI / 3.) / (40. / hz)).ceil() as usize
                        } else {
                            (0.7f32 / (12. / hz)).ceil() as usize
                        };
                        let mut pieces = vec![];
                        for i in 0..=steps {
                            if moving && i > 0 {
                                scene.update(1. / hz);
                            }
                            let t = i as f32 / steps as f32;
                            let angle = if rotation {
                                (30. - 60. * t).to_radians()
                            } else {
                                0.
                            };
                            let y = if rotation { 1.35 } else { 1.1 + 0.7 * t };
                            let grip = Mat4::from_rotation_translation(
                                Quat::from_rotation_x(angle),
                                Vec3::new(if miss { 0.6 } else { 0. }, y, -0.45),
                            );
                            pieces.extend(
                                c.update_timed(&mut scene, Some(grip), true, 1. / hz)
                                    .unwrap()
                                    .pieces,
                            );
                        }
                        assert_eq!(
                            pieces.len(),
                            if miss { 0 } else { 2 },
                            "hz={hz} rotation={rotation} miss={miss} moving={moving}"
                        );
                        assert_eq!(scene.object_exists(target), miss);
                    }
                }
            }
        }
    }

    #[test]
    fn xr_input_sparse_arc_covers_curved_tip_path_without_extending_reach() {
        // Only two tracking samples: their straight endpoint chord ends at
        // radius 0.81m, while the measured rigid rotation sweeps radius 0.95m.
        for radius in [0.92, 1.02] {
            let mut scene = SceneManager::default();
            let target = scene.add_triangle_mesh(apple()).unwrap();
            scene.set_object_scale(target, Vec3::splat(0.02));
            scene.set_object_position(target, Vec3::new(0., 1.35 - 0.023, -0.45 - radius));
            scene.set_cuttable(target, true, Vec3::ONE).unwrap();
            let sword = scene.add_cube(Vec3::ZERO);
            scene
                .set_cutter(sword, Some([-Vec3::Z * 0.1, -Vec3::Z * 0.95]))
                .unwrap();
            let mut c = TrackedController::default();
            c.bind(&mut scene, Some(sword), true).unwrap();
            let mut pieces = vec![];
            for angle in [0.55, -0.55] {
                let grip = Mat4::from_rotation_translation(
                    Quat::from_rotation_x(angle),
                    Vec3::new(0., 1.35, -0.45),
                );
                pieces.extend(
                    c.update_timed(&mut scene, Some(grip), true, 1. / 36.)
                        .unwrap()
                        .pieces,
                );
            }
            assert_eq!(
                pieces.len(),
                if radius < 0.95 { 2 } else { 0 },
                "radius={radius}"
            );
            assert_eq!(scene.object_exists(target), radius > 0.95);
        }
    }

    #[test]
    fn xr_input_fast_long_arc_preserves_complete_and_partial_hits() {
        for y in [1.2, 1.35] {
            let mut scene = SceneManager::default();
            let target = scene.add_triangle_mesh(apple()).unwrap();
            scene.set_object_scale(target, Vec3::splat(0.13));
            scene.set_object_position(target, Vec3::new(0., y, -1.2));
            scene.set_cuttable(target, true, Vec3::ONE).unwrap();
            let sword = scene.add_cube(Vec3::ZERO);
            scene
                .set_cutter(sword, Some([-Vec3::Z * 0.1, -Vec3::Z * 0.95]))
                .unwrap();
            let mut c = TrackedController::default();
            c.bind(&mut scene, Some(sword), true).unwrap();
            let mut pieces = vec![];
            for angle in [-1.2, -0.089, 1.022] {
                let grip = Mat4::from_rotation_translation(
                    Quat::from_rotation_x(angle),
                    Vec3::new(0., 1.35, -0.45),
                );
                pieces.extend(
                    c.update_timed(&mut scene, Some(grip), true, 1. / 36.)
                        .unwrap()
                        .pieces,
                );
            }
            assert_eq!(pieces.len(), 2, "apple_y={y}");
            assert!(!scene.object_exists(target));
        }
    }

    #[test]
    fn xr_input_timed_motion_guards_and_history_remain_bounded() {
        for dt in [0., f32::NAN, 0.08, 1. / 120.] {
            let (mut scene, mut c, target, _) = setup();
            c.update_timed(&mut scene, Some(pose(-0.3)), true, 1. / 90.)
                .unwrap();
            assert!(
                c.update_timed(&mut scene, Some(pose(0.3)), true, dt)
                    .unwrap()
                    .removed
                    .is_empty(),
                "dt={dt}"
            );
            assert!(scene.object_exists(target));
        }
        // Plausible elapsed time cannot authorize hard jumps or long gaps.
        for (position, angle, dt) in [(2., 0., 0.05), (0., 1.6, 0.05), (0., 1., 1. / 120.)] {
            let (mut scene, mut c, target, _) = setup();
            c.update_timed(&mut scene, Some(pose(-0.3)), true, 1. / 90.)
                .unwrap();
            let next = Mat4::from_rotation_translation(
                Quat::from_rotation_x(angle),
                Vec3::new(position, 2.3, 0.5),
            );
            assert!(c
                .update_timed(&mut scene, Some(next), true, dt)
                .unwrap()
                .removed
                .is_empty());
            assert!(scene.object_exists(target));
        }
        let (scene, mut c, target, _) = setup();
        for i in 0..2000 {
            let pose = pose(i as f32 * 0.001);
            c.record(
                &scene,
                Sample {
                    pose,
                    orientation: Quat::IDENTITY,
                    endpoints: [
                        pose.transform_point3(Vec3::ZERO),
                        pose.transform_point3(-Vec3::Z),
                    ],
                },
            );
        }
        assert!(c.ribbons[&target].len() <= MAX_RIBBON_SAMPLES);
        let length = c.ribbons[&target].len();
        let pose = pose(1.999);
        for _ in 0..2000 {
            c.record(
                &scene,
                Sample {
                    pose,
                    orientation: Quat::IDENTITY,
                    endpoints: [
                        pose.transform_point3(Vec3::ZERO),
                        pose.transform_point3(-Vec3::Z),
                    ],
                },
            );
        }
        assert_eq!(c.ribbons[&target].len(), length);
    }

    #[test]
    fn xr_input_subsamples_compensate_simultaneous_target_translation() {
        for hz in [72., 90., 120., 36.] {
            let mut scene = SceneManager::default();
            let target = scene.add_triangle_mesh(apple()).unwrap();
            scene.set_object_scale(target, Vec3::splat(0.13));
            scene.set_cuttable(target, true, Vec3::ONE).unwrap();
            let sword = scene.add_cube(Vec3::ZERO);
            scene
                .set_cutter(sword, Some([-Vec3::Z * 0.1, -Vec3::Z * 0.95]))
                .unwrap();
            let mut c = TrackedController::default();
            c.bind(&mut scene, Some(sword), true).unwrap();
            for i in 0..=4 {
                let y = 1.35 - i as f32 * 12. / hz;
                scene.set_object_position(target, Vec3::new(0., y, -1.2));
                let grip = Mat4::from_translation(Vec3::new(0., y + 0.12, -0.45));
                assert!(
                    c.update_timed(&mut scene, Some(grip), true, 1. / hz)
                        .unwrap()
                        .removed
                        .is_empty(),
                    "hz={hz} frame={i}"
                );
            }
            assert!(scene.object_exists(target));
        }
    }

    fn pose(y: f32) -> Mat4 {
        Mat4::from_translation(Vec3::new(0., y + 2., 0.5))
    }

    #[test]
    fn xr_input_symmetric_blade_midpoint_rotation_splits_and_tip_miss_stays_safe() {
        for depth in [-0.25, -0.6] {
            let (mut scene, mut controller, target, sword) = setup();
            scene.set_object_scale(target, Vec3::splat(0.05));
            scene.set_object_position(target, Vec3::new(0., 2., depth));
            scene
                .set_cutter(sword, Some([-Vec3::Z * 0.4, Vec3::Z * 0.4]))
                .unwrap();
            controller.bind(&mut scene, Some(sword), true).unwrap();
            let mut pieces = vec![];
            for i in 0..=20 {
                let grip = Mat4::from_rotation_translation(
                    Quat::from_rotation_x(-0.5 + i as f32 * 0.05),
                    Vec3::Y * 2.,
                );
                pieces.extend(
                    controller
                        .update(&mut scene, Some(grip), true)
                        .unwrap()
                        .pieces,
                );
            }
            if depth == -0.25 {
                assert_eq!(pieces.len(), 2);
                let volume: f64 = pieces
                    .iter()
                    .map(|id| scene.triangle_meshes()[id].mesh.volume())
                    .sum();
                assert!((volume - 8.).abs() < 1e-5);
                assert!(!scene.object_exists(target));
            } else {
                assert!(pieces.is_empty() && scene.object_exists(target));
            }
        }
    }

    #[test]
    fn xr_input_diagonal_ribbons_cover_full_cross_section_in_both_directions() {
        for direction in [-1., 1.] {
            for slope in [-1., 1.] {
                let (mut scene, mut controller, target, _) = setup();
                let mut pieces = vec![];
                for i in 0..=24 {
                    let travel = direction * (-0.3 + i as f32 * 0.025);
                    let grip = Mat4::from_translation(Vec3::new(travel * slope, 2. + travel, 0.5));
                    pieces.extend(
                        controller
                            .update(&mut scene, Some(grip), true)
                            .unwrap()
                            .pieces,
                    );
                }
                assert_eq!(pieces.len(), 2, "direction={direction} slope={slope}");
                assert!(!scene.object_exists(target));
                let volume: f64 = pieces
                    .iter()
                    .map(|id| scene.triangle_meshes()[id].mesh.volume())
                    .sum();
                assert!((volume - 8.).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn xr_input_slight_yaw_during_vertical_stroke_uses_swept_surface_plane() {
        for yaw in [0.03, 0.8] {
            for depth in [0., -0.8] {
                let (mut scene, mut controller, target, _) = setup();
                scene.set_object_position(target, Vec3::new(0., 2., depth));
                let mut pieces = vec![];
                for i in 0..=24 {
                    let grip = Mat4::from_rotation_translation(
                        Quat::from_rotation_y((i as f32 / 24. - 0.5) * yaw),
                        Vec3::new(0., 1.7 + i as f32 * 0.025, 0.5),
                    );
                    pieces.extend(
                        controller
                            .update(&mut scene, Some(grip), true)
                            .unwrap()
                            .pieces,
                    );
                }
                if depth == 0. && yaw == 0.03 {
                    assert_eq!(pieces.len(), 2);
                    assert!(!scene.object_exists(target));
                } else {
                    // Neither distant targets nor a heavily twisted surface
                    // may turn into an unbounded inferred cutting plane.
                    assert!(pieces.is_empty() && scene.object_exists(target));
                }
            }
        }
    }

    #[test]
    fn xr_input_stationary_and_slow_blade_cut_moving_target_with_finite_bounds() {
        for hand_step in [0., 0.000_02] {
            for depth in [0., -0.8] {
                let (mut scene, mut controller, target, _) = setup();
                let mut pieces = vec![];
                for i in 0..=24 {
                    // Fruit crosses the held blade; slow controller motion is
                    // below the old absolute-hand-motion early return.
                    scene.set_object_position(target, Vec3::new(0., 2.3 - i as f32 * 0.025, depth));
                    pieces.extend(
                        controller
                            .update(&mut scene, Some(pose(i as f32 * hand_step)), true)
                            .unwrap()
                            .pieces,
                    );
                }
                if depth == 0. {
                    assert_eq!(pieces.len(), 2, "hand_step={hand_step}");
                    assert!(!scene.object_exists(target));
                } else {
                    assert!(pieces.is_empty() && scene.object_exists(target));
                }
            }
        }
    }

    #[test]
    fn xr_input_other_hand_reuses_real_parent_sample_for_same_frame_apple_crosscut() {
        let mut scene = SceneManager::default();
        let target = scene.add_triangle_mesh(apple()).unwrap();
        scene.set_object_scale(target, Vec3::splat(0.13));
        scene.set_object_position(target, Vec3::new(0., 2., -0.5));
        scene.set_cuttable(target, true, Vec3::ONE).unwrap();
        let volume = scene.triangle_meshes()[&target].mesh.volume();
        let mut controllers = [TrackedController::default(), TrackedController::default()];
        for controller in &mut controllers {
            let sword = scene.add_cube(Vec3::ZERO);
            scene
                .set_cutter(sword, Some([-Vec3::Z * 0.1, -Vec3::Z * 0.95]))
                .unwrap();
            controller.bind(&mut scene, Some(sword), true).unwrap();
        }
        let y = 2.
            + scene.triangle_meshes()[&target]
                .mesh
                .cpu()
                .unwrap()
                .centroid()
                .y
                * 0.13;
        // The second controller has a real parent sample before the first cut.
        // Its next sample crosses both children within this same scene frame.
        controllers[1]
            .update(
                &mut scene,
                Some(Mat4::from_translation(Vec3::new(-0.3, y, 0.))),
                true,
            )
            .unwrap();
        let mut first = CutReport::default();
        for step in 0..=12 {
            let pose = Mat4::from_translation(Vec3::new(0., 2.45 - step as f32 * 0.05, 0.));
            append_report(
                &mut first,
                controllers[0].update(&mut scene, Some(pose), true).unwrap(),
            );
        }
        assert_eq!(first.removed, [target]);
        assert_eq!(first.pieces.len(), 2);
        let second = controllers[1]
            .update_timed(
                &mut scene,
                Some(Mat4::from_translation(Vec3::new(0.3, y, 0.))),
                true,
                0.05,
            )
            .unwrap();
        assert_eq!(second.removed.len(), 2);
        assert_eq!(second.pieces.len(), 4);
        assert!(first.pieces.iter().all(|id| !scene.object_exists(*id)));
        let after: f64 = second
            .pieces
            .iter()
            .map(|id| {
                scene.triangle_meshes()[id]
                    .mesh
                    .cpu()
                    .unwrap()
                    .validate()
                    .unwrap();
                scene.triangle_meshes()[id].mesh.volume()
            })
            .sum();
        assert!((after - volume).abs() < volume * 1e-4);
        for _ in 0..12 {
            assert!(controllers[1]
                .update(
                    &mut scene,
                    Some(Mat4::from_translation(Vec3::new(0.3, y, 0.))),
                    true
                )
                .unwrap()
                .pieces
                .is_empty());
        }
    }

    #[test]
    fn xr_input_accumulates_real_finite_split_and_expires_falling_fragments() {
        let (mut scene, mut controller, target, sword) = setup();
        let mut pieces = vec![];
        let mut removed = vec![];
        for i in 0..=12 {
            let report = controller
                .update(&mut scene, Some(pose(-0.3 + i as f32 * 0.05)), true)
                .unwrap();
            pieces.extend(report.pieces);
            removed.extend(report.removed);
        }
        assert_eq!(removed, [target]);
        assert_eq!(pieces.len(), 2);
        assert!(!scene.object_exists(target));
        let volume: f64 = pieces
            .iter()
            .map(|id| scene.triangle_meshes()[id].mesh.volume())
            .sum();
        assert!((volume - 8.).abs() < 1e-5);
        let before: Vec3 = pieces
            .iter()
            .map(|id| scene.objects()[id].transform.position)
            .sum::<Vec3>()
            / 2.;
        scene.update(0.1);
        let after: Vec3 = pieces
            .iter()
            .map(|id| scene.objects()[id].transform.position)
            .sum::<Vec3>()
            / 2.;
        assert!(
            after.x > before.x && after.y < before.y,
            "Inherited velocity and gravity"
        );
        assert_eq!(
            scene.objects()[&sword].transform.position,
            pose(0.3).w_axis.truncate()
        );
        for _ in 0..50 {
            scene.update(0.1);
        }
        assert!(pieces.iter().all(|id| !scene.object_exists(*id)));
    }

    #[test]
    fn xr_input_no_trigger_stationary_reacquire_teleport_and_reset_do_not_cut() {
        let (mut scene, mut c, target, _) = setup();
        for pressed in [false, true, true] {
            assert!(c
                .update(&mut scene, Some(pose(-0.3)), pressed)
                .unwrap()
                .removed
                .is_empty());
        }
        c.update(&mut scene, None, true).unwrap();
        assert!(c
            .update(&mut scene, Some(pose(0.3)), true)
            .unwrap()
            .removed
            .is_empty());
        c.reset(&mut scene);
        c.update(&mut scene, Some(pose(-2.)), true).unwrap();
        assert!(c
            .update(&mut scene, Some(pose(2.)), true)
            .unwrap()
            .removed
            .is_empty());
        assert!(scene.object_exists(target));
        c.bind(&mut scene, None, false).unwrap();
        assert!(c
            .update(&mut scene, Some(pose(0.)), true)
            .unwrap()
            .removed
            .is_empty());
    }

    #[test]
    fn xr_input_rejects_nonrigid_and_invalid_pose_hides_and_clears_anchor() {
        let (mut scene, mut c, target, sword) = setup();
        for invalid in [
            Mat4::from_scale(Vec3::splat(2.)),
            Mat4::from_scale(Vec3::new(-1., 1., 1.)),
            Mat4::from_translation(Vec3::splat(f32::NAN)),
            Mat4::from_translation(Vec3::X * 101.),
        ] {
            c.reset(&mut scene);
            c.update(&mut scene, Some(pose(-0.3)), true).unwrap();
            assert!(c.update(&mut scene, Some(invalid), true).is_err());
            assert_eq!(scene.objects()[&sword].transform.position, HIDDEN);
            assert!(c
                .update(&mut scene, Some(pose(0.3)), true)
                .unwrap()
                .removed
                .is_empty());
        }
        assert!(scene.object_exists(target));
    }

    #[test]
    fn xr_input_preserves_scale_and_full_grip_orientation_rebind_hides_old() {
        let (mut scene, mut c, _, sword) = setup();
        let scale = Vec3::new(0.2, 0.3, 0.4);
        scene.set_object_scale(sword, scale);
        c.bind(&mut scene, Some(sword), true).unwrap();
        let rotation = Quat::from_euler(EulerRot::YXZ, 1.1, 0.7, -0.4);
        let grip = Mat4::from_rotation_translation(rotation, Vec3::new(1., 2., 3.));
        c.update(&mut scene, Some(grip), false).unwrap();
        let transform = scene.objects()[&sword].transform;
        assert_eq!(transform.scale, scale);
        assert!(
            (transform.model().transform_point3(-Vec3::Z)
                - grip.transform_point3(-Vec3::Z * scale))
            .length()
                < 1e-5
        );
        let other = scene.add_cube(Vec3::ZERO);
        c.bind(&mut scene, Some(other), false).unwrap();
        assert_eq!(scene.objects()[&sword].transform.position, HIDDEN);
        assert!(c
            .update(&mut scene, Some(pose(0.3)), true)
            .unwrap()
            .removed
            .is_empty());
    }

    #[test]
    fn xr_input_removed_binding_unbinds_before_pose_and_can_rebind() {
        for missing_pose in [false, true] {
            let (mut scene, mut c, target, sword) = setup();
            c.update(&mut scene, Some(pose(-0.3)), true).unwrap();
            c.update(&mut scene, Some(pose(-0.1)), true).unwrap();
            assert!(scene.remove_object(sword));
            let next_pose = if missing_pose { None } else { Some(pose(0.3)) };
            assert!(c
                .update(&mut scene, next_pose, true)
                .unwrap()
                .removed
                .is_empty());
            assert!(c.id.is_none() && c.anchor.is_none() && c.previous.is_none());
            c.reset(&mut scene);
            assert!(c
                .update(&mut scene, Some(pose(0.3)), true)
                .unwrap()
                .removed
                .is_empty());
            assert!(scene.object_exists(target));
            let replacement = scene.add_cube(Vec3::ZERO);
            scene
                .set_cutter(replacement, Some([Vec3::ZERO, -Vec3::Z]))
                .unwrap();
            c.bind(&mut scene, Some(replacement), true).unwrap();
            assert!(c
                .update(&mut scene, Some(pose(0.3)), true)
                .unwrap()
                .removed
                .is_empty());
            assert!(scene.object_exists(target));
            scene.clear();
            assert!(c.update(&mut scene, None, true).unwrap().removed.is_empty());
            assert!(c.id.is_none());
        }
    }

    #[test]
    fn xr_input_large_rotation_and_direction_reversal_restart_stroke() {
        let (mut scene, mut c, target, _) = setup();
        c.update(&mut scene, Some(pose(-0.3)), true).unwrap();
        c.update(&mut scene, Some(pose(-0.1)), true).unwrap();
        c.update(&mut scene, Some(pose(-0.2)), true).unwrap();
        let rotation =
            Mat4::from_rotation_translation(Quat::from_rotation_z(1.), Vec3::new(0., 0.3, 0.5));
        assert!(c
            .update(&mut scene, Some(rotation), true)
            .unwrap()
            .removed
            .is_empty());
        assert!(scene.object_exists(target));
    }
}
