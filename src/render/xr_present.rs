//! Stage both eyes in owned textures before touching compositor attachments.
//! Browser views are used only by `present`, in one final queue submission.
use super::{target::RenderTarget, textures::ColorTarget};

struct EyeStage {
    target: ColorTarget,
    source: wgpu::BindGroup,
    ready: bool,
    write_depth: bool,
}

pub struct XrPresent {
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
    color_pipeline: wgpu::RenderPipeline,
    depth_pipeline: wgpu::RenderPipeline,
    eyes: [Option<EyeStage>; 2],
}

impl XrPresent {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("XR stereo presentation sources"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
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
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("XR stereo presentation shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("xr_present.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("XR stereo presentation layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |write_depth: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("XR stereo presentation"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(if write_depth { "fs_depth" } else { "fs_color" }),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: write_depth.then_some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(write_depth),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            format,
            color_pipeline: pipeline(false),
            depth_pipeline: pipeline(true),
            layout,
            eyes: [None, None],
        }
    }

    pub fn ensure_eye(
        &mut self,
        device: &wgpu::Device,
        index: usize,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let eye = self
            .eyes
            .get_mut(index)
            .ok_or("XR presentation eye index must be 0 or 1")?;
        if width == 0
            || height == 0
            || width > device.limits().max_texture_dimension_2d
            || height > device.limits().max_texture_dimension_2d
        {
            return Err("Invalid XR presentation stage dimensions".into());
        }
        if eye.as_ref().is_some_and(|eye| eye.ready) {
            return Err("XR presentation eye is already staged for this frame".into());
        }
        if eye
            .as_ref()
            .is_none_or(|eye| (eye.target.width, eye.target.height) != (width, height))
        {
            // Match canvas/PBR linear blending and hardware sRGB encoding.
            // The second UNORM view reads the encoded bytes without decoding them
            // again before the XR compositor consumes its UNORM texture.
            let target = ColorTarget::new_with_view_formats(
                device,
                "XR eye presentation stage",
                width,
                height,
                self.format.add_srgb_suffix(),
                &[self.format],
            );
            let encoded_color = target.texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("XR encoded color source"),
                format: Some(self.format),
                ..Default::default()
            });
            let source = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("XR eye presentation source"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&encoded_color),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&target.depth),
                    },
                ],
            });
            *eye = Some(EyeStage {
                target,
                source,
                ready: false,
                write_depth: false,
            });
        }
        Ok(())
    }

    pub fn target(&self, index: usize) -> Result<RenderTarget<'_>, String> {
        self.eyes
            .get(index)
            .and_then(Option::as_ref)
            .ok_or_else(|| "XR presentation eye stage is not initialized".into())
            .map(|eye| eye.target.target())
    }

    /// Called only after the complete eye render, including postprocessing.
    pub fn mark_ready(&mut self, index: usize, write_depth: bool) -> Result<(), String> {
        let eye = self
            .eyes
            .get_mut(index)
            .and_then(Option::as_mut)
            .ok_or("XR presentation eye stage is not initialized")?;
        if eye.ready {
            return Err("XR presentation eye is already staged for this frame".into());
        }
        eye.ready = true;
        eye.write_depth = write_depth;
        Ok(())
    }

    pub fn reset_frame(&mut self) {
        for eye in self.eyes.iter_mut().flatten() {
            eye.ready = false;
        }
    }

    pub fn is_ready(&self, index: usize) -> bool {
        self.eyes
            .get(index)
            .and_then(Option::as_ref)
            .is_some_and(|eye| eye.ready)
    }

    /// Validate the complete batch before recording any browser attachment use.
    /// Each supplied eye writes its color/depth slice once; no COPY_DST is needed.
    pub fn present(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        targets: &[(usize, RenderTarget<'_>)],
    ) -> Result<usize, String> {
        if targets.is_empty() || targets.len() > self.eyes.len() {
            return Err("XR presentation requires one or two staged eyes".into());
        }
        let mut seen = [false; 2];
        for (index, target) in targets {
            target.validate()?;
            let eye = self
                .eyes
                .get(*index)
                .and_then(Option::as_ref)
                .ok_or("XR presentation eye stage is not initialized")?;
            if !eye.ready || seen[*index] {
                return Err("XR presentation requires unique ready eyes".into());
            }
            seen[*index] = true;
            if target.color_format != self.format
                || target.depth_format != wgpu::TextureFormat::Depth32Float
                || (target.width, target.height) != (eye.target.width, eye.target.height)
                || target.viewport.x != 0
                || target.viewport.y != 0
                || (target.viewport.width, target.viewport.height) != (target.width, target.height)
                || target.auxiliary.is_some()
                || target.material_mask
            {
                return Err(
                    "XR presentation requires matching full-size color/depth slices".into(),
                );
            }
            // No destination may alias either owned source or another destination.
            if self.eyes.iter().flatten().any(|source| {
                target.color == &source.target.color || target.depth == &source.target.depth
            }) || targets.iter().any(|(other_index, other)| {
                other_index != index && (target.color == other.color || target.depth == other.depth)
            }) {
                return Err(
                    "XR presentation source and destination slices must be distinct".into(),
                );
            }
        }
        if self
            .eyes
            .iter()
            .enumerate()
            .any(|(index, eye)| eye.as_ref().is_some_and(|eye| eye.ready) != seen[index])
        {
            return Err("XR presentation must include every staged eye".into());
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("XR final stereo presentation encoder"),
        });
        for (index, target) in targets {
            let eye = self.eyes[*index].as_ref().unwrap();
            let stamp = super::profiler::stamp("xr.present");
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("XR final eye presentation"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.color,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: eye.write_depth.then_some(
                    wgpu::RenderPassDepthStencilAttachment {
                        view: target.depth,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    },
                ),
                timestamp_writes: stamp.as_ref().map(|stamp| stamp.writes()),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(if eye.write_depth {
                &self.depth_pipeline
            } else {
                &self.color_pipeline
            });
            pass.set_bind_group(0, &eye.source, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit(Some(encoder.finish()));
        self.reset_frame();
        Ok(targets.len())
    }
}
