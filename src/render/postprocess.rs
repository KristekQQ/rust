//! Depth-aware fragment postprocessing, independent of a browser or surface.
use super::{
    data::as_bytes,
    target::{RenderTarget, RenderView},
    textures::ColorTarget,
};
use glam::{Vec2, Vec3};

#[repr(C)]
#[derive(Clone, Copy)]
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
}
// Rust offsets match WGSL matrices and vec4 fields (288 bytes total).
const _: () = assert!(std::mem::size_of::<Uniforms>() == 288);
const _: () = assert!(std::mem::offset_of!(Uniforms, params) == 176);

// Add a WGSL fragment entry point and a descriptor to extend the chain.
// All passes read original scene depth; intermediate alpha is material metadata.
struct EffectDescriptor {
    name: &'static str,
    entry_point: &'static str,
    shader_source: &'static str,
}
const EFFECTS: &[EffectDescriptor] = &[
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
        Self {
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
            targets: None,
            submitted_frames: 0,
            last_pass_count: 0,
            volume_density: 0.035,
            volume_exposure: 2.5,
        }
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

    pub fn set_effect(&mut self, name: &str, enabled: bool, strength: f32) -> bool {
        if !strength.is_finite() || !(0.0..=1.0).contains(&strength) {
            return false;
        }
        let Some(effect) = self.effects.iter_mut().find(|effect| effect.name == name) else {
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
        self.effects
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
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: RenderView,
        light_position: Vec3,
        light_color: Vec3,
        output: &wgpu::TextureView,
        shadows_enabled: bool,
    ) {
        // new() has no queue; initialize its white fallback once, before sampling.
        if self.reflection_mask.needs_white_upload {
            self.reflection_mask.upload(queue, 1, 1, &[255]);
            self.reflection_mask.needs_white_upload = false;
        }
        let Some(targets) = &self.targets else {
            return;
        };
        let active: Vec<_> = self
            .effects
            .iter()
            .enumerate()
            .filter(|(_, effect)| effect.enabled && effect.strength > 0.0)
            .collect();
        if active.is_empty() {
            return;
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
    }
}
