//! Optional presentation-only smoothing for wire mirrors. Simulation stays authoritative.
use super::*;
use crate::visibility::ObjectRenderData;
use glam::Quat;

#[derive(Clone, Copy)]
struct Pose {
    scale: Vec3,
    rotation: Quat,
    translation: Vec3,
}
impl Pose {
    fn decompose(model: Mat4) -> Option<Self> {
        if !model.is_finite() || model.determinant().abs() < 1e-10 {
            return None;
        }
        let (scale, rotation, translation) = model.to_scale_rotation_translation();
        if !scale.is_finite() || scale.abs().min_element() < 1e-5 || !rotation.is_finite() {
            return None;
        }
        let reconstructed = Mat4::from_scale_rotation_translation(scale, rotation, translation);
        // Reject projective/sheared matrices rather than silently changing their geometry.
        if model
            .to_cols_array()
            .iter()
            .zip(reconstructed.to_cols_array())
            .any(|(a, b)| (a - b).abs() > 1e-5 * a.abs().max(1.))
        {
            return None;
        }
        Some(Self {
            scale,
            rotation,
            translation,
        })
    }
    fn matrix(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}
struct Track {
    from: Pose,
    to: Pose,
    target: ObjectRenderData,
    local_center: Vec3,
    local_extent: Vec3,
    elapsed: f32,
    duration: f32,
    velocity: Vec3,
    horizon: f32,
}
struct Sample {
    position: Vec3,
    time: f64,
    velocity: Vec3,
}
#[derive(Default)]
pub(super) struct MirrorInterpolation {
    duration: f32,
    max_distance: f32,
    active: BTreeMap<u32, Track>,
    horizon: f32,
    sample_time: Option<f64>,
    samples: BTreeMap<u32, Sample>,
}
fn matrix(data: &ObjectRenderData) -> Mat4 {
    Mat4::from_cols_array_2d(&data.instance.model)
}
fn extent(model: Mat4, half: Vec3) -> Vec3 {
    model.x_axis.truncate().abs() * half.x
        + model.y_axis.truncate().abs() * half.y
        + model.z_axis.truncate().abs() * half.z
}
// Keep the newest material/fade fields while substituting only the displayed pose.
fn copy_pose(to: &mut ObjectRenderData, from: &ObjectRenderData) {
    let fade = to.instance.normal_matrix[3][3];
    to.instance.model = from.instance.model;
    to.instance.normal_matrix = from.instance.normal_matrix;
    to.instance.normal_matrix[3][3] = fade;
    to.center = from.center;
    to.half_extent = from.half_extent;
}
impl MirrorInterpolation {
    pub(super) fn enabled(&self) -> bool {
        self.duration > 0.
    }
    pub(super) fn prediction_enabled(&self) -> bool {
        self.horizon > 0. && self.sample_time.is_some()
    }
    pub(super) fn clear(&mut self) {
        self.active.clear();
        self.samples.clear();
        self.sample_time = None;
        self.horizon = 0.;
    }
    pub(super) fn retain(&mut self, mut keep: impl FnMut(u32) -> bool) {
        self.active.retain(|&id, _| keep(id));
        self.samples.retain(|&id, _| keep(id));
    }
    /// Only wire-authoritative transforms enter this history, before smoothing mutates them.
    pub(super) fn sample(
        &mut self,
        id: u32,
        target: &SceneObject,
        eligible: bool,
        reset: bool,
    ) -> bool {
        if self.horizon == 0. || !eligible || target.render_options.is_static {
            self.samples.remove(&id);
            return false;
        }
        let Some(time) = self.sample_time else {
            return false;
        };
        let Some(pose) = Pose::decompose(matrix(&target.render_data)) else {
            self.samples.remove(&id);
            return false;
        };
        let position = pose.translation;
        let mut velocity = Vec3::ZERO;
        let mut discontinuity = false;
        if !reset {
            if let Some(previous) = self.samples.get(&id) {
                let dt = ((time - previous.time) / 1000.) as f32;
                let delta = position - previous.position;
                discontinuity = delta.length() > self.max_distance.min(2.);
                // Reject stale, duplicate, long-gap and teleport samples. Never clamp a
                // teleport into a plausible velocity, and never read the displayed pose.
                if (0.001..=0.5).contains(&dt) && delta.length() <= self.max_distance.min(2.) {
                    let candidate = delta / dt;
                    if candidate.length() <= 15. {
                        velocity = candidate;
                    }
                }
            }
        }
        self.samples.insert(
            id,
            Sample {
                position,
                time,
                velocity,
            },
        );
        discontinuity
    }
    pub(super) fn receive(
        &mut self,
        id: u32,
        old: Option<&SceneObject>,
        target: &mut SceneObject,
        geometry_changed: bool,
    ) {
        let Some(old) = old.filter(|o| {
            self.duration > 0.
                && !geometry_changed
                && o.mesh == target.mesh
                && o.local_bounds == target.local_bounds
        }) else {
            self.active.remove(&id);
            return;
        };
        if let Some(track) = self.active.get_mut(&id) {
            if self.horizon == 0.
                && track.target.instance.model == target.render_data.instance.model
            {
                track.target = target.render_data.clone();
                copy_pose(&mut target.render_data, &old.render_data);
                return;
            }
        }
        self.start(id, &old.render_data, target);
        if self.active.contains_key(&id) {
            copy_pose(&mut target.render_data, &old.render_data);
        }
    }
    /// A new topology ID inherits only the removed parent's displayed world pose.
    pub(super) fn receive_spawn(
        &mut self,
        id: u32,
        parent: &SceneObject,
        target: &mut SceneObject,
        birth_relative: Mat4,
    ) {
        if self.duration > 0. {
            let mut birth = parent.render_data.clone();
            birth.instance.model =
                (matrix(&parent.render_data) * birth_relative).to_cols_array_2d();
            self.start(id, &birth, target);
        }
    }
    fn start(&mut self, id: u32, from_data: &ObjectRenderData, target: &mut SceneObject) {
        self.active.remove(&id);
        let velocity = self.samples.get(&id).map_or(Vec3::ZERO, |s| s.velocity);
        if from_data.instance.model == target.render_data.instance.model && velocity == Vec3::ZERO {
            return;
        }
        let (Some(from), Some(to)) = (
            Pose::decompose(matrix(from_data)),
            Pose::decompose(matrix(&target.render_data)),
        ) else {
            return;
        };
        if from.translation.distance(to.translation) > self.max_distance
            || (from.scale * to.scale).min_element() <= 0.
        {
            return;
        }
        // Inverting the authoritative world AABB yields a conservative local AABB,
        // including parent transforms absent from SceneObject's local Transform.
        let inverse = matrix(&target.render_data).inverse();
        let local_center = inverse.transform_point3(target.render_data.center);
        let local_extent = extent(inverse, target.render_data.half_extent);
        if !local_center.is_finite() || !local_extent.is_finite() {
            return;
        }
        self.active.insert(
            id,
            Track {
                from,
                to,
                target: target.render_data.clone(),
                local_center,
                local_extent,
                elapsed: 0.,
                duration: self.duration,
                velocity,
                horizon: if velocity == Vec3::ZERO {
                    0.
                } else {
                    self.horizon.min(2. / velocity.length())
                },
            },
        );
        let fade = target.render_data.instance.normal_matrix[3][3];
        target.render_data.instance.model = from_data.instance.model;
        target.render_data.instance.normal_matrix = normal_matrix(from.matrix()).to_cols_array_2d();
        target.render_data.instance.normal_matrix[3][3] = fade;
        target.render_data.center = from.matrix().transform_point3(local_center);
        target.render_data.half_extent = extent(from.matrix(), local_extent);
    }
}
impl SceneManager {
    /// Bounded, render-only translation prediction. Zero disables and resets history.
    pub fn set_mirror_prediction(&mut self, horizon_ms: f32) -> Result<(), String> {
        if !horizon_ms.is_finite() || !(0. ..=300.).contains(&horizon_ms) {
            return Err("Mirror prediction requires horizon 0..300 ms".into());
        }
        if horizon_ms == 0. || self.mirror_interpolation.horizon != horizon_ms / 1000. {
            self.mirror_interpolation.samples.clear();
            self.mirror_interpolation.sample_time = None;
            let active = std::mem::take(&mut self.mirror_interpolation.active);
            for (id, track) in active {
                if let Some(object) = self.objects.get_mut(&id) {
                    copy_pose(&mut object.render_data, &track.target);
                    self.changed(id);
                }
            }
        }
        self.mirror_interpolation.horizon = horizon_ms / 1000.;
        Ok(())
    }
    /// Set the next publication's authoritative session-relative timestamp, in ms.
    pub fn set_mirror_prediction_sample_time(&mut self, time: f64) -> Result<(), String> {
        if !time.is_finite()
            || time < 0.
            || self
                .mirror_interpolation
                .sample_time
                .is_some_and(|old| time <= old)
        {
            return Err(
                "Mirror prediction sample time must be finite, nonnegative and strictly increasing"
                    .into(),
            );
        }
        self.mirror_interpolation.sample_time = Some(time);
        Ok(())
    }
    /// Configure wire-mirror presentation smoothing. Zero disables it (the default).
    pub fn set_mirror_interpolation(
        &mut self,
        duration_ms: f32,
        max_distance: f32,
    ) -> Result<(), String> {
        if !duration_ms.is_finite()
            || (duration_ms != 0. && !(8. ..=250.).contains(&duration_ms))
            || !max_distance.is_finite()
            || max_distance <= 0.
        {
            return Err("Mirror interpolation requires duration 0 or 8..250 ms and positive finite distance".into());
        }
        // Cadence adaptation affects new tracks only; do not snap a displayed pose.
        if duration_ms > 0. && self.mirror_interpolation.duration > 0. {
            self.mirror_interpolation.duration = duration_ms / 1000.;
            self.mirror_interpolation.max_distance = max_distance;
            return Ok(());
        }
        // Disabling settles existing tracks at the precise authoritative pose.
        let active = std::mem::take(&mut self.mirror_interpolation.active);
        for (id, track) in active {
            if let Some(object) = self.objects.get_mut(&id) {
                let changed = object.render_data.instance.model != track.target.instance.model;
                copy_pose(&mut object.render_data, &track.target);
                if changed {
                    self.changed(id);
                }
            }
        }
        self.mirror_interpolation.duration = duration_ms / 1000.;
        self.mirror_interpolation.max_distance = max_distance;
        Ok(())
    }
    pub(super) fn update_mirror_interpolation(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0. {
            return;
        }

        // Only active IDs are visited. No scene clones or per-frame scratch allocations.
        let mut render_changed = false;
        let mut static_changed = false;
        let mut static_shadow = false;
        let mut dynamic_shadow = false;
        let objects = &mut self.objects;
        self.mirror_interpolation.active.retain(|id, track| {
            let Some(object) = objects.get_mut(id) else {
                return false;
            };
            let duration = track.duration;
            track.elapsed = (track.elapsed + dt).min(duration.max(track.horizon));
            let t = if track.elapsed >= duration * (1. - 4. * f32::EPSILON) {
                1.
            } else {
                track.elapsed / duration
            };
            let previous = object.render_data.instance.model;
            if t >= 1. && track.horizon == 0. {
                copy_pose(&mut object.render_data, &track.target);
            } else {
                let model = Pose {
                    scale: track.from.scale.lerp(track.to.scale, t),
                    rotation: track.from.rotation.slerp(track.to.rotation, t),
                    translation: track.from.translation.lerp(
                        track.to.translation + track.velocity * track.elapsed.min(track.horizon),
                        t.min(1.),
                    ),
                }
                .matrix();
                let fade = object.render_data.instance.normal_matrix[3][3];
                object.render_data.instance.model = model.to_cols_array_2d();
                object.render_data.instance.normal_matrix = normal_matrix(model).to_cols_array_2d();
                object.render_data.instance.normal_matrix[3][3] = fade;
                object.render_data.center = model.transform_point3(track.local_center);
                object.render_data.half_extent = extent(model, track.local_extent);
            }
            if previous != object.render_data.instance.model {
                let options = object.render_options;
                render_changed = true;
                static_changed |= options.is_static;
                static_shadow |= options.casts_shadow && options.is_static;
                dynamic_shadow |= options.casts_shadow && !options.is_static;
            }
            t < 1. || track.horizon > 0.
        });
        self.render_revision = self.render_revision.wrapping_add(render_changed as u64);
        self.static_revision = self.static_revision.wrapping_add(static_changed as u64);
        self.shadow_revisions.static_casters = self
            .shadow_revisions
            .static_casters
            .wrapping_add(static_shadow as u64);
        self.shadow_revisions.dynamic_casters = self
            .shadow_revisions
            .dynamic_casters
            .wrapping_add(dynamic_shadow as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::super::wire::WirePublisher;
    use super::*;
    fn send(source: &mut SceneManager, publisher: &mut WirePublisher, mirror: &mut SceneManager) {
        mirror
            .apply_wire_publication(&publisher.capture(source).unwrap(), &[])
            .unwrap();
    }
    fn near(a: Vec3, b: Vec3) {
        assert!((a - b).length() < 1e-4, "{a:?} != {b:?}");
    }
    fn prediction_fixture() -> SceneManager {
        let mut mirror = SceneManager::default();
        mirror.set_mirror_interpolation(50., 4.).unwrap();
        mirror.set_mirror_prediction(300.).unwrap();
        mirror.set_mirror_prediction_sample_time(1000.).unwrap();
        mirror
            .apply_wire_publication(
                include_bytes!("../tests/fixtures/native-scene/initial.nwr"),
                &[],
            )
            .unwrap();
        assert!(mirror.triangle_meshes[&1].cuttable);
        assert!(!mirror.objects[&1].render_options.is_static);
        mirror.set_mirror_prediction_sample_time(1200.).unwrap();
        mirror
            .apply_wire_publication(
                include_bytes!("../tests/fixtures/native-scene/motion.nwr"),
                &[],
            )
            .unwrap();
        mirror
    }
    fn position(mirror: &SceneManager, id: u32) -> Vec3 {
        matrix(&mirror.objects[&id].render_data).w_axis.truncate()
    }
    #[test]
    fn mirror_prediction_continues_bounds_holds_and_disables() {
        let mut mirror = prediction_fixture();
        mirror.update_mirror_interpolation(0.1);
        near(position(&mirror, 1), Vec3::X * 1.8);
        mirror.update_mirror_interpolation(0.1);
        near(position(&mirror, 1), Vec3::X * 2.4);
        mirror.update_mirror_interpolation(0.1);
        near(position(&mirror, 1), Vec3::X * 3.);
        mirror.update_mirror_interpolation(2.);
        near(position(&mirror, 1), Vec3::X * 3.);
        near(position(&mirror, 2), Vec3::ZERO); // sword/cutter never predicts
        mirror.set_mirror_prediction(0.).unwrap();
        near(position(&mirror, 1), Vec3::X * 1.2);
        assert!(mirror.mirror_interpolation.samples.is_empty());
        mirror.set_mirror_prediction_sample_time(0.).unwrap();
    }
    #[test]
    fn mirror_prediction_stationary_recovery_cadence_and_authoritative_history() {
        let mut mirror = prediction_fixture();
        mirror.update_mirror_interpolation(0.2);
        let displayed = position(&mirror, 1);
        mirror.set_mirror_interpolation(80., 4.).unwrap();
        near(position(&mirror, 1), displayed);
        // A repeated authoritative position explicitly stops extrapolation, and
        // corrects continuously from the previously displayed predicted pose.
        let mut target = mirror.objects[&1].clone();
        target.render_data = mirror.mirror_interpolation.active[&1].target.clone();
        mirror.set_mirror_prediction_sample_time(1400.).unwrap();
        assert!(!mirror.mirror_interpolation.sample(1, &target, true, false));
        assert_eq!(mirror.mirror_interpolation.samples[&1].velocity, Vec3::ZERO);
        let old = mirror.objects[&1].clone();
        mirror
            .mirror_interpolation
            .receive(1, Some(&old), &mut target, false);
        mirror.objects.insert(1, target);
        near(position(&mirror, 1), displayed);
        mirror.update_mirror_interpolation(0.08);
        near(position(&mirror, 1), Vec3::X * 1.2);
        mirror.update_mirror_interpolation(1.);
        near(position(&mirror, 1), Vec3::X * 1.2);
    }
    #[test]
    fn mirror_prediction_recovery_uses_authority_and_caps_distance() {
        let mut mirror = prediction_fixture();
        mirror.update_mirror_interpolation(0.2);
        let displayed = position(&mirror, 1);
        let mut target = mirror.objects[&1].clone();
        target.render_data = mirror.mirror_interpolation.active[&1].target.clone();
        target.render_data.instance.model =
            Mat4::from_translation(Vec3::X * 1.8).to_cols_array_2d();
        mirror.set_mirror_prediction_sample_time(1300.).unwrap();
        assert!(!mirror.mirror_interpolation.sample(1, &target, true, false));
        near(
            mirror.mirror_interpolation.samples[&1].velocity,
            Vec3::X * 6.,
        );
        let old = mirror.objects[&1].clone();
        mirror
            .mirror_interpolation
            .receive(1, Some(&old), &mut target, false);
        mirror.objects.insert(1, target);
        near(position(&mirror, 1), displayed);
        mirror.update_mirror_interpolation(0.3);
        near(position(&mirror, 1), Vec3::X * 3.6);
        mirror.clear_scene();
        assert_eq!(mirror.mirror_interpolation.horizon, 0.);
        assert!(mirror.mirror_interpolation.samples.is_empty());
    }
    #[test]
    fn mirror_prediction_teleport_topology_static_and_deletion_reset() {
        let mut mirror = prediction_fixture();
        let mut target = mirror.objects[&1].clone();
        target.render_data.instance.model =
            Mat4::from_translation(Vec3::X * 20.).to_cols_array_2d();
        mirror.set_mirror_prediction_sample_time(1400.).unwrap();
        assert!(mirror.mirror_interpolation.sample(1, &target, true, false));
        assert_eq!(mirror.mirror_interpolation.samples[&1].velocity, Vec3::ZERO);
        // New topology and child birth never inherit a parent's estimated velocity.
        mirror.set_mirror_prediction_sample_time(1500.).unwrap();
        mirror.mirror_interpolation.sample(1, &target, true, true);
        assert_eq!(mirror.mirror_interpolation.samples[&1].velocity, Vec3::ZERO);
        target.render_options.is_static = true;
        mirror.mirror_interpolation.sample(1, &target, true, false);
        assert!(!mirror.mirror_interpolation.samples.contains_key(&1));
        mirror.mirror_interpolation.retain(|_| false);
        assert!(mirror.mirror_interpolation.active.is_empty());
        assert!(mirror.mirror_interpolation.samples.is_empty());
        assert!(mirror.set_mirror_prediction_sample_time(1500.).is_err());
        assert!(mirror.set_mirror_prediction(301.).is_err());
    }
    #[test]
    fn mirror_interpolation_spawn_uses_displayed_parent_and_child_bounds_before_next_packet() {
        let mut source = SceneManager::default();
        let parent = source.add_cube(Vec3::ZERO);
        let mut mirror = SceneManager::default();
        mirror.set_mirror_interpolation(200., 4.).unwrap();
        let mut publisher = WirePublisher::default();
        send(&mut source, &mut publisher, &mut mirror);
        source.set_object_position(parent, Vec3::X);
        send(&mut source, &mut publisher, &mut mirror);
        mirror.update(0.1);
        let displayed = matrix(&mirror.objects[&parent].render_data);
        near(displayed.w_axis.truncate(), Vec3::X * 0.5);
        source.remove_object(parent);
        let child = source.add_cube(Vec3::X * 2.);
        // Child-specific geometry bounds must replace the removed parent's bounds.
        source
            .objects
            .get_mut(&child)
            .unwrap()
            .render_data
            .half_extent = Vec3::new(0.1, 0.2, 0.3);
        source
            .objects
            .get_mut(&child)
            .unwrap()
            .render_data
            .instance
            .material = [0.1, 0.2, 0.3, 0.4];
        let bytes = publisher.capture(&mut source).unwrap();
        mirror
            .apply_wire_publication_with_spawns(&bytes, &[], &[child, parent])
            .unwrap();
        assert!(!mirror.objects.contains_key(&parent));
        let o = &mirror.objects[&child];
        assert_eq!(matrix(&o.render_data), displayed);
        near(o.render_data.half_extent, Vec3::new(0.1, 0.2, 0.3));
        near(o.render_data.center, Vec3::X * 0.5);
        assert_eq!(o.render_data.instance.material, [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(o.mesh, source.objects[&child].mesh);
        // No second wire publication: render frames alone advance the half.
        mirror.update(0.1);
        near(
            matrix(&mirror.objects[&child].render_data)
                .w_axis
                .truncate(),
            Vec3::X * 1.25,
        );
        near(mirror.objects[&child].render_data.center, Vec3::X * 1.25);
        mirror.update(0.1);
        assert_eq!(
            matrix(&mirror.objects[&child].render_data),
            matrix(&source.objects[&child].render_data)
        );
    }
    #[test]
    fn mirror_interpolation_spawn_disabled_existing_and_missing_parent() {
        for enabled in [false, true] {
            let mut source = SceneManager::default();
            let parent = source.add_cube(Vec3::ZERO);
            let existing = source.add_cube(Vec3::Y);
            let mut mirror = SceneManager::default();
            if enabled {
                mirror.set_mirror_interpolation(100., 4.).unwrap();
            }
            let mut publisher = WirePublisher::default();
            send(&mut source, &mut publisher, &mut mirror);
            source.remove_object(parent);
            source.set_object_position(existing, Vec3::Y * 2.);
            let child = source.add_cube(Vec3::X);
            let unknown_child = source.add_cube(Vec3::Z);
            let bytes = publisher.capture(&mut source).unwrap();
            mirror
                .apply_wire_publication_with_spawns(
                    &bytes,
                    &[],
                    &[child, parent, existing, parent, unknown_child, 99999],
                )
                .unwrap();
            near(
                matrix(&mirror.objects[&existing].render_data)
                    .w_axis
                    .truncate(),
                Vec3::Y * if enabled { 1. } else { 2. },
            );
            near(
                matrix(&mirror.objects[&child].render_data)
                    .w_axis
                    .truncate(),
                if enabled { Vec3::ZERO } else { Vec3::X },
            );
            near(
                matrix(&mirror.objects[&unknown_child].render_data)
                    .w_axis
                    .truncate(),
                Vec3::Z,
            );
            assert_eq!(
                mirror.mirror_interpolation.active.len(),
                if enabled { 2 } else { 0 }
            );
            // Repeated mappings for an already-present child preserve its current track.
            mirror.update(0.05);
            let displayed = matrix(&mirror.objects[&child].render_data);
            mirror
                .apply_wire_publication_with_spawns(
                    &publisher.capture(&mut source).unwrap(),
                    &[],
                    &[child, parent],
                )
                .unwrap();
            assert_eq!(matrix(&mirror.objects[&child].render_data), displayed);
        }
    }
    #[test]
    fn mirror_interpolation_spawn_invalid_inputs_are_atomic() {
        let mut source = SceneManager::default();
        let parent = source.add_cube(Vec3::ZERO);
        let mut mirror = SceneManager::default();
        mirror.set_mirror_interpolation(100., 4.).unwrap();
        let mut publisher = WirePublisher::default();
        send(&mut source, &mut publisher, &mut mirror);
        source.remove_object(parent);
        let child = source.add_cube(Vec3::X);
        let bytes = publisher.capture(&mut source).unwrap();
        let revision = mirror.render_revision;
        for (pairs, locals) in [
            (vec![child], vec![]),
            (vec![child, child], vec![]),
            (vec![child, u32::MAX], vec![]),
            (vec![u32::MAX, parent], vec![]),
            (vec![child, parent, child, 999], vec![]),
            (vec![999, parent], vec![]),
            (vec![child, parent], vec![child]),
            (vec![child, parent], vec![parent]),
        ] {
            assert!(mirror
                .apply_wire_publication_with_spawns(&bytes, &locals, &pairs)
                .is_err());
            assert_eq!(mirror.wire_publication_sequence(), 1);
            assert_eq!(mirror.render_revision, revision);
            assert!(mirror.objects.contains_key(&parent));
            assert!(!mirror.objects.contains_key(&child));
            assert!(mirror.mirror_interpolation.active.is_empty());
        }
        mirror
            .apply_wire_publication_with_spawns(&bytes, &[], &[child, parent])
            .unwrap();
        // A parent retained in the same incoming publication is contradictory.
        let other = source.add_cube(Vec3::Y);
        let next = publisher.capture(&mut source).unwrap();
        assert!(mirror
            .apply_wire_publication_with_spawns(&next, &[], &[other, child])
            .is_err());
        assert_eq!(mirror.wire_publication_sequence(), 2);
    }
    #[test]
    fn mirror_interpolation_world_midpoint_rotation_finish_and_idle() {
        let mut source = SceneManager::default();
        let id = source.add_cube(Vec3::ZERO);
        source.set_render_options(
            id,
            RenderOptions {
                is_static: true,
                ..Default::default()
            },
        );
        let mut mirror = SceneManager::default();
        mirror.set_mirror_interpolation(60., 2.).unwrap();
        let mut publisher = WirePublisher::default();
        send(&mut source, &mut publisher, &mut mirror);
        let from = matrix(&mirror.objects[&id].render_data);
        // Simulate a published parent transform different from the local transform.
        let target = Mat4::from_scale_rotation_translation(
            Vec3::new(2., 1., 0.5),
            Quat::from_rotation_y(2.),
            Vec3::X,
        );
        let o = source.objects.get_mut(&id).unwrap();
        o.render_data.instance.model = target.to_cols_array_2d();
        o.render_data.center = Vec3::X;
        o.render_data.half_extent = extent(target, Vec3::splat(0.5));
        o.render_data.instance.normal_matrix = normal_matrix(target).to_cols_array_2d();
        o.render_data.instance.material = [0.2, 0.3, 0.4, 0.5];
        source.changed(id);
        send(&mut source, &mut publisher, &mut mirror);
        assert_eq!(matrix(&mirror.objects[&id].render_data), from);
        mirror.update(0.03);
        let o = &mirror.objects[&id];
        let mid = matrix(&o.render_data);
        near(mid.w_axis.truncate(), Vec3::X * 0.5);
        near(
            mid.transform_vector3(Vec3::Z).normalize(),
            Quat::from_rotation_y(1.) * Vec3::Z,
        );
        assert_eq!(o.render_data.instance.material, [0.2, 0.3, 0.4, 0.5]);
        let normal = Mat4::from_cols_array_2d(&o.render_data.instance.normal_matrix)
            .transform_vector3(Vec3::new(1., 1., 0.));
        assert!(
            normal
                .dot(mid.transform_vector3(Vec3::new(1., -1., 0.)))
                .abs()
                < 1e-5
        );
        // Every target cube corner remains inside interpolated conservative bounds.
        for x in [-0.5, 0.5] {
            for y in [-0.5, 0.5] {
                for z in [-0.5, 0.5] {
                    assert!(
                        (mid.transform_point3(Vec3::new(x, y, z)) - o.render_data.center)
                            .abs()
                            .cmple(o.render_data.half_extent + Vec3::splat(1e-5))
                            .all()
                    );
                }
            }
        }
        let rev = (
            mirror.render_revision,
            mirror.static_revision,
            mirror.shadow_revision(true),
        );
        // An identical new packet must not restart or jump the active trajectory.
        send(&mut source, &mut publisher, &mut mirror);
        assert_eq!(matrix(&mirror.objects[&id].render_data), mid);
        mirror.update(0.03);
        assert_eq!(matrix(&mirror.objects[&id].render_data), target);
        assert!(
            mirror.render_revision > rev.0
                && mirror.static_revision > rev.1
                && mirror.shadow_revision(true) > rev.2
        );
        let rev = (
            mirror.render_revision,
            mirror.static_revision,
            mirror.shadow_revision(true),
        );
        mirror.update(0.03);
        send(&mut source, &mut publisher, &mut mirror);
        mirror.update(1.);
        assert_eq!(
            rev,
            (
                mirror.render_revision,
                mirror.static_revision,
                mirror.shadow_revision(true)
            )
        );
        assert!(mirror.mirror_interpolation.active.is_empty());
    }
    #[test]
    fn mirror_interpolation_retargets_continuously_and_prunes_removed_ids() {
        let mut source = SceneManager::default();
        let id = source.add_cube(Vec3::ZERO);
        let mut mirror = SceneManager::default();
        mirror.set_mirror_interpolation(60., 2.).unwrap();
        let mut publisher = WirePublisher::default();
        send(&mut source, &mut publisher, &mut mirror);
        source.set_object_position(id, Vec3::X);
        send(&mut source, &mut publisher, &mut mirror);
        mirror.update(0.03);
        source.set_object_position(id, Vec3::X * 1.5);
        let added = source.add_cube(Vec3::Y);
        send(&mut source, &mut publisher, &mut mirror);
        near(
            matrix(&mirror.objects[&id].render_data).w_axis.truncate(),
            Vec3::X * 0.5,
        );
        near(
            matrix(&mirror.objects[&added].render_data)
                .w_axis
                .truncate(),
            Vec3::Y,
        );
        mirror.update(0.03);
        near(
            matrix(&mirror.objects[&id].render_data).w_axis.truncate(),
            Vec3::X,
        );
        source.remove_object(id);
        send(&mut source, &mut publisher, &mut mirror);
        assert!(!mirror.objects.contains_key(&id));
        assert!(mirror.mirror_interpolation.active.is_empty());
    }
    #[test]
    fn mirror_interpolation_wire_geometry_replacement_and_disable() {
        let mut source = SceneManager::default();
        let id = source.add_info_panel("diagnostics", 1., 0.5).unwrap();
        let mut mirror = SceneManager::default();
        mirror.set_mirror_interpolation(60., 2.).unwrap();
        let mut publisher = WirePublisher::default();
        send(&mut source, &mut publisher, &mut mirror);
        source.set_object_position(id, Vec3::X);
        send(&mut source, &mut publisher, &mut mirror);
        mirror.update(0.02);
        assert!(!mirror.mirror_interpolation.active.is_empty());
        source.update_info_panel(id, &[("changed", "geometry".into())]);
        send(&mut source, &mut publisher, &mut mirror);
        assert!(mirror.mirror_interpolation.active.is_empty());
        assert_eq!(
            matrix(&mirror.objects[&id].render_data),
            matrix(&source.objects[&id].render_data)
        );
        source.set_object_position(id, Vec3::X * 1.5);
        send(&mut source, &mut publisher, &mut mirror);
        mirror.update(0.02);
        mirror.set_mirror_interpolation(0., 2.).unwrap();
        assert_eq!(
            matrix(&mirror.objects[&id].render_data),
            matrix(&source.objects[&id].render_data)
        );
        assert!(mirror.mirror_interpolation.active.is_empty());
    }
    #[test]
    fn mirror_interpolation_geometry_teleport_and_invalid_trs_snap() {
        let mut source = SceneManager::default();
        let id = source.add_cube(Vec3::ZERO);
        let mut mirror = SceneManager::default();
        mirror.set_mirror_interpolation(60., 2.).unwrap();
        let mut publisher = WirePublisher::default();
        send(&mut source, &mut publisher, &mut mirror);
        source.set_object_position(id, Vec3::X * 3.);
        send(&mut source, &mut publisher, &mut mirror);
        assert_eq!(
            matrix(&mirror.objects[&id].render_data),
            matrix(&source.objects[&id].render_data)
        );
        assert!(mirror.mirror_interpolation.active.is_empty());
        let old = source.objects[&id].clone();
        let mut target = old.clone();
        target.render_data.instance.model =
            Mat4::from_translation(Vec3::X * 3.5).to_cols_array_2d();
        mirror
            .mirror_interpolation
            .receive(id, Some(&old), &mut target, true);
        assert!(mirror.mirror_interpolation.active.is_empty());
        near(matrix(&target.render_data).w_axis.truncate(), Vec3::X * 3.5);
        let mut shear = Mat4::IDENTITY;
        shear.y_axis.x = 0.3;
        for model in [
            shear,
            Mat4::from_scale(Vec3::new(0., 1., 1.)),
            Mat4::from_cols_array(&[f32::NAN; 16]),
        ] {
            assert!(Pose::decompose(model).is_none());
        }
        source
            .objects
            .get_mut(&id)
            .unwrap()
            .render_data
            .instance
            .model = shear.to_cols_array_2d();
        source.changed(id);
        send(&mut source, &mut publisher, &mut mirror);
        assert_eq!(matrix(&mirror.objects[&id].render_data), shear);
        assert!(mirror.mirror_interpolation.active.is_empty());
        for duration in [-1., 1., 251., f32::NAN, f32::INFINITY] {
            assert!(mirror.set_mirror_interpolation(duration, 2.).is_err());
        }
        assert!(mirror.set_mirror_interpolation(60., 0.).is_err());
        assert!(mirror.set_mirror_interpolation(0., 2.).is_ok());
    }
}
