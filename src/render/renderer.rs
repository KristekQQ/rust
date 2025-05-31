use glam::Mat4;
use wasm_bindgen::JsValue;

use crate::render::{init::{GpuContext, init}, buffers::{GpuBuffers, create}, depth::create_depth_texture, pipeline::build, types::{Uniforms, as_bytes}};
use crate::scene::cube::INDICES;

pub struct Renderer {
    context: GpuContext,
    pipeline: wgpu::RenderPipeline,
    buffers: GpuBuffers,
    bind_group: wgpu::BindGroup,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
}

impl Renderer {
    pub async fn new(canvas: &web_sys::HtmlCanvasElement) -> Result<Self, JsValue> {
        let context = init(canvas).await?;
        let (depth_texture, depth_view) = create_depth_texture(
            &context.device,
            context.config.width,
            context.config.height,
            context.depth_format,
        );
        let buffers = create(&context.device);
        let bind_group_layout = context.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffers.uniform.as_entire_binding(),
            }],
            label: Some("bind group"),
        });
        let pipeline = build(&context.device, context.config.format);
        Ok(Self { context, pipeline, buffers, bind_group, depth_texture, depth_view })
    }

    pub fn aspect(&self) -> f32 {
        self.context.config.width as f32 / self.context.config.height as f32
    }

    pub fn update(&mut self, mvp: Mat4) {
        let uniform = Uniforms { mvp: mvp.to_cols_array_2d() };
        self.context.queue.write_buffer(&self.buffers.uniform, 0, as_bytes(&[uniform]));
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let frame = self.context.surface.get_current_texture()?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("encoder"),
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.1, b: 0.3, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            rp.set_pipeline(&self.pipeline);
            rp.set_bind_group(0, &self.bind_group, &[]);
            rp.set_vertex_buffer(0, self.buffers.vertex.slice(..));
            rp.set_index_buffer(self.buffers.index.slice(..), wgpu::IndexFormat::Uint16);
            rp.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
        }
        self.context.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}
