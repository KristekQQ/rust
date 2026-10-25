//! Sphere CCD against a translating/rotating finite paddle, with physical impulses.
//! The pose is the centre of the blade (not the grip); local X is its normal.
use glam::{Quat, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub center: Vec3,
    pub orientation: Quat,
}

#[derive(Clone, Copy, Debug)]
pub struct Motion {
    pub start: Pose,
    pub end: Pose,
    pub duration: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Contact {
    pub fraction: f32,
    pub normal: Vec3,
    /// Point on the actual blade, before the sphere-radius expansion.
    pub point: Vec3,
    pub surface_velocity: Vec3,
    pub corrected_center: Vec3,
}

// Exact signed distance and nearest surface point for a solid finite cylinder.
// Expanding this distance by the ball radius gives the rounded rim, rather than
// treating an infinite plane (or the blade's bounding sphere) as the collider.
fn cylinder(p: Vec3, radius: f32, half: f32) -> (f32, Vec3, Vec3) {
    let radial = Vec3::new(0., p.y, p.z);
    let length = radial.length();
    let radial_normal = if length > 1e-12 {
        radial / length
    } else {
        Vec3::Y
    };
    let axial_normal = Vec3::X * if p.x < 0. { -1. } else { 1. };
    let axial_gap = p.x.abs() - half;
    let radial_gap = length - radius;
    if axial_gap > 0. || radial_gap > 0. {
        let point = Vec3::X * p.x.clamp(-half, half) + radial_normal * length.min(radius);
        let delta = p - point;
        let distance = delta.length();
        (distance, delta / distance, point)
    } else if axial_gap >= radial_gap {
        (axial_gap, axial_normal, p - axial_normal * axial_gap)
    } else {
        (radial_gap, radial_normal, p - radial_normal * radial_gap)
    }
}

pub fn sweep_ball(
    start: Vec3,
    end: Vec3,
    ball_radius: f32,
    motion: Motion,
    paddle_radius: f32,
    half_thickness: f32,
) -> Option<Contact> {
    if !start.is_finite()
        || !end.is_finite()
        || !motion.start.center.is_finite()
        || !motion.end.center.is_finite()
        || !motion.start.orientation.is_finite()
        || !motion.end.orientation.is_finite()
        || motion.start.orientation.length_squared() < 1e-12
        || motion.end.orientation.length_squared() < 1e-12
        || !motion.duration.is_finite()
        || motion.duration <= 0.
        || !ball_radius.is_finite()
        || ball_radius <= 0.
        || !paddle_radius.is_finite()
        || paddle_radius <= 0.
        || !half_thickness.is_finite()
        || half_thickness < 0.
    {
        return None;
    }
    let q0 = motion.start.orientation.normalize();
    let mut q1 = motion.end.orientation.normalize();
    if q0.dot(q1) < 0. {
        q1 = -q1;
    }
    let rotation = (q1 * q0.conjugate()).normalize();
    let (axis, angle) = rotation.to_axis_angle();
    let omega = axis * (angle / motion.duration);
    let translation = motion.end.center - motion.start.center;
    let linear_velocity = translation / motion.duration;
    // A global bound on how fast the distance to any point of the blade can
    // decrease, per unit fraction. This remains valid when the rim rotates into
    // a stationary ball, and avoids time sampling that tunnels at high speed.
    let bound =
        (end - start - translation).length() + angle.abs() * paddle_radius.hypot(half_thickness);
    // Include the interpolation arithmetic's absolute error at large travel or
    // world coordinates. Otherwise a 200 m sweep can stall one fraction-ulp
    // before impact even though its residual gap is just a few micrometres.
    let coordinate_scale = start
        .abs()
        .max(end.abs())
        .max(motion.start.center.abs())
        .max(motion.end.center.abs())
        .max_element();
    let tolerance = 1e-6_f32
        .max(ball_radius * 1e-5)
        .max(coordinate_scale * (4. * f32::EPSILON));
    let mut fraction = 0.;
    for _ in 0..512 {
        let orientation = q0.slerp(q1, fraction).normalize();
        let center = motion.start.center + translation * fraction;
        let ball = start.lerp(end, fraction);
        let (distance, local_normal, local_point) = cylinder(
            orientation.conjugate() * (ball - center),
            paddle_radius,
            half_thickness,
        );
        let separation = distance - ball_radius;
        if separation <= tolerance {
            let normal = orientation * local_normal;
            let offset = orientation * local_point;
            let point = center + offset;
            return Some(Contact {
                fraction,
                normal,
                point,
                surface_velocity: linear_velocity + omega.cross(offset),
                corrected_center: point + normal * (ball_radius + 2. * tolerance),
            });
        }
        if bound <= 1e-12 || fraction >= 1. {
            return None;
        }
        let next = fraction + separation / bound;
        if next > 1. {
            return None;
        }
        // Floating point stagnation can only occur within sub-ulp distance of
        // a grazing configuration; never jump an arbitrary time interval.
        if next <= fraction {
            return None;
        }
        fraction = next;
    }
    None
}

/// Unit-mass impulse response for a thin hollow ball: I/m = 2 R² / 3.
/// Restitution acts on relative normal speed; Coulomb friction couples spin and
/// translation. A separating overlap is corrected by CCD but gets no impulse.
pub fn resolve(
    velocity: Vec3,
    spin: Vec3,
    contact: &Contact,
    ball_radius: f32,
    restitution: f32,
    friction: f32,
) -> (Vec3, Vec3) {
    if ball_radius <= 0. || !ball_radius.is_finite() {
        return (velocity, spin);
    }
    let normal = contact.normal;
    let arm = -normal * ball_radius;
    let relative = velocity + spin.cross(arm) - contact.surface_velocity;
    let normal_speed = relative.dot(normal);
    if normal_speed >= 0. {
        return (velocity, spin);
    }
    let normal_impulse = -(1. + restitution.clamp(0., 1.)) * normal_speed;
    let slip = relative - normal * normal_speed;
    // 1/m + R²/I = 5/(2m), so a unit-mass no-slip impulse is slip / 2.5.
    let mut tangent_impulse = -slip / 2.5;
    let limit = friction.max(0.) * normal_impulse;
    let tangent_length = tangent_impulse.length();
    if tangent_length > limit {
        tangent_impulse *= limit / tangent_length;
    }
    let new_velocity = velocity + normal * normal_impulse + tangent_impulse;
    let new_spin = spin + arm.cross(tangent_impulse) * (1.5 / (ball_radius * ball_radius));
    (new_velocity, new_spin)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn motion(q: Quat) -> Motion {
        let pose = Pose {
            center: Vec3::ZERO,
            orientation: q,
        };
        Motion {
            start: pose,
            end: pose,
            duration: 0.01,
        }
    }
    fn hit(start: Vec3, end: Vec3, motion: Motion) -> Contact {
        sweep_ball(start, end, 0.02, motion, 0.085, 0.005).unwrap()
    }

    #[test]
    fn falling_ball_rebounds_from_horizontal_paddle() {
        let m = motion(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2));
        let c = hit(Vec3::Y * 0.05, Vec3::Y * -0.05, m);
        let (v, w) = resolve(-Vec3::Y * 10., Vec3::ZERO, &c, 0.02, 0.8, 0.5);
        assert!((v - Vec3::Y * 8.).length() < 1e-4);
        assert!(w.length() < 1e-3);
        assert!((c.fraction - 0.25).abs() < 1e-4);
    }

    #[test]
    fn tilted_face_reflects_about_actual_normal() {
        let q = Quat::from_rotation_z(0.6);
        let n = q * Vec3::X;
        let c = hit(n * 0.1, -n * 0.1, motion(q));
        let (v, _) = resolve(-n * 5., Vec3::ZERO, &c, 0.02, 1., 0.);
        assert!((v - n * 5.).length() < 1e-4);
    }

    #[test]
    fn faster_paddle_produces_stronger_rebound() {
        let still = motion(Quat::IDENTITY);
        let mut moving = still;
        moving.end.center.x = 0.02;
        let c0 = hit(Vec3::X * 0.08, -Vec3::X * 0.02, still);
        let c1 = hit(Vec3::X * 0.08, -Vec3::X * 0.02, moving);
        let (v0, _) = resolve(-Vec3::X * 10., Vec3::ZERO, &c0, 0.02, 0.8, 0.);
        let (v1, _) = resolve(-Vec3::X * 10., Vec3::ZERO, &c1, 0.02, 0.8, 0.);
        assert!((v1.x - v0.x - 3.6).abs() < 1e-4);
    }

    #[test]
    fn tangential_brush_transfers_spin_without_increasing_energy() {
        let c = hit(Vec3::X * 0.1, -Vec3::X * 0.1, motion(Quat::IDENTITY));
        let input = Vec3::new(-5., 4., 0.);
        let (v, w) = resolve(input, Vec3::ZERO, &c, 0.02, 1., 0.5);
        assert!(v.y > 0. && v.y < input.y);
        assert!(w.z > 0.);
        let energy = v.length_squared() + 2. / 3. * 0.02_f32.powi(2) * w.length_squared();
        assert!(energy <= input.length_squared() + 1e-4);
        assert!((v + w.cross(-Vec3::X * 0.02)).y.abs() < 1e-4);
    }

    #[test]
    fn high_speed_sweep_and_rounded_edge() {
        let m = motion(Quat::IDENTITY);
        let c = hit(Vec3::new(100., 0.095, 0.), Vec3::new(-100., 0.095, 0.), m);
        assert!(c.normal.x > 0.8 && c.normal.y > 0.45);
        assert!((c.point.y - 0.085).abs() < 1e-5);
        assert!(sweep_ball(
            Vec3::new(100., 0.106, 0.),
            Vec3::new(-100., 0.106, 0.),
            0.02,
            m,
            0.085,
            0.005
        )
        .is_none());
    }

    #[test]
    fn fast_rotating_edge_hits_stationary_ball() {
        let mut m = motion(Quat::IDENTITY);
        m.end.orientation = Quat::from_rotation_z(2.8);
        let ball = Vec3::new(0.075, 0., 0.);
        let c = hit(ball, ball, m);
        assert!(c.fraction > 0. && c.fraction < 1.);
        assert!(c.surface_velocity.length() > 10.);
        let (v, _) = resolve(Vec3::ZERO, Vec3::ZERO, &c, 0.02, 0.8, 0.3);
        assert!(v.length() > 1.);
    }

    #[test]
    fn separating_overlap_does_not_bounce_again() {
        let c = hit(Vec3::X * 0.024, Vec3::X * 0.04, motion(Quat::IDENTITY));
        let input = Vec3::X * 2.;
        let spin = Vec3::Y * 30.;
        assert_eq!(resolve(input, spin, &c, 0.02, 0.9, 0.5), (input, spin));
        assert!(c.corrected_center.x > 0.025);
    }
}
