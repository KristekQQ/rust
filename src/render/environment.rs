//! A static local probe. Six independently culled views, captured only on scene/light changes.
use super::{
    renderer::SceneRenderer,
    target::{RenderTarget, RenderView, Viewport},
    textures::DepthTarget,
};
use glam::Vec3;

pub struct EnvironmentProbe {
    _texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    faces: [wgpu::TextureView; 6],
    depth: DepthTarget,
    pub center: Vec3,
    pub half_extents: Vec3,
    revision: Option<(u64, Vec<(Vec3, Vec3)>)>,
    pub captures: u32,
}
impl EnvironmentProbe {
    pub fn new(device: &wgpu::Device, center: Vec3, half_extents: Vec3) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("static environment cubemap"),
            size: wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let faces = std::array::from_fn(|i| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: i as u32,
                array_layer_count: Some(1),
                ..Default::default()
            })
        });
        Self {
            _texture: texture,
            view,
            faces,
            depth: DepthTarget::new(device, "environment depth", 256),
            center,
            half_extents,
            revision: None,
            captures: 0,
        }
    }
    pub fn update(&mut self, renderer: &mut SceneRenderer) -> Result<(), String> {
        let key = (
            renderer.scene.static_revision(),
            renderer
                .scene
                .lights()
                .values()
                .map(|l| (l.position, l.color))
                .collect(),
        );
        if self.revision.as_ref() == Some(&key) {
            return Ok(());
        }
        let directions = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
        let ups = [Vec3::Y, Vec3::Y, -Vec3::Z, Vec3::Z, Vec3::Y, Vec3::Y];
        for i in 0..6 {
            let matrix = glam::camera::lh::proj::directx::perspective(
                std::f32::consts::FRAC_PI_2,
                1.0,
                0.05,
                100.0,
            ) * glam::camera::lh::view::look_at_mat4(
                self.center,
                self.center + directions[i],
                ups[i],
            );
            let target = RenderTarget {
                color: &self.faces[i],
                depth: &self.depth.view,
                auxiliary: None,
                material_mask: false,
                color_format: wgpu::TextureFormat::Rgba8UnormSrgb,
                depth_format: DepthTarget::FORMAT,
                width: 256,
                height: 256,
                viewport: Viewport::full(256, 256),
                color_load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: 0.002,
                    g: 0.004,
                    b: 0.012,
                    a: 1.0,
                }),
                depth_load: wgpu::LoadOp::Clear(1.0),
            };
            renderer.render_environment(
                RenderView {
                    view_projection: matrix,
                    camera_position: self.center,
                },
                &target,
            )?;
        }
        self.revision = Some(key);
        self.captures += 1;
        Ok(())
    }
}
