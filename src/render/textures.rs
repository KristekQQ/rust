//! Owned, reusable single-sample render attachments, independent of presentation.
use super::target::{DepthRenderTarget, RenderTarget, Viewport};
pub struct DepthTarget {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub size: u32,
}
impl DepthTarget {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    pub fn target(&self) -> DepthRenderTarget<'_> {
        DepthRenderTarget {
            depth: &self.view,
            format: Self::FORMAT,
            width: self.size,
            height: self.size,
            viewport: Viewport::full(self.size, self.size),
            load: wgpu::LoadOp::Clear(1.0),
        }
    }
    pub fn new(device: &wgpu::Device, label: &str, size: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            texture,
            view,
            size,
        }
    }
}
pub struct ColorTarget {
    pub texture: wgpu::Texture,
    pub color: wgpu::TextureView,
    pub depth_texture: wgpu::Texture,
    pub depth: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
}
impl ColorTarget {
    pub fn new(
        device: &wgpu::Device,
        label: &str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        assert!(width > 0 && height > 0);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color = texture.create_view(&Default::default());
        let (depth_texture, depth) =
            super::depth::create(device, width, height, DepthTarget::FORMAT);
        Self {
            texture,
            color,
            depth_texture,
            depth,
            width,
            height,
            format,
        }
    }
    pub fn target(&self) -> RenderTarget<'_> {
        RenderTarget {
            color: &self.color,
            depth: &self.depth,
            color_format: self.format,
            depth_format: DepthTarget::FORMAT,
            width: self.width,
            height: self.height,
            viewport: Viewport::full(self.width, self.height),
            color_load: wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.1,
                g: 0.1,
                b: 0.3,
                a: 1.0,
            }),
            depth_load: wgpu::LoadOp::Clear(1.0),
        }
    }
}
