//! Frame-scoped wrappers of browser-owned XR projection attachments.
//! Dropping a wgpu wrapper never destroys the compositor's GPUTexture.
use super::target::{RenderTarget, Viewport};
use wasm_bindgen::{JsCast, JsValue};

fn property(object: &JsValue, key: &str) -> Result<JsValue, JsValue> {
    js_sys::Reflect::get(object, &JsValue::from_str(key))
}
fn uint(object: &JsValue, key: &str) -> Result<u32, JsValue> {
    let n = property(object, key)?
        .as_f64()
        .ok_or_else(|| JsValue::from_str("Invalid XR texture metadata"))?;
    if !n.is_finite() || n < 0. || n > u32::MAX as f64 || n.fract() != 0. {
        return Err(JsValue::from_str("Invalid XR texture dimensions"));
    }
    Ok(n as u32)
}
fn format(value: &JsValue) -> Result<wgpu::TextureFormat, JsValue> {
    match property(value, "format")?.as_string().as_deref() {
        Some("rgba8unorm") => Ok(wgpu::TextureFormat::Rgba8Unorm),
        Some("rgba8unorm-srgb") => Ok(wgpu::TextureFormat::Rgba8UnormSrgb),
        Some("bgra8unorm") => Ok(wgpu::TextureFormat::Bgra8Unorm),
        Some("bgra8unorm-srgb") => Ok(wgpu::TextureFormat::Bgra8UnormSrgb),
        Some("depth32float") => Ok(wgpu::TextureFormat::Depth32Float),
        _ => Err(JsValue::from_str("Unsupported XR attachment format")),
    }
}
fn wrap(
    device: &wgpu::Device,
    value: JsValue,
    layer: u32,
    opaque_color: bool,
) -> Result<(wgpu::TextureView, wgpu::TextureFormat, u32, u32), JsValue> {
    let width = uint(&value, "width")?;
    let height = uint(&value, "height")?;
    let layers = uint(&value, "depthOrArrayLayers")?;
    let mut mip_levels = uint(&value, "mipLevelCount")?;
    let mut samples = uint(&value, "sampleCount")?;
    let mut usage = wgpu::TextureUsages::from_bits_truncate(uint(&value, "usage")?);
    // Quest Browser 149 returns all three as zero for compositor-owned color
    // textures. The projection layer is explicitly created single-sampled,
    // single-mip, with RENDER_ATTACHMENT usage. Do not relax other metadata or
    // apply this compatibility path to ordinary/depth textures.
    if opaque_color && mip_levels == 0 && samples == 0 && usage.is_empty() {
        mip_levels = 1;
        samples = 1;
        usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
    }
    let format = format(&value)?;
    if width == 0
        || height == 0
        || layer >= layers
        || samples != 1
        || mip_levels != 1
        || property(&value, "dimension")?.as_string().as_deref() != Some("2d")
        || !usage.contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
    {
        return Err(JsValue::from_str("Invalid XR projection attachment"));
    }
    let texture = device.create_texture_from_webgpu_handle(
        value.unchecked_into(),
        &wgpu::TextureDescriptor {
            label: Some("browser-owned XR attachment"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        },
        None,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2),
        base_array_layer: layer,
        array_layer_count: Some(1),
        ..Default::default()
    });
    Ok((view, format, width, height))
}
pub struct XrAttachments {
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
}
impl XrAttachments {
    pub fn new(device: &wgpu::Device, image: &JsValue) -> Result<Self, JsValue> {
        if device.as_webgpu().is_none() {
            return Err(JsValue::from_str("XR projection requires WebGPU"));
        }
        let descriptor = property(image, "viewDescriptor")?;
        let layer = uint(&descriptor, "baseArrayLayer")?;
        if property(&descriptor, "dimension")?.as_string().as_deref() != Some("2d")
            || uint(&descriptor, "arrayLayerCount")? != 1
        {
            return Err(JsValue::from_str("XR requires one 2d array layer per eye"));
        }
        let (color, format, width, height) =
            wrap(device, property(image, "colorTexture")?, layer, true)?;
        let (depth, depth_format, dw, dh) = wrap(
            device,
            property(image, "depthStencilTexture")?,
            layer,
            false,
        )?;
        let viewport = property(image, "viewport")?;
        // Projection layers have an independent full-size array slice per eye.
        // Reject an atlas explicitly: postprocessing must never overwrite the other eye.
        if depth_format != wgpu::TextureFormat::Depth32Float
            || (width, height) != (dw, dh)
            || uint(&viewport, "x")? != 0
            || uint(&viewport, "y")? != 0
            || uint(&viewport, "width")? != width
            || uint(&viewport, "height")? != height
        {
            return Err(JsValue::from_str(
                "XR projection requires matching full-size color/depth slices",
            ));
        }
        Ok(Self {
            color,
            depth,
            format,
            width,
            height,
        })
    }
    pub fn target(&self) -> RenderTarget<'_> {
        RenderTarget {
            color: &self.color,
            depth: &self.depth,
            auxiliary: None,
            material_mask: false,
            color_format: self.format,
            depth_format: wgpu::TextureFormat::Depth32Float,
            width: self.width,
            height: self.height,
            viewport: Viewport::full(self.width, self.height),
            color_load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            depth_load: wgpu::LoadOp::Clear(1.),
        }
    }
}
