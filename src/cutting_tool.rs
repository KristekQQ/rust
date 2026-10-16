//! Input-independent pose and finite swing shared by a cursor-held or planned blade.
use glam::Vec3;

pub struct ToolAim {
    pub center: Vec3,
    pub yaw: f32,
    pub tilt: f32,
    pub height: f32,
    pub floor: f32,
}
pub struct ToolPose {
    pub position: Vec3,
    pub rotation: Vec3,
    pub motion: Vec3,
    pub guide_center: Vec3,
    pub direction: Vec3,
    pub length: f32,
}
impl ToolAim {
    pub fn pose(&self, blade: [Vec3; 2], scale: Vec3) -> Result<ToolPose, String> {
        if !self.center.is_finite()
            || self.center.abs().max_element() > 10_000.0
            || !self.yaw.is_finite()
            || !self.tilt.is_finite()
            || self.tilt.abs() > 0.53
            || !self.height.is_finite()
            || !self.floor.is_finite()
            || self.height.abs() > 10_000.0
            || self.floor.abs() > 10_000.0
            || self.height - self.floor < 0.2
            || self.height - self.floor > 20.0
            || (blade[0].x - blade[1].x).abs() > 1e-5
            || (blade[0].y - blade[1].y).abs() > 1e-5
        {
            return Err("Aim requires finite values and a local Z-aligned blade".into());
        }
        let normal = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
        let direction = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos());
        let middle = (blade[0] + blade[1]) * 0.5 * scale;
        let edge_height = self.height + middle.y;
        let depth = edge_height - self.floor + 0.005;
        if depth <= 0.0 || !depth.is_finite() {
            return Err("Blade is already below the cutting floor".into());
        }
        let mut position = self.center
            - normal * (self.tilt.tan() * (edge_height - self.center.y) + middle.x)
            - direction * middle.z;
        position.y = self.height;
        let pose = ToolPose {
            position,
            rotation: Vec3::new(0.0, self.yaw, 0.0),
            motion: normal * self.tilt.tan() * depth - Vec3::Y * depth,
            guide_center: Vec3::new(self.center.x, self.floor + 0.018, self.center.z)
                + normal * self.tilt.tan() * (self.center.y - self.floor),
            direction,
            length: (blade[1].z - blade[0].z).abs() * scale.z.abs(),
        };
        if !pose.position.is_finite()
            || !pose.motion.is_finite()
            || !pose.guide_center.is_finite()
            || !pose.length.is_finite()
            || pose.length < 1e-5
        {
            return Err("Cutting pose exceeds finite bounds".into());
        }
        Ok(pose)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cutting_tool_guide_matches_actual_tilted_sweep() {
        for yaw in [0.0, 0.7, 2.1] {
            for tilt in [-0.4, 0.0, 0.3] {
                let blade = [Vec3::new(0., -0.1, -2.65), Vec3::new(0., -0.1, 2.65)];
                let aim = ToolAim {
                    center: Vec3::new(2., 1.15, -1.),
                    yaw,
                    tilt,
                    height: 3.6,
                    floor: 0.2,
                };
                let p = aim.pose(blade, Vec3::ONE).unwrap();
                let matrix =
                    crate::scene::Transform::new(p.position, p.rotation, Vec3::ONE).model();
                let a = matrix.transform_point3(blade[0]);
                let b = matrix.transform_point3(blade[1]);
                let normal = (b - a).cross(p.motion).normalize();
                let board_point = p.guide_center - Vec3::Y * 0.018;
                assert!(normal.dot(board_point - a).abs() < 1e-5);
                assert!((a.y + p.motion.y - (aim.floor - 0.005)).abs() < 1e-5);
                assert!(normal.dot(p.direction).abs() < 1e-5);
            }
        }
    }
    #[test]
    fn cutting_tool_rejects_invalid_aim() {
        let mut aim = ToolAim {
            center: Vec3::ZERO,
            yaw: 0.,
            tilt: 0.,
            height: 3.,
            floor: 0.,
        };
        assert!(aim.pose([Vec3::ZERO, Vec3::X], Vec3::ONE).is_err());
        aim.tilt = f32::NAN;
        assert!(aim.pose([Vec3::ZERO, Vec3::Z], Vec3::ONE).is_err());
    }
    #[test]
    fn cutting_tool_invalid_guide_preserves_existing_pose() {
        let mut scene = crate::scene::SceneManager::default();
        let sword = scene.add_cube(Vec3::new(1., 2., 3.));
        scene
            .set_cutter(sword, Some([Vec3::NEG_Z, Vec3::Z]))
            .unwrap();
        let aim = ToolAim {
            center: Vec3::ZERO,
            yaw: 0.,
            tilt: 0.,
            height: 3.,
            floor: 0.2,
        };
        assert!(scene.aim_cutter(sword, &[9999], aim).is_err());
        assert_eq!(
            scene.objects()[&sword].transform.position,
            Vec3::new(1., 2., 3.)
        );
    }
    #[test]
    fn cutting_surface_guide_tracks_finite_sweep_and_lifecycle() {
        use crate::scene::SceneManager;
        let mut scene = SceneManager::default();
        let sword = scene.add_cube(Vec3::ZERO);
        let guide = scene.add_cube(Vec3::ZERO);
        let blade = [Vec3::new(0., -0.1, -3.), Vec3::new(0., -0.1, 3.)];
        scene.set_cutter(sword, Some(blade)).unwrap();
        for tilt in [-0.4, 0., 0.4] {
            let motion = scene
                .aim_cutter(
                    sword,
                    &[guide],
                    ToolAim {
                        center: Vec3::new(1., 1.15, -2.),
                        yaw: 0.7,
                        tilt,
                        height: 3.6,
                        floor: 0.2,
                    },
                )
                .unwrap();
            let [plane, origin, axis] = scene.cutting_guide_uniforms();
            let normal = Vec3::from_slice(&plane);
            let model = scene.objects()[&sword].transform.model();
            for point in [
                model.transform_point3(blade[0]),
                model.transform_point3(blade[1]),
                model.transform_point3(blade[1]) + motion,
            ] {
                assert!((normal.dot(point) + plane[3]).abs() < 1e-5);
            }
            assert!((normal.length() - 1.).abs() < 1e-5);
            assert!((Vec3::from_slice(&origin).y - 3.5).abs() < 1e-5);
            assert_eq!(origin[3], 0.2);
            assert!((axis[3] - 3.).abs() < 1e-5);
        }
        let previous = scene.cutting_guide_uniforms();
        assert!(scene
            .aim_cutter(
                sword,
                &[9999],
                ToolAim {
                    center: Vec3::ZERO,
                    yaw: 0.,
                    tilt: 0.,
                    height: 3.,
                    floor: 0.2,
                }
            )
            .is_err());
        assert_eq!(
            scene.cutting_guide_uniforms(),
            previous,
            "Invalid preview is atomic"
        );
        scene.set_object_scale(guide, Vec3::splat(0.00001));
        assert_eq!(scene.cutting_guide_uniforms(), [[0.; 4]; 3]);
        scene.set_object_scale(guide, Vec3::ONE);
        assert_eq!(scene.cutting_guide_uniforms(), previous);
        scene.remove_object(sword);
        assert_eq!(scene.cutting_guide_uniforms(), [[0.; 4]; 3]);
        scene.clear();
        assert_eq!(scene.cutting_guide_uniforms(), [[0.; 4]; 3]);
    }
}
