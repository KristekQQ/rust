//! Explicit scene appearance; disabled by default to preserve existing scenes.
use glam::{Mat4, Vec3};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub sky: [f32; 3],
    pub ground: [f32; 3],
    pub sun_direction: [f32; 3],
    pub exposure: f32,
    pub fog_color: [f32; 3],
    pub fog_near: f32,
    pub fog_far: f32,
    pub shadow_extent: f32,
    pub shadow_target: [f32; 3],
    pub shadow_near: f32,
    pub shadow_far: f32,
    #[serde(default)]
    pub shadow_normal_bias: f32,
    #[serde(rename = "specularAA")]
    pub specular_aa: bool,
}
#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Appearance {
    pub sky: [f32; 4],
    pub ground: [f32; 4],
    pub sun: [f32; 4],
    pub fog: [f32; 4],
    pub params: [f32; 4],
    pub shadow: [f32; 4],
}
impl Appearance {
    pub fn from_config(c: Config) -> Result<Self, String> {
        let colors = [c.sky, c.ground, c.fog_color];
        let numbers = [
            c.exposure,
            c.fog_near,
            c.fog_far,
            c.shadow_extent,
            c.shadow_near,
            c.shadow_far,
            c.shadow_normal_bias,
        ];
        let direction = Vec3::from_array(c.sun_direction);
        if colors
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || *v < 0.0 || *v > 100.0)
            || numbers.iter().any(|v| !v.is_finite())
            || !direction.is_finite()
            || !direction.length_squared().is_finite()
            || direction.length_squared() < 1e-8
            || !Vec3::from_array(c.shadow_target).is_finite()
            || Vec3::from_array(c.shadow_target).abs().max_element() > 1e6
            || c.exposure <= 0.0
            || c.exposure > 16.0
            || c.fog_near < 0.0
            || c.fog_far - c.fog_near < 0.001
            || c.fog_far > 1e6
            || c.shadow_extent < 0.001
            || c.shadow_extent > 10000.0
            || c.shadow_near < 0.001
            || c.shadow_far - c.shadow_near < 0.001
            || c.shadow_far > 1e6
            || c.shadow_normal_bias < 0.0
            || c.shadow_normal_bias > 1.0
        {
            return Err("Invalid scene appearance".into());
        }
        let d = direction.normalize();
        if !d.is_finite() {
            return Err("Invalid sun direction".into());
        }
        Ok(Self {
            sky: [c.sky[0], c.sky[1], c.sky[2], 1.0],
            ground: [c.ground[0], c.ground[1], c.ground[2], c.shadow_normal_bias],
            sun: [d.x, d.y, d.z, c.exposure],
            fog: [c.fog_color[0], c.fog_color[1], c.fog_color[2], c.fog_near],
            params: [
                c.fog_far,
                f32::from(c.specular_aa),
                c.shadow_extent,
                c.shadow_far,
            ],
            shadow: [
                c.shadow_target[0],
                c.shadow_target[1],
                c.shadow_target[2],
                c.shadow_near,
            ],
        })
    }
    pub fn enabled(&self) -> bool {
        self.sky[3] > 0.5
    }
    pub fn shadow_matrix(&self, position: Vec3) -> Mat4 {
        let direction = Vec3::from_array([self.sun[0], self.sun[1], self.sun[2]]);
        // A directional light uses a parallel projection; position anchors the volume.
        let center = Vec3::new(self.shadow[0], self.shadow[1], self.shadow[2]);
        let position = center + direction * position.distance(center).max(self.shadow[3] + 1.0);
        let target = position - direction;
        let up = if direction.dot(Vec3::Y).abs() > 0.99 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        let extent = self.params[2];
        glam::camera::lh::proj::directx::orthographic(
            -extent,
            extent,
            -extent,
            extent,
            self.shadow[3],
            self.params[3],
        ) * glam::camera::lh::view::look_at_mat4(position, target, up)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Config {
        Config {
            sky: [1.0; 3],
            ground: [0.2; 3],
            sun_direction: [-45.0, 75.0, 50.0],
            exposure: 1.25,
            shadow_normal_bias: 0.12,
            fog_color: [0.5; 3],
            fog_near: 90.0,
            fog_far: 245.0,
            shadow_extent: 95.0,
            shadow_target: [0.0, 0.0, -15.0],
            shadow_near: 1.0,
            shadow_far: 220.0,
            specular_aa: true,
        }
    }
    #[test]
    fn appearance_validates_and_preserves_disabled_default() {
        assert!(!Appearance::default().enabled());
        let a = Appearance::from_config(config()).unwrap();
        assert!(a.enabled());
        assert_eq!(std::mem::size_of::<Appearance>(), 96);
        let m = a.shadow_matrix(Vec3::new(-45.0, 75.0, 35.0));
        let p = m.project_point3(Vec3::ZERO);
        assert!(p.x.abs() < 1.0 && p.y.abs() < 1.0 && (0.0..1.0).contains(&p.z));
        let mut c = config();
        c.fog_far = c.fog_near;
        assert!(Appearance::from_config(c).is_err());
        let mut c = config();
        c.sun_direction = [0.0; 3];
        assert!(Appearance::from_config(c).is_err());
        let mut c = config();
        c.sun_direction = [1e20, 0.0, 0.0];
        assert!(Appearance::from_config(c).is_err());
        let mut c = config();
        c.shadow_extent = 1e-40;
        assert!(Appearance::from_config(c).is_err());
    }
}
