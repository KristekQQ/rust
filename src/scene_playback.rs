//! Source-timestamp playback. Immutable deltas remain queued until their boundary.
//! The clock advances by render dt, stalls at latest+extrapolation, and explicitly
//! rebuffers after a >1s source gap. Arrival never retargets a displayed pose.
use super::*;
use glam::Quat;
use std::collections::VecDeque;

const MAX_FRAMES: usize = 128;
const MAX_BUFFER: usize = 32 * 1024 * 1024;
struct Pending {
    frame: Frame,
    time: f64,
    bytes: usize,
    locals: Vec<u32>,
}
struct Snapshot {
    time: f64,
    objects: BTreeMap<u32, SceneObject>,
    eligible: BTreeSet<u32>,
    breaks: BTreeSet<u32>,
}
#[derive(Default)]
pub(crate) struct ScenePlayback {
    delay: f64,
    extrapolation: f64,
    max_distance: f32,
    time: Option<f64>,
    last_time: Option<f64>,
    last_sequence: Option<u64>,
    queue: VecDeque<Pending>,
    bytes: usize,
    previous: Option<Snapshot>,
    current: Option<Snapshot>,
    underruns: u64,
    rebufferings: u64,
    recovering: bool,
    lag_duration_ms: f64,
    was_starved: bool,
    corrections: BTreeMap<u32, PoseCorrection>,
    error: Option<String>,
}
impl ScenePlayback {
    pub(crate) fn enabled(&self) -> bool {
        self.delay > 0.
    }
}
struct PoseCorrection {
    position: Vec3,
    rotation: Quat,
    scale: Vec3,
    elapsed: f32,
}
#[derive(Clone, Copy)]
struct PlaybackPose {
    scale: Vec3,
    rotation: Quat,
    position: Vec3,
}
fn pose(object: &SceneObject) -> Option<PlaybackPose> {
    let m = Mat4::from_cols_array_2d(&object.render_data.instance.model);
    if !m.is_finite() || m.determinant().abs() < 1e-10 {
        return None;
    }
    let (scale, rotation, position) = m.to_scale_rotation_translation();
    if !scale.is_finite() || !rotation.is_finite() || scale.abs().min_element() < 1e-5 {
        return None;
    }
    let rebuilt = Mat4::from_scale_rotation_translation(scale, rotation, position);
    if m.to_cols_array()
        .iter()
        .zip(rebuilt.to_cols_array())
        .any(|(a, b)| (a - b).abs() > 1e-5 * a.abs().max(1.))
    {
        return None;
    }
    Some(PlaybackPose {
        scale,
        rotation,
        position,
    })
}
fn snapshot(frame: &Frame, time: f64, locals: &[u32]) -> Snapshot {
    let mut breaks: BTreeSet<_> = frame
        .new_triangles
        .keys()
        .chain(frame.new_panels.keys())
        .copied()
        .collect();
    for (id, triangle) in &frame.triangles {
        if triangle
            .instance
            .is_some_and(|(handle, _)| frame.new_variants.contains_key(&handle))
        {
            breaks.insert(*id);
        }
    }
    let eligible = frame
        .triangles
        .iter()
        .filter_map(|(id, t)| {
            ((t.cuttable || frame.cut_lineage.iter().any(|(child, _)| child == id))
                && !frame.numeric_children.contains(id)
                && !frame.font_children.contains(id)
                && !frame.font_texts.contains(id)
                && !frame.font_owners.contains(id)
                && !frame.font_assets.contains(id))
            .then_some(*id)
        })
        .collect();
    Snapshot {
        time,
        objects: frame
            .objects
            .iter()
            .filter(|(id, _)| !locals.contains(id))
            .cloned()
            .collect(),
        eligible,
        breaks,
    }
}
fn compatible(
    a: &SceneObject,
    b: &SceneObject,
    max_distance: f32,
) -> Option<(PlaybackPose, PlaybackPose)> {
    if a.mesh != b.mesh || a.local_bounds != b.local_bounds {
        return None;
    }
    let (a, b) = (pose(a)?, pose(b)?);
    ((a.scale * b.scale).min_element() > 0. && a.position.distance(b.position) <= max_distance)
        .then_some((a, b))
}
fn velocity(
    previous: Option<&Snapshot>,
    current: &Snapshot,
    id: u32,
    max_distance: f32,
) -> Option<Vec3> {
    let previous = previous?;
    if current.breaks.contains(&id) {
        return None;
    }
    let dt = ((current.time - previous.time) / 1000.) as f32;
    if !(0.001..=0.5).contains(&dt) {
        return None;
    }
    let (a, b) = compatible(
        previous.objects.get(&id)?,
        current.objects.get(&id)?,
        max_distance,
    )?;
    let v = (b.position - a.position) / dt;
    (v.length() <= 15.).then_some(v)
}
// Monotone component tangents: reversals become zero tangent, avoiding a curve
// continuing through a turning point. Tangent magnitude is bounded by the secant.
fn tangent(v: Vec3, secant: Vec3) -> Vec3 {
    let mut result = Vec3::ZERO;
    for i in 0..3 {
        if v[i] * secant[i] > 0. {
            result[i] = v[i].signum() * v[i].abs().min(3. * secant[i].abs());
        }
    }
    result
}
fn write_pose(object: &mut SceneObject, source: &SceneObject, p: PlaybackPose) -> bool {
    let model = Mat4::from_scale_rotation_translation(p.scale, p.rotation, p.position);
    if object.render_data.instance.model == model.to_cols_array_2d() {
        return false;
    }
    let original = Mat4::from_cols_array_2d(&source.render_data.instance.model);
    let inv = original.inverse();
    let local_center = inv.transform_point3(source.render_data.center);
    let e = source.render_data.half_extent;
    let half = inv.x_axis.truncate().abs() * e.x
        + inv.y_axis.truncate().abs() * e.y
        + inv.z_axis.truncate().abs() * e.z;
    let fade = object.render_data.instance.normal_matrix[3][3];
    object.render_data.instance.model = model.to_cols_array_2d();
    object.render_data.instance.normal_matrix = normal_matrix(model).to_cols_array_2d();
    object.render_data.instance.normal_matrix[3][3] = fade;
    object.render_data.center = model.transform_point3(local_center);
    object.render_data.half_extent = model.x_axis.truncate().abs() * half.x
        + model.y_axis.truncate().abs() * half.y
        + model.z_axis.truncate().abs() * half.z;
    true
}
impl SceneManager {
    /// Delay 0 resets the timeline; updating an enabled delay preserves its clock.
    pub fn set_scene_playback(
        &mut self,
        delay_ms: f64,
        extrapolation_ms: f64,
        max_distance: f32,
    ) -> Result<(), String> {
        if !delay_ms.is_finite()
            || !(0. ..=2000.).contains(&delay_ms)
            || !extrapolation_ms.is_finite()
            || !(0. ..=80.).contains(&extrapolation_ms)
            || !max_distance.is_finite()
            || max_distance <= 0.
        {
            return Err("Invalid scene playback options".into());
        }
        if delay_ms == 0. || !self.scene_playback.enabled() {
            self.scene_playback = ScenePlayback::default();
        }
        if delay_ms > 0. {
            self.set_mirror_interpolation(0., max_distance)?;
            self.set_mirror_prediction(0.)?;
            self.scene_playback.delay = delay_ms;
            self.scene_playback.extrapolation = extrapolation_ms;
            self.scene_playback.max_distance = max_distance;
        }
        Ok(())
    }
    /// Ordered packets only. Overflow rejects atomically; caller must reset/reconnect
    /// or retry the SAME packet after draining, never skip an immutable asset delta.
    pub fn enqueue_scene_publication(
        &mut self,
        bytes: &[u8],
        sampled_at_ms: f64,
        local_ids: &[u32],
    ) -> Result<(), String> {
        let p = &self.scene_playback;
        if !p.enabled() {
            return Err("Scene playback is disabled".into());
        }
        if let Some(error) = &p.error {
            return Err(error.clone());
        }
        if !sampled_at_ms.is_finite() || p.last_time.is_some_and(|t| sampled_at_ms <= t) {
            return Err("Scene playback timestamps must increase".into());
        }
        if p.queue.len() >= MAX_FRAMES || p.bytes.saturating_add(bytes.len()) > MAX_BUFFER {
            return Err("Scene playback overflow: retry in order or reset session".into());
        }
        let frame = decode(bytes)?;
        let expected = p
            .last_sequence
            .unwrap_or(self.wire_state.sequence)
            .checked_add(1);
        if expected != Some(frame.sequence) {
            return Err("Out-of-order scene playback publication".into());
        }
        let p = &mut self.scene_playback;
        if p.time.is_none() {
            p.time = Some(sampled_at_ms - p.delay);
        }
        if p.last_time.is_some_and(|t| sampled_at_ms - t > 1000.) {
            p.time = Some(p.time.unwrap().max(sampled_at_ms - p.delay));
            p.rebufferings += 1;
            // Explicit long-gap discontinuity: no pose error may bridge it.
            p.was_starved = false;
            p.corrections.clear();
        }
        p.last_time = Some(sampled_at_ms);
        p.last_sequence = Some(frame.sequence);
        p.bytes += bytes.len();
        p.queue.push_back(Pending {
            frame,
            time: sampled_at_ms,
            bytes: bytes.len(),
            locals: local_ids.to_vec(),
        });
        Ok(())
    }
    pub fn scene_playback_state(&self) -> serde_json::Value {
        let p = &self.scene_playback;
        serde_json::json!({"enabled":p.enabled(),"timeMs":p.time,"sequence":self.wire_state.sequence.to_string(),
            "extrapolating":p.time.zip(p.last_time).is_some_and(|(time,latest)|time>latest),
            "queued":p.queue.len(),"bufferBytes":p.bytes,"underruns":p.underruns,"rebufferings":p.rebufferings,"error":p.error})
    }
    pub(crate) fn update_scene_playback(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0. || self.scene_playback.error.is_some() {
            return;
        }
        let mut p = std::mem::take(&mut self.scene_playback);
        let (Some(time), Some(latest)) = (p.time, p.last_time) else {
            self.scene_playback = p;
            return;
        };
        // Correct sustained startup/render backlog as well as starvation. A short
        // arrival burst must not retime a trajectory; wait 500ms before enabling
        // the bounded clock controller. Deadband absorbs packet cadence.
        let target = latest - p.delay;
        if time < target - 60. {
            p.lag_duration_ms += dt as f64 * 1000.;
            if p.lag_duration_ms >= 500. {
                p.recovering = true;
            }
        } else {
            p.lag_duration_ms = 0.;
        }
        let rate = if p.recovering && time > target + 60. {
            0.9
        } else if p.recovering && time < target - 60. {
            1.1
        } else {
            1.
        };
        let next = time + dt as f64 * 1000. * rate;
        p.time = Some(next.min(latest + p.extrapolation));
        if next > latest + 0.001 {
            p.recovering = true;
            p.underruns += 1;
        }
        let time = p.time.unwrap();
        // Preserve presentation BEFORE committing overdue packets: a late sample
        // may already be behind our capped clock and overwrite a held sword.
        let begin_recovery = p.was_starved && !p.queue.is_empty() && p.corrections.is_empty();
        let displayed: BTreeMap<_, _> = if begin_recovery {
            p.current
                .as_ref()
                .into_iter()
                .flat_map(|snapshot| snapshot.objects.keys())
                .filter_map(|id| self.objects.get(id).cloned().map(|object| (*id, object)))
                .collect()
        } else {
            BTreeMap::new()
        };
        let mut crossed_breaks = BTreeSet::new();
        while p
            .queue
            .front()
            .is_some_and(|frame| frame.time <= time + 0.001)
        {
            let pending = p.queue.pop_front().unwrap();
            p.bytes -= pending.bytes;
            let mut current = snapshot(&pending.frame, pending.time, &pending.locals);
            // Topology can change by reusing an existing variant asset; no new
            // asset bytes are required in that case. Teleports also reset velocity.
            for (id, triangle) in &pending.frame.triangles {
                if triangle.instance != self.variant_instances.get(id).copied() {
                    current.breaks.insert(*id);
                }
            }
            if let Some(previous) = &p.current {
                for (&id, object) in &current.objects {
                    if previous
                        .objects
                        .get(&id)
                        .is_none_or(|old| compatible(old, object, p.max_distance).is_none())
                    {
                        current.breaks.insert(id);
                    }
                }
            }
            crossed_breaks.extend(current.breaks.iter().copied());
            if let Err(error) =
                self.apply_decoded_wire_publication(pending.frame, &pending.locals, &[])
            {
                p.error = Some(error);
                break;
            }
            p.previous = p.current.take();
            p.current = Some(current);
        }
        if p.error.is_none() {
            if let Some(current) = &p.current {
                let future = p.queue.front();
                // Remember even a short underrun: held swords can need recovery
                // before the extrapolation cap. An active correction never rearms.
                p.was_starved = !begin_recovery
                    && p.corrections.is_empty()
                    && future.is_none()
                    && time > current.time + 0.001;
                p.corrections.retain(|id, _| {
                    current.objects.contains_key(id)
                        && !current.breaks.contains(id)
                        && !crossed_breaks.contains(id)
                });
                for correction in p.corrections.values_mut() {
                    correction.elapsed += dt;
                }
                let mut changed = false;
                let mut static_changed = false;
                let mut static_shadow = false;
                let mut dynamic_shadow = false;
                for (&id, source) in &current.objects {
                    let Some(object) = self.objects.get_mut(&id) else {
                        continue;
                    };
                    let future_object = future.and_then(|pending| get(&pending.frame.objects, id));
                    let may_extrapolate = future.is_none()
                        && current.eligible.contains(&id)
                        && !source.render_options.is_static;
                    if !may_extrapolate
                        && !p.corrections.contains_key(&id)
                        && !displayed.contains_key(&id)
                        && object.render_data.instance.model == source.render_data.instance.model
                        && future_object.is_none_or(|target| {
                            target.render_data.instance.model == source.render_data.instance.model
                        })
                    {
                        continue;
                    }
                    let Some(mut result) = pose(source) else {
                        continue;
                    };
                    let mut compatible_bracket = false;
                    if let Some(future) = future {
                        // Future IDs/materials/visibility/assets are never applied here.
                        let target = future_object;
                        let broken = future.frame.new_triangles.contains_key(&id)
                            || future.frame.new_panels.contains_key(&id)
                            || get(&future.frame.triangles, id).is_some_and(|t| {
                                t.instance != self.variant_instances.get(&id).copied()
                                    || t.instance.is_some_and(|(handle, _)| {
                                        future.frame.new_variants.contains_key(&handle)
                                    })
                            });
                        let span = ((future.time - current.time) / 1000.) as f32;
                        if !broken && (0.001..=0.5).contains(&span) {
                            if let Some((a, b)) =
                                target.and_then(|target| compatible(source, target, p.max_distance))
                            {
                                compatible_bracket = true;
                                let t = ((time - current.time) / (future.time - current.time))
                                    .clamp(0., 1.) as f32;
                                let secant = (b.position - a.position) / span;
                                let v0 = tangent(
                                    velocity(p.previous.as_ref(), current, id, p.max_distance)
                                        .unwrap_or(secant),
                                    secant,
                                );
                                let v1 = secant;
                                let hermite = a.position * (2. * t * t * t - 3. * t * t + 1.)
                                    + v0 * span * (t * t * t - 2. * t * t + t)
                                    + b.position * (-2. * t * t * t + 3. * t * t)
                                    + v1 * span * (t * t * t - t * t);
                                result.position = hermite
                                    .clamp(a.position.min(b.position), a.position.max(b.position));
                                result.rotation = a.rotation.slerp(b.rotation, t);
                                result.scale = a.scale.lerp(b.scale, t);
                            }
                        }
                    } else if current.eligible.contains(&id) && !source.render_options.is_static {
                        if let Some(v) = velocity(p.previous.as_ref(), current, id, p.max_distance)
                        {
                            result.position += v
                                * ((time - current.time).clamp(0., p.extrapolation) / 1000.) as f32;
                        }
                    }
                    // A single bounded correction when starvation ends, shared by
                    // all persistent IDs (including held swords). Ordinary packets
                    // never restart it. New topology/teleports always snap.
                    if future.is_some() && !compatible_bracket {
                        p.corrections.remove(&id);
                    }
                    if begin_recovery
                        && (future.is_none() || compatible_bracket)
                        && !current.breaks.contains(&id)
                        && !crossed_breaks.contains(&id)
                    {
                        if let Some((_, displayed)) = displayed
                            .get(&id)
                            .and_then(|object| compatible(source, object, p.max_distance))
                        {
                            p.corrections.insert(
                                id,
                                PoseCorrection {
                                    position: displayed.position - result.position,
                                    rotation: displayed.rotation * result.rotation.inverse(),
                                    scale: displayed.scale / result.scale,
                                    elapsed: 0.,
                                },
                            );
                        }
                    }
                    if let Some(correction) = p.corrections.get(&id) {
                        let weight = (1. - correction.elapsed / 0.08).clamp(0., 1.);
                        result.position += correction.position * weight;
                        result.rotation =
                            Quat::IDENTITY.slerp(correction.rotation, weight) * result.rotation;
                        result.scale *= Vec3::ONE.lerp(correction.scale, weight);
                    }
                    if write_pose(object, source, result) {
                        changed = true;
                        static_changed |= object.render_options.is_static;
                        static_shadow |=
                            object.render_options.is_static && object.render_options.casts_shadow;
                        dynamic_shadow |=
                            !object.render_options.is_static && object.render_options.casts_shadow;
                    }
                }
                p.corrections
                    .retain(|_, correction| correction.elapsed < 0.08);
                self.render_revision = self.render_revision.wrapping_add(changed as u64);
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
        self.scene_playback = p;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn capture(
        source: &mut SceneManager,
        publisher: &mut WirePublisher,
        id: u32,
        x: f32,
        rotation: f32,
    ) -> Vec<u8> {
        source.set_object_transform(id, Vec3::X * x, Vec3::Y * rotation, Vec3::ONE);
        publisher.capture(source).unwrap()
    }
    fn x(scene: &SceneManager, id: u32) -> f32 {
        pose(&scene.objects[&id]).unwrap().position.x
    }
    fn mirror() -> SceneManager {
        let mut scene = SceneManager::default();
        scene.set_scene_playback(100., 80., 2.).unwrap();
        scene
    }
    #[test]
    fn playback_source_time_is_independent_of_arrival_and_render_partition() {
        let mut source = SceneManager::default();
        let id = source.add_cube(Vec3::ZERO);
        let mut publisher = WirePublisher::default();
        let frames: Vec<_> = (0..5)
            .map(|i| capture(&mut source, &mut publisher, id, i as f32 * 0.1, 0.))
            .collect();
        let mut a = mirror();
        let mut b = mirror();
        for (i, frame) in frames.iter().enumerate() {
            a.enqueue_scene_publication(frame, i as f64 * 100., &[])
                .unwrap();
        }
        b.enqueue_scene_publication(&frames[0], 0., &[]).unwrap();
        b.update(0.05);
        for (i, frame) in frames.iter().enumerate().skip(1) {
            b.enqueue_scene_publication(frame, i as f64 * 100., &[])
                .unwrap();
        }
        a.update(0.35);
        for _ in 0..6 {
            b.update(0.05);
        }
        assert!((x(&a, id) - 0.25).abs() < 1e-5);
        assert!((x(&a, id) - x(&b, id)).abs() < 1e-5);
        assert_eq!(a.wire_publication_sequence(), 3);
        let old_time = a.scene_playback.time;
        a.set_scene_playback(150., 80., 2.).unwrap();
        assert_eq!(a.scene_playback.time, old_time);
    }
    #[test]
    fn playback_startup_backlog_converges_without_an_underrun() {
        let mut source = SceneManager::default();
        let id = source.add_cube(Vec3::ZERO);
        let mut publisher = WirePublisher::default();
        let mut m = mirror();
        // Startup uploads lose 250ms while snapshots continue arriving.
        for i in 0..6 {
            let f = capture(&mut source, &mut publisher, id, 0., 0.);
            m.enqueue_scene_publication(&f, i as f64 * 50., &[])
                .unwrap();
        }
        for i in 6..126 {
            let f = capture(&mut source, &mut publisher, id, 0., 0.);
            m.enqueue_scene_publication(&f, i as f64 * 50., &[])
                .unwrap();
            let before = m.scene_playback.time.unwrap();
            m.update(0.05);
            let step = m.scene_playback.time.unwrap() - before;
            assert!((44.99..=55.01).contains(&step), "no time jump: {step}");
        }
        let p = &m.scene_playback;
        let lag = p.last_time.unwrap() - p.time.unwrap();
        assert!(lag <= p.delay + 60., "backlog must converge: {lag}");
        assert_eq!(p.underruns, 0);
        assert!(p.error.is_none());
    }
    #[test]
    fn playback_rotation_reversal_and_teleport_are_guarded() {
        let mut source = SceneManager::default();
        let id = source.add_cube(Vec3::ZERO);
        let mut publisher = WirePublisher::default();
        let mut m = mirror();
        for (i, x, r) in [(0, 0., 0.), (1, 0.1, 0.5), (2, 0., 1.), (3, 10., 1.5)] {
            let frame = capture(&mut source, &mut publisher, id, x, r);
            m.enqueue_scene_publication(&frame, i as f64 * 100., &[])
                .unwrap();
        }
        m.update(0.25);
        assert!((0. ..=0.1).contains(&x(&m, id)));
        let r = pose(&m.objects[&id]).unwrap().rotation;
        assert!(r.dot(Quat::from_rotation_y(0.75)).abs() > 0.9999);
        m.update(0.1);
        assert!(x(&m, id).abs() < 1e-5);
        m.update(0.05);
        assert!((x(&m, id) - 10.).abs() < 1e-5);
    }
    #[test]
    fn playback_topology_and_material_wait_for_boundary() {
        let mut source = SceneManager::default();
        let parent = source.add_cube(Vec3::ZERO);
        let mut publisher = WirePublisher::default();
        let mut m = mirror();
        let first = publisher.capture(&mut source).unwrap();
        m.enqueue_scene_publication(&first, 0., &[]).unwrap();
        source.remove_object(parent);
        let child = source.add_sphere(Vec3::X);
        let second = publisher.capture(&mut source).unwrap();
        m.enqueue_scene_publication(&second, 100., &[]).unwrap();
        m.update(0.15);
        assert!(m.objects.contains_key(&parent));
        assert!(!m.objects.contains_key(&child));
        m.update(0.05);
        assert!(!m.objects.contains_key(&parent));
        assert!(m.objects.contains_key(&child));
        assert!(m.scene_playback.error.is_none());
    }
    #[test]
    fn playback_gap_clock_is_bounded_and_long_gap_rebuffers() {
        let mut source = SceneManager::default();
        let id = source.add_cube(Vec3::ZERO);
        let mut publisher = WirePublisher::default();
        let mut m = mirror();
        for i in 0..2 {
            let f = capture(&mut source, &mut publisher, id, i as f32 * 0.1, 0.);
            m.enqueue_scene_publication(&f, i as f64 * 100., &[])
                .unwrap();
        }
        m.update(2.);
        assert_eq!(m.scene_playback.time, Some(180.));
        // Cubes/swords are not prediction eligible.
        assert!((x(&m, id) - 0.1).abs() < 1e-5);
        let f = capture(&mut source, &mut publisher, id, 0.2, 0.);
        m.enqueue_scene_publication(&f, 5000., &[]).unwrap();
        assert_eq!(m.scene_playback.rebufferings, 1);
        m.update(0.1);
        assert_eq!(m.wire_publication_sequence(), 3);
        assert!((x(&m, id) - 0.2).abs() < 1e-5);
    }
    #[test]
    fn playback_overflow_rejects_without_losing_delta_sequence_and_reset_preserves_legacy() {
        let mut source = SceneManager::default();
        source.add_cube(Vec3::ZERO);
        let mut publisher = WirePublisher::default();
        let mut m = mirror();
        for i in 0..MAX_FRAMES {
            let frame = publisher.capture(&mut source).unwrap();
            m.enqueue_scene_publication(&frame, i as f64, &[]).unwrap();
        }
        let next = publisher.capture(&mut source).unwrap();
        assert!(m
            .enqueue_scene_publication(&next, MAX_FRAMES as f64, &[])
            .unwrap_err()
            .contains("overflow"));
        assert_eq!(m.scene_playback.queue.len(), MAX_FRAMES);
        m.update(1.);
        m.enqueue_scene_publication(&next, MAX_FRAMES as f64, &[])
            .unwrap();
        m.set_scene_playback(0., 80., 2.).unwrap();
        assert!(!m.scene_playback.enabled());
        assert!(m.scene_playback.queue.is_empty());
        let fresh = publisher.capture(&mut source).unwrap();
        // Pending next was deliberately reset: the wire API still rejects gaps.
        assert!(m.apply_wire_publication(&fresh, &[]).is_err());
        m.apply_wire_publication(&next, &[]).unwrap();
        m.apply_wire_publication(&fresh, &[]).unwrap();
    }
    #[test]
    fn playback_fruit_extrapolation_is_short_and_recovery_clock_is_continuous() {
        let mut source = SceneManager::default();
        let mesh = crate::cutting::CutMesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
            &[0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3],
            Vec3::ONE,
        )
        .unwrap();
        let id = source.add_triangle_mesh(mesh).unwrap();
        source.set_cuttable(id, true, Vec3::ONE).unwrap();
        let mut publisher = WirePublisher::default();
        let mut m = mirror();
        for i in 0..2 {
            let f = capture(&mut source, &mut publisher, id, i as f32 * 0.1, 0.);
            m.enqueue_scene_publication(&f, i as f64 * 100., &[])
                .unwrap();
        }
        m.update(0.5);
        assert_eq!(m.scene_playback.time, Some(180.));
        assert!((x(&m, id) - 0.18).abs() < 1e-5);
        let before = x(&m, id);
        let f = capture(&mut source, &mut publisher, id, 0.2, 0.);
        m.enqueue_scene_publication(&f, 200., &[]).unwrap();
        // Receiving a packet does not alter the displayed pose or source clock.
        assert_eq!(m.scene_playback.time, Some(180.));
        assert_eq!(x(&m, id), before);
        m.update(0.01);
        let advance = m.scene_playback.time.unwrap() - 180.;
        assert!((9. - 1e-4..=11. + 1e-4).contains(&advance));
        assert!((x(&m, id) - before).abs() < 0.012);
    }
    #[test]
    fn playback_held_sword_recovery_is_bounded_and_not_restarted_by_packets() {
        let mut source = SceneManager::default();
        let id = source.add_cube(Vec3::ZERO);
        let mut publisher = WirePublisher::default();
        let mut m = mirror();
        for i in 0..2 {
            let f = capture(
                &mut source,
                &mut publisher,
                id,
                i as f32 * 0.1,
                i as f32 * 0.1,
            );
            m.enqueue_scene_publication(&f, i as f64 * 100., &[])
                .unwrap();
        }
        m.update(0.5);
        let held = pose(&m.objects[&id]).unwrap();
        let f = capture(&mut source, &mut publisher, id, 0.2, 0.2);
        m.enqueue_scene_publication(&f, 200., &[]).unwrap();
        m.update(0.001);
        assert!((x(&m, id) - held.position.x).abs() < 1e-5);
        assert!(
            pose(&m.objects[&id])
                .unwrap()
                .rotation
                .dot(held.rotation)
                .abs()
                > 0.99999
        );
        let mut previous = x(&m, id);
        for i in 0..9 {
            if i == 2 {
                let f = capture(&mut source, &mut publisher, id, 0.3, 0.3);
                m.enqueue_scene_publication(&f, 300., &[]).unwrap();
            }
            m.update(0.01);
            assert!((x(&m, id) - previous).abs() < 0.025);
            previous = x(&m, id);
        }
        assert!(m.scene_playback.corrections.is_empty());
        assert!((x(&m, id) - m.scene_playback.time.unwrap() as f32 / 1000.).abs() < 1e-4);
    }
    #[test]
    fn playback_overdue_fifty_ms_packets_preserve_held_pose_before_commit() {
        for with_future in [false, true] {
            let mut source = SceneManager::default();
            let id = source.add_cube(Vec3::ZERO);
            let mut publisher = WirePublisher::default();
            let mut m = mirror();
            for i in 0..2 {
                let f = capture(
                    &mut source,
                    &mut publisher,
                    id,
                    i as f32 * 0.05,
                    i as f32 * 0.1,
                );
                m.enqueue_scene_publication(&f, i as f64 * 50., &[])
                    .unwrap();
            }
            m.update(0.5);
            assert_eq!(m.scene_playback.time, Some(130.));
            let held = pose(&m.objects[&id]).unwrap();
            let due = capture(&mut source, &mut publisher, id, 0.1, 0.2);
            m.enqueue_scene_publication(&due, 100., &[]).unwrap();
            if with_future {
                let future = capture(&mut source, &mut publisher, id, 0.15, 0.3);
                m.enqueue_scene_publication(&future, 150., &[]).unwrap();
            }
            m.update(0.001);
            assert_eq!(m.wire_publication_sequence(), 3);
            assert!(
                (x(&m, id) - held.position.x).abs() < 1e-5,
                "future={with_future}"
            );
            assert!(
                pose(&m.objects[&id])
                    .unwrap()
                    .rotation
                    .dot(held.rotation)
                    .abs()
                    > 0.99999
            );
            assert_eq!(m.scene_playback.corrections[&id].elapsed, 0.);
            if !with_future {
                let future = capture(&mut source, &mut publisher, id, 0.15, 0.3);
                m.enqueue_scene_publication(&future, 150., &[]).unwrap();
            }
            for i in 0..9 {
                m.update(0.01);
                if i < 7 {
                    assert!(
                        (m.scene_playback.corrections[&id].elapsed - (i + 1) as f32 * 0.01).abs()
                            < 1e-5
                    );
                }
            }
            assert!(m.scene_playback.corrections.is_empty());
            assert!((x(&m, id) - 0.15).abs() < 1e-5);
        }
    }
    #[test]
    fn playback_short_underrun_recovers_swords_and_fruit_before_cap() {
        for fruit in [false, true] {
            let mut source = SceneManager::default();
            let id = if fruit {
                let mesh = crate::cutting::CutMesh::new(
                    vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
                    &[0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3],
                    Vec3::ONE,
                )
                .unwrap();
                let id = source.add_triangle_mesh(mesh).unwrap();
                source.set_cuttable(id, true, Vec3::ONE).unwrap();
                id
            } else {
                source.add_cube(Vec3::ZERO)
            };
            let mut publisher = WirePublisher::default();
            let mut m = mirror();
            for i in 0..2 {
                let f = capture(
                    &mut source,
                    &mut publisher,
                    id,
                    i as f32 * 0.1,
                    i as f32 * 0.1,
                );
                m.enqueue_scene_publication(&f, i as f64 * 100., &[])
                    .unwrap();
            }
            m.update(0.24);
            assert!((m.scene_playback.time.unwrap() - 140.).abs() < 1e-4);
            let held = pose(&m.objects[&id]).unwrap();
            // Reversal makes old fruit extrapolation differ from the new bracket.
            let f = capture(&mut source, &mut publisher, id, 0.05, 0.2);
            m.enqueue_scene_publication(&f, 150., &[]).unwrap();
            m.update(0.001);
            let recovered = pose(&m.objects[&id]).unwrap();
            assert!(
                (recovered.position - held.position).length() < 1e-5,
                "fruit={fruit}"
            );
            assert!(recovered.rotation.dot(held.rotation).abs() > 0.99999);
            assert_eq!(m.scene_playback.corrections[&id].elapsed, 0.);
        }
    }
    #[test]
    fn playback_active_recovery_does_not_cross_intermediate_drained_teleport() {
        let mut source = SceneManager::default();
        let id = source.add_cube(Vec3::ZERO);
        let mut publisher = WirePublisher::default();
        let mut m = mirror();
        for i in 0..2 {
            let f = capture(&mut source, &mut publisher, id, i as f32 * 0.1, 0.);
            m.enqueue_scene_publication(&f, i as f64 * 100., &[])
                .unwrap();
        }
        m.update(0.24);
        for (time, x) in [(150., 0.15), (160., 0.16)] {
            let f = capture(&mut source, &mut publisher, id, x, 0.);
            m.enqueue_scene_publication(&f, time, &[]).unwrap();
        }
        m.update(0.001);
        assert!(m.scene_playback.corrections.contains_key(&id));
        for (time, x) in [(170., 10.), (180., 10.01)] {
            let f = capture(&mut source, &mut publisher, id, x, 0.);
            m.enqueue_scene_publication(&f, time, &[]).unwrap();
        }
        m.update(0.05);
        assert_eq!(m.wire_publication_sequence(), 6);
        assert!(m.scene_playback.corrections.is_empty());
        assert!((x(&m, id) - 10.01).abs() < 1e-5);
    }
    #[test]
    fn playback_retained_cut_lineage_is_not_a_discontinuity() {
        let mut source = SceneManager::default();
        let id = source.add_cube(Vec3::ZERO);
        source.cut_lineage.insert(id, (99999, Mat4::IDENTITY));
        let mut publisher = WirePublisher::default();
        let mut m = mirror();
        for i in 0..4 {
            let f = capture(&mut source, &mut publisher, id, i as f32 * 0.1, 0.);
            m.enqueue_scene_publication(&f, i as f64 * 100., &[])
                .unwrap();
        }
        m.update(0.35);
        assert!((x(&m, id) - 0.25).abs() < 1e-5);
        assert!(m.scene_playback.error.is_none());
    }
    #[test]
    fn playback_material_changes_only_on_boundary() {
        let mut source = SceneManager::default();
        let id = source.add_cube(Vec3::ZERO);
        let mut publisher = WirePublisher::default();
        let mut m = mirror();
        let f = publisher.capture(&mut source).unwrap();
        m.enqueue_scene_publication(&f, 0., &[]).unwrap();
        source.set_emission(id, 7.);
        let f = publisher.capture(&mut source).unwrap();
        m.enqueue_scene_publication(&f, 100., &[]).unwrap();
        m.update(0.15);
        assert_ne!(m.objects[&id].emission, 7.);
        m.update(0.05);
        assert_eq!(m.objects[&id].emission, 7.);
    }
    #[test]
    fn playback_bad_reference_halts_with_visible_error() {
        let mut source = SceneManager::default();
        source.add_cube(Vec3::ZERO);
        let mut publisher = WirePublisher::default();
        let bytes = publisher.capture(&mut source).unwrap();
        let mut m = mirror();
        m.enqueue_scene_publication(&bytes, 0., &[]).unwrap();
        m.scene_playback.queue.front_mut().unwrap().frame.mirror = Some(99999);
        m.update(0.1);
        assert!(m
            .scene_playback
            .error
            .as_deref()
            .unwrap()
            .contains("tagged object"));
        assert_eq!(m.wire_publication_sequence(), 0);
        assert!(m.enqueue_scene_publication(&bytes, 1., &[]).is_err());
    }
}
