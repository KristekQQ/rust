//! Portable glTF rendering: immutable rigid geometry is transformed on the GPU;
//! supported skins use joint palettes, while morph/arbitrary skins retain CPU deformation.
use super::data::as_bytes;
use crate::gltf_model::*;
use glam::{Mat4, Vec3, Vec4};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    tangent: [f32; 4],
    uvs: [[f32; 2]; 5],
    color: [f32; 4],
}
impl Vertex {
    fn attributes(texture_mask: u8, compact: bool) -> Vec<wgpu::VertexAttribute> {
        const A: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x4,3=>Float32x2,4=>Float32x2,5=>Float32x2,6=>Float32x2,7=>Float32x2,8=>Float32x4];
        let mut offset = 0;
        A.into_iter()
            .filter(|a| {
                a.shader_location < 2
                    || (a.shader_location == 2 && texture_mask & (1 << 2) != 0)
                    || a.shader_location == 8
                    || (a.shader_location >= 3
                        && texture_mask & (1 << (a.shader_location - 3)) != 0)
            })
            .map(|mut a| {
                if compact {
                    a.offset = offset;
                    offset += a.format.size();
                }
                a
            })
            .collect()
    }
    /// Lossless immutable GPU storage: omit only attributes the material's
    /// existing pipeline already omits. CPU query/deformation vertices stay 96B.
    fn compact(vertices: &[Self], texture_mask: u8) -> Vec<u8> {
        let attributes = Self::attributes(texture_mask, false);
        let stride = attributes
            .iter()
            .map(|a| a.format.size() as usize)
            .sum::<usize>();
        let mut packed = Vec::with_capacity(vertices.len() * stride);
        for vertex in vertices {
            let bytes = as_bytes(std::slice::from_ref(vertex));
            for a in &attributes {
                let start = a.offset as usize;
                packed.extend_from_slice(&bytes[start..start + a.format.size() as usize]);
            }
        }
        packed
    }
    fn layout(attributes: &[wgpu::VertexAttribute], compact: bool) -> wgpu::VertexBufferLayout<'_> {
        wgpu::VertexBufferLayout {
            array_stride: if compact {
                attributes.iter().map(|a| a.format.size()).sum()
            } else {
                96
            },
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes,
        }
    }
}
/// Seven instance attributes keep the common path within WebGL2's 16-attribute budget.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct RigidInstance {
    model: [[f32; 4]; 4],
    normal: [[f32; 4]; 3],
}
impl RigidInstance {
    fn from_uniform(u: &Uniform) -> Self {
        let mut normal = [u.normal[0], u.normal[1], u.normal[2]];
        normal[0][3] = u.transform[0];
        Self {
            model: u.model,
            normal,
        }
    }
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        const A: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![9=>Float32x4,10=>Float32x4,11=>Float32x4,12=>Float32x4,13=>Float32x4,14=>Float32x4,15=>Float32x4];
        wgpu::VertexBufferLayout {
            array_stride: 112,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &A,
        }
    }
}
struct InstancePage {
    buffer: Option<wgpu::Buffer>,
    material_buffer: wgpu::Buffer,
    bind: wgpu::BindGroup,
    material: Option<Uniform>,
    capacity: usize,
    last: Vec<u8>,
}
// Bounded pass/view-owned stream. Material uniforms remain per draw group;
// only the existing rigid instance records share one contiguous upload.
struct InstanceStream {
    buffer: wgpu::Buffer,
    // Bytes, because groups can mix 16B translation and 112B affine records.
    capacity: usize,
    last: Vec<u8>,
    scratch: Vec<u8>,
}
struct InstanceBinding {
    buffer: wgpu::Buffer,
    offset: u64,
    bytes: u64,
}
struct PreparedDraw {
    variant: Variant,
    textures: Rc<wgpu::BindGroup>,
    bind: wgpu::BindGroup,
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    skin: Option<wgpu::Buffer>,
    instance: Option<InstanceBinding>,
    count: u32,
    instances: u32,
    records: Vec<(u32, usize, usize)>,
}
#[derive(Default)]
pub struct PreparedDraws {
    draws: Vec<PreparedDraw>,
    pub upload_bytes: usize,
}

/// Frame-local indices only: no references or GPU resources survive a preparation call.
#[derive(Clone, Copy)]
struct QueueItem {
    model: usize,
    draw: usize,
}
#[derive(Default)]
struct QueueScratch {
    eligible: Vec<QueueItem>,
    model_ranges: Vec<std::ops::Range<usize>>,
    cursors: Vec<usize>,
    ordered: Vec<QueueItem>,
    groups: Vec<std::ops::Range<usize>>,
    occupied: crate::render_overlap::OrderedBounds,
    payload: Vec<u8>,
    translated: Vec<bool>,
}
impl QueueScratch {
    fn clear(&mut self) {
        self.eligible.clear();
        self.model_ranges.clear();
        self.cursors.clear();
        self.ordered.clear();
        self.groups.clear();
        self.occupied.clear();
        self.payload.clear();
        self.translated.clear();
    }
    fn capacities(&self) -> [usize; 8] {
        [
            self.eligible.capacity(),
            self.model_ranges.capacity(),
            self.cursors.capacity(),
            self.ordered.capacity(),
            self.groups.capacity(),
            self.occupied.capacity(),
            self.payload.capacity(),
            self.translated.capacity(),
        ]
    }
    fn bytes(&self) -> usize {
        let sizes = [
            std::mem::size_of::<QueueItem>(),
            std::mem::size_of::<std::ops::Range<usize>>(),
            std::mem::size_of::<usize>(),
            std::mem::size_of::<QueueItem>(),
            std::mem::size_of::<std::ops::Range<usize>>(),
            0,
            1,
            std::mem::size_of::<bool>(),
        ];
        self.capacities()
            .into_iter()
            .zip(sizes)
            .map(|(n, size)| n * size)
            .sum::<usize>()
            + self.occupied.bytes()
    }
    fn singleton(&mut self, item: QueueItem) {
        let start = self.ordered.len();
        self.ordered.push(item);
        self.groups.push(start..start + 1);
    }
    fn append_compatible(&mut self, item: QueueItem, models: &[(u32, &GpuModel)]) -> Option<usize> {
        let model = models[item.model].1;
        let draw = &model.draws[item.draw];
        // First failed condition, in the original short-circuit order. A first
        // group is not a rejected merge; no counter is emitted for that case.
        let rejection = self.groups.last().map(|range| {
            let previous = self.ordered[range.start];
            let other_model = models[previous.model].1;
            let other = &other_model.draws[previous.draw];
            if range.len() >= 1024 {
                Some(0)
            } else if previous.draw != item.draw {
                Some(1)
            } else if !Rc::ptr_eq(&other_model._shared, &model._shared) {
                Some(2)
            } else if !draw.rigid || !other.rigid {
                Some(3)
            } else if draw.negative != other.negative {
                Some(4)
            } else if draw.material != other.material {
                Some(5)
            } else if draw.primitive != other.primitive {
                Some(6)
            } else if draw.last_uniform.options != other.last_uniform.options {
                Some(7)
            } else {
                None
            }
        });
        if rejection == Some(None) {
            self.ordered.push(item);
            self.groups.last_mut().unwrap().end += 1;
        } else {
            self.singleton(item);
        }
        rejection.flatten()
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    base: [f32; 4],
    emissive: [f32; 4],
    factors: [f32; 4],
    uv: [u32; 4],
    extra: [f32; 4],
    options: [f32; 4],
    model: [[f32; 4]; 4],
    normal: [[f32; 4]; 4],
    transform: [f32; 4],
}
impl Uniform {
    fn affine_basis(&self) -> bool {
        self.model[0][3] == 0.0
            && self.model[1][3] == 0.0
            && self.model[2][3] == 0.0
            && self.model[3][3] == 1.0
            && self.model.iter().flatten().all(|x| x.is_finite())
            && self.normal[..3]
                .iter()
                .all(|n| n[..3].iter().all(|x| x.is_finite()))
    }
    fn same_basis(&self, other: &Self) -> bool {
        other.model[3][3] == 1.0
            && other.model[3][..3].iter().all(|x| x.is_finite())
            && self.model[..3] == other.model[..3]
            && (0..3).all(|i| self.normal[i][..3] == other.normal[i][..3])
            && self.transform[..2] == other.transform[..2]
    }
}
struct TranslationInstance;
impl TranslationInstance {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        const A: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![9=>Float32x4];
        wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &A,
        }
    }
}

struct Material {
    textures: Rc<wgpu::BindGroup>,
    texture_mask: u8,
    uniform: Uniform,
    alpha: AlphaMode,
    double_sided: bool,
}
const MAX_GPU_JOINTS: usize = 128;
const PALETTE_BYTES: u64 = (MAX_GPU_JOINTS * 64) as u64;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SkinInfluences {
    joints0: [u16; 4],
    joints1: [u16; 4],
    weights0: [f32; 4],
    weights1: [f32; 4],
}
impl SkinInfluences {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        const A: [wgpu::VertexAttribute; 4] =
            wgpu::vertex_attr_array![9=>Uint16x4,10=>Uint16x4,11=>Float32x4,12=>Float32x4];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &A,
        }
    }
}
struct PreparedSkin {
    vertices: Vec<Vertex>,
    influences: Vec<SkinInfluences>,
    mean_terms: Vec<(usize, Vec4)>,
    identity_mean: Vec4,
}
struct SharedSkin {
    vertex: wgpu::Buffer,
    influences: wgpu::Buffer,
    // Sum of w*[position,1]/vertex_count per joint, plus uninfluenced vertices.
    mean_terms: Vec<(usize, Vec4)>,
    identity_mean: Vec4,
}
struct SkinPalette {
    buffer: wgpu::Buffer,
    last: Vec<[[f32; 4]; 4]>,
}
struct Geometry {
    vertex: Option<wgpu::Buffer>,
    skin: Option<Rc<SharedSkin>>,
    index: wgpu::Buffer,
    count: u32,
    topology: wgpu::PrimitiveTopology,
    center: Vec3,
}
struct SharedAsset {
    asset: Arc<ModelAsset>,
    materials: Rc<Vec<Material>>,
    geometry: Vec<Option<Geometry>>,
}
/// Cumulative work counters; resource counts describe currently live shared assets.
#[derive(Clone, Copy, Debug, Default)]
pub struct GltfGpuStats {
    pub cpu_deformed_vertices: u64,
    pub rigid_upload_bytes: u64,
    pub skin_upload_bytes: u64,
    /// Included in uniform_upload_bytes as well.
    pub skin_palette_upload_bytes: u64,
    pub draw_calls: u64,
    pub pipeline_changes: u64,
    pub bind_group_changes: u64,
    pub uniform_upload_bytes: u64,
    pub buffer_upload_calls: u64,
    pub geometry_cache_hits: u64,
    pub material_cache_hits: u64,
    pub geometry_resources: usize,
    pub material_resources: usize,
    pub cached_assets: usize,
    pub primitive_tests: u64,
    pub primitive_culled: u64,
    pub root_bounds_tests: u64,
    pub root_fully_inside: u64,
    pub instanced_draw_calls: u64,
    pub translated_draw_calls: u64,
    pub translated_instances: u64,
    pub instances_drawn: u64,
    pub triangles_drawn: u64,
    pub overlap_tests: u64,
    pub overlap_node_tests: u64,
    /// Root-run barriers with an eligible draw: blend, unsupported, bounds, asset, overlap, spatial.
    pub root_batch_breaks: [u64; 6],
    /// First failed merge condition: cap, ordinal, asset, rigid, mirror, material, primitive, options.
    pub draw_batch_breaks: [u64; 8],
    pub instance_pages_created: u64,
    pub instance_buffers_created: u64,
    pub instance_bind_groups_created: u64,
    pub instance_pages_active: usize,
    pub instance_pages_retained: usize,
    pub instance_upload_bytes: u64,
    pub instance_buffer_bytes: usize,
    pub queue_scratch_growths: u64,
    pub queue_scratch_bytes: usize,
}
struct Draw {
    node: usize,
    primitive: usize,
    material: usize,
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    count: u32,
    topology: wgpu::PrimitiveTopology,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    center: Vec3,
    negative: bool,
    rigid: bool,
    skin: Option<Rc<SharedSkin>>,
    local_center: Vec3,
    last_uniform: Uniform,
    uploaded_uniform: Cell<Uniform>,
    bounds: Option<(Vec3, Vec3)>,
    scratch: Vec<Vertex>,
}
pub struct GpuModel {
    draws: Vec<Draw>,
    materials: Rc<Vec<Material>>,
    _shared: Rc<SharedAsset>,
    palettes: HashMap<usize, SkinPalette>,
    last_pose: Option<(u64, Mat4, [f32; 4])>,
    active: HashSet<usize>,
    pub opaque_reorderable: bool,
    reorderable_draws: bool,
    // Union of all active draw bounds, invalidated by the existing pose stamp.
    bounds: Option<(Vec3, Vec3)>,
    // Per-pose draw classification; not rebuilt for each eye/pass/frame.
    active_draws: [Vec<usize>; 2],
}
impl GpuModel {
    pub(crate) fn active_draw_index_bytes(&self) -> usize {
        self.active_draws
            .iter()
            .map(|v| v.capacity() * std::mem::size_of::<usize>())
            .sum()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PipelineKey {
    pub format: wgpu::TextureFormat,
    pub depth_format: wgpu::TextureFormat,
    pub reflected: bool,
    // Clip-plane use is independent of winding (e.g. environment views).
    pub clipped: bool,
    pub material_mask: bool,
    pub packed_depth: bool,
    pub shadow: bool,
    pub single_static_shadow: bool,
    pub(crate) light_class: crate::render_lights::LightClass,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Variant {
    key: PipelineKey,
    texture_mask: u8,
    opaque_unclipped: bool,
    topology: wgpu::PrimitiveTopology,
    blend: bool,
    double_sided: bool,
    negative: bool,
    skinned: bool,
    instanced: bool,
    translated: bool,
    compact_vertex: bool,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ShaderKey {
    texture_mask: u8,
    shadow: bool,
    opaque_unclipped: bool,
    skinned: bool,
    instanced: bool,
    translated: bool,
    single_static_shadow: bool,
    light_class: crate::render_lights::LightClass,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DrawRecord {
    object: u32,
    node: usize,
    primitive: usize,
    indices: u32,
    instances: u32,
    objects: Vec<u32>,
    view_slot: usize,
    shadow: bool,
    reflected: bool,
    blend: bool,
    material_mask: bool,
}
pub struct GltfGpu {
    global: wgpu::BindGroupLayout,
    effects: wgpu::BindGroupLayout,
    textures: wgpu::BindGroupLayout,
    material: wgpu::BindGroupLayout,
    skin_material: Option<wgpu::BindGroupLayout>,
    // 32 map classes x (generic color, opaque unclipped color, shadow).
    shaders: HashMap<ShaderKey, wgpu::ShaderModule>,
    empty_layout: wgpu::BindGroupLayout,
    empty_bind: wgpu::BindGroup,
    pipelines: HashMap<Variant, wgpu::RenderPipeline>,
    assets: RefCell<HashMap<usize, Weak<SharedAsset>>>,
    counters: Cell<GltfGpuStats>,
    diagnostics: Cell<bool>,
    draw_records: RefCell<Vec<DrawRecord>>,
    instance_pages: HashMap<(PipelineKey, usize), Vec<InstancePage>>,
    instance_streams: HashMap<(PipelineKey, usize), InstanceStream>,
    active_instance_pages: HashMap<(PipelineKey, usize), usize>,
    instancing_supported: bool,
    queue_scratch: QueueScratch,
}
impl GltfGpu {
    pub fn new(
        device: &wgpu::Device,
        global: &wgpu::BindGroupLayout,
        effects: &wgpu::BindGroupLayout,
    ) -> Self {
        let mut entries = Vec::new();
        for i in 0..5 {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: i * 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: i * 2 + 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
        }
        let textures = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glTF five material maps"),
            entries: &entries,
        });
        let material = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glTF material"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Uniform>() as u64),
                },
                count: None,
            }],
        });
        let limits = device.limits();
        let skin_material = (limits.max_vertex_attributes >= 13
            && limits.max_vertex_buffers >= 2
            && limits.max_uniform_buffer_binding_size >= PALETTE_BYTES)
            .then(|| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("glTF GPU skin material and palette"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(
                                    std::mem::size_of::<Uniform>() as u64,
                                ),
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::VERTEX,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: wgpu::BufferSize::new(PALETTE_BYTES),
                            },
                            count: None,
                        },
                    ],
                })
            });
        let empty_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glTF empty shadow effects"),
            entries: &[],
        });
        let empty_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glTF empty shadow effects"),
            layout: &empty_layout,
            entries: &[],
        });
        Self {
            global: global.clone(),
            effects: effects.clone(),
            textures,
            material,
            skin_material,
            shaders: HashMap::new(),
            empty_layout,
            empty_bind,
            pipelines: HashMap::new(),
            assets: RefCell::new(HashMap::new()),
            counters: Cell::new(GltfGpuStats::default()),
            diagnostics: Cell::new(false),
            draw_records: RefCell::new(Vec::new()),
            instance_pages: HashMap::new(),
            instance_streams: HashMap::new(),
            active_instance_pages: HashMap::new(),
            queue_scratch: QueueScratch::default(),
            instancing_supported: limits.max_vertex_attributes >= 16
                && limits.max_vertex_buffers >= 2,
        }
    }
    pub fn upload(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        asset: &Arc<ModelAsset>,
    ) -> Result<GpuModel, String> {
        let identity = Arc::as_ptr(asset) as usize;
        let shared = {
            let mut cache = self.assets.borrow_mut();
            cache.retain(|_, entry| entry.strong_count() > 0);
            cache.get(&identity).and_then(Weak::upgrade)
        };
        if let Some(shared) = shared {
            let mut stats = self.counters.get();
            stats.geometry_cache_hits += shared.geometry.iter().flatten().count() as u64;
            stats.material_cache_hits += shared.materials.len() as u64;
            self.counters.set(stats);
            return Ok(self.create_model(device, shared));
        }
        let limits = device.limits();
        for image in &asset.images {
            if image.width == 0
                || image.height == 0
                || image.width > limits.max_texture_dimension_2d
                || image.height > limits.max_texture_dimension_2d
                || image.rgba.len() as u64 != image.width as u64 * image.height as u64 * 4
            {
                return Err("glTF image exceeds device limits or has invalid pixels".into());
            }
        }
        for p in &asset.primitives {
            if p.vertices.is_empty()
                || p.vertices.end > asset.vertices.len()
                || p.vertices.len() as u64 * 96 > limits.max_buffer_size
                || p.indices.len() as u64 * 12 > limits.max_buffer_size
                || p.indices.iter().any(|i| *i as usize >= p.vertices.len())
            {
                return Err(
                    "glTF primitive exceeds device buffer limits or has invalid indices".into(),
                );
            }
        }
        // Bound the aggregate allocation before creating any GPU resources.
        let mut image_roles = std::collections::HashSet::new();
        let mut budget = 0u64;
        for material in &asset.materials {
            for (slot, srgb) in [
                (material.base_color_texture, true),
                (material.metallic_roughness_texture, false),
                (material.normal_texture, false),
                (material.occlusion_texture, false),
                (material.emissive_texture, true),
            ] {
                if let Some(slot) = slot {
                    let texture = asset
                        .textures
                        .get(slot.texture)
                        .ok_or("Invalid material texture")?;
                    let image = asset
                        .images
                        .get(texture.image)
                        .ok_or("Invalid texture image")?;
                    if texture.sampler.is_some_and(|s| s >= asset.samplers.len()) {
                        return Err("Invalid texture sampler".into());
                    }
                    // Per texture because samplers may differ even for one image.
                    if image_roles.insert((slot.texture, srgb)) {
                        budget = budget.saturating_add(image.rgba.len() as u64 * 2);
                    }
                }
            }
        }
        for node in &asset.nodes {
            if let Some(mesh) = node.mesh {
                for &primitive in &asset
                    .meshes
                    .get(mesh)
                    .ok_or("Invalid node mesh")?
                    .primitives
                {
                    let primitive = asset
                        .primitives
                        .get(primitive)
                        .ok_or("Invalid mesh primitive")?;
                    if primitive
                        .material
                        .is_some_and(|m| m >= asset.materials.len())
                    {
                        return Err("Invalid primitive material".into());
                    }
                    budget = budget.saturating_add(
                        primitive.vertices.len() as u64 * 96
                            + primitive.indices.len().max(primitive.vertices.len()) as u64 * 12
                            + std::mem::size_of::<Uniform>() as u64,
                    );
                    if self.skin_material.is_some()
                        && node
                            .skin
                            .and_then(|i| asset.skins.get(i))
                            .is_some_and(|s| s.joints.len() <= MAX_GPU_JOINTS)
                        && primitive.morph_targets.is_empty()
                    {
                        // Existing budget covers source vertices; add influence
                        // attributes and one conservative palette per draw.
                        budget = budget
                            .saturating_add(primitive.vertices.len() as u64 * 48 + PALETTE_BYTES);
                    }
                }
            }
        }
        if budget > 512 * 1024 * 1024 {
            return Err("glTF resources exceed aggregate GPU upload budget".into());
        }
        let mut materials = Vec::new();
        let mut texture_cache: HashMap<(usize, bool), (wgpu::TextureView, wgpu::Sampler)> =
            HashMap::new();
        let mut binding_cache: HashMap<[(usize, bool); 5], Rc<wgpu::BindGroup>> = HashMap::new();
        if asset
            .images
            .iter()
            .map(|i| i.rgba.len() as u64)
            .sum::<u64>()
            > 256 * 1024 * 1024
        {
            return Err("glTF images exceed GPU upload budget".into());
        }
        for source in asset
            .materials
            .iter()
            .map(Some)
            .chain(std::iter::once(None))
        {
            let slots = source
                .map(|m| {
                    [
                        m.base_color_texture,
                        m.metallic_roughness_texture,
                        m.normal_texture,
                        m.occlusion_texture,
                        m.emissive_texture,
                    ]
                })
                .unwrap_or([None; 5]);
            let texture_mask = slots.iter().enumerate().fold(0u8, |mask, (i, slot)| {
                mask | if slot.is_some() { 1 << i } else { 0 }
            });
            // Only texture/sampler resources identify group 2. Colors, factors,
            // UV sets and alpha coverage remain independent material uniforms.
            let binding_keys: [(usize, bool); 5] = std::array::from_fn(|i| {
                (
                    slots[i]
                        .map(|v| v.texture)
                        .unwrap_or(usize::MAX - usize::from(i == 2)),
                    i == 0 || i == 4,
                )
            });
            let mut views = Vec::new();
            let mut samplers = Vec::new();
            for (i, slot) in slots.iter().enumerate() {
                let (texture_identity, srgb) = binding_keys[i];
                let cache_key = (texture_identity, srgb);
                if let Some((view, sampler)) = texture_cache.get(&cache_key) {
                    views.push(view.clone());
                    samplers.push(sampler.clone());
                    continue;
                }
                let (width, height, pixels, sampler) = if let Some(slot) = slot {
                    let tex = asset
                        .textures
                        .get(slot.texture)
                        .ok_or("Invalid material texture")?;
                    let img = asset.images.get(tex.image).ok_or("Invalid texture image")?;
                    (
                        img.width,
                        img.height,
                        img.rgba.clone(),
                        tex.sampler.and_then(|s| asset.samplers.get(s)),
                    )
                } else {
                    (
                        1,
                        1,
                        if i == 2 {
                            vec![128, 128, 255, 255]
                        } else {
                            vec![255; 4]
                        },
                        None,
                    )
                };
                let levels = 32 - width.max(height).leading_zeros();
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("glTF material texture"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: levels,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: if srgb {
                        wgpu::TextureFormat::Rgba8UnormSrgb
                    } else {
                        wgpu::TextureFormat::Rgba8Unorm
                    },
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                let (mut w, mut h, mut rgba) = (width, height, pixels);
                for level in 0..levels {
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &texture,
                            mip_level: level,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &rgba,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(w * 4),
                            rows_per_image: Some(h),
                        },
                        wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                    );
                    if level + 1 < levels {
                        let nw = (w / 2).max(1);
                        let nh = (h / 2).max(1);
                        let mut next = vec![0; nw as usize * nh as usize * 4];
                        for y in 0..nh {
                            for x in 0..nw {
                                for c in 0..4 {
                                    let mut sum = 0.0f32;
                                    for dy in 0..2 {
                                        for dx in 0..2 {
                                            let value = rgba[(((y * 2 + dy).min(h - 1) * w
                                                + (x * 2 + dx).min(w - 1))
                                                * 4
                                                + c)
                                                as usize]
                                                as f32
                                                / 255.0;
                                            sum += if srgb && c < 3 {
                                                if value <= 0.04045 {
                                                    value / 12.92
                                                } else {
                                                    ((value + 0.055) / 1.055).powf(2.4)
                                                }
                                            } else {
                                                value
                                            };
                                        }
                                    }
                                    next[((y * nw + x) * 4 + c) as usize] = {
                                        let value = sum / 4.0;
                                        let encoded = if srgb && c < 3 {
                                            if value <= 0.0031308 {
                                                value * 12.92
                                            } else {
                                                1.055 * value.powf(1.0 / 2.4) - 0.055
                                            }
                                        } else {
                                            value
                                        };
                                        (encoded.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
                                    };
                                }
                            }
                        }
                        rgba = next;
                        w = nw;
                        h = nh;
                    }
                }
                views.push(texture.create_view(&Default::default()));
                let min = sampler
                    .and_then(|s| s.min_filter)
                    .unwrap_or(FilterMode::LinearMipmapLinear);
                let mag = sampler
                    .and_then(|s| s.mag_filter)
                    .unwrap_or(FilterMode::Linear);
                let filter = |f| {
                    if matches!(
                        f,
                        FilterMode::Nearest
                            | FilterMode::NearestMipmapNearest
                            | FilterMode::NearestMipmapLinear
                    ) {
                        wgpu::FilterMode::Nearest
                    } else {
                        wgpu::FilterMode::Linear
                    }
                };
                samplers.push(device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("glTF sampler"),
                    address_mode_u: wrap(sampler.map(|s| s.wrap_s)),
                    address_mode_v: wrap(sampler.map(|s| s.wrap_t)),
                    mag_filter: filter(mag),
                    min_filter: filter(min),
                    mipmap_filter: if matches!(
                        min,
                        FilterMode::NearestMipmapLinear | FilterMode::LinearMipmapLinear
                    ) {
                        wgpu::MipmapFilterMode::Linear
                    } else {
                        wgpu::MipmapFilterMode::Nearest
                    },
                    lod_max_clamp: if matches!(min, FilterMode::Nearest | FilterMode::Linear) {
                        0.0
                    } else {
                        32.0
                    },
                    ..Default::default()
                }));
                texture_cache.insert(
                    cache_key,
                    (
                        views.last().unwrap().clone(),
                        samplers.last().unwrap().clone(),
                    ),
                );
            }
            let mut entries = Vec::new();
            for i in 0..5 {
                entries.push(wgpu::BindGroupEntry {
                    binding: i * 2,
                    resource: wgpu::BindingResource::TextureView(&views[i as usize]),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: i * 2 + 1,
                    resource: wgpu::BindingResource::Sampler(&samplers[i as usize]),
                });
            }
            let textures = binding_cache
                .entry(binding_keys)
                .or_insert_with(|| {
                    Rc::new(device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("glTF maps"),
                        layout: &self.textures,
                        entries: &entries,
                    }))
                })
                .clone();
            let alpha = source.map(|m| m.alpha_mode).unwrap_or(AlphaMode::Opaque);
            let double_sided = source.is_some_and(|m| m.double_sided);
            let uniform = Uniform {
                base: source.map(|m| m.base_color_factor).unwrap_or([1.0; 4]),
                emissive: source
                    .map(|m| {
                        [
                            m.emissive_factor[0],
                            m.emissive_factor[1],
                            m.emissive_factor[2],
                            slots[4].map(|s| s.tex_coord as f32).unwrap_or(0.0),
                        ]
                    })
                    .unwrap_or([0.0; 4]),
                factors: source
                    .map(|m| {
                        [
                            m.metallic_factor,
                            m.roughness_factor,
                            m.normal_scale,
                            m.occlusion_strength,
                        ]
                    })
                    .unwrap_or([1.0; 4]),
                uv: std::array::from_fn(|i| slots[i].map(|s| s.tex_coord).unwrap_or(0)),
                extra: [
                    source.map(|m| m.alpha_cutoff).unwrap_or(0.5),
                    match alpha {
                        AlphaMode::Opaque => 0.0,
                        AlphaMode::Mask => 1.0,
                        AlphaMode::Blend => 2.0,
                    },
                    if double_sided { 1.0 } else { 0.0 },
                    0.0,
                ],
                options: [0.0; 4],
                model: Mat4::IDENTITY.to_cols_array_2d(),
                normal: Mat4::IDENTITY.to_cols_array_2d(),
                transform: [1.0, 0.0, 0.0, 0.0],
            };
            materials.push(Material {
                textures,
                texture_mask,
                uniform,
                alpha,
                double_sided,
            });
        }
        let mut geometry = Vec::with_capacity(asset.primitives.len());
        let mut stats = self.counters.get();
        for (primitive, p) in asset.primitives.iter().enumerate() {
            if !asset.nodes.iter().any(|n| {
                n.mesh
                    .is_some_and(|m| asset.meshes[m].primitives.contains(&primitive))
            }) {
                geometry.push(None);
                continue;
            }
            let (indices, topology) = indices(p);
            let has_rigid_node = p.morph_targets.is_empty()
                && asset.nodes.iter().any(|n| {
                    n.skin.is_none()
                        && n.mesh
                            .is_some_and(|m| asset.meshes[m].primitives.contains(&primitive))
                });
            let slots = material_slots(asset, p);
            let mut center = Vec3::ZERO;
            let vertex = if has_rigid_node {
                let vertices: Vec<_> = asset.vertices[p.vertices.clone()]
                    .iter()
                    .map(|v| {
                        center += Vec3::from(v.position);
                        // Match deformed_vertices normalization and Gram-Schmidt.
                        let n = Vec3::from(v.normal).try_normalize().unwrap_or(Vec3::Y);
                        let t = Vec3::from_slice(&v.tangent[..3]);
                        let t = (t - n * n.dot(t)).try_normalize().unwrap_or(Vec3::ZERO);
                        packed_vertex(v, Vec3::from(v.position), n, t, 1.0, &slots)
                    })
                    .collect();
                center /= vertices.len().max(1) as f32;
                let texture_mask =
                    materials[p.material.unwrap_or(asset.materials.len())].texture_mask;
                let packed = Vertex::compact(&vertices, texture_mask);
                stats.rigid_upload_bytes += (packed.len() + indices.len() * 4) as u64;
                Some(
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("glTF shared rigid vertices"),
                        contents: &packed,
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                )
            } else {
                None
            };
            let index = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("glTF shared indices"),
                contents: as_bytes(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            let has_skin_node = self.skin_material.is_some()
                && p.morph_targets.is_empty()
                && asset.nodes.iter().any(|n| {
                    n.mesh
                        .is_some_and(|m| asset.meshes[m].primitives.contains(&primitive))
                        && n.skin
                            .and_then(|i| asset.skins.get(i))
                            .is_some_and(|skin| skin.joints.len() <= MAX_GPU_JOINTS)
                });
            let skin = if has_skin_node {
                prepare_skin(asset, p, &slots).map(|prepared| {
                    let PreparedSkin {
                        vertices,
                        influences,
                        mean_terms,
                        identity_mean,
                    } = prepared;
                    stats.skin_upload_bytes += (vertices.len() * 96
                        + influences.len() * std::mem::size_of::<SkinInfluences>())
                        as u64;
                    if vertex.is_none() {
                        stats.skin_upload_bytes += (indices.len() * 4) as u64;
                    }
                    Rc::new(SharedSkin {
                        vertex: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("glTF shared skin source vertices"),
                            contents: as_bytes(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        }),
                        influences: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("glTF shared eight influences"),
                            contents: as_bytes(&influences),
                            usage: wgpu::BufferUsages::VERTEX,
                        }),
                        mean_terms,
                        identity_mean,
                    })
                })
            } else {
                None
            };
            geometry.push(Some(Geometry {
                vertex,
                skin,
                index,
                count: indices.len() as u32,
                topology,
                center,
            }));
        }
        self.counters.set(stats);
        let shared = Rc::new(SharedAsset {
            asset: asset.clone(),
            materials: Rc::new(materials),
            geometry,
        });
        self.assets
            .borrow_mut()
            .insert(identity, Rc::downgrade(&shared));
        Ok(self.create_model(device, shared))
    }
    /// Snapshot counters without retaining any asset or GPU resource.
    pub fn stats(&self) -> GltfGpuStats {
        let mut stats = self.counters.get();
        for shared in self.assets.borrow().values().filter_map(Weak::upgrade) {
            stats.cached_assets += 1;
            stats.geometry_resources += shared.geometry.iter().flatten().count();
            stats.material_resources += shared.materials.len();
        }
        stats.queue_scratch_bytes = self.queue_scratch.bytes();
        stats.instance_pages_active = self.active_instance_pages.values().sum();
        stats.instance_pages_retained = self.instance_pages.values().map(Vec::len).sum();
        stats.instance_buffer_bytes = self
            .instance_pages
            .values()
            .flatten()
            .map(|p| p.buffer.as_ref().map_or(0, |_| p.capacity) + std::mem::size_of::<Uniform>())
            .sum::<usize>()
            + self
                .instance_streams
                .values()
                .map(|s| s.capacity)
                .sum::<usize>();
        stats
    }
    fn create_model(&self, device: &wgpu::Device, shared: Rc<SharedAsset>) -> GpuModel {
        let asset = &shared.asset;
        let materials = shared.materials.clone();
        let mut palettes: HashMap<usize, SkinPalette> = HashMap::new();
        let mut draws = Vec::new();
        for (node, n) in asset.nodes.iter().enumerate() {
            let Some(mesh) = n.mesh else { continue };
            for &primitive in &asset.meshes[mesh].primitives {
                let p = &asset.primitives[primitive];
                let geometry = shared.geometry[primitive]
                    .as_ref()
                    .expect("referenced primitive geometry");
                let rigid = n.skin.is_none() && p.morph_targets.is_empty();
                let skin = n
                    .skin
                    .and_then(|i| asset.skins.get(i))
                    .filter(|skin| skin.joints.len() <= MAX_GPU_JOINTS)
                    .and_then(|_| geometry.skin.clone());
                if skin.is_some() && !palettes.contains_key(&node) {
                    let initial = vec![Mat4::IDENTITY.to_cols_array_2d(); MAX_GPU_JOINTS];
                    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("glTF node skin palette"),
                        contents: as_bytes(&initial),
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    });
                    palettes.insert(
                        node,
                        SkinPalette {
                            buffer,
                            last: Vec::with_capacity(MAX_GPU_JOINTS),
                        },
                    );
                }
                let material = p.material.unwrap_or(asset.materials.len());
                let vertex = if let Some(skin) = &skin {
                    skin.vertex.clone()
                } else if rigid {
                    geometry
                        .vertex
                        .as_ref()
                        .expect("rigid primitive buffer")
                        .clone()
                } else {
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("glTF CPU deformed vertices"),
                        size: p.vertices.len() as u64 * 96,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    })
                };
                let initial = materials[material].uniform;
                let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("glTF material and transform"),
                    contents: as_bytes(&[initial]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });
                let mut entries = vec![wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                }];
                if let Some(palette) = palettes.get(&node).filter(|_| skin.is_some()) {
                    entries.push(wgpu::BindGroupEntry {
                        binding: 1,
                        resource: palette.buffer.as_entire_binding(),
                    });
                }
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("glTF model material"),
                    layout: if skin.is_some() {
                        self.skin_material.as_ref().expect("supported skin layout")
                    } else {
                        &self.material
                    },
                    entries: &entries,
                });
                draws.push(Draw {
                    node,
                    primitive,
                    material,
                    vertex,
                    index: geometry.index.clone(),
                    count: geometry.count,
                    topology: geometry.topology,
                    uniform,
                    bind,
                    center: Vec3::ZERO,
                    negative: false,
                    rigid,
                    skin,
                    local_center: geometry.center,
                    last_uniform: initial,
                    uploaded_uniform: Cell::new(initial),
                    bounds: None,
                    scratch: Vec::new(),
                });
            }
        }
        let mut stats = self.counters.get();
        stats.uniform_upload_bytes += (draws.len() * std::mem::size_of::<Uniform>()) as u64
            + palettes.len() as u64 * PALETTE_BYTES;
        stats.skin_palette_upload_bytes += palettes.len() as u64 * PALETTE_BYTES;
        self.counters.set(stats);
        let reorderable_draws = draws
            .iter()
            .all(|d| d.rigid && materials[d.material].alpha != AlphaMode::Blend);
        let active_draws = std::array::from_fn(|index| {
            Vec::with_capacity(
                draws
                    .iter()
                    .filter(|d| (materials[d.material].alpha == AlphaMode::Blend) == (index == 1))
                    .count(),
            )
        });
        GpuModel {
            active_draws,
            opaque_reorderable: false,
            reorderable_draws,
            draws,
            materials,
            _shared: shared,
            palettes,
            last_pose: None,
            active: HashSet::new(),
            bounds: None,
        }
    }
    #[allow(clippy::too_many_arguments)] // Explicit GPU/FFI parameter layout.
    pub(crate) fn update(
        &self,
        queue: &wgpu::Queue,
        model: &mut GpuModel,
        asset: &ModelAsset,
        bounds_cache: &crate::gltf_bounds::RenderBoundsCache,
        pose: &ModelPose,
        revision: u64,
        root: Mat4,
        options: [f32; 4],
    ) -> Result<usize, String> {
        let _scope = super::profiler::cpu_scope("gltf.update");
        let stamp = (revision, root, options);
        if model.last_pose.as_ref() == Some(&stamp) {
            return Ok(0);
        }
        model.active.clear();
        model.active.extend(pose.active_nodes.iter().copied());
        let mut stats = self.counters.get();
        let mut bytes = 0;
        for (&node, palette) in &mut model.palettes {
            if !model.active.contains(&node) {
                continue;
            }
            let matrices = pose
                .skin_matrices
                .get(node)
                .ok_or("Pose skin node out of range")?;
            if matrices.len() > MAX_GPU_JOINTS {
                return Err("GPU skin palette exceeds supported size".into());
            }
            if palette.last.len() != matrices.len()
                || !palette
                    .last
                    .iter()
                    .zip(matrices)
                    .all(|(a, b)| *a == b.to_cols_array_2d())
            {
                palette.last.clear();
                palette
                    .last
                    .extend(matrices.iter().map(|matrix| matrix.to_cols_array_2d()));
                if !palette.last.is_empty() {
                    queue.write_buffer(&palette.buffer, 0, as_bytes(&palette.last));
                    stats.buffer_upload_calls += 1;
                    let uploaded = palette.last.len() * 64;
                    bytes += uploaded;
                    stats.uniform_upload_bytes += uploaded as u64;
                    stats.skin_palette_upload_bytes += uploaded as u64;
                }
            }
        }
        for draw in &mut model.draws {
            if !model.active.contains(&draw.node) {
                continue;
            }
            let primitive = &asset.primitives[draw.primitive];
            let matrix = root * crate::gltf_animation::node_render_matrix(pose, draw.node);
            let inverse = matrix.inverse().transpose();
            let normal = if inverse.is_finite() { inverse } else { matrix };
            draw.negative = matrix.determinant() < 0.0;
            draw.bounds =
                bounds_cache.primitive_bounds(asset, pose, draw.node, draw.primitive, root);
            if draw.rigid {
                // Exact affine vertex mean, rather than the bounds center.
                draw.center = matrix.transform_point3(draw.local_center);
            } else if let Some(skin) = &draw.skin {
                let matrices = &pose.skin_matrices[draw.node];
                let mut mean = skin.identity_mean;
                for &(joint, term) in &skin.mean_terms {
                    mean += *matrices
                        .get(joint)
                        .ok_or("Vertex joint index out of skin range")?
                        * term;
                }
                draw.center = matrix.transform_point3(mean.truncate());
            } else {
                let _deform_scope = super::profiler::cpu_scope("gltf.deform");
                let slots = material_slots(asset, primitive);
                draw.scratch.clear();
                let mut center = Vec3::ZERO;
                for v in crate::gltf_animation::deformed_vertices(
                    asset,
                    pose,
                    draw.node,
                    draw.primitive,
                )? {
                    let pos = matrix.transform_point3(Vec3::from(v.position));
                    let n = normal
                        .transform_vector3(Vec3::from(v.normal))
                        .normalize_or_zero();
                    let t = matrix
                        .transform_vector3(Vec3::from_slice(&v.tangent[..3]))
                        .normalize_or_zero();
                    center += pos;
                    draw.scratch.push(packed_vertex(
                        &v,
                        pos,
                        n,
                        t,
                        if draw.negative { -1.0 } else { 1.0 },
                        &slots,
                    ));
                }
                draw.center = center / draw.scratch.len().max(1) as f32;
                queue.write_buffer(&draw.vertex, 0, as_bytes(&draw.scratch));
                stats.buffer_upload_calls += 1;
                bytes += draw.scratch.len() * 96;
                stats.cpu_deformed_vertices += draw.scratch.len() as u64;
            }
            let mut uniform = model.materials[draw.material].uniform;
            if draw.rigid || draw.skin.is_some() {
                uniform.model = matrix.to_cols_array_2d();
                uniform.normal = normal.to_cols_array_2d();
                uniform.transform = [if draw.negative { -1.0 } else { 1.0 }, 1.0, 0.0, 0.0];
            }
            uniform.options = options;
            uniform.options[3] = if primitive.flat_normals { 1.0 } else { 0.0 };
            uniform.extra[3] = if matches!(
                draw.topology,
                wgpu::PrimitiveTopology::PointList
                    | wgpu::PrimitiveTopology::LineList
                    | wgpu::PrimitiveTopology::LineStrip
            ) {
                1.0
            } else {
                0.0
            };
            if uniform != draw.last_uniform {
                draw.last_uniform = uniform;
            }
        }
        self.counters.set(stats);
        for list in &mut model.active_draws {
            list.clear();
        }
        for (ordinal, d) in model.draws.iter().enumerate() {
            if model.active.contains(&d.node) {
                let blend = model.materials[d.material].alpha == AlphaMode::Blend;
                model.active_draws[usize::from(blend)].push(ordinal);
            }
        }
        model.bounds = crate::render_overlap::union_bounds(
            model
                .active_draws
                .iter()
                .flatten()
                .map(|&i| model.draws[i].bounds),
        );
        model.last_pose = Some(stamp);
        Ok(bytes)
    }
    fn ensure_pipeline(&mut self, device: &wgpu::Device, variant: Variant) {
        if self.pipelines.contains_key(&variant) {
            return;
        }
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glTF pipeline layout"),
            bind_group_layouts: &[
                Some(&self.global),
                Some(if variant.key.shadow {
                    &self.empty_layout
                } else {
                    &self.effects
                }),
                Some(&self.textures),
                Some(if variant.skinned {
                    self.skin_material.as_ref().expect("supported skin layout")
                } else {
                    &self.material
                }),
            ],
            immediate_size: 0,
        });
        let shader_key = ShaderKey {
            texture_mask: variant.texture_mask,
            shadow: variant.key.shadow,
            opaque_unclipped: variant.opaque_unclipped,
            skinned: variant.skinned,
            instanced: variant.instanced,
            translated: variant.translated,
            single_static_shadow: variant.key.single_static_shadow,
            light_class: variant.key.light_class,
        };
        let shader = self.shaders.entry(shader_key).or_insert_with(|| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("glTF material-class shader"),
                source: wgpu::ShaderSource::Wgsl(material_shader_source(shader_key).into()),
            })
        });
        let mut targets = Vec::new();
        if !variant.key.shadow {
            targets.push(Some(wgpu::ColorTargetState {
                format: variant.key.format,
                blend: Some(if variant.blend {
                    wgpu::BlendState::ALPHA_BLENDING
                } else {
                    wgpu::BlendState::REPLACE
                }),
                write_mask: wgpu::ColorWrites::ALL,
            }));
            if variant.key.packed_depth {
                targets.push(Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                }));
            }
        }
        let attributes = Vertex::attributes(variant.texture_mask, variant.compact_vertex);
        let vertex_buffers = [
            Some(Vertex::layout(&attributes, variant.compact_vertex)),
            Some(if variant.translated {
                TranslationInstance::layout()
            } else if variant.instanced {
                RigidInstance::layout()
            } else {
                SkinInfluences::layout()
            }),
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("glTF PBR variant"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: if variant.skinned || variant.instanced {
                    &vertex_buffers
                } else {
                    &vertex_buffers[..1]
                },
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(if variant.key.shadow {
                    "fs_shadow"
                } else if variant.key.packed_depth {
                    "fs_postprocess"
                } else if variant.key.material_mask {
                    "fs_material_mask"
                } else {
                    "fs_main"
                }),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            primitive: wgpu::PrimitiveState {
                topology: variant.topology,
                strip_index_format: if matches!(
                    variant.topology,
                    wgpu::PrimitiveTopology::LineStrip | wgpu::PrimitiveTopology::TriangleStrip
                ) {
                    Some(wgpu::IndexFormat::Uint32)
                } else {
                    None
                },
                cull_mode: if variant.double_sided || variant.key.shadow {
                    None
                } else {
                    Some(if variant.key.reflected ^ variant.negative {
                        wgpu::Face::Back
                    } else {
                        wgpu::Face::Front
                    })
                },
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: variant.key.depth_format,
                depth_write_enabled: Some(variant.key.shadow || !variant.blend),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: if variant.key.shadow {
                    wgpu::DepthBiasState {
                        constant: 2,
                        slope_scale: 2.0,
                        clamp: 0.0,
                    }
                } else {
                    Default::default()
                },
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.insert(variant, pipeline);
    }
    pub fn begin_frame(&self) {
        self.draw_records.borrow_mut().clear();
    }
    pub fn set_diagnostics(&self, enabled: bool) {
        self.diagnostics.set(enabled);
        if !enabled {
            self.draw_records.borrow_mut().clear();
        }
    }
    pub fn draw_diagnostics(&self) -> serde_json::Value {
        serde_json::json!({"enabled":self.diagnostics.get(),"draws":*self.draw_records.borrow(),"limit":20000})
    }
    /// Pages contain transforms only. Drop cached pages as soon as all models disappear.
    pub fn clear_instance_pages(&mut self) {
        self.instance_pages.clear();
        self.instance_streams.clear();
        self.active_instance_pages.clear();
        self.queue_scratch = QueueScratch::default();
    }
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_draws<'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        models: impl Iterator<Item = (u32, &'a GpuModel)>,
        key: PipelineKey,
        blend: bool,
        camera: Vec3,
        view_slot: usize,
        view_projection: Mat4,
        culling: bool,
    ) -> PreparedDraws {
        let _scope = super::profiler::cpu_scope("gltf.queue");
        let upload_before = self.counters.get().uniform_upload_bytes;
        // Lighting changes the program, never the instance payload or material
        // layout. Keep one stream/page set across all light program classes.
        let storage_key = PipelineKey {
            light_class: Default::default(),
            ..key
        };
        let models: Vec<_> = models.collect();
        let frustum = crate::visibility::Frustum::from_view_projection(view_projection);
        let mut stats = self.counters.get();
        let mut scratch = std::mem::take(&mut self.queue_scratch);
        let capacities_before = scratch.capacities();
        scratch.clear();
        for (model_index, &(_, model)) in models.iter().enumerate() {
            let start = scratch.eligible.len();
            let fully_inside = culling
                && model.bounds.is_some_and(|(min, max)| {
                    stats.root_bounds_tests += 1;
                    if frustum.fully_contains_aabb((min + max) * 0.5, (max - min) * 0.5) {
                        stats.root_fully_inside += 1;
                        true
                    } else {
                        false
                    }
                });
            for &ordinal in &model.active_draws[usize::from(blend)] {
                let d = &model.draws[ordinal];
                if culling && !fully_inside {
                    if let Some((min, max)) = d.bounds {
                        stats.primitive_tests += 1;
                        if !frustum.intersects_aabb((min + max) * 0.5, (max - min) * 0.5) {
                            stats.primitive_culled += 1;
                            continue;
                        }
                    }
                }
                scratch.eligible.push(QueueItem {
                    model: model_index,
                    draw: ordinal,
                });
            }
            scratch.model_ranges.push(start..scratch.eligible.len());
            scratch.cursors.push(start);
        }
        self.counters.set(stats);
        // `reflected` is a winding flag (also set by authored cameras), not
        // the pass identity. Real planar reflection uses `clipped`.
        // Keep auxiliary passes on their existing ordering path. The authored
        // policy permits overlapping opaque roots in the color pass; it does
        // not make blend, skin/morph or coplanar content automatically safe.
        let reorderable = |m: &GpuModel| {
            !blend && !key.shadow && !key.clipped && m.opaque_reorderable && m.reorderable_draws
        };
        let mut overlap_tests = 0;
        let mut overlap_node_tests = 0;
        let mut root_batch_breaks = [0; 6];
        let mut draw_batch_breaks = [0; 8];
        let mut start = 0;
        while start < models.len() {
            let mut end = start + 1;
            let may_reorder = reorderable(models[start].1);
            let bounds = models[start].1.bounds;
            let mut depth_window = bounds
                .and_then(|b| crate::render_batch::DepthWindow::new(view_projection, camera, b));
            let mut run_has_draws = !scratch.model_ranges[start].is_empty();
            scratch.occupied.clear();
            if let Some(b) = bounds {
                scratch.occupied.push(b);
            }
            if blend {
                root_batch_breaks[0] += u64::from(run_has_draws);
            } else if !self.instancing_supported {
                root_batch_breaks[1] += u64::from(run_has_draws);
            } else if bounds.is_none() {
                root_batch_breaks[2] += u64::from(run_has_draws);
            } else {
                while end < models.len() {
                    if !Rc::ptr_eq(&models[start].1._shared, &models[end].1._shared) {
                        root_batch_breaks[3] += u64::from(run_has_draws);
                        break;
                    }
                    // An order-sensitive root is a barrier, never jumped over.
                    if reorderable(models[end].1) != may_reorder {
                        break;
                    }
                    if may_reorder {
                        // A nearby compatible run is not split by root count.
                        // Limit its camera-relative depth span instead, so part-
                        // major submission cannot span an arbitrarily deep scene.
                        if !models[end]
                            .1
                            .bounds
                            .is_some_and(|b| depth_window.as_mut().is_some_and(|w| w.include(b)))
                        {
                            root_batch_breaks[5] += u64::from(run_has_draws);
                            break;
                        }
                        run_has_draws |= !scratch.model_ranges[end].is_empty();
                        end += 1;
                        continue;
                    }
                    let Some(b) = models[end].1.bounds else {
                        root_batch_breaks[2] += u64::from(run_has_draws);
                        break;
                    };
                    let probe = scratch.occupied.probe(b);
                    overlap_tests += probe.leaf_tests;
                    overlap_node_tests += probe.node_tests;
                    if probe.overlap {
                        root_batch_breaks[4] += u64::from(run_has_draws);
                        break;
                    }
                    scratch.occupied.push(b);
                    run_has_draws |= !scratch.model_ranges[end].is_empty();
                    end += 1;
                }
            }
            if end - start > 1 {
                // Shared assets create identical draw ordinals. Merge the already
                // ordered per-model lists by ordinal, then model, without a tree
                // or an allocation per column/group. Gaps from culling stay gaps.
                let mut ordinal = 0;
                loop {
                    let mut next = usize::MAX;
                    for model in start..end {
                        let cursor = scratch.cursors[model];
                        if cursor < scratch.model_ranges[model].end {
                            let item = scratch.eligible[cursor];
                            if item.draw == ordinal {
                                if let Some(reason) = scratch.append_compatible(item, &models) {
                                    draw_batch_breaks[reason] += 1;
                                }
                                scratch.cursors[model] += 1;
                            }
                            let cursor = scratch.cursors[model];
                            if cursor < scratch.model_ranges[model].end {
                                next = next.min(scratch.eligible[cursor].draw);
                            }
                        }
                    }
                    if next == usize::MAX {
                        break;
                    }
                    ordinal = next;
                }
            } else {
                for index in scratch.model_ranges[start].clone() {
                    scratch.singleton(scratch.eligible[index]);
                }
            }
            start = end;
        }
        if blend {
            scratch.groups.sort_by(|a, b| {
                let item_a = scratch.ordered[a.start];
                let item_b = scratch.ordered[b.start];
                models[item_b.model].1.draws[item_b.draw]
                    .center
                    .distance_squared(camera)
                    .total_cmp(
                        &models[item_a.model].1.draws[item_a.draw]
                            .center
                            .distance_squared(camera),
                    )
            });
        }
        let mut stats = self.counters.get();
        stats.overlap_tests += overlap_tests;
        stats.overlap_node_tests += overlap_node_tests;
        for (total, count) in stats.root_batch_breaks.iter_mut().zip(root_batch_breaks) {
            *total += count;
        }
        for (total, count) in stats.draw_batch_breaks.iter_mut().zip(draw_batch_breaks) {
            *total += count;
        }
        self.counters.set(stats);
        let mut prepared = PreparedDraws {
            draws: Vec::with_capacity(scratch.groups.len()),
            upload_bytes: 0,
        };
        // Blend preparation shares this key with the opaque preparation and
        // occurs before its submission. It must never overwrite/drop its stream.
        let mut records = 0usize;
        let mut required_bytes = 0usize;
        for group in &scratch.groups {
            let item = scratch.ordered[group.start];
            let first = &models[item.model].1.draws[item.draw].last_uniform;
            let translated = group.len() > 1
                && !blend
                && !key.shadow
                && !key.clipped
                && first.affine_basis()
                && scratch.ordered[group.clone()].iter().all(|item| {
                    first.same_basis(&models[item.model].1.draws[item.draw].last_uniform)
                });
            scratch.translated.push(translated);
            if group.len() > 1 {
                records = records.saturating_add(group.len());
                required_bytes =
                    required_bytes.saturating_add(group.len().saturating_mul(if translated {
                        16
                    } else {
                        112
                    }));
            }
        }
        let stream_limit = device.limits().max_buffer_size.min(16 * 1024 * 1024);
        let stream_bytes = (required_bytes as u64 <= stream_limit).then_some(required_bytes);
        let mut stream = if !blend && records > 0 && stream_bytes.is_some() {
            let capacity = required_bytes
                .next_power_of_two()
                .min(stream_limit as usize);
            let mut existing = self.instance_streams.remove(&(storage_key, view_slot));
            if existing
                .as_ref()
                .is_none_or(|s| s.capacity < required_bytes)
            {
                let mut stats = self.counters.get();
                stats.instance_buffers_created += 1;
                self.counters.set(stats);
                existing = Some(InstanceStream {
                    buffer: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("glTF pass-view instance stream"),
                        size: capacity as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }),
                    capacity,
                    last: Vec::new(),
                    scratch: Vec::new(),
                });
            }
            existing.as_mut().unwrap().scratch.clear();
            existing
        } else {
            if !blend {
                self.instance_streams.remove(&(storage_key, view_slot));
            }
            None
        };
        let mut page_index = 0;
        for (group_index, group) in scratch.groups.iter().enumerate() {
            let translated = scratch.translated[group_index];
            let first = scratch.ordered[group.start];
            let model = models[first.model].1;
            let d = &model.draws[first.draw];
            let material = &model.materials[d.material];
            let instanced = group.len() > 1;
            let variant = Variant {
                key,
                texture_mask: material.texture_mask,
                opaque_unclipped: material.alpha == AlphaMode::Opaque
                    && !key.clipped
                    && !key.shadow,
                topology: d.topology,
                blend,
                double_sided: material.double_sided,
                negative: d.negative,
                skinned: d.skin.is_some(),
                instanced,
                translated,
                compact_vertex: d.rigid,
            };
            self.ensure_pipeline(device, variant);
            self.ensure_pipeline(
                device,
                Variant {
                    negative: !variant.negative,
                    ..variant
                },
            );
            let mut stats = self.counters.get();
            if !instanced && d.uploaded_uniform.get() != d.last_uniform {
                queue.write_buffer(&d.uniform, 0, as_bytes(&[d.last_uniform]));
                d.uploaded_uniform.set(d.last_uniform);
                stats.buffer_upload_calls += 1;
                stats.uniform_upload_bytes += std::mem::size_of::<Uniform>() as u64;
            }
            let mut draw_bind = d.bind.clone();
            let instance = if instanced {
                scratch.payload.clear();
                for item in &scratch.ordered[group.clone()] {
                    let u = &models[item.model].1.draws[item.draw].last_uniform;
                    if translated {
                        scratch.payload.extend_from_slice(as_bytes(&[u.model[3]]));
                    } else {
                        scratch
                            .payload
                            .extend_from_slice(as_bytes(&[RigidInstance::from_uniform(u)]));
                    }
                }
                let payload = &scratch.payload;
                let pages = self
                    .instance_pages
                    .entry((storage_key, view_slot))
                    .or_default();
                let capacity = payload.len().next_power_of_two();
                if pages.len() <= page_index
                    || (stream.is_none()
                        && (pages[page_index].buffer.is_none()
                            || pages[page_index].capacity < capacity))
                {
                    stats.instance_pages_created += 1;
                    stats.instance_buffers_created += if stream.is_some() { 1 } else { 2 };
                    stats.instance_bind_groups_created += 1;
                    let material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("glTF instanced material"),
                        size: std::mem::size_of::<Uniform>() as u64,
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("glTF instanced material"),
                        layout: &self.material,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: material_buffer.as_entire_binding(),
                        }],
                    });
                    let page = InstancePage {
                        material_buffer,
                        bind,
                        material: None,
                        buffer: stream.is_none().then(|| {
                            device.create_buffer(&wgpu::BufferDescriptor {
                                label: Some("glTF rigid instances"),
                                size: capacity as u64,
                                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                                mapped_at_creation: false,
                            })
                        }),
                        capacity,
                        last: Vec::new(),
                    };
                    if pages.len() <= page_index {
                        pages.push(page);
                    } else {
                        pages[page_index] = page;
                    }
                }
                let page = &mut pages[page_index];
                let mut material = d.last_uniform;
                if translated {
                    material.model[3] = [0., 0., 0., 1.];
                    for n in &mut material.normal[..3] {
                        n[3] = 0.;
                    }
                    material.normal[3] = [0., 0., 0., 1.];
                } else {
                    material.model = Mat4::IDENTITY.to_cols_array_2d();
                    material.normal = Mat4::IDENTITY.to_cols_array_2d();
                    material.transform = [1., 1., 0., 0.];
                }
                if page.material != Some(material) {
                    queue.write_buffer(&page.material_buffer, 0, as_bytes(&[material]));
                    page.material = Some(material);
                    stats.buffer_upload_calls += 1;
                    stats.uniform_upload_bytes += std::mem::size_of::<Uniform>() as u64;
                }
                draw_bind = page.bind.clone();
                let binding = if let Some(s) = stream.as_mut() {
                    let offset = s.scratch.len() as u64;
                    s.scratch.extend_from_slice(payload);
                    page.buffer = None;
                    page.last.clear();
                    InstanceBinding {
                        buffer: s.buffer.clone(),
                        offset,
                        bytes: payload.len() as u64,
                    }
                } else {
                    let buffer = page.buffer.as_ref().expect("standalone instance buffer");
                    if page.last != *payload {
                        queue.write_buffer(buffer, 0, payload);
                        stats.buffer_upload_calls += 1;
                        stats.uniform_upload_bytes += payload.len() as u64;
                        stats.instance_upload_bytes += payload.len() as u64;
                        page.last.clone_from(payload);
                    }
                    InstanceBinding {
                        buffer: buffer.clone(),
                        offset: 0,
                        bytes: payload.len() as u64,
                    }
                };
                page_index += 1;
                Some(binding)
            } else {
                None
            };
            self.counters.set(stats);
            prepared.draws.push(PreparedDraw {
                variant,
                textures: material.textures.clone(),
                bind: draw_bind,
                vertex: d.vertex.clone(),
                index: d.index.clone(),
                skin: d.skin.as_ref().map(|s| s.influences.clone()),
                instance,
                count: d.count,
                instances: group.len() as u32,
                records: if self.diagnostics.get() {
                    scratch.ordered[group.clone()]
                        .iter()
                        .map(|item| {
                            let (id, model) = models[item.model];
                            let draw = &model.draws[item.draw];
                            (id, draw.node, draw.primitive)
                        })
                        .collect()
                } else {
                    Vec::new()
                },
            });
        }
        if let Some(mut s) = stream {
            if s.last != s.scratch {
                queue.write_buffer(&s.buffer, 0, &s.scratch);
                let bytes = s.scratch.len() as u64;
                let mut stats = self.counters.get();
                stats.buffer_upload_calls += 1;
                stats.uniform_upload_bytes += bytes;
                stats.instance_upload_bytes += bytes;
                self.counters.set(stats);
                std::mem::swap(&mut s.last, &mut s.scratch);
            }
            self.instance_streams.insert((storage_key, view_slot), s);
        }
        if !blend {
            self.active_instance_pages
                .insert((storage_key, view_slot), page_index);
            if let Some(pages) = self.instance_pages.get_mut(&(storage_key, view_slot)) {
                pages.truncate(page_index);
            }
        }
        let mut stats = self.counters.get();
        stats.queue_scratch_growths += capacities_before
            .into_iter()
            .zip(scratch.capacities())
            .filter(|(before, after)| before < after)
            .count() as u64;
        self.counters.set(stats);
        self.queue_scratch = scratch;
        prepared.upload_bytes = (self.counters.get().uniform_upload_bytes - upload_before) as usize;
        prepared
    }
    pub fn draw_prepared(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        prepared: &PreparedDraws,
        key: PipelineKey,
        view_slot: usize,
    ) -> usize {
        let mut stats = self.counters.get();
        if key.shadow {
            pass.set_bind_group(1, &self.empty_bind, &[]);
            stats.bind_group_changes += 1;
        }
        let mut last_variant = None;
        let mut last_textures = None;
        for d in &prepared.draws {
            let Some(pipeline) = self.pipelines.get(&d.variant) else {
                continue;
            };
            if last_variant != Some(d.variant) {
                pass.set_pipeline(pipeline);
                stats.pipeline_changes += 1;
                last_variant = Some(d.variant);
            }
            // Track the canonical Rust owner; preparation clones only an Rc,
            // rather than a backend BindGroup handle for every prepared draw.
            let texture_identity = Rc::as_ptr(&d.textures);
            if last_textures != Some(texture_identity) {
                pass.set_bind_group(2, d.textures.as_ref(), &[]);
                stats.bind_group_changes += 1;
                last_textures = Some(texture_identity);
            }
            pass.set_bind_group(3, &d.bind, &[]);
            stats.bind_group_changes += 1;
            pass.set_vertex_buffer(0, d.vertex.slice(..));
            if let Some(binding) = &d.instance {
                pass.set_vertex_buffer(
                    1,
                    binding
                        .buffer
                        .slice(binding.offset..binding.offset + binding.bytes),
                );
            } else if let Some(buffer) = &d.skin {
                pass.set_vertex_buffer(1, buffer.slice(..));
            }
            pass.set_index_buffer(d.index.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..d.count, 0, 0..d.instances);
            stats.draw_calls += 1;
            stats.instances_drawn += d.instances as u64;
            stats.triangles_drawn += match d.variant.topology {
                wgpu::PrimitiveTopology::TriangleList => d.count / 3,
                wgpu::PrimitiveTopology::TriangleStrip => d.count.saturating_sub(2),
                _ => 0,
            } as u64
                * d.instances as u64;
            if d.variant.translated {
                stats.translated_draw_calls += 1;
                stats.translated_instances += d.instances as u64;
            }
            if d.instances > 1 {
                stats.instanced_draw_calls += 1;
            }
            if self.diagnostics.get() {
                let mut r = self.draw_records.borrow_mut();
                if r.len() < 20000 {
                    r.push(DrawRecord {
                        object: d.records[0].0,
                        node: d.records[0].1,
                        primitive: d.records[0].2,
                        indices: d.count,
                        instances: d.instances,
                        objects: d.records.iter().map(|r| r.0).collect(),
                        view_slot,
                        shadow: key.shadow,
                        reflected: key.reflected,
                        blend: d.variant.blend,
                        material_mask: key.material_mask,
                    });
                }
            }
        }
        self.counters.set(stats);
        prepared.draws.len()
    }
}
// Replace sample expressions before compilation so absent maps incur no texture
// operation on either backend. Raw fallback texels preserve the original decoder.
fn material_shader_source(key: ShaderKey) -> String {
    let ShaderKey {
        texture_mask,
        shadow,
        opaque_unclipped,
        skinned,
        instanced,
        translated,
        single_static_shadow,
        light_class,
    } = key;
    let mut source = concat!(
        include_str!("gltf_pbr.wgsl"),
        "\n",
        include_str!("appearance.wgsl")
    )
    .to_owned();
    if single_static_shadow && !shadow {
        // Like Three's light/shadow program parameters, specialize only a proven
        // uniform state. Keep the same ordered 3x3 bilinear PCF kernel and bias.
        let start = source.find("fn shadow_visibility(").expect("shadow helper");
        let end = start + source[start..].find("struct Material").expect("material");
        source.replace_range(start..end, r#"fn shadow_visibility(index:u32,world:vec3<f32>,normal:vec3<f32>,light_direction:vec3<f32>)->f32 {
    if(index!=0u) { return 1.0; }
    let shadow_world=world+normal*scene.appearance_ground.w;
    let clip=effects.shadow_matrices[0]*vec4(shadow_world,1.0);
    let ndc=clip.xyz/clip.w;
    if (clip.w<=0.0 || ndc.z<=0.0 || ndc.z>=1.0 || any(abs(ndc.xy)>vec2(1.0))) {return 1.0;}
    let uv=ndc.xy*vec2(0.5,-0.5)+vec2(0.5);
    let bias=effects.params.w*(1.0+2.0*(1.0-max(dot(normal,light_direction),0.0)));
    let depth=ndc.z-bias;
    let texel=1.0/vec2<f32>(textureDimensions(static_shadow));
    var visibility=0.0;
    for(var y=-1;y<=1;y++) {for(var x=-1;x<=1;x++) {
        let sample_uv=uv+vec2<f32>(f32(x),f32(y))*texel;
        visibility+=textureSampleCompareLevel(static_shadow,shadow_sampler,sample_uv,depth);
    }}
    return visibility/9.0;
}

"#);
    }
    if !shadow {
        light_class.specialize(&mut source);
    }
    if skinned {
        source = source.replacen("@location(8) color:vec4<f32> };", "@location(8) color:vec4<f32>, @location(9) joints0:vec4<u32>, @location(10) joints1:vec4<u32>, @location(11) weights0:vec4<f32>, @location(12) weights1:vec4<f32> };", 1);
        source = source.replace("    // GPU_SKIN_VERTEX", "    let deformed=skin_vertex(v); local_position=deformed.position; local_normal=deformed.normal; local_tangent=deformed.tangent;");
    } else {
        // Exclude the entire palette/helper declarations from rigid variants.
        let start = source.find("// GPU_SKIN_BEGIN").expect("skin helper start");
        let end =
            source.find("// GPU_SKIN_END").expect("skin helper end") + "// GPU_SKIN_END".len();
        source.replace_range(start..end, "");
    }
    for (i, (sample, fallback)) in [
        ("textureSample(base_map,base_sampler,uv)", "vec4<f32>(1.0)"),
        ("textureSample(mr_map,mr_sampler,uv)", "vec4<f32>(1.0)"),
        (
            "textureSample(normal_map,normal_sampler,uv)",
            "vec4<f32>(128.0/255.0,128.0/255.0,1.0,1.0)",
        ),
        ("textureSample(ao_map,ao_sampler,uv)", "vec4<f32>(1.0)"),
        (
            "textureSample(emissive_map,emissive_sampler,uv)",
            "vec4<f32>(1.0)",
        ),
    ]
    .iter()
    .enumerate()
    {
        if texture_mask & (1 << i) == 0 {
            source = source.replace(sample, fallback);
        }
    }
    if texture_mask & (1 << 2) == 0 {
        // glTF without normalTexture uses the authored shading normal. Do not
        // decode an artificial 128/255 normal or construct an unused tangent frame.
        let start = source
            .find("    let mapped=sample_normal(v.uv2)")
            .expect("normal-map start");
        let end = start + source[start..].find("    let mr=").expect("normal-map end");
        source.replace_range(start..end, "");
        let derivatives = " let u1=dpdx(v.uv2); let u2=dpdy(v.uv2);";
        assert_eq!(
            source.matches(derivatives).count(),
            1,
            "normal-map derivatives"
        );
        source = source.replace(derivatives, "");
        // A tangent affects only the removed normal-map branch. Omit its input
        // fetch/layout as Three does for materials without a normal map. Keep
        // unused deformation helper outputs well-typed without altering position
        // or normal calculations, including the skinning path.
        source = source.replacen("@location(2) tangent:vec4<f32>, ", "", 1);
        source = source.replace("v.tangent", "vec4<f32>(0.0,0.0,0.0,1.0)");
    }
    source = source.replace(
        "fn uv(v:Output,index:u32)->vec2<f32> { return select(v.uv0,v.uv1,index==1u); }",
        "",
    );
    for (i, name) in ["base", "mr", "normal", "ao", "emissive"]
        .iter()
        .enumerate()
    {
        if texture_mask & (1 << i) != 0 {
            continue;
        }
        source = source.replace(&format!("sample_{name}(v.uv{i})"), "vec4<f32>(1.0)");
        source = source.replace(&format!("@location({}) uv{i}:vec2<f32>, ", i + 3), "");
        source = source.replace(&format!("o.uv{i}=v.uv{i}; "), "");
    }
    if translated {
        source = source.replacen(
            "@location(8) color:vec4<f32> };",
            "@location(8) color:vec4<f32>, @location(9) translation:vec4<f32> };",
            1,
        );
        let a = source
            .find("    if(material.transform.y>0.5) {")
            .expect("rigid transform");
        let b = a + source[a..].find("    o.pos=").expect("vertex projection");
        source.replace_range(a..b,"    let model=material.model;\n    let normal=mat3x3<f32>(material.normal[0].xyz,material.normal[1].xyz,material.normal[2].xyz);\n    o.world=(model*vec4(local_position,1.0)).xyz+v.translation.xyz;\n    o.normal=transform_unit(normal*local_normal);\n    o.tangent=vec4(transform_unit((model*vec4(local_tangent.xyz,0.0)).xyz),local_tangent.w*material.transform.x);\n");
    } else if instanced {
        source=source.replacen("@location(8) color:vec4<f32> };", "@location(8) color:vec4<f32>, @location(9) model0:vec4<f32>, @location(10) model1:vec4<f32>, @location(11) model2:vec4<f32>, @location(12) model3:vec4<f32>, @location(13) normal0:vec4<f32>, @location(14) normal1:vec4<f32>, @location(15) normal2:vec4<f32> };",1);
        let a = source
            .find("    if(material.transform.y>0.5) {")
            .expect("rigid transform");
        let b = a + source[a..].find("    o.pos=").expect("vertex projection");
        source.replace_range(a..b,"    let model=mat4x4<f32>(v.model0,v.model1,v.model2,v.model3);\n    let normal=mat3x3<f32>(v.normal0.xyz,v.normal1.xyz,v.normal2.xyz);\n    o.world=(model*vec4(local_position,1.0)).xyz;\n    o.normal=transform_unit(normal*local_normal);\n    o.tangent=vec4(transform_unit((model*vec4(local_tangent.xyz,0.0)).xyz),local_tangent.w*v.normal0.w);\n");
    }
    if opaque_unclipped && !shadow {
        // Remove only the color entrypoints' discard call. fs_shadow continues
        // to reach alpha_test and retains MASK/BLEND shadow coverage.
        source = source.replace("    surface_alpha_clip(v,base.a);", "");
    }
    if shadow {
        source = source
            .replace(
                "var<uniform> scene: SceneUniforms;",
                "var<uniform> shadow_view: mat4x4<f32>;",
            )
            .replace("scene.view_projection", "shadow_view")
            .replace("scene.camera_pos", "vec3(0.0)")
            .replace("scene.lights[i]", "Light(vec3(0.0),0.0,vec3(0.0),0.0)");
        // Shadow variants expose only the view matrix. Color-only helpers are
        // still parsed by both backends even though fs_shadow cannot reach them.
        for field in ["sky", "ground", "sun", "fog", "params", "shadow"] {
            // Keep fog edges distinct: WebGPU rejects constant smoothstep(0,0).
            let value = if field == "params" {
                "vec4<f32>(1.0)"
            } else {
                "vec4<f32>(0.0)"
            };
            source = source.replace(&format!("scene.appearance_{field}"), value);
        }
    }
    source
}
pub fn draw_count(model: &GpuModel, blend: bool) -> usize {
    model
        .last_pose
        .as_ref()
        .map(|_| {
            model
                .draws
                .iter()
                .filter(|d| {
                    model.active.contains(&d.node)
                        && (model.materials[d.material].alpha == AlphaMode::Blend) == blend
                })
                .count()
        })
        .unwrap_or(0)
}
fn wrap(mode: Option<WrapMode>) -> wgpu::AddressMode {
    match mode.unwrap_or(WrapMode::Repeat) {
        WrapMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        WrapMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
        WrapMode::Repeat => wgpu::AddressMode::Repeat,
    }
}
fn indices(p: &ModelPrimitive) -> (Vec<u32>, wgpu::PrimitiveTopology) {
    let input = if p.indices.is_empty() {
        (0..p.vertices.len() as u32).collect()
    } else {
        p.indices.clone()
    };
    match p.mode {
        PrimitiveMode::Points => (input, wgpu::PrimitiveTopology::PointList),
        PrimitiveMode::Lines => (input, wgpu::PrimitiveTopology::LineList),
        PrimitiveMode::LineStrip => (input, wgpu::PrimitiveTopology::LineStrip),
        PrimitiveMode::Triangles => (input, wgpu::PrimitiveTopology::TriangleList),
        PrimitiveMode::TriangleStrip => (input, wgpu::PrimitiveTopology::TriangleStrip),
        PrimitiveMode::LineLoop => {
            let mut out = Vec::new();
            for i in 0..input.len() {
                out.push(input[i]);
                out.push(input[(i + 1) % input.len()]);
            }
            (out, wgpu::PrimitiveTopology::LineList)
        }
        PrimitiveMode::TriangleFan => {
            let mut out = Vec::new();
            for i in 1..input.len().saturating_sub(1) {
                out.extend_from_slice(&[input[0], input[i], input[i + 1]]);
            }
            (out, wgpu::PrimitiveTopology::TriangleList)
        }
    }
}

fn material_slots(asset: &ModelAsset, primitive: &ModelPrimitive) -> [Option<TextureSlot>; 5] {
    primitive
        .material
        .and_then(|m| asset.materials.get(m))
        .map(|m| {
            [
                m.base_color_texture,
                m.metallic_roughness_texture,
                m.normal_texture,
                m.occlusion_texture,
                m.emissive_texture,
            ]
        })
        .unwrap_or([None; 5])
}
fn packed_vertex(
    v: &ModelVertex,
    pos: Vec3,
    n: Vec3,
    t: Vec3,
    sign: f32,
    slots: &[Option<TextureSlot>; 5],
) -> Vertex {
    Vertex {
        position: pos.into(),
        normal: n.into(),
        tangent: [t.x, t.y, t.z, v.tangent[3] * sign],
        uvs: std::array::from_fn(|i| {
            v.tex_coords
                .get(slots[i].map(|s| s.tex_coord as usize).unwrap_or(0))
                .copied()
                .unwrap_or([0.0; 2])
        }),
        color: v.color,
    }
}

// Preparation is one-off per immutable primitive. Keep nonzero influence order:
// merging duplicates or renormalizing would change the authoritative CPU math.
fn prepare_skin(
    asset: &ModelAsset,
    primitive: &ModelPrimitive,
    slots: &[Option<TextureSlot>; 5],
) -> Option<PreparedSkin> {
    let source = &asset.vertices[primitive.vertices.clone()];
    let mut vertices = Vec::with_capacity(source.len());
    let mut influences = Vec::with_capacity(source.len());
    let mut terms = vec![Vec4::ZERO; MAX_GPU_JOINTS];
    let mut identity = Vec4::ZERO;
    for v in source {
        let mut packed = SkinInfluences {
            joints0: [0; 4],
            joints1: [0; 4],
            weights0: [0.0; 4],
            weights1: [0.0; 4],
        };
        let mut count = 0;
        let mut mass = 0.0;
        if v.influences.is_empty() {
            identity += Vec3::from(v.position).extend(1.0);
        }
        for &(joint, weight) in &v.influences {
            if !weight.is_finite() || weight < 0.0 {
                return None;
            }
            if weight == 0.0 {
                continue;
            }
            if count >= 8 || joint as usize >= MAX_GPU_JOINTS {
                return None;
            }
            if count < 4 {
                packed.joints0[count] = joint;
                packed.weights0[count] = weight;
            } else {
                packed.joints1[count - 4] = joint;
                packed.weights1[count - 4] = weight;
            }
            terms[joint as usize] += Vec3::from(v.position).extend(1.0) * weight;
            mass += weight;
            count += 1;
        }
        if !v.influences.is_empty() && (!mass.is_finite() || mass <= 0.0) {
            return None;
        }
        vertices.push(packed_vertex(
            v,
            Vec3::from(v.position),
            Vec3::from(v.normal),
            Vec3::from_slice(&v.tangent[..3]),
            1.0,
            slots,
        ));
        influences.push(packed);
    }
    let divisor = source.len().max(1) as f32;
    let terms = terms
        .into_iter()
        .enumerate()
        .filter(|(_, q)| *q != Vec4::ZERO)
        .map(|(j, q)| (j, q / divisor))
        .collect();
    Some(PreparedSkin {
        vertices,
        influences,
        mean_terms: terms,
        identity_mean: identity / divisor,
    })
}
