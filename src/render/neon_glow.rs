//! Texture-free halos drawn over actual bright mesh edges.
use super::{data::Vertex, target::RenderView};
use crate::{gltf_loader::ImportedMesh, visibility::InstanceData};
use glam::Vec3;
use std::collections::{BTreeMap, HashMap};
use wgpu::util::DeviceExt;

pub struct GlowMesh {
    pub buffer: wgpu::Buffer,
    pub vertices: u32,
}
struct GlowEdge {
    color_sum: Vec3,
    count: usize,
    first_normal: Vec3,
    crease: bool,
}
impl GlowMesh {
    pub fn new(device: &wgpu::Device, mesh: &ImportedMesh) -> Option<Self> {
        let mut vertices = Vec::new();
        let mut edges = BTreeMap::<[[u32; 3]; 2], GlowEdge>::new();
        let max_vertices = (device.limits().max_buffer_size / std::mem::size_of::<Vertex>() as u64)
            .min(u32::MAX as u64) as usize;
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let Some(a) = mesh.vertices.get(triangle[0] as usize) else {
                continue;
            };
            let Some(b) = mesh.vertices.get(triangle[1] as usize) else {
                continue;
            };
            let Some(c) = mesh.vertices.get(triangle[2] as usize) else {
                continue;
            };
            let color = (Vec3::from(a.color) + Vec3::from(b.color) + Vec3::from(c.color)) / 3.0;
            let points = [
                Vec3::from(a.position),
                Vec3::from(b.position),
                Vec3::from(c.position),
            ];
            let face_normal = (points[1] - points[0]).cross(points[2] - points[0]);
            let normal_length_squared = face_normal.length_squared();
            if !color.is_finite()
                || color.max_element() < 0.25
                || points.iter().any(|p| !p.is_finite())
                || !normal_length_squared.is_finite()
                || normal_length_squared <= 1e-20
            {
                continue;
            }
            let face_normal = face_normal / normal_length_squared.sqrt();
            // Weld split mesh vertices by exact position, including signed zero.
            let bits = |p: Vec3| {
                p.to_array()
                    .map(|value| if value == 0.0 { 0 } else { value.to_bits() })
            };
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                let length_squared = points[a].distance_squared(points[b]);
                if !length_squared.is_finite() || length_squared <= 1e-20 {
                    continue;
                }
                let mut ends = [bits(points[a]), bits(points[b])];
                ends.sort_unstable();
                edges
                    .entry(ends)
                    .and_modify(|edge| {
                        edge.color_sum += color;
                        edge.count += 1;
                        // Winding supplies consistently outward face normals.
                        edge.crease |= edge.first_normal.dot(face_normal) < 0.95;
                    })
                    .or_insert(GlowEdge {
                        color_sum: color,
                        count: 1,
                        first_normal: face_normal,
                        crease: false,
                    });
            }
        }
        // Ordered keys make generated capsules stable across builds and runs.
        for (ends, edge) in edges {
            // Bright/dark boundaries and creases glow; coplanar diagonals do not.
            if edge.count != 1 && !edge.crease {
                continue;
            }
            let vertex = Vertex {
                position: ends[0].map(f32::from_bits),
                color: (edge.color_sum / edge.count as f32).to_array(),
                normal: ends[1].map(f32::from_bits),
            };
            if vertices.len().checked_add(6)? > max_vertices {
                return None;
            }
            vertices.extend_from_slice(&[vertex; 6]);
        }
        if vertices.is_empty() {
            return None;
        }
        let count = u32::try_from(vertices.len()).ok()?;
        Some(Self {
            buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("neon glow edge capsules"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            vertices: count,
        })
    }
}
impl Drop for GlowMesh {
    fn drop(&mut self) {
        self.buffer.destroy();
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_projection: [[f32; 4]; 4],
    // Pixel viewport dimensions, strength, world-space radius.
    params: [f32; 4],
}
type PipelineKey = (wgpu::TextureFormat, wgpu::TextureFormat);

pub struct NeonGlow {
    strength: f32,
    radius: f32,
    uniform: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    active: Option<PipelineKey>,
}
impl NeonGlow {
    pub fn new(device: &wgpu::Device) -> Self {
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("neon glow view uniform"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("neon glow view layout"),
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
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("neon glow view"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        Self {
            strength: 0.0,
            radius: 0.015,
            uniform,
            layout,
            bind_group,
            pipelines: HashMap::new(),
            active: None,
        }
    }

    pub fn configure(&mut self, strength: f32, radius: f32) -> bool {
        if !strength.is_finite()
            || !radius.is_finite()
            || !(0.0..=2.0).contains(&strength)
            || !(0.001..=0.05).contains(&radius)
        {
            return false;
        }
        self.strength = strength;
        self.radius = radius;
        true
    }
    pub fn enabled(&self) -> bool {
        self.strength > 0.0
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: RenderView,
        target: &super::target::RenderTarget<'_>,
    ) {
        let color_format = target.color_format;
        let depth_format = target.depth_format;
        debug_assert!(target.auxiliary.is_none());
        let width = target.viewport.width;
        let height = target.viewport.height;
        if !self.enabled() || width == 0 || height == 0 {
            self.active = None;
            return;
        }
        let key = (color_format, depth_format);
        self.pipelines.entry(key).or_insert_with(|| {
            let shader = device.create_shader_module(wgpu::include_wgsl!("neon_glow.wgsl"));
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("neon glow pipeline layout"),
                bind_group_layouts: &[Some(&self.layout)],
                immediate_size: 0,
            });
            let targets = vec![Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Max,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                // Preserve alpha via the blend equation, while leaving the WebGL
                // color mask complete for browser surface presentation/readback.
                write_mask: wgpu::ColorWrites::ALL,
            })];
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("neon glow max-blended capsules"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(Vertex::layout()), Some(InstanceData::layout())],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &targets,
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth_format,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        });
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&Uniforms {
                view_projection: view.view_projection.to_cols_array_2d(),
                params: [width as f32, height as f32, self.strength, self.radius],
            }),
        );
        self.active = Some(key);
    }

    pub fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        mesh: &'a GlowMesh,
        instance_buffer: &'a wgpu::Buffer,
        instance_count: u32,
    ) {
        if !self.enabled() || instance_count == 0 {
            return;
        }
        let Some(key) = self.active else { return };
        pass.set_pipeline(&self.pipelines[&key]);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, mesh.buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        pass.draw(0..mesh.vertices, 0..instance_count);
    }
}
impl Drop for NeonGlow {
    fn drop(&mut self) {
        self.uniform.destroy();
    }
}
