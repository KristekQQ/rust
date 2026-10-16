use super::effects::Effects;
use super::target::DepthRenderTarget;
use crate::scene::{MeshKind, SceneManager};
use crate::visibility::{InstanceData, RenderPassKind, RenderQueue};
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

type ColorPipelineKey = (wgpu::TextureFormat, wgpu::TextureFormat, bool, bool);
type ColorPipelines = (
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
);

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
    index_format: wgpu::IndexFormat,
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
            index_format: wgpu::IndexFormat::Uint16,
        }
    }
    fn imported(device: &wgpu::Device, model: &crate::gltf_loader::ImportedMesh) -> Self {
        let vertices: Vec<Vertex> = model
            .vertices
            .iter()
            .map(|v| Vertex {
                position: v.position,
                color: v.color,
                normal: v.normal,
            })
            .collect();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("glTF vertices"),
            contents: data::as_bytes(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("glTF u32 indices"),
            contents: data::as_bytes(&model.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            vertex_buffer,
            index_buffer,
            index_count: model.indices.len() as u32,
            index_format: wgpu::IndexFormat::Uint32,
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
        cutting_guide: [[0.0; 4]; 3],
    }
}

struct PassCache {
    camera_position: Vec3,
    queue: RenderQueue,
    key: Option<(Mat4, u64, bool)>,
    batches: [InstanceBatch; 3],
    mirrored_batches: [InstanceBatch; 3],
    imported: HashMap<u32, InstanceBatch>,
}
impl PassCache {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            queue: RenderQueue::default(),
            camera_position: Vec3::ZERO,
            key: None,
            batches: std::array::from_fn(|_| InstanceBatch::new(device, 1)),
            mirrored_batches: std::array::from_fn(|_| InstanceBatch::new(device, 1)),
            imported: HashMap::new(),
        }
    }
}

pub struct SceneRenderer {
    grid_vertex_buffer: wgpu::Buffer,
    grid_vertex_count: u32,
    light_vertex_buffer: wgpu::Buffer,
    light_vertex_count: u32,
    pub draw_grid: bool,
    pub draw_light_helpers: bool,
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pipelines: HashMap<ColorPipelineKey, ColorPipelines>,
    bind_group_layout: wgpu::BindGroupLayout,
    cube_mesh: Mesh,
    plane_mesh: Mesh,
    sphere_mesh: Mesh,
    imported_meshes: HashMap<u32, Mesh>,
    gltf_gpu: super::gltf_gpu::GltfGpu,
    gltf_models: HashMap<u32, super::gltf_gpu::GpuModel>,
    grid_uniform_buffer: wgpu::Buffer,
    grid_bind_group: wgpu::BindGroup,
    pub scene: SceneManager,
    passes: [PassCache; 2 + MAX_LIGHTS * 2],
    active_cache: usize,
    helper_pipelines: HashMap<(wgpu::TextureFormat, wgpu::TextureFormat), wgpu::RenderPipeline>,
    shadow_pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
    effects: Option<Effects>,
    pub frustum_culling: bool,
    pub upload_bytes: usize,
    pub prepare_ms: f64,
    /// Actual draw/draw_indexed commands in the current/last submitted frame.
    pub draw_calls: usize,
    pub main_draw_calls: usize,
    debug_enabled: bool,
    cull_ms: f64,
    cull_cached: bool,
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
        let passes = std::array::from_fn(|_| PassCache::new(&device));
        let effects = Some(Effects::new(&device));
        let gltf_gpu = super::gltf_gpu::GltfGpu::new(
            &device,
            &bind_group_layout,
            &effects.as_ref().unwrap().layout,
        );
        Self {
            grid_vertex_buffer,
            grid_vertex_count,
            light_vertex_buffer,
            light_vertex_count,
            draw_grid: true,
            draw_light_helpers: true,
            device,
            queue,
            pipelines: HashMap::new(),
            bind_group_layout,
            cube_mesh,
            plane_mesh,
            sphere_mesh,
            imported_meshes: HashMap::new(),
            gltf_gpu,
            gltf_models: HashMap::new(),
            grid_uniform_buffer,
            grid_bind_group,
            scene: SceneManager::default(),
            passes,
            active_cache: 0,
            shadow_pipelines: HashMap::new(),
            helper_pipelines: HashMap::new(),
            effects,
            frustum_culling: true,
            upload_bytes: 0,
            prepare_ms: 0.0,
            draw_calls: 0,
            main_draw_calls: 0,
            debug_enabled: false,
            cull_ms: 0.0,
            cull_cached: true,
            frame_metrics: crate::frame_metrics::FrameMetrics::default(),
            last_lights: lights_array,
        }
    }
    /// Stable first-light maps; postprocessing shares these resources instead of re-rendering shadows.
    pub fn shadow_maps(&self) -> (&wgpu::TextureView, &wgpu::TextureView) {
        let effects = self
            .effects
            .as_ref()
            .expect("effects available outside render_view");
        (
            &effects.static_shadow[0].view,
            &effects.dynamic_shadow[0].view,
        )
    }
    pub fn shadows_enabled(&self) -> bool {
        self.effects.as_ref().is_some_and(|effects| effects.shadows)
    }
    pub fn set_grid_visible(&mut self, show: bool) {
        self.draw_grid = show;
    }

    pub fn set_light_helpers_visible(&mut self, show: bool) {
        self.draw_light_helpers = show;
    }

    fn mesh(&self, kind: MeshKind) -> &Mesh {
        match kind {
            MeshKind::Cube => &self.cube_mesh,
            MeshKind::Plane => &self.plane_mesh,
            MeshKind::Sphere => &self.sphere_mesh,
            MeshKind::Imported(id) => &self.imported_meshes[&id],
        }
    }

    /// Advance simulation once per frame, independently of the number of views.
    pub fn install_resident(
        &mut self,
        id: u32,
        piece: super::cutting_resident::ResidentPiece,
    ) -> super::cutting_resident::ResidentSource {
        self.imported_meshes.insert(
            id,
            Mesh {
                vertex_buffer: piece.render_vertices,
                index_buffer: piece.indices,
                index_count: piece.vertex_count,
                index_format: wgpu::IndexFormat::Uint32,
            },
        );
        piece.source
    }
    pub fn import_model(
        &mut self,
        model: &crate::gltf_loader::ImportedMesh,
    ) -> Result<u32, String> {
        let limit = self.device.limits().max_buffer_size;
        if model.vertices.len() as u64 * 36 > limit || model.indices.len() as u64 * 4 > limit {
            return Err("glTF model překračuje limity GPU bufferu".into());
        }
        let mesh = Mesh::imported(&self.device, model);
        let id = self.scene.add_imported(
            Vec3::from_array(model.bounds_min),
            Vec3::from_array(model.bounds_max),
        );
        if id == crate::scene::INVALID_ID {
            return Err("glTF model nelze přidat do scény".into());
        }
        self.imported_meshes.insert(id, mesh);
        Ok(id)
    }
    pub fn advance_scene(&mut self, dt: f32) {
        self.upload_bytes = 0;
        self.frame_metrics.record(dt);
        self.scene.update(dt);
        // New meshes are immutable per stable ID. Upload only once; all passes share
        // the same existing imported-mesh path, bounds and cache invalidation.
        for (&id, object) in self.scene.triangle_meshes() {
            self.imported_meshes.entry(id).or_insert_with(|| {
                Mesh::imported(
                    &self.device,
                    &object
                        .mesh
                        .ready_cpu()
                        .expect("Resident buffers installed before rendering")
                        .render_mesh(),
                )
            });
        }
        self.gltf_models
            .retain(|id, _| self.scene.object_exists(*id));
        for (id, gpu) in &mut self.gltf_models {
            if let (Some(model), Some(object)) =
                (self.scene.model(*id), self.scene.objects().get(id))
            {
                let options = object.render_options;
                match self.gltf_gpu.update(
                    &self.queue,
                    gpu,
                    &model.asset,
                    &model.pose,
                    model.revision,
                    object.transform.model(),
                    [
                        options.reflectivity,
                        if options.receives_shadow { 1.0 } else { 0.0 },
                        0.0,
                        0.0,
                    ],
                ) {
                    Ok(bytes) => self.upload_bytes += bytes,
                    Err(error) => {
                        web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&error))
                    }
                }
            }
        }
        self.imported_meshes
            .retain(|id, _| self.scene.object_exists(*id));
        for cache in &mut self.passes {
            cache.imported.retain(|id, _| self.scene.object_exists(*id));
        }
    }
    pub fn import_gltf_model(
        &mut self,
        asset: std::sync::Arc<crate::gltf_model::ModelAsset>,
    ) -> Result<u32, String> {
        // Validate CPU pose before allocating GPU resources or exposing a root ID.
        let instance = crate::gltf_model::ModelInstance::new(asset.clone(), None)?;
        let mut gpu = self.gltf_gpu.upload(&self.device, &self.queue, &asset)?;
        self.gltf_gpu.update(
            &self.queue,
            &mut gpu,
            &asset,
            &instance.pose,
            instance.revision,
            Mat4::IDENTITY,
            [0.0, 1.0, 0.0, 0.0],
        )?;
        let id = self.scene.add_model(asset)?;
        self.gltf_models.insert(id, gpu);
        Ok(id)
    }

    pub fn prepare_view(&mut self, view: RenderView) {
        self.prepare_pass(view, RenderPassKind::Main);
    }
    pub(super) fn prepare_pass(&mut self, view: RenderView, pass: RenderPassKind) {
        self.prepare_pass_cache(view, pass, pass.index());
    }
    pub(super) fn prepare_shadow_pass(
        &mut self,
        view: RenderView,
        pass: RenderPassKind,
        light: usize,
    ) {
        self.prepare_pass_cache(
            view,
            pass,
            2 + light * 2 + usize::from(pass == RenderPassKind::ShadowDynamic),
        );
    }
    fn prepare_pass_cache(&mut self, view: RenderView, pass: RenderPassKind, index: usize) {
        self.active_cache = index;
        let cache = &mut self.passes[index];
        cache.camera_position = view.camera_position;
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
            if pass == RenderPassKind::ShadowStatic {
                self.scene.static_revision()
            } else {
                self.scene.render_revision()
            },
            self.frustum_culling,
        );

        // No additional clock reads or BVH counters when diagnostics are off.
        let clock = (self.debug_enabled && index == 0)
            .then(|| web_sys::window().and_then(|w| w.performance()))
            .flatten();
        if index == 0 && self.debug_enabled {
            self.cull_cached = cache.key == Some(key);
            self.cull_ms = 0.0;
        }
        if cache.key != Some(key) {
            let start = clock.as_ref().map(|p| p.now());
            cache
                .queue
                .extract_pass(&self.scene, camera_matrix, self.frustum_culling, pass);
            if let (Some(clock), Some(start)) = (clock, start) {
                self.cull_ms = clock.now() - start;
            }
            for (index, instances) in cache.queue.batches.iter().enumerate() {
                if !instances
                    .iter()
                    .any(|i| Mat4::from_cols_array_2d(&i.model).determinant() < 0.0)
                {
                    self.upload_bytes +=
                        cache.batches[index].upload(&self.device, &self.queue, instances);
                    self.upload_bytes +=
                        cache.mirrored_batches[index].upload(&self.device, &self.queue, &[]);
                    continue;
                }
                // A negative determinant reverses triangle winding. Keep instancing
                // within each winding group rather than drawing inside-out surfaces.
                let (mirrored, normal): (Vec<_>, Vec<_>) = instances
                    .iter()
                    .copied()
                    .partition(|i| Mat4::from_cols_array_2d(&i.model).determinant() < 0.0);
                self.upload_bytes +=
                    cache.batches[index].upload(&self.device, &self.queue, &normal);
                self.upload_bytes +=
                    cache.mirrored_batches[index].upload(&self.device, &self.queue, &mirrored);
            }
            cache.imported.retain(|id, _| self.scene.object_exists(*id));
            for (id, instances) in &cache.queue.imported {
                let batch = cache
                    .imported
                    .entry(*id)
                    .or_insert_with(|| InstanceBatch::new(&self.device, 1));
                self.upload_bytes += batch.upload(&self.device, &self.queue, instances);
            }
            cache.key = Some(key);
        }
        let mut grid_uniform = build_uniform(camera_matrix, camera_pos, lights);
        // Surface-only preview in the main color pass, never shadows/reflections.
        if pass == RenderPassKind::Main {
            grid_uniform.cutting_guide = self.scene.cutting_guide_uniforms();
        }
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

    pub(super) fn pass_visible(&self) -> usize {
        self.passes[self.active_cache].queue.visible
    }
    pub fn render_stats(&self) -> [f64; 8] {
        [
            self.passes[0].queue.total as f64,
            self.passes[0].queue.visible as f64,
            (self.passes[0].queue.total - self.passes[0].queue.visible) as f64,
            (self.passes[0].queue.draw_calls()
                + self.passes[0]
                    .batches
                    .iter()
                    .zip(&self.passes[0].mirrored_batches)
                    .filter(|(a, b)| !a.uploaded.is_empty() && !b.uploaded.is_empty())
                    .count()
                + self.passes[0]
                    .queue
                    .imported
                    .keys()
                    .filter_map(|id| self.gltf_models.get(id))
                    .map(|m| {
                        super::gltf_gpu::draw_count(m, false) + super::gltf_gpu::draw_count(m, true)
                    })
                    .sum::<usize>()
                - self.passes[0]
                    .queue
                    .imported
                    .keys()
                    .filter(|id| self.gltf_models.contains_key(id))
                    .count()) as f64,
            self.upload_bytes as f64,
            self.prepare_ms,
            self.frame_metrics.fps,
            self.frame_metrics.frame_ms,
        ]
    }

    pub fn set_debug_diagnostics(&mut self, enabled: bool) {
        self.debug_enabled = enabled;
        // Only the main-view HUD reports traversal; auxiliary passes need no counters.
        self.passes[0].queue.collect_stats = enabled;
        if !enabled {
            self.passes[0].queue.culling_stats = Default::default();
            self.cull_ms = 0.0;
            self.cull_cached = true;
        }
    }
    pub fn debug_values(&self) -> serde_json::Value {
        let stats = self.render_stats();
        let bvh = self.passes[0].queue.culling_stats;
        serde_json::json!({
            "fps": stats[6], "frameMs": stats[7], "cpuMs": stats[5],
            "cullMs": self.cull_ms, "cullCached": self.cull_cached,
            "total": stats[0], "visible": stats[1], "culled": stats[2],
            "drawCalls": self.draw_calls, "mainDrawCalls": self.main_draw_calls,
            "bvhNodes": bvh.nodes_visited, "objectTests": bvh.object_tests,
            "staticRebuilds": bvh.static_rebuilds, "dynamicRebuilds": bvh.dynamic_rebuilds,
        })
    }
    pub fn debug_lines(&self) -> Vec<String> {
        let q = &self.passes[0].queue;
        vec![
            format!(
                "FPS {:.1} FRAME {:.2} MS",
                self.frame_metrics.fps, self.frame_metrics.frame_ms
            ),
            if self.cull_cached {
                format!("CPU {:.2} MS CULL CACHED", self.prepare_ms)
            } else {
                format!("CPU {:.2} MS CULL {:.2} MS", self.prepare_ms, self.cull_ms)
            },
            format!(
                "FRUSTUM {}/{} CULLED {}",
                q.visible,
                q.total,
                q.total.saturating_sub(q.visible)
            ),
            format!(
                "BVH {} NODES TESTS {}",
                q.culling_stats.nodes_visited, q.culling_stats.object_tests
            ),
            format!("DRAWS {} MAIN {}", self.draw_calls, self.main_draw_calls),
        ]
    }

    pub fn set_effects(&mut self, shadows: bool, reflections: bool) {
        let effects = self.effects.as_mut().unwrap();
        if effects.shadows != shadows {
            effects.reflection_key = None;
        }
        effects.shadows = shadows;
        effects.reflections = reflections;
    }
    pub fn effects_stats(&self) -> [f64; 19] {
        let e = self.effects.as_ref().unwrap();
        let s = &e.stats;
        [
            s.static_updates as f64,
            s.dynamic_updates as f64,
            s.reflection_updates as f64,
            s.static_visible as f64,
            s.dynamic_visible as f64,
            s.reflection_visible as f64,
            s.passes as f64,
            e.static_shadow[0].size as f64,
            e.reflection.width as f64,
            e.reflection.height as f64,
            s.shadow_light_count as f64,
            s.static_updates_by_light[0] as f64,
            s.static_updates_by_light[1] as f64,
            s.static_updates_by_light[2] as f64,
            s.static_updates_by_light[3] as f64,
            s.dynamic_updates_by_light[0] as f64,
            s.dynamic_updates_by_light[1] as f64,
            s.dynamic_updates_by_light[2] as f64,
            s.dynamic_updates_by_light[3] as f64,
        ]
    }
    /// Auxiliary views have independent culling/buffer caches; simulation advances separately.
    /// Submit each pass before preparing another view, preserving shared uniform write ordering.
    pub fn render_view(
        &mut self,
        view: RenderView,
        target: &RenderTarget<'_>,
    ) -> Result<(), String> {
        target.validate()?;
        let mut effects = self.effects.take().unwrap();
        let result = effects.render(self, view, target);
        self.effects = Some(effects);
        result
    }
    /// Submit a culled depth-only view without updating simulation or running color effects.
    pub fn render_depth_view(
        &mut self,
        view: RenderView,
        target: &DepthRenderTarget<'_>,
        pass: RenderPassKind,
    ) -> Result<(), String> {
        target.validate()?;
        self.prepare_pass(view, pass);
        self.draw_depth(target)
    }
    pub(super) fn draw_depth(&mut self, target: &DepthRenderTarget<'_>) -> Result<(), String> {
        target.validate()?;
        self.shadow_pipelines
            .entry(target.format)
            .or_insert_with(|| {
                pipeline::build_shadow(&self.device, &self.bind_group_layout, target.format)
            });
        let gltf_key = super::gltf_gpu::PipelineKey {
            format: wgpu::TextureFormat::Rgba8Unorm,
            depth_format: target.format,
            reflected: false,
            material_mask: false,
            shadow: true,
        };
        for id in self.passes[self.active_cache].queue.imported.keys() {
            if let Some(model) = self.gltf_models.get(id) {
                self.gltf_gpu
                    .prepare_pipeline(&self.device, gltf_key, model);
            }
        }
        let pipeline = &self.shadow_pipelines[&target.format];
        let cache = &self.passes[self.active_cache];
        let mut draws = 0;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("shadow encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow casters"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: target.load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_viewport(
                target.viewport.x as f32,
                target.viewport.y as f32,
                target.viewport.width as f32,
                target.viewport.height as f32,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(
                target.viewport.x,
                target.viewport.y,
                target.viewport.width,
                target.viewport.height,
            );
            pass.set_bind_group(0, &self.grid_bind_group, &[]);
            for (index, kind) in [MeshKind::Cube, MeshKind::Plane, MeshKind::Sphere]
                .into_iter()
                .enumerate()
            {
                let mesh = self.mesh(kind);
                for batch in [&cache.batches[index], &cache.mirrored_batches[index]] {
                    let count = batch.uploaded.len() as u32;
                    if count == 0 {
                        continue;
                    }
                    pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    pass.set_vertex_buffer(1, batch.buffer.slice(..));
                    pass.set_index_buffer(mesh.index_buffer.slice(..), mesh.index_format);
                    pass.draw_indexed(0..mesh.index_count, 0, 0..count);
                    draws += 1;
                }
            }
            for (id, instances) in &cache.queue.imported {
                if instances.is_empty() {
                    continue;
                }
                let Some(mesh) = self.imported_meshes.get(id) else {
                    continue;
                };
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_vertex_buffer(1, cache.imported[id].buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), mesh.index_format);
                pass.draw_indexed(0..mesh.index_count, 0, 0..instances.len() as u32);
                draws += 1;
            }
            for id in cache.queue.imported.keys() {
                if let Some(model) = self.gltf_models.get(id) {
                    draws += self.gltf_gpu.draw(
                        &mut pass,
                        model,
                        gltf_key,
                        false,
                        cache.camera_position,
                    );
                }
            }
        }
        self.queue.submit(Some(encoder.finish()));
        self.draw_calls += draws;
        Ok(())
    }
    /// Draw developer overlays after postprocessing; exclude them from reflection/depth inputs.
    pub fn draw_helpers(&mut self, target: &RenderTarget<'_>) {
        if !self.draw_grid && !self.draw_light_helpers {
            return;
        }
        let key = (target.color_format, target.depth_format);
        self.helper_pipelines.entry(key).or_insert_with(|| {
            pipeline::build_lines(&self.device, key.0, key.1, &self.bind_group_layout)
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("postprocess helpers"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("developer overlays"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.color,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.helper_pipelines[&key]);
            pass.set_bind_group(0, &self.grid_bind_group, &[]);
            pass.set_viewport(
                target.viewport.x as f32,
                target.viewport.y as f32,
                target.viewport.width as f32,
                target.viewport.height as f32,
                0.0,
                1.0,
            );
            if self.draw_grid {
                pass.set_vertex_buffer(0, self.grid_vertex_buffer.slice(..));
                pass.draw(0..self.grid_vertex_count, 0..1);
                self.draw_calls += 1;
                self.main_draw_calls += 1;
            }
            if self.draw_light_helpers {
                pass.set_vertex_buffer(0, self.light_vertex_buffer.slice(..));
                pass.draw(0..self.light_vertex_count, 0..1);
                self.draw_calls += 1;
                self.main_draw_calls += 1;
            }
        }
        self.queue.submit(Some(encoder.finish()));
    }
    pub(super) fn draw_color(
        &mut self,
        target: &RenderTarget<'_>,
        effects: &Effects,
        reflected: bool,
    ) -> Result<(), String> {
        target.validate()?;
        let mut draws = 0;
        let key = (
            target.color_format,
            target.depth_format,
            self.passes[self.active_cache]
                .key
                .is_some_and(|k| k.0.determinant() < 0.0),
            target.auxiliary.is_some(),
        );
        self.pipelines.entry(key).or_insert_with(|| {
            (
                pipeline::build(
                    &self.device,
                    key.0,
                    key.1,
                    &self.bind_group_layout,
                    &effects.layout,
                    key.2,
                    key.3,
                ),
                pipeline::build_lines(&self.device, key.0, key.1, &self.bind_group_layout),
                pipeline::build(
                    &self.device,
                    key.0,
                    key.1,
                    &self.bind_group_layout,
                    &effects.layout,
                    !key.2,
                    key.3,
                ),
            )
        });
        let gltf_key = super::gltf_gpu::PipelineKey {
            format: target.color_format,
            depth_format: target.depth_format,
            reflected: self.passes[self.active_cache]
                .key
                .is_some_and(|k| k.0.determinant() < 0.0),
            material_mask: target.auxiliary.is_some(),
            shadow: false,
        };
        for id in self.passes[self.active_cache].queue.imported.keys() {
            if let Some(model) = self.gltf_models.get(id) {
                self.gltf_gpu
                    .prepare_pipeline(&self.device, gltf_key, model);
            }
        }
        // Immutable lookup ends the map mutation before borrowing mesh resources.
        let pipelines = &self.pipelines[&key];
        let cache = &self.passes[self.active_cache];
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });
        let mut attachments = vec![Some(wgpu::RenderPassColorAttachment {
            view: target.color,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: target.color_load,
                store: wgpu::StoreOp::Store,
            },
        })];
        if let Some(auxiliary) = target.auxiliary {
            attachments.push(Some(wgpu::RenderPassColorAttachment {
                view: auxiliary,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: match target.depth_load {
                        wgpu::LoadOp::Load => wgpu::LoadOp::Load,
                        wgpu::LoadOp::Clear(_) => wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        wgpu::LoadOp::DontCare(v) => wgpu::LoadOp::DontCare(v),
                    },
                    store: wgpu::StoreOp::Store,
                },
            }));
        }
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render"),
                color_attachments: &attachments,
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
            rp.set_bind_group(
                1,
                if reflected {
                    &effects.reflection_group
                } else {
                    &effects.scene_group
                },
                &[],
            );
            rp.set_bind_group(0, &self.grid_bind_group, &[]);
            for (index, kind) in [MeshKind::Cube, MeshKind::Plane, MeshKind::Sphere]
                .into_iter()
                .enumerate()
            {
                let mesh = self.mesh(kind);
                for (batch, pipeline) in [
                    (&cache.batches[index], &pipelines.0),
                    (&cache.mirrored_batches[index], &pipelines.2),
                ] {
                    let count = batch.uploaded.len() as u32;
                    if count == 0 {
                        continue;
                    }
                    rp.set_pipeline(pipeline);
                    rp.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    rp.set_vertex_buffer(1, batch.buffer.slice(..));
                    rp.set_index_buffer(mesh.index_buffer.slice(..), mesh.index_format);
                    rp.draw_indexed(0..mesh.index_count, 0, 0..count);
                    draws += 1;
                }
            }
            for (id, instances) in &cache.queue.imported {
                if instances.is_empty() {
                    continue;
                }
                let Some(mesh) = self.imported_meshes.get(id) else {
                    continue;
                };
                rp.set_pipeline(
                    if Mat4::from_cols_array_2d(&instances[0].model).determinant() < 0.0 {
                        &pipelines.2
                    } else {
                        &pipelines.0
                    },
                );
                rp.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rp.set_vertex_buffer(1, cache.imported[id].buffer.slice(..));
                rp.set_index_buffer(mesh.index_buffer.slice(..), mesh.index_format);
                rp.draw_indexed(0..mesh.index_count, 0, 0..instances.len() as u32);
                draws += 1;
            }
            for id in cache.queue.imported.keys() {
                if let Some(model) = self.gltf_models.get(id) {
                    draws +=
                        self.gltf_gpu
                            .draw(&mut rp, model, gltf_key, false, cache.camera_position);
                }
            }
            if reflected || target.auxiliary.is_none() {
                draws += self.gltf_gpu.draw_many(
                    &mut rp,
                    cache
                        .queue
                        .imported
                        .keys()
                        .filter_map(|id| self.gltf_models.get(id)),
                    gltf_key,
                    true,
                    cache.camera_position,
                );
            }
            if !reflected
                && target.auxiliary.is_none()
                && (self.draw_grid || self.draw_light_helpers)
            {
                rp.set_pipeline(&pipelines.1);
                rp.set_bind_group(0, &self.grid_bind_group, &[]);
                if target.auxiliary.is_none() && self.draw_grid {
                    rp.set_vertex_buffer(0, self.grid_vertex_buffer.slice(..));
                    rp.draw(0..self.grid_vertex_count, 0..1);
                    draws += 1;
                }
                if target.auxiliary.is_none() && self.draw_light_helpers {
                    rp.set_vertex_buffer(0, self.light_vertex_buffer.slice(..));
                    rp.draw(0..self.light_vertex_count, 0..1);
                    draws += 1;
                }
            }
        }
        self.queue.submit(Some(encoder.finish()));
        self.draw_calls += draws;
        if !reflected {
            self.main_draw_calls += draws;
        }
        Ok(())
    }
    /// Transparent materials use opaque depth after screen-space effects.
    pub fn draw_transparent(&mut self, target: &RenderTarget<'_>) -> Result<(), String> {
        let cache = &self.passes[0];
        if !cache
            .queue
            .imported
            .keys()
            .filter_map(|id| self.gltf_models.get(id))
            .any(|m| super::gltf_gpu::draw_count(m, true) > 0)
        {
            return Ok(());
        }
        let key = super::gltf_gpu::PipelineKey {
            format: target.color_format,
            depth_format: target.depth_format,
            reflected: cache.key.is_some_and(|k| k.0.determinant() < 0.0),
            material_mask: false,
            shadow: false,
        };
        for id in cache.queue.imported.keys() {
            if let Some(model) = self.gltf_models.get(id) {
                self.gltf_gpu.prepare_pipeline(&self.device, key, model);
            }
        }
        let effects = self.effects.as_ref().unwrap();
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("glTF transparent overlay"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("glTF transparent overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.color,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(
                target.viewport.x as f32,
                target.viewport.y as f32,
                target.viewport.width as f32,
                target.viewport.height as f32,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(
                target.viewport.x,
                target.viewport.y,
                target.viewport.width,
                target.viewport.height,
            );
            pass.set_bind_group(0, &self.grid_bind_group, &[]);
            pass.set_bind_group(1, &effects.scene_group, &[]);
            let draws = self.gltf_gpu.draw_many(
                &mut pass,
                cache
                    .queue
                    .imported
                    .keys()
                    .filter_map(|id| self.gltf_models.get(id)),
                key,
                true,
                cache.camera_position,
            );
            self.draw_calls += draws;
            self.main_draw_calls += draws;
        }
        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }
}
