//! Opt-in prototype: one local boundary-distance quad per actual mesh-font glyph.
use super::target::{RenderTarget, RenderView};
use crate::cutting_mesh::MeshData;
use crate::visibility::InstanceData;
use glam::{Mat4, Vec2, Vec3};
use std::collections::{BTreeMap, HashMap};
use wgpu::util::DeviceExt;
const RES: u32 = 256;
const MAX_GLYPHS: usize = 128;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ViewUniform {
    vp: [[f32; 4]; 4],
    camera: [f32; 4],
    params: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GlyphUniform {
    bounds: [f32; 4],
    depth_pad: [f32; 4],
    front: [f32; 4],
    back: [f32; 4],
}
struct Glyph {
    source: MeshData,
    data: Option<std::sync::Arc<GlyphData>>,
}
struct GlyphData {
    _texture: wgpu::Texture,
    _uniform: wgpu::Buffer,
    group: wgpu::BindGroup,
    pad: f32,
}
pub(super) struct FontGlow {
    uniform: wgpu::Buffer,
    view_layout: wgpu::BindGroupLayout,
    view_group: wgpu::BindGroup,
    glyph_layout: wgpu::BindGroupLayout,
    pipelines: HashMap<(wgpu::TextureFormat, wgpu::TextureFormat), wgpu::RenderPipeline>,
    glyphs: HashMap<u32, Glyph>,
    // Exact content keys survive collection/recreation of font asset IDs.
    warmed: HashMap<Vec<u32>, Option<std::sync::Arc<GlyphData>>>,
    owners: HashMap<Vec<u32>, std::collections::HashSet<u32>>,
    active: Option<(wgpu::TextureFormat, wgpu::TextureFormat)>,
    eligible: std::collections::HashSet<u32>,
    bakes: u64,
    eligible_instances: usize,
    drawn_instances: std::cell::Cell<usize>,
}
impl FontGlow {
    pub fn new(device: &wgpu::Device) -> Self {
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("font halo view"),
            size: std::mem::size_of::<ViewUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let view_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("font halo view"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let view_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("font halo view"),
            layout: &view_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let glyph_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("font halo glyph"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        Self {
            uniform,
            view_layout,
            view_group,
            glyph_layout,
            pipelines: HashMap::new(),
            glyphs: HashMap::new(),
            warmed: HashMap::new(),
            owners: HashMap::new(),
            active: None,
            eligible: Default::default(),
            bakes: 0,
            eligible_instances: 0,
            drawn_instances: std::cell::Cell::new(0),
        }
    }
    pub fn stats(&self) -> serde_json::Value {
        serde_json::json!({"cachedGlyphs":self.glyphs.len(),"bakes":self.bakes,
            "readyGlyphs":self.warmed.values().filter(|g|g.is_some()).count(),
            "prewarmedSources":self.warmed.len(),
            "eligibleGlyphs":self.eligible.len(),"eligibleInstances":self.eligible_instances,
            "drawnInstances":self.drawn_instances.get(),"textureBytes":self.warmed.values().filter(|g|g.is_some()).count()* (RES*RES*4) as usize,
            "resolution":RES,"maxGlyphs":MAX_GLYPHS})
    }
    pub fn retain(&mut self, ids: &std::collections::HashSet<u32>) {
        self.glyphs.retain(|id, _| ids.contains(id));
        self.eligible.retain(|id| ids.contains(id));
        if self.glyphs.is_empty() {
            self.active = None;
            self.eligible_instances = 0;
            self.drawn_instances.set(0);
        }
    }
    /// Resolve a new live handle using warmed content only. Unknown glyphs
    /// retain ordinary capsules; render preparation never bakes or uploads.
    pub fn bind_source(&mut self, id: u32, source: &MeshData) {
        if self
            .glyphs
            .get(&id)
            .is_some_and(|g| g.source.same_source(source))
        {
            return;
        }
        if self.glyphs.len() >= MAX_GLYPHS && !self.glyphs.contains_key(&id) {
            return;
        }
        let data = signature(source).and_then(|key| self.warmed.get(&key).cloned().flatten());
        self.glyphs.insert(
            id,
            Glyph {
                source: source.clone(),
                data,
            },
        );
    }
    pub fn retain_fonts(&mut self, fonts: &std::collections::HashSet<u32>) {
        self.owners.retain(|_, owners| {
            owners.retain(|id| fonts.contains(id));
            !owners.is_empty()
        });
        self.warmed.retain(|key, _| self.owners.contains_key(key));
    }
    /// Explicit loading-time operation; preserves already-warmed content.
    pub fn prewarm(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sources: Vec<(u32, MeshData)>,
        fonts: &std::collections::HashSet<u32>,
    ) {
        self.retain_fonts(fonts);
        for (font, source) in sources {
            let Some(key) = signature(&source) else {
                continue;
            };
            if self.warmed.contains_key(&key) {
                self.owners.entry(key).or_default().insert(font);
                continue;
            }
            if self.warmed.len() >= MAX_GLYPHS {
                continue;
            }
            self.prewarm_source(device, queue, &source, key.clone());
            self.owners.entry(key).or_default().insert(font);
        }
        // Previously unknown live handles must resolve again after explicit warmup.
        self.glyphs.clear();
        self.eligible.clear();
        self.active = None;
        self.eligible_instances = 0;
        self.drawn_instances.set(0);
    }
    fn prewarm_source(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &MeshData,
        key: Vec<u32>,
    ) {
        self.bakes += 1;
        let data = bake(source).map(|(uniform, pixels)| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("font boundary distance"),
                size: wgpu::Extent3d {
                    width: RES,
                    height: RES,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
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
                bytemuck::cast_slice(&pixels),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(RES * 4),
                    rows_per_image: Some(RES),
                },
                texture.size(),
            );
            let view = texture.create_view(&Default::default());
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("font halo glyph bounds"),
                contents: bytemuck::bytes_of(&uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("font halo glyph"),
                layout: &self.glyph_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: buffer.as_entire_binding(),
                    },
                ],
            });
            GlyphData {
                _texture: texture,
                _uniform: buffer,
                group,
                pad: uniform.depth_pad[2],
            }
        });
        self.warmed.insert(key, data.map(std::sync::Arc::new));
    }
    #[allow(clippy::too_many_arguments)] // Explicit GPU, view and effect preparation state.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: RenderView,
        target: &RenderTarget<'_>,
        strength: f32,
        radius: f32,
        batches: &BTreeMap<u32, Vec<crate::visibility::InstanceData>>,
    ) {
        self.eligible.clear();
        self.eligible_instances = 0;
        self.drawn_instances.set(0);
        for (id, instances) in batches {
            let Some(data) = self.glyphs.get(id).and_then(|g| g.data.as_ref()) else {
                continue;
            };
            if !instances.is_empty()
                && instances
                    .iter()
                    .all(|i| eligible(i, view.camera_position, radius, data.pad))
            {
                self.eligible.insert(*id);
                self.eligible_instances += instances.len();
            }
        }
        self.active = None;
        if self.eligible.is_empty() {
            return;
        }
        let key = (target.color_format, target.depth_format);
        self.pipelines.entry(key).or_insert_with(|| {
            let shader = device.create_shader_module(wgpu::include_wgsl!("font_glow.wgsl"));
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("font halo"),
                bind_group_layouts: &[Some(&self.view_layout), Some(&self.glyph_layout)],
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("font halo quad"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(InstanceData::layout())],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target.color_format,
                        write_mask: wgpu::ColorWrites::ALL,
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
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: target.depth_format,
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
            bytemuck::bytes_of(&ViewUniform {
                vp: view.view_projection.to_cols_array_2d(),
                camera: view.camera_position.extend(1.).to_array(),
                params: [strength, radius, 0., 0.],
            }),
        );
        self.active = Some(key);
    }
    pub fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        id: u32,
        instances: wgpu::BufferSlice<'a>,
        count: u32,
    ) -> bool {
        if !self.eligible.contains(&id) {
            return false;
        }
        let Some(key) = self.active else { return false };
        let Some(data) = self.glyphs.get(&id).and_then(|g| g.data.as_ref()) else {
            return false;
        };
        pass.set_pipeline(&self.pipelines[&key]);
        pass.set_bind_group(0, &self.view_group, &[]);
        pass.set_bind_group(1, &data.group, &[]);
        pass.set_vertex_buffer(0, instances);
        pass.draw(0..6, 0..count);
        self.drawn_instances
            .set(self.drawn_instances.get() + count as usize);
        true
    }
}
fn eligible(i: &crate::visibility::InstanceData, camera: Vec3, radius: f32, pad: f32) -> bool {
    let m = Mat4::from_cols_array_2d(&i.model);
    let axes = [
        m.x_axis.truncate(),
        m.y_axis.truncate(),
        m.z_axis.truncate(),
    ];
    let s = axes[0].length();
    s.is_finite()
        && s > 1e-5
        && i.normal_matrix[3][3] >= 0.
        && radius * 1.6 / s < pad * 0.8
        && axes.iter().all(|a| (a.length() / s - 1.).abs() < 0.001)
        && axes[0].dot(axes[1]).abs() < s * s * 0.001
        && axes[0].dot(axes[2]).abs() < s * s * 0.001
        && axes[1].dot(axes[2]).abs() < s * s * 0.001
        // A planar halo omits the extruded side silhouette. Restrict this
        // approximation to within ~26 degrees of either cap normal; oblique
        // and side views retain the original geometric edge capsules.
        && axes[2]
            .normalize()
            .dot((camera - m.w_axis.truncate()).normalize_or_zero())
            .abs()
            >= 0.9
}
// Extract cap boundaries, including hole contours; no filled silhouette shortcut.
fn bake(source: &MeshData) -> Option<(GlyphUniform, Vec<f32>)> {
    let mesh = source.ready_cpu()?;
    let (min, max) = source.bounds();
    let extent = max - min;
    let span = extent.x.max(extent.y);
    if !span.is_finite() || span <= 0. || extent.z > span * 0.5 {
        return None;
    }
    type Edge = ([u32; 2], [u32; 2]);
    let mut caps: [BTreeMap<Edge, u32>; 2] = Default::default();
    let mut colors = [Vec3::ZERO; 2];
    let mut counts = [0u32; 2];
    let eps = span * 1e-5;
    for t in &mesh.triangles {
        let p = t.indices.map(|i| mesh.positions[i as usize]);
        for (side, z) in [min.z, max.z].into_iter().enumerate() {
            if !p.iter().all(|p| (p.z - z).abs() <= eps) {
                continue;
            }
            colors[side] += t.color;
            counts[side] += 1;
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                let bits = |p: Vec3| {
                    [
                        if p.x == 0. { 0 } else { p.x.to_bits() },
                        if p.y == 0. { 0 } else { p.y.to_bits() },
                    ]
                };
                let mut e = [bits(p[a]), bits(p[b])];
                e.sort_unstable();
                *caps[side].entry((e[0], e[1])).or_default() += 1;
            }
        }
    }
    if counts.contains(&0) {
        return None;
    }
    for c in &mut caps {
        c.retain(|_, count| *count == 1);
    }
    if caps[0].is_empty() || caps[0] != caps[1] || caps[0].len() > 4096 {
        return None;
    }
    let edges: Vec<_> = caps[0]
        .keys()
        .map(|(a, b)| {
            (
                Vec2::new(f32::from_bits(a[0]), f32::from_bits(a[1])),
                Vec2::new(f32::from_bits(b[0]), f32::from_bits(b[1])),
            )
        })
        .collect();
    let pad = span;
    let lo = min.truncate() - Vec2::splat(pad);
    let hi = max.truncate() + Vec2::splat(pad);
    let mut pixels = Vec::with_capacity((RES * RES) as usize);
    for y in 0..RES {
        for x in 0..RES {
            let p = lo
                + (hi - lo)
                    * Vec2::new((x as f32 + 0.5) / RES as f32, (y as f32 + 0.5) / RES as f32);
            let d = edges
                .iter()
                .map(|(a, b)| {
                    let e = *b - *a;
                    let t = ((p - *a).dot(e) / e.length_squared().max(1e-20)).clamp(0., 1.);
                    p.distance(*a + e * t)
                })
                .fold(f32::INFINITY, f32::min);
            pixels.push(d);
        }
    }
    Some((
        GlyphUniform {
            bounds: [lo.x, lo.y, hi.x, hi.y],
            depth_pad: [min.z, max.z, pad, 0.],
            front: (colors[1] / counts[1] as f32).extend(1.).to_array(),
            back: (colors[0] / counts[0] as f32).extend(1.).to_array(),
        },
        pixels,
    ))
}

fn signature(source: &MeshData) -> Option<Vec<u32>> {
    let mesh = source.ready_cpu()?;
    let mut key = Vec::with_capacity(2 + mesh.positions.len() * 3 + mesh.triangles.len() * 6);
    key.extend([mesh.positions.len() as u32, mesh.triangles.len() as u32]);
    for p in &mesh.positions {
        key.extend(p.to_array().map(f32::to_bits));
    }
    for t in &mesh.triangles {
        key.extend(t.indices);
        key.extend(t.color.to_array().map(f32::to_bits));
    }
    Some(key)
}
