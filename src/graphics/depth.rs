use wgpu::{Device, Texture, TextureFormat, TextureView, TextureDescriptor, Extent3d, TextureDimension, TextureUsages};

pub fn create(device: &Device, width: u32, height: u32, format: TextureFormat) -> (Texture, TextureView) {
    let tex = device.create_texture(&TextureDescriptor {
        label: Some("depth texture"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    (tex, view)
}

