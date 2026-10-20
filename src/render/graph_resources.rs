//! Owns actual GPU allocations for a compiled graph's transient slots.
//! Keep this object alongside cached render targets; rebuild on plan/size changes.
use crate::frame_graph::{CompiledGraph, ResourceDesc, ResourceId};

enum Slot {
    Texture {
        texture: wgpu::Texture,
        view: wgpu::TextureView,
    },
    Buffer(wgpu::Buffer),
}

pub struct GraphResources {
    resource_slots: Vec<Option<usize>>,
    slots: Vec<Slot>,
    allocated_bytes_estimate: u64,
}

impl GraphResources {
    /// External and persistent resources remain caller-owned. No usage flags
    /// are added to the declarations and no device-global cache is installed.
    /// Device-specific format/usage support is still validated by wgpu.
    pub fn new(device: &wgpu::Device, plan: &CompiledGraph) -> Result<Self, String> {
        let limits = device.limits();
        for slot in plan.resource_slots.iter().flatten() {
            if *slot >= plan.slots.len() {
                return Err(format!(
                    "frame graph maps a resource to missing slot {slot}"
                ));
            }
        }
        // Validate the whole plan before creating the first allocation.
        for (index, desc) in plan.slots.iter().enumerate() {
            validate_slot(desc, &limits)
                .map_err(|error| format!("frame graph slot {index}: {error}"))?;
        }
        let mut slots = Vec::with_capacity(plan.slots.len());
        let mut allocated_bytes_estimate = 0u64;
        for (index, desc) in plan.slots.iter().enumerate() {
            let label = format!("frame graph transient slot {index}");
            allocated_bytes_estimate =
                allocated_bytes_estimate.saturating_add(estimate_bytes(desc));
            slots.push(match *desc {
                ResourceDesc::Texture {
                    width,
                    height,
                    layers,
                    samples,
                    format,
                    usage,
                } => {
                    let texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(&label),
                        size: wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: layers,
                        },
                        mip_level_count: 1,
                        sample_count: samples,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage,
                        view_formats: &[],
                    });
                    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                    Slot::Texture { texture, view }
                }
                ResourceDesc::Buffer { size, usage } => {
                    Slot::Buffer(device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(&label),
                        size,
                        usage,
                        mapped_at_creation: false,
                    }))
                }
            });
        }
        Ok(Self {
            resource_slots: plan.resource_slots.clone(),
            slots,
            allocated_bytes_estimate,
        })
    }

    pub fn view(&self, resource: ResourceId) -> Result<&wgpu::TextureView, String> {
        match self.slot(resource)? {
            Slot::Texture { view, .. } => Ok(view),
            Slot::Buffer(_) => Err(format!(
                "resource {} is a buffer, not a texture",
                resource.0
            )),
        }
    }

    pub fn texture(&self, resource: ResourceId) -> Result<&wgpu::Texture, String> {
        match self.slot(resource)? {
            Slot::Texture { texture, .. } => Ok(texture),
            Slot::Buffer(_) => Err(format!(
                "resource {} is a buffer, not a texture",
                resource.0
            )),
        }
    }

    pub fn buffer(&self, resource: ResourceId) -> Result<&wgpu::Buffer, String> {
        match self.slot(resource)? {
            Slot::Buffer(buffer) => Ok(buffer),
            Slot::Texture { .. } => Err(format!(
                "resource {} is a texture, not a buffer",
                resource.0
            )),
        }
    }

    pub fn texture_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| matches!(slot, Slot::Texture { .. }))
            .count()
    }

    pub fn buffer_count(&self) -> usize {
        self.slots.len() - self.texture_count()
    }

    /// Texel/block payload only: excludes implementation alignment and metadata.
    /// Opaque depth/stencil formats use an approximate 4/8 bytes per texel.
    pub fn allocated_bytes_estimate(&self) -> u64 {
        self.allocated_bytes_estimate
    }

    fn slot(&self, resource: ResourceId) -> Result<&Slot, String> {
        let slot = self.resource_slots.get(resource.0)
            .ok_or_else(|| format!("unknown frame graph resource {}", resource.0))?
            .ok_or_else(|| format!(
                "resource {} has no transient allocation (unused or caller-owned external/persistent resource)", resource.0
            ))?;
        Ok(&self.slots[slot])
    }
}

fn validate_slot(desc: &ResourceDesc, limits: &wgpu::Limits) -> Result<(), String> {
    match *desc {
        ResourceDesc::Texture {
            width,
            height,
            layers,
            samples,
            format,
            usage,
        } => {
            if width == 0
                || height == 0
                || layers == 0
                || width > limits.max_texture_dimension_2d
                || height > limits.max_texture_dimension_2d
                || layers > limits.max_texture_array_layers
            {
                return Err("invalid 2D texture extent or device texture limit exceeded".into());
            }
            if !matches!(samples, 1 | 2 | 4 | 8 | 16) || (samples > 1 && layers != 1) {
                return Err("invalid sample count or multisampled array texture".into());
            }
            if usage.is_empty() || format.is_multi_planar_format() {
                return Err("texture requires nonempty usage and a single-plane format".into());
            }
            let (block_width, block_height) = format.block_dimensions();
            if !width.is_multiple_of(block_width) || !height.is_multiple_of(block_height) {
                return Err("texture dimensions must align to format block dimensions".into());
            }
        }
        ResourceDesc::Buffer { size, usage } => {
            if size == 0 || size > limits.max_buffer_size || usage.is_empty() {
                return Err("invalid buffer size/usage or device buffer limit exceeded".into());
            }
        }
    }
    Ok(())
}

fn estimate_bytes(desc: &ResourceDesc) -> u64 {
    match *desc {
        ResourceDesc::Buffer { size, .. } => size,
        ResourceDesc::Texture {
            width,
            height,
            layers,
            samples,
            format,
            ..
        } => {
            let (block_width, block_height) = format.block_dimensions();
            let block_size = format.block_copy_size(None).unwrap_or_else(|| {
                if format == wgpu::TextureFormat::Depth32FloatStencil8 {
                    8
                } else {
                    4
                }
            });
            u64::from(width.div_ceil(block_width))
                .saturating_mul(u64::from(height.div_ceil(block_height)))
                .saturating_mul(u64::from(layers))
                .saturating_mul(u64::from(samples))
                .saturating_mul(u64::from(block_size))
        }
    }
}
