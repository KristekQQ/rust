//! Request-local GPU cutting. Successful reads transfer metadata only (640 bytes).
//! The caller owns eligibility, immutable source uploads, and transactional scene changes.
use glam::Vec3;
use js_sys::Promise;
use std::{future::Future, pin::Pin};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use wgpu::util::DeviceExt;

const MAX_TRIANGLES: u32 = 20_000;
pub const RESIDENT_METADATA_BYTES: u64 = 640;
type ErrorFutures = Vec<Pin<Box<dyn Future<Output = Option<wgpu::Error>>>>>;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ResidentVertex {
    pub position: [f32; 4],
    pub normal: [f32; 4],
    pub color: [f32; 4],
}

pub struct ResidentSource {
    /// Immutable triangle soup; requires STORAGE and COPY_SRC usages.
    pub vertices: wgpu::Buffer,
    pub vertex_count: u32,
}

pub struct ResidentCutParams {
    pub a: Vec3,
    pub b: Vec3,
    pub motion: Vec3,
    pub cap_color: Vec3,
    pub source_volume: f32,
    /// First 64 directions address the positive half, next 64 the negative half.
    /// Supports use the same local coordinates as `source`, without recentering.
    pub support_dirs: [[f32; 4]; 128],
}

pub struct ResidentPiece {
    pub source: ResidentSource,
    pub render_vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub vertex_count: u32,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub volume: f32,
    pub support: [f32; 64],
}

pub enum ResidentOutcome {
    Miss,
    Unsupported(String),
    Pieces(Box<[ResidentPiece; 2]>),
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Parameters {
    plane: [f32; 4],
    a: [f32; 4],
    blade: [f32; 4],
    motion: [f32; 4],
    color: [f32; 4],
    // input triangles, per-half vertex capacity, segment capacity, hash slots
    counts: [u32; 4],
    directions: [[f32; 4]; 128],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Metadata {
    count: u32,
    segments: u32,
    status: u32,
    sides: u32,
    lo: [f32; 4],
    hi: [f32; 4],
    volume: [f32; 4],
    support: [f32; 64],
}

const _: () = {
    assert!(std::mem::size_of::<ResidentVertex>() == 48);
    assert!(std::mem::size_of::<Metadata>() * 2 == RESIDENT_METADATA_BYTES as usize);
    assert!(std::mem::size_of::<Parameters>() == 2144);
};

pub struct ResidentCutter {
    layout: wgpu::BindGroupLayout,
    pipelines: [wgpu::ComputePipeline; 5],
    initialization_errors: std::cell::RefCell<ErrorFutures>,
}

pub struct PendingResidentCut {
    staging: wgpu::Buffer,
    output: wgpu::Buffer,
    packed: wgpu::Buffer,
    // Explicit ownership until read/cancellation; command buffers also retain these.
    _retained: [wgpu::Buffer; 4],
    capacity: u32,
    source_volume: f32,
    errors: Vec<Pin<Box<dyn Future<Output = Option<wgpu::Error>>>>>,
}

// Pop immediately before returning control to the renderer: an outstanding
// request must never leave scopes installed on the shared device.
fn error_scopes(device: &wgpu::Device) -> [wgpu::ErrorScopeGuard; 3] {
    [
        device.push_error_scope(wgpu::ErrorFilter::OutOfMemory),
        device.push_error_scope(wgpu::ErrorFilter::Internal),
        device.push_error_scope(wgpu::ErrorFilter::Validation),
    ]
}

fn pop_scopes(
    scopes: [wgpu::ErrorScopeGuard; 3],
) -> Vec<Pin<Box<dyn Future<Output = Option<wgpu::Error>>>>> {
    scopes
        .into_iter()
        .rev()
        .map(|scope| Box::pin(scope.pop()) as Pin<Box<dyn Future<Output = Option<wgpu::Error>>>>)
        .collect()
}

async fn check_errors(
    errors: Vec<Pin<Box<dyn Future<Output = Option<wgpu::Error>>>>>,
) -> Result<(), String> {
    for error in errors {
        if let Some(error) = error.await {
            return Err(format!("Resident cutting GPU error: {error}"));
        }
    }
    Ok(())
}

struct Mapping(wgpu::Buffer);
impl Drop for Mapping {
    fn drop(&mut self) {
        self.0.unmap();
    }
}

fn buffer(
    device: &wgpu::Device,
    label: &str,
    size: u64,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage,
        mapped_at_creation: false,
    })
}

async fn mapped_bytes(staging: wgpu::Buffer) -> Result<Vec<u8>, String> {
    // RAII unmaps even if this future is dropped while map_async is pending.
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
                    let _ =
                        reject.call1(&JsValue::UNDEFINED, &JsValue::from_str(&error.to_string()));
                }
            });
    });
    JsFuture::from(promise).await.map_err(|error| {
        error
            .as_string()
            .unwrap_or_else(|| format!("Resident cut map failed: {error:?}"))
    })?;
    let view = mapping
        .0
        .slice(..)
        .get_mapped_range()
        .map_err(|error| error.to_string())?;
    Ok(view.to_vec())
}

impl ResidentCutter {
    pub fn new(device: &wgpu::Device) -> Self {
        let scopes = error_scopes(device);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("resident cutting"),
            source: wgpu::ShaderSource::Wgsl(include_str!("cutting_resident.wgsl").into()),
        });
        let entries: Vec<_> = (0..6)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: if binding == 5 {
                        wgpu::BufferBindingType::Uniform
                    } else {
                        wgpu::BufferBindingType::Storage {
                            read_only: binding == 0,
                        }
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            })
            .collect();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("resident cutting"),
            entries: &entries,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("resident cutting"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipelines = ["clip", "close_boundary", "measure", "support", "pack"].map(|entry| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        });
        Self {
            layout,
            pipelines,
            initialization_errors: std::cell::RefCell::new(pop_scopes(scopes)),
        }
    }

    pub fn dispatch(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &ResidentSource,
        params: &ResidentCutParams,
    ) -> Result<PendingResidentCut, String> {
        let count = source.vertex_count;
        if count == 0 || !count.is_multiple_of(3) || count / 3 > MAX_TRIANGLES {
            return Err("Resident source must contain 1..20000 complete triangles".into());
        }
        if !source
            .vertices
            .usage()
            .contains(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC)
            || source.vertices.size() < u64::from(count) * 48
        {
            return Err(
                "Resident source requires a sufficiently large STORAGE | COPY_SRC buffer".into(),
            );
        }
        let blade = params.b - params.a;
        let cross = blade.cross(params.motion);
        if !params.a.is_finite()
            || !params.b.is_finite()
            || !params.motion.is_finite()
            || !params.cap_color.is_finite()
            || !cross.is_finite()
            || cross.length() < 1e-5
            || !params.source_volume.is_finite()
            || params.source_volume <= 0.0
            || params.support_dirs.iter().flatten().any(|v| !v.is_finite())
        {
            return Err("Resident cut parameters must be finite, with a nondegenerate sweep and positive volume".into());
        }
        let normal = cross.normalize();
        let plane_d = normal.dot(params.a);
        let uu = blade.length_squared();
        let vv = params.motion.length_squared();
        let uv = blade.dot(params.motion);
        let det = uu * vv - uv * uv;
        if !plane_d.is_finite() || !det.is_finite() || det <= (uu * vv) * 1e-6 {
            return Err("Resident sweep is numerically ill-conditioned".into());
        }
        let triangles = count / 3;
        // Quest-oriented bounded boundary scratch: excessive contour complexity
        // falls back atomically instead of allocating tens of MB per apple.
        let segments = (triangles * 3).min(4096);
        let capacity = (triangles * 9).min(triangles * 3 + segments * 6);
        let hash = (segments * 2).next_power_of_two();
        let output_bytes = u64::from(capacity) * 2 * 48;
        let scratch_bytes = u64::from(segments + 2 * hash) * 2 * 48;
        // Each half is [render: capacity * 36][indices: capacity * 4].
        let packed_bytes = u64::from(capacity) * 2 * 40;
        let limits = device.limits();
        let max_binding = limits
            .max_storage_buffer_binding_size
            .min(limits.max_buffer_size);
        if [
            output_bytes,
            scratch_bytes,
            packed_bytes,
            source.vertices.size(),
        ]
        .iter()
        .any(|&bytes| bytes > max_binding)
            || limits.max_storage_buffers_per_shader_stage < 5
            || limits.max_compute_workgroup_size_x < 64
            || limits.max_compute_invocations_per_workgroup < 64
            || limits.max_compute_workgroups_per_dimension < capacity.div_ceil(64).max(128)
            || limits.max_uniform_buffer_binding_size < std::mem::size_of::<Parameters>() as u64
        {
            return Err("Resident cutting exceeds device compute/buffer limits".into());
        }
        let scopes = error_scopes(device);
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC;
        let output = buffer(device, "resident cut soup scratch", output_bytes, storage);
        let scratch = buffer(
            device,
            "resident cut boundary scratch",
            scratch_bytes,
            wgpu::BufferUsages::STORAGE,
        );
        let packed = buffer(device, "resident cut packed scratch", packed_bytes, storage);
        let metadata = buffer(
            device,
            "resident cut metadata",
            RESIDENT_METADATA_BYTES,
            storage,
        );
        let staging = buffer(
            device,
            "resident cut metadata readback",
            RESIDENT_METADATA_BYTES,
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        );
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("resident cut parameters"),
            contents: bytemuck::bytes_of(&Parameters {
                plane: [normal.x, normal.y, normal.z, plane_d],
                a: params
                    .a
                    .extend(
                        if params.support_dirs[..64]
                            .iter()
                            .all(|d| *d == params.support_dirs[0])
                            && params.support_dirs[64..]
                                .iter()
                                .all(|d| *d == params.support_dirs[64])
                        {
                            1.
                        } else {
                            64.
                        },
                    )
                    .to_array(),
                blade: blade.extend(0.0).to_array(),
                motion: params.motion.extend(0.0).to_array(),
                color: params.cap_color.extend(1.0).to_array(),
                counts: [triangles, capacity, segments, hash],
                directions: params.support_dirs,
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let buffers = [
            &source.vertices,
            &output,
            &scratch,
            &metadata,
            &packed,
            &uniform,
        ];
        let entries: Vec<_> = buffers
            .iter()
            .enumerate()
            .map(|(i, b)| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: b.as_entire_binding(),
            })
            .collect();
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("resident cut request"),
            layout: &self.layout,
            entries: &entries,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("resident cut phases"),
        });
        // Distinct passes provide storage visibility between globally dependent phases.
        for (pipeline, groups) in self.pipelines.iter().zip([
            [triangles.div_ceil(64), 1],
            [2, 1],
            [2, 1],
            [128, 1],
            [capacity.div_ceil(64), 2],
        ]) {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("resident cut phase"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(groups[0], groups[1], 1);
        }
        encoder.copy_buffer_to_buffer(&metadata, 0, &staging, 0, RESIDENT_METADATA_BYTES);
        queue.submit([encoder.finish()]);
        let mut errors = std::mem::take(&mut *self.initialization_errors.borrow_mut());
        errors.extend(pop_scopes(scopes));
        Ok(PendingResidentCut {
            staging,
            output,
            packed,
            _retained: [source.vertices.clone(), scratch, metadata, uniform],
            capacity,
            source_volume: params.source_volume,
            errors,
        })
    }
}

impl PendingResidentCut {
    pub async fn read(
        self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<ResidentOutcome, String> {
        check_errors(self.errors).await?;
        let bytes = mapped_bytes(self.staging).await?;
        let metadata: [Metadata; 2] = bytemuck::pod_read_unaligned(&bytes);
        // Unsupported takes precedence over miss: a damaged source must not be accepted.
        if let Some(m) = metadata.iter().find(|m| m.status >= 2) {
            let reason = match m.status {
                2 => "ambiguous, nonmanifold or multiple cap contours",
                3 => "triangle, segment or hash budget exceeded",
                4 => "nonfinite or near-EPS numerically ambiguous geometry",
                5 => "cap contour has no safe average-center fan",
                _ => "unknown GPU status",
            };
            return Ok(ResidentOutcome::Unsupported(reason.into()));
        }
        if metadata
            .iter()
            .any(|m| m.status == 1 || m.sides != 3 || m.count == 0)
        {
            return Ok(ResidentOutcome::Miss);
        }
        if metadata.iter().any(|m| m.count / 3 > MAX_TRIANGLES) {
            return Ok(ResidentOutcome::Unsupported(
                "Resident piece exceeds the CPU-fallback triangle budget".into(),
            ));
        }
        for m in &metadata {
            if m.count > self.capacity
                || !m.count.is_multiple_of(3)
                || m.lo
                    .iter()
                    .chain(&m.hi)
                    .chain(&m.volume)
                    .chain(&m.support)
                    .any(|v| !v.is_finite())
                || (0..3).any(|i| m.lo[i] > m.hi[i])
                || m.volume[0] <= 0.0
            {
                return Ok(ResidentOutcome::Unsupported(
                    "Invalid resident metadata".into(),
                ));
            }
        }
        let total = metadata[0].volume[0] + metadata[1].volume[0];
        if (total - self.source_volume).abs() > self.source_volume * 0.001 {
            return Ok(ResidentOutcome::Unsupported(
                "Resident volume conservation exceeded 0.001 relative tolerance".into(),
            ));
        }
        let scopes = error_scopes(device);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("resident cut exact GPU copies"),
        });
        let pieces = std::array::from_fn(|half| {
            let m = metadata[half];
            let count = u64::from(m.count);
            let soup = buffer(
                device,
                "resident immutable piece soup",
                count * 48,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            );
            let render = buffer(
                device,
                "resident piece vertices",
                count * 36,
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            );
            let indices = buffer(
                device,
                "resident piece indices",
                count * 4,
                wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            );
            let base = half as u64 * u64::from(self.capacity);
            encoder.copy_buffer_to_buffer(&self.output, base * 48, &soup, 0, count * 48);
            encoder.copy_buffer_to_buffer(&self.packed, base * 40, &render, 0, count * 36);
            encoder.copy_buffer_to_buffer(
                &self.packed,
                base * 40 + u64::from(self.capacity) * 36,
                &indices,
                0,
                count * 4,
            );
            ResidentPiece {
                source: ResidentSource {
                    vertices: soup,
                    vertex_count: m.count,
                },
                render_vertices: render,
                indices,
                vertex_count: m.count,
                bounds_min: Vec3::from_slice(&m.lo[..3]),
                bounds_max: Vec3::from_slice(&m.hi[..3]),
                volume: m.volume[0],
                support: m.support,
            }
        });
        queue.submit([encoder.finish()]);
        check_errors(pop_scopes(scopes)).await?;
        Ok(ResidentOutcome::Pieces(Box::new(pieces)))
    }
}

impl ResidentSource {
    /// Explicit diagnostic geometry readback; never used by the successful cut path.
    pub async fn read_vertices(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<Vec<ResidentVertex>, String> {
        if self.vertex_count == 0 {
            return Ok(Vec::new());
        }
        let size = u64::from(self.vertex_count) * 48;
        if size > self.vertices.size()
            || !self.vertices.usage().contains(wgpu::BufferUsages::COPY_SRC)
        {
            return Err("Resident diagnostic source is not copyable or is truncated".into());
        }
        let scopes = error_scopes(device);
        let staging = buffer(
            device,
            "resident diagnostic geometry readback",
            size,
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("resident diagnostics"),
        });
        encoder.copy_buffer_to_buffer(&self.vertices, 0, &staging, 0, size);
        queue.submit([encoder.finish()]);
        check_errors(pop_scopes(scopes)).await?;
        let bytes = mapped_bytes(staging).await?;
        Ok(bytes
            .as_chunks::<48>()
            .0
            .iter()
            .map(|bytes| bytemuck::pod_read_unaligned(bytes))
            .collect())
    }
}
