//! Depth-aware fragment postprocessing, independent of a browser or surface.
use super::{
    data::as_bytes,
    target::{RenderTarget, RenderView},
    textures::ColorTarget,
};
use glam::{Vec2, Vec3};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    inverse_view_projection: [[f32; 4]; 4],
    view_projection: [[f32; 4]; 4],
    camera: [f32; 4],
    light_position: [f32; 4],
    light_color: [f32; 4],
    // strength, final output flag, width, height; all fields are 16-byte aligned.
    params: [f32; 4],
    mask_params: [f32; 4],
    shadow_view_projection: [[f32; 4]; 4],
    volume_params: [f32; 4],
    environment_center: [f32; 4],
    environment_half_extents: [f32; 4],
    sky_cube_params: [f32; 4],
    sky_cube_center: [f32; 4],
    sky_cube_half_extents: [f32; 4],
    // Ray steps, refinement steps, quadratic spacing; XR keeps the same range.
    reflection_trace: [f32; 4],
}
// Rust offsets match WGSL matrices and vec4 fields (384 bytes total).
const _: () = assert!(std::mem::size_of::<Uniforms>() == 384);
const _: () = assert!(std::mem::offset_of!(Uniforms, params) == 176);
const _: () = assert!(std::mem::offset_of!(Uniforms, environment_center) == 288);

// Add a WGSL fragment entry point and a descriptor to extend the chain.
// All passes read original scene depth; intermediate alpha is material metadata.
struct EffectDescriptor {
    name: &'static str,
    entry_point: &'static str,
    shader_source: &'static str,
}
const EFFECTS: &[EffectDescriptor] = &[
    EffectDescriptor {
        name: "environment",
        entry_point: "fs_environment",
        shader_source: include_str!("postprocess.wgsl"),
    },
    EffectDescriptor {
        name: "ssr",
        entry_point: "fs_ssr",
        shader_source: include_str!("postprocess.wgsl"),
    },
    EffectDescriptor {
        name: "godRays",
        entry_point: "fs_god_rays",
        shader_source: include_str!("postprocess.wgsl"),
    },
    EffectDescriptor {
        name: "bloom",
        entry_point: "fs_bloom",
        shader_source: include_str!("postprocess.wgsl"),
    },
];
struct Effect {
    name: &'static str,
    enabled: bool,
    strength: f32,
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
}
struct Intermediate {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}
impl Intermediate {
    fn new(device: &wgpu::Device, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("postprocess ping-pong"),
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
        let view = texture.create_view(&Default::default());
        Self {
            _texture: texture,
            view,
        }
    }
}
struct ReflectionMask {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    params: [f32; 4],
    enabled: bool,
    needs_white_upload: bool,
}
impl ReflectionMask {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("world XZ reflection mask"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            texture,
            view,
            params: [0.0, 0.0, 1.0, 1.0],
            enabled: false,
            needs_white_upload: true,
        }
    }
    fn upload(&self, queue: &wgpu::Queue, width: u32, height: u32, pixels: &[u8]) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }
}
struct Targets {
    scene: ColorTarget,
    depth_color: Intermediate,
    ping: [Intermediate; 2],
    bind_groups: Vec<[wgpu::BindGroup; 3]>,
}
pub struct PostProcessor {
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    depth_sampler: wgpu::Sampler,
    shadow_layout: wgpu::BindGroupLayout,
    shadow_sampler: wgpu::Sampler,
    shadow_bind_group: wgpu::BindGroup,
    shadow_views: Option<(wgpu::TextureView, wgpu::TextureView)>,
    effects: Vec<Effect>,
    reflection_mask: ReflectionMask,
    sky_cube_color: wgpu::TextureView,
    sky_cube_emission: wgpu::TextureView,
    sky_cube_enabled: bool,
    sky_cube_intensity: f32,
    sky_cube_projection: Option<(Vec3, Vec3)>,
    environment_fallback: wgpu::TextureView,
    environment_view: Option<wgpu::TextureView>,
    environment_center: Vec3,
    environment_half_extents: Vec3,
    targets: Option<Targets>,
    submitted_frames: u64,
    last_pass_count: usize,
    volume_density: f32,
    volume_exposure: f32,
}
impl PostProcessor {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("postprocess layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<Uniforms>() as u64
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("postprocess linear clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let depth_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("postprocess nearest depth"),
            ..Default::default()
        });
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("volume shadow layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("volume comparison sampler"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // Valid fallback resources until the renderer supplies its persistent maps.
        // Their contents are not sampled when shadows_enabled is false.
        let fallback = super::textures::DepthTarget::new(device, "volume shadow fallback", 1);
        let shadow_bind_group = Self::shadow_group(
            device,
            &shadow_layout,
            &shadow_sampler,
            &fallback.view,
            &fallback.view,
        );
        let effects = EFFECTS
            .iter()
            .map(|descriptor| {
                let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(descriptor.name),
                    source: wgpu::ShaderSource::Wgsl(descriptor.shader_source.into()),
                });
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("postprocess pipeline layout"),
                        bind_group_layouts: &[Some(&layout), Some(&shadow_layout)],
                        immediate_size: 0,
                    });
                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("depth-aware postprocess"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some(descriptor.entry_point),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: Some(wgpu::BlendState::REPLACE),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    multiview_mask: None,
                    cache: None,
                });
                Effect {
                    name: descriptor.name,
                    pipeline,
                    enabled: false,
                    strength: 0.5,
                    uniform: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(descriptor.name),
                        size: std::mem::size_of::<Uniforms>() as u64,
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }),
                }
            })
            .collect();
        // A cube binding must remain valid even while environment reflections are disabled.
        // wgpu initializes this texture to zero; the shader skips it without an environment.
        let environment_fallback = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("postprocess environment fallback"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 6,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            });
        let sky_cube_fallback = Self::sky_cube_texture(device, 1);
        Self {
            sky_cube_color: sky_cube_fallback.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            }),
            sky_cube_emission: sky_cube_fallback.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            }),
            sky_cube_enabled: false,
            sky_cube_intensity: 1.2,
            sky_cube_projection: None,
            format,
            layout,
            sampler,
            depth_sampler,
            shadow_layout,
            shadow_sampler,
            shadow_bind_group,
            shadow_views: None,
            effects,
            reflection_mask: ReflectionMask::new(device, 1, 1),
            environment_fallback,
            environment_view: None,
            environment_center: Vec3::ZERO,
            environment_half_extents: Vec3::ONE,
            targets: None,
            submitted_frames: 0,
            last_pass_count: 0,
            volume_density: 0.035,
            volume_exposure: 2.5,
        }
    }
    fn sky_cube_texture(device: &wgpu::Device, size: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("photographic environment cube"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn set_environment_cube_map(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: u32,
        color: &[u8],
        emission: &[u8],
        intensity: f32,
        projection: &[f32],
    ) -> bool {
        // Validate all projection input before allocating or replacing textures.
        let projection = match projection {
            [] => None,
            [cx, cy, cz, hx, hy, hz]
                if [cx, cy, cz].iter().all(|v| v.is_finite() && v.abs() <= 1e6)
                    && [hx, hy, hz]
                        .iter()
                        .all(|v| v.is_finite() && **v > 0.0 && **v <= 1e6) =>
            {
                Some((Vec3::new(*cx, *cy, *cz), Vec3::new(*hx, *hy, *hz)))
            }
            _ => return false,
        };
        if size == 0
            || size > 2048
            || size > device.limits().max_texture_dimension_2d
            || color.len() != size as usize * size as usize * 6 * 4
            || emission.len() != color.len()
            || !intensity.is_finite()
            || !(0.0..=8.0).contains(&intensity)
        {
            return false;
        }
        let upload = |pixels: &[u8]| {
            let texture = Self::sky_cube_texture(device, size);
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size * 4),
                    rows_per_image: Some(size),
                },
                wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 6,
                },
            );
            texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            })
        };
        self.sky_cube_color = upload(color);
        self.sky_cube_emission = upload(emission);
        self.sky_cube_enabled = true;
        self.sky_cube_intensity = intensity;
        self.sky_cube_projection = projection;
        self.targets = None;
        true
    }
    pub fn clear_environment_cube_map(&mut self) {
        self.sky_cube_enabled = false;
        self.sky_cube_projection = None;
        // Reuse the existing zero cube; dropping views and bind groups releases uploads.
        self.sky_cube_color = self.environment_fallback.clone();
        self.sky_cube_emission = self.environment_fallback.clone();
        self.targets = None;
        self.last_pass_count = 0;
    }
    pub fn set_environment_intensity(&mut self, intensity: f32) -> bool {
        if !intensity.is_finite() || !(0.0..=8.0).contains(&intensity) {
            return false;
        }
        self.sky_cube_intensity = intensity;
        true
    }
    fn shadow_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        static_map: &wgpu::TextureView,
        dynamic_map: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("volume first-light shadows"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(static_map),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(dynamic_map),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
    /// Rebind only when the renderer replaces its first-light shadow textures.
    pub fn set_shadow_maps(
        &mut self,
        device: &wgpu::Device,
        static_map: &wgpu::TextureView,
        dynamic_map: &wgpu::TextureView,
    ) {
        if self
            .shadow_views
            .as_ref()
            .is_some_and(|(cached_static, cached_dynamic)| {
                cached_static == static_map && cached_dynamic == dynamic_map
            })
        {
            return;
        }
        self.shadow_bind_group = Self::shadow_group(
            device,
            &self.shadow_layout,
            &self.shadow_sampler,
            static_map,
            dynamic_map,
        );
        self.shadow_views = Some((static_map.clone(), dynamic_map.clone()));
    }
    /// R8 bytes map world XZ to UV: (position - center) / size + 0.5.
    /// Wet/dry base-color shading is applied by the SSR pass only.
    #[allow(clippy::too_many_arguments)] // Explicit GPU/FFI parameter layout.
    pub fn set_reflection_mask(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        pixels: &[u8],
        center: Vec2,
        size: Vec2,
    ) -> bool {
        if width == 0
            || height == 0
            || width > 1024
            || height > 1024
            || !center.is_finite()
            || !size.is_finite()
            || size.x <= 0.0
            || size.y <= 0.0
            || width
                .checked_mul(height)
                .and_then(|n| usize::try_from(n).ok())
                != Some(pixels.len())
        {
            return false;
        }
        let mut mask = ReflectionMask::new(device, width, height);
        mask.upload(queue, width, height, pixels);
        mask.params = [center.x, center.y, size.x, size.y];
        mask.enabled = true;
        mask.needs_white_upload = false;
        self.reflection_mask = mask;
        self.targets = None;
        self.last_pass_count = 0;
        true
    }
    pub fn configure_volume(&mut self, density: f32, exposure: f32) -> bool {
        if !density.is_finite()
            || !(0.0..=1.0).contains(&density)
            || !exposure.is_finite()
            || !(0.0..=100.0).contains(&exposure)
        {
            return false;
        }
        self.volume_density = density;
        self.volume_exposure = exposure;
        true
    }

    /// Supply a static, linear-sampled cubemap and its box-projection volume.
    /// None (or invalid bounds) restores the original screen-space-only behavior.
    pub fn set_environment(
        &mut self,
        view: Option<wgpu::TextureView>,
        center: Vec3,
        half_extents: Vec3,
    ) {
        let valid =
            center.is_finite() && half_extents.is_finite() && half_extents.cmpgt(Vec3::ZERO).all();
        let view = view.filter(|_| valid);
        let center = if valid { center } else { Vec3::ZERO };
        let half_extents = if valid { half_extents } else { Vec3::ONE };
        if self.environment_view == view
            && self.environment_center == center
            && self.environment_half_extents == half_extents
        {
            return;
        }
        let rebind = self.environment_view != view;
        self.environment_view = view;
        self.environment_center = center;
        self.environment_half_extents = half_extents;
        if rebind {
            self.targets = None;
            self.last_pass_count = 0;
        }
    }

    pub fn set_effect(&mut self, name: &str, enabled: bool, strength: f32) -> bool {
        if !strength.is_finite() || !(0.0..=1.0).contains(&strength) {
            return false;
        }
        let Some(effect) = self
            .effects
            .iter_mut()
            .find(|effect| effect.name == name && name != "environment")
        else {
            return false;
        };
        effect.enabled = enabled;
        effect.strength = strength;
        if !self.enabled() {
            self.targets = None;
            self.last_pass_count = 0;
        }
        true
    }
    pub fn enabled(&self) -> bool {
        self.sky_cube_enabled
            || self
                .effects
                .iter()
                .any(|effect| effect.enabled && effect.strength > 0.0)
    }
    pub fn ensure_size(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if !self.enabled() || width == 0 || height == 0 {
            self.targets = None;
            self.last_pass_count = 0;
            return;
        }
        if self
            .targets
            .as_ref()
            .is_some_and(|targets| targets.scene.width == width && targets.scene.height == height)
        {
            return;
        }
        let scene = ColorTarget::new(device, "postprocess scene", width, height, self.format);
        let depth_color = Intermediate::new(device, width, height, wgpu::TextureFormat::Rgba8Unorm);
        let ping: [Intermediate; 2] =
            std::array::from_fn(|_| Intermediate::new(device, width, height, self.format));
        let sources = [&scene.color, &ping[0].view, &ping[1].view];
        let bind_groups = self.effects.iter().map(|effect| {
            std::array::from_fn(|source_index| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(effect.name),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(sources[source_index]),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&depth_color.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: effect.uniform.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::Sampler(&self.depth_sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: wgpu::BindingResource::TextureView(
                                &self.reflection_mask.view,
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 7,
                            resource: wgpu::BindingResource::TextureView(
                                self.environment_view
                                    .as_ref()
                                    .unwrap_or(&self.environment_fallback),
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 8,
                            resource: wgpu::BindingResource::TextureView(&self.sky_cube_color),
                        },
                        wgpu::BindGroupEntry {
                            binding: 9,
                            resource: wgpu::BindingResource::TextureView(&self.sky_cube_emission),
                        },
                    ],
                })
            })
        });
        let bind_groups = bind_groups.collect();
        self.targets = Some(Targets {
            scene,
            depth_color,
            ping,
            bind_groups,
        });
    }
    /// Call ensure_size before requesting the scene render attachments.
    pub fn target(&self) -> RenderTarget<'_> {
        let targets = self
            .targets
            .as_ref()
            .expect("ensure_size before postprocess target");
        let mut target = targets.scene.target();
        target.auxiliary = Some(&targets.depth_color.view);
        target
    }
    /// Frames count successful queue.submit calls, not GPU completion timestamps.
    pub fn stats(&self) -> [f64; 5] {
        let passes = if self.enabled() {
            self.last_pass_count as f64
        } else {
            0.0
        };
        self.targets.as_ref().map_or(
            [passes, 0.0, 0.0, 0.0, self.submitted_frames as f64],
            |targets| {
                [
                    passes,
                    targets.scene.width as f64,
                    targets.scene.height as f64,
                    4.0,
                    self.submitted_frames as f64,
                ]
            },
        )
    }
    #[allow(clippy::too_many_arguments)] // Explicit GPU/FFI parameter layout.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: RenderView,
        light_position: Vec3,
        light_color: Vec3,
        output: &wgpu::TextureView,
        shadows_enabled: bool,
        xr_rendering: bool,
    ) -> usize {
        // new() has no queue; initialize its white fallback once, before sampling.
        if self.reflection_mask.needs_white_upload {
            self.reflection_mask.upload(queue, 1, 1, &[255]);
            self.reflection_mask.needs_white_upload = false;
        }
        let Some(targets) = &self.targets else {
            return 0;
        };
        let active: Vec<_> = self
            .effects
            .iter()
            .enumerate()
            .filter(|(_, effect)| {
                (effect.name == "environment" && self.sky_cube_enabled)
                    || (effect.enabled && effect.strength > 0.0)
            })
            .collect();
        if active.is_empty() {
            return 0;
        }
        let shadow_matrix = crate::render_math::shadow_view_projection(light_position);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("postprocess chain"),
        });
        for (index, (effect_index, effect)) in active.iter().enumerate() {
            let uniforms = Uniforms {
                inverse_view_projection: view.view_projection.inverse().to_cols_array_2d(),
                view_projection: view.view_projection.to_cols_array_2d(),
                camera: view
                    .camera_position
                    .extend(if self.reflection_mask.enabled {
                        1.0
                    } else {
                        0.0
                    })
                    .to_array(),
                light_position: light_position.extend(1.0).to_array(),
                light_color: light_color.extend(1.0).to_array(),
                params: [
                    effect.strength,
                    if index + 1 == active.len() { 1.0 } else { 0.0 },
                    targets.scene.width as f32,
                    targets.scene.height as f32,
                ],
                mask_params: self.reflection_mask.params,
                shadow_view_projection: shadow_matrix.to_cols_array_2d(),
                volume_params: [
                    if shadows_enabled { 1.0 } else { 0.0 },
                    self.volume_density,
                    24.0,
                    self.volume_exposure,
                ],
                environment_center: self
                    .environment_center
                    .extend(if self.environment_view.is_some() {
                        1.0
                    } else {
                        0.0
                    })
                    .to_array(),
                environment_half_extents: self.environment_half_extents.extend(0.0).to_array(),
                sky_cube_params: [
                    if self.sky_cube_enabled { 1.0 } else { 0.0 },
                    self.sky_cube_intensity,
                    0.0,
                    0.0,
                ],
                sky_cube_center: self
                    .sky_cube_projection
                    .map_or([0.0; 4], |(center, _)| center.extend(1.0).to_array()),
                sky_cube_half_extents: self
                    .sky_cube_projection
                    .map_or([1.0; 4], |(_, extents)| extents.extend(0.0).to_array()),
                reflection_trace: if xr_rendering {
                    [16.0, 3.0, 48.0 / 225.0, 0.0]
                } else {
                    [64.0, 5.0, 0.0, 0.0]
                },
            };
            queue.write_buffer(&effect.uniform, 0, as_bytes(&[uniforms]));
            let destination = if index + 1 == active.len() {
                output
            } else {
                &targets.ping[index % 2].view
            };
            let source_index = if index == 0 { 0 } else { 1 + (index - 1) % 2 };
            let bind_group = &targets.bind_groups[*effect_index][source_index];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(effect.name),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: destination,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&effect.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.set_bind_group(1, &self.shadow_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit(Some(encoder.finish()));
        self.last_pass_count = active.len();
        self.submitted_frames = self.submitted_frames.saturating_add(1);
        active.len()
    }
}
