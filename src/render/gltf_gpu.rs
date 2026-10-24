//! Portable glTF rendering: immutable rigid geometry is transformed on the GPU;
//! supported skins use joint palettes, while morph/arbitrary skins retain CPU deformation.
use super::data::as_bytes;
use crate::gltf_model::*;
use glam::{Mat4, Vec3, Vec4};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    tangent: [f32; 4],
    uvs: [[f32; 2]; 5],
    color: [f32; 4],
}
impl Vertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        const A: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x4,3=>Float32x2,4=>Float32x2,5=>Float32x2,6=>Float32x2,7=>Float32x2,8=>Float32x4];
        wgpu::VertexBufferLayout {
            array_stride: 96,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &A,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    base: [f32; 4],
    emissive: [f32; 4],
    factors: [f32; 4],
    uv: [u32; 4],
    extra: [f32; 4],
    options: [f32; 4],
    model: [[f32; 4]; 4],
    normal: [[f32; 4]; 4],
    transform: [f32; 4],
}
struct Material {
    textures: wgpu::BindGroup,
    texture_mask: u8,
    uniform: Uniform,
    alpha: AlphaMode,
    double_sided: bool,
}
const MAX_GPU_JOINTS: usize = 128;
const PALETTE_BYTES: u64 = (MAX_GPU_JOINTS * 64) as u64;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SkinInfluences {
    joints0: [u16; 4],
    joints1: [u16; 4],
    weights0: [f32; 4],
    weights1: [f32; 4],
}
impl SkinInfluences {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        const A: [wgpu::VertexAttribute; 4] =
            wgpu::vertex_attr_array![9=>Uint16x4,10=>Uint16x4,11=>Float32x4,12=>Float32x4];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &A,
        }
    }
}
struct PreparedSkin {
    vertices: Vec<Vertex>,
    influences: Vec<SkinInfluences>,
    mean_terms: Vec<(usize, Vec4)>,
    identity_mean: Vec4,
}
struct SharedSkin {
    vertex: wgpu::Buffer,
    influences: wgpu::Buffer,
    // Sum of w*[position,1]/vertex_count per joint, plus uninfluenced vertices.
    mean_terms: Vec<(usize, Vec4)>,
    identity_mean: Vec4,
}
struct SkinPalette {
    buffer: wgpu::Buffer,
    last: Vec<[[f32; 4]; 4]>,
}
struct Geometry {
    vertex: Option<wgpu::Buffer>,
    skin: Option<Rc<SharedSkin>>,
    index: wgpu::Buffer,
    count: u32,
    topology: wgpu::PrimitiveTopology,
    center: Vec3,
}
struct SharedAsset {
    asset: Arc<ModelAsset>,
    materials: Rc<Vec<Material>>,
    geometry: Vec<Option<Geometry>>,
}
/// Cumulative work counters; resource counts describe currently live shared assets.
#[derive(Clone, Copy, Debug, Default)]
pub struct GltfGpuStats {
    pub cpu_deformed_vertices: u64,
    pub rigid_upload_bytes: u64,
    pub skin_upload_bytes: u64,
    /// Included in uniform_upload_bytes as well.
    pub skin_palette_upload_bytes: u64,
    pub draw_calls: u64,
    pub pipeline_changes: u64,
    pub bind_group_changes: u64,
    pub uniform_upload_bytes: u64,
    pub geometry_cache_hits: u64,
    pub material_cache_hits: u64,
    pub geometry_resources: usize,
    pub material_resources: usize,
    pub cached_assets: usize,
}
struct Draw {
    node: usize,
    primitive: usize,
    material: usize,
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    count: u32,
    topology: wgpu::PrimitiveTopology,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    center: Vec3,
    negative: bool,
    rigid: bool,
    skin: Option<Rc<SharedSkin>>,
    local_center: Vec3,
    last_uniform: Uniform,
    scratch: Vec<Vertex>,
}
pub struct GpuModel {
    draws: Vec<Draw>,
    materials: Rc<Vec<Material>>,
    _shared: Rc<SharedAsset>,
    palettes: HashMap<usize, SkinPalette>,
    last_pose: Option<(u64, Mat4, [f32; 4])>,
    active: HashSet<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PipelineKey {
    pub format: wgpu::TextureFormat,
    pub depth_format: wgpu::TextureFormat,
    pub reflected: bool,
    // Clip-plane use is independent of winding (e.g. environment views).
    pub clipped: bool,
    pub material_mask: bool,
    pub packed_depth: bool,
    pub shadow: bool,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Variant {
    key: PipelineKey,
    texture_mask: u8,
    opaque_unclipped: bool,
    topology: wgpu::PrimitiveTopology,
    blend: bool,
    double_sided: bool,
    negative: bool,
    skinned: bool,
}
pub struct GltfGpu {
    global: wgpu::BindGroupLayout,
    effects: wgpu::BindGroupLayout,
    textures: wgpu::BindGroupLayout,
    material: wgpu::BindGroupLayout,
    skin_material: Option<wgpu::BindGroupLayout>,
    // 32 map classes x (generic color, opaque unclipped color, shadow).
    shaders: HashMap<(u8, bool, bool, bool), wgpu::ShaderModule>,
    empty_layout: wgpu::BindGroupLayout,
    empty_bind: wgpu::BindGroup,
    pipelines: HashMap<Variant, wgpu::RenderPipeline>,
    assets: RefCell<HashMap<usize, Weak<SharedAsset>>>,
    counters: Cell<GltfGpuStats>,
}
impl GltfGpu {
    pub fn new(
        device: &wgpu::Device,
        global: &wgpu::BindGroupLayout,
        effects: &wgpu::BindGroupLayout,
    ) -> Self {
        let mut entries = Vec::new();
        for i in 0..5 {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: i * 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: i * 2 + 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
        }
        let textures = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glTF five material maps"),
            entries: &entries,
        });
        let material = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glTF material"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Uniform>() as u64),
                },
                count: None,
            }],
        });
        let limits = device.limits();
        let skin_material = (limits.max_vertex_attributes >= 13
            && limits.max_vertex_buffers >= 2
            && limits.max_uniform_buffer_binding_size >= PALETTE_BYTES)
            .then(|| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("glTF GPU skin material and palette"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(
                                    std::mem::size_of::<Uniform>() as u64,
                                ),
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::VERTEX,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(PALETTE_BYTES),
                            },
                            count: None,
                        },
                    ],
                })
            });
        let empty_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glTF empty shadow effects"),
            entries: &[],
        });
        let empty_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glTF empty shadow effects"),
            layout: &empty_layout,
            entries: &[],
        });
        Self {
            global: global.clone(),
            effects: effects.clone(),
            textures,
            material,
            skin_material,
            shaders: HashMap::new(),
            empty_layout,
            empty_bind,
            pipelines: HashMap::new(),
            assets: RefCell::new(HashMap::new()),
            counters: Cell::new(GltfGpuStats::default()),
        }
    }
    pub fn upload(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        asset: &Arc<ModelAsset>,
    ) -> Result<GpuModel, String> {
        let identity = Arc::as_ptr(asset) as usize;
        let shared = {
            let mut cache = self.assets.borrow_mut();
            cache.retain(|_, entry| entry.strong_count() > 0);
            cache.get(&identity).and_then(Weak::upgrade)
        };
        if let Some(shared) = shared {
            let mut stats = self.counters.get();
            stats.geometry_cache_hits += shared.geometry.iter().flatten().count() as u64;
            stats.material_cache_hits += shared.materials.len() as u64;
            self.counters.set(stats);
            return Ok(self.create_model(device, shared));
        }
        let limits = device.limits();
        for image in &asset.images {
            if image.width == 0
                || image.height == 0
                || image.width > limits.max_texture_dimension_2d
                || image.height > limits.max_texture_dimension_2d
                || image.rgba.len() as u64 != image.width as u64 * image.height as u64 * 4
            {
                return Err("glTF image exceeds device limits or has invalid pixels".into());
            }
        }
        for p in &asset.primitives {
            if p.vertices.is_empty()
                || p.vertices.end > asset.vertices.len()
                || p.vertices.len() as u64 * 96 > limits.max_buffer_size
                || p.indices.len() as u64 * 12 > limits.max_buffer_size
                || p.indices.iter().any(|i| *i as usize >= p.vertices.len())
            {
                return Err(
                    "glTF primitive exceeds device buffer limits or has invalid indices".into(),
                );
            }
        }
        // Bound the aggregate allocation before creating any GPU resources.
        let mut image_roles = std::collections::HashSet::new();
        let mut budget = 0u64;
        for material in &asset.materials {
            for (slot, srgb) in [
                (material.base_color_texture, true),
                (material.metallic_roughness_texture, false),
                (material.normal_texture, false),
                (material.occlusion_texture, false),
                (material.emissive_texture, true),
            ] {
                if let Some(slot) = slot {
                    let texture = asset
                        .textures
                        .get(slot.texture)
                        .ok_or("Invalid material texture")?;
                    let image = asset
                        .images
                        .get(texture.image)
                        .ok_or("Invalid texture image")?;
                    if texture.sampler.is_some_and(|s| s >= asset.samplers.len()) {
                        return Err("Invalid texture sampler".into());
                    }
                    // Per texture because samplers may differ even for one image.
                    if image_roles.insert((slot.texture, srgb)) {
                        budget = budget.saturating_add(image.rgba.len() as u64 * 2);
                    }
                }
            }
        }
        for node in &asset.nodes {
            if let Some(mesh) = node.mesh {
                for &primitive in &asset
                    .meshes
                    .get(mesh)
                    .ok_or("Invalid node mesh")?
                    .primitives
                {
                    let primitive = asset
                        .primitives
                        .get(primitive)
                        .ok_or("Invalid mesh primitive")?;
                    if primitive
                        .material
                        .is_some_and(|m| m >= asset.materials.len())
                    {
                        return Err("Invalid primitive material".into());
                    }
                    budget = budget.saturating_add(
                        primitive.vertices.len() as u64 * 96
                            + primitive.indices.len().max(primitive.vertices.len()) as u64 * 12
                            + std::mem::size_of::<Uniform>() as u64,
                    );
                    if self.skin_material.is_some()
                        && node
                            .skin
                            .and_then(|i| asset.skins.get(i))
                            .is_some_and(|s| s.joints.len() <= MAX_GPU_JOINTS)
                        && primitive.morph_targets.is_empty()
                    {
                        // Existing budget covers source vertices; add influence
                        // attributes and one conservative palette per draw.
                        budget = budget
                            .saturating_add(primitive.vertices.len() as u64 * 48 + PALETTE_BYTES);
                    }
                }
            }
        }
        if budget > 512 * 1024 * 1024 {
            return Err("glTF resources exceed aggregate GPU upload budget".into());
        }
        let mut materials = Vec::new();
        let mut texture_cache: HashMap<(usize, bool), (wgpu::TextureView, wgpu::Sampler)> =
            HashMap::new();
        if asset
            .images
            .iter()
            .map(|i| i.rgba.len() as u64)
            .sum::<u64>()
            > 256 * 1024 * 1024
        {
            return Err("glTF images exceed GPU upload budget".into());
        }
        for source in asset
            .materials
            .iter()
            .map(Some)
            .chain(std::iter::once(None))
        {
            let slots = source
                .map(|m| {
                    [
                        m.base_color_texture,
                        m.metallic_roughness_texture,
                        m.normal_texture,
                        m.occlusion_texture,
                        m.emissive_texture,
                    ]
                })
                .unwrap_or([None; 5]);
            let texture_mask = slots.iter().enumerate().fold(0u8, |mask, (i, slot)| {
                mask | if slot.is_some() { 1 << i } else { 0 }
            });
            let mut views = Vec::new();
            let mut samplers = Vec::new();
            for (i, slot) in slots.iter().enumerate() {
                let srgb = i == 0 || i == 4;
                let cache_key = (
                    slot.map(|v| v.texture)
                        .unwrap_or(usize::MAX - if i == 2 { 1 } else { 0 }),
                    srgb,
                );
                if let Some((view, sampler)) = texture_cache.get(&cache_key) {
                    views.push(view.clone());
                    samplers.push(sampler.clone());
                    continue;
                }
                let (width, height, pixels, sampler) = if let Some(slot) = slot {
                    let tex = asset
                        .textures
                        .get(slot.texture)
                        .ok_or("Invalid material texture")?;
                    let img = asset.images.get(tex.image).ok_or("Invalid texture image")?;
                    (
                        img.width,
                        img.height,
                        img.rgba.clone(),
                        tex.sampler.and_then(|s| asset.samplers.get(s)),
                    )
                } else {
                    (
                        1,
                        1,
                        if i == 2 {
                            vec![128, 128, 255, 255]
                        } else {
                            vec![255; 4]
                        },
                        None,
                    )
                };
                let levels = 32 - width.max(height).leading_zeros();
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("glTF material texture"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: levels,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: if srgb {
                        wgpu::TextureFormat::Rgba8UnormSrgb
                    } else {
                        wgpu::TextureFormat::Rgba8Unorm
                    },
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                let (mut w, mut h, mut rgba) = (width, height, pixels);
                for level in 0..levels {
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &texture,
                            mip_level: level,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &rgba,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(w * 4),
                            rows_per_image: Some(h),
                        },
                        wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                    );
                    if level + 1 < levels {
                        let nw = (w / 2).max(1);
                        let nh = (h / 2).max(1);
                        let mut next = vec![0; nw as usize * nh as usize * 4];
                        for y in 0..nh {
                            for x in 0..nw {
                                for c in 0..4 {
                                    let mut sum = 0.0f32;
                                    for dy in 0..2 {
                                        for dx in 0..2 {
                                            let value = rgba[(((y * 2 + dy).min(h - 1) * w
                                                + (x * 2 + dx).min(w - 1))
                                                * 4
                                                + c)
                                                as usize]
                                                as f32
                                                / 255.0;
                                            sum += if srgb && c < 3 {
                                                if value <= 0.04045 {
                                                    value / 12.92
                                                } else {
                                                    ((value + 0.055) / 1.055).powf(2.4)
                                                }
                                            } else {
                                                value
                                            };
                                        }
                                    }
                                    next[((y * nw + x) * 4 + c) as usize] = {
                                        let value = sum / 4.0;
                                        let encoded = if srgb && c < 3 {
                                            if value <= 0.0031308 {
                                                value * 12.92
                                            } else {
                                                1.055 * value.powf(1.0 / 2.4) - 0.055
                                            }
                                        } else {
                                            value
                                        };
                                        (encoded.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
                                    };
                                }
                            }
                        }
                        rgba = next;
                        w = nw;
                        h = nh;
                    }
                }
                views.push(texture.create_view(&Default::default()));
                let min = sampler
                    .and_then(|s| s.min_filter)
                    .unwrap_or(FilterMode::LinearMipmapLinear);
                let mag = sampler
                    .and_then(|s| s.mag_filter)
                    .unwrap_or(FilterMode::Linear);
                let filter = |f| {
                    if matches!(
                        f,
                        FilterMode::Nearest
                            | FilterMode::NearestMipmapNearest
                            | FilterMode::NearestMipmapLinear
                    ) {
                        wgpu::FilterMode::Nearest
                    } else {
                        wgpu::FilterMode::Linear
                    }
                };
                samplers.push(device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("glTF sampler"),
                    address_mode_u: wrap(sampler.map(|s| s.wrap_s)),
                    address_mode_v: wrap(sampler.map(|s| s.wrap_t)),
                    mag_filter: filter(mag),
                    min_filter: filter(min),
                    mipmap_filter: if matches!(
                        min,
                        FilterMode::NearestMipmapLinear | FilterMode::LinearMipmapLinear
                    ) {
                        wgpu::MipmapFilterMode::Linear
                    } else {
                        wgpu::MipmapFilterMode::Nearest
                    },
                    lod_max_clamp: if matches!(min, FilterMode::Nearest | FilterMode::Linear) {
                        0.0
                    } else {
                        32.0
                    },
                    ..Default::default()
                }));
                texture_cache.insert(
                    cache_key,
                    (
                        views.last().unwrap().clone(),
                        samplers.last().unwrap().clone(),
                    ),
                );
            }
            let mut entries = Vec::new();
            for i in 0..5 {
                entries.push(wgpu::BindGroupEntry {
                    binding: i * 2,
                    resource: wgpu::BindingResource::TextureView(&views[i as usize]),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: i * 2 + 1,
                    resource: wgpu::BindingResource::Sampler(&samplers[i as usize]),
                });
            }
            let textures = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("glTF maps"),
                layout: &self.textures,
                entries: &entries,
            });
            let alpha = source.map(|m| m.alpha_mode).unwrap_or(AlphaMode::Opaque);
            let double_sided = source.is_some_and(|m| m.double_sided);
            let uniform = Uniform {
                base: source.map(|m| m.base_color_factor).unwrap_or([1.0; 4]),
                emissive: source
                    .map(|m| {
                        [
                            m.emissive_factor[0],
                            m.emissive_factor[1],
                            m.emissive_factor[2],
                            slots[4].map(|s| s.tex_coord as f32).unwrap_or(0.0),
                        ]
                    })
                    .unwrap_or([0.0; 4]),
                factors: source
                    .map(|m| {
                        [
                            m.metallic_factor,
                            m.roughness_factor,
                            m.normal_scale,
                            m.occlusion_strength,
                        ]
                    })
                    .unwrap_or([1.0; 4]),
                uv: std::array::from_fn(|i| slots[i].map(|s| s.tex_coord).unwrap_or(0)),
                extra: [
                    source.map(|m| m.alpha_cutoff).unwrap_or(0.5),
                    match alpha {
                        AlphaMode::Opaque => 0.0,
                        AlphaMode::Mask => 1.0,
                        AlphaMode::Blend => 2.0,
                    },
                    if double_sided { 1.0 } else { 0.0 },
                    0.0,
                ],
                options: [0.0; 4],
                model: Mat4::IDENTITY.to_cols_array_2d(),
                normal: Mat4::IDENTITY.to_cols_array_2d(),
                transform: [1.0, 0.0, 0.0, 0.0],
            };
            materials.push(Material {
                textures,
                texture_mask,
                uniform,
                alpha,
                double_sided,
            });
        }
        let mut geometry = Vec::with_capacity(asset.primitives.len());
        let mut stats = self.counters.get();
        for (primitive, p) in asset.primitives.iter().enumerate() {
            if !asset.nodes.iter().any(|n| {
                n.mesh
                    .is_some_and(|m| asset.meshes[m].primitives.contains(&primitive))
            }) {
                geometry.push(None);
                continue;
            }
            let (indices, topology) = indices(p);
            let has_rigid_node = p.morph_targets.is_empty()
                && asset.nodes.iter().any(|n| {
                    n.skin.is_none()
                        && n.mesh
                            .is_some_and(|m| asset.meshes[m].primitives.contains(&primitive))
                });
            let slots = material_slots(asset, p);
            let mut center = Vec3::ZERO;
            let vertex = if has_rigid_node {
                let vertices: Vec<_> = asset.vertices[p.vertices.clone()]
                    .iter()
                    .map(|v| {
                        center += Vec3::from(v.position);
                        // Match deformed_vertices normalization and Gram-Schmidt.
                        let n = Vec3::from(v.normal).try_normalize().unwrap_or(Vec3::Y);
                        let t = Vec3::from_slice(&v.tangent[..3]);
                        let t = (t - n * n.dot(t)).try_normalize().unwrap_or(Vec3::ZERO);
                        packed_vertex(v, Vec3::from(v.position), n, t, 1.0, &slots)
                    })
                    .collect();
                center /= vertices.len().max(1) as f32;
                stats.rigid_upload_bytes += (vertices.len() * 96 + indices.len() * 4) as u64;
                Some(
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("glTF shared rigid vertices"),
                        contents: as_bytes(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                )
            } else {
                None
            };
            let index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("glTF shared indices"),
                contents: as_bytes(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            let has_skin_node = self.skin_material.is_some()
                && p.morph_targets.is_empty()
                && asset.nodes.iter().any(|n| {
                    n.mesh
                        .is_some_and(|m| asset.meshes[m].primitives.contains(&primitive))
                        && n.skin
                            .and_then(|i| asset.skins.get(i))
                            .is_some_and(|skin| skin.joints.len() <= MAX_GPU_JOINTS)
                });
            let skin = if has_skin_node {
                prepare_skin(asset, p, &slots).map(|prepared| {
                    let PreparedSkin {
                        vertices,
                        influences,
                        mean_terms,
                        identity_mean,
                    } = prepared;
                    stats.skin_upload_bytes += (vertices.len() * 96
                        + influences.len() * std::mem::size_of::<SkinInfluences>())
                        as u64;
                    if vertex.is_none() {
                        stats.skin_upload_bytes += (indices.len() * 4) as u64;
                    }
                    Rc::new(SharedSkin {
                        vertex: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("glTF shared skin source vertices"),
                            contents: as_bytes(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        }),
                        influences: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("glTF shared eight influences"),
                            contents: as_bytes(&influences),
                            usage: wgpu::BufferUsages::VERTEX,
                        }),
                        mean_terms,
                        identity_mean,
                    })
                })
            } else {
                None
            };
            geometry.push(Some(Geometry {
                vertex,
                skin,
                index,
                count: indices.len() as u32,
                topology,
                center,
            }));
        }
        self.counters.set(stats);
        let shared = Rc::new(SharedAsset {
            asset: asset.clone(),
            materials: Rc::new(materials),
            geometry,
        });
        self.assets
            .borrow_mut()
            .insert(identity, Rc::downgrade(&shared));
        Ok(self.create_model(device, shared))
    }
    /// Snapshot counters without retaining any asset or GPU resource.
    pub fn stats(&self) -> GltfGpuStats {
        let mut stats = self.counters.get();
        for shared in self.assets.borrow().values().filter_map(Weak::upgrade) {
            stats.cached_assets += 1;
            stats.geometry_resources += shared.geometry.iter().flatten().count();
            stats.material_resources += shared.materials.len();
        }
        stats
    }
    fn create_model(&self, device: &wgpu::Device, shared: Rc<SharedAsset>) -> GpuModel {
        let asset = &shared.asset;
        let materials = shared.materials.clone();
        let mut palettes: HashMap<usize, SkinPalette> = HashMap::new();
        let mut draws = Vec::new();
        for (node, n) in asset.nodes.iter().enumerate() {
            let Some(mesh) = n.mesh else { continue };
            for &primitive in &asset.meshes[mesh].primitives {
                let p = &asset.primitives[primitive];
                let geometry = shared.geometry[primitive]
                    .as_ref()
                    .expect("referenced primitive geometry");
                let rigid = n.skin.is_none() && p.morph_targets.is_empty();
                let skin = n
                    .skin
                    .and_then(|i| asset.skins.get(i))
                    .filter(|skin| skin.joints.len() <= MAX_GPU_JOINTS)
                    .and_then(|_| geometry.skin.clone());
                if skin.is_some() && !palettes.contains_key(&node) {
                    let initial = vec![Mat4::IDENTITY.to_cols_array_2d(); MAX_GPU_JOINTS];
                    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("glTF node skin palette"),
                        contents: as_bytes(&initial),
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    });
                    palettes.insert(
                        node,
                        SkinPalette {
                            buffer,
                            last: Vec::with_capacity(MAX_GPU_JOINTS),
                        },
                    );
                }
                let material = p.material.unwrap_or(asset.materials.len());
                let vertex = if let Some(skin) = &skin {
                    skin.vertex.clone()
                } else if rigid {
                    geometry
                        .vertex
                        .as_ref()
                        .expect("rigid primitive buffer")
                        .clone()
                } else {
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("glTF CPU deformed vertices"),
                        size: p.vertices.len() as u64 * 96,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    })
                };
                let initial = materials[material].uniform;
                let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("glTF material and transform"),
                    contents: as_bytes(&[initial]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });
                let mut entries = vec![wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                }];
                if let Some(palette) = palettes.get(&node).filter(|_| skin.is_some()) {
                    entries.push(wgpu::BindGroupEntry {
                        binding: 1,
                        resource: palette.buffer.as_entire_binding(),
                    });
                }
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("glTF model material"),
                    layout: if skin.is_some() {
                        self.skin_material.as_ref().expect("supported skin layout")
                    } else {
                        &self.material
                    },
                    entries: &entries,
                });
                draws.push(Draw {
                    node,
                    primitive,
                    material,
                    vertex,
                    index: geometry.index.clone(),
                    count: geometry.count,
                    topology: geometry.topology,
                    uniform,
                    bind,
                    center: Vec3::ZERO,
                    negative: false,
                    rigid,
                    skin,
                    local_center: geometry.center,
                    last_uniform: initial,
                    scratch: Vec::new(),
                });
            }
        }
        let mut stats = self.counters.get();
        stats.uniform_upload_bytes += (draws.len() * std::mem::size_of::<Uniform>()) as u64
            + palettes.len() as u64 * PALETTE_BYTES;
        stats.skin_palette_upload_bytes += palettes.len() as u64 * PALETTE_BYTES;
        self.counters.set(stats);
        GpuModel {
            draws,
            materials,
            _shared: shared,
            palettes,
            last_pose: None,
            active: HashSet::new(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Explicit GPU/FFI parameter layout.
    pub fn update(
        &self,
        queue: &wgpu::Queue,
        model: &mut GpuModel,
        asset: &ModelAsset,
        pose: &ModelPose,
        revision: u64,
        root: Mat4,
        options: [f32; 4],
    ) -> Result<usize, String> {
        let _scope = super::profiler::cpu_scope("gltf.update");
        let stamp = (revision, root, options);
        if model.last_pose.as_ref() == Some(&stamp) {
            return Ok(0);
        }
        model.active.clear();
        model.active.extend(pose.active_nodes.iter().copied());
        let mut stats = self.counters.get();
        let mut bytes = 0;
        for (&node, palette) in &mut model.palettes {
            if !model.active.contains(&node) {
                continue;
            }
            let matrices = pose
                .skin_matrices
                .get(node)
                .ok_or("Pose skin node out of range")?;
            if matrices.len() > MAX_GPU_JOINTS {
                return Err("GPU skin palette exceeds supported size".into());
            }
            if palette.last.len() != matrices.len()
                || !palette
                    .last
                    .iter()
                    .zip(matrices)
                    .all(|(a, b)| *a == b.to_cols_array_2d())
            {
                palette.last.clear();
                palette
                    .last
                    .extend(matrices.iter().map(|matrix| matrix.to_cols_array_2d()));
                if !palette.last.is_empty() {
                    queue.write_buffer(&palette.buffer, 0, as_bytes(&palette.last));
                    let uploaded = palette.last.len() * 64;
                    bytes += uploaded;
                    stats.uniform_upload_bytes += uploaded as u64;
                    stats.skin_palette_upload_bytes += uploaded as u64;
                }
            }
        }
        for draw in &mut model.draws {
            if !model.active.contains(&draw.node) {
                continue;
            }
            let primitive = &asset.primitives[draw.primitive];
            let matrix = root * crate::gltf_animation::node_render_matrix(pose, draw.node);
            let inverse = matrix.inverse().transpose();
            let normal = if inverse.is_finite() { inverse } else { matrix };
            draw.negative = matrix.determinant() < 0.0;
            if draw.rigid {
                // Exact affine vertex mean, rather than the bounds center.
                draw.center = matrix.transform_point3(draw.local_center);
            } else if let Some(skin) = &draw.skin {
                let matrices = &pose.skin_matrices[draw.node];
                let mut mean = skin.identity_mean;
                for &(joint, term) in &skin.mean_terms {
                    mean += *matrices
                        .get(joint)
                        .ok_or("Vertex joint index out of skin range")?
                        * term;
                }
                draw.center = matrix.transform_point3(mean.truncate());
            } else {
                let _deform_scope = super::profiler::cpu_scope("gltf.deform");
                let slots = material_slots(asset, primitive);
                draw.scratch.clear();
                let mut center = Vec3::ZERO;
                for v in crate::gltf_animation::deformed_vertices(
                    asset,
                    pose,
                    draw.node,
                    draw.primitive,
                )? {
                    let pos = matrix.transform_point3(Vec3::from(v.position));
                    let n = normal
                        .transform_vector3(Vec3::from(v.normal))
                        .normalize_or_zero();
                    let t = matrix
                        .transform_vector3(Vec3::from_slice(&v.tangent[..3]))
                        .normalize_or_zero();
                    center += pos;
                    draw.scratch.push(packed_vertex(
                        &v,
                        pos,
                        n,
                        t,
                        if draw.negative { -1.0 } else { 1.0 },
                        &slots,
                    ));
                }
                draw.center = center / draw.scratch.len().max(1) as f32;
                queue.write_buffer(&draw.vertex, 0, as_bytes(&draw.scratch));
                bytes += draw.scratch.len() * 96;
                stats.cpu_deformed_vertices += draw.scratch.len() as u64;
            }
            let mut uniform = model.materials[draw.material].uniform;
            if draw.rigid || draw.skin.is_some() {
                uniform.model = matrix.to_cols_array_2d();
                uniform.normal = normal.to_cols_array_2d();
                uniform.transform = [if draw.negative { -1.0 } else { 1.0 }, 1.0, 0.0, 0.0];
            }
            uniform.options = options;
            uniform.options[3] = if primitive.flat_normals { 1.0 } else { 0.0 };
            uniform.extra[3] = if matches!(
                draw.topology,
                wgpu::PrimitiveTopology::PointList
                    | wgpu::PrimitiveTopology::LineList
                    | wgpu::PrimitiveTopology::LineStrip
            ) {
                1.0
            } else {
                0.0
            };
            if uniform != draw.last_uniform {
                queue.write_buffer(&draw.uniform, 0, as_bytes(&[uniform]));
                draw.last_uniform = uniform;
                bytes += std::mem::size_of::<Uniform>();
                stats.uniform_upload_bytes += std::mem::size_of::<Uniform>() as u64;
            }
        }
        self.counters.set(stats);
        model.last_pose = Some(stamp);
        Ok(bytes)
    }
    pub fn prepare_pipeline(&mut self, device: &wgpu::Device, key: PipelineKey, model: &GpuModel) {
        for draw in &model.draws {
            let topology = draw.topology;
            let material = &model.materials[draw.material];
            let blend = material.alpha == AlphaMode::Blend;
            // Transparent glTF is drawn into the final single-color overlay.
            // Do not create an unused MRT alpha pipeline: baseline WebGL2 has
            // no independent blending between color and packed-depth targets.
            if blend && (key.material_mask || key.packed_depth) {
                continue;
            }
            let double_sided = material.double_sided;
            for negative in [false, true] {
                let variant = Variant {
                    key,
                    texture_mask: material.texture_mask,
                    opaque_unclipped: material.alpha == AlphaMode::Opaque
                        && !key.clipped
                        && !key.shadow,
                    topology,
                    blend,
                    double_sided,
                    negative,
                    skinned: draw.skin.is_some(),
                };
                if self.pipelines.contains_key(&variant) {
                    continue;
                }
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("glTF pipeline layout"),
                    bind_group_layouts: &[
                        Some(&self.global),
                        Some(if key.shadow {
                            &self.empty_layout
                        } else {
                            &self.effects
                        }),
                        Some(&self.textures),
                        Some(if variant.skinned {
                            self.skin_material.as_ref().expect("supported skin layout")
                        } else {
                            &self.material
                        }),
                    ],
                    immediate_size: 0,
                });
                let shader = self
                    .shaders
                    .entry((
                        material.texture_mask,
                        key.shadow,
                        variant.opaque_unclipped,
                        variant.skinned,
                    ))
                    .or_insert_with(|| {
                        device.create_shader_module(wgpu::ShaderModuleDescriptor {
                            label: Some("glTF material-class shader"),
                            source: wgpu::ShaderSource::Wgsl(
                                material_shader_source(
                                    material.texture_mask,
                                    key.shadow,
                                    variant.opaque_unclipped,
                                    variant.skinned,
                                )
                                .into(),
                            ),
                        })
                    });
                let mut targets = Vec::new();
                if !key.shadow {
                    targets.push(Some(wgpu::ColorTargetState {
                        format: key.format,
                        blend: Some(if blend {
                            wgpu::BlendState::ALPHA_BLENDING
                        } else {
                            wgpu::BlendState::REPLACE
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    }));
                    if key.packed_depth {
                        targets.push(Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        }));
                    }
                }
                let vertex_buffers = [Some(Vertex::layout()), Some(SkinInfluences::layout())];
                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("glTF PBR variant"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: shader,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: if variant.skinned {
                            &vertex_buffers
                        } else {
                            &vertex_buffers[..1]
                        },
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: shader,
                        entry_point: Some(if key.shadow {
                            "fs_shadow"
                        } else if key.packed_depth {
                            "fs_postprocess"
                        } else if key.material_mask {
                            "fs_material_mask"
                        } else {
                            "fs_main"
                        }),
                        compilation_options: Default::default(),
                        targets: &targets,
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology,
                        strip_index_format: if matches!(
                            topology,
                            wgpu::PrimitiveTopology::LineStrip
                                | wgpu::PrimitiveTopology::TriangleStrip
                        ) {
                            Some(wgpu::IndexFormat::Uint32)
                        } else {
                            None
                        },
                        cull_mode: if double_sided || key.shadow {
                            None
                        } else {
                            Some(if key.reflected ^ negative {
                                wgpu::Face::Back
                            } else {
                                wgpu::Face::Front
                            })
                        },
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: key.depth_format,
                        depth_write_enabled: Some(key.shadow || !blend),
                        depth_compare: Some(wgpu::CompareFunction::LessEqual),
                        stencil: Default::default(),
                        bias: if key.shadow {
                            wgpu::DepthBiasState {
                                constant: 2,
                                slope_scale: 2.0,
                                clamp: 0.0,
                            }
                        } else {
                            Default::default()
                        },
                    }),
                    multisample: Default::default(),
                    multiview_mask: None,
                    cache: None,
                });
                self.pipelines.insert(variant, pipeline);
            }
        }
    }
    pub fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        model: &GpuModel,
        key: PipelineKey,
        blend: bool,
        camera_pos: Vec3,
    ) -> usize {
        self.draw_many(pass, std::iter::once(model), key, blend, camera_pos)
    }
    /// Sort transparent primitives across all visible model roots.
    pub fn draw_many<'a>(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        models: impl Iterator<Item = &'a GpuModel>,
        key: PipelineKey,
        blend: bool,
        camera_pos: Vec3,
    ) -> usize {
        let mut stats = self.counters.get();
        if key.shadow {
            pass.set_bind_group(1, &self.empty_bind, &[]);
            stats.bind_group_changes += 1;
        }
        let mut draws: Vec<_> = models
            .flat_map(|model| {
                model
                    .draws
                    .iter()
                    .filter(move |d| {
                        model
                            .last_pose
                            .as_ref()
                            .is_some_and(|_| model.active.contains(&d.node))
                            && (model.materials[d.material].alpha == AlphaMode::Blend) == blend
                    })
                    .map(move |d| (model, d))
            })
            .collect();
        if blend {
            draws.sort_by(|(_, a), (_, b)| {
                b.center
                    .distance_squared(camera_pos)
                    .total_cmp(&a.center.distance_squared(camera_pos))
            });
        }
        let mut submitted = 0;
        let mut last_variant = None;
        let mut last_material = None;
        for (model, draw) in draws {
            let material = &model.materials[draw.material];
            let variant = Variant {
                key,
                texture_mask: material.texture_mask,
                opaque_unclipped: material.alpha == AlphaMode::Opaque
                    && !key.clipped
                    && !key.shadow,
                topology: draw.topology,
                blend,
                double_sided: material.double_sided,
                negative: draw.negative,
                skinned: draw.skin.is_some(),
            };
            let Some(pipeline) = self.pipelines.get(&variant) else {
                continue;
            };
            if last_variant != Some(variant) {
                pass.set_pipeline(pipeline);
                stats.pipeline_changes += 1;
                last_variant = Some(variant);
            }
            let material_identity = material as *const Material;
            if last_material != Some(material_identity) {
                pass.set_bind_group(2, &material.textures, &[]);
                stats.bind_group_changes += 1;
                last_material = Some(material_identity);
            }
            pass.set_bind_group(3, &draw.bind, &[]);
            stats.bind_group_changes += 1;
            pass.set_vertex_buffer(0, draw.vertex.slice(..));
            if let Some(skin) = &draw.skin {
                pass.set_vertex_buffer(1, skin.influences.slice(..));
            }
            pass.set_index_buffer(draw.index.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..draw.count, 0, 0..1);
            submitted += 1;
            stats.draw_calls += 1;
        }
        self.counters.set(stats);
        submitted
    }
}
// Replace sample expressions before compilation so absent maps incur no texture
// operation on either backend. Raw fallback texels preserve the original decoder.
fn material_shader_source(
    texture_mask: u8,
    shadow: bool,
    opaque_unclipped: bool,
    skinned: bool,
) -> String {
    let mut source = concat!(
        include_str!("gltf_pbr.wgsl"),
        "\n",
        include_str!("appearance.wgsl")
    )
    .to_owned();
    if skinned {
        source = source.replacen("@location(8) color:vec4<f32> };", "@location(8) color:vec4<f32>, @location(9) joints0:vec4<u32>, @location(10) joints1:vec4<u32>, @location(11) weights0:vec4<f32>, @location(12) weights1:vec4<f32> };", 1);
        source = source.replace("    // GPU_SKIN_VERTEX", "    let deformed=skin_vertex(v); local_position=deformed.position; local_normal=deformed.normal; local_tangent=deformed.tangent;");
    } else {
        // Exclude the entire palette/helper declarations from rigid variants.
        let start = source.find("// GPU_SKIN_BEGIN").expect("skin helper start");
        let end =
            source.find("// GPU_SKIN_END").expect("skin helper end") + "// GPU_SKIN_END".len();
        source.replace_range(start..end, "");
    }
    for (i, (sample, fallback)) in [
        ("textureSample(base_map,base_sampler,uv)", "vec4<f32>(1.0)"),
        ("textureSample(mr_map,mr_sampler,uv)", "vec4<f32>(1.0)"),
        (
            "textureSample(normal_map,normal_sampler,uv)",
            "vec4<f32>(128.0/255.0,128.0/255.0,1.0,1.0)",
        ),
        ("textureSample(ao_map,ao_sampler,uv)", "vec4<f32>(1.0)"),
        (
            "textureSample(emissive_map,emissive_sampler,uv)",
            "vec4<f32>(1.0)",
        ),
    ]
    .iter()
    .enumerate()
    {
        if texture_mask & (1 << i) == 0 {
            source = source.replace(sample, fallback);
        }
    }
    if opaque_unclipped && !shadow {
        // Remove only the color entrypoints' discard call. fs_shadow continues
        // to reach alpha_test and retains MASK/BLEND shadow coverage.
        source = source.replace("    surface_alpha_clip(v,base.a);", "");
    }
    if shadow {
        source = source
            .replace(
                "var<uniform> scene: SceneUniforms;",
                "var<uniform> shadow_view: mat4x4<f32>;",
            )
            .replace("scene.view_projection", "shadow_view")
            .replace("scene.camera_pos", "vec3(0.0)")
            .replace("scene.lights[i]", "Light(vec3(0.0),0.0,vec3(0.0),0.0)");
        // Shadow variants expose only the view matrix. Color-only helpers are
        // still parsed by both backends even though fs_shadow cannot reach them.
        for field in ["sky", "ground", "sun", "fog", "params", "shadow"] {
            // Keep fog edges distinct: WebGPU rejects constant smoothstep(0,0).
            let value = if field == "params" {
                "vec4<f32>(1.0)"
            } else {
                "vec4<f32>(0.0)"
            };
            source = source.replace(&format!("scene.appearance_{field}"), value);
        }
    }
    source
}
pub fn draw_count(model: &GpuModel, blend: bool) -> usize {
    model
        .last_pose
        .as_ref()
        .map(|_| {
            model
                .draws
                .iter()
                .filter(|d| {
                    model.active.contains(&d.node)
                        && (model.materials[d.material].alpha == AlphaMode::Blend) == blend
                })
                .count()
        })
        .unwrap_or(0)
}
fn wrap(mode: Option<WrapMode>) -> wgpu::AddressMode {
    match mode.unwrap_or(WrapMode::Repeat) {
        WrapMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        WrapMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
        WrapMode::Repeat => wgpu::AddressMode::Repeat,
    }
}
fn indices(p: &ModelPrimitive) -> (Vec<u32>, wgpu::PrimitiveTopology) {
    let input = if p.indices.is_empty() {
        (0..p.vertices.len() as u32).collect()
    } else {
        p.indices.clone()
    };
    match p.mode {
        PrimitiveMode::Points => (input, wgpu::PrimitiveTopology::PointList),
        PrimitiveMode::Lines => (input, wgpu::PrimitiveTopology::LineList),
        PrimitiveMode::LineStrip => (input, wgpu::PrimitiveTopology::LineStrip),
        PrimitiveMode::Triangles => (input, wgpu::PrimitiveTopology::TriangleList),
        PrimitiveMode::TriangleStrip => (input, wgpu::PrimitiveTopology::TriangleStrip),
        PrimitiveMode::LineLoop => {
            let mut out = Vec::new();
            for i in 0..input.len() {
                out.push(input[i]);
                out.push(input[(i + 1) % input.len()]);
            }
            (out, wgpu::PrimitiveTopology::LineList)
        }
        PrimitiveMode::TriangleFan => {
            let mut out = Vec::new();
            for i in 1..input.len().saturating_sub(1) {
                out.extend_from_slice(&[input[0], input[i], input[i + 1]]);
            }
            (out, wgpu::PrimitiveTopology::TriangleList)
        }
    }
}

fn material_slots(asset: &ModelAsset, primitive: &ModelPrimitive) -> [Option<TextureSlot>; 5] {
    primitive
        .material
        .and_then(|m| asset.materials.get(m))
        .map(|m| {
            [
                m.base_color_texture,
                m.metallic_roughness_texture,
                m.normal_texture,
                m.occlusion_texture,
                m.emissive_texture,
            ]
        })
        .unwrap_or([None; 5])
}
fn packed_vertex(
    v: &ModelVertex,
    pos: Vec3,
    n: Vec3,
    t: Vec3,
    sign: f32,
    slots: &[Option<TextureSlot>; 5],
) -> Vertex {
    Vertex {
        position: pos.into(),
        normal: n.into(),
        tangent: [t.x, t.y, t.z, v.tangent[3] * sign],
        uvs: std::array::from_fn(|i| {
            v.tex_coords
                .get(slots[i].map(|s| s.tex_coord as usize).unwrap_or(0))
                .copied()
                .unwrap_or([0.0; 2])
        }),
        color: v.color,
    }
}

// Preparation is one-off per immutable primitive. Keep nonzero influence order:
// merging duplicates or renormalizing would change the authoritative CPU math.
fn prepare_skin(
    asset: &ModelAsset,
    primitive: &ModelPrimitive,
    slots: &[Option<TextureSlot>; 5],
) -> Option<PreparedSkin> {
    let source = &asset.vertices[primitive.vertices.clone()];
    let mut vertices = Vec::with_capacity(source.len());
    let mut influences = Vec::with_capacity(source.len());
    let mut terms = vec![Vec4::ZERO; MAX_GPU_JOINTS];
    let mut identity = Vec4::ZERO;
    for v in source {
        let mut packed = SkinInfluences {
            joints0: [0; 4],
            joints1: [0; 4],
            weights0: [0.0; 4],
            weights1: [0.0; 4],
        };
        let mut count = 0;
        let mut mass = 0.0;
        if v.influences.is_empty() {
            identity += Vec3::from(v.position).extend(1.0);
        }
        for &(joint, weight) in &v.influences {
            if !weight.is_finite() || weight < 0.0 {
                return None;
            }
            if weight == 0.0 {
                continue;
            }
            if count >= 8 || joint as usize >= MAX_GPU_JOINTS {
                return None;
            }
            if count < 4 {
                packed.joints0[count] = joint;
                packed.weights0[count] = weight;
            } else {
                packed.joints1[count - 4] = joint;
                packed.weights1[count - 4] = weight;
            }
            terms[joint as usize] += Vec3::from(v.position).extend(1.0) * weight;
            mass += weight;
            count += 1;
        }
        if !v.influences.is_empty() && (!mass.is_finite() || mass <= 0.0) {
            return None;
        }
        vertices.push(packed_vertex(
            v,
            Vec3::from(v.position),
            Vec3::from(v.normal),
            Vec3::from_slice(&v.tangent[..3]),
            1.0,
            slots,
        ));
        influences.push(packed);
    }
    let divisor = source.len().max(1) as f32;
    let terms = terms
        .into_iter()
        .enumerate()
        .filter(|(_, q)| *q != Vec4::ZERO)
        .map(|(j, q)| (j, q / divisor))
        .collect();
    Some(PreparedSkin {
        vertices,
        influences,
        mean_terms: terms,
        identity_mean: identity / divisor,
    })
}
