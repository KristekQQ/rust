//! Explicit, bounded presentation effects. Geometry is allocated once; frame updates
//! touch transforms/emission only. This child module shares SceneManager ownership.
use super::*;

pub const DURATION: f32 = 0.65;
const HIDDEN: Vec3 = Vec3::new(0., -10000., 0.);
const DEFAULT_LABEL: &str = "CISTY REZ";
#[derive(Clone, Copy)]
struct PieceMotion {
    id: u32,
    start: Transform,
    pivot: Vec3,
    velocity: Vec3,
    angular: Vec3,
    angular_limit: f32,
    cap_normal: Vec3,
}
#[derive(Clone, Copy)]
struct Burst {
    mirror_text: bool,
    ids: [u32; 8],
    center: Vec3,
    orientation: glam::Quat,
    radius: f32,
    strength: f32,
    title_height: f32,
    elapsed: f32,
    pieces: [Option<PieceMotion>; 32],
}
#[derive(Clone, Copy)]
struct Trail {
    sword: u32,
    ids: [u32; 3],
    enabled: bool,
    previous: Option<[Vec3; 2]>,
    poses: [Option<Transform>; 3],
    ages: [f32; 3],
}
#[derive(Default)]
pub(super) struct Feedback {
    bursts: [Option<Burst>; 2],
    next: usize,
    trails: [Option<Trail>; 2],
    retired_pieces: Vec<u32>,
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const TEXTURE_SIZE: u32 = 256;

/// Rasterized once per GPU renderer; smooth alpha, broken energy arcs and a soft rim.
/// The transparent image is sampled by the dedicated depth-tested FX material.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn ring_texture() -> Vec<u8> {
    let mut pixels = Vec::with_capacity((TEXTURE_SIZE * TEXTURE_SIZE * 4) as usize);
    for y in 0..TEXTURE_SIZE {
        for x in 0..TEXTURE_SIZE {
            let u = (x as f32 + 0.5) / TEXTURE_SIZE as f32 * 2. - 1.;
            let v = (y as f32 + 0.5) / TEXTURE_SIZE as f32 * 2. - 1.;
            let r = u.hypot(v);
            let a = v.atan2(u);
            let gaussian = |center: f32, width: f32| (-((r - center) / width).powi(2)).exp();
            let noise = (a * 17. + (a * 7.).sin() * 2. + r * 24.).sin() * 0.5 + 0.5;
            let arcs = (a * 9. + r * 19. + (a * 3.).sin()).sin().max(0.).powi(3);
            let filaments = gaussian(0.79 + (a * 11.).sin() * 0.018, 0.013) * (0.25 + 0.75 * arcs);
            let rim = gaussian(0.87 + (a * 7.).sin() * 0.006, 0.017);
            let mist = gaussian(0.76, 0.12) * (0.18 + 0.22 * noise);
            let streaks = gaussian(0.68, 0.10) * arcs * 0.3;
            let boundary = ((r - 0.465) / 0.09).clamp(0., 1.) * ((1. - r) / 0.09).clamp(0., 1.);
            let alpha = ((rim * 0.9 + filaments * 0.6 + mist + streaks) * boundary).clamp(0., 0.95);
            let hot = (rim + filaments).clamp(0., 1.);
            pixels.extend_from_slice(&[
                ((0.08 + hot * 0.65) * 255.) as u8,
                ((0.55 + hot * 0.45) * 255.) as u8,
                255,
                (alpha * 255.) as u8,
            ]);
        }
    }
    pixels
}

fn ring() -> Result<crate::cutting::CutMesh, String> {
    // One broad, flat shockwave disc with an open center, aligned to the actual cut plane.
    let mut p = Vec::with_capacity(128);
    let mut indices = Vec::with_capacity(768);
    for i in 0..32 {
        for j in 0..4 {
            let a = i as f32 * std::f32::consts::TAU / 32.;
            // Rectangular cross section: broad annular faces, thin depth.
            let (r, z) = [(1.08, 0.007), (0.5, 0.007), (0.5, -0.007), (1.08, -0.007)][j];
            p.push(Vec3::new(r * a.cos(), r * a.sin(), z));
            let v = (i * 4 + j) as u32;
            let n = (((i + 1) % 32) * 4 + j) as u32;
            let w = (i * 4 + (j + 1) % 4) as u32;
            let q = (((i + 1) % 32) * 4 + (j + 1) % 4) as u32;
            indices.extend_from_slice(&[v, n, w, w, n, q]);
        }
    }
    let mut mesh = crate::cutting::CutMesh::new(p, &indices, Vec3::new(0.1, 1., 0.85))?;
    for (i, pair) in mesh.triangles.chunks_mut(2).enumerate() {
        let color = match i % 4 {
            3 => Vec3::new(0.5, 1., 1.),     // bright outer edge
            1 => Vec3::new(0.02, 0.2, 0.28), // softer inner boundary
            _ => Vec3::new(0.025, 0.55, 0.7),
        };
        for triangle in pair {
            triangle.color = color;
        }
    }
    Ok(mesh)
}
fn diamond(color: Vec3) -> Result<crate::cutting::CutMesh, String> {
    crate::cutting::CutMesh::new(
        vec![Vec3::X, Vec3::Y, -Vec3::X, -Vec3::Y, Vec3::Z, -Vec3::Z],
        &[
            0, 1, 4, 1, 2, 4, 2, 3, 4, 3, 0, 4, 1, 0, 5, 2, 1, 5, 3, 2, 5, 0, 3, 5,
        ],
        color,
    )
}
impl SceneManager {
    /// Capture real tracked blade speed on the resulting fragments, before JS receives the cut.
    pub(crate) fn set_cut_feedback_speed(&mut self, pieces: &[u32], speed: f32) {
        if !speed.is_finite() || speed < 0. {
            return;
        }
        for id in pieces {
            if let Some(fragment) = self.cut_fragments.get_mut(id) {
                fragment.cut_speed = speed;
            }
        }
    }
    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn is_cut_feedback_ring(&self, id: u32) -> bool {
        self.feedback
            .bursts
            .iter()
            .flatten()
            .any(|burst| burst.ids[0] == id)
    }
    fn park_feedback(&mut self, id: u32) {
        self.set_object_position(id, HIDDEN);
        self.set_emission(id, 0.);
    }
    fn effect_mesh(&mut self, mesh: crate::cutting::CutMesh) -> Result<u32, String> {
        let id = self.add_triangle_mesh(mesh)?;
        self.set_render_options(
            id,
            RenderOptions {
                casts_shadow: false,
                receives_shadow: false,
                ..Default::default()
            },
        );
        self.park_feedback(id);
        Ok(id)
    }
    /// Atomically reject stale/non-mesh/duplicate inputs before touching the scene.
    pub fn play_cut_feedback(&mut self, pieces: &[u32]) -> Result<Vec<u32>, String> {
        self.play_cut_feedback_facing(pieces, false)
    }
    pub fn play_cut_feedback_sized(
        &mut self,
        pieces: &[u32],
        mirror_text: bool,
        size: f32,
    ) -> Result<Vec<u32>, String> {
        self.play_cut_feedback_labeled(pieces, mirror_text, size, None)
    }
    /// Emit a bounded hit popup, replacing the default title only for this hit.
    pub fn play_cut_feedback_labeled(
        &mut self,
        pieces: &[u32],
        mirror_text: bool,
        size: f32,
        label: Option<&str>,
    ) -> Result<Vec<u32>, String> {
        if label.is_some_and(|label| {
            !(1..=32).contains(&label.len())
                || !label.bytes().all(|byte| (b' '..=b'~').contains(&byte))
        }) {
            return Err("Cut feedback label requires 1..32 printable ASCII characters".into());
        }
        if !size.is_finite() || !(0.01..=2.).contains(&size) {
            return Err("Cut feedback size must be finite in 0.01..2".into());
        }
        let ids = self.play_cut_feedback_with_label(pieces, mirror_text, label)?;
        let fragments = &self.cut_fragments;
        for burst in self.feedback.bursts.iter_mut().flatten() {
            if ids.contains(&burst.ids[0]) {
                if burst
                    .pieces
                    .iter()
                    .flatten()
                    .any(|piece| fragments.contains_key(&piece.id))
                {
                    // The user's size setting controls gentle hits only. Never suppress
                    // the upper end of a real fast stroke with that same multiplier.
                    let attack = ((burst.strength - 0.12) / 1.88).clamp(0., 1.);
                    burst.strength = 0.12 * size + (2. - 0.12 * size) * attack;
                } else {
                    burst.strength *= size;
                }
            }
        }
        self.update_cut_feedback(0.);
        Ok(ids)
    }
    /// Emit one popup per cut family, using the score already awarded to its
    /// original target. Missing or zero rewards retain the impact effect without a score caption.
    pub fn play_cut_feedback_scored(
        &mut self,
        pieces: &[u32],
        mirror_text: bool,
        size: f32,
        rewards: &[(u32, u64)],
    ) -> Result<Vec<u32>, String> {
        if !size.is_finite() || !(0.01..=2.).contains(&size) {
            return Err("Cut feedback size must be finite in 0.01..2".into());
        }
        if rewards.len() > 32
            || rewards
                .iter()
                .enumerate()
                .any(|(i, (id, _))| rewards[..i].iter().any(|(other, _)| other == id))
        {
            return Err("Cut feedback rewards require at most 32 unique original IDs".into());
        }
        if !(2..=32).contains(&pieces.len())
            || pieces.iter().enumerate().any(|(i, id)| {
                pieces[..i].contains(id)
                    || !self.triangle_meshes.contains_key(id)
                    || self
                        .feedback
                        .bursts
                        .iter()
                        .flatten()
                        .any(|burst| burst.ids.contains(id))
                    || self
                        .feedback
                        .trails
                        .iter()
                        .flatten()
                        .any(|trail| trail.ids.contains(id))
            })
        {
            return Err("Cut feedback requires 2..32 unique live solid pieces".into());
        }
        let mut groups = BTreeMap::<u64, Vec<u32>>::new();
        for &id in pieces {
            self.triangle_meshes[&id].mesh.cpu()?;
            let family = self
                .cut_fragments
                .get(&id)
                .map_or(0, |fragment| fragment.family);
            groups.entry(family).or_default().push(id);
        }
        let mut objects = std::collections::BTreeSet::new();
        for group in groups.values() {
            let parent = self
                .cut_fragments
                .get(&group[0])
                .map(|fragment| fragment.parent);
            let reward = parent
                .and_then(|parent| {
                    rewards
                        .iter()
                        .find(|(id, _)| *id == parent)
                        .map(|(_, value)| *value)
                })
                .unwrap_or(0);
            let label = if reward == 0 {
                String::new()
            } else {
                format!("+{reward}")
            };
            let ids = self.emit_cut_feedback_group(group, mirror_text, Some(&label))?;
            let fragments = &self.cut_fragments;
            for burst in self.feedback.bursts.iter_mut().flatten() {
                if ids.contains(&burst.ids[0]) {
                    if burst
                        .pieces
                        .iter()
                        .flatten()
                        .any(|piece| fragments.contains_key(&piece.id))
                    {
                        let attack = ((burst.strength - 0.12) / 1.88).clamp(0., 1.);
                        burst.strength = 0.12 * size + (2. - 0.12 * size) * attack;
                    } else {
                        burst.strength *= size;
                    }
                }
            }
            objects.extend(ids);
        }
        self.update_cut_feedback(0.);
        Ok(objects.into_iter().collect())
    }
    pub fn play_cut_feedback_facing(
        &mut self,
        pieces: &[u32],
        mirror_text: bool,
    ) -> Result<Vec<u32>, String> {
        self.play_cut_feedback_with_label(pieces, mirror_text, None)
    }
    fn play_cut_feedback_with_label(
        &mut self,
        pieces: &[u32],
        mirror_text: bool,
        label: Option<&str>,
    ) -> Result<Vec<u32>, String> {
        if !(2..=32).contains(&pieces.len())
            || pieces.iter().enumerate().any(|(i, id)| {
                pieces[..i].contains(id)
                    || !self.triangle_meshes.contains_key(id)
                    || self
                        .feedback
                        .bursts
                        .iter()
                        .flatten()
                        .any(|b| b.ids.contains(id))
                    || self
                        .feedback
                        .trails
                        .iter()
                        .flatten()
                        .any(|t| t.ids.contains(id))
            })
        {
            return Err("Cut feedback requires 2..32 unique live solid pieces".into());
        }
        // One stroke can cut several fruit. Never put their feedback at an averaged
        // point between unrelated originals: each real fragment family has its own hit.
        let mut groups = BTreeMap::<u64, Vec<u32>>::new();
        for &id in pieces {
            self.triangle_meshes[&id].mesh.cpu()?;
            let family = self
                .cut_fragments
                .get(&id)
                .map_or(0, |fragment| fragment.family);
            groups.entry(family).or_default().push(id);
        }
        let mut objects = std::collections::BTreeSet::new();
        for group in groups.values() {
            objects.extend(self.emit_cut_feedback_group(group, mirror_text, label)?);
        }
        Ok(objects.into_iter().collect())
    }
    fn emit_cut_feedback_group(
        &mut self,
        pieces: &[u32],
        mirror_text: bool,
        label: Option<&str>,
    ) -> Result<Vec<u32>, String> {
        let mut motions = [None; 32];
        let mut weighted = Vec3::ZERO;
        let mut weight = 0.;
        let mut plane_normal = Vec3::ZERO;
        let mut cap_center = Vec3::ZERO;
        let mut cap_weight = 0.;
        let mut world_min = Vec3::splat(f32::INFINITY);
        let mut world_max = Vec3::splat(f32::NEG_INFINITY);
        for (i, &id) in pieces.iter().enumerate() {
            let object = &self.objects[&id];
            let triangle_object = &self.triangle_meshes[&id];
            let mesh = triangle_object.mesh.cpu()?;
            let matrix = object.transform.model();
            for vertex in &mesh.positions {
                let point = matrix.transform_point3(*vertex);
                world_min = world_min.min(point);
                world_max = world_max.max(point);
            }
            // Generated flesh caps have their own authored material. Sum their
            // oriented triangle areas before applying the inverse transpose.
            let mut cap_area = Vec3::ZERO;
            for triangle in &mesh.triangles {
                if triangle.color.distance_squared(triangle_object.cap_color) < 1e-8 {
                    let [a, b, c] = triangle.indices.map(|i| mesh.positions[i as usize]);
                    cap_area += (b - a).cross(c - a);
                    let [wa, wb, wc] = [a, b, c].map(|p| matrix.transform_point3(p));
                    let area = (wb - wa).cross(wc - wa).length();
                    cap_center += (wa + wb + wc) / 3. * area;
                    cap_weight += area;
                }
            }
            let cap_normal = normal_matrix(matrix)
                .transform_vector3(cap_area)
                .normalize_or_zero();
            if plane_normal == Vec3::ZERO && cap_normal != Vec3::ZERO {
                plane_normal = cap_normal;
            }
            let pivot = mesh.centroid();
            let center = object.transform.model().transform_point3(pivot);
            let volume =
                (mesh.volume() * f64::from(object.transform.scale.element_product().abs())) as f32;
            weighted += center * volume;
            weight += volume;
            motions[i] = Some(PieceMotion {
                id,
                start: object.transform,
                pivot,
                velocity: self.projectiles.get(&id).map_or(Vec3::ZERO, |body| body.0),
                angular: Vec3::ZERO,
                angular_limit: f32::INFINITY,
                cap_normal,
            });
        }
        let center = if cap_weight > 0. {
            cap_center / cap_weight
        } else {
            weighted / weight
        };
        let plane_normal = if plane_normal != Vec3::ZERO {
            plane_normal
        } else {
            Vec3::Z
        };
        let orientation = glam::Quat::from_rotation_arc(Vec3::Z, plane_normal);
        // Fit in the cut plane, not the camera plane. Cap centroids anchor off-center cuts too.
        let mut silhouette_radius = 0.0_f32;
        for &id in pieces {
            let matrix = self.objects[&id].transform.model();
            for vertex in &self.triangle_meshes[&id].mesh.cpu()?.positions {
                let delta = matrix.transform_point3(*vertex) - center;
                silhouette_radius = silhouette_radius
                    .max((delta - plane_normal * delta.dot(plane_normal)).length());
            }
        }
        let radius = (silhouette_radius + 0.025).clamp(0.04, 2.0);
        let speed = pieces
            .iter()
            .filter_map(|id| self.cut_fragments.get(id))
            .map(|fragment| fragment.cut_speed)
            .reduce(f32::max);
        // Keep ordinary strokes restrained; reserve the big burst for forceful attacks.
        let strength = speed.map_or(0.2, |speed| {
            let attack = ((speed - 0.75) / 2.25).clamp(0., 1.);
            0.12 + 1.88 * attack * attack
        });
        let title_height = (world_max.y - center.y).clamp(0.0, 2.0) + 0.12;
        debug_assert!(world_min.is_finite() && world_max.is_finite());
        let slot = self.feedback.next;
        let mut burst = if let Some(old) = self.feedback.bursts[slot] {
            // Only emission rebuilds glyph geometry; frame updates retain this panel
            // and its object ID. Always restore the default after a labeled hit.
            if let Some(panel) = self.info_panels.get_mut(&old.ids[7]) {
                panel.mesh = crate::info_panel::InfoPanel::text(
                    label.unwrap_or(DEFAULT_LABEL),
                    &[],
                    panel.width,
                    panel.height,
                )?
                .mesh;
                panel.dirty = true;
                panel.revision = panel.revision.wrapping_add(1);
            }
            // Reclaim the previous burst's short lived fragments when the bounded slot wraps.
            for p in old.pieces.into_iter().flatten() {
                if !pieces.contains(&p.id) {
                    // The flash slot may be reused before real debris finishes falling.
                    if old.elapsed * p.angular.length() < p.angular_limit {
                        self.set_object_spin(p.id, p.angular);
                    }
                    if self.fading_pieces.contains_key(&p.id) {
                        continue;
                    }
                    self.schedule_remove_object(p.id, (1.5 - old.elapsed).max(0.01));
                    if self.feedback.retired_pieces.len() >= 64 {
                        let oldest = self.feedback.retired_pieces.remove(0);
                        self.remove_object(oldest);
                    }
                    self.feedback.retired_pieces.push(p.id);
                }
            }
            Burst {
                mirror_text,
                radius,
                strength,
                title_height,
                orientation,
                center,
                elapsed: 0.,
                pieces: motions,
                ..old
            }
        } else {
            let mut ids = [0; 8];
            let result = (|| {
                ids[0] = self.effect_mesh(ring()?)?;
                for id in &mut ids[1..7] {
                    *id = self.effect_mesh(diamond(Vec3::new(0.75, 0.22, 1.))?)?;
                }
                ids[7] = self.add_text_panel(label.unwrap_or(DEFAULT_LABEL), &[], 0.55, 0.24)?;
                self.park_feedback(ids[7]);
                Ok::<(), String>(())
            })();
            if let Err(e) = result {
                for id in ids {
                    if id != 0 {
                        self.remove_object(id);
                    }
                }
                return Err(e);
            }
            Burst {
                mirror_text,
                radius,
                strength,
                title_height,
                ids,
                orientation,
                center,
                elapsed: 0.,
                pieces: motions,
            }
        };
        for (i, p) in burst.pieces.iter_mut().enumerate() {
            if let Some(p) = p {
                let c = p.start.model().transform_point3(p.pivot);
                let radial = (c - center).try_normalize().unwrap_or(if i % 2 == 0 {
                    -Vec3::X
                } else {
                    Vec3::X
                });
                let direction = if p.cap_normal != Vec3::ZERO {
                    -p.cap_normal
                } else {
                    radial
                };
                p.velocity = (p.velocity + direction * 0.6 + Vec3::Y * 0.24).clamp_length_max(7.);
                if p.cap_normal != Vec3::ZERO {
                    let cross = p.cap_normal.cross(Vec3::Z);
                    let axis = cross.try_normalize().unwrap_or(if p.cap_normal.z < 0. {
                        Vec3::Y
                    } else {
                        Vec3::ZERO
                    });
                    p.angular = axis * 2.4;
                    p.angular_limit = p.cap_normal.dot(Vec3::Z).clamp(-1., 1.).acos();
                } else {
                    p.angular = Vec3::new(radial.x * 0.7, radial.x * 1.3, radial.x * 0.5);
                }
                self.set_projectile(p.id, p.velocity, 9.81, 0.45);
                self.spins.remove(&p.id);
                self.cutting_scatter.remove(&p.id);
                self.blade_cut_states.remove(&p.id);
                self.remove_collider(p.id);
                self.triangle_meshes.get_mut(&p.id).unwrap().cuttable = false;
            }
        }
        // A repeated command must not leave a second animator owning the same piece.
        for previous in self.feedback.bursts.iter_mut().flatten() {
            for p in &mut previous.pieces {
                if p.is_some_and(|p| pieces.contains(&p.id)) {
                    *p = None;
                }
            }
        }
        let ids = burst.ids.to_vec();
        self.feedback.bursts[slot] = Some(burst);
        self.feedback.next = (slot + 1) % 2;
        self.update_cut_feedback(0.);
        Ok(ids)
    }
    /// Park visual effects and stop their fragment presentation immediately.
    pub fn clear_cut_feedback(&mut self) {
        let mut feedback = std::mem::take(&mut self.feedback);
        while let Some((&id, _)) = self.fading_pieces.first_key_value() {
            self.remove_object(id);
        }
        for id in feedback.retired_pieces.drain(..) {
            self.remove_object(id);
        }
        for b in feedback.bursts.iter_mut().flatten() {
            b.elapsed = 2.;
            for id in b.ids {
                self.park_feedback(id);
            }
            for p in &mut b.pieces {
                if let Some(p) = p.take() {
                    self.remove_object(p.id);
                }
            }
        }
        for trail in feedback.trails.iter_mut().flatten() {
            trail.previous = None;
            trail.poses = [None; 3];
            trail.ages = [1.; 3];
            for id in trail.ids {
                self.park_feedback(id);
            }
        }
        self.feedback = feedback;
    }
    pub(super) fn remove_blade_trail_binding(&mut self, sword: u32) {
        if let Some(slot) = self
            .feedback
            .trails
            .iter()
            .position(|t| t.is_some_and(|t| t.sword == sword))
        {
            let trail = self.feedback.trails[slot].take().unwrap();
            for id in trail.ids {
                self.remove_object(id);
            }
        }
    }
    pub(crate) fn update_cut_feedback(&mut self, dt: f32) {
        let mut feedback = std::mem::take(&mut self.feedback);
        feedback.retired_pieces.retain(|id| self.object_exists(*id));
        for b in feedback.bursts.iter_mut().flatten() {
            let before = b.elapsed;
            b.elapsed += dt;
            let t = (b.elapsed / DURATION).min(1.);
            if before <= DURATION {
                for (i, id) in b.ids.into_iter().enumerate() {
                    if t >= 1. {
                        self.park_feedback(id);
                        continue;
                    }
                    let (offset, mut scale) = if i == 0 {
                        // Fast initial spread, then ease out while the same disc fades.
                        let spread = 1.4 + 1.8 * (1. - (1. - t).powi(2));
                        (Vec3::ZERO, Vec3::splat(b.radius * spread * b.strength))
                    } else if i == 7 {
                        (
                            Vec3::new(0., b.title_height + 0.18 * t, 0.),
                            Vec3::splat(0.85 + 0.25 * t),
                        )
                    } else {
                        let a = (i - 1) as f32 * std::f32::consts::TAU / 6.;
                        (
                            b.orientation
                                * Vec3::new(a.cos(), a.sin(), 0.)
                                * (b.radius * (1.5 + 2.2 * t) * b.strength),
                            Vec3::splat(0.032 * (1. - 0.8 * t) * b.strength),
                        )
                    };
                    if i == 7 && b.mirror_text {
                        scale.x = -scale.x;
                    }
                    let rotation = if i == 0 {
                        let (y, x, z) = b.orientation.to_euler(glam::EulerRot::YXZ);
                        Vec3::new(x, y, z)
                    } else {
                        Vec3::ZERO
                    };
                    self.set_object_transform(id, b.center + offset, rotation, scale);
                    self.set_emission(id, 2.8 * (1. - t));
                }
            }
            for p in &mut b.pieces {
                if let Some(m) = *p {
                    if b.elapsed >= 1.5 && !self.fading_pieces.contains_key(&m.id) {
                        self.remove_object(m.id);
                        *p = None;
                        continue;
                    }
                    let Some(object) = self.objects.get(&m.id) else {
                        *p = None;
                        continue;
                    };
                    let mut transform = object.transform;
                    let pivot = transform.model().transform_point3(m.pivot);
                    if let Some(axis) = m.angular.try_normalize() {
                        let start = glam::Quat::from_euler(
                            glam::EulerRot::YXZ,
                            m.start.rotation.y,
                            m.start.rotation.x,
                            m.start.rotation.z,
                        );
                        let angle = (m.angular.length() * b.elapsed).min(m.angular_limit);
                        let rotation = glam::Quat::from_axis_angle(axis, angle) * start;
                        let (y, x, z) = rotation.to_euler(glam::EulerRot::YXZ);
                        transform.rotation = Vec3::new(x, y, z);
                    }
                    // Translation is integrated once by projectile physics, including all contacts.
                    transform.position = Vec3::ZERO;
                    transform.position = pivot - transform.model().transform_point3(m.pivot);
                    self.set_object_transform(
                        m.id,
                        transform.position,
                        transform.rotation,
                        transform.scale,
                    );
                }
            }
        }
        for trail in feedback.trails.iter_mut().flatten() {
            for i in 0..3 {
                trail.ages[i] += dt;
                if trail.ages[i] >= 0.06 {
                    if trail.poses[i].take().is_some() {
                        self.park_feedback(trail.ids[i]);
                    }
                } else if trail.poses[i].is_some() {
                    self.set_emission(trail.ids[i], 2. * (1. - trail.ages[i] / 0.06));
                }
            }
        }
        self.feedback = feedback;
    }
    pub fn set_blade_trail(&mut self, sword: u32, enabled: bool) -> Result<(), String> {
        if self.tracked_blade(sword).is_none() {
            return Err("Blade trail requires a live configured cutter".into());
        }
        if let Some(slot) = self
            .feedback
            .trails
            .iter()
            .position(|t| t.is_some_and(|t| t.sword == sword))
        {
            self.reset_blade_trail(sword);
            self.feedback.trails[slot].as_mut().unwrap().enabled = enabled;
            return Ok(());
        }
        if !enabled {
            return Ok(());
        }
        let slot = self
            .feedback
            .trails
            .iter()
            .position(Option::is_none)
            .ok_or("At most two blade trails")?;
        let mut ids = [0; 3];
        for (i, id) in ids.iter_mut().enumerate() {
            *id = self.effect_mesh(diamond(if i % 2 == 0 {
                Vec3::new(0.08, 0.9, 1.)
            } else {
                Vec3::new(0.65, 0.16, 1.)
            })?)?;
        }
        self.feedback.trails[slot] = Some(Trail {
            sword,
            ids,
            enabled,
            previous: None,
            poses: [None; 3],
            ages: [1.; 3],
        });
        Ok(())
    }
    pub(crate) fn reset_blade_trail(&mut self, sword: u32) {
        let Some(slot) = self
            .feedback
            .trails
            .iter()
            .position(|t| t.is_some_and(|t| t.sword == sword))
        else {
            return;
        };
        let trail = self.feedback.trails[slot].as_mut().unwrap();
        trail.previous = None;
        trail.poses = [None; 3];
        trail.ages = [1.; 3];
        for id in trail.ids {
            self.park_feedback(id);
        }
    }
    pub(crate) fn sample_blade_trail(&mut self, sword: u32, points: [Vec3; 2], dt: f32) {
        let Some(slot) = self
            .feedback
            .trails
            .iter()
            .position(|t| t.is_some_and(|t| t.sword == sword && t.enabled))
        else {
            return;
        };
        let mut trail = self.feedback.trails[slot].unwrap();
        let previous = trail.previous.replace(points);
        if let Some(previous) = previous {
            let distance = points[0]
                .distance(previous[0])
                .max(points[1].distance(previous[1]));
            if dt.is_finite() && dt > 0. && dt <= 0.1 && distance > 0.004 && distance < 0.5 {
                for i in 0..3 {
                    let alpha = (i + 1) as f32 / 4.;
                    let a = points[0].lerp(previous[0], alpha);
                    let b = points[1].lerp(previous[1], alpha);
                    let direction = (b - a).normalize_or_zero();
                    let q = glam::Quat::from_rotation_arc(Vec3::Z, direction);
                    let (y, x, z) = q.to_euler(glam::EulerRot::YXZ);
                    let transform = Transform::new(
                        (a + b) * 0.5,
                        Vec3::new(x, y, z),
                        Vec3::new(0.007 * (1. - alpha), 0.003, (b - a).length() * 0.5),
                    );
                    self.set_object_transform(
                        trail.ids[i],
                        transform.position,
                        transform.rotation,
                        transform.scale,
                    );
                    trail.poses[i] = Some(transform);
                    trail.ages[i] = alpha * 0.035;
                    self.set_emission(trail.ids[i], 2. * (1. - alpha));
                }
            }
        }
        self.feedback.trails[slot] = Some(trail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pieces(scene: &mut SceneManager) -> [u32; 2] {
        std::array::from_fn(|i| {
            let id = scene
                .add_triangle_mesh(diamond(Vec3::new(0.7, 0.2, 0.1)).unwrap())
                .unwrap();
            scene.set_object_transform(
                id,
                Vec3::new(i as f32 * 0.2, 1., -1.),
                Vec3::ZERO,
                Vec3::splat(0.1),
            );
            scene.set_color_cutout(id, 0.42);
            scene.set_emission(id, 0.7);
            id
        })
    }
    fn assert_label(scene: &SceneManager, id: u32, label: &str) {
        let panel = &scene.info_panels[&id];
        let expected = crate::info_panel::InfoPanel::text(label, &[], 0.55, 0.24).unwrap();
        assert_eq!(panel.mesh.indices, expected.mesh.indices);
        let vertices = |mesh: &crate::gltf_loader::ImportedMesh| {
            mesh.vertices
                .iter()
                .map(|vertex| (vertex.position, vertex.normal, vertex.color))
                .collect::<Vec<_>>()
        };
        assert_eq!(vertices(&panel.mesh), vertices(&expected.mesh));
    }
    #[test]
    fn cut_feedback_custom_label_updates_actual_geometry_without_frame_rebuilds() {
        let mut scene = SceneManager::default();
        let pieces = pieces(&mut scene);
        let ids = scene
            .play_cut_feedback_labeled(&pieces, false, 0.15, Some("BONUS 960"))
            .unwrap();
        assert_label(&scene, ids[7], "BONUS 960");
        let revision = scene.info_panels[&ids[7]].revision;
        scene.update_cut_feedback(0.1);
        scene.update_cut_feedback(0.1);
        assert_eq!(scene.info_panels[&ids[7]].revision, revision);
        assert_label(&scene, ids[7], "BONUS 960");
        let default = crate::info_panel::InfoPanel::text(DEFAULT_LABEL, &[], 0.55, 0.24).unwrap();
        assert_ne!(
            scene.info_panels[&ids[7]].mesh.indices,
            default.mesh.indices
        );
    }
    #[test]
    fn cut_feedback_default_methods_restore_labels_and_keep_pool_ids() {
        for method in 0..3 {
            let mut scene = SceneManager::default();
            let pieces = pieces(&mut scene);
            let first = scene
                .play_cut_feedback_labeled(&pieces, false, 1., Some("BONUS 960"))
                .unwrap();
            let second = scene
                .play_cut_feedback_labeled(&pieces, false, 1., Some("BONUS 120"))
                .unwrap();
            let count = scene.objects.len();
            for index in 0..8 {
                let ids = match method {
                    0 => scene.play_cut_feedback(&pieces),
                    1 => scene.play_cut_feedback_facing(&pieces, true),
                    _ => scene.play_cut_feedback_sized(&pieces, true, 0.15),
                }
                .unwrap();
                assert_eq!(ids, if index % 2 == 0 { &first } else { &second }.clone());
                assert_label(&scene, ids[7], DEFAULT_LABEL);
                assert_eq!(scene.objects.len(), count);
                assert_eq!(scene.info_panels.len(), 2);
            }
        }
    }
    #[test]
    fn cut_feedback_malformed_labels_are_atomic_for_live_pool() {
        let mut scene = SceneManager::default();
        let pieces = pieces(&mut scene);
        let ids = scene
            .play_cut_feedback_labeled(&pieces, false, 1., Some("BONUS 960"))
            .unwrap();
        let count = scene.objects.len();
        let next = scene.feedback.next;
        let revision = scene.info_panels[&ids[7]].revision;
        let positions: Vec<_> = scene
            .objects
            .values()
            .map(|object| object.transform.model())
            .collect();
        for label in [
            "",
            "BAD\nLABEL",
            "BAD\tLABEL",
            "BONUS č",
            "\u{7f}",
            &"A".repeat(33),
        ] {
            assert!(scene
                .play_cut_feedback_labeled(&pieces, true, 0.15, Some(label))
                .is_err());
            assert_eq!(scene.objects.len(), count);
            assert_eq!(scene.feedback.next, next);
            assert_eq!(scene.info_panels[&ids[7]].revision, revision);
            assert_label(&scene, ids[7], "BONUS 960");
            assert_eq!(
                scene
                    .objects
                    .values()
                    .map(|object| object.transform.model())
                    .collect::<Vec<_>>(),
                positions
            );
        }
    }
    #[test]
    fn cut_feedback_size_is_configurable_and_invalid_values_are_atomic() {
        let mut scene = SceneManager::default();
        let pieces = pieces(&mut scene);
        for size in [f32::NAN, 0., -1., 3.] {
            assert!(scene.play_cut_feedback_sized(&pieces, false, size).is_err());
            assert!(scene.feedback.bursts.iter().all(Option::is_none));
        }
        let normal = scene.play_cut_feedback_sized(&pieces, false, 1.).unwrap();
        let radius = scene.objects[&normal[0]].transform.scale.x;
        let small = scene.play_cut_feedback_sized(&pieces, false, 0.15).unwrap();
        assert!((scene.objects[&small[0]].transform.scale.x / radius - 0.15).abs() < 1e-6);
        scene.update_cut_feedback(0.2);
        assert!(
            (scene.objects[&small[0]].transform.scale.x
                / scene.objects[&normal[0]].transform.scale.x
                - 0.15)
                .abs()
                < 1e-6
        );
    }
    #[test]
    fn cut_feedback_speed_scales_radius_and_caps_at_double_through_animation() {
        for (speed, expected) in [
            (0., 0.12),
            (1., 0.14320988),
            (2., 0.7002469),
            (3., 2.),
            (5., 2.),
            (7., 2.),
            (40., 2.),
        ] {
            let mut scene = SceneManager::default();
            let pieces = pieces(&mut scene);
            for &id in &pieces {
                scene.cut_fragments.insert(
                    id,
                    CutFragment {
                        cut_speed: 0.,
                        family: 1,
                        blade_grace_until: 0.,
                        radius: 0.1,
                        generation: 1,
                        expires_at: 4.,
                        birth_cutter: None,
                        parent: 99,
                        parent_offset: Vec3::ZERO,
                    },
                );
            }
            scene.set_cut_feedback_speed(&pieces, speed);
            scene.set_cut_feedback_speed(&pieces, f32::NAN);
            scene.play_cut_feedback_sized(&pieces, false, 0.15).unwrap();
            let tuned = scene.feedback.bursts[0].unwrap();
            let attack = ((speed - 0.75) / 2.25).clamp(0., 1.).powi(2);
            assert!(
                (tuned.strength - (0.018 + 1.982 * attack)).abs() < 1e-5,
                "gentle setting must keep tiny slow hits without suppressing fast attacks"
            );
            let ids = scene.play_cut_feedback(&pieces).unwrap();
            let burst = scene.feedback.bursts[1].unwrap();
            assert!((burst.strength - expected).abs() < 1e-6);
            assert!(
                (scene.objects[&ids[0]].transform.scale.x / (burst.radius * 1.4) - expected).abs()
                    < 1e-5
            );
            scene.update_cut_feedback(DURATION * 0.5);
            assert!(
                (scene.objects[&ids[0]].transform.scale.x / (burst.radius * 2.75) - expected).abs()
                    < 1e-5
            );
        }
    }
    #[test]
    fn cut_feedback_texture_has_soft_alpha_and_clear_center() {
        let texture = ring_texture();
        assert_eq!(texture.len(), (TEXTURE_SIZE * TEXTURE_SIZE * 4) as usize);
        let alpha = |x: u32, y: u32| texture[((y * TEXTURE_SIZE + x) * 4 + 3) as usize];
        assert_eq!(alpha(TEXTURE_SIZE / 2, TEXTURE_SIZE / 2), 0);
        assert_eq!(alpha(0, 0), 0);
        assert!(
            texture
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[3] > 5 && p[3] < 200)
                .count()
                > 8000
        );
        assert!(texture.as_chunks::<4>().0.iter().any(|p| p[3] > 210));
        let mut scene = SceneManager::default();
        let pieces = pieces(&mut scene);
        let effects = scene.play_cut_feedback(&pieces).unwrap();
        assert!(scene.is_cut_feedback_ring(effects[0]));
        assert!(!scene.is_cut_feedback_ring(effects[1]));
        scene.clear();
        assert!(!scene.is_cut_feedback_ring(effects[0]));
    }
    #[test]
    fn cut_feedback_uses_one_closed_rim_instead_of_a_three_ring_sphere() {
        let mesh = ring().unwrap();
        mesh.validate().unwrap();
        let (min, max) = mesh.bounds();
        assert!((max - min).z < 0.05);
        assert_eq!(mesh.triangles.len(), 256);
    }
    #[test]
    fn cut_feedback_simultaneous_fruit_keep_separate_world_hit_centers() {
        let mut scene = SceneManager::default();
        let a = pieces(&mut scene);
        let b = pieces(&mut scene);
        for (family, ids, x) in [(11, a, -0.7), (12, b, 0.7)] {
            for (i, id) in ids.into_iter().enumerate() {
                scene.set_object_position(id, Vec3::new(x + (i as f32 - 0.5) * 0.08, 1.4, -1.2));
                scene.cut_fragments.insert(
                    id,
                    CutFragment {
                        cut_speed: 0.,
                        family,
                        blade_grace_until: 0.,
                        radius: 0.15,
                        generation: 1,
                        expires_at: 4.,
                        birth_cutter: None,
                        parent: family as u32,
                        parent_offset: Vec3::ZERO,
                    },
                );
            }
        }
        let ids = scene.play_cut_feedback(&[a[0], a[1], b[0], b[1]]).unwrap();
        assert_eq!(ids.len(), 16);
        let mut centers: Vec<_> = scene
            .feedback
            .bursts
            .iter()
            .flatten()
            .map(|b| b.center)
            .collect();
        centers.sort_by(|a, b| a.x.total_cmp(&b.x));
        assert!((centers[0] - Vec3::new(-0.7, 1.4, -1.2)).length() < 1e-5);
        assert!((centers[1] - Vec3::new(0.7, 1.4, -1.2)).length() < 1e-5);
        assert!(a.iter().chain(b.iter()).all(|id| scene.object_exists(*id)));
    }
    #[test]
    fn scored_cut_feedback_uses_each_original_reward_and_reuses_pool_ids() {
        let mut scene = SceneManager::default();
        let a = pieces(&mut scene);
        let b = pieces(&mut scene);
        for (family, ids, parent) in [(11, a, 101), (12, b, 202)] {
            for id in ids {
                scene.cut_fragments.insert(
                    id,
                    CutFragment {
                        cut_speed: 0.,
                        family,
                        blade_grace_until: 0.,
                        radius: 0.15,
                        generation: 1,
                        expires_at: 4.,
                        birth_cutter: None,
                        parent,
                        parent_offset: Vec3::ZERO,
                    },
                );
            }
        }
        let pieces = [a[0], a[1], b[0], b[1]];
        let rewards = [(101, 10), (202, 37)];
        let first = scene
            .play_cut_feedback_scored(&pieces, false, 0.8, &rewards)
            .unwrap();
        assert_eq!(first.len(), 16);
        assert_label(&scene, scene.feedback.bursts[0].unwrap().ids[7], "+10");
        assert_label(&scene, scene.feedback.bursts[1].unwrap().ids[7], "+37");
        let second = scene
            .play_cut_feedback_scored(&pieces, false, 0.8, &rewards)
            .unwrap();
        assert_eq!(first, second);
        assert_label(&scene, scene.feedback.bursts[0].unwrap().ids[7], "+10");
        assert_label(&scene, scene.feedback.bursts[1].unwrap().ids[7], "+37");
    }
    #[test]
    fn scored_cut_feedback_missing_original_reward_hides_score_caption() {
        let mut scene = SceneManager::default();
        let pieces = pieces(&mut scene);
        let ids = scene
            .play_cut_feedback_scored(&pieces, false, 1., &[(999, 10)])
            .unwrap();
        assert_label(&scene, ids[7], "");
    }
    #[test]
    fn cut_feedback_real_debris_keeps_momentum_and_external_impulses() {
        let mut scene = SceneManager::default();
        let ids = pieces(&mut scene);
        for id in ids {
            scene.set_object_position(id, Vec3::new(id as f32 * 0.3, 2., -1.));
            scene.set_projectile(id, Vec3::new(0., 1.8, 0.), 3., 0.);
        }
        scene.play_cut_feedback(&ids).unwrap();
        for id in ids {
            let body = scene.projectiles[&id];
            assert!((body.0.y - 2.04).abs() < 1e-5);
            assert_eq!(body.1, 9.81);
            assert_eq!(body.2, 0.45);
        }
        scene.update(0.1);
        let before = scene.objects[&ids[0]].transform.position;
        scene.set_projectile(ids[0], Vec3::new(2., 3., 0.), 9.81, 0.45);
        scene.update(0.1);
        let after = scene.objects[&ids[0]].transform.position;
        assert!((after.x - before.x - 0.2).abs() < 1e-4);
        assert!(after.y > before.y + 0.2);
    }
    #[test]
    fn piece_fade_uses_easing_and_removes_real_objects() {
        let mut scene = SceneManager::default();
        let ids = pieces(&mut scene);
        scene.play_cut_feedback(&ids).unwrap();
        scene.fade_cut_pieces(&ids, 0.125, 1.).unwrap();
        scene.update(0.125);
        assert_eq!(scene.objects[&ids[0]].visual_fade, 1.);
        for expected in [0.84375, 0.5, 0.15625] {
            scene.update(0.25);
            assert!((scene.objects[&ids[0]].visual_fade - expected).abs() < 1e-6);
        }
        scene.update(0.25);
        assert!(ids.iter().all(|id| !scene.object_exists(*id)
            && !scene.projectiles.contains_key(id)
            && !scene.triangle_meshes.contains_key(id)));
    }
    #[test]
    fn piece_fade_ignores_ground_and_live_fruit_and_survives_pool_wrap() {
        let mut scene = SceneManager::default();
        let first = pieces(&mut scene);
        scene.play_cut_feedback(&first).unwrap();
        scene.fade_cut_pieces(&first, 0., 1.).unwrap();
        let second = pieces(&mut scene);
        scene.play_cut_feedback(&second).unwrap();
        let third = pieces(&mut scene);
        scene.play_cut_feedback(&third).unwrap();
        scene.set_object_position(first[0], Vec3::new(0., 0.01, -1.));
        scene.set_projectile(first[0], Vec3::new(2., -3., 0.), 9.81, 1.);
        scene.set_object_position(second[0], Vec3::new(0.2, -0.34, -1.));
        scene.set_projectile(second[0], Vec3::ZERO, 0., 0.);
        scene.update(0.1);
        let p = scene.objects[&first[0]].transform.position;
        assert!((p.x - 0.2).abs() < 1e-5);
        assert!(
            (p.y - (0.01 - 0.3 - 0.04905)).abs() < 1e-5,
            "debris passes through ground and live body"
        );
        assert!((scene.objects[&first[0]].visual_fade - 0.972).abs() < 1e-5);
        assert!(scene.projectiles[&first[0]].0.y < -3.);
        for _ in 0..10 {
            scene.update(0.1);
        }
        assert!(first.iter().all(|id| !scene.object_exists(*id)));
    }
    #[test]
    fn piece_fade_long_timing_owns_lifetime_after_pool_wrap() {
        let mut scene = SceneManager::default();
        let ids = pieces(&mut scene);
        scene.play_cut_feedback(&ids).unwrap();
        scene.fade_cut_pieces(&ids, 1., 1.).unwrap();
        for _ in 0..3 {
            let next = pieces(&mut scene);
            scene.play_cut_feedback(&next).unwrap();
        }
        for _ in 0..7 {
            scene.update(0.25);
        }
        assert!((scene.objects[&ids[0]].visual_fade - 0.15625).abs() < 1e-6);
        scene.update(0.25);
        assert!(ids.iter().all(|id| !scene.object_exists(*id)));
        let ids = pieces(&mut scene);
        scene.play_cut_feedback(&ids).unwrap();
        scene.fade_cut_pieces(&ids, 0., 3.).unwrap();
        for _ in 0..3 {
            let next = pieces(&mut scene);
            scene.play_cut_feedback(&next).unwrap();
        }
        scene.clear_cut_feedback();
        assert!(ids.iter().all(|id| !scene.object_exists(*id)));
    }
    #[test]
    fn piece_dissolve_shares_easing_collision_free_motion_and_cleanup() {
        let mut scene = SceneManager::default();
        let ids = pieces(&mut scene);
        scene.play_cut_feedback(&ids).unwrap();
        scene.dissolve_cut_pieces(&ids, 0., 1.).unwrap();
        assert!(ids.iter().all(|id| scene.objects[id].visual_dissolve));
        scene.set_object_position(ids[0], Vec3::ZERO);
        scene.set_projectile(ids[0], -Vec3::Y, 0., 1.);
        scene.update(0.25);
        assert!((scene.objects[&ids[0]].visual_fade - 0.84375).abs() < 1e-6);
        assert!((scene.objects[&ids[0]].transform.position.y + 0.25).abs() < 1e-6);
        let mut queue = crate::visibility::RenderQueue::default();
        queue.extract(&scene, glam::Mat4::IDENTITY, false);
        let packed = queue.imported[&ids[0]][0].normal_matrix[3][3];
        assert!((packed + 1.84375).abs() < 1e-6);
        assert!(queue.imported[&ids[0]][0].needs_translucent_overlay());
        for _ in 0..3 {
            scene.update(0.25);
        }
        assert!(ids.iter().all(|id| !scene.object_exists(*id)));
    }
    #[test]
    fn piece_fade_invalid_options_are_atomic() {
        let mut scene = SceneManager::default();
        let ids = pieces(&mut scene);
        for (delay, duration) in [(f32::NAN, 1.), (-1., 1.), (0., 0.), (0., f32::INFINITY)] {
            assert!(scene.fade_cut_pieces(&ids, delay, duration).is_err());
        }
        assert!(scene.fade_cut_pieces(&[ids[0], u32::MAX], 0., 1.).is_err());
        scene.update(0.25);
        assert!(ids.iter().all(|id| scene.objects[id].visual_fade == 1.));
    }
    #[test]
    fn cut_feedback_debris_bounces_and_survives_flash_pool_wrap() {
        let mut scene = SceneManager::default();
        let first = pieces(&mut scene);
        scene.play_cut_feedback(&first).unwrap();
        let second = pieces(&mut scene);
        scene.play_cut_feedback(&second).unwrap();
        let third = pieces(&mut scene);
        scene.play_cut_feedback(&third).unwrap();
        assert!(first.iter().all(|id| scene.object_exists(*id)));
        assert_eq!(scene.feedback.retired_pieces.len(), 2);
        let mut bounced = false;
        for _ in 0..90 {
            let old = scene.projectiles[&first[0]].0.y;
            scene.update(1. / 90.);
            let new = scene.projectiles[&first[0]].0.y;
            if old < 0. && new > 0. {
                bounced = true;
            }
        }
        assert!(bounced);
        scene.clear_cut_feedback();
        assert!(first
            .iter()
            .chain(second.iter())
            .chain(third.iter())
            .all(|id| !scene.object_exists(*id)));
    }
    #[test]
    fn cut_feedback_pool_reuses_ids_and_expires_without_changing_geometry() {
        let mut scene = SceneManager::default();
        let pieces = pieces(&mut scene);
        let original: Vec<_> = pieces
            .iter()
            .map(|id| scene.triangle_meshes[id].mesh.volume())
            .collect();
        let a = scene.play_cut_feedback(&pieces).unwrap();
        assert_eq!(a.len(), 8);
        assert!(scene.objects[&a[0]].transform.position.y > 0.);
        let b = scene.play_cut_feedback(&pieces).unwrap();
        assert_ne!(a, b);
        let next = scene.next_id;
        assert_eq!(a, scene.play_cut_feedback(&pieces).unwrap());
        assert_eq!(next, scene.next_id);
        for _ in 0..72 {
            scene.update(1. / 90.);
        }
        for (i, id) in pieces.into_iter().enumerate() {
            let mesh = scene.triangle_meshes[&id].mesh.cpu().unwrap();
            mesh.validate().unwrap();
            assert_eq!(original[i], mesh.volume());
            assert_eq!(scene.objects[&id].color_cutout, 0.42);
            assert_eq!(scene.objects[&id].emission, 0.7);
            assert!(!scene.triangle_meshes[&id].cuttable);
            assert!(scene.objects[&id].transform.rotation.length() > 0.1);
        }
        assert!(
            scene.objects[&pieces[1]].transform.position.x
                - scene.objects[&pieces[0]].transform.position.x
                > 0.9
        );
        assert!(a
            .iter()
            .all(|id| scene.objects[id].transform.position == HIDDEN));
        scene.update(0.71);
        assert!(pieces.iter().all(|id| !scene.object_exists(*id)));
        assert_eq!(scene.objects.len(), 16);
    }
    #[test]
    fn cut_feedback_fits_real_world_fragment_silhouette_and_keeps_title_above_bounds() {
        let mut radii = [0.; 2];
        for (sample, (origin, scale)) in [
            (Vec3::new(-0.7, 1.4, -1.8), Vec3::new(0.06, 0.09, 0.05)),
            (Vec3::new(0.9, 2.1, -3.6), Vec3::new(0.24, 0.33, 0.17)),
        ]
        .into_iter()
        .enumerate()
        {
            let mut scene = SceneManager::default();
            let pieces = pieces(&mut scene);
            for (side, id) in pieces.into_iter().enumerate() {
                scene.set_object_transform(
                    id,
                    origin + Vec3::X * (side as f32 - 0.5) * scale.x,
                    Vec3::new(0.2, 0.3, -0.15),
                    scale,
                );
            }
            let mut expected_top = f32::NEG_INFINITY;
            let mut projected_radius = 0.0_f32;
            for id in pieces {
                let matrix = scene.objects[&id].transform.model();
                for vertex in &scene.triangle_meshes[&id].mesh.cpu().unwrap().positions {
                    let world = matrix.transform_point3(*vertex);
                    expected_top = expected_top.max(world.y);
                    projected_radius = projected_radius.max((world - origin).truncate().length());
                }
            }
            let ids = scene.play_cut_feedback(&pieces).unwrap();
            let ring = &scene.objects[&ids[0]];
            assert!(ring.transform.position.distance(origin) < 1e-5);
            assert!((ring.transform.scale.x - (projected_radius + 0.025) * 1.4 * 0.2).abs() < 1e-5);
            radii[sample] = ring.transform.scale.x;
            let text = &scene.objects[&ids[7]];
            assert!((text.transform.position.z - origin.z).abs() < 1e-5);
            let lowest_text = scene.info_panels[&ids[7]]
                .mesh
                .vertices
                .iter()
                .map(|v| {
                    text.transform
                        .model()
                        .transform_point3(Vec3::from_array(v.position))
                        .y
                })
                .fold(f32::INFINITY, f32::min);
            assert!(lowest_text > expected_top + 0.1);
            for id in &ids[1..7] {
                assert!(
                    (scene.objects[id].transform.position - origin)
                        .truncate()
                        .length()
                        >= projected_radius * 0.2
                );
            }
            scene.update(0.2);
            assert!(scene.objects[&ids[0]].transform.position.distance(origin) < 1e-5);
        }
        assert!(radii[1] > radii[0] * 2.);
    }
    #[test]
    fn cut_feedback_invalidates_warmed_collision_world_after_removing_piece_colliders() {
        let mut scene = SceneManager::default();
        let pieces = pieces(&mut scene);
        for id in pieces {
            assert!(scene.set_box_collider(id, Vec3::splat(0.5), Vec3::ZERO));
        }
        let query = crate::collision::Aabb::new(Vec3::splat(-10.), Vec3::splat(10.)).unwrap();
        assert_eq!(scene.collision_world().query(query, None).len(), 2);
        assert!(!scene.collision_dirty);
        scene.play_cut_feedback(&pieces).unwrap();
        assert!(scene.collision_world().query(query, None).is_empty());
    }
    #[test]
    fn cut_feedback_rotated_cut_ejects_along_real_caps_and_exposes_closed_flesh() {
        let mut scene = SceneManager::default();
        let cap = Vec3::new(0.32, 0.11, 0.18);
        let original = diamond(Vec3::new(1., 0.4, 0.1)).unwrap();
        let original_volume = original.volume();
        let split = original
            .slice(
                Vec3::new(0.12, -2., -2.),
                Vec3::new(0.12, 2., -2.),
                Vec3::Z * 4.,
                cap,
            )
            .unwrap()
            .unwrap();
        assert_eq!(split.pieces.len(), 2);
        let transform = Transform::new(
            Vec3::new(0.8, 1.6, -2.4),
            Vec3::new(0.35, 0.65, -0.4),
            Vec3::new(0.18, 0.25, 0.15),
        );
        let expected_normal = normal_matrix(transform.model())
            .transform_vector3(Vec3::X)
            .normalize();
        let mut ids = Vec::new();
        let mut volumes = Vec::new();
        let mut centers = Vec::new();
        for mesh in split.pieces {
            let centroid = mesh.centroid();
            volumes.push(mesh.volume());
            centers.push(transform.model().transform_point3(centroid));
            let id = scene.add_triangle_mesh(mesh).unwrap();
            scene.set_object_transform(id, transform.position, transform.rotation, transform.scale);
            scene.set_cuttable(id, true, cap).unwrap();
            ids.push(id);
        }
        assert!((volumes.iter().sum::<f64>() - original_volume).abs() < 1e-6);
        let effects = scene.play_cut_feedback(&ids).unwrap();
        let cut_center = transform.model().transform_point3(Vec3::new(0.12, 0., 0.));
        let rim = scene.objects[&effects[0]].transform;
        assert!(
            rim.position.distance(cut_center) < 1e-5,
            "off-center cut must anchor on the actual cap"
        );
        let rim_normal = rim.model().transform_vector3(Vec3::Z).normalize();
        assert!(rim_normal.dot(expected_normal).abs() > 0.9999);
        for id in &effects[1..7] {
            assert!(
                (scene.objects[id].transform.position - cut_center)
                    .dot(expected_normal)
                    .abs()
                    < 1e-5
            );
        }
        let motions = scene.feedback.bursts[0].unwrap().pieces;
        for motion in motions.into_iter().flatten() {
            assert!(motion.cap_normal.dot(expected_normal).abs() > 0.9999);
            let ejection = motion.velocity - Vec3::Y * 0.24;
            assert!((ejection + motion.cap_normal * 0.6).length() < 1e-6);
        }
        let before = (centers[1] - centers[0]).dot(expected_normal).abs();
        scene.update(0.3);
        assert!(
            scene.objects[&effects[0]].transform.scale.x > rim.scale.x * 1.8,
            "impact disc must expand substantially beyond the fruit"
        );
        let animated_normal = scene.objects[&effects[0]]
            .transform
            .model()
            .transform_vector3(Vec3::Z)
            .normalize();
        assert!(
            animated_normal.dot(expected_normal).abs() > 0.9999,
            "expanding rim must stay in the cut plane"
        );
        let mut after_centers = Vec::new();
        for (i, id) in ids.into_iter().enumerate() {
            let object = &scene.objects[&id];
            let mesh = scene.triangle_meshes[&id].mesh.cpu().unwrap();
            mesh.validate().unwrap();
            assert_eq!(volumes[i], mesh.volume());
            after_centers.push(object.transform.model().transform_point3(mesh.centroid()));
            let initial = motions[i].unwrap().cap_normal;
            let local_normal = if initial.dot(expected_normal) > 0. {
                Vec3::X
            } else {
                -Vec3::X
            };
            let facing = normal_matrix(object.transform.model())
                .transform_vector3(local_normal)
                .normalize();
            assert!(facing.dot(Vec3::Z) > initial.dot(Vec3::Z) + 0.01);
        }
        assert!(
            (after_centers[1] - after_centers[0])
                .dot(expected_normal)
                .abs()
                > before + 0.35
        );
    }
    #[test]
    fn cut_feedback_text_mirror_persists_through_native_animation_and_reuse() {
        let mut scene = SceneManager::default();
        let pieces = pieces(&mut scene);
        let mirrored = scene.play_cut_feedback_facing(&pieces, true).unwrap();
        assert!(scene.objects[&mirrored[7]].transform.scale.x < 0.);
        assert!(mirrored[..7]
            .iter()
            .all(|id| scene.objects[id].transform.scale.x > 0.));
        scene.update(0.2);
        assert!(scene.objects[&mirrored[7]].transform.scale.x < 0.);
        let normal = scene.play_cut_feedback(&pieces).unwrap();
        assert!(scene.objects[&normal[7]].transform.scale.x > 0.);
        assert_eq!(mirrored, scene.play_cut_feedback(&pieces).unwrap());
        scene.update(0.2);
        assert!(scene.objects[&mirrored[7]].transform.scale.x > 0.);
    }
    #[test]
    fn cut_feedback_rejects_atomically_and_resets_pool() {
        let mut scene = SceneManager::default();
        let p = pieces(&mut scene);
        let before = scene.objects.len();
        assert!(scene.play_cut_feedback(&[p[0], INVALID_ID]).is_err());
        assert!(scene.play_cut_feedback(&[p[0], p[0]]).is_err());
        assert_eq!(scene.objects.len(), before);
        let ids = scene.play_cut_feedback(&p).unwrap();
        scene.clear_cut_feedback();
        assert!(ids
            .iter()
            .all(|id| scene.objects[id].transform.position == HIDDEN));
        scene.clear_scene();
        assert_eq!(scene.objects.len(), 0);
        let p = pieces(&mut scene);
        let next = scene.play_cut_feedback(&p).unwrap();
        assert_ne!(ids, next);
    }
    #[test]
    fn cut_feedback_actual_controller_loss_parks_immediately() {
        let mut scene = SceneManager::default();
        let sword = scene.add_cube(Vec3::ZERO);
        scene
            .set_cutter(sword, Some([Vec3::ZERO, -Vec3::Z]))
            .unwrap();
        scene.set_blade_trail(sword, true).unwrap();
        let ids = scene.feedback.trails[0].unwrap().ids;
        let mut controller = crate::xr_input::TrackedController::default();
        controller.bind(&mut scene, Some(sword), true).unwrap();
        controller
            .update_timed(&mut scene, Some(Mat4::IDENTITY), false, 0.01)
            .unwrap();
        controller
            .update_timed(
                &mut scene,
                Some(Mat4::from_translation(Vec3::X * 0.1)),
                false,
                0.01,
            )
            .unwrap();
        assert!(ids
            .iter()
            .all(|id| scene.objects[id].transform.position != HIDDEN));
        controller
            .update_timed(&mut scene, None, false, 0.01)
            .unwrap();
        assert!(ids
            .iter()
            .all(|id| scene.objects[id].transform.position == HIDDEN));
        scene.remove_object(sword);
        assert!(ids.iter().all(|id| !scene.object_exists(*id)));
        let replacement = scene.add_cube(Vec3::ZERO);
        scene
            .set_cutter(replacement, Some([Vec3::ZERO, -Vec3::Z]))
            .unwrap();
        scene.set_blade_trail(replacement, true).unwrap();
        assert_eq!(scene.feedback.trails.iter().flatten().count(), 1);
        assert_eq!(scene.feedback.trails[0].unwrap().sword, replacement);
        assert!(scene.feedback.trails[0]
            .unwrap()
            .ids
            .iter()
            .all(|id| scene.object_exists(*id)));
    }
    #[test]
    fn cut_feedback_blade_trails_motion_only_bounded_and_loss_parks() {
        let mut scene = SceneManager::default();
        let sword = scene.add_cube(Vec3::ZERO);
        assert!(scene.set_blade_trail(sword, true).is_err());
        scene
            .set_cutter(sword, Some([Vec3::ZERO, -Vec3::Z]))
            .unwrap();
        scene.set_blade_trail(sword, true).unwrap();
        let ids = scene.feedback.trails[0].unwrap().ids;
        scene.sample_blade_trail(sword, [Vec3::ZERO, -Vec3::Z], 0.01);
        assert!(ids
            .iter()
            .all(|id| scene.objects[id].transform.position == HIDDEN));
        scene.sample_blade_trail(sword, [Vec3::X * 0.1, Vec3::X * 0.1 - Vec3::Z], 0.01);
        assert!(ids
            .iter()
            .all(|id| scene.objects[id].transform.position != HIDDEN));
        let count = scene.objects.len();
        scene.update(0.07);
        assert!(ids
            .iter()
            .all(|id| scene.objects[id].transform.position == HIDDEN));
        scene.sample_blade_trail(sword, [Vec3::X * 0.2, Vec3::X * 0.2 - Vec3::Z], 0.01);
        scene.reset_blade_trail(sword);
        assert!(ids
            .iter()
            .all(|id| scene.objects[id].transform.position == HIDDEN));
        scene.set_blade_trail(sword, false).unwrap();
        scene.sample_blade_trail(sword, [Vec3::ZERO, -Vec3::Z], 0.01);
        scene.set_blade_trail(sword, true).unwrap();
        assert_eq!(count, scene.objects.len());
    }
}
