//! Opt-in one-sided sharp edge and finite kinematic blade contacts.
//! Fruit contacts use a cached actual-vertex sphere clipped by oriented mesh bounds;
//! cutting still uses the real mesh. No gravity is integrated a second time here.
use glam::{Mat4, Quat, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct BladePhysics {
    pub edge: Vec3,
    pub half_width: f32,
    pub half_thickness: f32,
}
impl BladePhysics {
    pub(crate) fn validate(self, blade: [Vec3; 2]) -> Result<(), String> {
        if !self.edge.is_finite()
            || (self.edge.length_squared() - 1.).abs() > 0.002
            || self.edge.dot((blade[1] - blade[0]).normalize()).abs() > 0.001
            || !self.half_width.is_finite()
            || !self.half_thickness.is_finite()
            || self.half_width <= 0.
            || self.half_width > 1.
            || self.half_thickness <= 0.
            || self.half_thickness > self.half_width
        {
            return Err("Blade edge must be unit and perpendicular to its axis; finite dimensions require 0 < thickness <= width <= 1m".into());
        }
        Ok(())
    }
    pub(crate) fn scaled(self, scale: Vec3, blade: [Vec3; 2]) -> Self {
        let axis = (blade[1] - blade[0]).normalize();
        let edge = self.edge * scale;
        let scaled_axis = (axis * scale).normalize();
        Self {
            edge: (edge - scaled_axis * edge.dot(scaled_axis)).normalize(),
            half_width: self.half_width * edge.length(),
            half_thickness: self.half_thickness * (axis.cross(self.edge) * scale).length(),
        }
    }
    pub(crate) fn sharp(self, pose: Mat4, axis: Vec3, relative_motion: Vec3) -> bool {
        let edge = pose.transform_vector3(self.edge).normalize();
        let flat = axis.normalize().cross(edge).normalize();
        let forward = relative_motion.dot(edge);
        forward > 1e-6 && forward > relative_motion.dot(flat).abs() * 1.15
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn contact(
    config: BladePhysics,
    blade: [Vec3; 2],
    from: Mat4,
    to: Mat4,
    old_center: Vec3,
    center: Vec3,
    radius: f32,
    oriented_bounds: Option<(Quat, Vec3)>,
    mut velocity: Vec3,
    bounce: f32,
    dt: f32,
) -> Option<(Vec3, Vec3)> {
    let axis_local = (blade[1] - blade[0]).normalize();
    let mid_local = (blade[0] + blade[1]) * 0.5;
    let half_length = blade[0].distance(blade[1]) * 0.5;
    let old_mid = from.transform_point3(mid_local);
    let new_mid = to.transform_point3(mid_local);
    let broad_radius = half_length + config.half_width + radius;
    let min = old_mid.min(new_mid) - Vec3::splat(broad_radius);
    let max = old_mid.max(new_mid) + Vec3::splat(broad_radius);
    if old_center.min(center).cmpgt(max).any() || old_center.max(center).cmplt(min).any() {
        return None;
    }
    let qa = Quat::from_mat4(&from).normalize();
    let qb = Quat::from_mat4(&to).normalize();
    let travel = old_mid.distance(new_mid)
        + center.distance(old_center)
        + qa.angle_between(qb).abs() * half_length;
    let steps = (travel / (radius * 0.2).clamp(0.002, 0.015))
        .ceil()
        .clamp(1., 256.) as u32;
    let mut correction = Vec3::ZERO;
    let mut touched = false;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let rotation = qa.slerp(qb, t).normalize();
        let pose = Mat4::from_rotation_translation(
            rotation,
            from.w_axis.truncate().lerp(to.w_axis.truncate(), t),
        );
        let mid = pose.transform_point3(mid_local);
        let axis = rotation * axis_local;
        let edge = rotation * config.edge;
        let flat = axis.cross(edge).normalize();
        let axes = [axis, edge, flat];
        let half = Vec3::new(half_length, config.half_width, config.half_thickness);
        let point = old_center.lerp(center, t) + correction;
        let relative = point - mid;
        let local = Vec3::new(relative.dot(axis), relative.dot(edge), relative.dot(flat));
        let closest = local.clamp(-half, half);
        let offset = local - closest;
        if offset.length_squared() > radius * radius {
            continue;
        }
        // Use the material point nearest this fruit, including angular motion.
        let material = mid_local
            + axis_local * closest.x
            + config.edge * closest.y
            + axis_local.cross(config.edge) * closest.z;
        let blade_velocity = (to.transform_point3(material) - from.transform_point3(material)) / dt;
        let relative_motion = blade_velocity * dt - (center - old_center);
        let (normal_local, depth) = if offset.length_squared() > 1e-12 {
            (offset.normalize(), radius - offset.length())
        } else {
            let gaps = half - local.abs();
            let k = if gaps.x < gaps.y && gaps.x < gaps.z {
                0
            } else if gaps.y < gaps.z {
                1
            } else {
                2
            };
            let mut n = Vec3::ZERO;
            n[k] = if local[k] < 0. { -1. } else { 1. };
            (n, radius + gaps[k])
        };
        let normal = axes[0] * normal_local.x + axes[1] * normal_local.y + axes[2] * normal_local.z;
        let flat_radius = oriented_bounds.map_or(radius, |(rotation, half)| {
            radius.min((rotation.conjugate() * flat).abs().dot(half))
        });
        // Edge-facing translation of a supporting flat is still a flat
        // contact. Only an edge approaching the fruit's interior cross-section
        // bypasses rigid response so the finite mesh cut can finish.
        let supporting_flat = normal_local.z.abs() > 0.75
            && local.z.abs() >= config.half_thickness + flat_radius * 0.8;
        if !supporting_flat && config.sharp(pose, axis, relative_motion) {
            continue;
        }
        // Both the actual-vertex sphere and oriented bounds enclose the mesh.
        // Their intersection avoids hovering on flat faces of authored fruit.
        let support_radius = oriented_bounds.map_or(radius, |(rotation, half)| {
            radius.min((rotation.conjugate() * normal).abs().dot(half))
        });
        let depth = depth - (radius - support_radius);
        if depth < 0. {
            continue;
        }
        correction += normal * (depth + 1e-5);
        let relative_velocity = velocity - blade_velocity;
        let incoming = relative_velocity.dot(normal);
        if incoming < 0. {
            velocity -= normal * incoming * (1. + bounce);
            // Coulomb-like friction transfers supporting blade motion, bounded
            // by the normal impulse, and leaves release velocity intact.
            let tangent = relative_velocity - normal * incoming;
            let friction = (-incoming * 0.8).min(tangent.length());
            velocity -= tangent.normalize_or_zero() * friction;
        }
        touched = true;
    }
    touched.then_some((center + correction, velocity))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> BladePhysics {
        BladePhysics {
            edge: Vec3::X,
            half_width: 0.08,
            half_thickness: 0.01,
        }
    }
    #[test]
    fn blade_physics_flat_support_carry_and_release() {
        let blade = [Vec3::ZERO, -Vec3::Z];
        let mut center = Vec3::new(0., 0.2, -0.5);
        let mut velocity = Vec3::ZERO;
        let dt = 1. / 90.;
        let mut pose = Mat4::IDENTITY;
        for frame in 0..180 {
            let previous = center;
            velocity.y -= 9.81 * dt;
            center += velocity * dt;
            let next = Mat4::from_translation(Vec3::new(0., 0., frame as f32 * 0.0005));
            if let Some((p, v)) = contact(
                config(),
                blade,
                pose,
                next,
                previous,
                center,
                0.1,
                None,
                velocity,
                0.,
                dt,
            ) {
                center = p;
                velocity = v;
            }
            pose = next;
        }
        assert!((center.y - 0.11).abs() < 0.002, "{center:?}");
        assert!(velocity.y.abs() < 0.002, "{velocity:?}");
        assert!(center.z > -0.45, "carried: {center:?}");
        // Removing tracking means no solver call: normal projectile gravity resumes.
        let y = center.y;
        for _ in 0..20 {
            velocity.y -= 9.81 * dt;
            center += velocity * dt;
        }
        assert!(center.y < y - 0.2);
    }
    #[test]
    fn blade_physics_fast_flat_sweep_does_not_tunnel() {
        let from = Mat4::from_translation(-Vec3::Y * 0.4);
        let to = Mat4::from_translation(Vec3::Y * 0.4);
        let center = Vec3::new(0., 0., -0.5);
        let (resolved, velocity) = contact(
            config(),
            [Vec3::ZERO, -Vec3::Z],
            from,
            to,
            center,
            center,
            0.1,
            None,
            Vec3::ZERO,
            0.,
            0.05,
        )
        .unwrap();
        assert!(resolved.y > 0.5, "{resolved:?}");
        assert!(velocity.y > 15.);
    }
    #[test]
    fn blade_physics_supporting_flat_keeps_friction_during_edge_facing_translation() {
        let center = Vec3::new(0., 0.1039, -0.5);
        let (_, velocity) = contact(
            BladePhysics {
                edge: Vec3::X,
                half_width: 0.0225,
                half_thickness: 0.004,
            },
            [Vec3::ZERO, -Vec3::Z],
            Mat4::IDENTITY,
            Mat4::from_translation(Vec3::X / 3000.),
            center,
            center,
            0.16,
            Some((Quat::IDENTITY, Vec3::new(0.15, 0.1, 0.15))),
            -Vec3::Y * (8. / 90.),
            0.,
            1. / 90.,
        )
        .expect("edge-facing board motion must preserve supporting flat contact");
        assert!(velocity.x > 0.025, "{velocity:?}");
        assert!(velocity.y.abs() < 1e-4, "{velocity:?}");
    }
    #[test]
    fn blade_physics_edge_and_flat_rotate_with_pose() {
        let c = config();
        let pose = Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2);
        assert!(c.sharp(pose, -Vec3::Z, Vec3::Y));
        assert!(!c.sharp(pose, -Vec3::Z, -Vec3::Y));
        assert!(!c.sharp(pose, -Vec3::Z, Vec3::X));
    }
}
