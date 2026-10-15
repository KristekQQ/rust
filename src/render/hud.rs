//! Optional final-output pass. No resources exist until first enabled.
use super::target::RenderTarget;
use crate::debug_overlay::HudRect;

#[derive(Default)]
pub struct DebugHud {
    pipeline: Option<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    buffer: Option<wgpu::Buffer>,
    capacity: usize,
    count: u32,
    size: (u32, u32),
    elapsed: f32,
    pub frames: u64,
    pub refreshes: u64,
}
impl DebugHud {
    pub fn invalidate(&mut self) {
        self.size = (0, 0);
    }
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &RenderTarget<'_>,
        dt: f32,
        lines: impl FnOnce() -> Vec<String>,
    ) -> usize {
        self.elapsed += dt.clamp(0.0, 0.25);
        let mut uploaded = 0;
        if self.size != (target.width, target.height) || self.elapsed >= 0.25 {
            let rects = crate::debug_overlay::geometry(&lines(), target.width, target.height);
            if rects.len() > self.capacity {
                self.capacity = rects.len().next_power_of_two();
                self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("debug HUD pixels"),
                    size: (self.capacity * std::mem::size_of::<HudRect>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            }
            let bytes = bytemuck::cast_slice(&rects);
            queue.write_buffer(self.buffer.as_ref().unwrap(), 0, bytes);
            uploaded = bytes.len();
            self.count = rects.len() as u32;
            self.size = (target.width, target.height);
            self.elapsed = 0.0;
            self.refreshes += 1;
        }
        if self.pipeline.as_ref().map(|p| p.0) != Some(target.color_format) {
            let shader = device.create_shader_module(wgpu::include_wgsl!("hud.wgsl"));
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("debug HUD layout"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("debug HUD"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<HudRect>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4],
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target.color_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
            self.pipeline = Some((target.color_format, pipeline));
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("debug HUD"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("debug HUD"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline.as_ref().unwrap().1);
            pass.set_vertex_buffer(0, self.buffer.as_ref().unwrap().slice(..));
            pass.draw(0..6, 0..self.count);
        }
        queue.submit(Some(encoder.finish()));
        self.frames += 1;
        uploaded
    }
}
