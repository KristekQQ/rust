#![cfg(target_arch = "wasm32")]

use crate::render::data::Vertex;
use wgpu::{BindGroupLayout, Device, RenderPipeline, TextureFormat};

#[allow(clippy::too_many_arguments)] // Explicit render target and surface state.
pub fn build(
    device: &Device,
    format: TextureFormat,
    depth_format: TextureFormat,
    layout: &BindGroupLayout,
    effects: &BindGroupLayout,
    reflected: bool,
    material_mask: bool,
    packed_depth: bool,
    double_sided: bool,
) -> RenderPipeline {
    build_surface(
        device,
        format,
        depth_format,
        layout,
        effects,
        reflected,
        material_mask,
        packed_depth,
        double_sided,
        None,
    )
}

pub fn build_translucent(
    device: &Device,
    format: TextureFormat,
    depth_format: TextureFormat,
    layout: &BindGroupLayout,
    effects: &BindGroupLayout,
    cull: wgpu::Face,
) -> RenderPipeline {
    build_surface(
        device,
        format,
        depth_format,
        layout,
        effects,
        false,
        false,
        false,
        true,
        Some(cull),
    )
}

#[allow(clippy::too_many_arguments)]
fn build_surface(
    device: &Device,
    format: TextureFormat,
    depth_format: TextureFormat,
    layout: &BindGroupLayout,
    effects: &BindGroupLayout,
    reflected: bool,
    material_mask: bool,
    packed_depth: bool,
    double_sided: bool,
    translucent_cull: Option<wgpu::Face>,
) -> RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("scene shader"),
        source: wgpu::ShaderSource::Wgsl(
            concat!(
                include_str!("../shader.wgsl"),
                "\n",
                include_str!("appearance.wgsl")
            )
            .into(),
        ),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pipeline layout"),
        bind_group_layouts: &[Some(layout), Some(effects)],
        immediate_size: 0,
    });
    let mut targets = vec![Some(wgpu::ColorTargetState {
        format,
        blend: Some(if translucent_cull.is_some() {
            wgpu::BlendState::ALPHA_BLENDING
        } else {
            wgpu::BlendState::REPLACE
        }),
        write_mask: wgpu::ColorWrites::ALL,
    })];
    if packed_depth {
        targets.push(Some(wgpu::ColorTargetState {
            format: TextureFormat::Rgba8Unorm,
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        }));
    }
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[
                Some(Vertex::layout()),
                Some(crate::visibility::InstanceData::layout()),
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(if translucent_cull.is_some() {
                "fs_translucent"
            } else if packed_depth {
                "fs_postprocess"
            } else if material_mask {
                "fs_material_mask"
            } else {
                "fs_main"
            }),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &targets,
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            // The left-handed camera projection reverses outward CCW meshes
            // in screen space. Cull CCW here and keep their visible CW faces.
            cull_mode: if let Some(cull) = translucent_cull {
                Some(cull)
            } else if double_sided {
                None
            } else {
                Some(if reflected {
                    wgpu::Face::Back
                } else {
                    wgpu::Face::Front
                })
            },
            front_face: wgpu::FrontFace::Ccw,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: Some(translucent_cull.is_none()),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

pub fn build_lines(
    device: &Device,
    format: TextureFormat,
    depth_format: TextureFormat,
    layout: &BindGroupLayout,
) -> RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("scene shader"),
        source: wgpu::ShaderSource::Wgsl(
            concat!(
                include_str!("../shader.wgsl"),
                "\n",
                include_str!("appearance.wgsl")
            )
            .into(),
        ),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("grid pipeline layout"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("grid pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_lines"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(Vertex::layout())],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_color"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            cull_mode: None,
            front_face: wgpu::FrontFace::Ccw,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

pub fn build_shadow(
    device: &Device,
    layout: &BindGroupLayout,
    format: TextureFormat,
) -> RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shadow.wgsl"));
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("shadow layout"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("depth-only shadows"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[
                Some(Vertex::layout()),
                Some(crate::visibility::InstanceData::layout()),
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[],
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            },
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}
