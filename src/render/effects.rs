//! Persistent pass resources, scheduling and dirty keys, independent of canvas/DOM.
use super::target::{RenderTarget, RenderView};
use super::{
    data::{self, EffectsUniforms, Light, MAX_LIGHTS},
    textures::{ColorTarget, DepthTarget},
};
use crate::visibility::RenderPassKind;
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;
#[derive(Default)]
pub struct EffectStats {
    pub static_updates: u32,
    pub dynamic_updates: u32,
    pub reflection_updates: u32,
    pub static_visible: usize,
    static_visible_by_light: [usize; MAX_LIGHTS],
    dynamic_visible_by_light: [usize; MAX_LIGHTS],
    pub dynamic_visible: usize,
    pub reflection_visible: usize,
    pub passes: u32,
    pub shadow_light_count: usize,
    pub static_updates_by_light: [u32; MAX_LIGHTS],
    pub dynamic_updates_by_light: [u32; MAX_LIGHTS],
}
pub struct Effects {
    pub layout: wgpu::BindGroupLayout,
    pub uniform: wgpu::Buffer,
    last_uniform: EffectsUniforms,
    pub scene_group: wgpu::BindGroup,
    pub reflection_group: wgpu::BindGroup,
    pub static_shadow: [DepthTarget; MAX_LIGHTS],
    pub dynamic_shadow: [DepthTarget; MAX_LIGHTS],
    pub reflection: ColorTarget,
    fallback: ColorTarget,
    comparison: wgpu::Sampler,
    sampler: wgpu::Sampler,
    pub static_key: [Option<(Mat4, u64, bool)>; MAX_LIGHTS],
    pub dynamic_key: [Option<(Mat4, u64, bool)>; MAX_LIGHTS],
    pub reflection_key: Option<(Mat4, u64, [Light; MAX_LIGHTS], bool, bool)>,
    pub shadows: bool,
    pub reflections: bool,
    pub stats: EffectStats,
}
impl Effects {
    pub fn new(device: &wgpu::Device) -> Self {
        let mut entries = vec![];
        for binding in [6, 7, 8, 9, 10, 11] {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
        }
        entries.extend_from_slice(&[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ]);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene effects layout"),
            entries: &entries,
        });
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("effects uniforms"),
            contents: data::as_bytes(&[EffectsUniforms::default()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let static_shadow = std::array::from_fn(|_| DepthTarget::new(device, "static shadows", 1));
        let dynamic_shadow =
            std::array::from_fn(|_| DepthTarget::new(device, "dynamic shadows", 1));
        let reflection = ColorTarget::new(
            device,
            "planar reflection",
            1,
            1,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );
        let fallback = ColorTarget::new(
            device,
            "reflection fallback",
            1,
            1,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );
        let comparison = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow PCF sampler"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("reflection sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let scene_group = Self::group(
            device,
            &layout,
            &uniform,
            &static_shadow,
            &dynamic_shadow,
            &comparison,
            &reflection,
            &sampler,
        );
        let reflection_group = Self::group(
            device,
            &layout,
            &uniform,
            &static_shadow,
            &dynamic_shadow,
            &comparison,
            &fallback,
            &sampler,
        );
        Self {
            layout,
            uniform,
            last_uniform: EffectsUniforms::default(),
            scene_group,
            reflection_group,
            static_shadow,
            dynamic_shadow,
            reflection,
            fallback,
            comparison,
            sampler,
            static_key: [None; MAX_LIGHTS],
            dynamic_key: [None; MAX_LIGHTS],
            reflection_key: None,
            shadows: false,
            reflections: false,
            stats: Default::default(),
        }
    }
    fn group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform: &wgpu::Buffer,
        static_shadow: &[DepthTarget; MAX_LIGHTS],
        dynamic_shadow: &[DepthTarget; MAX_LIGHTS],
        comparison: &wgpu::Sampler,
        reflection: &ColorTarget,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(comparison),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&reflection.color),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ];
        for i in 0..MAX_LIGHTS {
            let binding = if i == 0 { 1 } else { 4 + i as u32 * 2 };
            entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(&static_shadow[i].view),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: binding + 1,
                resource: wgpu::BindingResource::TextureView(&dynamic_shadow[i].view),
            });
        }
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene effects"),
            layout,
            entries: &entries,
        })
    }
    fn write_uniform(&mut self, queue: &wgpu::Queue, value: EffectsUniforms) -> usize {
        if self.last_uniform == value {
            return 0;
        }
        queue.write_buffer(&self.uniform, 0, data::as_bytes(&[value]));
        self.last_uniform = value;
        std::mem::size_of::<EffectsUniforms>()
    }
    pub fn ensure(&mut self, device: &wgpu::Device, width: u32, height: u32, light_count: usize) {
        let mut changed = false;
        for i in 0..light_count {
            if self.shadows && self.static_shadow[i].size != 1024 {
                self.static_shadow[i] = DepthTarget::new(device, "static shadows", 1024);
                self.dynamic_shadow[i] = DepthTarget::new(device, "dynamic shadows", 1024);
                self.static_key[i] = None;
                self.dynamic_key[i] = None;
                changed = true;
            }
        }
        // Half resolution, capped at 1024 on the longer axis, with camera aspect preserved.
        let scale = (0.5f32).min(1024.0 / width.max(height) as f32);
        let w = (width as f32 * scale).round().max(1.0) as u32;
        let h = (height as f32 * scale).round().max(1.0) as u32;
        if self.reflections && (self.reflection.width != w || self.reflection.height != h) {
            self.reflection = ColorTarget::new(
                device,
                "planar reflection",
                w,
                h,
                wgpu::TextureFormat::Rgba8UnormSrgb,
            );
            self.reflection_key = None;
            changed = true;
        }
        if changed {
            self.scene_group = Self::group(
                device,
                &self.layout,
                &self.uniform,
                &self.static_shadow,
                &self.dynamic_shadow,
                &self.comparison,
                &self.reflection,
                &self.sampler,
            );
            self.reflection_group = Self::group(
                device,
                &self.layout,
                &self.uniform,
                &self.static_shadow,
                &self.dynamic_shadow,
                &self.comparison,
                &self.fallback,
                &self.sampler,
            );
        }
        self.stats.passes = 0;
    }
}

impl Effects {
    pub(super) fn render(
        &mut self,
        renderer: &mut super::renderer::SceneRenderer,
        view: RenderView,
        target: &RenderTarget<'_>,
    ) -> Result<(), String> {
        let light_count = renderer.scene.lights().len().min(MAX_LIGHTS);
        self.ensure(&renderer.device, target.width, target.height, light_count);
        renderer.upload_bytes = 0;
        let mut lights = [super::data::EMPTY_LIGHT; MAX_LIGHTS];
        for (dst, light) in lights.iter_mut().zip(renderer.scene.lights().values()) {
            *dst = Light {
                position: light.position.to_array(),
                _pad_p: 0.0,
                color: light.color.to_array(),
                _pad_c: 0.0,
            };
        }
        let shadow_count = if self.shadows { light_count } else { 0 };
        self.stats.shadow_light_count = shadow_count;
        let mut shadow_matrices = [Mat4::IDENTITY; MAX_LIGHTS];
        for i in 0..shadow_count {
            shadow_matrices[i] =
                crate::render_math::shadow_view_projection(Vec3::from_array(lights[i].position));
            for pass in [RenderPassKind::ShadowStatic, RenderPassKind::ShadowDynamic] {
                let key = (
                    shadow_matrices[i],
                    if pass == RenderPassKind::ShadowStatic {
                        renderer.scene.static_revision()
                    } else {
                        renderer.scene.render_revision()
                    },
                    renderer.frustum_culling,
                );
                let previous = if pass == RenderPassKind::ShadowStatic {
                    self.static_key[i]
                } else {
                    self.dynamic_key[i]
                };
                if previous == Some(key) {
                    continue;
                }
                renderer.prepare_shadow_pass(
                    RenderView {
                        view_projection: shadow_matrices[i],
                        camera_position: Vec3::from_array(lights[i].position),
                    },
                    pass,
                    i,
                );
                let map = if pass == RenderPassKind::ShadowStatic {
                    &self.static_shadow[i]
                } else {
                    &self.dynamic_shadow[i]
                };
                renderer.draw_depth(&map.target())?;
                let visible = renderer.pass_visible();
                if pass == RenderPassKind::ShadowStatic {
                    self.static_key[i] = Some(key);
                    self.stats.static_updates += 1;
                    self.stats.static_updates_by_light[i] += 1;
                    self.stats.static_visible_by_light[i] = visible;
                } else {
                    self.dynamic_key[i] = Some(key);
                    self.stats.dynamic_updates += 1;
                    self.stats.dynamic_updates_by_light[i] += 1;
                    self.stats.dynamic_visible_by_light[i] = visible;
                }
                self.stats.passes += 1;
            }
        }
        self.stats.static_visible = self.stats.static_visible_by_light[..shadow_count]
            .iter()
            .sum();
        self.stats.dynamic_visible = self.stats.dynamic_visible_by_light[..shadow_count]
            .iter()
            .sum();
        let mut uniforms = EffectsUniforms {
            shadow_matrices: shadow_matrices.map(|matrix| matrix.to_cols_array_2d()),
            ..Default::default()
        };
        uniforms.params[0] = shadow_count as f32;
        if let Some(plane) = renderer.scene.mirror_plane().filter(|_| {
            self.reflections
                && renderer
                    .scene
                    .mirror_visible(view.view_projection, renderer.frustum_culling)
        }) {
            let reflection_matrix = view.view_projection * plane.matrix();
            uniforms.reflection_matrix = reflection_matrix.to_cols_array_2d();
            let key = (
                reflection_matrix,
                renderer.scene.render_revision(),
                lights,
                renderer.frustum_culling,
                self.shadows,
            );
            if self.reflection_key != Some(key) {
                uniforms.clip_plane = plane.clip_plane(view.camera_position).to_array();
                uniforms.params[2] = 1.0;
                renderer.upload_bytes += self.write_uniform(&renderer.queue, uniforms);
                renderer.prepare_pass(
                    RenderView {
                        view_projection: reflection_matrix,
                        camera_position: plane.matrix().transform_point3(view.camera_position),
                    },
                    RenderPassKind::Reflection,
                );
                renderer.draw_color(&self.reflection.target(), self, true)?;
                self.stats.reflection_visible = renderer.pass_visible();
                self.stats.reflection_updates += 1;
                self.stats.passes += 1;
                self.reflection_key = Some(key);
            }
            uniforms.params[1] = 1.0;
        }
        uniforms.params[2] = 0.0;
        uniforms.clip_plane = [0.0; 4];
        renderer.upload_bytes += self.write_uniform(&renderer.queue, uniforms);
        renderer.prepare_pass(view, RenderPassKind::Main);
        renderer.draw_color(target, self, false)?;
        self.stats.passes += 1;
        Ok(())
    }
}
