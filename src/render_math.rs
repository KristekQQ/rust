//! View math shared by shadow/mirror passes and native tests.
use glam::{Mat4, Vec3, Vec4};
#[derive(Clone, Copy, Debug)]
pub struct MirrorPlane {
    pub normal: Vec3,
    pub distance: f32,
}
impl MirrorPlane {
    pub fn new(normal: Vec3, distance: f32) -> Option<Self> {
        let length = normal.length();
        if !normal.is_finite() || !distance.is_finite() || length < 1e-6 {
            return None;
        }
        Some(Self {
            normal: normal / length,
            distance: distance / length,
        })
    }
    pub fn matrix(self) -> Mat4 {
        let n = self.normal;
        Mat4::from_cols(
            (Vec3::X - 2.0 * n.x * n).extend(0.0),
            (Vec3::Y - 2.0 * n.y * n).extend(0.0),
            (Vec3::Z - 2.0 * n.z * n).extend(0.0),
            (-2.0 * self.distance * n).extend(1.0),
        )
    }
    pub fn clip_plane(self, camera: Vec3) -> Vec4 {
        let plane = self.normal.extend(self.distance);
        if self.normal.dot(camera) + self.distance < 0.0 {
            -plane
        } else {
            plane
        }
    }
}
pub fn shadow_view_projection(position: Vec3) -> Mat4 {
    let center = Vec3::new(0.0, -0.5, 0.0);
    let target = if center.distance_squared(position) < 1e-8 {
        position + Vec3::NEG_Z
    } else {
        center
    };
    let direction = (target - position).normalize_or_zero();
    let up = if direction.dot(Vec3::Y).abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    glam::camera::lh::proj::directx::perspective(1.5, 1.0, 0.1, 30.0)
        * glam::camera::lh::view::look_at_mat4(position, target, up)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mirror_reflects_camera_and_preserves_surface_points() {
        let p = MirrorPlane::new(Vec3::Z * 2.0, 6.0).unwrap();
        let r = p.matrix();
        assert!(
            (r.transform_point3(Vec3::new(1.0, 2.0, 5.0)) - Vec3::new(1.0, 2.0, -11.0)).length()
                < 1e-5
        );
        let surface = Vec3::new(1.0, 2.0, -3.0);
        assert!((r.transform_point3(surface) - surface).length() < 1e-5);
        assert!((r * r - Mat4::IDENTITY)
            .to_cols_array()
            .iter()
            .all(|v| v.abs() < 1e-5));
        assert!(r.determinant() < 0.0);
        assert!(p.clip_plane(Vec3::ZERO).dot(Vec3::ZERO.extend(1.0)) > 0.0);
        assert!(MirrorPlane::new(Vec3::ZERO, 0.0).is_none());
    }
    #[test]
    fn light_frustum_keeps_off_camera_shadow_casters() {
        let light = Vec3::new(-3.0, 7.0, 4.0);
        let f = crate::visibility::Frustum::from_view_projection(shadow_view_projection(light));
        assert!(f.intersects_aabb(Vec3::ZERO, Vec3::splat(0.5)));
        assert!(!f.intersects_aabb(light * 2.0, Vec3::splat(0.1)));
    }
}
