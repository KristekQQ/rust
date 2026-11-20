//! Worker-owned native replay experiment. Each slot keeps its buffers until
//! the last scene/renderer lease is gone. New writes are ordered on the same GPU queue.
use super::{data, Mesh, PreparedCutBuffers};
use crate::gltf_loader::ImportedMesh;
use std::sync::{Arc, Weak};

const MAX_PAIRS: usize = 32;
const MAX_CAPACITY: u64 = 1024 * 1024;

#[derive(Clone, Copy, Default, serde::Serialize)]
pub struct CutPoolStats {
    pub pairs: usize,
    pub reserved_bytes: u64,
    pub reuses: usize,
    pub fallbacks: usize,
}
struct Slot {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    owner: Weak<()>,
}
impl Slot {
    fn bytes(&self) -> u64 {
        self.vertices.size() + self.indices.size()
    }
}

pub(crate) struct CutBufferPool {
    queue: wgpu::Queue,
    slots: Vec<Slot>,
    pub(crate) stats: CutPoolStats,
}
impl CutBufferPool {
    pub(crate) fn new(queue: wgpu::Queue) -> Self {
        Self {
            queue,
            slots: Vec::new(),
            stats: CutPoolStats::default(),
        }
    }

    // OpenXR and the replay submit their eye commands before the next scene
    // update can drop its last mesh lease. write_buffer on the SAME queue
    // executes after those draws even while they are still in flight.
    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        model: &ImportedMesh,
    ) -> PreparedCutBuffers {
        let vertices = data::as_bytes(&model.vertices);
        let indices = data::as_bytes(&model.indices);
        let vertex_len = vertices.len() as u64;
        let index_len = indices.len() as u64;
        let best = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.owner.strong_count() == 0
                    && s.vertices.size() >= vertex_len
                    && s.indices.size() >= index_len
            })
            .min_by_key(|(_, s)| s.bytes())
            .map(|(i, _)| i);
        let slot = if let Some(index) = best {
            self.stats.reuses += 1;
            &mut self.slots[index]
        } else {
            let vertex_size = vertex_len.max(1).next_multiple_of(4096);
            let index_size = index_len.max(1).next_multiple_of(256);
            let reserved = self.stats.reserved_bytes;
            if self.slots.len() == MAX_PAIRS || reserved + vertex_size + index_size > MAX_CAPACITY {
                self.stats.fallbacks += 1;
                return PreparedCutBuffers::new(device, model);
            }
            let buffer = |label, size, usage| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size,
                    usage: usage | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            };
            self.slots.push(Slot {
                vertices: buffer(
                    "pooled cut vertices",
                    vertex_size,
                    wgpu::BufferUsages::VERTEX,
                ),
                indices: buffer("pooled cut indices", index_size, wgpu::BufferUsages::INDEX),
                owner: Weak::new(),
            });
            self.stats.pairs += 1;
            self.stats.reserved_bytes += vertex_size + index_size;
            self.slots.last_mut().expect("new cut buffer slot")
        };
        self.queue.write_buffer(&slot.vertices, 0, vertices);
        self.queue.write_buffer(&slot.indices, 0, indices);
        let lease = Arc::new(());
        slot.owner = Arc::downgrade(&lease);
        PreparedCutBuffers {
            device: device.clone(),
            mesh: Mesh {
                vertex_buffer: slot.vertices.clone(),
                index_buffer: slot.indices.clone(),
                index_count: model.indices.len() as u32,
                index_format: wgpu::IndexFormat::Uint32,
                panel_dynamic: false,
                panel_indices: Vec::new(),
                _cut_lease: Some(lease),
            },
        }
    }
}
