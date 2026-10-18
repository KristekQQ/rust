//! Shared, lazily uploaded cut-ring texture and target-specific alpha pipelines.
use super::data::Vertex;
use crate::visibility::InstanceData;
use std::collections::HashMap;

type PipelineKey = (wgpu::TextureFormat, wgpu::TextureFormat, bool);

pub(super) struct CutFeedbackGpu {
    texture: wgpu::Texture,
    layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
}

impl CutFeedbackGpu {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let size = crate::scene::cut_feedback::TEXTURE_SIZE;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("cut feedback RGBA texture"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &crate::scene::cut_feedback::ring_texture(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cut feedback texture layout"),
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
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("cut feedback linear sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let view = texture.create_view(&Default::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cut feedback sampled texture"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self {
            texture,
            layout,
            bind_group,
            pipelines: HashMap::new(),
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        scene_layout: &wgpu::BindGroupLayout,
        key: PipelineKey,
    ) {
        self.pipelines.entry(key).or_insert_with(|| {
            let shader = device.create_shader_module(wgpu::include_wgsl!("cut_feedback.wgsl"));
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("cut feedback alpha layout"),
                bind_group_layouts: &[Some(scene_layout), Some(&self.layout)],
                immediate_size: 0,
            });
            let mut targets = vec![Some(wgpu::ColorTargetState {
                format: key.0,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })];
            if key.2 {
                targets.push(Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                }));
            }
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("cut feedback alpha pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(Vertex::layout()), Some(InstanceData::layout())],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(if key.2 { "fs_mask" } else { "fs_main" }),
                    compilation_options: Default::default(),
                    targets: &targets,
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: key.1,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        });
    }

    pub fn pipeline(&self, key: PipelineKey) -> &wgpu::RenderPipeline {
        &self.pipelines[&key]
    }
}

impl Drop for CutFeedbackGpu {
    fn drop(&mut self) {
        self.texture.destroy();
    }
}
