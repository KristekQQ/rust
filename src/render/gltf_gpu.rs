//! Portable glTF rendering: deformation is computed in Rust once per changed pose,
//! and the same world-space vertex buffers serve main/reflection/shadow passes.
use super::data::as_bytes;
use crate::gltf_model::*;
use glam::{Mat4, Vec3};
use std::collections::{HashMap, HashSet};
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
}
struct Material {
    textures: wgpu::BindGroup,
    uniform: Uniform,
    alpha: AlphaMode,
    double_sided: bool,
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
}
pub struct GpuModel {
    draws: Vec<Draw>,
    materials: Vec<Material>,
    last_pose: Option<(u64, Mat4, [f32; 4])>,
    active: HashSet<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PipelineKey {
    pub format: wgpu::TextureFormat,
    pub depth_format: wgpu::TextureFormat,
    pub reflected: bool,
    pub material_mask: bool,
    pub packed_depth: bool,
    pub shadow: bool,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Variant {
    key: PipelineKey,
    topology: wgpu::PrimitiveTopology,
    blend: bool,
    double_sided: bool,
    negative: bool,
}
pub struct GltfGpu {
    global: wgpu::BindGroupLayout,
    effects: wgpu::BindGroupLayout,
    textures: wgpu::BindGroupLayout,
    material: wgpu::BindGroupLayout,
    shader: wgpu::ShaderModule,
    shadow: wgpu::ShaderModule,
    empty_layout: wgpu::BindGroupLayout,
    empty_bind: wgpu::BindGroup,
    pipelines: HashMap<Variant, wgpu::RenderPipeline>,
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
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(96),
                },
                count: None,
            }],
        });
        let source = include_str!("gltf_pbr.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("glTF PBR"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let shadow_source = source
            .replace(
                "var<uniform> scene: SceneUniforms;",
                "var<uniform> shadow_view: mat4x4<f32>;",
            )
            .replace("scene.view_projection", "shadow_view")
            .replace("scene.camera_pos", "vec3(0.0)")
            .replace("scene.lights[i]", "Light(vec3(0.0),0.0,vec3(0.0),0.0)");
        let shadow = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("glTF masked shadow"),
            source: wgpu::ShaderSource::Wgsl(shadow_source.into()),
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
            shader,
            shadow,
            empty_layout,
            empty_bind,
            pipelines: HashMap::new(),
        }
    }
    pub fn upload(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        asset: &ModelAsset,
    ) -> Result<GpuModel, String> {
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
                            + 96,
                    );
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
            };
            materials.push(Material {
                textures,
                uniform,
                alpha,
                double_sided,
            });
        }
        let mut draws = Vec::new();
        for (node, n) in asset.nodes.iter().enumerate() {
            let Some(mesh) = n.mesh else { continue };
            for &primitive in &asset.meshes[mesh].primitives {
                let p = &asset.primitives[primitive];
                let (indices, topology) = indices(p);
                let material = p.material.unwrap_or(asset.materials.len());
                let vertex = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("glTF posed vertices"),
                    size: p.vertices.len() as u64 * 96,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("glTF indices"),
                    contents: as_bytes(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
                let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("glTF material options"),
                    contents: as_bytes(&[materials[material].uniform]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("glTF model material"),
                    layout: &self.material,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    }],
                });
                draws.push(Draw {
                    node,
                    primitive,
                    material,
                    vertex,
                    index,
                    count: indices.len() as u32,
                    topology,
                    uniform,
                    bind,
                    center: Vec3::ZERO,
                    negative: false,
                });
            }
        }
        Ok(GpuModel {
            draws,
            materials,
            last_pose: None,
            active: HashSet::new(),
        })
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
        let stamp = (revision, root, options);
        if model.last_pose.as_ref() == Some(&stamp) {
            return Ok(0);
        }
        model.active = pose.active_nodes.iter().copied().collect();
        let mut bytes = 0;
        for draw in &mut model.draws {
            if !model.active.contains(&draw.node) {
                continue;
            }
            let primitive = &asset.primitives[draw.primitive];
            let matrix = root * crate::gltf_animation::node_render_matrix(pose, draw.node);
            let inverse = matrix.inverse().transpose();
            let normal = if inverse.is_finite() { inverse } else { matrix };
            draw.negative = matrix.determinant() < 0.0;
            let source = primitive.material.and_then(|m| asset.materials.get(m));
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
            let mut vertices = Vec::with_capacity(primitive.vertices.len());
            let mut center = Vec3::ZERO;
            for v in
                crate::gltf_animation::deformed_vertices(asset, pose, draw.node, draw.primitive)?
            {
                let mut pos = Vec3::from(v.position);
                let mut n = Vec3::from(v.normal);
                let mut t = Vec3::from_slice(&v.tangent[..3]);
                pos = matrix.transform_point3(pos);
                n = normal.transform_vector3(n).normalize_or_zero();
                t = matrix.transform_vector3(t).normalize_or_zero();
                center += pos;
                vertices.push(Vertex {
                    position: pos.into(),
                    normal: n.into(),
                    tangent: [
                        t.x,
                        t.y,
                        t.z,
                        v.tangent[3] * if draw.negative { -1.0 } else { 1.0 },
                    ],
                    uvs: std::array::from_fn(|i| {
                        v.tex_coords
                            .get(slots[i].map(|s| s.tex_coord as usize).unwrap_or(0))
                            .copied()
                            .unwrap_or([0.0; 2])
                    }),
                    color: v.color,
                });
            }
            draw.center = center / vertices.len().max(1) as f32;
            queue.write_buffer(&draw.vertex, 0, as_bytes(&vertices));
            bytes += vertices.len() * 96;
            let mut uniform = model.materials[draw.material].uniform;
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
            queue.write_buffer(&draw.uniform, 0, as_bytes(&[uniform]));
            bytes += 96;
        }
        model.last_pose = Some(stamp);
        Ok(bytes)
    }
    pub fn prepare_pipeline(&mut self, device: &wgpu::Device, key: PipelineKey, model: &GpuModel) {
        for draw in &model.draws {
            let topology = draw.topology;
            let material = &model.materials[draw.material];
            let blend = material.alpha == AlphaMode::Blend;
            let double_sided = material.double_sided;
            for negative in [false, true] {
                let variant = Variant {
                    key,
                    topology,
                    blend,
                    double_sided,
                    negative,
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
                        Some(&self.material),
                    ],
                    immediate_size: 0,
                });
                let shader = if key.shadow {
                    &self.shadow
                } else {
                    &self.shader
                };
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
                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("glTF PBR variant"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: shader,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[Some(Vertex::layout())],
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
        if key.shadow {
            pass.set_bind_group(1, &self.empty_bind, &[]);
        }
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
        if key.shadow {
            pass.set_bind_group(1, &self.empty_bind, &[]);
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
        for (model, draw) in draws {
            let material = &model.materials[draw.material];
            let variant = Variant {
                key,
                topology: draw.topology,
                blend,
                double_sided: material.double_sided,
                negative: draw.negative,
            };
            let Some(pipeline) = self.pipelines.get(&variant) else {
                continue;
            };
            pass.set_pipeline(pipeline);
            pass.set_bind_group(2, &material.textures, &[]);
            pass.set_bind_group(3, &draw.bind, &[]);
            pass.set_vertex_buffer(0, draw.vertex.slice(..));
            pass.set_index_buffer(draw.index.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..draw.count, 0, 0..1);
            submitted += 1;
        }
        submitted
    }
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
