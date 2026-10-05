use crate::scene::{MeshKind, SceneManager};
use crate::visibility::{InstanceData, RenderQueue};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::render::data::{
    self, Light, SceneUniforms, Vertex, EMPTY_LIGHT, LIGHT_VERTICES_PER_LIGHT, MAX_LIGHTS,
};
use crate::render::{
    pipeline,
    target::{RenderTarget, RenderView},
};
use std::collections::HashMap;

struct InstanceBatch {
    buffer: wgpu::Buffer,
    capacity: usize,
    uploaded: Vec<InstanceData>,
}
impl InstanceBatch {
    fn new(device: &wgpu::Device, capacity: usize) -> Self {
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mesh instance buffer"),
                size: (capacity * std::mem::size_of::<InstanceData>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
            uploaded: Vec::new(),
        }
    }
    fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[InstanceData],
    ) -> usize {
        if instances.len() > self.capacity {
            *self = Self::new(device, instances.len().next_power_of_two());
        }
        if self.uploaded == instances {
            return 0;
        }
        self.uploaded.clear();
        self.uploaded.extend_from_slice(instances);
        if instances.is_empty() {
            return 0;
        }
        let bytes = data::as_bytes(instances);
        queue.write_buffer(&self.buffer, 0, bytes);
        bytes.len()
    }
}

struct Mesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl Mesh {
    fn new(device: &wgpu::Device, label: &str, vertices: &[Vertex], indices: &[u16]) -> Self {
        let vertex_label = format!("{label} vertex buffer");
        let index_label = format!("{label} index buffer");
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(vertex_label.as_str()),
            contents: data::as_bytes(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(index_label.as_str()),
            contents: data::as_bytes(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }
}

fn lights_array(lights: &[Light]) -> [Light; MAX_LIGHTS] {
    let mut out = [EMPTY_LIGHT; MAX_LIGHTS];
    for (dst, src) in out.iter_mut().zip(lights.iter()) {
        *dst = *src;
    }
    out
}

fn build_uniform(
    camera_matrix: Mat4,
    camera_pos: Vec3,
    lights: [Light; MAX_LIGHTS],
) -> SceneUniforms {
    SceneUniforms {
        view_projection: camera_matrix.to_cols_array_2d(),
        camera_pos: camera_pos.into(),
        _pad0: 0.0,
        lights,
    }
}

pub struct SceneRenderer {
    grid_vertex_buffer: wgpu::Buffer,
    grid_vertex_count: u32,
    light_vertex_buffer: wgpu::Buffer,
    light_vertex_count: u32,
    pub draw_grid: bool,
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pipelines: HashMap<
        (wgpu::TextureFormat, wgpu::TextureFormat),
        (wgpu::RenderPipeline, wgpu::RenderPipeline),
    >,
    bind_group_layout: wgpu::BindGroupLayout,
    cube_mesh: Mesh,
    plane_mesh: Mesh,
    sphere_mesh: Mesh,
    grid_uniform_buffer: wgpu::Buffer,
    grid_bind_group: wgpu::BindGroup,
    pub scene: SceneManager,
    render_queue: RenderQueue,
    extraction_key: Option<(Mat4, u64, bool)>,
    batches: [InstanceBatch; 3],
    pub frustum_culling: bool,
    pub upload_bytes: usize,
    pub prepare_ms: f64,
    frame_metrics: crate::frame_metrics::FrameMetrics,
    last_lights: [Light; MAX_LIGHTS],
}

impl SceneRenderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let cube_mesh = Mesh::new(&device, "cube", data::VERTICES, data::INDICES);
        let (plane_vertices, plane_indices) = data::plane_mesh();
        let plane_mesh = Mesh::new(&device, "plane", &plane_vertices, &plane_indices);
        let (sphere_vertices, sphere_indices) = data::sphere_mesh(24, 16);
        let sphere_mesh = Mesh::new(&device, "sphere", &sphere_vertices, &sphere_indices);

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

        let grid_vertices = data::grid_vertices(10);
        let grid_vertex_count = grid_vertices.len() as u32;
        let grid_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid vertex buffer"),
            contents: data::as_bytes(&grid_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let active_lights = Vec::new();
        let lights_array = lights_array(&active_lights);
        let grid_uniform = build_uniform(Mat4::IDENTITY, Vec3::ZERO, lights_array);

        let light_vertices = data::light_rays(&active_lights);
        let light_vertex_count = light_vertices.len() as u32;
        let max_light_vertices = (MAX_LIGHTS * LIGHT_VERTICES_PER_LIGHT) as u64;
        let light_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("light vertex buffer"),
            size: max_light_vertices * std::mem::size_of::<data::Vertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let grid_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("grid uniform buffer"),
            contents: data::as_bytes(&[grid_uniform]),
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
        let batches = std::array::from_fn(|_| InstanceBatch::new(&device, 1));
        Self {
            grid_vertex_buffer,
            grid_vertex_count,
            light_vertex_buffer,
            light_vertex_count,
            draw_grid: true,
            device,
            queue,
            pipelines: HashMap::new(),
            bind_group_layout,
            cube_mesh,
            plane_mesh,
            sphere_mesh,
            grid_uniform_buffer,
            grid_bind_group,
            scene: SceneManager::default(),
            render_queue: RenderQueue::default(),
            extraction_key: None,
            batches,
            frustum_culling: true,
            upload_bytes: 0,
            prepare_ms: 0.0,
            frame_metrics: crate::frame_metrics::FrameMetrics::default(),
            last_lights: lights_array,
        }
    }
    pub fn set_grid_visible(&mut self, show: bool) {
        self.draw_grid = show;
    }

    fn mesh(&self, kind: MeshKind) -> &Mesh {
        match kind {
            MeshKind::Cube => &self.cube_mesh,
            MeshKind::Plane => &self.plane_mesh,
            MeshKind::Sphere => &self.sphere_mesh,
        }
    }

    /// Advance simulation once per frame, independently of the number of views.
    pub fn advance_scene(&mut self, dt: f32) {
        self.frame_metrics.record(dt);
        self.scene.update(dt);
    }

    pub fn prepare_view(&mut self, view: RenderView) {
        let camera_matrix = view.view_projection;
        let camera_pos = view.camera_position;
        let active_lights: Vec<Light> = self
            .scene
            .lights()
            .values()
            .map(|light| Light {
                position: light.position.to_array(),
                _pad_p: 0.0,
                color: light.color.to_array(),
                _pad_c: 0.0,
            })
            .collect();
        let lights = lights_array(&active_lights);
        let key = (
            camera_matrix,
            self.scene.render_revision(),
            self.frustum_culling,
        );
        self.upload_bytes = 0;
        if self.extraction_key != Some(key) {
            self.render_queue
                .extract(&self.scene, camera_matrix, self.frustum_culling);
            for (batch, instances) in self.batches.iter_mut().zip(&self.render_queue.batches) {
                self.upload_bytes += batch.upload(&self.device, &self.queue, instances);
            }
            self.extraction_key = Some(key);
        }
        let grid_uniform = build_uniform(camera_matrix, camera_pos, lights);
        self.queue.write_buffer(
            &self.grid_uniform_buffer,
            0,
            data::as_bytes(&[grid_uniform]),
        );
        self.upload_bytes += std::mem::size_of::<SceneUniforms>();
        if self.last_lights != lights {
            self.last_lights = lights;
            let light_vertices = data::light_rays(&active_lights);
            self.light_vertex_count = light_vertices.len() as u32;
            if !light_vertices.is_empty() {
                self.queue.write_buffer(
                    &self.light_vertex_buffer,
                    0,
                    data::as_bytes(&light_vertices),
                );
                self.upload_bytes += light_vertices.len() * std::mem::size_of::<Vertex>();
            }
        }
    }

    pub fn render_stats(&self) -> [f64; 8] {
        [
            self.render_queue.total as f64,
            self.render_queue.visible as f64,
            (self.render_queue.total - self.render_queue.visible) as f64,
            self.render_queue.draw_calls() as f64,
            self.upload_bytes as f64,
            self.prepare_ms,
            self.frame_metrics.fps,
            self.frame_metrics.frame_ms,
        ]
    }

    /// Prepare and submit one view. Simulation advances separately, once per frame.
    /// Submission before the next view keeps shared buffer writes ordered correctly.
    pub fn render_view(
        &mut self,
        view: RenderView,
        target: &RenderTarget<'_>,
    ) -> Result<(), String> {
        target.validate()?;
        self.prepare_view(view);
        self.draw(target)
    }

    pub fn draw(&mut self, target: &RenderTarget<'_>) -> Result<(), String> {
        target.validate()?;
        let key = (target.color_format, target.depth_format);
        self.pipelines.entry(key).or_insert_with(|| {
            (
                pipeline::build(&self.device, key.0, key.1, &self.bind_group_layout),
                pipeline::build_lines(&self.device, key.0, key.1, &self.bind_group_layout),
            )
        });
        // Immutable lookup ends the map mutation before borrowing mesh resources.
        let pipelines = &self.pipelines[&key];
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.color,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: target.color_load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: target.depth_load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            rp.set_viewport(
                target.viewport.x as f32,
                target.viewport.y as f32,
                target.viewport.width as f32,
                target.viewport.height as f32,
                0.0,
                1.0,
            );
            rp.set_scissor_rect(
                target.viewport.x,
                target.viewport.y,
                target.viewport.width,
                target.viewport.height,
            );
            rp.set_pipeline(&pipelines.0);
            rp.set_bind_group(0, &self.grid_bind_group, &[]);
            for (index, kind) in [MeshKind::Cube, MeshKind::Plane, MeshKind::Sphere]
                .into_iter()
                .enumerate()
            {
                let count = self.render_queue.batches[index].len() as u32;
                if count == 0 {
                    continue;
                }
                let mesh = self.mesh(kind);
                rp.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rp.set_vertex_buffer(1, self.batches[index].buffer.slice(..));
                rp.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                rp.draw_indexed(0..mesh.index_count, 0, 0..count);
            }
            if self.draw_grid {
                rp.set_pipeline(&pipelines.1);
                rp.set_bind_group(0, &self.grid_bind_group, &[]);
                rp.set_vertex_buffer(0, self.grid_vertex_buffer.slice(..));
                rp.draw(0..self.grid_vertex_count, 0..1);
                rp.set_vertex_buffer(0, self.light_vertex_buffer.slice(..));
                rp.draw(0..self.light_vertex_count, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }
}
