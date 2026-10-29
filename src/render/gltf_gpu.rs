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
    fn attributes(texture_mask: u8, compact: bool, omit_color: bool) -> Vec<wgpu::VertexAttribute> {
        const A: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x4,3=>Float32x2,4=>Float32x2,5=>Float32x2,6=>Float32x2,7=>Float32x2,8=>Float32x4];
        let mut offset = 0;
        A.into_iter()
            .filter(|a| {
                a.shader_location < 2
                    || (a.shader_location == 2 && texture_mask & (1 << 2) != 0)
                    || (a.shader_location == 8 && !omit_color)
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
    fn compact(vertices: &[Self], texture_mask: u8, omit_color: bool) -> Vec<u8> {
        let attributes = Self::attributes(texture_mask, false, omit_color);
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
// Count engine-owned handles, including clones pinned by prepared rows. Dropping
// the last lease releases our ownership; wgpu may defer GPU destruction until
// submitted commands finish. Never call Buffer::destroy on pooled resources.
#[derive(Clone, Copy, Default)]
struct PoolMetrics {
    live_bytes: usize,
    peak_live_bytes: usize,
    buffers_created: u64,
    buffers_released: u64,
    binds_created: u64,
    binds_released: u64,
    created_bytes: u64,
    released_bytes: u64,
    reasons: [u64; 5],
    reason_bytes: [u64; 5],
    reuses: u64,
    evictions: u64,
    over_budget: u64,
}
#[derive(Default)]
struct PoolLedger(Cell<PoolMetrics>);
struct PoolLease {
    ledger: Rc<PoolLedger>,
    bytes: usize,
    buffers: u64,
    binds: u64,
}
impl PoolLedger {
    // Called only after the corresponding native create_buffer/create_bind_group.
    fn lease(self: &Rc<Self>, bytes: usize, binds: u64, reason: usize) -> Rc<PoolLease> {
        let mut m = self.0.get();
        m.live_bytes += bytes;
        m.peak_live_bytes = m.peak_live_bytes.max(m.live_bytes);
        m.buffers_created += 1;
        m.binds_created += binds;
        m.created_bytes += bytes as u64;
        m.reasons[reason] += 1;
        m.reason_bytes[reason] += bytes as u64;
        self.0.set(m);
        Rc::new(PoolLease {
            ledger: self.clone(),
            bytes,
            buffers: 1,
            binds,
        })
    }
}
impl Drop for PoolLease {
    fn drop(&mut self) {
        let mut m = self.ledger.0.get();
        m.live_bytes -= self.bytes;
        m.buffers_released += self.buffers;
        m.binds_released += self.binds;
        m.released_bytes += self.bytes as u64;
        self.ledger.0.set(m);
    }
}
type InstanceOwner = (PipelineKey, usize);
const POOL_BYTES: usize = 64 * 1024 * 1024;
const POOL_RESOURCES: usize = 4096;
const STREAM_BYTES: usize = 16 * 1024 * 1024;
struct InstancePage {
    generation: u64,
    buffer: Option<wgpu::Buffer>,
    material_buffer: wgpu::Buffer,
    bind: wgpu::BindGroup,
    material: Option<Uniform>,
    capacity: usize,
    last: Vec<u8>,
    material_lease: Rc<PoolLease>,
    buffer_lease: Option<Rc<PoolLease>>,
}
// Bounded pass/view-owned stream. Material uniforms remain per draw group;
// only the existing rigid instance records share one contiguous upload.
struct InstanceStream {
    generation: u64,
    buffer: wgpu::Buffer,
    // Bytes, because groups can mix 16B translation and 112B affine records.
    capacity: usize,
    last: Vec<u8>,
    scratch: Vec<u8>,
    lease: Rc<PoolLease>,
}
struct InstanceBinding {
    buffer: wgpu::Buffer,
    offset: u64,
    bytes: u64,
    _lease: Rc<PoolLease>,
}
struct PreparedDraw {
    variant: Variant,
    textures: Rc<wgpu::BindGroup>,
    bind: wgpu::BindGroup,
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    index_format: wgpu::IndexFormat,
    skin: Option<wgpu::Buffer>,
    instance: Option<InstanceBinding>,
    count: u32,
    instances: u32,
    records: Vec<(u32, usize, usize)>,
    _material_lease: Option<Rc<PoolLease>>,
}
#[derive(Default)]
pub struct PreparedDraws {
    draws: Rc<Vec<Rc<PreparedDraw>>>,
    pub upload_bytes: usize,
}

// One entry per logical pass/view, never one entry per camera matrix. The
// content epoch covers every mutable glTF buffer write, including writes made
// through a different pipeline/light variant sharing the same storage.
const PLAN_SLOTS: usize = 64;
const PLAN_BYTES: usize = 16 * 1024 * 1024;
#[derive(Clone, PartialEq, Eq)]
struct PlanKey {
    pipeline: PipelineKey,
    camera: [u32; 3],
    view_projection: [u32; 16],
    scene: u64,
    inputs: u64,
    content: u64,
    culling: bool,
    diagnostics: bool,
}
struct CachedPlan {
    key: PlanKey,
    models: Vec<(u32, u64, u64, bool)>,
    draws: Rc<Vec<Rc<PreparedDraw>>>,
    bytes: usize,
}
struct PlanCache {
    enabled: bool,
    slots: [Option<CachedPlan>; PLAN_SLOTS],
    hits: u64,
    misses: u64,
    // cold, scene, inputs, membership, view, pipeline, content, diagnostics,
    // lifecycle, toggle, budget eviction/rejection, unsupported slot.
    reasons: [u64; 12],
    bytes: usize,
}
impl Default for PlanCache {
    fn default() -> Self {
        Self {
            enabled: true,
            slots: std::array::from_fn(|_| None),
            hits: 0,
            misses: 0,
            reasons: [0; 12],
            bytes: 0,
        }
    }
}
impl PlanCache {
    fn evict_slot(&mut self, slot: usize) {
        if let Some(Some(old)) = self.slots.get_mut(slot).map(Option::take) {
            self.bytes -= old.bytes;
            self.reasons[10] += 1;
        }
    }
    fn clear(&mut self, reason: usize) {
        if self.bytes != 0 {
            self.slots.fill_with(|| None);
            self.bytes = 0;
        }
        self.reasons[reason] += 1;
    }
    fn lookup(
        &mut self,
        slot: usize,
        key: &PlanKey,
        models: &[(u32, &GpuModel)],
    ) -> Option<Rc<Vec<Rc<PreparedDraw>>>> {
        let reason = match self.slots.get(slot) {
            None => 11,
            Some(None) => 0,
            Some(Some(p)) => {
                if p.key.scene != key.scene {
                    1
                } else if p.key.inputs != key.inputs {
                    2
                } else if p.models.len() != models.len()
                    || !p.models.iter().zip(models).all(|(a, (id, m))| {
                        *a == (*id, m.generation, m.data_revision, m.opaque_reorderable)
                    })
                {
                    3
                } else if p.key.camera != key.camera
                    || p.key.view_projection != key.view_projection
                    || p.key.culling != key.culling
                {
                    4
                } else if p.key.pipeline != key.pipeline {
                    5
                } else if p.key.content != key.content {
                    6
                } else if p.key.diagnostics != key.diagnostics {
                    7
                } else {
                    self.hits += 1;
                    return Some(p.draws.clone());
                }
            }
        };
        self.misses += 1;
        self.reasons[reason] += 1;
        // Drop invalid entries immediately; no stale resource ownership across
        // misses or lifecycle transitions, even when the next plan is too big.
        if let Some(entry) = self.slots.get_mut(slot) {
            if let Some(old) = entry.take() {
                self.bytes -= old.bytes;
            }
        }
        None
    }
    fn store(
        &mut self,
        slot: usize,
        key: PlanKey,
        models: &[(u32, &GpuModel)],
        draws: Rc<Vec<Rc<PreparedDraw>>>,
    ) {
        if slot >= PLAN_SLOTS {
            return;
        }
        let models: Vec<_> = models
            .iter()
            .map(|(id, m)| (*id, m.generation, m.data_revision, m.opaque_reorderable))
            .collect();
        let bytes = draws.capacity() * std::mem::size_of::<Rc<PreparedDraw>>()
            + draws
                .iter()
                .map(|d| d.records.capacity() * std::mem::size_of::<(u32, usize, usize)>())
                .sum::<usize>()
            + draws.len() * std::mem::size_of::<PreparedDraw>()
            + models.capacity() * std::mem::size_of::<(u32, u64, u64, bool)>()
            + std::mem::size_of::<CachedPlan>();
        if bytes > PLAN_BYTES {
            self.reasons[10] += 1;
            return;
        }
        for entry in &mut self.slots {
            if self.bytes + bytes <= PLAN_BYTES {
                break;
            }
            if let Some(old) = entry.take() {
                self.bytes -= old.bytes;
                self.reasons[10] += 1;
            }
        }
        self.bytes += bytes;
        self.slots[slot] = Some(CachedPlan {
            key,
            models,
            draws,
            bytes,
        });
    }
}

#[derive(Clone, Copy)]
struct FineSource {
    owner: u64,
    draw: u64,
    data: u64,
    basis: u64,
    matched: bool,
}
#[derive(Default)]
struct FineGroup {
    sources: Vec<FineSource>,
    members_same: bool,
    first_basis: Option<(u64, u64)>,
    first_affine: bool,
    matched: usize,
    translated: bool,
    payload_slot: Option<(u64, usize, usize)>,
    row_storage: (u64, u64, u64, u64),
    active_bytes: usize,
    reserved_bytes: usize,
    row: Option<Rc<PreparedDraw>>,
}
impl FineGroup {
    fn select(
        &mut self,
        items: &[QueueItem],
        models: &[(u32, &GpuModel)],
        translation_allowed: bool,
        stats: &mut GltfGpuStats,
    ) {
        stats.fine_groups_visited += 1;
        self.members_same = self.sources.len() == items.len()
            && self
                .sources
                .iter()
                .zip(items)
                .all(|(s, i)| s.draw == models[i.model].1.draws[i.draw].identity);
        if self.members_same {
            stats.fine_group_membership_hits += 1;
        } else {
            stats.fine_group_membership_misses += 1;
            self.sources.clear();
            if self.sources.capacity() < items.len() {
                self.sources = Vec::with_capacity(items.len());
            }
            self.sources.extend(items.iter().map(|i| {
                let m = models[i.model].1;
                let d = &m.draws[i.draw];
                FineSource {
                    owner: m.generation,
                    draw: d.identity,
                    data: u64::MAX,
                    basis: u64::MAX,
                    matched: false,
                }
            }));
            self.first_basis = None;
            self.payload_slot = None;
            self.row = None;
        }
        if !translation_allowed {
            self.translated = false;
            return;
        }
        let i = items[0];
        let first = &models[i.model].1.draws[i.draw];
        let stamp = (first.identity, first.basis_revision);
        let changed = self.first_basis != Some(stamp);
        if changed {
            self.first_affine = first.last_uniform.affine_basis();
            self.matched = 0;
        }
        for (source, i) in self.sources.iter_mut().zip(items) {
            let d = &models[i.model].1.draws[i.draw];
            if changed || source.basis != d.basis_revision {
                stats.fine_basis_comparisons += 1;
                let matches = first.last_uniform.same_basis(&d.last_uniform);
                if changed {
                    self.matched += usize::from(matches);
                } else if matches != source.matched {
                    if matches {
                        self.matched += 1;
                    } else {
                        self.matched -= 1;
                    }
                }
                source.basis = d.basis_revision;
                source.matched = matches;
            }
        }
        self.first_basis = Some(stamp);
        self.translated = self.first_affine && self.matched == items.len();
    }
    // The retained stream/page mirror owns actual bytes. A record is copied only
    // after source revision or layout changes. World uniforms are already canonical;
    // pack a full record only inside the dirty full112B branch.
    #[allow(clippy::too_many_arguments)]
    fn patch(
        &mut self,
        target: &mut [u8],
        storage: u64,
        offset: usize,
        items: &[QueueItem],
        models: &[(u32, &GpuModel)],
        stats: &mut GltfGpuStats,
        uploads: &mut crate::instance_upload::DirtyRanges,
    ) {
        let stride = if self.translated { 16 } else { 112 };
        let full = self.payload_slot != Some((storage, offset, stride)) || !self.members_same;
        let mut rebuilt = full;
        for (index, (source, i)) in self.sources.iter_mut().zip(items).enumerate() {
            let d = &models[i.model].1.draws[i.draw];
            if full || source.data != d.data_revision {
                rebuilt = true;
                let packed;
                let bytes = if self.translated {
                    as_bytes(std::slice::from_ref(&d.last_uniform.model[3]))
                } else {
                    packed = RigidInstance::from_uniform(&d.last_uniform);
                    stats.fine_full_record_packs += 1;
                    as_bytes(std::slice::from_ref(&packed))
                };
                let start = offset + index * stride;
                let end = start + stride;
                if target[start..end] != *bytes {
                    // Expensive exact-byte accounting/timing is opt-in, never clean timing.
                    let clock = uploads
                        .diagnostic
                        .then(|| web_sys::window().and_then(|w| w.performance()))
                        .flatten();
                    let began = clock.as_ref().map(|p| p.now());
                    if uploads.diagnostic {
                        stats.instance_changed_bytes += target[start..end]
                            .iter()
                            .zip(bytes)
                            .filter(|(a, b)| a != b)
                            .count() as u64;
                        stats.instance_diff_scanned_bytes += stride as u64;
                    }
                    uploads.mark(start, end);
                    if let (Some(clock), Some(began)) = (clock, began) {
                        stats.instance_range_cpu_ms += clock.now() - began;
                    }
                    target[start..end].copy_from_slice(bytes);
                    stats.fine_payload_records_written += 1;
                    stats.fine_payload_bytes_written += stride as u64;
                }
                source.data = d.data_revision;
            }
        }
        self.payload_slot = Some((storage, offset, stride));
        if rebuilt {
            stats.fine_payload_groups_rebuilt += 1;
            stats.prepare_payload_rebuilds += 1;
        } else {
            stats.fine_payload_groups_reused += 1;
        }
    }
    fn bytes(&self) -> usize {
        self.sources.capacity() * std::mem::size_of::<FineSource>()
            + self.row.as_ref().map_or(0, |r| {
                std::mem::size_of::<PreparedDraw>()
                    + r.records.capacity() * std::mem::size_of::<(u32, usize, usize)>()
            })
    }
}
struct FineView {
    storage_key: PipelineKey,
    diagnostics: bool,
    groups: Vec<FineGroup>,
}
impl FineView {
    fn bytes(&self) -> usize {
        self.groups.capacity() * std::mem::size_of::<FineGroup>()
            + self.groups.iter().map(FineGroup::bytes).sum::<usize>()
    }
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
    compatibility_hits: u64,
    uploads: crate::instance_upload::DirtyRanges,
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
        self.compatibility_hits = 0;
        self.uploads.clear();
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
            + self.uploads.retained_bytes()
    }
    fn singleton(&mut self, item: QueueItem) {
        let start = self.ordered.len();
        self.ordered.push(item);
        self.groups.push(start..start + 1);
    }
    fn append_compatible(
        &mut self,
        item: QueueItem,
        models: &[(u32, &GpuModel)],
        reuse: bool,
    ) -> Option<usize> {
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
            } else if reuse
                && draw
                    .batch_class
                    .as_ref()
                    .zip(other.batch_class.as_ref())
                    .is_some_and(|(a, b)| Rc::ptr_eq(a, b))
            {
                self.compatibility_hits += 1;
                None
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
    omit_color: bool,
    vertex_stride: u64,
    index_max: Option<u32>,
    vertex: Option<wgpu::Buffer>,
    skin: Option<Rc<SharedSkin>>,
    index: wgpu::Buffer,
    index_format: wgpu::IndexFormat,
    count: u32,
    topology: wgpu::PrimitiveTopology,
    center: Vec3,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BatchClassKey {
    ordinal: usize,
    negative: bool,
    options: [u32; 4],
}
#[derive(Default)]
struct BatchClasses {
    entries: HashMap<BatchClassKey, Weak<()>>,
    saturated: bool,
}
impl BatchClasses {
    fn get(&mut self, key: BatchClassKey) -> Option<Rc<()>> {
        if let Some(class) = self.entries.get(&key).and_then(Weak::upgrade) {
            return Some(class);
        }
        if self.saturated {
            return None;
        }
        if self.entries.len() >= 4096 {
            self.entries.retain(|_, v| v.strong_count() > 0);
            if self.entries.len() >= 4096 {
                self.saturated = true;
                return None;
            }
        }
        let class = Rc::new(());
        self.entries.insert(key, Rc::downgrade(&class));
        Some(class)
    }
}
struct SharedAsset {
    batch_classes: RefCell<BatchClasses>,
    asset: Arc<ModelAsset>,
    materials: Rc<Vec<Material>>,
    geometry: Vec<Option<Geometry>>,
}
/// Cumulative work counters; resource counts describe currently live shared assets.
#[derive(Clone, Copy, Debug, Default)]
pub struct GltfGpuStats {
    pub cpu_deformed_vertices: u64,
    pub rigid_upload_bytes: u64,
    pub rigid_vertex_upload_bytes: u64,
    pub rigid_color_omitted_bytes: u64,
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
    pub instance_upload_calls: u64,
    pub instance_changed_bytes: u64,
    pub instance_diff_scanned_bytes: u64,
    pub instance_dirty_record_bytes: u64,
    pub instance_gap_upload_bytes: u64,
    pub instance_range_overflows: u64,
    pub instance_range_cpu_ms: f64,
    pub instance_buffer_bytes: usize,
    pub queue_scratch_growths: u64,
    pub queue_scratch_bytes: usize,
    pub prepare_eligible_rebuilds: u64,
    pub prepare_group_rebuilds: u64,
    pub prepare_opaque_group_rebuilds: u64,
    pub prepare_payload_rebuilds: u64,
    pub prepare_record_rebuilds: u64,
    pub prepare_timed_calls: u64,
    pub prepare_stage_ms: [f64; 6],
    pub fine_groups_visited: u64,
    pub fine_group_membership_hits: u64,
    pub fine_group_membership_misses: u64,
    pub fine_prepared_rows_reused: u64,
    pub fine_prepared_rows_created: u64,
    pub fine_payload_groups_reused: u64,
    pub fine_payload_groups_rebuilt: u64,
    pub fine_payload_records_written: u64,
    pub fine_payload_bytes_written: u64,
    pub fine_basis_comparisons: u64,
    pub fine_world_data_updates: u64,
    pub fine_full_record_packs: u64,
    pub fine_compatibility_hits: u64,
    pub fine_cache_budget_fallbacks: u64,
    pub fine_dynamic_fallbacks: u64,
    pub fine_no_instance_fallbacks: u64,
    pub fine_cache_bytes: usize,
    pub fine_layout_active_bytes: usize,
    pub fine_layout_reserved_bytes: usize,
}
struct Draw {
    identity: u64,
    data_revision: u64,
    basis_revision: u64,
    batch_class: Option<Rc<()>>,
    batch_key: Option<BatchClassKey>,
    node: usize,
    primitive: usize,
    material: usize,
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    index_format: wgpu::IndexFormat,
    count: u32,
    topology: wgpu::PrimitiveTopology,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    center: Vec3,
    negative: bool,
    rigid: bool,
    omit_color: bool,
    skin: Option<Rc<SharedSkin>>,
    local_center: Vec3,
    last_uniform: Uniform,
    uploaded_uniform: Cell<Uniform>,
    bounds: Option<(Vec3, Vec3)>,
    scratch: Vec<Vertex>,
}
pub struct GpuModel {
    generation: u64,
    data_revision: u64,
    draws: Vec<Draw>,
    materials: Rc<Vec<Material>>,
    _shared: Rc<SharedAsset>,
    palettes: HashMap<usize, SkinPalette>,
    last_pose: Option<(u64, Mat4, [f32; 4])>,
    active: HashSet<usize>,
    pub opaque_reorderable: bool,
    reorderable_draws: bool,
    // Immutable material/draw membership, independent of pose and visibility.
    has_blend_draws: bool,
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
    cheap_fragment: bool,
    key: PipelineKey,
    texture_mask: u8,
    opaque_unclipped: bool,
    topology: wgpu::PrimitiveTopology,
    strip_index_format: Option<wgpu::IndexFormat>,
    blend: bool,
    double_sided: bool,
    negative: bool,
    skinned: bool,
    instanced: bool,
    translated: bool,
    compact_vertex: bool,
    omit_color: bool,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ShaderKey {
    cheap_fragment: bool,
    omit_color: bool,
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
    omit_white_colors: bool,
    compact_indices: bool,
    experiment: u32,
    index_upload_bytes: Cell<u64>,
    index_allocated_bytes: Cell<u64>,
    counters: Cell<GltfGpuStats>,
    diagnostics: Cell<bool>,
    draw_records: RefCell<Vec<DrawRecord>>,
    instance_pages: HashMap<(PipelineKey, usize), Vec<InstancePage>>,
    instance_streams: HashMap<(PipelineKey, usize), InstanceStream>,
    active_instance_pages: HashMap<(PipelineKey, usize), usize>,
    pool_ledger: Rc<PoolLedger>,
    pool_byte_limit: usize,
    pool_resource_limit: usize,
    stream_byte_limit: usize,
    pool_frame: u64,
    owner_frames: HashMap<InstanceOwner, u64>,
    owner_access: HashMap<InstanceOwner, u64>,
    pool_clock: u64,
    used_streams: HashSet<InstanceOwner>,
    instancing_supported: bool,
    queue_scratch: QueueScratch,
    upload_policy: (u32, usize, bool),
    plan_cache: RefCell<PlanCache>,
    input_epoch: Cell<u64>,
    content_epoch: Cell<u64>,
    next_resource: Cell<u64>,
    fine_views: RefCell<[Option<FineView>; PLAN_SLOTS]>,
    model_data_epoch: Cell<u64>,
    model_data_marks: [u64; PLAN_SLOTS],
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
            omit_white_colors: true,
            compact_indices: true,
            experiment: 0,
            index_upload_bytes: Cell::new(0),
            index_allocated_bytes: Cell::new(0),
            counters: Cell::new(GltfGpuStats::default()),
            diagnostics: Cell::new(false),
            draw_records: RefCell::new(Vec::new()),
            instance_pages: HashMap::new(),
            instance_streams: HashMap::new(),
            active_instance_pages: HashMap::new(),
            pool_ledger: Rc::default(),
            pool_byte_limit: POOL_BYTES,
            pool_resource_limit: POOL_RESOURCES,
            stream_byte_limit: STREAM_BYTES,
            pool_frame: 0,
            owner_frames: HashMap::new(),
            owner_access: HashMap::new(),
            pool_clock: 0,
            used_streams: HashSet::new(),
            queue_scratch: QueueScratch::default(),
            upload_policy: (0, 0, false),
            plan_cache: RefCell::new(PlanCache::default()),
            input_epoch: Cell::new(0),
            content_epoch: Cell::new(0),
            next_resource: Cell::new(0),
            fine_views: RefCell::new(std::array::from_fn(|_| None)),
            model_data_epoch: Cell::new(0),
            model_data_marks: [0; PLAN_SLOTS],
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
            let (indices, mode, index_max) = crate::gltf_indices::prepare(
                &p.indices,
                p.vertices.len(),
                p.mode,
                self.compact_indices,
            );
            let topology = topology(mode);
            let index_format = if indices.is_u16() {
                wgpu::IndexFormat::Uint16
            } else {
                wgpu::IndexFormat::Uint32
            };
            let has_rigid_node = p.morph_targets.is_empty()
                && asset.nodes.iter().any(|n| {
                    n.skin.is_none()
                        && n.mesh
                            .is_some_and(|m| asset.meshes[m].primitives.contains(&primitive))
                });
            let slots = material_slots(asset, p);
            let mut center = Vec3::ZERO;
            let omit_color = has_rigid_node
                && self.omit_white_colors
                && exact_white_vertex_colors(&asset.vertices[p.vertices.clone()]);
            let texture_mask = materials[p.material.unwrap_or(asset.materials.len())].texture_mask;
            let vertex_stride = Vertex::attributes(texture_mask, true, omit_color)
                .iter()
                .map(|a| a.format.size())
                .sum();
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
                let packed = Vertex::compact(&vertices, texture_mask, omit_color);
                stats.rigid_vertex_upload_bytes += packed.len() as u64;
                if omit_color {
                    stats.rigid_color_omitted_bytes += (vertices.len() * 16) as u64;
                }
                stats.rigid_upload_bytes += (packed.len() + indices.bytes().len()) as u64;
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
                contents: indices.bytes(),
                usage: wgpu::BufferUsages::INDEX,
            });
            self.index_upload_bytes
                .set(self.index_upload_bytes.get() + indices.bytes().len() as u64);
            self.index_allocated_bytes
                .set(self.index_allocated_bytes.get() + index.size());
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
                        stats.skin_upload_bytes += (indices.bytes().len()) as u64;
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
                omit_color,
                vertex_stride,
                index_max,
                vertex,
                skin,
                index,
                index_format,
                count: indices.len() as u32,
                topology,
                center,
            }));
        }
        self.counters.set(stats);
        let shared = Rc::new(SharedAsset {
            asset: asset.clone(),
            batch_classes: RefCell::new(BatchClasses::default()),
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
        stats.fine_cache_bytes = self
            .fine_views
            .borrow()
            .iter()
            .flatten()
            .map(FineView::bytes)
            .sum();
        for view in self.fine_views.borrow().iter().flatten() {
            for group in &view.groups {
                stats.fine_layout_active_bytes += group.active_bytes;
                stats.fine_layout_reserved_bytes += group.reserved_bytes;
            }
        }
        stats.instance_pages_active = self
            .active_instance_pages
            .iter()
            .filter(|(owner, _)| self.owner_frames.get(owner) == Some(&self.pool_frame))
            .map(|(_, count)| *count)
            .sum();
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
    fn invalidate_plan_inputs(&self) {
        self.input_epoch.set(
            self.input_epoch
                .get()
                .checked_add(1)
                .expect("glTF input epoch exhausted"),
        );
        self.plan_cache.borrow_mut().clear(2);
    }
    fn resource_id(&self) -> u64 {
        let id = self
            .next_resource
            .get()
            .checked_add(1)
            .expect("glTF resource identity exhausted");
        self.next_resource.set(id);
        id
    }
    pub(crate) fn model_generation(model: &GpuModel) -> u64 {
        model.generation
    }
    pub(crate) fn models_removed(&self, removed: &HashSet<u64>) {
        self.invalidate_plan_inputs();
        for view in self.fine_views.borrow_mut().iter_mut().flatten() {
            for group in &mut view.groups {
                if group.sources.iter().any(|s| removed.contains(&s.owner)) {
                    *group = FineGroup::default();
                }
            }
        }
    }
    /// Reference control. Existing immutable GPU assets never change layout in place.
    pub fn set_white_color_omission_enabled(&mut self, enabled: bool) -> bool {
        if self.omit_white_colors == enabled {
            return true;
        }
        if self.assets.borrow().values().any(|a| a.strong_count() > 0) {
            return false;
        }
        self.invalidate_plan_inputs();
        self.omit_white_colors = enabled;
        true
    }
    pub fn set_instance_upload_policy(&mut self, mode: u32, gap: u32, diagnostic: bool) -> bool {
        if mode > 2 || gap > 65536 || !gap.is_multiple_of(4) {
            return false;
        }
        if self.upload_policy != (mode, gap as usize, diagnostic) {
            self.invalidate_plan_inputs();
            self.upload_policy = (mode, gap as usize, diagnostic);
        }
        true
    }
    pub fn instance_upload_policy(&self) -> serde_json::Value {
        serde_json::json!({"mode":self.upload_policy.0,"gapBytes":self.upload_policy.1,"diagnostic":self.upload_policy.2,"scratchBytes":self.queue_scratch.uploads.retained_bytes(),"rangeLimit":4096})
    }
    /// Bounded diagnostic controls. Production remains mode0; no arbitrary shaders.
    pub fn set_experiment(&mut self, mode: u32) -> bool {
        if mode > 1 {
            return false;
        }
        if self.experiment != mode {
            self.invalidate_plan_inputs();
            self.fine_views.borrow_mut().fill_with(|| None);
            self.experiment = mode;
        }
        true
    }
    /// Original Uint32 oracle. Never mutate immutable geometry held by live models.
    pub fn set_compact_indices_enabled(&mut self, enabled: bool) -> bool {
        if self.compact_indices == enabled {
            return true;
        }
        if self.assets.borrow().values().any(|a| a.strong_count() > 0) {
            return false;
        }
        self.invalidate_plan_inputs();
        self.compact_indices = enabled;
        true
    }
    pub fn index_layouts(&self) -> serde_json::Value {
        let mut layouts = Vec::new();
        for asset in self.assets.borrow().values().filter_map(Weak::upgrade) {
            for (primitive, g) in asset
                .geometry
                .iter()
                .enumerate()
                .filter_map(|(i, g)| g.as_ref().map(|g| (i, g)))
            {
                layouts.push(serde_json::json!({"primitive":primitive,"format":format!("{:?}",g.index_format),"topology":format!("{:?}",g.topology),"count":g.count,"max":g.index_max,"payloadBytes":u64::from(g.count)*u64::from(g.index_format.byte_size()),"bufferBytes":g.index.size()}));
            }
        }
        serde_json::Value::Array(layouts)
    }
    pub fn index_stats(&self) -> serde_json::Value {
        let (mut u16_count, mut u32_count, mut bytes, mut payload) = (0usize, 0usize, 0u64, 0u64);
        for asset in self.assets.borrow().values().filter_map(Weak::upgrade) {
            for g in asset.geometry.iter().flatten() {
                if g.index_format == wgpu::IndexFormat::Uint16 {
                    u16_count += 1;
                } else {
                    u32_count += 1;
                }
                bytes += g.index.size();
                payload += u64::from(g.count) * u64::from(g.index_format.byte_size());
            }
        }
        serde_json::json!({"enabled":self.compact_indices,"experiment":self.experiment,"uint16Primitives":u16_count,"uint32Primitives":u32_count,"bufferBytes":bytes,"payloadBytes":payload,"paddingBytes":bytes-payload,"uploadBytes":self.index_upload_bytes.get(),"allocatedBytes":self.index_allocated_bytes.get(),"memoryScope":"requested live shared index buffers; excludes driver overhead; upload/allocated cumulative"})
    }
    /// Detailed diagnostics are deliberately separate from regularly polled stats.
    pub fn vertex_layouts(&self) -> serde_json::Value {
        let mut layouts = Vec::new();
        for asset in self.assets.borrow().values().filter_map(Weak::upgrade) {
            for (primitive, g) in asset
                .geometry
                .iter()
                .enumerate()
                .filter_map(|(i, g)| g.as_ref().map(|g| (i, g)))
            {
                if let Some(vertex) = &g.vertex {
                    layouts.push(serde_json::json!({"primitive":primitive,"stride":g.vertex_stride,"bufferBytes":vertex.size(),"omitColor":g.omit_color}));
                }
            }
        }
        serde_json::Value::Array(layouts)
    }
    pub fn vertex_color_stats(&self) -> serde_json::Value {
        let (mut bytes, mut primitives, mut colorless) = (0u64, 0usize, 0usize);
        for asset in self.assets.borrow().values().filter_map(Weak::upgrade) {
            for g in asset.geometry.iter().flatten() {
                if let Some(vertex) = &g.vertex {
                    bytes += vertex.size();
                    primitives += 1;
                    colorless += usize::from(g.omit_color);
                }
            }
        }
        let counters = self.counters.get();
        serde_json::json!({"enabled":self.omit_white_colors,"rigidVertexBufferBytes":bytes,
            "rigidPrimitives":primitives,"colorlessPrimitives":colorless,
            "colorlessShaders":self.shaders.keys().filter(|k|k.omit_color).count(),
            "coloredShaders":self.shaders.keys().filter(|k|!k.omit_color).count(),
            "colorlessPipelines":self.pipelines.keys().filter(|k|k.omit_color).count(),
            "rigidVertexUploadBytes":counters.rigid_vertex_upload_bytes,
            "omittedColorBytes":counters.rigid_color_omitted_bytes,
            "memoryScope":"requested live shared rigid vertex-buffer bytes; excludes indices, textures, skin/morph and driver overhead"})
    }
    pub fn set_plan_cache_enabled(&self, enabled: bool) {
        let mut cache = self.plan_cache.borrow_mut();
        if cache.enabled != enabled {
            cache.clear(9);
            self.fine_views.borrow_mut().fill_with(|| None);
            cache.enabled = enabled;
        }
    }
    pub fn plan_cache_stats(&self) -> serde_json::Value {
        let c = self.plan_cache.borrow();
        serde_json::json!({"enabled":c.enabled,"hits":c.hits,"misses":c.misses,
            "reasons":c.reasons,"reasonNames":["cold","scene","inputs","membership","view","pipeline","content","diagnostics","lifecycle","toggle","budget","unsupportedSlot"],
            "retainedBytes":c.bytes,"byteLimit":PLAN_BYTES,"slotLimit":PLAN_SLOTS,
            "retainedSlots":c.slots.iter().filter(|s|s.is_some()).count(),
            "inputEpoch":self.input_epoch.get(),"contentEpoch":self.content_epoch.get(),
            "memoryScope":"CPU plan element storage; existing referenced GPU allocations reported separately"})
    }
    fn create_model(&self, device: &wgpu::Device, shared: Rc<SharedAsset>) -> GpuModel {
        self.invalidate_plan_inputs();
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
                    identity: self.resource_id(),
                    data_revision: 0,
                    basis_revision: 0,
                    batch_class: None,
                    batch_key: None,
                    node,
                    primitive,
                    material,
                    vertex,
                    index: geometry.index.clone(),
                    index_format: geometry.index_format,
                    count: geometry.count,
                    topology: geometry.topology,
                    uniform,
                    bind,
                    center: Vec3::ZERO,
                    negative: false,
                    rigid,
                    omit_color: rigid && geometry.omit_color,
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
            generation: self.input_epoch.get(),
            data_revision: 0,
            active_draws,
            opaque_reorderable: false,
            reorderable_draws,
            has_blend_draws: draws
                .iter()
                .any(|d| materials[d.material].alpha == AlphaMode::Blend),
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
        let epoch = self
            .model_data_epoch
            .get()
            .checked_add(1)
            .expect("glTF model data epoch exhausted");
        self.model_data_epoch.set(epoch);
        model.data_revision = epoch;
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
                    self.content_epoch.set(
                        self.content_epoch
                            .get()
                            .checked_add(1)
                            .expect("glTF content epoch exhausted"),
                    );
                    stats.buffer_upload_calls += 1;
                    let uploaded = palette.last.len() * 64;
                    bytes += uploaded;
                    stats.uniform_upload_bytes += uploaded as u64;
                    stats.skin_palette_upload_bytes += uploaded as u64;
                }
            }
        }
        for (ordinal, draw) in model.draws.iter_mut().enumerate() {
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
                self.content_epoch.set(
                    self.content_epoch
                        .get()
                        .checked_add(1)
                        .expect("glTF content epoch exhausted"),
                );
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
                if draw.rigid {
                    // last_uniform already owns the canonical world data. Mark
                    // publication conservatively; build112B only for a dirty
                    // full-record payload, never for compact translation groups.
                    draw.data_revision = draw
                        .data_revision
                        .checked_add(1)
                        .expect("glTF draw revision exhausted");
                    stats.fine_world_data_updates += 1;
                }
                if !super::data::instance_basis_unchanged(
                    &draw.last_uniform.model,
                    &draw.last_uniform.normal,
                    &draw.last_uniform.transform,
                    &uniform.model,
                    &uniform.normal,
                    &uniform.transform,
                ) {
                    draw.basis_revision = draw
                        .basis_revision
                        .checked_add(1)
                        .expect("glTF basis revision exhausted");
                }
                draw.last_uniform = uniform;
            }
            if draw.rigid && draw.last_uniform.options.iter().all(|v| v.is_finite()) {
                let key = BatchClassKey {
                    ordinal,
                    negative: draw.negative,
                    options: draw
                        .last_uniform
                        .options
                        .map(|v| if v == 0. { 0 } else { v.to_bits() }),
                };
                // Only actual compatibility changes enter the bounded shared map.
                // Translation/pose data never re-intern an unchanged class.
                if draw.batch_key != Some(key) {
                    draw.batch_class = model._shared.batch_classes.borrow_mut().get(key);
                    draw.batch_key = Some(key);
                }
            } else {
                draw.batch_class = None;
                draw.batch_key = None;
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
            cheap_fragment: variant.cheap_fragment,
            omit_color: variant.omit_color,
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
        let attributes = Vertex::attributes(
            variant.texture_mask,
            variant.compact_vertex,
            variant.omit_color,
        );
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
                strip_index_format: variant.strip_index_format,
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
                bias: if variant.key.shadow
                    && matches!(
                        variant.topology,
                        wgpu::PrimitiveTopology::TriangleList
                            | wgpu::PrimitiveTopology::TriangleStrip
                    ) {
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
    pub fn configure_instance_pool(
        &mut self,
        bytes: Option<u32>,
        resources: Option<u32>,
        stream: Option<u32>,
    ) -> bool {
        if bytes.is_some_and(|n| n > 256 * 1024 * 1024)
            || resources.is_some_and(|n| n > 65536)
            || stream.is_some_and(|n| n < 16 || n as usize > STREAM_BYTES || n % 16 != 0)
        {
            return false;
        }
        if let Some(n) = bytes {
            self.pool_byte_limit = n as usize;
        }
        if let Some(n) = resources {
            self.pool_resource_limit = n as usize;
        }
        if let Some(n) = stream {
            if self.stream_byte_limit != n as usize {
                self.stream_byte_limit = n as usize;
                self.plan_cache.borrow_mut().clear(5);
                self.fine_views.borrow_mut().fill_with(|| None);
            }
        }
        self.trim_instance_pool();
        true
    }
    fn pool_over_budget(&self) -> bool {
        let m = self.pool_ledger.0.get();
        m.live_bytes > self.pool_byte_limit
            || (m.buffers_created - m.buffers_released + m.binds_created - m.binds_released)
                > self.pool_resource_limit as u64
    }
    fn protected_pages(&self, owner: &InstanceOwner) -> usize {
        // A pass scheduled later this frame still owns last frame's allocation.
        // One-frame grace avoids repeatedly stealing the other eye's resources.
        if self
            .owner_frames
            .get(owner)
            .is_some_and(|f| *f + 1 >= self.pool_frame)
        {
            self.active_instance_pages.get(owner).copied().unwrap_or(0)
        } else {
            0
        }
    }
    fn trim_instance_pool(&mut self) {
        if !self.pool_over_budget() {
            return;
        }
        let mut owners: Vec<_> = self.owner_access.iter().map(|(o, t)| (*t, *o)).collect();
        owners.sort_unstable_by_key(|(t, _)| *t);
        for (_, owner) in owners {
            if !self.pool_over_budget() {
                break;
            }
            let protected = self.protected_pages(&owner);
            let streaming = protected > 0 && self.used_streams.contains(&owner);
            let tails = self
                .instance_pages
                .get(&owner)
                .is_some_and(|p| p.len() > protected);
            let idle_stream = !streaming && self.instance_streams.contains_key(&owner);
            let idle_buffer = streaming
                && self
                    .instance_pages
                    .get(&owner)
                    .is_some_and(|p| p.iter().take(protected).any(|p| p.buffer.is_some()));
            if !(tails || idle_stream || idle_buffer) {
                continue;
            }
            self.plan_cache.borrow_mut().evict_slot(owner.1);
            if let Some(slot) = self.fine_views.borrow_mut().get_mut(owner.1) {
                *slot = None;
            }
            while self.pool_over_budget()
                && self
                    .instance_pages
                    .get(&owner)
                    .is_some_and(|p| p.len() > protected)
            {
                self.instance_pages.get_mut(&owner).unwrap().pop();
                let mut m = self.pool_ledger.0.get();
                m.evictions += 1;
                self.pool_ledger.0.set(m);
            }
            if self.pool_over_budget() && idle_stream {
                self.instance_streams.remove(&owner);
                let mut m = self.pool_ledger.0.get();
                m.evictions += 1;
                self.pool_ledger.0.set(m);
            }
            if streaming {
                for i in 0..protected {
                    if !self.pool_over_budget() {
                        break;
                    }
                    let Some(page) = self
                        .instance_pages
                        .get_mut(&owner)
                        .and_then(|p| p.get_mut(i))
                    else {
                        break;
                    };
                    if page.buffer.take().is_some() {
                        page.buffer_lease = None;
                        page.capacity = 0;
                        page.last = Vec::new();
                        let mut m = self.pool_ledger.0.get();
                        m.evictions += 1;
                        self.pool_ledger.0.set(m);
                    }
                }
            }
            if self.instance_pages.get(&owner).is_some_and(Vec::is_empty) {
                self.instance_pages.remove(&owner);
            }
            if !self.instance_pages.contains_key(&owner)
                && !self.instance_streams.contains_key(&owner)
            {
                self.owner_access.remove(&owner);
                self.owner_frames.remove(&owner);
                self.active_instance_pages.remove(&owner);
                self.used_streams.remove(&owner);
            }
        }
        if self.pool_over_budget() {
            let mut m = self.pool_ledger.0.get();
            m.over_budget += 1;
            self.pool_ledger.0.set(m);
        }
    }
    pub fn instance_pool_stats(&self) -> serde_json::Value {
        let m = self.pool_ledger.0.get();
        let mut owned = 0;
        let mut active = 0;
        let mut pages = 0;
        let mut active_pages = 0;
        let mut active_streams = 0;
        for (owner, entries) in &self.instance_pages {
            let count = if self.owner_frames.get(owner) == Some(&self.pool_frame) {
                self.active_instance_pages.get(owner).copied().unwrap_or(0)
            } else {
                0
            };
            for (i, p) in entries.iter().enumerate() {
                let bytes = std::mem::size_of::<Uniform>();
                owned += bytes + p.buffer.as_ref().map_or(0, |_| p.capacity);
                pages += 1;
                if i < count {
                    active_pages += 1;
                    active += bytes
                        + if self.used_streams.contains(owner) {
                            0
                        } else {
                            p.capacity
                        };
                }
            }
        }
        for (owner, stream) in &self.instance_streams {
            owned += stream.capacity;
            if self.owner_frames.get(owner) == Some(&self.pool_frame)
                && self.used_streams.contains(owner)
            {
                active_streams += 1;
                active += stream.capacity;
            }
        }
        serde_json::json!({
            "byteLimit":self.pool_byte_limit,"resourceLimit":self.pool_resource_limit,
            "streamByteLimit":self.stream_byte_limit,
            "ownedBytes":owned,"activeBytes":active,"inactiveBytes":owned-active,
            "liveBytes":m.live_bytes,"retiredBytes":m.live_bytes.saturating_sub(owned),
            "peakLiveBytes":m.peak_live_bytes,
            "activePages":active_pages,"inactivePages":pages-active_pages,
            "activeStreams":active_streams,"inactiveStreams":self.instance_streams.len()-active_streams,
            "buffersCreated":m.buffers_created,"buffersReleased":m.buffers_released,
            "bindGroupsCreated":m.binds_created,"bindGroupsReleased":m.binds_released,
            "createdBytes":m.created_bytes,"releasedBytes":m.released_bytes,
            "liveBuffers":m.buffers_created-m.buffers_released,
            "liveBindGroups":m.binds_created-m.binds_released,
            "reuses":m.reuses,"evictions":m.evictions,"overBudgetEvents":m.over_budget,
            "createReasons":m.reasons,"createReasonBytes":m.reason_bytes,
            "reasonNames":["streamCold","streamGrowth","pageMaterial","standaloneCold","standaloneGrowth"],
            "memoryScope":"known instance buffer capacities and material uniforms; bind-group driver bytes unavailable; global handle limit; active/pinned working set and one-frame pass grace can exceed budget",
            "releaseScope":"last engine lease dropped; GPU destruction may be deferred by wgpu"
        })
    }
    pub fn begin_frame(&mut self) {
        self.pool_frame += 1;
        self.draw_records.borrow_mut().clear();
    }
    pub fn set_diagnostics(&self, enabled: bool) {
        if self.diagnostics.get() != enabled {
            self.plan_cache.borrow_mut().clear(7);
            self.fine_views.borrow_mut().fill_with(|| None);
        }
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
        self.plan_cache.borrow_mut().clear(8);
        self.fine_views.borrow_mut().fill_with(|| None);
        self.content_epoch.set(
            self.content_epoch
                .get()
                .checked_add(1)
                .expect("glTF content epoch exhausted"),
        );
        self.instance_pages.clear();
        self.instance_streams.clear();
        self.active_instance_pages.clear();
        self.owner_frames.clear();
        self.owner_access.clear();
        self.used_streams.clear();
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
        scene_revision: u64,
    ) -> PreparedDraws {
        let _scope = super::profiler::cpu_scope("gltf.queue");
        let mut stages = super::profiler::CpuStages::new([
            "gltf.prepare.eligible",
            "gltf.prepare.groups",
            "gltf.prepare.basis",
            "gltf.prepare.payload",
            "gltf.prepare.records",
            "gltf.prepare.resources",
        ]);
        let upload_before = self.counters.get().uniform_upload_bytes;
        // Lighting changes the program, never the instance payload or material
        // layout. Keep one stream/page set across all light program classes.
        let storage_key = PipelineKey {
            light_class: Default::default(),
            ..key
        };
        let owner = (storage_key, view_slot);
        if !blend {
            self.owner_frames.insert(owner, self.pool_frame);
            self.pool_clock += 1;
            self.owner_access.insert(owner, self.pool_clock);
        }
        let models: Vec<_> = models.collect();
        let mut plan_key = PlanKey {
            pipeline: key,
            camera: camera.to_array().map(f32::to_bits),
            view_projection: view_projection.to_cols_array().map(f32::to_bits),
            scene: scene_revision,
            inputs: self.input_epoch.get(),
            content: self.content_epoch.get(),
            culling,
            diagnostics: self.diagnostics.get(),
        };
        let cache_enabled = !blend && !models.is_empty() && self.plan_cache.borrow().enabled;
        if cache_enabled {
            let hit = self
                .plan_cache
                .borrow_mut()
                .lookup(view_slot, &plan_key, &models);
            if let Some(draws) = hit {
                // Do not attribute lookup/model-list validation to rebuild stages.
                // These are cumulative work counters, not the parent queue scope.
                stages.finish();
                self.trim_instance_pool();
                return PreparedDraws {
                    draws,
                    upload_bytes: 0,
                };
            }
        }
        stages.switch(Some(0));
        let frustum = crate::visibility::Frustum::from_view_projection(view_projection);
        let mut stats = self.counters.get();
        let mut scratch = std::mem::take(&mut self.queue_scratch);
        scratch.uploads.mode = self.upload_policy.0;
        scratch.uploads.gap = self.upload_policy.1;
        scratch.uploads.diagnostic = self.upload_policy.2;
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
        stats.prepare_eligible_rebuilds += 1;
        stats.prepare_group_rebuilds += 1;
        stats.prepare_opaque_group_rebuilds += u64::from(!blend);
        self.counters.set(stats);
        stages.switch(Some(1));
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
                                if let Some(reason) =
                                    scratch.append_compatible(item, &models, cache_enabled)
                                {
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
        stats.fine_compatibility_hits += scratch.compatibility_hits;
        stats.overlap_tests += overlap_tests;
        stats.overlap_node_tests += overlap_node_tests;
        for (total, count) in stats.root_batch_breaks.iter_mut().zip(root_batch_breaks) {
            *total += count;
        }
        for (total, count) in stats.draw_batch_breaks.iter_mut().zip(draw_batch_breaks) {
            *total += count;
        }
        self.counters.set(stats);
        // Fine metadata regressed for a heavily changing fleet. Count only
        // models actually contributing to this view, never global primitive
        // publications: a single large or offscreen model must not discard the
        // unaffected static groups. Weight current eligible items, not equal
        // root votes: small moving fleets can dominate work beside many small
        // static roots. At least eight changed roots exclude single-root
        // invalidation. The epoch is monotonic across lifecycle.
        let epoch = self.model_data_epoch.get();
        let previous = self
            .model_data_marks
            .get(view_slot)
            .copied()
            .unwrap_or(epoch);
        let mut changed = 0usize;
        let mut changed_items = 0usize;
        if cache_enabled {
            if let Some(mark) = self.model_data_marks.get_mut(view_slot) {
                *mark = epoch;
            }
            for (index, (_, model)) in models.iter().enumerate() {
                if !scratch.model_ranges[index].is_empty() && model.data_revision > previous {
                    changed += 1;
                    changed_items += scratch.model_ranges[index].len();
                }
            }
        }
        let dynamic_fallback = cache_enabled
            && previous > 0
            && changed >= 8
            && scratch.ordered.len() >= 512
            && changed_items.saturating_mul(4) >= scratch.ordered.len().saturating_mul(3);
        if dynamic_fallback {
            if view_slot < PLAN_SLOTS {
                self.fine_views.borrow_mut()[view_slot] = None;
            }
            let mut stats = self.counters.get();
            stats.fine_dynamic_fallbacks += 1;
            self.counters.set(stats);
        }
        // Singleton-only views have no payload work to save; ordinary Rome
        // pilots showed metadata overhead here. Keep their original builder.
        let no_instances = !scratch.groups.iter().any(|g| g.len() > 1);
        if cache_enabled && no_instances {
            if view_slot < PLAN_SLOTS {
                self.fine_views.borrow_mut()[view_slot] = None;
            }
            let mut stats = self.counters.get();
            stats.fine_no_instance_fallbacks += 1;
            self.counters.set(stats);
        }
        let mut fine =
            if cache_enabled && !dynamic_fallback && !no_instances && view_slot < PLAN_SLOTS {
                let estimate = scratch
                    .ordered
                    .len()
                    .saturating_mul(std::mem::size_of::<FineSource>())
                    + scratch.groups.len().saturating_mul(
                        std::mem::size_of::<FineGroup>() + std::mem::size_of::<PreparedDraw>(),
                    )
                    + if self.diagnostics.get() {
                        scratch
                            .ordered
                            .len()
                            .saturating_mul(std::mem::size_of::<(u32, usize, usize)>())
                    } else {
                        0
                    };
                if estimate <= PLAN_BYTES {
                    let old = self.fine_views.borrow_mut()[view_slot].take();
                    Some(
                        old.filter(|v| {
                            v.storage_key == storage_key && v.diagnostics == self.diagnostics.get()
                        })
                        .unwrap_or_else(|| FineView {
                            storage_key,
                            diagnostics: self.diagnostics.get(),
                            groups: Vec::new(),
                        }),
                    )
                } else {
                    self.fine_views.borrow_mut()[view_slot] = None;
                    let mut stats = self.counters.get();
                    stats.fine_cache_budget_fallbacks += 1;
                    self.counters.set(stats);
                    None
                }
            } else {
                None
            };
        if let Some(v) = fine.as_mut() {
            v.groups
                .resize_with(scratch.groups.len(), FineGroup::default);
            v.groups.truncate(scratch.groups.len());
        }
        stages.switch(Some(4));
        let mut prepared_draws = Vec::with_capacity(scratch.groups.len());
        // Blend preparation shares this key with the opaque preparation and
        // occurs before its submission. It must never overwrite/drop its stream.
        let mut records = 0usize;
        let mut required_bytes = 0usize;
        stages.switch(Some(2));
        for (group_index, group) in scratch.groups.iter().enumerate() {
            let item = scratch.ordered[group.start];
            let first = &models[item.model].1.draws[item.draw].last_uniform;
            let allowed = group.len() > 1 && !blend && !key.shadow && !key.clipped;
            let translated = if let Some(view) = fine.as_mut() {
                let mut stats = self.counters.get();
                view.groups[group_index].select(
                    &scratch.ordered[group.clone()],
                    &models,
                    allowed,
                    &mut stats,
                );
                self.counters.set(stats);
                view.groups[group_index].translated
            } else {
                allowed
                    && first.affine_basis()
                    && scratch.ordered[group.clone()].iter().all(|item| {
                        first.same_basis(&models[item.model].1.draws[item.draw].last_uniform)
                    })
            };
            scratch.translated.push(translated);
            if group.len() > 1 {
                records = records.saturating_add(group.len());
                let payload_bytes = group
                    .len()
                    .saturating_mul(if translated { 16 } else { 112 });
                let reserved = if let Some(view) = fine.as_mut() {
                    let g = &mut view.groups[group_index];
                    g.active_bytes = payload_bytes;
                    // Keep current group slots stable across small culling/count
                    // changes. Bind only live bytes; padding is never rendered.
                    g.reserved_bytes = g.reserved_bytes.max(payload_bytes.next_power_of_two());
                    g.reserved_bytes
                } else {
                    payload_bytes
                };
                required_bytes = required_bytes.saturating_add(reserved);
            } else if let Some(view) = fine.as_mut() {
                view.groups[group_index].active_bytes = 0;
                required_bytes =
                    required_bytes.saturating_add(view.groups[group_index].reserved_bytes);
            }
        }
        stages.switch(Some(5));
        let stream_limit = device
            .limits()
            .max_buffer_size
            .min(self.stream_byte_limit as u64);
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
                let reason = usize::from(existing.is_some());
                existing = Some(InstanceStream {
                    generation: self.resource_id(),
                    buffer: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("glTF pass-view instance stream"),
                        size: capacity as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }),
                    capacity,
                    last: Vec::new(),
                    scratch: Vec::new(),
                    lease: self.pool_ledger.lease(capacity, 0, reason),
                });
            } else {
                let mut m = self.pool_ledger.0.get();
                m.reuses += 1;
                self.pool_ledger.0.set(m);
            }
            if fine.is_some() {
                existing.as_mut().unwrap().last.resize(required_bytes, 0);
            } else {
                existing.as_mut().unwrap().scratch.clear();
            }
            existing
        } else {
            None
        };
        if !blend {
            if stream.is_some() {
                self.used_streams.insert(owner);
            } else {
                self.used_streams.remove(&owner);
            }
        }
        let mut page_index = 0;
        let mut stream_offset = 0usize;
        scratch.uploads.clear();
        for (group_index, group) in scratch.groups.iter().enumerate() {
            let translated = scratch.translated[group_index];
            let first = scratch.ordered[group.start];
            let model = models[first.model].1;
            let d = &model.draws[first.draw];
            let material = &model.materials[d.material];
            let instanced = group.len() > 1;
            let variant = Variant {
                cheap_fragment: self.experiment == 1,
                key,
                texture_mask: material.texture_mask,
                opaque_unclipped: material.alpha == AlphaMode::Opaque
                    && !key.clipped
                    && !key.shadow,
                topology: d.topology,
                strip_index_format: matches!(
                    d.topology,
                    wgpu::PrimitiveTopology::LineStrip | wgpu::PrimitiveTopology::TriangleStrip
                )
                .then_some(d.index_format),
                blend,
                double_sided: material.double_sided,
                negative: d.negative,
                skinned: d.skin.is_some(),
                instanced,
                translated,
                compact_vertex: d.rigid,
                omit_color: d.omit_color,
            };
            let mut stats = self.counters.get();
            if !instanced && d.uploaded_uniform.get() != d.last_uniform {
                queue.write_buffer(&d.uniform, 0, as_bytes(&[d.last_uniform]));
                self.content_epoch.set(
                    self.content_epoch
                        .get()
                        .checked_add(1)
                        .expect("glTF content epoch exhausted"),
                );
                d.uploaded_uniform.set(d.last_uniform);
                stats.buffer_upload_calls += 1;
                stats.uniform_upload_bytes += std::mem::size_of::<Uniform>() as u64;
            }
            let mut draw_bind = d.bind.clone();
            let mut material_lease = None;
            let mut row_storage = (d.identity, 0, 0, 0);
            let instance = if instanced {
                let payload_bytes = group.len() * if translated { 16 } else { 112 };
                if fine.is_none() {
                    stages.switch(Some(3));
                    stats.prepare_payload_rebuilds += 1;
                    scratch.payload.clear();
                    for item in &scratch.ordered[group.clone()] {
                        let draw = &models[item.model].1.draws[item.draw];
                        if translated {
                            scratch
                                .payload
                                .extend_from_slice(as_bytes(&[draw.last_uniform.model[3]]));
                        } else {
                            let packed = RigidInstance::from_uniform(&draw.last_uniform);
                            stats.fine_full_record_packs += 1;
                            scratch.payload.extend_from_slice(as_bytes(&[packed]));
                        }
                    }
                    stages.switch(Some(5));
                }
                let pages = self
                    .instance_pages
                    .entry((storage_key, view_slot))
                    .or_default();
                let capacity = payload_bytes.next_power_of_two();
                if pages.len() <= page_index {
                    stats.instance_pages_created += 1;
                    stats.instance_buffers_created += 1;
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
                    let generation = self.next_resource.get() + 1;
                    self.next_resource.set(generation);
                    pages.push(InstancePage {
                        generation,
                        material_buffer,
                        bind,
                        material: None,
                        buffer: None,
                        capacity: 0,
                        last: Vec::new(),
                        material_lease: self.pool_ledger.lease(
                            std::mem::size_of::<Uniform>(),
                            1,
                            2,
                        ),
                        buffer_lease: None,
                    });
                } else {
                    let mut m = self.pool_ledger.0.get();
                    m.reuses += 1;
                    self.pool_ledger.0.set(m);
                }
                let page = &mut pages[page_index];
                if stream.is_none() && (page.buffer.is_none() || page.capacity < capacity) {
                    let reason = if page.buffer.is_some() { 4 } else { 3 };
                    page.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("glTF rigid instances"),
                        size: capacity as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }));
                    page.buffer_lease = Some(self.pool_ledger.lease(capacity, 0, reason));
                    page.capacity = capacity;
                    page.last.clear();
                    page.generation = self.next_resource.get() + 1;
                    self.next_resource.set(page.generation);
                    stats.instance_buffers_created += 1;
                }
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
                    self.content_epoch.set(
                        self.content_epoch
                            .get()
                            .checked_add(1)
                            .expect("glTF content epoch exhausted"),
                    );
                    page.material = Some(material);
                    stats.buffer_upload_calls += 1;
                    stats.uniform_upload_bytes += std::mem::size_of::<Uniform>() as u64;
                }
                draw_bind = page.bind.clone();
                material_lease = Some(page.material_lease.clone());
                let binding = if let Some(s) = stream.as_mut() {
                    let offset = if fine.is_some() {
                        stream_offset
                    } else {
                        s.scratch.len()
                    };
                    stages.switch(Some(3));
                    if let Some(view) = fine.as_mut() {
                        view.groups[group_index].patch(
                            &mut s.last,
                            s.generation,
                            offset,
                            &scratch.ordered[group.clone()],
                            &models,
                            &mut stats,
                            &mut scratch.uploads,
                        );
                    } else {
                        s.scratch.extend_from_slice(&scratch.payload);
                    }
                    stages.switch(Some(5));
                    row_storage = (
                        page.generation,
                        s.generation,
                        offset as u64,
                        payload_bytes as u64,
                    );
                    stream_offset += fine
                        .as_ref()
                        .map_or(payload_bytes, |v| v.groups[group_index].reserved_bytes);
                    InstanceBinding {
                        buffer: s.buffer.clone(),
                        offset: offset as u64,
                        bytes: payload_bytes as u64,
                        _lease: s.lease.clone(),
                    }
                } else {
                    let buffer = page.buffer.as_ref().expect("standalone instance buffer");
                    scratch.uploads.clear();
                    if let Some(view) = fine.as_mut() {
                        stages.switch(Some(3));
                        page.last.resize(payload_bytes, 0);
                        view.groups[group_index].patch(
                            &mut page.last,
                            page.generation,
                            0,
                            &scratch.ordered[group.clone()],
                            &models,
                            &mut stats,
                            &mut scratch.uploads,
                        );
                        stages.switch(Some(5));
                    } else if page.last != scratch.payload {
                        if scratch.uploads.diagnostic {
                            let clock = web_sys::window().and_then(|w| w.performance());
                            let start = clock.as_ref().map(|p| p.now());
                            stats.instance_changed_bytes += scratch
                                .payload
                                .iter()
                                .enumerate()
                                .filter(|(i, b)| page.last.get(*i).copied().unwrap_or(0) != **b)
                                .count()
                                as u64;
                            stats.instance_diff_scanned_bytes += scratch.payload.len() as u64;
                            if let (Some(clock), Some(start)) = (clock, start) {
                                stats.instance_range_cpu_ms += clock.now() - start;
                            }
                        }
                        page.last.clone_from(&scratch.payload);
                        scratch.uploads.mark(0, payload_bytes);
                    }
                    let dirty_bytes = scratch.uploads.changed_records_bytes as u64;
                    stats.instance_dirty_record_bytes += dirty_bytes;
                    stats.instance_range_overflows += u64::from(scratch.uploads.overflow);
                    let mut sent = 0;
                    for &(start, end) in scratch.uploads.writes(payload_bytes) {
                        queue.write_buffer(buffer, start as u64, &page.last[start..end]);
                        self.content_epoch.set(
                            self.content_epoch
                                .get()
                                .checked_add(1)
                                .expect("glTF content epoch exhausted"),
                        );
                        stats.buffer_upload_calls += 1;
                        stats.instance_upload_calls += 1;
                        stats.uniform_upload_bytes += (end - start) as u64;
                        stats.instance_upload_bytes += (end - start) as u64;
                        sent += (end - start) as u64;
                    }
                    stats.instance_gap_upload_bytes += sent - dirty_bytes;
                    row_storage = (page.generation, 0, 0, payload_bytes as u64);
                    InstanceBinding {
                        buffer: buffer.clone(),
                        offset: 0,
                        bytes: payload_bytes as u64,
                        _lease: page.buffer_lease.as_ref().unwrap().clone(),
                    }
                };
                page_index += 1;
                Some(binding)
            } else {
                if stream.is_some() {
                    stream_offset += fine
                        .as_ref()
                        .map_or(0, |v| v.groups[group_index].reserved_bytes);
                }
                None
            };
            let retained = fine.as_ref().and_then(|v| {
                let g = &v.groups[group_index];
                g.row
                    .as_ref()
                    .filter(|r| {
                        g.members_same && g.row_storage == row_storage && r.variant == variant
                    })
                    .cloned()
            });
            let row = if let Some(row) = retained {
                stats.fine_prepared_rows_reused += 1;
                row
            } else {
                stats.prepare_record_rebuilds += 1;
                if fine.is_some() {
                    stats.fine_prepared_rows_created += 1;
                }
                self.counters.set(stats);
                self.ensure_pipeline(device, variant);
                self.ensure_pipeline(
                    device,
                    Variant {
                        negative: !variant.negative,
                        ..variant
                    },
                );
                stats = self.counters.get();
                stages.switch(Some(4));
                let row = Rc::new(PreparedDraw {
                    variant,
                    textures: material.textures.clone(),
                    bind: draw_bind,
                    _material_lease: material_lease,
                    vertex: d.vertex.clone(),
                    index: d.index.clone(),
                    index_format: d.index_format,
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

                if let Some(view) = fine.as_mut() {
                    let g = &mut view.groups[group_index];
                    g.row = Some(row.clone());
                    g.row_storage = row_storage;
                }
                row
            };
            prepared_draws.push(row);
            self.counters.set(stats);
            stages.switch(Some(5));
        }
        if let Some(mut s) = stream {
            if fine.is_none() && s.last != s.scratch && !s.scratch.is_empty() {
                if scratch.uploads.diagnostic {
                    let mut stats = self.counters.get();
                    let clock = web_sys::window().and_then(|w| w.performance());
                    let start = clock.as_ref().map(|p| p.now());
                    stats.instance_changed_bytes += s
                        .scratch
                        .iter()
                        .enumerate()
                        .filter(|(i, b)| s.last.get(*i).copied().unwrap_or(0) != **b)
                        .count() as u64;
                    stats.instance_diff_scanned_bytes += s.scratch.len() as u64;
                    if let (Some(clock), Some(start)) = (clock, start) {
                        stats.instance_range_cpu_ms += clock.now() - start;
                    }
                    self.counters.set(stats);
                }
                scratch.uploads.mark(0, s.scratch.len());
            }
            let data = if fine.is_some() { &s.last } else { &s.scratch };
            let dirty_bytes = scratch.uploads.changed_records_bytes as u64;
            let mut stats = self.counters.get();
            stats.instance_dirty_record_bytes += dirty_bytes;
            stats.instance_range_overflows += u64::from(scratch.uploads.overflow);
            let mut sent = 0;
            for &(start, end) in scratch.uploads.writes(data.len()) {
                queue.write_buffer(&s.buffer, start as u64, &data[start..end]);
                self.content_epoch.set(
                    self.content_epoch
                        .get()
                        .checked_add(1)
                        .expect("glTF content epoch exhausted"),
                );
                stats.buffer_upload_calls += 1;
                stats.instance_upload_calls += 1;
                stats.uniform_upload_bytes += (end - start) as u64;
                stats.instance_upload_bytes += (end - start) as u64;
                sent += (end - start) as u64;
            }
            stats.instance_gap_upload_bytes += sent - dirty_bytes;
            self.counters.set(stats);
            if fine.is_none() && sent > 0 {
                std::mem::swap(&mut s.last, &mut s.scratch);
            }
            self.instance_streams.insert((storage_key, view_slot), s);
        }
        if !blend {
            self.active_instance_pages
                .insert((storage_key, view_slot), page_index);
        }
        stages.switch(Some(4));
        let mut stats = self.counters.get();
        stats.queue_scratch_growths += capacities_before
            .into_iter()
            .zip(scratch.capacities())
            .filter(|(before, after)| before < after)
            .count() as u64;
        self.counters.set(stats);
        self.queue_scratch = scratch;
        let prepared = PreparedDraws {
            draws: Rc::new(prepared_draws),
            upload_bytes: (self.counters.get().uniform_upload_bytes - upload_before) as usize,
        };
        if let Some(view) = fine {
            let bytes = view.bytes();
            if bytes <= PLAN_BYTES {
                let mut entries = self.fine_views.borrow_mut();
                let mut total = entries.iter().flatten().map(FineView::bytes).sum::<usize>();
                for entry in entries.iter_mut() {
                    if total + bytes <= PLAN_BYTES {
                        break;
                    }
                    if let Some(old) = entry.take() {
                        total -= old.bytes();
                    }
                }
                entries[view_slot] = Some(view);
            } else {
                let mut stats = self.counters.get();
                stats.fine_cache_budget_fallbacks += 1;
                self.counters.set(stats);
            }
        }
        if cache_enabled {
            plan_key.content = self.content_epoch.get();
            self.plan_cache.borrow_mut().store(
                view_slot,
                plan_key,
                &models,
                prepared.draws.clone(),
            );
        }
        if !blend {
            self.trim_instance_pool();
        }
        if let Some(times) = stages.finish() {
            let mut stats = self.counters.get();
            stats.prepare_timed_calls += 1;
            for (total, elapsed) in stats.prepare_stage_ms.iter_mut().zip(times) {
                *total += elapsed;
            }
            self.counters.set(stats);
        }
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
        for d in prepared.draws.iter() {
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
            pass.set_index_buffer(d.index.slice(..), d.index_format);
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
        cheap_fragment,
        omit_color,
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
    if omit_color {
        assert!(!skinned, "color omission is rigid-only");
        // Do this after skin/instance declarations have been injected. Omit the
        // input fetch, but retain the color interpolator: replacing its smooth
        // perspective result with fragment constant 1 changed a repeatable XR
        // pixel (max 2/255). Keep the original finite-precision fragment path.
        source = source.replacen(", @location(8) color:vec4<f32>", "", 1);
        source = source.replace("o.color=v.color;", "o.color=vec4<f32>(1.0);");
    }
    if cheap_fragment && !shadow {
        let start = source
            .find("fn surface(v:Output,front:bool)->vec4<f32> {")
            .expect("surface entry");
        let end = start
            + source[start..]
                .find("@fragment fn fs_main")
                .expect("surface end");
        source.replace_range(start..end, "fn surface(v:Output,front:bool)->vec4<f32> {\n    let base=base_color(v);\n    surface_alpha_clip(v,base.a);\n    return base;\n}\n");
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
/// Optional diagnostic work counts. No per-primitive writes in clean runs.
#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlendSelectionStats {
    pub gltf_models_visited: u64,
    pub gltf_primitives_visited: u64,
    pub gltf_active_tests: u64,
    pub gltf_immutable_rejects: u64,
    pub gltf_no_pose_rejects: u64,
}

/// Presence, not a published pose count. The rejection class is immutable;
/// models containing BLEND still consult the live active set, including after
/// a failed pose update. Visibility and transparent ordering stay downstream.
pub fn has_blend_draw<const DIAGNOSTICS: bool>(
    model: &GpuModel,
    reference: bool,
    stats: &mut BlendSelectionStats,
) -> bool {
    if DIAGNOSTICS {
        stats.gltf_models_visited += 1;
    }
    if model.last_pose.is_none() {
        if DIAGNOSTICS {
            stats.gltf_no_pose_rejects += 1;
        }
        return false;
    }
    if reference {
        if !DIAGNOSTICS {
            return draw_count(model, true) > 0;
        }
        // Match the original full-count traversal and active-before-alpha order.
        let mut count = 0;
        for draw in &model.draws {
            stats.gltf_primitives_visited += 1;
            stats.gltf_active_tests += 1;
            if model.active.contains(&draw.node)
                && model.materials[draw.material].alpha == AlphaMode::Blend
            {
                count += 1;
            }
        }
        return count > 0;
    }
    if !model.has_blend_draws {
        if DIAGNOSTICS {
            stats.gltf_immutable_rejects += 1;
        }
        return false;
    }
    model.draws.iter().any(|draw| {
        if DIAGNOSTICS {
            stats.gltf_primitives_visited += 1;
        }
        if model.materials[draw.material].alpha != AlphaMode::Blend {
            return false;
        }
        if DIAGNOSTICS {
            stats.gltf_active_tests += 1;
        }
        model.active.contains(&draw.node)
    })
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
fn topology(mode: PrimitiveMode) -> wgpu::PrimitiveTopology {
    match mode {
        PrimitiveMode::Points => wgpu::PrimitiveTopology::PointList,
        PrimitiveMode::Lines => wgpu::PrimitiveTopology::LineList,
        PrimitiveMode::LineStrip => wgpu::PrimitiveTopology::LineStrip,
        PrimitiveMode::Triangles => wgpu::PrimitiveTopology::TriangleList,
        PrimitiveMode::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
        PrimitiveMode::LineLoop | PrimitiveMode::TriangleFan => {
            unreachable!("normalized index topology")
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
