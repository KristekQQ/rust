#![cfg(target_arch = "wasm32")]

use glam::Mat4;
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;
use wgpu::util::DeviceExt;

use crate::render::data::{self, SceneUniforms, Light};
use crate::render::{depth, pipeline};

pub struct State {
    grid_pipeline: wgpu::RenderPipeline,
    grid_vertex_buffer: wgpu::Buffer,
    grid_vertex_count: u32,
    pub draw_grid: bool,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    cube_uniform_buffer: wgpu::Buffer,
    grid_uniform_buffer: wgpu::Buffer,
    cube_bind_group: wgpu::BindGroup,
    grid_bind_group: wgpu::BindGroup,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    depth_format: wgpu::TextureFormat,
    pub aspect: f32,
}

impl State {
    pub async fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| JsValue::from_str(&format!("{e:?}")))?;
        let surface =
            unsafe { std::mem::transmute::<wgpu::Surface<'_>, wgpu::Surface<'static>>(surface) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: adapter.limits(),
                    memory_hints: wgpu::MemoryHints::default(),
                    trace: wgpu::Trace::default(),
                },
            )
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let caps = surface.get_capabilities(&adapter);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: caps.formats[0],
            width: canvas.width(),
            height: canvas.height(),
            present_mode: caps.present_modes[0],
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let aspect = config.width as f32 / config.height as f32;

        let depth_format = wgpu::TextureFormat::Depth32Float;
        let (depth_texture, depth_view) =
            depth::create(&device, config.width, config.height, depth_format);

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex buffer"),
            contents: data::as_bytes(data::VERTICES),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index buffer"),
            contents: data::as_bytes(data::INDICES),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline = pipeline::build(&device, config.format, &bind_group_layout);
        let grid_pipeline = pipeline::build_lines(&device, config.format, &bind_group_layout);
        let grid_vertices = data::grid_vertices(10);
        let grid_vertex_count = grid_vertices.len() as u32;
        let grid_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid vertex buffer"),
            contents: data::as_bytes(&grid_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let uniform = SceneUniforms {
            mvp: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            camera_pos: [0.0, 0.0, 0.0],
            _pad0: 0.0,
            lights: [
                Light {
                    position: [1.5, 1.0, 2.0],
                    _pad_p: 0.0,
                    color: [1.0, 1.0, 1.0],
                    _pad_c: 0.0,
                },
                Light {
                    position: [-1.5, 1.0, -2.0],
                    _pad_p: 0.0,
                    color: [1.0, 0.0, 0.0],
                    _pad_c: 0.0,
                },
            ],
        };
        let cube_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube uniform buffer"),
            contents: data::as_bytes(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let cube_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: cube_uniform_buffer.as_entire_binding(),
            }],
            label: Some("bind group"),
        });
        let grid_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid uniform buffer"),
            contents: data::as_bytes(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let grid_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: grid_uniform_buffer.as_entire_binding(),
            }],
            label: Some("grid bind group"),
        });

        Ok(Self {
            grid_pipeline,
            grid_vertex_buffer,
            grid_vertex_count,
            draw_grid: true,
            surface,
            device,
            queue,
            pipeline,
            vertex_buffer,
            index_buffer,
            cube_uniform_buffer,
            grid_uniform_buffer,
            cube_bind_group,
            grid_bind_group,
            depth_texture,
            depth_view,
            depth_format,
            aspect,
        })
    }
    pub fn set_grid_visible(&mut self, show: bool) {
        self.draw_grid = show;
    }



    pub fn update(&self, camera_matrix: Mat4, model: Mat4, camera_pos: glam::Vec3) {
        let cube_mvp = camera_matrix * model;
        let cube_uniform = SceneUniforms {
            mvp: cube_mvp.to_cols_array_2d(),
            camera_pos: camera_pos.into(),
            _pad0: 0.0,
            lights: [
                Light {
                    position: [1.5, 1.0, 2.0],
                    _pad_p: 0.0,
                    color: [1.0, 1.0, 1.0],
                    _pad_c: 0.0,
                },
                Light {
                    position: [-1.5, 1.0, -2.0],
                    _pad_p: 0.0,
                    color: [1.0, 0.0, 0.0],
                    _pad_c: 0.0,
                },
            ],
        };
        let grid_uniform = SceneUniforms {
            mvp: camera_matrix.to_cols_array_2d(),
            camera_pos: camera_pos.into(),
            _pad0: 0.0,
            lights: [
                Light {
                    position: [1.5, 1.0, 2.0],
                    _pad_p: 0.0,
                    color: [1.0, 1.0, 1.0],
                    _pad_c: 0.0,
                },
                Light {
                    position: [-1.5, 1.0, -2.0],
                    _pad_p: 0.0,
                    color: [1.0, 0.0, 0.0],
                    _pad_c: 0.0,
                },
            ],
        };
        self.queue
            .write_buffer(&self.cube_uniform_buffer, 0, data::as_bytes(&[cube_uniform]));
        self.queue
            .write_buffer(&self.grid_uniform_buffer, 0, data::as_bytes(&[grid_uniform]));
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.3,
                            a: 1.0,
                        }),
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
            rp.set_bind_group(0, &self.cube_bind_group, &[]);
            rp.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            rp.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            rp.draw_indexed(0..data::INDICES.len() as u32, 0, 0..1);
            if self.draw_grid {
                rp.set_pipeline(&self.grid_pipeline);
                rp.set_bind_group(0, &self.grid_bind_group, &[]);
                rp.set_vertex_buffer(0, self.grid_vertex_buffer.slice(..));
                rp.draw(0..self.grid_vertex_count, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}
