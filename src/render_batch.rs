//! View-local scheduling for explicitly order-independent opaque model runs.
//! Instance-page capacity is separate from spatial scheduling: a dense nearby
//! group may contain many roots, while a long depth chain is split early.
use glam::{Mat4, Vec3};

type Bounds = (Vec3, Vec3);
const MAX_DEPTH_RATIO: f32 = 2.0;

pub(crate) struct DepthWindow {
    camera: Vec3,
    forward: Vec3,
    near: f32,
    far: f32,
}
impl DepthWindow {
    pub fn new(view_projection: Mat4, camera: Vec3, bounds: Bounds) -> Option<Self> {
        if !view_projection.is_finite() || !camera.is_finite() {
            return None;
        }
        let rows = view_projection.transpose();
        // Perspective clip-w is forward depth. Orthographic clip-w is constant;
        // its clip-z normal supplies the view axis instead. No inverse per root.
        let forward = rows
            .w_axis
            .truncate()
            .try_normalize()
            .or_else(|| rows.z_axis.truncate().try_normalize())?;
        let mut window = Self {
            camera,
            forward,
            near: 0.0,
            far: 0.0,
        };
        (window.near, window.far) = window.interval(bounds)?;
        Some(window)
    }
    fn interval(&self, (min, max): Bounds) -> Option<(f32, f32)> {
        if !min.is_finite() || !max.is_finite() || !min.cmple(max).all() {
            return None;
        }
        let center = (min + max) * 0.5;
        let radius = self.forward.abs().dot((max - min) * 0.5);
        let depth = self.forward.dot(center - self.camera);
        let near = depth - radius;
        let far = depth + radius;
        (near > 0.0 && near.is_finite() && far.is_finite()).then_some((near, far))
    }
    /// A rejection does not mutate the accepted run. Only reorderable rigid
    /// opaque draws may use this cost policy; it grants no ordering permission.
    pub fn include(&mut self, bounds: Bounds) -> bool {
        let Some((near, far)) = self.interval(bounds) else {
            return false;
        };
        let near = self.near.min(near);
        let far = self.far.max(far);
        if far / near > MAX_DEPTH_RATIO {
            return false;
        }
        self.near = near;
        self.far = far;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bounds(z: f32) -> Bounds {
        (Vec3::new(-0.2, -0.2, z - 0.1), Vec3::new(0.2, 0.2, z + 0.1))
    }
    fn projection() -> Mat4 {
        glam::camera::rh::proj::directx::perspective(1.0, 1.5, 0.08, 200.0)
    }
    #[test]
    fn dense_roots_do_not_split_at_an_arbitrary_object_count() {
        let mut w = DepthWindow::new(projection(), Vec3::ZERO, bounds(-4.0)).unwrap();
        for i in 0..2048 {
            assert!(w.include(bounds(-4.0 - (i % 512) as f32 * 0.005)));
        }
        assert!(!w.include(bounds(-30.0)));
        assert!(w.include(bounds(-5.0))); // Rejection did not poison the run.
    }
    #[test]
    fn camera_rotation_translation_and_orthographic_depth_are_view_local() {
        let camera = Vec3::new(8.0, 2.0, -3.0);
        let view = glam::camera::rh::view::look_at_mat4(camera, camera + Vec3::X, Vec3::Y);
        let local = |x: f32| {
            (
                camera + Vec3::new(x - 0.1, -0.2, -0.2),
                camera + Vec3::new(x + 0.1, 0.2, 0.2),
            )
        };
        for p in [
            projection(),
            glam::camera::rh::proj::directx::orthographic(-4.0, 4.0, -3.0, 3.0, 0.08, 200.0),
        ] {
            let mut w = DepthWindow::new(p * view, camera, local(4.0)).unwrap();
            assert!(w.include(local(6.0)));
            assert!(!w.include(local(30.0)));
            assert!(!w.include(local(-2.0)));
        }
    }
    #[test]
    fn frustum_containment_never_accepts_partial_touching_or_invalid_bounds() {
        let f = crate::visibility::Frustum::from_view_projection(Mat4::IDENTITY);
        assert!(f.fully_contains_aabb(Vec3::new(0.0, 0.0, 0.5), Vec3::splat(0.2)));
        for (c, e) in [
            (Vec3::new(0.99, 0.0, 0.5), Vec3::splat(0.1)),
            (Vec3::new(0.0, 0.0, 0.0), Vec3::ZERO),
            (Vec3::new(0.0, 0.0, 0.95), Vec3::splat(0.1)),
        ] {
            assert!(f.intersects_aabb(c, e));
            assert!(!f.fully_contains_aabb(c, e));
        }
        assert!(!f.fully_contains_aabb(Vec3::splat(f32::NAN), Vec3::ONE));
        assert!(!f.fully_contains_aabb(Vec3::ZERO, -Vec3::ONE));
    }
    #[test]
    fn invalid_and_camera_crossing_bounds_are_conservative() {
        assert!(DepthWindow::new(Mat4::ZERO, Vec3::ZERO, bounds(-4.0)).is_none());
        assert!(DepthWindow::new(projection(), Vec3::ZERO, bounds(0.0)).is_none());
        assert!(DepthWindow::new(projection(), Vec3::ZERO, (Vec3::ONE, Vec3::ZERO)).is_none());
        let mut w = DepthWindow::new(projection(), Vec3::ZERO, bounds(-4.0)).unwrap();
        assert!(!w.include((Vec3::splat(f32::NAN), Vec3::ONE)));
        assert!(!w.include(bounds(0.0)));
        assert!(w.include(bounds(-5.0)));
    }
}
