//! View math shared by shadow/mirror passes and native tests.
use glam::{Mat4, Vec3, Vec4};

/// Normalized DOM coordinates (left/top=0) projected onto world horizontal plane.
pub fn screen_to_horizontal_plane(matrix: Mat4, u: f32, v: f32, height: f32) -> Option<Vec3> {
    if !u.is_finite()
        || !v.is_finite()
        || !height.is_finite()
        || !(0.0..=1.0).contains(&u)
        || !(0.0..=1.0).contains(&v)
    {
        return None;
    }
    let inverse = matrix.inverse();
    if !inverse.is_finite() {
        return None;
    }
    let near = inverse.project_point3(Vec3::new(u * 2.0 - 1.0, 1.0 - v * 2.0, 0.0));
    let far = inverse.project_point3(Vec3::new(u * 2.0 - 1.0, 1.0 - v * 2.0, 1.0));
    let direction = far - near;
    if !near.is_finite() || !direction.is_finite() || direction.y.abs() < 1e-6 {
        return None;
    }
    let t = (height - near.y) / direction.y;
    let point = near + direction * t;
    (t >= 0.0 && point.is_finite()).then_some(point)
}
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
    fn pointer_projection_matches_visible_camera_and_rejects_invalid_rays() {
        let mut camera = crate::input::camera::Camera::new(1.5);
        assert!(camera.set_view(Vec3::new(5., 6., 7.), Vec3::new(0., 1., 0.)));
        let matrix = camera.matrix();
        let p = screen_to_horizontal_plane(matrix, 0.5, 0.5, 1.0).unwrap();
        assert!(p.length().is_finite());
        assert!(p.distance(Vec3::Y) < 1e-3);
        let visible = matrix.project_point3(p);
        assert!(visible.x.abs() < 1e-4 && visible.y.abs() < 1e-4);
        assert!(screen_to_horizontal_plane(matrix, -0.1, 0.5, 0.).is_none());
        assert!(screen_to_horizontal_plane(Mat4::ZERO, 0.5, 0.5, 0.).is_none());
        assert!(screen_to_horizontal_plane(Mat4::IDENTITY, 0.5, 0.5, 0.).is_none());
    }
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

/// WebXR uses OpenGL [-1,1] depth; wgpu render views use [0,1].
pub fn xr_view(projection: &[f32], world: &[f32]) -> Option<(Mat4, Vec3)> {
    let projection: &[f32; 16] = projection.try_into().ok()?;
    let world: &[f32; 16] = world.try_into().ok()?;
    let projection = Mat4::from_cols_array(projection);
    let world = Mat4::from_cols_array(world);
    if !projection.is_finite() || !world.is_finite() || world.determinant().abs() < 1e-8 {
        return None;
    }
    let depth = Mat4::from_cols(
        Vec4::X,
        Vec4::Y,
        Vec4::new(0., 0., 0.5, 0.),
        Vec4::new(0., 0., 0.5, 1.),
    );
    Some((
        depth * projection * world.inverse(),
        world.transform_point3(Vec3::ZERO),
    ))
}

/// WebGPU-compatible XR sessions already supply zero-to-one clip depth.
pub fn xr_gpu_view(projection: &[f32], world: &[f32]) -> Option<(Mat4, Vec3)> {
    let projection = Mat4::from_cols_array(projection.try_into().ok()?);
    let world = Mat4::from_cols_array(world.try_into().ok()?);
    if !projection.is_finite() || !world.is_finite() || world.determinant().abs() < 1e-8 {
        return None;
    }
    Some((
        projection * world.inverse(),
        world.transform_point3(Vec3::ZERO),
    ))
}

#[cfg(test)]
mod xr_tests {
    use super::*;
    #[test]
    fn xr_gpu_preserves_zero_to_one_projection() {
        let p = glam::camera::rh::proj::directx::perspective(1., 1., 0.1, 100.);
        let world = Mat4::from_translation(Vec3::new(0.032, 1.6, 0.));
        let (vp, eye) = xr_gpu_view(&p.to_cols_array(), &world.to_cols_array()).unwrap();
        assert!(vp.project_point3(eye - Vec3::Z * 0.1).z.abs() < 1e-5);
        assert!((vp.project_point3(eye - Vec3::Z * 100.).z - 1.).abs() < 1e-5);
        assert!(xr_gpu_view(&[0.; 15], &[0.; 16]).is_none());
        assert!(xr_gpu_view(&[f32::NAN; 16], &[0.; 16]).is_none());
        assert!(xr_gpu_view(&p.to_cols_array(), &[0.; 16]).is_none());
    }
    #[test]
    fn xr_eye_pose_and_depth_conversion() {
        let p = glam::camera::rh::proj::opengl::perspective(1.0, 1.0, 0.1, 100.0);
        let world = Mat4::from_translation(Vec3::new(0.032, 1.6, 0.));
        let (vp, eye) = xr_view(&p.to_cols_array(), &world.to_cols_array()).unwrap();
        assert!((eye - Vec3::new(0.032, 1.6, 0.)).length() < 1e-6);
        assert!(vp.project_point3(eye - Vec3::Z * 0.1).z.abs() < 1e-5);
        assert!((vp.project_point3(eye - Vec3::Z * 100.).z - 1.).abs() < 1e-5);
        assert!(xr_view(&[0.; 15], &[0.; 16]).is_none());
        assert!(xr_view(&[f32::NAN; 16], &[0.; 16]).is_none());
        assert!(xr_view(&p.to_cols_array(), &[0.; 16]).is_none());
    }
}
