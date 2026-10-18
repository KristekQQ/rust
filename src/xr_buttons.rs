//! World-space hit testing for simple front-facing XR panels.
use glam::{Mat4, Quat, Vec3};

/// Finite blade contact and a bounded grip-pose sweep against a panel volume.
/// The second result remains useful when the first pose or motion is invalid.
pub fn blade_panel_contact(
    from: Option<Mat4>,
    to: Mat4,
    blade: [Vec3; 2],
    scale: Vec3,
    panel_world: Mat4,
    size: [f32; 2],
    dt: f32,
) -> [bool; 2] {
    if !valid_grip(to)
        || !panel_world.is_finite()
        || !panel_world.determinant().is_finite()
        || panel_world.determinant().abs() <= 1e-8
        || size.iter().any(|v| !v.is_finite() || *v <= 0.)
        || !scale.is_finite()
        || blade.iter().any(|p| !p.is_finite())
    {
        return [false; 2];
    }
    let inverse = panel_world.inverse();
    if !inverse.is_finite()
        || panel_world.x_axis.w.abs() > 1e-5
        || panel_world.y_axis.w.abs() > 1e-5
        || panel_world.z_axis.w.abs() > 1e-5
        || (panel_world.w_axis.w - 1.).abs() > 1e-5
    {
        return [false; 2];
    }
    // Convert the world-space blade tolerance to each panel-local slab.
    let tolerance = Vec3::new(
        Vec3::new(inverse.x_axis.x, inverse.y_axis.x, inverse.z_axis.x).length(),
        Vec3::new(inverse.x_axis.y, inverse.y_axis.y, inverse.z_axis.y).length(),
        Vec3::new(inverse.x_axis.z, inverse.y_axis.z, inverse.z_axis.z).length(),
    ) * 0.025;
    let half = Vec3::new(size[0] * 0.5, size[1] * 0.5, 0.0175) + tolerance;
    let blade = blade.map(|p| p * scale);
    if blade.iter().any(|p| !p.is_finite()) || !half.is_finite() {
        return [false; 2];
    }
    let touches = |pose: Mat4| {
        let [a, b] = blade.map(|p| inverse.transform_point3(pose.transform_point3(p)));
        segment_box(a, b, half)
    };
    let current = touches(to);
    let Some(from) = from.filter(|p| valid_grip(*p)) else {
        return [false, current];
    };
    let start = from.w_axis.truncate();
    let end = to.w_axis.truncate();
    let q0 = Quat::from_mat4(&from).normalize();
    let q1 = Quat::from_mat4(&to).normalize();
    let translation = start.distance(end);
    let angle = q0.angle_between(q1).abs();
    if !dt.is_finite()
        || dt <= 0.
        || dt > 0.05
        || translation > 1.
        || translation > 15. * dt + 0.025
        || angle > 1.5
        || angle > 45. * dt + 0.03
    {
        return [false, current];
    }
    let radius = blade[0].length().max(blade[1].length());
    let steps = ((translation + angle * radius) / 0.01)
        .ceil()
        .clamp(1., 64.) as usize;
    let swept = (0..=steps).any(|i| {
        let t = i as f32 / steps as f32;
        touches(Mat4::from_rotation_translation(
            q0.slerp(q1, t),
            start.lerp(end, t),
        ))
    });
    [swept, current]
}

fn valid_grip(pose: Mat4) -> bool {
    let axes = [
        pose.x_axis.truncate(),
        pose.y_axis.truncate(),
        pose.z_axis.truncate(),
    ];
    pose.is_finite()
        && pose.w_axis.truncate().abs().max_element() <= 100.
        && pose.x_axis.w.abs() < 1e-4
        && pose.y_axis.w.abs() < 1e-4
        && pose.z_axis.w.abs() < 1e-4
        && (pose.w_axis.w - 1.).abs() < 1e-4
        && axes.iter().all(|a| (a.length_squared() - 1.).abs() < 0.002)
        && axes[0].dot(axes[1]).abs() < 0.001
        && axes[0].dot(axes[2]).abs() < 0.001
        && axes[1].dot(axes[2]).abs() < 0.001
        && (pose.determinant() - 1.).abs() < 0.003
}

fn segment_box(a: Vec3, b: Vec3, half: Vec3) -> bool {
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    let direction = b - a;
    let mut near: f32 = 0.;
    let mut far: f32 = 1.;
    for axis in 0..3 {
        if direction[axis].abs() <= 1e-8 {
            if a[axis].abs() > half[axis] {
                return false;
            }
        } else {
            let t0 = (-half[axis] - a[axis]) / direction[axis];
            let t1 = (half[axis] - a[axis]) / direction[axis];
            near = near.max(t0.min(t1));
            far = far.min(t0.max(t1));
            if near > far {
                return false;
            }
        }
    }
    true
}

/// Tests a target-ray-space pose (local -Z forward) against a centered XY panel
/// whose visible face points along local +Z. The distance limit is in world meters.
pub fn ray_hits_panel(
    ray_world: Mat4,
    panel_world: Mat4,
    width: f32,
    height: f32,
    max_distance: f32,
) -> bool {
    ray_panel_intersection(ray_world, panel_world, width, height, max_distance).is_some()
}

/// Returns the world-space point where the ray intersects the panel, if it hits.
pub fn ray_panel_intersection(
    ray_world: Mat4,
    panel_world: Mat4,
    width: f32,
    height: f32,
    max_distance: f32,
) -> Option<Vec3> {
    fn finite(m: Mat4) -> bool {
        m.to_cols_array().iter().all(|v| v.is_finite())
    }
    if !finite(ray_world)
        || !finite(panel_world)
        || !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || !max_distance.is_finite()
        || max_distance < 0.0
        || !panel_world.determinant().is_finite()
        || panel_world.determinant().abs() <= 1e-8
    {
        return None;
    }
    let inverse = panel_world.inverse();
    if !finite(inverse) {
        return None;
    }
    let origin_world = ray_world.transform_point3(Vec3::ZERO);
    let direction_world = ray_world.transform_vector3(-Vec3::Z);
    if !origin_world.is_finite()
        || !direction_world.is_finite()
        || direction_world.length_squared() <= 1e-12
    {
        return None;
    }
    let direction_world = direction_world.normalize();
    let origin = inverse.transform_point3(origin_world);
    let direction = inverse.transform_vector3(direction_world);
    if !origin.is_finite() || !direction.is_finite() || direction.z >= -1e-8 {
        return None;
    }
    let distance = -origin.z / direction.z;
    if !distance.is_finite() || distance < 0.0 || distance > max_distance {
        return None;
    }
    let hit = origin + direction * distance;
    if hit.x.abs() > width * 0.5 || hit.y.abs() > height * 0.5 {
        return None;
    }
    let world_hit = panel_world.transform_point3(hit);
    world_hit.is_finite().then_some(world_hit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;

    fn blade_contact(from: Option<Mat4>, to: Mat4, panel: Mat4, dt: f32) -> [bool; 2] {
        blade_panel_contact(
            from,
            to,
            [Vec3::ZERO, -Vec3::Z],
            Vec3::ONE,
            panel,
            [0.4, 0.3],
            dt,
        )
    }

    #[test]
    fn blade_current_contact_is_finite_and_works_from_either_side() {
        let forward = Mat4::from_translation(Vec3::new(0., 0., 0.5));
        assert_eq!(
            blade_contact(None, forward, Mat4::IDENTITY, 0.01),
            [false, true]
        );
        let reverse = Mat4::from_rotation_translation(
            Quat::from_rotation_y(std::f32::consts::PI),
            Vec3::new(0., 0., -0.5),
        );
        assert_eq!(
            blade_contact(None, reverse, Mat4::IDENTITY, 0.01),
            [false, true]
        );
        for position in [
            Vec3::new(0., 0., 1.1),
            Vec3::new(0.24, 0., 0.5),
            Vec3::new(0., 0.19, 0.5),
        ] {
            assert_eq!(
                blade_contact(None, Mat4::from_translation(position), Mat4::IDENTITY, 0.01),
                [false; 2]
            );
        }
    }

    #[test]
    fn blade_sweep_catches_between_frame_crossing_and_rotation() {
        let from = Mat4::from_translation(Vec3::new(-0.3, 0., 0.5));
        let to = Mat4::from_translation(Vec3::new(0.3, 0., 0.5));
        assert_eq!(
            blade_contact(Some(from), to, Mat4::IDENTITY, 0.05),
            [true, false]
        );
        let panel = Mat4::from_translation(Vec3::new(0., 0., -0.9));
        let from = Mat4::from_quat(Quat::from_rotation_y(-0.5));
        let to = Mat4::from_quat(Quat::from_rotation_y(0.5));
        assert_eq!(blade_contact(Some(from), to, panel, 0.03), [true, false]);
        let shifted = Mat4::from_translation(Vec3::new(0., 0.3, 0.));
        assert_eq!(
            blade_contact(Some(shifted * from), shifted * to, panel, 0.03),
            [false; 2]
        );
    }

    #[test]
    fn blade_contact_uses_panel_transform_and_cutter_scale() {
        let rigid =
            Mat4::from_rotation_translation(Quat::from_rotation_y(0.7), Vec3::new(1., 1., -2.));
        let panel = rigid * Mat4::from_scale(Vec3::new(2., 0.5, 1.5));
        let pose = rigid * Mat4::from_translation(Vec3::new(0.35, 0., 0.5));
        assert_eq!(blade_contact(None, pose, panel, 0.01), [false, true]);
        let miss = rigid * Mat4::from_translation(Vec3::new(0.45, 0., 0.5));
        assert_eq!(blade_contact(None, miss, panel, 0.01), [false; 2]);
        assert_eq!(
            blade_panel_contact(
                None,
                pose,
                [Vec3::ZERO, -Vec3::Z],
                Vec3::splat(0.2),
                panel,
                [0.4, 0.3],
                0.01
            ),
            [false; 2]
        );
    }

    #[test]
    fn blade_invalid_motion_suppresses_activation_but_preserves_current_touch() {
        let touching = Mat4::from_translation(Vec3::new(0., 0., 0.5));
        for dt in [0., -0.01, 0.051, f32::NAN] {
            assert_eq!(
                blade_contact(Some(touching), touching, Mat4::IDENTITY, dt),
                [false, true]
            );
        }
        for from in [
            Mat4::ZERO,
            Mat4::from_scale(Vec3::splat(2.)),
            Mat4::from_translation(Vec3::splat(f32::NAN)),
            Mat4::from_translation(Vec3::new(1.1, 0., 0.5)),
            Mat4::from_translation(Vec3::new(0.3, 0., 0.5)),
            Mat4::from_rotation_translation(Quat::from_rotation_y(1.6), Vec3::new(0., 0., 0.5)),
            Mat4::from_rotation_translation(Quat::from_rotation_y(0.6), Vec3::new(0., 0., 0.5)),
        ] {
            assert_eq!(
                blade_contact(Some(from), touching, Mat4::IDENTITY, 0.01),
                [false, true]
            );
        }
        for invalid in [
            Mat4::ZERO,
            Mat4::from_scale(Vec3::splat(2.)),
            Mat4::from_translation(Vec3::splat(f32::NAN)),
        ] {
            assert_eq!(
                blade_contact(None, invalid, Mat4::IDENTITY, 0.01),
                [false; 2]
            );
        }
        assert_eq!(
            blade_contact(Some(touching), touching, Mat4::ZERO, 0.01),
            [false; 2]
        );
    }

    fn ray(origin: Vec3, direction: Vec3) -> Mat4 {
        let rotation = Quat::from_rotation_arc(-Vec3::Z, direction.normalize());
        Mat4::from_rotation_translation(rotation, origin)
    }

    #[test]
    fn hit_testing_handles_standing_pose_transform_and_rejects_misses() {
        let panel = Mat4::from_translation(Vec3::new(0.0, 1.1, -1.6));
        let center_ray = ray(Vec3::new(0.0, 1.1, 0.0), -Vec3::Z);
        assert!(ray_hits_panel(center_ray, panel, 0.6, 0.35, 2.0));
        let hit = ray_panel_intersection(center_ray, panel, 0.6, 0.35, 2.0).unwrap();
        assert!(hit.abs_diff_eq(Vec3::new(0.0, 1.1, -1.6), 1e-6));

        let transformed = Mat4::from_scale_rotation_translation(
            Vec3::new(1.5, 0.75, 1.0),
            Quat::from_rotation_y(0.45),
            Vec3::new(0.2, 1.1, -1.6),
        );
        let target = transformed.transform_point3(Vec3::ZERO);
        let front = transformed.transform_vector3(Vec3::Z).normalize();
        let transformed_hit =
            ray_panel_intersection(ray(target + front, -front), transformed, 0.6, 0.35, 1.1)
                .unwrap();
        assert!(transformed_hit.abs_diff_eq(target, 1e-5));
        assert!(ray_hits_panel(
            ray(target + front * 1.0, -front),
            transformed,
            0.6,
            0.35,
            1.1
        ));
        assert!(!ray_hits_panel(
            ray(target + front, -front),
            transformed,
            0.6,
            0.35,
            0.9
        ));
        assert!(!ray_hits_panel(
            ray(Vec3::new(0.5, 1.1, 0.0), -Vec3::Z),
            panel,
            0.6,
            0.35,
            2.0
        ));
        assert!(!ray_hits_panel(
            ray(Vec3::new(0.0, 1.1, -2.0), Vec3::Z),
            panel,
            0.6,
            0.35,
            2.0
        ));
        assert!(!ray_hits_panel(
            ray(Vec3::new(0.0, 1.1, 0.0), Vec3::X),
            panel,
            0.6,
            0.35,
            2.0
        ));
    }

    #[test]
    fn hit_testing_rejects_invalid_or_singular_inputs() {
        assert!(!ray_hits_panel(Mat4::IDENTITY, Mat4::ZERO, 1.0, 1.0, 2.0));
        assert!(!ray_hits_panel(
            Mat4::from_translation(Vec3::splat(f32::NAN)),
            Mat4::IDENTITY,
            1.0,
            1.0,
            2.0
        ));
        assert!(!ray_hits_panel(
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            f32::NAN,
            1.0,
            2.0
        ));
    }
}
