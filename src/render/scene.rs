#![cfg(target_arch = "wasm32")]

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::data::{self, Vertex, SceneUniforms, Light as SceneLight};
use super::pipeline;

pub trait SceneObject {
    fn update(&mut self, dt: f32);
    fn draw<'r>(&'r self, pass: &mut wgpu::RenderPass<'r>, bind_group_layout: &wgpu::BindGroupLayout);
    fn set_camera(&mut self, _view_proj: Mat4, _cam_pos: Vec3, _lights: &[SceneLight; 3]) {}
    fn light_data(&self) -> Option<SceneLight> { None }
}

pub struct Mesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    model: Mat4,
    queue: wgpu::Queue,
}

impl Mesh {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, verts: &[Vertex], idx: &[u16], format: wgpu::TextureFormat, layout: &wgpu::BindGroupLayout, model: Mat4) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh vertices"),
            contents: data::as_bytes(verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh indices"),
            contents: data::as_bytes(idx),
            usage: wgpu::BufferUsages::INDEX,
        });
        let uniform = SceneUniforms {
            mvp: [[0.0;4];4],
            model: model.to_cols_array_2d(),
            camera_pos: [0.0;3],
            _pad0: 0.0,
            lights: [SceneLight { position: [0.0; 3], _pad_p: 0.0, color: [0.0; 3], _pad_c: 0.0 }; 3],
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh uniform"),
            contents: data::as_bytes(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("mesh bind group"),
        });
        let pipeline = pipeline::build(device, format, layout);
        Self {
            vertex_buffer,
            index_buffer,
            index_count: idx.len() as u32,
            uniform_buffer,
            bind_group,
            pipeline,
            model,
            queue: queue.clone(),
        }
    }

    pub fn cube(device: &wgpu::Device, queue: &wgpu::Queue, pos: [f32; 3], size: f32, format: wgpu::TextureFormat, layout: &wgpu::BindGroupLayout) -> Self {
        let mut verts = data::VERTICES.to_vec();
        for v in verts.iter_mut() {
            v.position[0] = v.position[0] * size + pos[0];
            v.position[1] = v.position[1] * size + pos[1];
            v.position[2] = v.position[2] * size + pos[2];
        }
        Self::new(device, queue, &verts, data::INDICES, format, layout, Mat4::IDENTITY)
    }
}

impl SceneObject for Mesh {
    fn update(&mut self, dt: f32) {
        // simple spin around Z for demo
        self.model = self.model * Mat4::from_rotation_z(dt);
    }

    fn draw<'r>(&'r self, pass: &mut wgpu::RenderPass<'r>, _layout: &wgpu::BindGroupLayout) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }

    fn set_camera(&mut self, view_proj: Mat4, cam_pos: Vec3, lights: &[SceneLight; 3]) {
        let mvp = view_proj * self.model;
        let uniform = SceneUniforms {
            mvp: mvp.to_cols_array_2d(),
            model: self.model.to_cols_array_2d(),
            camera_pos: cam_pos.into(),
            _pad0: 0.0,
            lights: *lights,
        };
        self.queue.write_buffer(&self.uniform_buffer, 0, data::as_bytes(&[uniform]));
    }
}

pub struct Light {
    position: [f32;3],
    color: [f32;3],
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    queue: wgpu::Queue,
}

impl Light {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, pos: [f32;3], color: [f32;3], format: wgpu::TextureFormat, layout: &wgpu::BindGroupLayout) -> Self {
        let l = SceneLight { position: pos, _pad_p: 0.0, color, _pad_c: 0.0 };
        let verts = data::light_rays(&[l]);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("light verts"),
            contents: data::as_bytes(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let uniform = SceneUniforms {
            mvp: [[0.0;4];4],
            model: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0;3],
            _pad0: 0.0,
            lights: [SceneLight { position: [0.0;3], _pad_p: 0.0, color: [0.0;3], _pad_c: 0.0 }; 3],
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("light uniform"),
            contents: data::as_bytes(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() }],
            label: Some("light bind group"),
        });
        let pipeline = pipeline::build_lines(device, format, layout);
        Self {
            position: pos,
            color,
            vertex_buffer,
            vertex_count: verts.len() as u32,
            uniform_buffer,
            bind_group,
            pipeline,
            queue: queue.clone(),
        }
    }
}

impl SceneObject for Light {
    fn update(&mut self, _dt: f32) {}

    fn draw<'r>(&'r self, pass: &mut wgpu::RenderPass<'r>, _layout: &wgpu::BindGroupLayout) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..self.vertex_count, 0..1);
    }

    fn set_camera(&mut self, view_proj: Mat4, cam_pos: Vec3, lights: &[SceneLight; 3]) {
        let uniform = SceneUniforms {
            mvp: view_proj.to_cols_array_2d(),
            model: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: cam_pos.into(),
            _pad0: 0.0,
            lights: *lights,
        };
        self.queue.write_buffer(&self.uniform_buffer, 0, data::as_bytes(&[uniform]));
    }

    fn light_data(&self) -> Option<SceneLight> {
        Some(SceneLight { position: self.position, _pad_p: 0.0, color: self.color, _pad_c: 0.0 })
    }
}

pub struct Grid {
    vertex_buffer: wgpu::Buffer,
    vertex_count: u32,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    queue: wgpu::Queue,
}

impl Grid {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, size: i32, format: wgpu::TextureFormat, layout: &wgpu::BindGroupLayout) -> Self {
        let verts = data::grid_vertices(size);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid verts"),
            contents: data::as_bytes(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let uniform = SceneUniforms {
            mvp: [[0.0; 4]; 4],
            model: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0; 3],
            _pad0: 0.0,
            lights: [SceneLight { position: [0.0; 3], _pad_p: 0.0, color: [0.0; 3], _pad_c: 0.0 }; 3],
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid uniform"),
            contents: data::as_bytes(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() }],
            label: Some("grid bind group"),
        });
        let pipeline = pipeline::build_lines(device, format, layout);
        Self { vertex_buffer, vertex_count: verts.len() as u32, uniform_buffer, bind_group, pipeline, queue: queue.clone() }
    }
}

impl SceneObject for Grid {
    fn update(&mut self, _dt: f32) {}

    fn draw<'r>(&'r self, pass: &mut wgpu::RenderPass<'r>, _layout: &wgpu::BindGroupLayout) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..self.vertex_count, 0..1);
    }

    fn set_camera(&mut self, view_proj: Mat4, cam_pos: Vec3, lights: &[SceneLight; 3]) {
        let uniform = SceneUniforms {
            mvp: view_proj.to_cols_array_2d(),
            model: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: cam_pos.into(),
            _pad0: 0.0,
            lights: *lights,
        };
        self.queue.write_buffer(&self.uniform_buffer, 0, data::as_bytes(&[uniform]));
    }
}

pub struct SceneManager {
    objects: Vec<Box<dyn SceneObject>>,
    bind_group_layout: wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl SceneManager {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene bind layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        Self { objects: Vec::new(), bind_group_layout, format, device, queue }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout { &self.bind_group_layout }
    pub fn device(&self) -> &wgpu::Device { &self.device }
    pub fn queue(&self) -> &wgpu::Queue { &self.queue }
    pub fn format(&self) -> wgpu::TextureFormat { self.format }

    pub fn add_mesh(&mut self, verts: &[Vertex], idx: &[u16], model: Mat4) -> usize {
        let mesh = Mesh::new(&self.device, &self.queue, verts, idx, self.format, &self.bind_group_layout, model);
        let id = self.objects.len();
        self.objects.push(Box::new(mesh));
        id
    }

    pub fn add_light(&mut self, pos: [f32;3], color: [f32;3]) -> usize {
        let light = Light::new(&self.device, &self.queue, pos, color, self.format, &self.bind_group_layout);
        let id = self.objects.len();
        self.objects.push(Box::new(light));
        id
    }

    pub fn add_grid(&mut self, size: i32) -> usize {
        let grid = Grid::new(&self.device, &self.queue, size, self.format, &self.bind_group_layout);
        let id = self.objects.len();
        self.objects.push(Box::new(grid));
        id
    }

    pub fn remove(&mut self, id: usize) {
        if id < self.objects.len() {
            self.objects.remove(id);
        }
    }

    fn lights_array(&self) -> [SceneLight; 3] {
        // Pokud není přidáno žádné světlo, chceme mít prázdné sloty
        let default = SceneLight { position: [0.0; 3], _pad_p: 0.0, color: [0.0; 3], _pad_c: 0.0 };
        let mut arr = [default; 3];
        let mut i = 0;
        for o in self.objects.iter() {
            if let Some(l) = o.light_data() {
                if i < 3 { arr[i] = l; i += 1; }
            }
        }
        arr
    }

    pub fn update(&mut self, dt: f32) {
        for o in self.objects.iter_mut() {
            o.update(dt);
        }
    }

    pub fn set_camera(&mut self, view_proj: Mat4, cam_pos: Vec3) {
        let lights = self.lights_array();
        for o in self.objects.iter_mut() {
            o.set_camera(view_proj, cam_pos, &lights);
        }
    }

    pub fn draw<'r>(&'r self, pass: &mut wgpu::RenderPass<'r>) {
        for o in self.objects.iter() {
            o.draw(pass, &self.bind_group_layout);
        }
    }
}

