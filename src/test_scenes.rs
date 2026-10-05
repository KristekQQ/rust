//! Native test fixtures only. Browser examples live in examples/scenes.js.
use crate::scene::{MeshKind, RenderOptions, SceneManager, Transform};
use glam::Vec3;
impl SceneManager {
    pub fn load_culling_demo(&mut self) {
        self.clear();
        for x in 0..20 {
            for y in 0..25 {
                for z in 0..20 {
                    let position = Vec3::new(
                        (x as f32 - 9.5) * 3.0,
                        (y as f32 - 12.0) * 1.8,
                        (z as f32 - 9.5) * 3.0,
                    );
                    self.add_object(
                        MeshKind::Cube,
                        Transform::new(position, Vec3::ZERO, Vec3::splat(0.65)),
                    );
                }
            }
        }
        for position in [
            Vec3::new(-20.0, 20.0, -20.0),
            Vec3::new(20.0, 20.0, 20.0),
            Vec3::new(-20.0, -20.0, 20.0),
            Vec3::new(20.0, -20.0, -20.0),
        ] {
            self.add_light(position, Vec3::splat(0.35));
        }
    }
    pub fn load_effects_demo(&mut self) {
        self.clear();
        self.paused = false;
        let static_options = RenderOptions {
            is_static: true,
            ..Default::default()
        };
        let floor = self.add_object(
            MeshKind::Plane,
            Transform::new(
                Vec3::new(0.0, -1.0, 0.0),
                Vec3::ZERO,
                Vec3::new(12.0, 1.0, 12.0),
            ),
        );
        self.set_render_options(
            floor,
            RenderOptions {
                casts_shadow: false,
                ..static_options
            },
        );
        let mirror = self.add_object(
            MeshKind::Plane,
            Transform::new(
                Vec3::new(0.0, 0.5, -3.0),
                Vec3::new(std::f32::consts::FRAC_PI_2, 0.0, 0.0),
                Vec3::new(5.5, 1.0, 3.0),
            ),
        );
        self.mirror_object = Some(mirror);
        self.set_render_options(
            mirror,
            RenderOptions {
                casts_shadow: false,
                receives_shadow: false,
                reflectivity: 0.94,
                ..static_options
            },
        );
        for position in [
            Vec3::new(-2.3, -0.3, 0.0),
            Vec3::new(2.2, -0.5, 1.2),
            Vec3::new(-1.5, -0.5, -1.8),
        ] {
            let id = self.add_cube(position);
            self.set_render_options(id, static_options);
        }
        // Thin frame makes the finite mirror surface easy to identify.
        for (position, scale) in [
            (Vec3::new(0.0, 2.08, -3.0), Vec3::new(5.8, 0.16, 0.18)),
            (Vec3::new(0.0, -1.08, -3.0), Vec3::new(5.8, 0.16, 0.18)),
            (Vec3::new(-2.83, 0.5, -3.0), Vec3::new(0.16, 3.3, 0.18)),
            (Vec3::new(2.83, 0.5, -3.0), Vec3::new(0.16, 3.3, 0.18)),
        ] {
            let id = self.add_object(MeshKind::Cube, Transform::new(position, Vec3::ZERO, scale));
            self.set_render_options(id, static_options);
        }
        let cube = self.add_object(
            MeshKind::Cube,
            Transform::new(
                Vec3::new(0.0, 0.1, 0.0),
                Vec3::ZERO,
                Vec3::new(1.2, 1.5, 1.0),
            ),
        );
        self.set_object_spin(cube, Vec3::new(0.25, 0.8, 0.1));
        let sphere = self.add_object(
            MeshKind::Sphere,
            Transform::new(
                Vec3::new(1.8, 0.5, -1.2),
                Vec3::ZERO,
                Vec3::new(0.5, 1.0, 0.5),
            ),
        );
        self.set_object_spin(sphere, Vec3::new(0.0, 0.0, 0.55));
        // Warm key light and blue fill; both illuminate the scene and its reflection.
        self.add_light(Vec3::new(-2.5, 3.0, -1.5), Vec3::new(0.8, 0.224, 0.056));
        self.add_light(Vec3::new(2.5, 2.0, 2.0), Vec3::new(0.052, 0.234, 0.65));
    }
    pub fn load_example(&mut self) {
        self.clear();
        self.add_object(
            MeshKind::Plane,
            Transform::new(
                Vec3::new(0.0, -0.6, 0.0),
                Vec3::ZERO,
                Vec3::new(8.0, 1.0, 8.0),
            ),
        );
        self.add_cube(Vec3::ZERO);
        self.add_object(
            MeshKind::Sphere,
            Transform::new(Vec3::new(1.8, 0.4, 0.2), Vec3::ZERO, Vec3::splat(0.6)),
        );
        self.add_light(Vec3::new(2.5, 3.0, 2.5), Vec3::ONE);
        self.add_light(Vec3::new(-2.0, 1.5, -1.5), Vec3::new(0.2, 0.6, 1.0));
    }
}
