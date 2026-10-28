//! Opt-in WebGL XR: owned sRGB eye targets and a borrowed, render-only XR FBO.
//! No raw GL binds; the pinned GLES backend owns state transitions and submission.
use super::{
    target::{RenderTarget, Viewport},
    textures::ColorTarget,
};
use wasm_bindgen::{JsCast, JsValue};

pub fn context(device: &wgpu::Device) -> Result<web_sys::WebGl2RenderingContext, JsValue> {
    // SAFETY: clone the context handle only; never mutate/destroy HAL resources.
    unsafe { device.as_hal::<wgpu::hal::api::Gles>() }
        .map(|hal| hal.context().webgl2_context.clone())
        .ok_or_else(|| JsValue::from_str("WebGL device unavailable"))
}
struct Eye {
    target: ColorTarget,
    source: wgpu::BindGroup,
    ready: bool,
}
pub struct GlXrOutput {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pipeline: wgpu::RenderPipeline,
    eyes: [Option<Eye>; 2],
}
impl GlXrOutput {
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("WebGL XR source layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("WebGL XR encoded presentation"),
            source: wgpu::ShaderSource::Wgsl(include_str!("xr_gl_present.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("WebGL XR presentation"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("WebGL XR borrowed framebuffer"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            layout,
            sampler,
            pipeline,
            eyes: [None, None],
        }
    }
    pub fn reset_frame(&mut self) {
        for eye in self.eyes.iter_mut().flatten() {
            eye.ready = false;
        }
    }
    pub fn ensure_eye(
        &mut self,
        device: &wgpu::Device,
        index: usize,
        width: u32,
        height: u32,
    ) -> Result<(), JsValue> {
        if index >= 2
            || width == 0
            || height == 0
            || width > device.limits().max_texture_dimension_2d
            || height > device.limits().max_texture_dimension_2d
        {
            return Err(JsValue::from_str("Invalid WebGL XR eye dimensions"));
        }
        let slot = &mut self.eyes[index];
        if slot.as_ref().is_some_and(|e| e.ready) {
            return Err(JsValue::from_str("Duplicate WebGL XR eye"));
        }
        if slot
            .as_ref()
            .is_none_or(|e| (e.target.width, e.target.height) != (width, height))
        {
            let target = ColorTarget::new(
                device,
                "WebGL XR owned eye",
                width,
                height,
                wgpu::TextureFormat::Rgba8UnormSrgb,
            );
            let source = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("WebGL XR source"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&target.color),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            *slot = Some(Eye {
                target,
                source,
                ready: false,
            });
        }
        Ok(())
    }
    pub fn target(&self, index: usize) -> RenderTarget<'_> {
        self.eyes[index]
            .as_ref()
            .expect("validated eye")
            .target
            .target()
    }
    pub fn mark_ready(&mut self, index: usize) {
        self.eyes[index].as_mut().expect("validated eye").ready = true;
    }
    pub fn present(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        framebuffer: JsValue,
        width: u32,
        height: u32,
        viewports: &[Viewport],
    ) -> Result<usize, JsValue> {
        if viewports.is_empty()
            || viewports.len() > 2
            || width > device.limits().max_texture_dimension_2d
            || height > device.limits().max_texture_dimension_2d
        {
            return Err(JsValue::from_str("Invalid WebGL XR framebuffer dimensions"));
        }
        for (index, v) in viewports.iter().enumerate() {
            v.validate(width, height)
                .map_err(|e| JsValue::from_str(&e))?;
            if self.eyes[index].as_ref().is_none_or(|e| {
                !e.ready || (e.target.width, e.target.height) != (v.width, v.height)
            }) {
                return Err(JsValue::from_str("WebGL XR eye is not ready"));
            }
            if viewports[..index].iter().any(|a| {
                a.x < v.x + v.width
                    && v.x < a.x + a.width
                    && a.y < v.y + v.height
                    && v.y < a.y + a.height
            }) {
                return Err(JsValue::from_str("Overlapping WebGL XR viewports"));
            }
        }
        if self.eyes[viewports.len()..]
            .iter()
            .flatten()
            .any(|e| e.ready)
        {
            return Err(JsValue::from_str("Missing WebGL XR eye viewport"));
        }
        let framebuffer = framebuffer
            .dyn_into::<web_sys::WebGlFramebuffer>()
            .map_err(|_| JsValue::from_str("Invalid WebGL XR framebuffer"))?;
        let gl = context(device)?;
        if gl.is_context_lost() || !gl.is_framebuffer(Some(&framebuffer)) {
            return Err(JsValue::from_str(
                "WebGL XR framebuffer must belong to the renderer context",
            ));
        }
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let desc = wgpu::TextureDescriptor {
            label: Some("borrowed WebGL XR framebuffer"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };
        let raw = wgpu::hal::gles::Texture {
            inner: wgpu::hal::gles::TextureInner::ExternalFramebuffer { inner: framebuffer },
            mip_level_count: 1,
            array_layer_count: 1,
            format,
            format_desc: wgpu::hal::gles::TextureFormatDesc {
                internal: web_sys::WebGl2RenderingContext::RGBA8,
                external: web_sys::WebGl2RenderingContext::RGBA,
                data_type: web_sys::WebGl2RenderingContext::UNSIGNED_BYTE,
            },
            copy_size: wgpu::hal::CopyExtent {
                width,
                height,
                depth: 1,
            },
            drop_guard: None,
        };
        // SAFETY: same device/context verified above; XR host supplies the active
        // frame's single-sampled normalized RGB(A)8 layer (alpha is opaque). Render-only wrapper, no sampling,
        // read/copy or attachment modification. HAL ExternalFramebuffer drop is
        // a no-op; wgpu-core registers its counter via add_raw_texture. Commands
        // are submitted synchronously inside the XR callback; no handle retained.
        let texture = unsafe {
            device.create_texture_from_hal::<wgpu::hal::api::Gles>(
                raw,
                &desc,
                wgpu::TextureUses::COLOR_TARGET,
            )
        };
        let view = texture.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("WebGL XR presentation"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("WebGL XR final stereo"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            for (index, v) in viewports.iter().enumerate() {
                // Pinned GLES set_viewport/set_scissor forward GL coordinates.
                pass.set_viewport(
                    v.x as f32,
                    v.y as f32,
                    v.width as f32,
                    v.height as f32,
                    0.0,
                    1.0,
                );
                pass.set_scissor_rect(v.x, v.y, v.width, v.height);
                pass.set_bind_group(0, &self.eyes[index].as_ref().unwrap().source, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        queue.submit(Some(encoder.finish()));
        self.reset_frame();
        Ok(viewports.len())
    }
}
