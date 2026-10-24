//! Depth-aware fragment postprocessing, independent of a browser or surface.
use super::graph_resources::GraphResources;
use super::{
    data::as_bytes,
    target::{RenderTarget, RenderView},
    textures::ColorTarget,
};
use crate::frame_graph::ResourceId;
use crate::postprocess_plan::{EffectSpec, PostprocessPlan};
use glam::{Vec2, Vec3, Vec4};

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
    receiver_bounds_min: [f32; 4],
    receiver_bounds_max: [f32; 4],
}
// Rust offsets match WGSL matrices and vec4 fields (416 bytes total).
const _: () = assert!(std::mem::size_of::<Uniforms>() == 416);
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
    // Filter the completed opaque color chain before transparent/UI overlays.
    EffectDescriptor {
        name: "fxaa",
        entry_point: "fs_fxaa",
        shader_source: include_str!("postprocess.wgsl"),
    },
];
struct Effect {
    name: &'static str,
    enabled: bool,
    strength: f32,
    pipelines: Vec<(
        wgpu::TextureFormat,
        Option<wgpu::TextureFormat>,
        wgpu::RenderPipeline,
    )>,
    uniform: super::view_uniforms::ViewUniforms,
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
    depth_color: Option<Intermediate>,
    key: (u64, bool),
    plan: PostprocessPlan,
    resources: GraphResources,
    bind_groups: Vec<wgpu::BindGroup>,
}
struct SsrTarget {
    resource: ResourceId,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}
/// Frame-local final fullscreen draw. It retains no browser attachments and
/// must execute before the next view rewrites any shared resources.
pub struct FinalPostprocessDraw<'a> {
    pub name: &'static str,
    pipeline: &'a wgpu::RenderPipeline,
    depth_pipeline: &'a wgpu::RenderPipeline,
    source: &'a wgpu::BindGroup,
    shadows: &'a wgpu::BindGroup,
    trace: Option<&'a wgpu::BindGroup>,
    uniform_offset: u32,
}
impl FinalPostprocessDraw<'_> {
    pub fn encode(&self, pass: &mut wgpu::RenderPass<'_>, with_depth: bool) {
        pass.set_pipeline(if with_depth {
            self.depth_pipeline
        } else {
            self.pipeline
        });
        pass.set_bind_group(0, self.source, &[self.uniform_offset]);
        pass.set_bind_group(1, self.shadows, &[]);
        if let Some(trace) = self.trace {
            pass.set_bind_group(2, trace, &[]);
        }
        pass.draw(0..3, 0..1);
    }
}

pub struct PostProcessor {
    ssr_resolution_scale: f32,
    ssr_region: Option<(Vec3, Vec3)>,
    ssr_work: [f64; 2],
    ssr_target: Option<SsrTarget>,
    ssr_layout: wgpu::BindGroupLayout,
    ssr_trace_pipeline: wgpu::RenderPipeline,
    ssr_composite_pipelines: Vec<(
        wgpu::TextureFormat,
        Option<wgpu::TextureFormat>,
        wgpu::RenderPipeline,
    )>,
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
    fn pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        layout: &wgpu::BindGroupLayout,
        shadow_layout: &wgpu::BindGroupLayout,
        descriptor: &EffectDescriptor,
        ssr_layout: Option<&wgpu::BindGroupLayout>,
        depth_format: Option<wgpu::TextureFormat>,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(descriptor.name),
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    "{}\n{}",
                    descriptor.shader_source,
                    if device.as_webgpu().is_some() {
                        include_str!("postprocess_depth_native.wgsl")
                    } else {
                        include_str!("postprocess_depth_packed.wgsl")
                    }
                )
                .into(),
            ),
        });
        let mut layouts = vec![Some(layout), Some(shadow_layout)];
        if let Some(layout) = ssr_layout {
            layouts.push(Some(layout));
        }
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("postprocess pipeline layout"),
            bind_group_layouts: &layouts,
            immediate_size: 0,
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
                format,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    }

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
                        sample_type: if device.as_webgpu().is_some() {
                            wgpu::TextureSampleType::Depth
                        } else {
                            wgpu::TextureSampleType::Float { filterable: true }
                        },
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
                        has_dynamic_offset: true,
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
        let ssr_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSR trace composite layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let ssr_trace_pipeline = Self::pipeline(
            device,
            format,
            &layout,
            &shadow_layout,
            &EffectDescriptor {
                name: "SSR half-resolution trace",
                entry_point: "fs_ssr_trace",
                shader_source: include_str!("postprocess.wgsl"),
            },
            None,
            None,
        );
        let ssr_composite_pipeline = Self::pipeline(
            device,
            format,
            &layout,
            &shadow_layout,
            &EffectDescriptor {
                name: "SSR depth-aware composite",
                entry_point: "fs_ssr_composite",
                shader_source: include_str!("postprocess.wgsl"),
            },
            Some(&ssr_layout),
            None,
        );
        let effects = EFFECTS
            .iter()
            .map(|descriptor| {
                let pipeline = Self::pipeline(
                    device,
                    format,
                    &layout,
                    &shadow_layout,
                    descriptor,
                    None,
                    None,
                );
                Effect {
                    name: descriptor.name,
                    pipelines: vec![(format, None, pipeline)],
                    enabled: false,
                    strength: 0.5,
                    uniform: super::view_uniforms::ViewUniforms::new(
                        device,
                        descriptor.name,
                        std::mem::size_of::<Uniforms>() as u64,
                        3,
                    ),
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
            ssr_resolution_scale: 1.0,
            ssr_region: None,
            ssr_work: [0.0; 2],
            ssr_target: None,
            ssr_layout,
            ssr_trace_pipeline,
            ssr_composite_pipelines: vec![(format, None, ssr_composite_pipeline)],
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
        let was_active = effect.enabled && effect.strength > 0.0;
        effect.enabled = enabled;
        effect.strength = strength;
        if was_active != (enabled && strength > 0.0) {
            self.targets = None;
            self.ssr_target = None;
            self.last_pass_count = 0;
        }
        if !self.ssr_enabled() {
            self.ssr_target = None;
        }
        if !self.enabled() {
            self.targets = None;
            self.last_pass_count = 0;
        }
        true
    }
    /// Keep 1.0 as the full-resolution quality reference; 0.5 traces a quarter of the pixels.
    pub fn configure_ssr(&mut self, resolution_scale: f32) -> bool {
        if !resolution_scale.is_finite() || (resolution_scale != 0.5 && resolution_scale != 1.0) {
            return false;
        }
        if self.ssr_resolution_scale != resolution_scale {
            self.ssr_resolution_scale = resolution_scale;
            self.ssr_target = None;
            self.targets = None;
            self.last_pass_count = 0;
        }
        true
    }
    /// Receiver bounds are an explicit promise that reflection receivers lie inside this box.
    pub fn configure_ssr_region(&mut self, min: Vec3, max: Vec3, enabled: bool) -> bool {
        if !enabled {
            self.ssr_region = None;
            return true;
        }
        if !min.is_finite()
            || !max.is_finite()
            || !min.cmple(max).all()
            || min.abs().max_element() > 1e6
            || max.abs().max_element() > 1e6
        {
            return false;
        }
        self.ssr_region = Some((min, max));
        true
    }
    /// Last submitted eye: scissored trace pixels and unscissored reference pixels.
    pub fn ssr_work_stats(&self) -> [f64; 2] {
        if self.ssr_enabled() {
            self.ssr_work
        } else {
            [0.0; 2]
        }
    }
    /// A bounded receiver outside this eye needs no offscreen scene or composite.
    /// Other enabled effects still require their normal postprocess chain.
    pub fn needs_render(&self, view: RenderView, width: u32, height: u32) -> bool {
        if self.sky_cube_enabled
            || self
                .effects
                .iter()
                .any(|effect| effect.name != "ssr" && effect.enabled && effect.strength > 0.0)
        {
            return true;
        }
        if !self.ssr_enabled() {
            return false;
        }
        let rect = self.ssr_rect(view, width, height);
        rect[2] > 0 && rect[3] > 0
    }
    pub fn skip_render(&mut self) {
        self.last_pass_count = 0;
        self.ssr_work = [0.0; 2];
    }
    fn ssr_rect(&self, view: RenderView, width: u32, height: u32) -> [u32; 4] {
        let full = [0, 0, width, height];
        let Some((min, max)) = self.ssr_region else {
            return full;
        };
        // A camera inside/on a receiver is a deliberately conservative fallback.
        if view.camera_position.cmpge(min).all() && view.camera_position.cmple(max).all() {
            return full;
        }
        let vertices: [Vec4; 8] = std::array::from_fn(|i| {
            view.view_projection
                * Vec3::new(
                    if i & 1 == 0 { min.x } else { max.x },
                    if i & 2 == 0 { min.y } else { max.y },
                    if i & 4 == 0 { min.z } else { max.z },
                )
                .extend(1.0)
        });
        if vertices.iter().any(|v| !v.is_finite()) {
            return full;
        }
        let mut lo = Vec2::splat(f32::INFINITY);
        let mut hi = Vec2::splat(f32::NEG_INFINITY);
        // Clip all six faces, including the new vertices where near/w planes
        // intersect a face. Projecting only the original corners is unsafe near the eye.
        for face in [
            [0, 2, 6, 4],
            [1, 3, 7, 5],
            [0, 1, 5, 4],
            [2, 3, 7, 6],
            [0, 1, 3, 2],
            [4, 5, 7, 6],
        ] {
            let mut polygon: Vec<Vec4> = face.iter().map(|&i| vertices[i]).collect();
            for plane in 0..2 {
                let input = std::mem::take(&mut polygon);
                if input.is_empty() {
                    break;
                }
                let distance = |v: Vec4| if plane == 0 { v.z } else { v.w - 0.000001 };
                let mut previous = *input.last().unwrap();
                let mut previous_distance = distance(previous);
                for current in input {
                    let current_distance = distance(current);
                    if (previous_distance >= 0.0) != (current_distance >= 0.0) {
                        polygon.push(previous.lerp(
                            current,
                            previous_distance / (previous_distance - current_distance),
                        ));
                    }
                    if current_distance >= 0.0 {
                        polygon.push(current);
                    }
                    previous = current;
                    previous_distance = current_distance;
                }
            }
            for point in polygon {
                let uv = Vec2::new(point.x / point.w * 0.5 + 0.5, 0.5 - point.y / point.w * 0.5);
                if !uv.is_finite() {
                    return full;
                }
                lo = lo.min(uv);
                hi = hi.max(uv);
            }
        }
        if lo.x > hi.x || lo.y > hi.y {
            return [0; 4];
        }
        // Guard the half-resolution reconstruction taps and raster edge rounding.
        let size = Vec2::new(width as f32, height as f32);
        let lo = (lo * size - Vec2::splat(2.0))
            .floor()
            .clamp(Vec2::ZERO, size);
        let hi = (hi * size + Vec2::splat(2.0))
            .ceil()
            .clamp(Vec2::ZERO, size);
        [
            lo.x as u32,
            lo.y as u32,
            (hi.x - lo.x) as u32,
            (hi.y - lo.y) as u32,
        ]
    }
    fn ssr_enabled(&self) -> bool {
        self.effects
            .iter()
            .any(|effect| effect.name == "ssr" && effect.enabled && effect.strength > 0.0)
    }
    pub fn ssr_stats(&self) -> [f64; 3] {
        if !self.ssr_enabled() {
            return [0.0; 3];
        }
        if self.ssr_resolution_scale == 0.5 {
            return self.ssr_target.as_ref().map_or([0.0; 3], |target| {
                [
                    target.width as f64,
                    target.height as f64,
                    target.width as f64 * target.height as f64,
                ]
            });
        }
        self.targets.as_ref().map_or([0.0; 3], |targets| {
            [
                targets.scene.width as f64,
                targets.scene.height as f64,
                targets.scene.width as f64 * targets.scene.height as f64,
            ]
        })
    }
    pub fn enabled(&self) -> bool {
        self.sky_cube_enabled
            || self
                .effects
                .iter()
                .any(|effect| effect.enabled && effect.strength > 0.0)
    }
    pub fn ensure_size(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        if !self.enabled() || width == 0 || height == 0 {
            self.targets = None;
            self.ssr_target = None;
            self.last_pass_count = 0;
            return Ok(());
        }
        let mask = self
            .effects
            .iter()
            .enumerate()
            .fold(0u64, |mask, (index, effect)| {
                if (effect.name == "environment" && self.sky_cube_enabled)
                    || (effect.enabled && effect.strength > 0.0)
                {
                    mask | (1 << index)
                } else {
                    mask
                }
            });
        let key = (mask, self.ssr_resolution_scale == 0.5);
        if self.targets.as_ref().is_some_and(|targets| {
            targets.scene.width == width && targets.scene.height == height && targets.key == key
        }) {
            return Ok(());
        }
        let _scope = super::profiler::cpu_scope("graph.compile.allocate");
        let specs: Vec<_> = self
            .effects
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(index, effect)| EffectSpec {
                index,
                name: effect.name,
                reads_depth: !matches!(effect.name, "bloom" | "fxaa"),
                half_trace: effect.name == "ssr" && key.1,
            })
            .collect();
        let plan = PostprocessPlan::new(
            &specs,
            width,
            height,
            self.format,
            std::mem::size_of::<Uniforms>() as u64,
        )?;
        let resources = GraphResources::new(device, &plan.compiled)?;
        let scene = ColorTarget::new(device, "postprocess scene", width, height, self.format);
        let depth_color = device
            .as_webgpu()
            .is_none()
            .then(|| Intermediate::new(device, width, height, wgpu::TextureFormat::Rgba8Unorm));
        let mut bind_groups = Vec::with_capacity(plan.steps.len());
        let mut ssr_target = None;
        for step in &plan.steps {
            let effect = &self.effects[step.effect];
            let source = if step.source == plan.scene_color {
                &scene.color
            } else {
                resources.view(step.source)?
            };
            bind_groups.push(
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(effect.name),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(source),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(
                                if device.as_webgpu().is_some() {
                                    &scene.depth
                                } else {
                                    &depth_color.as_ref().expect("WebGL packed depth").view
                                },
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: effect.uniform.binding(),
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
                }),
            );
            if let Some(trace) = step.trace {
                ssr_target = Some(SsrTarget {
                    resource: trace,
                    width: width.div_ceil(2),
                    height: height.div_ceil(2),
                    bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("SSR trace composite"),
                        layout: &self.ssr_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(resources.view(trace)?),
                        }],
                    }),
                });
            }
        }
        self.ssr_target = ssr_target;
        self.targets = Some(Targets {
            scene,
            depth_color,
            key,
            plan,
            resources,
            bind_groups,
        });
        Ok(())
    }
    pub fn graph_stats(&self) -> serde_json::Value {
        self.targets.as_ref().map_or_else(|| serde_json::json!({"active":false,"passes":[],"transientTextures":0,"transientBuffers":0}), |targets| {
            let mut summary = targets.plan.graph.debug_summary(&targets.plan.compiled);
            summary["active"] = true.into();
            summary["transientTextures"] = targets.resources.texture_count().into();
            summary["transientBuffers"] = targets.resources.buffer_count().into();
            summary["transientBytesEstimate"] = targets.resources.allocated_bytes_estimate().into();
            summary
        })
    }
    /// Call ensure_size before requesting the scene render attachments.
    pub fn target(&self) -> RenderTarget<'_> {
        let targets = self
            .targets
            .as_ref()
            .expect("ensure_size before postprocess target");
        let mut target = targets.scene.target();
        target.material_mask = true;
        target.auxiliary = targets.depth_color.as_ref().map(|depth| &depth.view);
        target
    }
    /// Called after the owning view's command batch is actually submitted.
    pub fn record_submission(&mut self) {
        self.submitted_frames = self.submitted_frames.saturating_add(1);
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
                    1.0 + f64::from(targets.depth_color.is_some())
                        + targets.resources.texture_count() as f64,
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
        output: &RenderTarget<'_>,
        shadows_enabled: bool,
        xr_rendering: bool,
        view_slot: u32,
    ) -> (
        usize,
        Option<wgpu::CommandBuffer>,
        Option<FinalPostprocessDraw<'_>>,
    ) {
        // new() has no queue; initialize its white fallback once, before sampling.
        if self.reflection_mask.needs_white_upload {
            self.reflection_mask.upload(queue, 1, 1, &[255]);
            self.reflection_mask.needs_white_upload = false;
        }
        // Cache the final attachment's pipeline separately from sRGB intermediates.
        // XR compositor slices use the corresponding non-sRGB texture format.
        for (effect, descriptor) in self.effects.iter_mut().zip(EFFECTS) {
            // A new XR output format must not compile disabled effect shaders.
            if !((effect.name == "environment" && self.sky_cube_enabled)
                || (effect.enabled && effect.strength > 0.0))
            {
                continue;
            }
            if effect.name == "ssr" && self.ssr_target.is_some() {
                continue;
            }
            if !effect
                .pipelines
                .iter()
                .any(|(format, depth, _)| *format == output.color_format && depth.is_none())
            {
                effect.pipelines.push((
                    output.color_format,
                    None,
                    Self::pipeline(
                        device,
                        output.color_format,
                        &self.layout,
                        &self.shadow_layout,
                        descriptor,
                        None,
                        None,
                    ),
                ));
            }
        }
        if self.ssr_enabled()
            && self.ssr_resolution_scale == 0.5
            && !self
                .ssr_composite_pipelines
                .iter()
                .any(|(format, depth, _)| *format == output.color_format && depth.is_none())
        {
            self.ssr_composite_pipelines.push((
                output.color_format,
                None,
                Self::pipeline(
                    device,
                    output.color_format,
                    &self.layout,
                    &self.shadow_layout,
                    &EffectDescriptor {
                        name: "SSR depth-aware composite",
                        entry_point: "fs_ssr_composite",
                        shader_source: include_str!("postprocess.wgsl"),
                    },
                    Some(&self.ssr_layout),
                    None,
                ),
            ));
        }
        // The final draw can share the overlay pass. Cache both variants so an
        // empty overlay list does not force an unnecessary depth attachment.
        if let Some(step) = self.targets.as_ref().and_then(|t| t.plan.steps.last()) {
            let descriptor = if step.trace.is_some() {
                &EffectDescriptor {
                    name: "SSR depth-aware composite",
                    entry_point: "fs_ssr_composite",
                    shader_source: include_str!("postprocess.wgsl"),
                }
            } else {
                &EFFECTS[step.effect]
            };
            let pipelines = if step.trace.is_some() {
                &mut self.ssr_composite_pipelines
            } else {
                &mut self.effects[step.effect].pipelines
            };
            if !pipelines.iter().any(|(format, depth, _)| {
                *format == output.color_format && *depth == Some(output.depth_format)
            }) {
                pipelines.push((
                    output.color_format,
                    Some(output.depth_format),
                    Self::pipeline(
                        device,
                        output.color_format,
                        &self.layout,
                        &self.shadow_layout,
                        descriptor,
                        step.trace.map(|_| &self.ssr_layout),
                        Some(output.depth_format),
                    ),
                ));
            }
        }
        self.ssr_work = [0.0; 2];
        let Some(targets) = &self.targets else {
            return (0, None, None);
        };
        let active_count = targets.plan.steps.len();
        if active_count == 0 {
            return (0, None, None);
        }
        let _scope = super::profiler::cpu_scope("postprocess.encode");
        let shadow_matrix = crate::render_math::shadow_view_projection(light_position);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("postprocess chain"),
        });
        let mut final_draw = None;
        let mut encoded_passes = 0;
        for (index, step) in targets.plan.steps.iter().enumerate() {
            let effect = &self.effects[step.effect];
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
                    if index + 1 == active_count { 1.0 } else { 0.0 },
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
                receiver_bounds_min: self
                    .ssr_region
                    .map_or([0.0; 4], |(min, _)| min.extend(1.0).to_array()),
                receiver_bounds_max: self
                    .ssr_region
                    .map_or([0.0; 4], |(_, max)| max.extend(0.0).to_array()),
                reflection_trace: if xr_rendering {
                    [16.0, 3.0, 48.0 / 225.0, 0.0]
                } else {
                    [64.0, 5.0, 0.0, 0.0]
                },
            };
            let uniform_offset = effect
                .uniform
                .write(queue, view_slot, as_bytes(&[uniforms]));
            let destination = if step.destination == targets.plan.output {
                output.color
            } else {
                targets
                    .resources
                    .view(step.destination)
                    .expect("compiled color resource")
            };
            let bind_group = &targets.bind_groups[index];
            let half_ssr = step.trace.is_some();
            if effect.name == "ssr" {
                let pixels = targets.scene.width as f64 * targets.scene.height as f64;
                self.ssr_work = [pixels, pixels];
            }
            if half_ssr {
                let trace = self.ssr_target.as_ref().unwrap();
                let rect = self.ssr_rect(view, trace.width, trace.height);
                self.ssr_work = [
                    rect[2] as f64 * rect[3] as f64,
                    trace.width as f64 * trace.height as f64,
                ];
                encoded_passes += 1;
                let stamp = super::profiler::stamp("ssr.trace");
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("SSR half-resolution trace"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: targets
                            .resources
                            .view(trace.resource)
                            .expect("compiled SSR trace"),
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: stamp.as_ref().map(|stamp| stamp.writes()),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                if rect[2] > 0 && rect[3] > 0 {
                    pass.set_pipeline(&self.ssr_trace_pipeline);
                    pass.set_bind_group(0, bind_group, &[uniform_offset]);
                    pass.set_bind_group(1, &self.shadow_bind_group, &[]);
                    pass.set_scissor_rect(rect[0], rect[1], rect[2], rect[3]);
                    pass.draw(0..3, 0..1);
                }
            }
            let format = if index + 1 == active_count {
                output.color_format
            } else {
                self.format
            };
            let pipelines = if half_ssr {
                &self.ssr_composite_pipelines
            } else {
                &effect.pipelines
            };
            let pipeline = &pipelines
                .iter()
                .find(|(candidate, depth, _)| *candidate == format && depth.is_none())
                .expect("cached postprocess output format")
                .2;
            if index + 1 == active_count {
                let depth_pipeline = &pipelines
                    .iter()
                    .find(|(candidate, depth, _)| {
                        *candidate == format && *depth == Some(output.depth_format)
                    })
                    .expect("cached final postprocess depth-compatible pipeline")
                    .2;
                final_draw = Some(FinalPostprocessDraw {
                    name: effect.name,
                    pipeline,
                    depth_pipeline,
                    source: bind_group,
                    shadows: &self.shadow_bind_group,
                    trace: half_ssr.then(|| &self.ssr_target.as_ref().unwrap().bind_group),
                    uniform_offset,
                });
                continue;
            }
            encoded_passes += 1;
            let stamp = super::profiler::stamp(effect.name);
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
                timestamp_writes: stamp.as_ref().map(|stamp| stamp.writes()),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            if half_ssr {
                pass.set_bind_group(2, &self.ssr_target.as_ref().unwrap().bind_group, &[]);
            }
            pass.set_bind_group(0, bind_group, &[uniform_offset]);
            pass.set_bind_group(1, &self.shadow_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.last_pass_count =
            active_count + usize::from(self.ssr_target.is_some() && self.ssr_enabled());
        (
            self.last_pass_count,
            (encoded_passes > 0).then(|| encoder.finish()),
            final_draw,
        )
    }
}
