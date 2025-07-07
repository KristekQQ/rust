#![cfg(target_arch = "wasm32")]

use glam::{Mat4, Vec3};
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;
use wgpu::util::DeviceExt;

use crate::render::data::{self, Light as ShaderLight, SceneUniforms};
use crate::render::{depth, pipeline};
use crate::scene::{Light, Node, Scene};
use std::mem;

const NODE_SIZE: wgpu::BufferAddress = mem::size_of::<Node>() as wgpu::BufferAddress;
const LIGHT_SIZE: wgpu::BufferAddress = mem::size_of::<Light>() as wgpu::BufferAddress;

pub struct State {
    grid_pipeline: wgpu::RenderPipeline,
    grid_vertex_buffer: wgpu::Buffer,
    grid_vertex_count: u32,
    light_vertex_buffer: wgpu::Buffer,
    light_vertex_count: u32,
    pub draw_grid: bool,
    pub scene: Scene,
    instance_uniform_buffer: wgpu::Buffer,
    light_uniform_buffer: wgpu::Buffer,
    node_capacity: usize,
    light_capacity: usize,
    bind_group_layout: wgpu::BindGroupLayout,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
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
    camera_matrix: Mat4,
    camera_pos: glam::Vec3,
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
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: adapter.limits(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::default(),
            })
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
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
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

        let scene = Scene::new();

        let node_capacity = 1usize;
        let light_capacity = 1usize;
        let instance_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instance buffer"),
            size: NODE_SIZE * node_capacity as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light buffer"),
            size: LIGHT_SIZE * light_capacity as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform = SceneUniforms {
            mvp: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            model: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            camera_pos: [0.0, 0.0, 0.0],
            _pad0: 0.0,
            lights: [
                ShaderLight {
                    position: [0.0, 0.0, 0.0],
                    _pad_p: 0.0,
                    color: [0.0, 0.0, 0.0],
                    _pad_c: 0.0,
                },
                ShaderLight {
                    position: [0.0, 0.0, 0.0],
                    _pad_p: 0.0,
                    color: [0.0, 0.0, 0.0],
                    _pad_c: 0.0,
                },
            ],
        };

        let light_vertices = data::light_rays(&uniform.lights);
        let light_vertex_count = light_vertices.len() as u32;
        let light_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("light vertex buffer"),
            contents: data::as_bytes(&light_vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let cube_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube uniform buffer"),
            contents: data::as_bytes(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let cube_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cube_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: instance_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: light_uniform_buffer.as_entire_binding(),
                },
            ],
            label: Some("bind group"),
        });
        let grid_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid uniform buffer"),
            contents: data::as_bytes(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let grid_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: grid_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: instance_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: light_uniform_buffer.as_entire_binding(),
                },
            ],
            label: Some("grid bind group"),
        });

        Ok(Self {
            grid_pipeline,
            grid_vertex_buffer,
            grid_vertex_count,
            light_vertex_buffer,
            light_vertex_count,
            draw_grid: true,
            scene,
            instance_uniform_buffer,
            light_uniform_buffer,
            node_capacity,
            light_capacity,
            bind_group_layout,
            surface,
            device,
            queue,
            config,
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
            camera_matrix: Mat4::IDENTITY,
            camera_pos: Vec3::ZERO,
        })
    }
    pub fn set_grid_visible(&mut self, show: bool) {
        self.draw_grid = show;
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.aspect = width as f32 / height as f32;
        self.surface.configure(&self.device, &self.config);
        let (depth_texture, depth_view) =
            depth::create(&self.device, width, height, self.depth_format);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
    }

    pub fn update(&mut self, camera_matrix: Mat4, model: Mat4, camera_pos: Vec3) {
        self.camera_matrix = camera_matrix;
        self.camera_pos = camera_pos;
        self.ensure_capacity();
        if let Some(node) = self.scene.nodes.get_mut(0) {
            node.model = model.to_cols_array_2d();
        }
        let mut lights_arr = [ShaderLight {
            position: [0.0; 3],
            _pad_p: 0.0,
            color: [0.0; 3],
            _pad_c: 0.0,
        }; 2];
        for (i, l) in self.scene.lights.iter().take(2).enumerate() {
            lights_arr[i] = ShaderLight {
                position: l.position,
                _pad_p: l._pad_p,
                color: l.color,
                _pad_c: l._pad_c,
            };
        }
        let grid_uniform = SceneUniforms {
            mvp: camera_matrix.to_cols_array_2d(),
            model: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            camera_pos: camera_pos.into(),
            _pad0: 0.0,
            lights: lights_arr,
        };
        self.queue.write_buffer(
            &self.grid_uniform_buffer,
            0,
            data::as_bytes(&[grid_uniform]),
        );
        self.queue.write_buffer(
            &self.instance_uniform_buffer,
            0,
            data::as_bytes(&self.scene.nodes),
        );
        self.queue.write_buffer(
            &self.light_uniform_buffer,
            0,
            data::as_bytes(&self.scene.lights),
        );
        let light_vertices = data::light_rays(&lights_arr);
        self.queue.write_buffer(
            &self.light_vertex_buffer,
            0,
            data::as_bytes(&light_vertices),
        );
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

            let mut lights_arr = [ShaderLight {
                position: [0.0; 3],
                _pad_p: 0.0,
                color: [0.0; 3],
                _pad_c: 0.0,
            }; 2];
            for (i, l) in self.scene.lights.iter().take(2).enumerate() {
                lights_arr[i] = ShaderLight {
                    position: l.position,
                    _pad_p: l._pad_p,
                    color: l.color,
                    _pad_c: l._pad_c,
                };
            }

            for (i, node) in self.scene.nodes.iter().enumerate() {
                let mut model = Mat4::from_cols_array_2d(&node.model);
                let mut p = node.parent;
                while p >= 0 {
                    let parent = &self.scene.nodes[p as usize];
                    model = Mat4::from_cols_array_2d(&parent.model) * model;
                    p = parent.parent;
                }
                let mvp = self.camera_matrix * model;
                let cube_uniform = SceneUniforms {
                    mvp: mvp.to_cols_array_2d(),
                    model: model.to_cols_array_2d(),
                    camera_pos: self.camera_pos.into(),
                    _pad0: 0.0,
                    lights: lights_arr,
                };
                self.queue.write_buffer(
                    &self.cube_uniform_buffer,
                    0,
                    data::as_bytes(&[cube_uniform]),
                );
                rp.draw_indexed(0..data::INDICES.len() as u32, 0, i as u32..i as u32 + 1);
            }
            if self.draw_grid {
                rp.set_pipeline(&self.grid_pipeline);
                rp.set_bind_group(0, &self.grid_bind_group, &[]);
                rp.set_vertex_buffer(0, self.grid_vertex_buffer.slice(..));
                rp.draw(0..self.grid_vertex_count, 0..1);
                rp.set_vertex_buffer(0, self.light_vertex_buffer.slice(..));
                rp.draw(0..self.light_vertex_count, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }

    pub fn ensure_capacity(&mut self) {
        let mut changed = false;
        let needed_nodes = self.scene.nodes.len().max(1);
        if needed_nodes > self.node_capacity {
            while self.node_capacity < needed_nodes {
                self.node_capacity *= 2;
            }
            self.instance_uniform_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instance buffer"),
                size: NODE_SIZE * self.node_capacity as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            changed = true;
        }
        let needed_lights = self.scene.lights.len().max(1);
        if needed_lights > self.light_capacity {
            while self.light_capacity < needed_lights {
                self.light_capacity *= 2;
            }
            self.light_uniform_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("light buffer"),
                size: LIGHT_SIZE * self.light_capacity as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            changed = true;
        }
        if changed {
            self.cube_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.cube_uniform_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.instance_uniform_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: self.light_uniform_buffer.as_entire_binding() },
                ],
                label: Some("bind group"),
            });
            self.grid_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.grid_uniform_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: self.instance_uniform_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: self.light_uniform_buffer.as_entire_binding() },
                ],
                label: Some("grid bind group"),
            });
        }
    }
}
