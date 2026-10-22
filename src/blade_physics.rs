//! Opt-in one-sided sharp edge and finite kinematic blade contacts.
//! CPU fruit uses baked convex proxies and contact-point angular impulses.
//! Lazy GPU-resident pieces retain the bounds fallback without CPU readback.
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
    /// Assisted blades accept either edge direction, while rejecting flat sweeps.
    pub(crate) fn assisted_sharp(self, pose: Mat4, axis: Vec3, motion: Vec3) -> bool {
        let edge = pose.transform_vector3(self.edge).normalize();
        let flat = axis.normalize().cross(edge).normalize();
        let advance = motion.dot(edge).abs();
        advance > 1e-6 && advance > motion.dot(flat).abs() * 0.75
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
    assisted: bool,
    edge_cut_eligible: bool,
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
        if edge_cut_eligible
            && !supporting_flat
            && if assisted {
                config.assisted_sharp(pose, axis, blade_velocity * dt)
            } else {
                config.sharp(pose, axis, relative_motion)
            }
        {
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

pub(crate) struct BodyContact {
    pub center: Vec3,
    pub velocity: Vec3,
    pub angular: Vec3,
}

/// Convex proxy versus finite blade box. All shape preparation is done at creation;
/// queries only transform up to 26 vertices and cached separating axes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn contact_shape(
    config: BladePhysics,
    blade: [Vec3; 2],
    from: Mat4,
    to: Mat4,
    old_center: Vec3,
    center: Vec3,
    shape: &crate::contact_shape::ContactShape,
    rotation: Quat,
    scale: Vec3,
    mut velocity: Vec3,
    mut angular: Vec3,
    bounce: f32,
    dt: f32,
    assisted: bool,
    edge_cut_eligible: bool,
) -> Option<BodyContact> {
    let axis_local = (blade[1] - blade[0]).normalize();
    let mid_local = (blade[0] + blade[1]) * 0.5;
    let half_length = blade[0].distance(blade[1]) * 0.5;
    let half = Vec3::new(half_length, config.half_width, config.half_thickness);
    let old_mid = from.transform_point3(mid_local);
    let new_mid = to.transform_point3(mid_local);
    let radius = (shape.half * scale.abs()).length();
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
    let h = shape.half * scale.abs();
    // Unit-mass box inertia approximation; contact shape controls the lever arm.
    let inverse_inertia = Vec3::new(
        3. / (h.y * h.y + h.z * h.z).max(1e-8),
        3. / (h.x * h.x + h.z * h.z).max(1e-8),
        3. / (h.x * h.x + h.y * h.y).max(1e-8),
    );
    let inverse = |v: Vec3| rotation * (inverse_inertia * (rotation.conjugate() * v));
    let mut correction = Vec3::ZERO;
    let mut touched = false;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let q = qa.slerp(qb, t).normalize();
        let mid = from.w_axis.truncate().lerp(to.w_axis.truncate(), t) + q * mid_local;
        let axes = [
            q * axis_local,
            q * config.edge,
            q * axis_local.cross(config.edge),
        ];
        let to_local = |v: Vec3| Vec3::new(v.dot(axes[0]), v.dot(axes[1]), v.dot(axes[2]));
        let to_world = |v: Vec3| axes[0] * v.x + axes[1] * v.y + axes[2] * v.z;
        let point = old_center.lerp(center, t) + correction;
        let mut vertices = [Vec3::ZERO; 26];
        for (out, p) in vertices.iter_mut().zip(&shape.points) {
            *out = to_local(point - mid + rotation * (*p * scale));
        }
        let vertices = &vertices[..shape.points.len()];
        let mut depth = f32::INFINITY;
        let mut normal_local = Vec3::ZERO;
        let candidates = [Vec3::X, Vec3::Y, Vec3::Z]
            .into_iter()
            .chain(
                shape
                    .normals
                    .iter()
                    .map(|n| to_local(rotation * (*n / scale))),
            )
            .chain(shape.edges.iter().flat_map(|e| {
                let e = to_local(rotation * (*e * scale));
                [e.cross(Vec3::X), e.cross(Vec3::Y), e.cross(Vec3::Z)]
            }));
        let mut separated = false;
        for axis in candidates {
            if axis.length_squared() < 1e-10 {
                continue;
            }
            let axis = axis.normalize();
            let mut low = f32::INFINITY;
            let mut high = f32::NEG_INFINITY;
            for p in vertices {
                let d = p.dot(axis);
                low = low.min(d);
                high = high.max(d);
            }
            let extent = axis.abs().dot(half);
            let positive = extent - low;
            let negative = high + extent;
            if positive < 0. || negative < 0. {
                separated = true;
                break;
            }
            let (overlap, direction) = if positive < negative {
                (positive, axis)
            } else {
                (negative, -axis)
            };
            if overlap < depth {
                depth = overlap;
                normal_local = direction;
            }
        }
        if separated {
            continue;
        }
        let normal = to_world(normal_local);
        let support = vertices
            .iter()
            .map(|p| p.dot(normal_local))
            .fold(f32::INFINITY, f32::min);
        let mut surface = Vec3::ZERO;
        let mut count = 0.;
        for p in vertices {
            if p.dot(normal_local) < support + 0.0005 {
                surface += *p;
                count += 1.;
            }
        }
        surface /= count;
        let closest = surface.clamp(-half, half);
        let material = mid_local
            + axis_local * closest.x
            + config.edge * closest.y
            + axis_local.cross(config.edge) * closest.z;
        let blade_velocity = (to.transform_point3(material) - from.transform_point3(material)) / dt;
        let pose = Mat4::from_rotation_translation(q, mid - q * mid_local);
        let relative_motion = blade_velocity * dt - (center - old_center);
        // A face normal alone also occurs while the sharp edge is already
        // crossing the interior. Only the outer flat is a supporting contact.
        let center_flat = to_local(point - mid).z;
        let flat_radius = vertices
            .iter()
            .map(|p| (center_flat - p.z) * center_flat.signum())
            .fold(0., f32::max);
        let supporting_flat = normal_local.z.abs() > 0.75
            && center_flat.abs() >= config.half_thickness + flat_radius * 0.8;
        if edge_cut_eligible
            && !supporting_flat
            && if assisted {
                config.assisted_sharp(pose, axes[0], blade_velocity * dt)
            } else {
                config.sharp(pose, axes[0], relative_motion)
            }
        {
            continue;
        }

        // Clamp the manifold to the finite blade: an off-center landing must tip.
        let contact = closest + normal_local * (surface - closest).dot(normal_local);
        let lever = mid + to_world(contact) - point;
        correction += normal * (depth + 1e-5);
        let relative = velocity + angular.cross(lever) - blade_velocity;
        let incoming = relative.dot(normal);
        if incoming < 0. {
            let effective_mass = 1. + normal.dot(inverse(lever.cross(normal)).cross(lever));
            let restitution = if incoming < -0.5 { bounce } else { 0. };
            let impulse = -incoming * (1. + restitution) / effective_mass;
            velocity += normal * impulse;
            angular += inverse(lever.cross(normal * impulse));
            let relative = velocity + angular.cross(lever) - blade_velocity;
            let tangent = relative - normal * relative.dot(normal);
            if tangent.length_squared() > 1e-12 {
                let direction = tangent.normalize();
                let mass = 1. + direction.dot(inverse(lever.cross(direction)).cross(lever));
                let friction = -direction * (tangent.length() / mass).min(impulse * 0.8);
                velocity += friction;
                angular += inverse(lever.cross(friction));
            }
            // A small finite contact patch supplies static rolling resistance.
            // This prevents numerical drift on the apple's bottom pole without
            // cancelling the much larger moment of an off-center landing.
            let patch = (radius * 0.04).min(config.half_width * 0.5);
            let direction = angular.normalize_or_zero();
            let resistance = inverse(direction).length() * impulse * patch;
            angular -= direction * resistance.min(angular.length());
            // Additional material damping acts only while supported.
            angular *= (-5. * dt / steps as f32).exp();
        }
        touched = true;
    }
    touched.then_some(BodyContact {
        center: center + correction,
        velocity,
        angular,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn octahedron() -> crate::contact_shape::ContactShape {
        crate::contact_shape::ContactShape::new(
            &[Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z],
            (-Vec3::ONE, Vec3::ONE),
            Vec3::ZERO,
        )
    }
    #[test]
    fn blade_physics_convex_proxy_rejects_empty_bounds_corner_and_handles_mirrored_scale() {
        let shape = octahedron();
        for scale in [Vec3::splat(0.1), Vec3::new(-0.1, 0.1, 0.1)] {
            let center = Vec3::new(0.15, 0.08, -0.5);
            assert!(
                contact_shape(
                    config(),
                    [Vec3::ZERO, -Vec3::Z],
                    Mat4::IDENTITY,
                    Mat4::IDENTITY,
                    center,
                    center,
                    &shape,
                    Quat::IDENTITY,
                    scale,
                    Vec3::ZERO,
                    Vec3::ZERO,
                    0.,
                    1. / 90.,
                    false,
                    false,
                )
                .is_none(),
                "empty AABB corner is not a surface"
            );
            let center = Vec3::new(0., 0.09, -0.5);
            let hit = contact_shape(
                config(),
                [Vec3::ZERO, -Vec3::Z],
                Mat4::IDENTITY,
                Mat4::IDENTITY,
                center,
                center,
                &shape,
                Quat::IDENTITY,
                scale,
                -Vec3::Y,
                Vec3::ZERO,
                0.,
                1. / 90.,
                false,
                false,
            )
            .unwrap();
            assert!((hit.center.y - 0.11001).abs() < 1e-4);
            assert!(hit.angular.length() < 1e-4);
        }
    }
    #[test]
    fn blade_physics_convex_fast_flat_sweep_does_not_tunnel() {
        let shape = octahedron();
        let center = Vec3::new(0., 0., -0.5);
        let result = contact_shape(
            config(),
            [Vec3::ZERO, -Vec3::Z],
            Mat4::from_translation(-Vec3::Y * 0.4),
            Mat4::from_translation(Vec3::Y * 0.4),
            center,
            center,
            &shape,
            Quat::IDENTITY,
            Vec3::splat(0.1),
            Vec3::ZERO,
            Vec3::ZERO,
            0.,
            0.05,
            false,
            false,
        )
        .unwrap();
        assert!(result.center.y > 0.5, "{:?}", result.center);
        assert!(result.velocity.y > 15.);
    }
    #[test]
    fn blade_physics_sharp_interior_is_not_a_supporting_flat() {
        let shape = octahedron();
        let center = Vec3::new(0., 0.02, -0.5);
        assert!(
            contact_shape(
                config(),
                [Vec3::ZERO, -Vec3::Z],
                Mat4::from_translation(-Vec3::X * 0.02),
                Mat4::from_translation(Vec3::X * 0.02),
                center,
                center,
                &shape,
                Quat::IDENTITY,
                Vec3::splat(0.1),
                Vec3::ZERO,
                Vec3::ZERO,
                0.,
                0.02,
                false,
                true,
            )
            .is_none(),
            "interior sharp traversal must remain available to the mesh cutter"
        );
    }
    #[test]
    fn blade_physics_asymmetric_fragment_keeps_its_near_supporting_face() {
        let points = [
            Vec3::new(-0.1, -0.025, -0.1),
            Vec3::new(0.1, -0.025, -0.1),
            Vec3::new(0., -0.025, 0.1),
            Vec3::new(0., 0.075, 0.),
        ];
        let shape = crate::contact_shape::ContactShape::new(
            &points,
            (Vec3::new(-0.1, -0.025, -0.1), Vec3::new(0.1, 0.075, 0.1)),
            Vec3::ZERO,
        );
        let center = Vec3::new(0., 0.0349, -0.5);
        assert!(
            contact_shape(
                config(),
                [Vec3::ZERO, -Vec3::Z],
                Mat4::IDENTITY,
                Mat4::from_translation(Vec3::X * 0.0003),
                center,
                center,
                &shape,
                Quat::IDENTITY,
                Vec3::ONE,
                -Vec3::Y * 0.1,
                Vec3::ZERO,
                0.,
                1. / 90.,
                false,
                true,
            )
            .is_some(),
            "the opposite distant vertex must not hide the supporting face"
        );
    }
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
                false,
                true,
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
            false,
            true,
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
            false,
            true,
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
        assert!(c.assisted_sharp(pose, -Vec3::Z, Vec3::Y));
        assert!(c.assisted_sharp(pose, -Vec3::Z, -Vec3::Y));
        assert!(!c.assisted_sharp(pose, -Vec3::Z, Vec3::X));
        assert!(!c.assisted_sharp(pose, -Vec3::Z, Vec3::ZERO));
    }
}
