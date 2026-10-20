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
struct AttachmentInfo {
    width: u32,
    height: u32,
    layers: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
}
fn inspect_attachment(
    value: &JsValue,
    layer: u32,
    opaque_color: bool,
) -> Result<AttachmentInfo, JsValue> {
    let width = uint(value, "width")?;
    let height = uint(value, "height")?;
    let layers = uint(value, "depthOrArrayLayers")?;
    let mut mip_levels = uint(value, "mipLevelCount")?;
    let mut samples = uint(value, "sampleCount")?;
    let mut usage = wgpu::TextureUsages::from_bits_truncate(uint(value, "usage")?);
    // Quest Browser 149 returns all three as zero for compositor-owned color
    // textures. The projection layer is explicitly created single-sampled,
    // single-mip, with RENDER_ATTACHMENT usage. Do not relax other metadata or
    // apply this compatibility path to ordinary/depth textures.
    if opaque_color && mip_levels == 0 && samples == 0 && usage.is_empty() {
        mip_levels = 1;
        samples = 1;
        usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
    }
    let format = format(value)?;
    if width == 0
        || height == 0
        || layer >= layers
        || samples != 1
        || mip_levels != 1
        || property(value, "dimension")?.as_string().as_deref() != Some("2d")
        || !usage.contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
    {
        return Err(JsValue::from_str("Invalid XR projection attachment"));
    }
    Ok(AttachmentInfo {
        width,
        height,
        layers,
        format,
        usage,
    })
}
fn wrap(
    device: &wgpu::Device,
    value: JsValue,
    info: &AttachmentInfo,
    layer: u32,
) -> wgpu::TextureView {
    let texture = device.create_texture_from_webgpu_handle(
        value.unchecked_into(),
        &wgpu::TextureDescriptor {
            label: Some("browser-owned XR attachment"),
            size: wgpu::Extent3d {
                width: info.width,
                height: info.height,
                depth_or_array_layers: info.layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: info.format,
            usage: info.usage,
            view_formats: &[],
        },
        None,
    );
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2),
        base_array_layer: layer,
        array_layer_count: Some(1),
        ..Default::default()
    })
}
/// Validated projection metadata without browser handles or GPU resources.
pub struct XrImageInfo {
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    pub layer: u32,
}
struct InspectedImage {
    color: JsValue,
    depth: JsValue,
    color_info: AttachmentInfo,
    depth_info: AttachmentInfo,
    layer: u32,
}
impl InspectedImage {
    fn info(&self) -> XrImageInfo {
        XrImageInfo {
            width: self.color_info.width,
            height: self.color_info.height,
            format: self.color_info.format,
            layer: self.layer,
        }
    }
}
pub struct XrAttachments {
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
}
fn inspect_image(image: &JsValue) -> Result<InspectedImage, JsValue> {
    let descriptor = property(image, "viewDescriptor")?;
    let layer = uint(&descriptor, "baseArrayLayer")?;
    if property(&descriptor, "dimension")?.as_string().as_deref() != Some("2d")
        || uint(&descriptor, "arrayLayerCount")? != 1
    {
        return Err(JsValue::from_str("XR requires one 2d array layer per eye"));
    }
    let color = property(image, "colorTexture")?;
    let color_info = inspect_attachment(&color, layer, true)?;
    let depth = property(image, "depthStencilTexture")?;
    let depth_info = inspect_attachment(&depth, layer, false)?;
    let (width, height) = (color_info.width, color_info.height);
    let viewport = property(image, "viewport")?;
    // Projection layers have an independent full-size array slice per eye.
    // Reject an atlas explicitly: postprocessing must never overwrite the other eye.
    if depth_info.format != wgpu::TextureFormat::Depth32Float
        || (width, height) != (depth_info.width, depth_info.height)
        || uint(&viewport, "x")? != 0
        || uint(&viewport, "y")? != 0
        || uint(&viewport, "width")? != width
        || uint(&viewport, "height")? != height
    {
        return Err(JsValue::from_str(
            "XR projection requires matching full-size color/depth slices",
        ));
    }
    Ok(InspectedImage {
        color,
        depth,
        color_info,
        depth_info,
        layer,
    })
}
impl XrAttachments {
    /// Validate an eye's attachments without importing textures or creating views.
    pub fn inspect(image: &JsValue) -> Result<XrImageInfo, JsValue> {
        Ok(inspect_image(image)?.info())
    }
    pub fn new(device: &wgpu::Device, image: &JsValue) -> Result<Self, JsValue> {
        if device.as_webgpu().is_none() {
            return Err(JsValue::from_str("XR projection requires WebGPU"));
        }
        let inspected = inspect_image(image)?;
        let info = inspected.info();
        let color = wrap(device, inspected.color, &inspected.color_info, info.layer);
        let depth = wrap(device, inspected.depth, &inspected.depth_info, info.layer);
        Ok(Self {
            color,
            depth,
            format: info.format,
            width: info.width,
            height: info.height,
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
