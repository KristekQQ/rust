//! WebGPU geometry preparation only; topology remains owned by `cutting`.
use std::collections::{HashMap, HashSet};

use js_sys::Promise;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wgpu::util::DeviceExt;

use crate::cutting::CutGeometryInput;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Parameters {
    plane: [f32; 4],
    counts: [u32; 4],
}

pub struct ComputeCutter {
    pipeline: wgpu::ComputePipeline,
    positions: HashMap<u32, wgpu::Buffer>,
}

/// Owns request-local readback resources; no mapped buffer enters the cache.
pub struct PendingCutGeometry {
    staging: Option<wgpu::Buffer>,
    rows: Vec<usize>,
    error: Option<String>,
}

// Also unmaps if a caller cancels `read` while its Promise is pending.
struct Mapping(wgpu::Buffer);

impl Drop for Mapping {
    fn drop(&mut self) {
        self.0.unmap();
    }
}

impl ComputeCutter {
    pub fn retain(&mut self, alive: impl Fn(u32) -> bool) {
        self.positions.retain(|&id, _| alive(id));
    }
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cut geometry preparation"),
            source: wgpu::ShaderSource::Wgsl(include_str!("cutting_compute.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("cut geometry preparation"),
            layout: None,
            module: &shader,
            entry_point: Some("prepare"),
            compilation_options: Default::default(),
            cache: None,
        });
        Self {
            pipeline,
            positions: HashMap::new(),
        }
    }

    /// Mesh IDs identify immutable position arrays. Edges and planes are per request.
    /// Only caller-supplied BVH candidates are evaluated, in their original order.
    pub fn dispatch(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        inputs: &[CutGeometryInput],
    ) -> PendingCutGeometry {
        let ids: HashSet<_> = inputs.iter().map(|input| input.id).collect();
        self.positions.retain(|id, _| ids.contains(id));
        let mut pending = PendingCutGeometry {
            staging: None,
            rows: Vec::with_capacity(inputs.len()),
            error: None,
        };
        let limits = device.limits();
        let storage_limit = limits
            .max_storage_buffer_binding_size
            .min(limits.max_buffer_size);
        let mut total_bytes = 0u64;
        // Validate the entire batch before allocating or submitting any work.
        for input in inputs {
            let vertices = input.positions.len() as u64;
            let edges = input.edges.len() as u64;
            let rows = vertices + edges;
            let output_bytes = rows * 16;
            if rows == 0 {
                pending.rows.push(0);
                continue;
            }
            if vertices > u64::from(u32::MAX)
                || edges > u64::from(u32::MAX)
                || rows > u64::from(u32::MAX)
                || output_bytes > storage_limit
                || (vertices * 16).max(16) > storage_limit
                || (edges * 8).max(8) > storage_limit
                || limits.max_compute_workgroups_per_dimension == 0
                || limits.max_compute_invocations_per_workgroup < 64
                || limits.max_compute_workgroup_size_x < 64
                || limits.max_storage_buffers_per_shader_stage < 3
            {
                pending.error = Some(format!(
                    "Cut geometry input {} exceeds device limits",
                    input.id
                ));
                return pending;
            }
            let groups = (rows as u32).div_ceil(64);
            let x = groups.min(limits.max_compute_workgroups_per_dimension);
            if groups.div_ceil(x) > limits.max_compute_workgroups_per_dimension {
                pending.error = Some("Cut geometry dispatch exceeds workgroup limits".into());
                return pending;
            }
            total_bytes += output_bytes;
            if total_bytes > limits.max_buffer_size {
                pending.error = Some("Cut geometry readback exceeds device buffer limit".into());
                return pending;
            }
            pending.rows.push(rows as usize);
        }
        if total_bytes == 0 {
            return pending;
        }

        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cut geometry batched readback"),
            size: total_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("cut geometry batch"),
        });
        let layout = self.pipeline.get_bind_group_layout(0);
        let mut offset = 0u64;
        for (input, &rows) in inputs.iter().zip(&pending.rows) {
            if rows == 0 {
                continue;
            }
            let positions = self.positions.entry(input.id).or_insert_with(|| {
                let fallback = [[0.0f32; 4]];
                let values = if input.positions.is_empty() {
                    &fallback[..]
                } else {
                    &input.positions[..]
                };
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("cut immutable mesh positions"),
                    contents: bytemuck::cast_slice(values),
                    usage: wgpu::BufferUsages::STORAGE,
                })
            });
            let canonical_edges: Vec<[u32; 2]> = input
                .edges
                .iter()
                .map(|&[i, j]| [i.min(j), i.max(j)])
                .collect();
            let edge_values = if canonical_edges.is_empty() {
                &[[0u32; 2]][..]
            } else {
                &canonical_edges[..]
            };
            let edges = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("cut candidate edges"),
                contents: bytemuck::cast_slice(edge_values),
                usage: wgpu::BufferUsages::STORAGE,
            });
            let parameters = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("cut plane and counts"),
                contents: bytemuck::bytes_of(&Parameters {
                    plane: input.plane,
                    counts: [input.positions.len() as u32, input.edges.len() as u32, 0, 0],
                }),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bytes = rows as u64 * 16;
            let output = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("cut geometry output"),
                size: bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cut geometry bindings"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: positions.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: edges.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: output.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: parameters.as_entire_binding(),
                    },
                ],
            });
            let groups = (rows as u32).div_ceil(64);
            let x = groups.min(limits.max_compute_workgroups_per_dimension);
            let y = groups.div_ceil(x);
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("cut geometry preparation"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups(x, y, 1);
            }
            encoder.copy_buffer_to_buffer(&output, 0, &staging, offset, bytes);
            offset += bytes;
        }
        queue.submit([encoder.finish()]);
        pending.staging = Some(staging);
        pending
    }
}

impl PendingCutGeometry {
    pub async fn read(self) -> Result<Vec<Vec<[f32; 4]>>, String> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let Some(staging) = self.staging else {
            return Ok(self.rows.iter().map(|_| Vec::new()).collect());
        };
        let mapping = Mapping(staging);
        let promise = Promise::new(&mut |resolve, reject| {
            mapping
                .0
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| match result {
                    Ok(()) => {
                        let _ = resolve.call0(&JsValue::UNDEFINED);
                    }
                    Err(error) => {
                        let _ = reject
                            .call1(&JsValue::UNDEFINED, &JsValue::from_str(&error.to_string()));
                    }
                });
        });
        JsFuture::from(promise).await.map_err(|error| {
            error
                .as_string()
                .unwrap_or_else(|| format!("Cut geometry mapping failed: {error:?}"))
        })?;
        let view = mapping
            .0
            .slice(..)
            .get_mapped_range()
            .map_err(|error| error.to_string())?;
        let values: &[[f32; 4]] =
            bytemuck::try_cast_slice(&view).map_err(|error| error.to_string())?;
        let mut offset = 0;
        let mut results = Vec::with_capacity(self.rows.len());
        for rows in self.rows {
            results.push(values[offset..offset + rows].to_vec());
            offset += rows;
        }
        drop(view);
        drop(mapping);
        Ok(results)
    }
}
