//! Explicit GPU acceptance probe for graph-allocated storage buffers.
use super::graph_resources::GraphResources;
use crate::frame_graph::{Graph, Lifetime, ResourceDesc};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

const VALUES: u32 = 64;
const BYTES: u64 = VALUES as u64 * 4;
const SHADER: &str = r#"
@group(0) @binding(0) var<storage, read_write> destination: array<u32>;
@group(0) @binding(1) var<storage, read> source: array<u32>;
@compute @workgroup_size(64)
fn fill(@builtin(global_invocation_id) id: vec3<u32>) {
    destination[id.x] = id.x + 1u;
}
@compute @workgroup_size(64)
fn double_values(@builtin(global_invocation_id) id: vec3<u32>) {
    destination[id.x] = source[id.x] * 2u;
}
"#;

struct Mapping(wgpu::Buffer);
impl Drop for Mapping {
    fn drop(&mut self) {
        self.0.unmap();
    }
}

pub async fn run(device: wgpu::Device, queue: wgpu::Queue) -> Result<serde_json::Value, String> {
    if device.limits().max_compute_workgroups_per_dimension == 0 {
        return Ok(serde_json::json!({
            "supported": false, "reason": "compute shaders unavailable on this backend"
        }));
    }
    let mut graph = Graph::default();
    let descriptor = ResourceDesc::Buffer {
        size: BYTES,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    };
    let a = graph.resource("first.input", descriptor.clone(), Lifetime::Transient);
    let b = graph.resource("first.export", descriptor.clone(), Lifetime::Transient);
    let c = graph.resource("later.input", descriptor.clone(), Lifetime::Transient);
    let d = graph.resource("later.export", descriptor, Lifetime::Transient);
    let first_fill = graph.pass("first.fill", &[], &[a], false)?;
    let first_double = graph.pass("first.double", &[a], &[b], false)?;
    let later_fill = graph.pass("later.fill", &[], &[c], false)?;
    let later_double = graph.pass("later.double", &[c], &[d], false)?;
    let plan = graph.compile(&[b, d])?;
    if plan.resource_slots[a.0] == plan.resource_slots[b.0]
        || plan.resource_slots[c.0] == plan.resource_slots[d.0]
        || plan.resource_slots[a.0] != plan.resource_slots[c.0]
        || plan.resource_slots[b.0] == plan.resource_slots[c.0]
        || plan.resource_slots[b.0] == plan.resource_slots[d.0]
        || plan.resource_lifetimes[b.0].map(|range| range.last_use) != Some(3)
    {
        return Err("storage buffer aliasing or exported-output lifetime is invalid".into());
    }
    let resources = GraphResources::new(&device, &plan)?;
    if resources.buffer_count() != 3 || resources.texture_count() != 0 {
        return Err("expected three physical storage buffers for four logical buffers".into());
    }
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("frame graph buffer acceptance"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline = |entry| {
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(entry),
            layout: None,
            module: &shader,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            cache: None,
        })
    };
    let fill = pipeline("fill");
    let double = pipeline("double_values");
    let fill_group = |resource| -> Result<wgpu::BindGroup, String> {
        Ok(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame graph fill buffer"),
            layout: &fill.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: resources.buffer(resource)?.as_entire_binding(),
            }],
        }))
    };
    let double_group = |source, destination| -> Result<wgpu::BindGroup, String> {
        Ok(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame graph double buffers"),
            layout: &double.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: resources.buffer(destination)?.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: resources.buffer(source)?.as_entire_binding(),
                },
            ],
        }))
    };
    let first_fill_group = fill_group(a)?;
    let later_fill_group = fill_group(c)?;
    let first_double_group = double_group(a, b)?;
    let later_double_group = double_group(c, d)?;
    let readback = Mapping(device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frame graph exports readback"),
        size: BYTES * 2,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    }));
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("compiled frame graph buffer execution"),
    });
    for id in &plan.ordered_passes {
        let (pipeline, bindings) = if *id == first_fill {
            (&fill, &first_fill_group)
        } else if *id == first_double {
            (&double, &first_double_group)
        } else if *id == later_fill {
            (&fill, &later_fill_group)
        } else if *id == later_double {
            (&double, &later_double_group)
        } else {
            return Err(format!("missing GPU handler for compiled pass {}", id.0));
        };
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: graph.pass_name(*id),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bindings, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(resources.buffer(b)?, 0, &readback.0, 0, BYTES);
    encoder.copy_buffer_to_buffer(resources.buffer(d)?, 0, &readback.0, BYTES, BYTES);
    queue.submit(Some(encoder.finish()));
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        readback
            .0
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| match result {
                Ok(()) => {
                    let _ = resolve.call0(&JsValue::UNDEFINED);
                }
                Err(error) => {
                    let _ =
                        reject.call1(&JsValue::UNDEFINED, &JsValue::from_str(&error.to_string()));
                }
            });
    });
    JsFuture::from(promise)
        .await
        .map_err(|error| format!("buffer readback map failed: {error:?}"))?;
    let data = readback
        .0
        .slice(..)
        .get_mapped_range()
        .map_err(|error| error.to_string())?;
    for (index, bytes) in data.as_chunks::<4>().0.iter().enumerate() {
        let actual = u32::from_le_bytes(*bytes);
        let expected = 2 * (index as u32 % VALUES + 1);
        if actual != expected {
            return Err(format!(
                "buffer export {} element {}: expected {expected}, received {actual}",
                index / VALUES as usize,
                index % VALUES as usize
            ));
        }
    }
    drop(data);
    Ok(serde_json::json!({
        "supported": true,
        "verifiedValues": VALUES * 2,
        "logicalBuffers": 4,
        "physicalBuffers": resources.buffer_count(),
        "allocatedBytesEstimate": resources.allocated_bytes_estimate(),
        "overlapSeparated": true,
        "nonoverlapAliased": true,
        "exportRetained": true,
        "graph": graph.debug_summary(&plan),
    }))
}
