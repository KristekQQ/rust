//! Shared lower-resolution ordinary-edge halos. Resources/uniforms are per view.
use super::{
    neon_glow::NeonGlow,
    target::{RenderTarget, RenderView, Viewport},
};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

pub(super) struct HalfGlow {
    pub requested: f32,
    pub active: bool,
    pub draws: usize,
    pub size: [u32; 2],
    pub fallback_reason: Option<&'static str>,
    views: HashMap<usize, HalfGlowView>,
}
pub(super) struct HalfGlowView {
    pub glow: NeonGlow,
    color: wgpu::Texture,
    depth: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    pub depth_view: wgpu::TextureView,
    uniform: wgpu::Buffer,
    reduce_layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,
    reduce_group: Option<wgpu::BindGroup>,
    composite_group: Option<wgpu::BindGroup>,
    source_depth: Option<wgpu::TextureView>,
    reduce_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    format: wgpu::TextureFormat,
}
impl Default for HalfGlow {
    fn default() -> Self {
        Self {
            requested: 0.5,
            active: false,
            draws: 0,
            size: [0, 0],
            fallback_reason: Some("no-glow-draws"),
            views: HashMap::new(),
        }
    }
}
impl HalfGlow {
    pub fn reset(&mut self) {
        self.active = false;
        self.draws = 0;
        self.size = [0, 0];
        self.fallback_reason = Some("no-glow-draws");
    }
    pub fn clear(&mut self) {
        self.views.clear();
        self.reset();
    }
    pub fn cached_views(&self) -> usize {
        self.views.len()
    }
    pub fn view(&self, slot: usize) -> &HalfGlowView {
        &self.views[&slot]
    }
    #[allow(clippy::too_many_arguments)] // Explicit GPU, view and effect preparation state.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        slot: usize,
        target: &RenderTarget<'_>,
        view: RenderView,
        strength: f32,
        radius: f32,
    ) -> bool {
        self.reset();
        // Never sample attached GL depth. Check the backend before accessing
        // texture metadata: only our wgpu-owned/wrapped WebGPU/native views
        // participate in this path.
        let backend = device.adapter_info().backend;
        let unsupported = if self.requested != 0.5 {
            Some("reference-requested")
        } else if !matches!(
            backend,
            wgpu::Backend::Vulkan | wgpu::Backend::BrowserWebGpu
        ) {
            Some("backend-no-sampled-depth")
        } else if !target.viewport.width.is_multiple_of(2)
            || !target.viewport.height.is_multiple_of(2)
        {
            Some("odd-viewport")
        } else if target.depth_format != wgpu::TextureFormat::Depth32Float {
            Some("depth-format")
        } else {
            // The patched wgpu TextureView owns its Texture, including imported
            // browser views. This getter does not downcast the backend handle.
            let source = target.depth.texture();
            if source.format() != target.depth_format
                || source.sample_count() != 1
                || source.size().depth_or_array_layers != 1
                || source.width() != target.width
                || source.height() != target.height
                || !source
                    .usage()
                    .contains(wgpu::TextureUsages::TEXTURE_BINDING)
            {
                Some("depth-not-sampleable-2d")
            } else {
                None
            }
        };
        if let Some(reason) = unsupported {
            self.fallback_reason = Some(reason);
            return false;
        }
        let size = [
            target.viewport.width.div_ceil(2),
            target.viewport.height.div_ceil(2),
        ];
        let replace = self.views.get(&slot).is_none_or(|v| {
            v.color.width() != size[0]
                || v.color.height() != size[1]
                || v.format != target.color_format
        });
        if replace {
            self.views.insert(
                slot,
                HalfGlowView::new(device, size, target.color_format, target.depth_format),
            );
        }
        let v = self.views.get_mut(&slot).unwrap();
        queue.write_buffer(
            &v.uniform,
            0,
            bytemuck::cast_slice(&[
                target.viewport.x,
                target.viewport.y,
                target.viewport.width,
                target.viewport.height,
            ]),
        );
        // Identity includes the view (not only its parent texture), so stereo
        // layers and replaced attachments cannot reuse stale source bindings.
        if v.source_depth.as_ref() != Some(target.depth) {
            v.reduce_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("half halo depth source"),
                layout: &v.reduce_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(target.depth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: v.uniform.as_entire_binding(),
                    },
                ],
            }));
            v.composite_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("half halo composite source"),
                layout: &v.composite_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(target.depth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: v.uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&v.color_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&v.depth_view),
                    },
                ],
            }));
            v.source_depth = Some(target.depth.clone());
        }
        v.glow.configure(strength, radius);
        let low = RenderTarget {
            color: &v.color_view,
            depth: &v.depth_view,
            color_format: wgpu::TextureFormat::Rgba16Float,
            depth_format: wgpu::TextureFormat::Depth32Float,
            width: size[0],
            height: size[1],
            viewport: Viewport::full(size[0], size[1]),
            material_mask: false,
            auxiliary: None,
            color_load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            depth_load: wgpu::LoadOp::Load,
            depth_store: wgpu::StoreOp::Store,
        };
        v.glow.prepare_scaled(device, queue, view, &low, 0.5);
        self.active = true;
        self.fallback_reason = None;
        self.size = size;
        true
    }
}
impl HalfGlowView {
    fn new(
        device: &wgpu::Device,
        size: [u32; 2],
        format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let make = |format, label| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        };
        let color = make(wgpu::TextureFormat::Rgba16Float, "half halo color");
        let depth = make(
            wgpu::TextureFormat::Depth32Float,
            "half halo conservative depth",
        );
        let color_view = color.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("half halo viewport"),
            contents: &[0u8; 16],
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let texture = |binding, depth| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: if depth {
                    wgpu::TextureSampleType::Depth
                } else {
                    wgpu::TextureSampleType::Float { filterable: false }
                },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let u = wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let reduce_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("half halo reduce layout"),
            entries: &[texture(0, true), u],
        });
        let composite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("half halo composite layout"),
            entries: &[texture(0, true), u, texture(2, false), texture(3, true)],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("half_glow.wgsl"));
        let build = |layout: &wgpu::BindGroupLayout, reduce: bool| {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("half halo pipeline layout"),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            let target = Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Max,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(if reduce {
                    "half halo min depth"
                } else {
                    "half halo MAX composite"
                }),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(if reduce { "fs_reduce" } else { "fs_composite" }),
                    compilation_options: Default::default(),
                    targets: if reduce {
                        &[]
                    } else {
                        std::slice::from_ref(&target)
                    },
                }),
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: if reduce {
                        wgpu::TextureFormat::Depth32Float
                    } else {
                        depth_format
                    },
                    depth_write_enabled: Some(reduce),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let reduce_pipeline = build(&reduce_layout, true);
        let composite_pipeline = build(&composite_layout, false);
        Self {
            glow: NeonGlow::new(device),
            color,
            depth,
            color_view,
            depth_view,
            uniform,
            reduce_layout,
            composite_layout,
            reduce_group: None,
            composite_group: None,
            source_depth: None,
            reduce_pipeline,
            composite_pipeline,
            format,
        }
    }
    pub fn reduce(&self, encoder: &mut wgpu::CommandEncoder) {
        let stamp = super::profiler::stamp("neon.half.depth");
        let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("half halo min depth"),
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: stamp.as_ref().map(|stamp| stamp.writes()),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        p.set_pipeline(&self.reduce_pipeline);
        p.set_bind_group(0, self.reduce_group.as_ref().unwrap(), &[]);
        p.draw(0..3, 0..1);
    }
    pub fn composite<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.composite_pipeline);
        pass.set_bind_group(0, self.composite_group.as_ref().unwrap(), &[]);
        pass.draw(0..3, 0..1);
    }
}
impl Drop for HalfGlowView {
    fn drop(&mut self) {
        self.color.destroy();
        self.depth.destroy();
        self.uniform.destroy();
    }
}
