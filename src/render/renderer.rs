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

// Existing reflection, shadow and environment indices remain stable. XR main
// caches are appended only when their logical view slot is first rendered.
const BASE_PASS_CACHE_COUNT: usize = 3 + MAX_LIGHTS * 2;

type ColorPipelineKey = (wgpu::TextureFormat, wgpu::TextureFormat, bool, bool, bool);
type ColorPipelines = (
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
    wgpu::RenderPipeline,
);

struct InstanceBatch {
    buffer: wgpu::Buffer,
    capacity: usize,
    // Always describes this batch's private buffer, even while drawing an alias.
    uploaded: Vec<InstanceData>,
    draw_alias: Option<wgpu::Buffer>,
    alias_instances: Vec<InstanceData>,
    label: &'static str,
}
impl InstanceBatch {
    fn new(device: &wgpu::Device, capacity: usize, label: &'static str) -> Self {
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: (capacity * std::mem::size_of::<InstanceData>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
            uploaded: Vec::new(),
            draw_alias: None,
            alias_instances: Vec::new(),
            label,
        }
    }
    fn instances(&self) -> &[InstanceData] {
        if self.draw_alias.is_some() {
            &self.alias_instances
        } else {
            &self.uploaded
        }
    }
    fn draw_buffer(&self) -> &wgpu::Buffer {
        self.draw_alias.as_ref().unwrap_or(&self.buffer)
    }
    fn upload_shared(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[InstanceData],
        left: Option<&Self>,
    ) -> usize {
        // Only the right main view receives a left source. The source never
        // aliases another buffer, so its uploaded data describes its GPU bytes.
        if self.uploaded != instances && !instances.is_empty() {
            if let Some(left) = left.filter(|left| left.uploaded == instances) {
                self.draw_alias = Some(left.buffer.clone());
                if self.alias_instances != instances {
                    self.alias_instances.clear();
                    self.alias_instances.extend_from_slice(instances);
                }
                return 0;
            }
        }
        self.draw_alias = None;
        self.alias_instances.clear();
        self.upload(device, queue, instances)
    }
    fn refresh_alias(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        left: Option<&Self>,
    ) -> usize {
        if self.draw_alias.is_none() {
            return 0;
        }
        if let Some(left) = left.filter(|left| left.uploaded == self.alias_instances) {
            // Refresh the handle too: left may have replaced its private buffer.
            self.draw_alias = Some(left.buffer.clone());
            return 0;
        }
        // A left-only camera change can invalidate an alias while the right
        // visibility key still hits. Restore its saved payload before drawing.
        self.draw_alias = None;
        let mut instances = std::mem::take(&mut self.alias_instances);
        let bytes = self.upload(device, queue, &instances);
        instances.clear();
        self.alias_instances = instances;
        bytes
    }
    fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[InstanceData],
    ) -> usize {
        if instances.len() > self.capacity {
            *self = Self::new(device, instances.len().next_power_of_two(), self.label);
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

/// Imported meshes share bounded upload pages; each draw binds its own byte range.
/// glTF uses separate GPU pose buffers and never needs a page entry here.
struct ImportedInstances {
    pages: Vec<InstanceBatch>,
    ranges: HashMap<u32, (usize, std::ops::Range<u64>)>,
    scratch: Vec<InstanceData>,
}
impl ImportedInstances {
    fn new() -> Self {
        Self {
            pages: Vec::new(),
            ranges: HashMap::new(),
            scratch: Vec::new(),
        }
    }
    fn flush(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        page: usize,
        left: Option<&Self>,
    ) -> usize {
        if self.pages.len() <= page {
            self.pages
                .push(InstanceBatch::new(device, 1, "imported instance page"));
        }
        self.pages[page].upload_shared(
            device,
            queue,
            &self.scratch,
            left.and_then(|left| left.pages.get(page)),
        )
    }
    fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        imported: &std::collections::BTreeMap<u32, Vec<InstanceData>>,
        meshes: &HashMap<u32, Mesh>,
        left: Option<&Self>,
    ) -> usize {
        self.scratch.clear();
        let mut page = 0;
        let mut uploaded = 0;
        let stride = std::mem::size_of::<InstanceData>();
        // A large single-mesh batch stays contiguous; unrelated meshes use pages
        // rather than forcing one allocation to grow with the whole scene.
        let page_limit = 4096.min(device.limits().max_buffer_size as usize / stride);
        for (id, instances) in imported {
            if instances.is_empty() || !meshes.contains_key(id) {
                continue;
            }
            if !self.scratch.is_empty()
                && self.scratch.len().saturating_add(instances.len()) > page_limit
            {
                uploaded += self.flush(device, queue, page, left);
                page += 1;
                self.scratch.clear();
            }
            let start = self.scratch.len() * stride;
            self.scratch.extend_from_slice(instances);
            let end = self.scratch.len() * stride;
            self.ranges.insert(*id, (page, start as u64..end as u64));
        }
        if !self.scratch.is_empty() {
            uploaded += self.flush(device, queue, page, left);
        }
        uploaded
    }
    fn retain(&mut self, scene: &SceneManager) {
        self.ranges.retain(|id, _| scene.mesh_exists(*id));
        let pages = self
            .ranges
            .values()
            .map(|(page, _)| page + 1)
            .max()
            .unwrap_or(0);
        self.pages.truncate(pages);
        if pages == 0 {
            self.scratch = Vec::new();
        }
    }
    fn slice(&self, id: &u32) -> wgpu::BufferSlice<'_> {
        let (page, range) = &self.ranges[id];
        self.pages[*page].draw_buffer().slice(range.clone())
    }
}

struct VariantColors {
    _texture: wgpu::Texture,
    group: wgpu::BindGroup,
    bytes: usize,
}
impl VariantColors {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        uniform: &wgpu::Buffer,
        colors: &[[f32; 4]],
    ) -> Self {
        let width = colors
            .len()
            .min(device.limits().max_texture_dimension_2d as usize)
            .max(1) as u32;
        let height = colors.len().div_ceil(width as usize).max(1) as u32;
        let mut pixels = vec![[0.0; 4]; width as usize * height as usize];
        pixels[..colors.len()].copy_from_slice(colors);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shared triangle variant colors"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data::as_bytes(&pixels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 16),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene and shared variant colors"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });
        Self {
            _texture: texture,
            group,
            bytes: pixels.len() * 16,
        }
    }
}

struct AlphaIndices {
    threshold: f32,
    buffer: Option<wgpu::Buffer>,
    count: u32,
}

struct Mesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    index_format: wgpu::IndexFormat,
    panel_dynamic: bool,
    panel_indices: Vec<u32>,
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
            panel_dynamic: false,
            panel_indices: Vec::new(),
        }
    }
    fn panel_buffer(device: &wgpu::Device, bytes: u64, vertex: bool) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(if vertex {
                "panel bitmap vertices"
            } else {
                "panel bitmap indices"
            }),
            size: bytes,
            usage: (if vertex {
                wgpu::BufferUsages::VERTEX
            } else {
                wgpu::BufferUsages::INDEX
            }) | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
    fn update_panel(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        model: &crate::gltf_loader::ImportedMesh,
        stats: &mut PanelMeshStats,
    ) {
        let _scope = super::profiler::cpu_scope("panel.upload");
        let vertices: Vec<Vertex> = model
            .vertices
            .iter()
            .map(|v| Vertex {
                position: v.position,
                color: v.color,
                normal: v.normal,
            })
            .collect();
        for (buffer, bytes, vertex) in [
            (&mut self.vertex_buffer, data::as_bytes(&vertices), true),
            (
                &mut self.index_buffer,
                data::as_bytes(&model.indices),
                false,
            ),
        ] {
            let old = buffer.size();
            let capacity = crate::info_panel::buffer_capacity(
                bytes.len() as u64,
                old,
                device.limits().max_buffer_size,
            );
            if capacity != old {
                *buffer = Self::panel_buffer(device, capacity, vertex);
                stats.buffers_created += 1;
                stats.created_bytes += capacity;
                stats.replacements += 1;
                stats.growths += u64::from(capacity > old);
                stats.shrinks += u64::from(capacity < old);
            }
            // Only reuse bytes proven equal to this buffer's last publication.
            // A smaller draw may use an unchanged prefix; buffer replacement
            // invalidates it even when the CPU prefix still matches.
            if !vertex && capacity == old && self.panel_indices.starts_with(&model.indices) {
                stats.index_upload_reuses += 1;
                continue;
            }
            if !bytes.is_empty() {
                queue.write_buffer(buffer, 0, bytes);
                stats.write_calls += 1;
                stats.write_bytes += bytes.len() as u64;
            }
            if !vertex {
                self.panel_indices = model.indices.clone();
            }
        }
        self.index_count = model.indices.len() as u32;
    }
    fn panel(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        model: &crate::gltf_loader::ImportedMesh,
        stats: &mut PanelMeshStats,
    ) -> Self {
        let v = crate::info_panel::buffer_capacity(
            (model.vertices.len() * 36) as u64,
            0,
            device.limits().max_buffer_size,
        );
        let i = crate::info_panel::buffer_capacity(
            (model.indices.len() * 4) as u64,
            0,
            device.limits().max_buffer_size,
        );
        let mut mesh = Self {
            vertex_buffer: Self::panel_buffer(device, v, true),
            index_buffer: Self::panel_buffer(device, i, false),
            index_count: 0,
            index_format: wgpu::IndexFormat::Uint32,
            panel_dynamic: true,
            panel_indices: Vec::new(),
        };
        stats.buffers_created += 2;
        stats.created_bytes += v + i;
        mesh.update_panel(device, queue, model, stats);
        mesh
    }
    fn imported(device: &wgpu::Device, model: &crate::gltf_loader::ImportedMesh) -> Self {
        Self::imported_labeled(device, model, "glTF vertices", "glTF u32 indices")
    }
    fn imported_labeled(
        device: &wgpu::Device,
        model: &crate::gltf_loader::ImportedMesh,
        vertex_label: &str,
        index_label: &str,
    ) -> Self {
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
            label: Some(vertex_label),
            contents: data::as_bytes(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(index_label),
            contents: data::as_bytes(&model.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            vertex_buffer,
            index_buffer,
            index_count: model.indices.len() as u32,
            index_format: wgpu::IndexFormat::Uint32,
            panel_dynamic: false,
            panel_indices: Vec::new(),
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
        appearance: Default::default(),
    }
}

struct PassCache {
    camera_position: Vec3,
    light_class: crate::render_lights::LightClass,
    queue: RenderQueue,
    key: Option<(Mat4, u64, bool)>,
    batches: [InstanceBatch; 3],
    mirrored_batches: [InstanceBatch; 3],
    imported: ImportedInstances,
}
impl PassCache {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            queue: RenderQueue::default(),
            camera_position: Vec3::ZERO,
            light_class: Default::default(),
            key: None,
            batches: std::array::from_fn(|_| {
                InstanceBatch::new(device, 1, "primitive instance buffer")
            }),
            mirrored_batches: std::array::from_fn(|_| {
                InstanceBatch::new(device, 1, "primitive instance buffer")
            }),
            imported: ImportedInstances::new(),
        }
    }
}

#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlaySelectionStats {
    calls: u64,
    diagnostic_calls: u64,
    #[serde(flatten)]
    gltf: super::gltf_gpu::BlendSelectionStats,
    mesh_batches_visited: u64,
    mesh_imported_visited: u64,
    mesh_instances_visited: u64,
    neon_preparation_calls: u64,
    ring_preparation_calls: u64,
    with_gltf: u64,
    with_mesh: u64,
    with_neon: u64,
    with_rings: u64,
    with_helpers: u64,
    stage_ms: [f64; 4],
}

#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PanelMeshStats {
    updates: u64,
    buffers_created: u64,
    created_bytes: u64,
    replacements: u64,
    growths: u64,
    shrinks: u64,
    write_calls: u64,
    index_upload_reuses: u64,
    write_bytes: u64,
    initial_payload_bytes: u64,
    by_kind: std::collections::BTreeMap<String, u64>,
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
    cpu_mesh_sources: HashMap<u32, crate::cutting_mesh::MeshData>,
    alpha_indices: HashMap<u32, AlphaIndices>,
    translucent_indices: std::cell::Cell<usize>,
    translucent_full_indices: std::cell::Cell<usize>,
    translucent_draws: std::cell::Cell<usize>,
    panel_buffer_reuse: bool,
    panel_mesh_stats: PanelMeshStats,
    overlay_selection_reference: bool,
    overlay_selection: OverlaySelectionStats,
    neon_glow: Option<super::neon_glow::NeonGlow>,
    neon_meshes: HashMap<u32, Option<super::neon_glow::GlowMesh>>,
    neon_draws: usize,
    neon_segments: usize,
    gltf_gpu: super::gltf_gpu::GltfGpu,
    cut_feedback_gpu: Option<super::cut_feedback_gpu::CutFeedbackGpu>,
    gltf_models: HashMap<u32, super::gltf_gpu::GpuModel>,
    grid_uniform_buffer: wgpu::Buffer,
    grid_bind_group: wgpu::BindGroup,
    _default_colors: VariantColors,
    variant_colors: HashMap<u32, VariantColors>,
    pub scene: SceneManager,
    pub appearance: crate::render_appearance::Appearance,
    passes: Vec<PassCache>,
    active_cache: usize,
    main_view_slot: u32,
    last_main_cache: usize,
    main_cache_hits: u64,
    main_cache_misses: u64,
    main_instance_upload_bytes: u64,
    translucent_pipelines:
        HashMap<(wgpu::TextureFormat, wgpu::TextureFormat), [wgpu::RenderPipeline; 2]>,
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
    pub fn set_appearance(&mut self, value: crate::render_appearance::Appearance) {
        if self.appearance != value {
            self.appearance = value;
            if let Some(effects) = self.effects.as_mut() {
                effects.reflection_key = None;
            }
        }
    }
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let cube_mesh = Mesh::new(&device, "cube", data::VERTICES, data::INDICES);
        let (plane_vertices, plane_indices) = data::plane_mesh();
        let plane_mesh = Mesh::new(&device, "plane", &plane_vertices, &plane_indices);
        let (sphere_vertices, sphere_indices) = data::sphere_mesh(24, 16);
        let sphere_mesh = Mesh::new(&device, "sphere", &sphere_vertices, &sphere_indices);

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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
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
        let default_colors = VariantColors::new(
            &device,
            &queue,
            &bind_group_layout,
            &grid_uniform_buffer,
            &[[1.0; 4]],
        );
        let grid_bind_group = default_colors.group.clone();
        let passes = (0..BASE_PASS_CACHE_COUNT)
            .map(|_| PassCache::new(&device))
            .collect();
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
            cpu_mesh_sources: HashMap::new(),
            alpha_indices: HashMap::new(),
            translucent_indices: std::cell::Cell::new(0),
            translucent_full_indices: std::cell::Cell::new(0),
            translucent_draws: std::cell::Cell::new(0),
            // The cheaper eligibility query remains an explicit diagnostic
            // experiment: repeated stress pairs did not improve frame cadence.
            panel_buffer_reuse: false,
            panel_mesh_stats: PanelMeshStats::default(),
            overlay_selection_reference: true,
            overlay_selection: OverlaySelectionStats::default(),
            neon_glow: None,
            neon_meshes: HashMap::new(),
            neon_draws: 0,
            neon_segments: 0,
            gltf_gpu,
            cut_feedback_gpu: None,
            gltf_models: HashMap::new(),
            grid_uniform_buffer,
            grid_bind_group,
            _default_colors: default_colors,
            variant_colors: HashMap::new(),
            scene: SceneManager::default(),
            appearance: Default::default(),
            passes,
            active_cache: 0,
            main_view_slot: 0,
            last_main_cache: 0,
            main_cache_hits: 0,
            main_cache_misses: 0,
            main_instance_upload_bytes: 0,
            shadow_pipelines: HashMap::new(),
            translucent_pipelines: HashMap::new(),
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
        self.cpu_mesh_sources.remove(&id);
        self.alpha_indices.remove(&id);
        self.imported_meshes.insert(
            id,
            Mesh {
                vertex_buffer: piece.render_vertices,
                index_buffer: piece.indices,
                index_count: piece.vertex_count,
                index_format: wgpu::IndexFormat::Uint32,
                panel_dynamic: false,
                panel_indices: Vec::new(),
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
    /// Upload new shared numeric or general-font glyph assets before any pass.
    pub fn sync_numeric_font_meshes(&mut self) {
        self.scene.sync_numeric_font();
        self.scene.sync_font3d();
        if self
            .scene
            .numeric_font_assets()
            .chain(self.scene.font3d_assets())
            .any(|id| !self.variant_colors.contains_key(id))
        {
            self.sync_triangle_meshes();
        }
    }
    /// Refresh only opt-in dynamic text meshes, before any render pass uses them.
    fn refresh_info_panel_meshes(&mut self) {
        self.sync_numeric_font_meshes();
        for (&id, panel) in &mut self.scene.info_panels {
            if panel.numeric() {
                panel.dirty = false;
                continue;
            }
            if panel.dirty {
                let _scope = super::profiler::cpu_scope("panel.prepare");
                let upload_before =
                    self.panel_mesh_stats.write_bytes + self.panel_mesh_stats.initial_payload_bytes;
                self.panel_mesh_stats.updates += 1;
                *self
                    .panel_mesh_stats
                    .by_kind
                    .entry(format!("{:?}", panel.kind))
                    .or_default() += 1;
                if self.panel_buffer_reuse && panel.kind != crate::info_panel::PanelKind::Text {
                    if let Some(mesh) = self
                        .imported_meshes
                        .get_mut(&id)
                        .filter(|mesh| mesh.panel_dynamic)
                    {
                        mesh.update_panel(
                            &self.device,
                            &self.queue,
                            &panel.mesh,
                            &mut self.panel_mesh_stats,
                        );
                    } else {
                        self.imported_meshes.insert(
                            id,
                            Mesh::panel(
                                &self.device,
                                &self.queue,
                                &panel.mesh,
                                &mut self.panel_mesh_stats,
                            ),
                        );
                    }
                } else {
                    // Independent original allocation/upload path, also used by the diagnostic baseline.
                    let mesh = Mesh::imported_labeled(
                        &self.device,
                        &panel.mesh,
                        "panel bitmap vertices (reference)",
                        "panel bitmap indices (reference)",
                    );
                    self.panel_mesh_stats.buffers_created += 2;
                    self.panel_mesh_stats.created_bytes +=
                        mesh.vertex_buffer.size() + mesh.index_buffer.size();
                    self.panel_mesh_stats.initial_payload_bytes +=
                        (panel.mesh.vertices.len() * 36 + panel.mesh.indices.len() * 4) as u64;
                    self.imported_meshes.insert(id, mesh);
                }
                self.upload_bytes += (self.panel_mesh_stats.write_bytes
                    + self.panel_mesh_stats.initial_payload_bytes
                    - upload_before) as usize;
                if panel.kind.neon() {
                    if self.neon_glow.as_ref().is_some_and(|glow| glow.enabled()) {
                        self.neon_meshes.insert(
                            id,
                            super::neon_glow::GlowMesh::new(&self.device, &panel.mesh),
                        );
                    } else {
                        self.neon_meshes.remove(&id);
                    }
                }
                panel.dirty = false;
            }
        }
    }
    pub fn advance_scene(&mut self, dt: f32) {
        self.advance_scene_with_elapsed(dt, dt);
    }
    pub fn advance_scene_with_elapsed(&mut self, elapsed: f32, simulation_dt: f32) {
        self.advance_scene_timed(elapsed, simulation_dt);
        self.finish_scene_updates();
    }
    pub fn advance_xr_scene(&mut self, elapsed: f32, time_scale: f32) {
        // Tracked paddle physics needs the current frame hand poses.
        let table_tennis = self.scene.table_tennis.take();
        self.advance_scene_timed(elapsed, elapsed.min(0.1) * time_scale);
        self.scene.table_tennis = table_tennis;
    }
    fn advance_scene_timed(&mut self, elapsed: f32, simulation_dt: f32) {
        let _cpu_scope = super::profiler::cpu_scope("simulation");
        self.upload_bytes = 0;
        self.gltf_gpu.begin_frame();
        self.frame_metrics.record(elapsed);
        self.scene.update(simulation_dt);
    }
    /// Publish all CPU scene mutations once, before rendering either XR eye.
    /// Does not advance simulation or reset the current frame metrics.
    pub fn finish_scene_updates(&mut self) {
        let _cpu_scope = super::profiler::cpu_scope("scene.gpuUpdates");
        self.sync_triangle_meshes();
        let removed: std::collections::HashSet<_> = self
            .gltf_models
            .iter()
            .filter(|(id, _)| !self.scene.mesh_exists(**id))
            .map(|(_, m)| super::gltf_gpu::GltfGpu::model_generation(m))
            .collect();
        self.gltf_models.retain(|id, _| self.scene.mesh_exists(*id));
        if !removed.is_empty() {
            if self.gltf_models.is_empty() {
                self.gltf_gpu.clear_instance_pages();
            } else {
                self.gltf_gpu.models_removed(&removed);
            }
        }
        for (id, gpu) in &mut self.gltf_models {
            if let (Some(model), Some(object)) =
                (self.scene.model(*id), self.scene.objects().get(id))
            {
                let options = object.render_options;
                // Semantic queue policy is not shader data and must update even
                // when pose/root uniforms remain unchanged.
                gpu.opaque_reorderable = options.opaque_reorderable;
                match self.gltf_gpu.update(
                    &self.queue,
                    gpu,
                    &model.asset,
                    &model.render_bounds_cache,
                    &model.pose,
                    model.revision,
                    object.render_data.model(),
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
            .retain(|id, _| self.scene.mesh_exists(*id));
        self.cpu_mesh_sources.retain(|id, _| {
            self.scene.triangle_meshes().contains_key(id)
                || self.scene.mesh_variants.contains_key(id)
        });
        self.alpha_indices
            .retain(|id, _| self.cpu_mesh_sources.contains_key(id));
        self.neon_meshes.retain(|id, _| self.scene.mesh_exists(*id));
        self.variant_colors
            .retain(|id, _| self.scene.mesh_variants.contains_key(id));
        if self.scene.objects().is_empty() && self.passes.iter().any(|cache| cache.queue.total != 0)
        {
            self.passes.truncate(BASE_PASS_CACHE_COUNT);
            self.active_cache = 0;
            self.last_main_cache = 0;
            for cache in &mut self.passes {
                cache.key = None;
                cache.queue = RenderQueue::default();
            }
            self.passes[0].queue.collect_stats = self.debug_enabled;
        }
        for cache in &mut self.passes {
            cache.imported.retain(&self.scene);
        }
    }
    /// Cuts can create meshes after simulation, before either XR eye is drawn.
    /// Upload them without advancing time or resetting frame metrics.
    pub fn triangle_mesh_uploaded(&self, id: u32) -> bool {
        self.imported_meshes.contains_key(&id)
    }
    pub fn sync_triangle_meshes(&mut self) {
        let _cpu_scope = super::profiler::cpu_scope("mesh.sync");
        self.scene.sync_numeric_font();
        self.scene.sync_font3d();
        // New meshes are immutable per stable ID. Upload only once; all passes share
        // the same existing imported-mesh path, bounds and cache invalidation.
        for (&handle, asset) in &self.scene.mesh_variants {
            if self.variant_colors.contains_key(&handle) {
                continue;
            }
            let source = &asset.variants[0];
            // Numeric tubes use canonical render-only geometry and a per-fragment
            // neon material; their cutting recipe stays independent of shading.
            let mesh = self
                .scene
                .numeric_font_digit(handle)
                .map_or_else(|| asset.gpu_mesh(), crate::info_panel::numeric_glyph);
            self.imported_meshes
                .insert(handle, Mesh::imported(&self.device, &mesh));
            self.cpu_mesh_sources.insert(handle, source.clone());
            self.variant_colors.insert(
                handle,
                VariantColors::new(
                    &self.device,
                    &self.queue,
                    &self.bind_group_layout,
                    &self.grid_uniform_buffer,
                    &asset.gpu_colors(),
                ),
            );
        }
        for (&id, object) in self.scene.triangle_meshes() {
            if self.scene.variant_instances.contains_key(&id) {
                continue;
            }
            // GPU-resident topology is not the CPU recipe's vertex ordering.
            if object.mesh.is_resident() {
                continue;
            }
            let unchanged = self
                .cpu_mesh_sources
                .get(&id)
                .is_some_and(|source| source.same_source(&object.mesh));
            if unchanged && self.imported_meshes.contains_key(&id) {
                continue;
            }
            let Some(cpu) = object.mesh.ready_cpu() else {
                continue;
            };
            let _upload_scope = super::profiler::cpu_scope("mesh.upload");
            let render_mesh = cpu.render_mesh();
            self.upload_bytes += render_mesh.vertices.len() * 36 + render_mesh.indices.len() * 4;
            self.imported_meshes
                .insert(id, Mesh::imported(&self.device, &render_mesh));
            self.cpu_mesh_sources.insert(id, object.mesh.clone());
            self.alpha_indices.remove(&id);
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
            &instance.render_bounds_cache,
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
        self.main_view_slot = 0;
        self.prepare_pass(view, RenderPassKind::Main);
    }
    pub(super) fn prepare_pass(&mut self, view: RenderView, pass: RenderPassKind) {
        let index = if pass == RenderPassKind::Main {
            let index = if self.main_view_slot == 0 {
                0
            } else {
                BASE_PASS_CACHE_COUNT + self.main_view_slot as usize - 1
            };
            while self.passes.len() <= index {
                let mut cache = PassCache::new(&self.device);
                cache.queue.collect_stats = self.debug_enabled;
                self.passes.push(cache);
            }
            self.last_main_cache = index;
            index
        } else if pass == RenderPassKind::Environment {
            2 + MAX_LIGHTS * 2
        } else {
            pass.index()
        };
        self.prepare_pass_cache(view, pass, index);
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
        let _cpu_scope = super::profiler::cpu_scope("scene.prepare");
        self.active_cache = index;
        let is_main = pass == RenderPassKind::Main;
        let instance_upload_start = self.upload_bytes;
        let (cache, left) = if is_main && index == BASE_PASS_CACHE_COUNT + 1 {
            let (earlier, right) = self.passes.split_at_mut(index);
            (&mut right[0], Some(&earlier[BASE_PASS_CACHE_COUNT]))
        } else {
            (&mut self.passes[index], None)
        };
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
        let clock = (self.debug_enabled && is_main)
            .then(|| web_sys::window().and_then(|w| w.performance()))
            .flatten();
        if is_main {
            if cache.key == Some(key) {
                self.main_cache_hits += 1;
            } else {
                self.main_cache_misses += 1;
            }
        }
        if is_main && self.debug_enabled {
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
                    self.upload_bytes += cache.batches[index].upload_shared(
                        &self.device,
                        &self.queue,
                        instances,
                        left.map(|left| &left.batches[index]),
                    );
                    self.upload_bytes += cache.mirrored_batches[index].upload_shared(
                        &self.device,
                        &self.queue,
                        &[],
                        left.map(|left| &left.mirrored_batches[index]),
                    );
                    continue;
                }
                // A negative determinant reverses triangle winding. Keep instancing
                // within each winding group rather than drawing inside-out surfaces.
                let (mirrored, normal): (Vec<_>, Vec<_>) = instances
                    .iter()
                    .copied()
                    .partition(|i| Mat4::from_cols_array_2d(&i.model).determinant() < 0.0);
                self.upload_bytes += cache.batches[index].upload_shared(
                    &self.device,
                    &self.queue,
                    &normal,
                    left.map(|left| &left.batches[index]),
                );
                self.upload_bytes += cache.mirrored_batches[index].upload_shared(
                    &self.device,
                    &self.queue,
                    &mirrored,
                    left.map(|left| &left.mirrored_batches[index]),
                );
            }
            cache.imported.retain(&self.scene);
            self.upload_bytes += cache.imported.upload(
                &self.device,
                &self.queue,
                &cache.queue.imported,
                &self.imported_meshes,
                left.map(|left| &left.imported),
            );
            cache.key = Some(key);
        } else if let Some(left) = left {
            for (index, batch) in cache.batches.iter_mut().enumerate() {
                self.upload_bytes +=
                    batch.refresh_alias(&self.device, &self.queue, Some(&left.batches[index]));
            }
            for (index, batch) in cache.mirrored_batches.iter_mut().enumerate() {
                self.upload_bytes += batch.refresh_alias(
                    &self.device,
                    &self.queue,
                    Some(&left.mirrored_batches[index]),
                );
            }
            for (index, batch) in cache.imported.pages.iter_mut().enumerate() {
                self.upload_bytes +=
                    batch.refresh_alias(&self.device, &self.queue, left.imported.pages.get(index));
            }
        }
        if is_main {
            self.main_instance_upload_bytes += (self.upload_bytes - instance_upload_start) as u64;
        }
        let mut grid_uniform = build_uniform(camera_matrix, camera_pos, lights);
        grid_uniform.appearance = self.appearance;
        cache.light_class = crate::render_lights::LightClass::from_colors(
            lights.map(|light| light.color),
            self.appearance.enabled(),
        );
        // Reflection textures contain linear lighting; display transform belongs
        // to the final visible surface, not both the reflected and main draw.
        if pass == RenderPassKind::Reflection {
            grid_uniform.appearance.sun[3] = 0.0;
        }
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
        let cache = &self.passes[self.last_main_cache];
        [
            cache.queue.total as f64,
            cache.queue.visible as f64,
            (cache.queue.total - cache.queue.visible) as f64,
            (cache.queue.draw_calls()
                + cache
                    .batches
                    .iter()
                    .zip(&cache.mirrored_batches)
                    .filter(|(a, b)| !a.instances().is_empty() && !b.instances().is_empty())
                    .count()
                + cache
                    .queue
                    .visible_imported_ids()
                    .filter_map(|id| self.gltf_models.get(id))
                    .map(|m| {
                        super::gltf_gpu::draw_count(m, false) + super::gltf_gpu::draw_count(m, true)
                    })
                    .sum::<usize>()
                - cache
                    .queue
                    .visible_imported_ids()
                    .filter(|id| self.gltf_models.contains_key(id))
                    .count()) as f64,
            self.upload_bytes as f64,
            self.prepare_ms,
            self.frame_metrics.fps,
            self.frame_metrics.frame_ms,
        ]
    }

    pub fn set_debug_diagnostics(&mut self, enabled: bool) {
        if self.debug_enabled == enabled {
            return;
        }
        self.debug_enabled = enabled;
        self.gltf_gpu.set_diagnostics(enabled);
        // Only the main-view HUD reports traversal; auxiliary passes need no counters.
        for (index, cache) in self.passes.iter_mut().enumerate() {
            if index == 0 || index >= BASE_PASS_CACHE_COUNT {
                cache.queue.collect_stats = enabled;
                if !enabled {
                    cache.queue.culling_stats = Default::default();
                }
            }
        }
        if !enabled {
            self.cull_ms = 0.0;
            self.cull_cached = true;
        }
    }
    pub fn configure_neon_glow(&mut self, strength: f32, radius: f32) -> bool {
        if !strength.is_finite()
            || !(0.0..=2.0).contains(&strength)
            || !radius.is_finite()
            || !(0.001..=0.05).contains(&radius)
        {
            return false;
        }
        if strength == 0.0 {
            self.neon_glow = None;
            self.neon_meshes.clear();
            self.neon_draws = 0;
            self.neon_segments = 0;
            return true;
        }
        self.neon_glow
            .get_or_insert_with(|| super::neon_glow::NeonGlow::new(&self.device))
            .configure(strength, radius)
    }
    fn has_neon_geometry(&self, id: u32) -> bool {
        self.scene.triangle_meshes().contains_key(&id)
            || self.scene.mesh_variants.contains_key(&id)
            || self
                .scene
                .info_panels
                .get(&id)
                .is_some_and(|panel| panel.kind.neon())
    }
    fn prepare_neon_glow(&mut self, target: &RenderTarget<'_>) {
        let Some(glow) = &mut self.neon_glow else {
            return;
        };
        if !glow.enabled() {
            return;
        }
        let cache = &self.passes[self.active_cache];
        for (id, instances) in &cache.queue.imported {
            if self.neon_meshes.contains_key(id) || !instances.iter().any(|i| i.emission[0] > 0.0) {
                continue;
            }
            if let Some(digit) = self.scene.numeric_font_digit(*id) {
                self.neon_meshes.insert(
                    *id,
                    super::neon_glow::GlowMesh::new(
                        &self.device,
                        &crate::info_panel::numeric_glyph_glow(digit),
                    ),
                );
                continue;
            }
            if let Some(mesh) = self
                .scene
                .triangle_meshes()
                .get(id)
                .map(|object| &object.mesh)
                .or_else(|| {
                    self.scene
                        .mesh_variants
                        .get(id)
                        .map(|asset| &asset.variants[0])
                })
                .and_then(|mesh| mesh.ready_cpu())
            {
                self.neon_meshes.insert(
                    *id,
                    super::neon_glow::GlowMesh::new(&self.device, &mesh.render_mesh()),
                );
            } else if let Some(panel) = self
                .scene
                .info_panels
                .get(id)
                .filter(|panel| panel.kind.neon())
            {
                // Disabling glow releases its cache. Restore unchanged opt-in text
                // once on re-enable; dirty text is replaced by refresh_info_panel_meshes.
                self.neon_meshes.insert(
                    *id,
                    super::neon_glow::GlowMesh::new(&self.device, &panel.mesh),
                );
            }
        }
        self.neon_meshes.retain(|id, _| self.scene.mesh_exists(*id));
        self.variant_colors
            .retain(|id, _| self.scene.mesh_variants.contains_key(id));
        glow.prepare(
            &self.device,
            &self.queue,
            RenderView {
                view_projection: cache.key.expect("prepared render view").0,
                camera_position: cache.camera_position,
            },
            target,
        );
    }
    fn encode_neon_glow<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        cache_index: usize,
    ) -> (usize, usize) {
        let Some(glow) = &self.neon_glow else {
            return (0, 0);
        };
        let cache = &self.passes[cache_index];
        let mut stats = (0, 0);
        let mut bound = false;
        for (id, instances) in &cache.queue.imported {
            if instances.is_empty()
                || !instances.iter().any(|i| i.emission[0] > 0.)
                || self.scene.is_cut_feedback_ring(*id)
            {
                continue;
            }
            if let Some(Some(mesh)) = self.neon_meshes.get(id) {
                if !bound {
                    if !glow.bind(pass) {
                        return stats;
                    }
                    bound = true;
                }
                glow.draw(pass, mesh, cache.imported.slice(id), instances.len() as u32);
                stats.0 += 1;
                stats.1 += mesh.indices as usize / 6 * instances.len();
            }
        }
        stats
    }
    pub fn set_renderer_experiment(&mut self, mode: u32) -> bool {
        self.gltf_gpu.set_experiment(mode)
    }
    pub fn gltf_index_layouts(&self) -> serde_json::Value {
        self.gltf_gpu.index_layouts()
    }
    pub fn set_gltf_compact_indices_enabled(&mut self, enabled: bool) -> bool {
        self.gltf_gpu.set_compact_indices_enabled(enabled)
    }
    pub fn gltf_vertex_layouts(&self) -> serde_json::Value {
        self.gltf_gpu.vertex_layouts()
    }
    pub fn set_gltf_white_color_omission_enabled(&mut self, enabled: bool) -> bool {
        self.gltf_gpu.set_white_color_omission_enabled(enabled)
    }
    pub fn set_panel_buffer_reuse(&mut self, enabled: bool) {
        self.panel_buffer_reuse = enabled;
        // Force a real upload for the independent original/candidate pixel oracle.
        for panel in self.scene.info_panels.values_mut() {
            panel.dirty = true;
        }
    }
    pub fn set_overlay_selection_reference(&mut self, enabled: bool) {
        self.overlay_selection_reference = enabled;
    }
    pub fn set_prepared_plan_cache_enabled(&self, enabled: bool) {
        self.gltf_gpu.set_plan_cache_enabled(enabled);
    }
    pub fn configure_gltf_instance_pool(
        &mut self,
        bytes: Option<u32>,
        resources: Option<u32>,
        stream: Option<u32>,
    ) -> bool {
        self.gltf_gpu
            .configure_instance_pool(bytes, resources, stream)
    }
    pub fn debug_values(&self) -> serde_json::Value {
        let gltf = self.gltf_gpu.stats();
        let stats = self.render_stats();
        let bvh = self.passes[self.last_main_cache].queue.culling_stats;
        let fine_values = serde_json::json!({"fineGroupsVisited":gltf.fine_groups_visited,"fineGroupMembershipHits":gltf.fine_group_membership_hits,"fineGroupMembershipMisses":gltf.fine_group_membership_misses,"finePreparedRowsReused":gltf.fine_prepared_rows_reused,"finePreparedRowsCreated":gltf.fine_prepared_rows_created,"finePayloadGroupsReused":gltf.fine_payload_groups_reused,"finePayloadGroupsRebuilt":gltf.fine_payload_groups_rebuilt,"finePayloadRecordsWritten":gltf.fine_payload_records_written,"finePayloadBytesWritten":gltf.fine_payload_bytes_written,"fineBasisComparisons":gltf.fine_basis_comparisons,"fineWorldDataUpdates":gltf.fine_world_data_updates,"fineFullRecordPacks":gltf.fine_full_record_packs,"fineCompatibilityHits":gltf.fine_compatibility_hits,"fineCacheBudgetFallbacks":gltf.fine_cache_budget_fallbacks,"fineDynamicFallbacks":gltf.fine_dynamic_fallbacks,"fineNoInstanceFallbacks":gltf.fine_no_instance_fallbacks,"fineCacheBytes":gltf.fine_cache_bytes,"fineLayoutActiveBytes":gltf.fine_layout_active_bytes,"fineLayoutReservedBytes":gltf.fine_layout_reserved_bytes});
        let prepare_values = serde_json::json!({
                "preparedPlanCache": self.gltf_gpu.plan_cache_stats(),
                "prepareEligibleRebuilds": gltf.prepare_eligible_rebuilds,
                "prepareGroupRebuilds": gltf.prepare_group_rebuilds,
                "prepareOpaqueGroupRebuilds": gltf.prepare_opaque_group_rebuilds,
                "preparePayloadRebuilds": gltf.prepare_payload_rebuilds,
                "prepareRecordRebuilds": gltf.prepare_record_rebuilds,
                "prepareTimedCalls": gltf.prepare_timed_calls,
                "prepareStageMs": (gltf.prepare_timed_calls > 0).then_some(serde_json::json!({
                    "eligible":gltf.prepare_stage_ms[0], "groups":gltf.prepare_stage_ms[1],
                    "basis":gltf.prepare_stage_ms[2], "payload":gltf.prepare_stage_ms[3],
                    "records":gltf.prepare_stage_ms[4], "resources":gltf.prepare_stage_ms[5],
                })),
        });
        let mut gltf_values = serde_json::json!({
                "cpuDeformedVertices": gltf.cpu_deformed_vertices,
                "rigidUploadBytes": gltf.rigid_upload_bytes,
                "uniformUploadBytes": gltf.uniform_upload_bytes,
                "bufferUploadCalls": gltf.buffer_upload_calls,
                "skinUploadBytes": gltf.skin_upload_bytes,
                "skinPaletteUploadBytes": gltf.skin_palette_upload_bytes,
                "drawCalls": gltf.draw_calls,
                "primitiveTests": gltf.primitive_tests,
                "primitiveCulled": gltf.primitive_culled,
                "rootBoundsTests": gltf.root_bounds_tests,
                "rootFullyInside": gltf.root_fully_inside,
                "activeDrawIndexBytes": self.gltf_models.values().map(|m| m.active_draw_index_bytes()).sum::<usize>(),
                "instancedDrawCalls": gltf.instanced_draw_calls,
                "translatedDrawCalls": gltf.translated_draw_calls,
                "translatedInstances": gltf.translated_instances,
                "instancesDrawn": gltf.instances_drawn,
                "trianglesDrawn": gltf.triangles_drawn,
                "overlapTests": gltf.overlap_tests,
                "overlapNodeTests": gltf.overlap_node_tests,
                "batchBreaks": {
                    "rootBlend": gltf.root_batch_breaks[0],
                    "rootUnsupported": gltf.root_batch_breaks[1],
                    "rootBounds": gltf.root_batch_breaks[2],
                    "rootAsset": gltf.root_batch_breaks[3],
                    "rootOverlap": gltf.root_batch_breaks[4],
                    "rootSpatial": gltf.root_batch_breaks[5],
                    "drawCap": gltf.draw_batch_breaks[0],
                    "drawOrdinal": gltf.draw_batch_breaks[1],
                    "drawAsset": gltf.draw_batch_breaks[2],
                    "drawDeformed": gltf.draw_batch_breaks[3],
                    "drawMirror": gltf.draw_batch_breaks[4],
                    "drawMaterial": gltf.draw_batch_breaks[5],
                    "drawPrimitive": gltf.draw_batch_breaks[6],
                    "drawOptions": gltf.draw_batch_breaks[7],
                },
                "instancePool": self.gltf_gpu.instance_pool_stats(),
                "vertexColors": self.gltf_gpu.vertex_color_stats(),
                "indices": self.gltf_gpu.index_stats(),
                "instancePagesCreated": gltf.instance_pages_created,
                "instanceBuffersCreated": gltf.instance_buffers_created,
                "instanceBindGroupsCreated": gltf.instance_bind_groups_created,
                "instancePagesActive": gltf.instance_pages_active,
                "instancePagesRetained": gltf.instance_pages_retained,
                "instanceUploadBytes": gltf.instance_upload_bytes,
                "instanceBufferBytes": gltf.instance_buffer_bytes,
                "queueScratchGrowths": gltf.queue_scratch_growths,
                "queueScratchBytes": gltf.queue_scratch_bytes,
                "pipelineChanges": gltf.pipeline_changes,
                "bindGroupChanges": gltf.bind_group_changes,
                "geometryCacheHits": gltf.geometry_cache_hits,
                "materialCacheHits": gltf.material_cache_hits,
                "geometryResources": gltf.geometry_resources,
                "materialResources": gltf.material_resources,
                "cachedAssets": gltf.cached_assets,
        });
        if let serde_json::Value::Object(values) = fine_values {
            gltf_values.as_object_mut().unwrap().extend(values);
        }
        if let serde_json::Value::Object(values) = prepare_values {
            gltf_values.as_object_mut().unwrap().extend(values);
        }
        serde_json::json!({
            "font3d": {
                "runtime": self.scene.font3d_stats(),
                "geometryBytes": self.scene.font3d_assets().filter_map(|id| self.imported_meshes.get(id)).map(|mesh| mesh.vertex_buffer.size() + mesh.index_buffer.size()).sum::<u64>(),
                "instanceGeometryBytes": self.scene.font3d_children().filter_map(|id| self.imported_meshes.get(id)).map(|mesh| mesh.vertex_buffer.size() + mesh.index_buffer.size()).sum::<u64>(),
            },
            "numericFont": {
                "glyphMeshes": self.scene.numeric_font_stats().0,
                "liveGlyphs": self.scene.numeric_font_stats().1,
                "panels": self.scene.numeric_font_stats().2,
                "geometryBytes": self.scene.numeric_font_assets().filter_map(|id| self.imported_meshes.get(id)).map(|mesh| mesh.vertex_buffer.size() + mesh.index_buffer.size()).sum::<u64>(),
                "instanceGeometryBytes": self.scene.numeric_font_children().filter_map(|id| self.imported_meshes.get(id)).map(|mesh| mesh.vertex_buffer.size() + mesh.index_buffer.size()).sum::<u64>(),
            },
            "sharedMeshes": {
                "assets": self.scene.mesh_variants.len(),
                "liveInstances": self.scene.variant_instances.len(),
                "gpuMeshes": self.variant_colors.len(),
                "colorTableBytes": self.variant_colors.values().map(|colors| colors.bytes).sum::<usize>(),
                "geometryBytes": self.variant_colors.keys().filter_map(|id| self.imported_meshes.get(id)).map(|mesh| mesh.vertex_buffer.size() + mesh.index_buffer.size()).sum::<u64>(),
                "instanceGeometryBytes": self.scene.variant_instances.values().filter_map(|(id, _)| self.imported_meshes.get(id)).map(|mesh| mesh.vertex_buffer.size() + mesh.index_buffer.size()).sum::<u64>(),
            },
            "neonGlow": {"enabled":self.neon_glow.is_some(),"draws":self.neon_draws,"segments":self.neon_segments,
                "cachedMeshes":self.neon_meshes.len(), "bufferBytes":self.neon_meshes.values().flatten().map(|mesh|mesh.vertices as usize * std::mem::size_of::<Vertex>() + mesh.indices as usize * 4).sum::<usize>()},
            "panelBufferReuse": self.panel_buffer_reuse,
            "panelMeshes": {
                "events":self.panel_mesh_stats,
                "liveBuffers":self.scene.info_panels.keys().filter(|id|self.imported_meshes.contains_key(id)).count()*2,
                "capacityBytes":self.scene.info_panels.keys().filter_map(|id|self.imported_meshes.get(id)).map(|m|m.vertex_buffer.size()+m.index_buffer.size()).sum::<u64>(),
                "payloadBytes":self.scene.info_panels.iter().filter(|(id,_)|self.imported_meshes.contains_key(id)).map(|(_,p)|(p.mesh.vertices.len()*36+p.mesh.indices.len()*4) as u64).sum::<u64>(),
                "indexShadowBytes":self.imported_meshes.values().map(|m|m.panel_indices.capacity()*4).sum::<usize>(),
                "inactivePoolBytes":0,
            },
            "overlaySelection": self.overlay_selection,
            "overlaySelectionReference": self.overlay_selection_reference,
            "translucency": {
                "cachedMeshes": self.alpha_indices.len(),
                "indexBufferBytes": self.alpha_indices.values().map(|entry| entry.count as usize * 4).sum::<usize>(),
                "submittedIndices": self.translucent_indices.get(),
                "fullIndices": self.translucent_full_indices.get(),
                "draws": self.translucent_draws.get(),
            },
            "mainViewCache": {
                "slot": if self.last_main_cache == 0 { 0 } else { self.last_main_cache - BASE_PASS_CACHE_COUNT + 1 },
                "hits": self.main_cache_hits,
                "misses": self.main_cache_misses,
                "instanceUploadBytes": self.main_instance_upload_bytes,
                "slots": 1 + self.passes.len() - BASE_PASS_CACHE_COUNT,
            },
            "gltf": gltf_values,
            "gltfDraws": self.gltf_gpu.draw_diagnostics(),
            "instanceUploadPages": self.passes.iter().map(|cache| cache.imported.pages.len()).sum::<usize>(),
            "fps": stats[6], "frameMs": stats[7], "frameP95Ms": self.frame_metrics.frame_p95_ms, "cpuMs": stats[5],
            "cullMs": self.cull_ms, "cullCached": self.cull_cached,
            "total": stats[0], "visible": stats[1], "culled": stats[2],
            "drawCalls": self.draw_calls, "mainDrawCalls": self.main_draw_calls,
            "bvhNodes": bvh.nodes_visited, "objectTests": bvh.object_tests,
            "staticRebuilds": bvh.static_rebuilds, "dynamicRebuilds": bvh.dynamic_rebuilds,
        })
    }
    pub fn info_diagnostic_rows(&self) -> Vec<(&'static str, String)> {
        let queue = &self.passes[self.last_main_cache].queue;
        let profile = crate::render::profiler::stats();
        let gpu = &profile["gpu"];
        let gpu_ms = gpu["passesMs"]
            .as_object()
            .map(|passes| passes.values().filter_map(|v| v.as_f64()).sum::<f64>());
        vec![
            ("FPS", format!("{:.1}", self.frame_metrics.fps)),
            (
                "FRAME P95",
                format!("{:.2}", self.frame_metrics.frame_p95_ms),
            ),
            (
                "GPU SUM MS",
                gpu_ms.map_or_else(|| "N/A".into(), |v| format!("{v:.2}")),
            ),
            (
                "GPU AGE",
                gpu["ageFrames"]
                    .as_u64()
                    .map_or_else(|| "N/A".into(), |v| v.to_string()),
            ),
            ("UPLOAD B", self.upload_bytes.to_string()),
            ("FRAME MS", format!("{:.2}", self.frame_metrics.frame_ms)),
            ("CPU MS", format!("{:.2}", self.prepare_ms)),
            ("CULL MS", format!("{:.2}", self.cull_ms)),
            ("VISIBLE", queue.visible.to_string()),
            (
                "CULLED",
                queue.total.saturating_sub(queue.visible).to_string(),
            ),
            ("OBJECTS", queue.total.to_string()),
            ("DRAWS", self.draw_calls.to_string()),
        ]
    }
    pub fn xr_debug_content(
        &self,
        cuts: u64,
        round: Option<crate::xr_round::RoundClock>,
    ) -> crate::debug_overlay::XrHudContent {
        crate::debug_overlay::XrHudContent {
            scene: vec![
                ("FPS", format!("{:.1}", self.frame_metrics.fps)),
                ("FRAME MS", format!("{:.2}", self.frame_metrics.frame_ms)),
                ("CPU MS", format!("{:.2}", self.prepare_ms)),
                (
                    "OBJEKTY",
                    self.passes[self.last_main_cache].queue.total.to_string(),
                ),
                ("DRAWS", self.draw_calls.to_string()),
            ],
            cuts,
            round: round.map(|round| crate::debug_overlay::XrRoundDisplay {
                remaining: round.remaining,
                finished: round.finished(),
            }),
        }
    }
    pub fn debug_lines(&self) -> Vec<String> {
        let q = &self.passes[self.last_main_cache].queue;
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
    ) -> Result<wgpu::CommandBuffer, String> {
        self.render_view_slot(view, target, 0)
    }
    /// Logical main views: desktop 0, XR left 1 and XR right 2.
    pub fn render_view_slot(
        &mut self,
        view: RenderView,
        target: &RenderTarget<'_>,
        slot: u32,
    ) -> Result<wgpu::CommandBuffer, String> {
        if slot > 2 {
            return Err("Main view slot must be desktop 0 or XR eye 1..2".into());
        }
        target.validate()?;
        self.main_view_slot = slot;
        self.refresh_info_panel_meshes();
        let mut effects = self.effects.take().unwrap();
        let result = effects.render(self, view, target);
        self.effects = Some(effects);
        result
    }
    /// Static-only probe capture, independent from main/stereo visibility and overlays.
    pub fn render_environment(
        &mut self,
        view: RenderView,
        target: &RenderTarget<'_>,
    ) -> Result<(), String> {
        let mut effects = self.effects.take().unwrap();
        effects.write_uniform(&self.queue, data::EffectsUniforms::default());
        self.prepare_pass(view, RenderPassKind::Environment);
        let result = self.encode_color(target, &effects, true).map(|command| {
            self.queue.submit(Some(command));
        });
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
        self.refresh_info_panel_meshes();
        self.prepare_pass(view, pass);
        self.draw_depth(target)
    }
    pub(super) fn draw_depth(&mut self, target: &DepthRenderTarget<'_>) -> Result<(), String> {
        let _cpu_scope = super::profiler::cpu_scope("render.encode.depth");
        target.validate()?;
        self.shadow_pipelines
            .entry(target.format)
            .or_insert_with(|| {
                pipeline::build_shadow(&self.device, &self.bind_group_layout, target.format)
            });
        let gltf_key = super::gltf_gpu::PipelineKey {
            format: wgpu::TextureFormat::Rgba8Unorm,
            depth_format: target.format,
            clipped: false,
            reflected: false,
            material_mask: false,
            packed_depth: false,
            shadow: true,
            single_static_shadow: false,
            light_class: Default::default(),
        };
        let cache = &self.passes[self.active_cache];
        let gltf_opaque = self.gltf_gpu.prepare_draws(
            &self.device,
            &self.queue,
            cache
                .queue
                .visible_imported_ids()
                .filter_map(|id| self.gltf_models.get(id).map(|m| (*id, m))),
            gltf_key,
            false,
            cache.camera_position,
            self.active_cache,
            cache.key.expect("prepared view").0,
            self.frustum_culling,
            self.scene.render_revision(),
        );
        self.upload_bytes += gltf_opaque.upload_bytes;
        let pipeline = &self.shadow_pipelines[&target.format];
        let cache = &self.passes[self.active_cache];
        let mut draws = 0;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("shadow encoder"),
            });
        let stamp = super::profiler::stamp(if self.active_cache.is_multiple_of(2) {
            "shadow.static"
        } else {
            "shadow.dynamic"
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
                timestamp_writes: stamp.as_ref().map(|stamp| stamp.writes()),
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
                    let count = batch.instances().len() as u32;
                    if count == 0 {
                        continue;
                    }
                    pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    pass.set_vertex_buffer(1, batch.draw_buffer().slice(..));
                    pass.set_index_buffer(mesh.index_buffer.slice(..), mesh.index_format);
                    pass.draw_indexed(0..mesh.index_count, 0, 0..count);
                    draws += 1;
                }
            }
            for (id, instances) in &cache.queue.imported {
                if instances.is_empty() {
                    continue;
                }
                let Some(mesh) = self
                    .imported_meshes
                    .get(id)
                    .filter(|mesh| mesh.index_count > 0)
                else {
                    continue;
                };
                pass.set_bind_group(
                    0,
                    self.variant_colors
                        .get(id)
                        .map_or(&self.grid_bind_group, |colors| &colors.group),
                    &[],
                );
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_vertex_buffer(1, cache.imported.slice(id));
                pass.set_index_buffer(mesh.index_buffer.slice(..), mesh.index_format);
                pass.draw_indexed(0..mesh.index_count, 0, 0..instances.len() as u32);
                draws += 1;
            }
            draws +=
                self.gltf_gpu
                    .draw_prepared(&mut pass, &gltf_opaque, gltf_key, self.active_cache);
        }
        self.queue.submit(Some(encoder.finish()));
        self.draw_calls += draws;
        Ok(())
    }
    pub(super) fn encode_color(
        &mut self,
        target: &RenderTarget<'_>,
        effects: &Effects,
        reflected: bool,
    ) -> Result<wgpu::CommandBuffer, String> {
        let _cpu_scope = super::profiler::cpu_scope("render.encode.color");
        target.validate()?;
        let rings = self.cut_feedback_rings(self.active_cache);
        let draw_rings = !reflected && !target.material_mask;
        if draw_rings && rings.iter().any(Option::is_some) {
            self.prepare_cut_feedback(target);
        }
        if !reflected && !target.material_mask {
            self.prepare_neon_glow(target);
        }
        self.neon_draws = 0;
        self.neon_segments = 0;
        let mut neon_stats = (0, 0);
        let mut draws = 0;
        let key = (
            target.color_format,
            target.depth_format,
            self.passes[self.active_cache]
                .key
                .is_some_and(|k| k.0.determinant() < 0.0),
            target.material_mask,
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
                    key.4,
                    false,
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
                    key.4,
                    false,
                ),
                pipeline::build(
                    &self.device,
                    key.0,
                    key.1,
                    &self.bind_group_layout,
                    &effects.layout,
                    key.2,
                    key.3,
                    key.4,
                    true,
                ),
            )
        });
        let gltf_key = super::gltf_gpu::PipelineKey {
            format: target.color_format,
            depth_format: target.depth_format,
            clipped: reflected,
            reflected: self.passes[self.active_cache]
                .key
                .is_some_and(|k| k.0.determinant() < 0.0),
            material_mask: target.material_mask,
            packed_depth: target.auxiliary.is_some(),
            shadow: false,
            single_static_shadow: !reflected && effects.single_static_shadow(),
            light_class: self.passes[self.active_cache].light_class,
        };
        let cache = &self.passes[self.active_cache];
        let gltf_opaque = self.gltf_gpu.prepare_draws(
            &self.device,
            &self.queue,
            cache
                .queue
                .visible_imported_ids()
                .filter_map(|id| self.gltf_models.get(id).map(|m| (*id, m))),
            gltf_key,
            false,
            cache.camera_position,
            self.active_cache,
            cache.key.expect("prepared view").0,
            self.frustum_culling,
            self.scene.render_revision(),
        );
        self.upload_bytes += gltf_opaque.upload_bytes;
        let gltf_blend = if reflected || !target.material_mask {
            let cache = &self.passes[self.active_cache];
            let p = self.gltf_gpu.prepare_draws(
                &self.device,
                &self.queue,
                cache
                    .queue
                    .visible_imported_ids()
                    .filter_map(|id| self.gltf_models.get(id).map(|m| (*id, m))),
                gltf_key,
                true,
                cache.camera_position,
                self.active_cache,
                cache.key.expect("prepared view").0,
                self.frustum_culling,
                self.scene.render_revision(),
            );
            self.upload_bytes += p.upload_bytes;
            Some(p)
        } else {
            None
        };
        if !reflected && !target.material_mask {
            self.prepare_translucent(target, &effects.layout);
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
        let stamp = super::profiler::stamp(if self.active_cache == 2 + MAX_LIGHTS * 2 {
            "scene.environment"
        } else if reflected {
            "scene.reflection"
        } else {
            "scene.main"
        });
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
                timestamp_writes: stamp.as_ref().map(|stamp| stamp.writes()),
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
            let mut active_pipeline = 0;
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
                for (batch, pipeline_index, pipeline) in [
                    (&cache.batches[index], 0, &pipelines.0),
                    (&cache.mirrored_batches[index], 2, &pipelines.2),
                ] {
                    let count = batch.instances().len() as u32;
                    if count == 0 {
                        continue;
                    }
                    if active_pipeline != pipeline_index {
                        rp.set_pipeline(pipeline);
                        active_pipeline = pipeline_index;
                    }
                    rp.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    rp.set_vertex_buffer(1, batch.draw_buffer().slice(..));
                    rp.set_index_buffer(mesh.index_buffer.slice(..), mesh.index_format);
                    rp.draw_indexed(0..mesh.index_count, 0, 0..count);
                    draws += 1;
                }
            }
            for (id, instances) in &cache.queue.imported {
                // Presentation FX skip probes/reflections; material-metadata targets defer
                // their rings to the final transparent overlay.
                if instances.is_empty() || rings.contains(&Some(*id)) {
                    continue;
                }
                let Some(mesh) = self
                    .imported_meshes
                    .get(id)
                    .filter(|mesh| mesh.index_count > 0)
                else {
                    continue;
                };
                // Glass retains opaque neon seams on both sides of its translucent body.
                let mixed_winding = self.scene.mesh_variants.contains_key(id)
                    && instances.iter().any(|instance| {
                        (Mat4::from_cols_array_2d(&instance.model).determinant() < 0.0)
                            != (Mat4::from_cols_array_2d(&instances[0].model).determinant() < 0.0)
                    });
                let pipeline_index = if mixed_winding
                    || (instances
                        .iter()
                        .any(|instance| instance.emission[1] > 0.0 || instance.emission[2] < 1.0)
                        && self.has_neon_geometry(*id))
                {
                    3
                } else if Mat4::from_cols_array_2d(&instances[0].model).determinant() < 0.0 {
                    2
                } else {
                    0
                };
                if active_pipeline != pipeline_index {
                    rp.set_pipeline(match pipeline_index {
                        3 => &pipelines.3,
                        2 => &pipelines.2,
                        _ => &pipelines.0,
                    });
                    active_pipeline = pipeline_index;
                }
                rp.set_bind_group(
                    0,
                    self.variant_colors
                        .get(id)
                        .map_or(&self.grid_bind_group, |colors| &colors.group),
                    &[],
                );
                rp.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                rp.set_vertex_buffer(1, cache.imported.slice(id));
                rp.set_index_buffer(mesh.index_buffer.slice(..), mesh.index_format);
                rp.draw_indexed(0..mesh.index_count, 0, 0..instances.len() as u32);
                draws += 1;
            }
            draws +=
                self.gltf_gpu
                    .draw_prepared(&mut rp, &gltf_opaque, gltf_key, self.active_cache);
            if reflected || !target.material_mask {
                draws += self.gltf_gpu.draw_prepared(
                    &mut rp,
                    gltf_blend.as_ref().expect("transparent queue"),
                    gltf_key,
                    self.active_cache,
                );
            }
            if !reflected && !target.material_mask {
                rp.set_bind_group(0, &self.grid_bind_group, &[]);
                rp.set_bind_group(1, &effects.scene_group, &[]);
                draws += self.draw_translucent(&mut rp, self.active_cache, target);
                neon_stats = self.encode_neon_glow(&mut rp, self.active_cache);
                draws += neon_stats.0;
            }
            if draw_rings {
                draws += self.draw_cut_feedback(&mut rp, self.active_cache, &rings, target);
            }
            if !reflected && !target.material_mask && (self.draw_grid || self.draw_light_helpers) {
                rp.set_pipeline(&pipelines.1);
                rp.set_bind_group(0, &self.grid_bind_group, &[]);
                if !target.material_mask && self.draw_grid {
                    rp.set_vertex_buffer(0, self.grid_vertex_buffer.slice(..));
                    rp.draw(0..self.grid_vertex_count, 0..1);
                    draws += 1;
                }
                if !target.material_mask && self.draw_light_helpers {
                    rp.set_vertex_buffer(0, self.light_vertex_buffer.slice(..));
                    rp.draw(0..self.light_vertex_count, 0..1);
                    draws += 1;
                }
            }
        }
        self.neon_draws = neon_stats.0;
        self.neon_segments = neon_stats.1;
        self.draw_calls += draws;
        if !reflected {
            self.main_draw_calls += draws;
        }
        Ok(encoder.finish())
    }
    /// Fixed pool: reuse the existing imported mesh/instance buffers, at most two rings.
    fn cut_feedback_rings(&self, cache_index: usize) -> [Option<u32>; 2] {
        let mut rings = [None; 2];
        for (slot, id) in self.passes[cache_index]
            .queue
            .imported
            .iter()
            .filter(|(id, instances)| {
                !instances.is_empty()
                    && self.scene.is_cut_feedback_ring(**id)
                    && self.imported_meshes.contains_key(*id)
            })
            .map(|(id, _)| *id)
            .take(2)
            .enumerate()
        {
            rings[slot] = Some(id);
        }
        rings
    }

    fn prepare_cut_feedback(&mut self, target: &RenderTarget<'_>) {
        let gpu = self.cut_feedback_gpu.get_or_insert_with(|| {
            super::cut_feedback_gpu::CutFeedbackGpu::new(&self.device, &self.queue)
        });
        gpu.prepare(
            &self.device,
            &self.bind_group_layout,
            (
                target.color_format,
                target.depth_format,
                target.auxiliary.is_some(),
            ),
        );
    }

    fn draw_cut_feedback<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        cache_index: usize,
        rings: &[Option<u32>; 2],
        target: &RenderTarget<'_>,
    ) -> usize {
        let Some(gpu) = self.cut_feedback_gpu.as_ref() else {
            return 0;
        };
        if rings.iter().all(Option::is_none) {
            return 0;
        }
        pass.set_pipeline(gpu.pipeline((
            target.color_format,
            target.depth_format,
            target.auxiliary.is_some(),
        )));
        pass.set_bind_group(0, &self.grid_bind_group, &[]);
        pass.set_bind_group(1, &gpu.bind_group, &[]);
        let cache = &self.passes[cache_index];
        let mut draws = 0;
        for id in rings.iter().flatten() {
            let mesh = &self.imported_meshes[id];
            if mesh.index_count == 0 {
                continue;
            }
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_vertex_buffer(1, cache.imported.slice(id));
            pass.set_index_buffer(mesh.index_buffer.slice(..), mesh.index_format);
            pass.draw_indexed(
                0..mesh.index_count,
                0,
                0..cache.queue.imported[id].len() as u32,
            );
            draws += 1;
        }
        draws
    }

    fn has_translucent(&self, cache_index: usize) -> bool {
        let cache = &self.passes[cache_index];
        cache
            .batches
            .iter()
            .chain(cache.mirrored_batches.iter())
            .any(|b| {
                b.instances()
                    .iter()
                    .any(InstanceData::needs_translucent_overlay)
            })
            || cache.queue.imported.iter().any(|(id, instances)| {
                self.imported_meshes.contains_key(id)
                    && instances
                        .iter()
                        .any(InstanceData::needs_translucent_overlay)
            })
    }

    pub fn reset_translucency_metrics(&self) {
        self.translucent_indices.set(0);
        self.translucent_full_indices.set(0);
        self.translucent_draws.set(0);
    }

    fn prepare_translucent(&mut self, target: &RenderTarget<'_>, effects: &wgpu::BindGroupLayout) {
        if !self.has_translucent(self.active_cache) {
            return;
        }
        for (id, instances) in &self.passes[self.active_cache].queue.imported {
            let Some(source) = self.cpu_mesh_sources.get(id) else {
                continue;
            };
            let Some(first) = instances.first() else {
                continue;
            };
            let threshold = first.emission[3];
            // One bounded entry per mesh, never reuse across incompatible instances.
            if !threshold.is_finite() || instances.iter().any(|data| data.emission[3] != threshold)
            {
                self.alpha_indices.remove(id);
                continue;
            }
            if !instances.iter().any(|data| data.emission[2] < 1.0)
                || self
                    .alpha_indices
                    .get(id)
                    .is_some_and(|entry| entry.threshold == threshold)
            {
                continue;
            }
            self.alpha_indices.remove(id);
            let Some(cpu) = source.ready_cpu() else {
                continue;
            };
            let rendered = cpu.render_mesh();
            let indices = if let Some(asset) = self.scene.mesh_variants.get(id) {
                crate::translucency::variant_alpha_indices(
                    &asset.colors,
                    asset.variants[0].triangle_count(),
                    threshold,
                )
            } else {
                let Some(indices) = crate::translucency::alpha_indices(&rendered, threshold) else {
                    continue;
                };
                indices
            };
            let Some(mesh) = self.imported_meshes.get(id) else {
                continue;
            };
            if rendered.indices.len() != mesh.index_count as usize {
                continue;
            }
            let buffer = (!indices.is_empty()).then(|| {
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("translucent subset indices"),
                        contents: data::as_bytes(&indices),
                        usage: wgpu::BufferUsages::INDEX,
                    })
            });
            self.upload_bytes += indices.len() * 4;
            self.alpha_indices.insert(
                *id,
                AlphaIndices {
                    threshold,
                    buffer,
                    count: indices.len() as u32,
                },
            );
        }
        self.translucent_pipelines
            .entry((target.color_format, target.depth_format))
            .or_insert_with(|| {
                // LH winding: cull front-facing (CW geometric backs) second.
                [wgpu::Face::Back, wgpu::Face::Front].map(|cull| {
                    pipeline::build_translucent(
                        &self.device,
                        target.color_format,
                        target.depth_format,
                        &self.bind_group_layout,
                        effects,
                        cull,
                    )
                })
            });
    }

    fn draw_translucent<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        cache_index: usize,
        target: &RenderTarget<'_>,
    ) -> usize {
        let Some(pipelines) = self
            .translucent_pipelines
            .get(&(target.color_format, target.depth_format))
        else {
            return 0;
        };
        let cache = &self.passes[cache_index];
        // One entry per instance; reuse the view's uploaded buffers without any submits.
        let mut draws = Vec::new();
        for (index, kind) in [MeshKind::Cube, MeshKind::Plane, MeshKind::Sphere]
            .into_iter()
            .enumerate()
        {
            for (mirrored, batch) in [
                (false, &cache.batches[index]),
                (true, &cache.mirrored_batches[index]),
            ] {
                for (instance, data) in batch.instances().iter().enumerate() {
                    if !data.needs_translucent_overlay() {
                        continue;
                    }
                    let center = Mat4::from_cols_array_2d(&data.model).w_axis.truncate();
                    draws.push((
                        center.distance_squared(cache.camera_position),
                        kind,
                        mirrored,
                        instance as u32,
                        data.visual_fade() < 1.0,
                    ));
                }
            }
        }
        for (id, instances) in &cache.queue.imported {
            if !self.imported_meshes.contains_key(id) {
                continue;
            }
            for (instance, data) in instances.iter().enumerate() {
                if !data.needs_translucent_overlay() {
                    continue;
                }
                let model = Mat4::from_cols_array_2d(&data.model);
                draws.push((
                    model
                        .w_axis
                        .truncate()
                        .distance_squared(cache.camera_position),
                    MeshKind::Imported(*id),
                    model.determinant() < 0.0,
                    instance as u32,
                    data.visual_fade() < 1.0,
                ));
            }
        }
        draws.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut submitted_draws = 0;
        for (_, kind, mirrored, instance, fading) in &draws {
            let mesh = self.mesh(*kind);
            if mesh.index_count == 0 {
                continue;
            }
            let subset = match kind {
                // A fading object also needs its normally opaque neon seams.
                MeshKind::Imported(id) if !*fading => self.alpha_indices.get(id),
                _ => None,
            };
            let index_count = subset.map_or(mesh.index_count, |entry| entry.count);
            self.translucent_full_indices
                .set(self.translucent_full_indices.get() + mesh.index_count as usize * 2);
            if index_count == 0 {
                continue;
            }
            let color_group = match kind {
                MeshKind::Imported(id) => self.variant_colors.get(id).map(|colors| &colors.group),
                _ => None,
            };
            pass.set_bind_group(0, color_group.unwrap_or(&self.grid_bind_group), &[]);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            match kind {
                MeshKind::Imported(id) => pass.set_vertex_buffer(1, cache.imported.slice(id)),
                _ => {
                    let index = match kind {
                        MeshKind::Cube => 0,
                        MeshKind::Plane => 1,
                        _ => 2,
                    };
                    let batch = if *mirrored {
                        &cache.mirrored_batches[index]
                    } else {
                        &cache.batches[index]
                    };
                    pass.set_vertex_buffer(1, batch.draw_buffer().slice(..));
                }
            }
            if let Some(buffer) = subset.and_then(|entry| entry.buffer.as_ref()) {
                pass.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint32);
            } else {
                pass.set_index_buffer(mesh.index_buffer.slice(..), mesh.index_format);
            }
            // Convex fruit: rear then front surface, with depth tests and no depth writes.
            for side in 0..2 {
                pass.set_pipeline(&pipelines[side ^ usize::from(*mirrored)]);
                pass.draw_indexed(0..index_count, 0, *instance..*instance + 1);
            }
            submitted_draws += 2;
            self.translucent_indices
                .set(self.translucent_indices.get() + index_count as usize * 2);
        }
        self.translucent_draws
            .set(self.translucent_draws.get() + submitted_draws);
        submitted_draws
    }

    /// Compose the final effect and overlays in one pass, preserving their order.
    pub fn draw_postprocess_overlays(
        &mut self,
        target: &RenderTarget<'_>,
        composite: &super::postprocess::FinalPostprocessDraw<'_>,
    ) -> Result<wgpu::CommandBuffer, String> {
        let _cpu_scope = super::profiler::cpu_scope("render.encode.overlays");
        target.validate()?;
        if target.auxiliary.is_some() || target.material_mask {
            return Err("Postprocess overlays require a final single-color target".into());
        }
        self.active_cache = self.last_main_cache;
        let cache_index = self.last_main_cache;
        let rings = self.cut_feedback_rings(cache_index);
        let has_rings = rings.iter().any(Option::is_some);
        let select_scope = super::profiler::cpu_scope("render.overlays.select");
        let mut stages = super::profiler::CpuStages::new([
            "render.overlays.gltf",
            "render.overlays.mesh",
            "render.overlays.neon",
            "render.overlays.rings",
        ]);
        let detailed = stages.enabled();
        self.overlay_selection.calls += 1;
        stages.switch(Some(0));
        let reference = self.overlay_selection_reference;
        let stats = &mut self.overlay_selection.gltf;
        let has_transparent = {
            let mut models = self.passes[cache_index]
                .queue
                .visible_imported_ids()
                .filter_map(|id| self.gltf_models.get(id));
            if detailed {
                models.any(|model| super::gltf_gpu::has_blend_draw::<true>(model, reference, stats))
            } else {
                models
                    .any(|model| super::gltf_gpu::has_blend_draw::<false>(model, reference, stats))
            }
        };
        stages.switch(Some(1));
        let has_mesh_transparency = if detailed {
            let cache = &self.passes[cache_index];
            let stats = &mut self.overlay_selection;
            cache
                .batches
                .iter()
                .chain(cache.mirrored_batches.iter())
                .any(|batch| {
                    stats.mesh_batches_visited += 1;
                    batch.instances().iter().any(|instance| {
                        stats.mesh_instances_visited += 1;
                        instance.needs_translucent_overlay()
                    })
                })
                || cache.queue.imported.iter().any(|(id, instances)| {
                    stats.mesh_imported_visited += 1;
                    self.imported_meshes.contains_key(id)
                        && instances.iter().any(|instance| {
                            stats.mesh_instances_visited += 1;
                            instance.needs_translucent_overlay()
                        })
                })
        } else {
            self.has_translucent(cache_index)
        };
        stages.switch(Some(2));
        let has_helpers = self.draw_grid || self.draw_light_helpers;
        let may_have_neon =
            self.neon_glow.as_ref().is_some_and(|glow| glow.enabled())
                && self.passes[self.last_main_cache].queue.imported.iter().any(
                    |(id, instances)| {
                        !instances.is_empty()
                            && instances.iter().any(|instance| instance.emission[0] > 0.)
                            && !self.scene.is_cut_feedback_ring(*id)
                            && self.has_neon_geometry(*id)
                            && !matches!(self.neon_meshes.get(id), Some(None))
                    },
                );
        if may_have_neon {
            if detailed {
                self.overlay_selection.neon_preparation_calls += 1;
            }
            self.prepare_neon_glow(target);
        }
        let has_neon =
            may_have_neon
                && self.passes[self.last_main_cache].queue.imported.iter().any(
                    |(id, instances)| {
                        !instances.is_empty()
                            && instances.iter().any(|instance| instance.emission[0] > 0.)
                            && !self.scene.is_cut_feedback_ring(*id)
                            && self.neon_meshes.get(id).is_some_and(Option::is_some)
                    },
                );
        let has_overlays =
            has_transparent || has_mesh_transparency || has_rings || has_neon || has_helpers;
        stages.switch(Some(3));
        if has_rings {
            if detailed {
                self.overlay_selection.ring_preparation_calls += 1;
            }
            self.prepare_cut_feedback(target);
        }
        if let Some(ms) = stages.finish() {
            let stats = &mut self.overlay_selection;
            stats.diagnostic_calls += 1;
            stats.with_gltf += u64::from(has_transparent);
            stats.with_mesh += u64::from(has_mesh_transparency);
            stats.with_neon += u64::from(has_neon);
            stats.with_rings += u64::from(has_rings);
            stats.with_helpers += u64::from(has_helpers);
            for (total, elapsed) in stats.stage_ms.iter_mut().zip(ms) {
                *total += elapsed;
            }
        }
        drop(select_scope);
        let helper_key = (target.color_format, target.depth_format);
        if has_helpers {
            self.helper_pipelines.entry(helper_key).or_insert_with(|| {
                pipeline::build_lines(
                    &self.device,
                    helper_key.0,
                    helper_key.1,
                    &self.bind_group_layout,
                )
            });
        }
        if has_mesh_transparency {
            let effects = self.effects.take().unwrap();
            self.prepare_translucent(target, &effects.layout);
            self.effects = Some(effects);
        }
        let cache = &self.passes[self.last_main_cache];
        let key = super::gltf_gpu::PipelineKey {
            format: target.color_format,
            depth_format: target.depth_format,
            clipped: false,
            reflected: cache.key.is_some_and(|k| k.0.determinant() < 0.0),
            material_mask: false,
            packed_depth: false,
            shadow: false,
            single_static_shadow: false,
            light_class: cache.light_class,
        };
        let gltf_transparent = if has_transparent {
            let p = self.gltf_gpu.prepare_draws(
                &self.device,
                &self.queue,
                cache
                    .queue
                    .visible_imported_ids()
                    .filter_map(|id| self.gltf_models.get(id).map(|m| (*id, m))),
                key,
                true,
                cache.camera_position,
                self.last_main_cache,
                cache.key.expect("prepared view").0,
                self.frustum_culling,
                self.scene.render_revision(),
            );
            self.upload_bytes += p.upload_bytes;
            p
        } else {
            Default::default()
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("postprocess overlays"),
            });
        let mut draws = 0;
        let mut neon_stats = (0, 0);
        let label = if has_overlays {
            "final.postprocess.overlays"
        } else {
            composite.name
        };
        let stamp = super::profiler::stamp(label);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.color,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: has_overlays.then_some(
                    wgpu::RenderPassDepthStencilAttachment {
                        view: target.depth,
                        // Every overlay pipeline has depth writes disabled. Keep the
                        // attachment for occlusion, but do not request a depth store.
                        // Read-only also preserves it for a later XR depth resolve.
                        depth_ops: None,
                        stencil_ops: None,
                    },
                ),
                timestamp_writes: stamp.as_ref().map(|stamp| stamp.writes()),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(
                target.viewport.x as f32,
                target.viewport.y as f32,
                target.viewport.width as f32,
                target.viewport.height as f32,
                0.,
                1.,
            );
            pass.set_scissor_rect(
                target.viewport.x,
                target.viewport.y,
                target.viewport.width,
                target.viewport.height,
            );
            // Fullscreen composition reads scene depth but never writes it.
            // Geometry then uses the same attachment for read-only occlusion.
            composite.encode(&mut pass, has_overlays);
            if has_transparent {
                pass.set_bind_group(0, &self.grid_bind_group, &[]);
                pass.set_bind_group(1, &self.effects.as_ref().unwrap().scene_group, &[]);
                draws += self.gltf_gpu.draw_prepared(
                    &mut pass,
                    &gltf_transparent,
                    key,
                    self.last_main_cache,
                );
            }
            if has_mesh_transparency {
                pass.set_bind_group(0, &self.grid_bind_group, &[]);
                pass.set_bind_group(1, &self.effects.as_ref().unwrap().scene_group, &[]);
                draws += self.draw_translucent(&mut pass, cache_index, target);
            }
            draws += self.draw_cut_feedback(&mut pass, cache_index, &rings, target);
            if has_neon {
                neon_stats = self.encode_neon_glow(&mut pass, cache_index);
                draws += neon_stats.0;
            }
            if has_helpers {
                pass.set_pipeline(&self.helper_pipelines[&helper_key]);
                pass.set_bind_group(0, &self.grid_bind_group, &[]);
                if self.draw_grid {
                    pass.set_vertex_buffer(0, self.grid_vertex_buffer.slice(..));
                    pass.draw(0..self.grid_vertex_count, 0..1);
                    draws += 1;
                }
                if self.draw_light_helpers {
                    pass.set_vertex_buffer(0, self.light_vertex_buffer.slice(..));
                    pass.draw(0..self.light_vertex_count, 0..1);
                    draws += 1;
                }
            }
        }
        self.neon_draws = neon_stats.0;
        self.neon_segments = neon_stats.1;
        self.draw_calls += draws;
        self.main_draw_calls += draws;
        Ok(encoder.finish())
    }
}
