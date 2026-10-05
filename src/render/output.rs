//! Browser presentation adapter. Only this module knows about canvas/surfaces.
use super::{
    depth,
    target::{RenderTarget, Viewport},
    textures::ColorTarget,
};
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub struct CanvasFrame {
    frame: wgpu::SurfaceTexture,
    view: wgpu::TextureView,
    reconfigure: bool,
}
struct TextureOutput {
    target: ColorTarget,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
}
pub struct CanvasOutput {
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    canvas: HtmlCanvasElement,
    config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    pub offscreen: bool,
    texture_output: Option<TextureOutput>,
}
impl CanvasOutput {
    pub async fn new(
        canvas: &HtmlCanvasElement,
    ) -> Result<(Self, wgpu::Device, wgpu::Queue, String), JsValue> {
        let instance_desc = wgpu::InstanceDescriptor {
            backends: if canvas.get_attribute("data-backend").as_deref() == Some("webgl") {
                wgpu::Backends::GL
            } else {
                wgpu::Backends::all()
            },
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        };
        let instance = wgpu::util::new_instance_with_webgpu_detection(instance_desc).await;
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| JsValue::from_str(&format!("{e:?}")))?;
        let adapter = wgpu::util::initialize_adapter_from_env_or_default(&instance, Some(&surface))
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let backend = format!("{:?}", adapter.get_info().backend);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::default(),
                ..Default::default()
            })
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(caps.formats[0]);
        let view_format = format.add_srgb_suffix();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: canvas.width().max(1),
            height: canvas.height().max(1),
            present_mode: caps.present_modes[0],
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![view_format],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);

        let depth_format = wgpu::TextureFormat::Depth32Float;
        let (depth_texture, depth_view) =
            depth::create(&device, config.width, config.height, depth_format);

        Ok((
            Self {
                instance,
                surface,
                canvas: canvas.clone(),
                config,
                depth_texture,
                depth_view,
                offscreen: false,
                texture_output: None,
            },
            device,
            queue,
            backend,
        ))
    }
    pub fn aspect(&self) -> f32 {
        self.config.width as f32 / self.config.height as f32
    }
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(device, &self.config);
        (self.depth_texture, self.depth_view) = depth::create(device, width, height, DEPTH_FORMAT);
        // Recreate attachments and their bind group together on the next frame.
        self.texture_output = None;
    }
    pub fn acquire(&mut self, device: &wgpu::Device) -> Result<Option<CanvasFrame>, JsValue> {
        let (frame, reconfigure) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(device, &self.config);
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self
                    .instance
                    .create_surface(wgpu::SurfaceTarget::Canvas(self.canvas.clone()))
                    .map_err(|error| JsValue::from_str(&error.to_string()))?;
                self.surface.configure(device, &self.config);
                return Ok(None);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(JsValue::from_str("GPU surface validation failed"));
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.config.format.add_srgb_suffix()),
            ..Default::default()
        });
        Ok(Some(CanvasFrame {
            frame,
            view,
            reconfigure,
        }))
    }
    pub fn ensure_target(&mut self, device: &wgpu::Device) {
        if !self.offscreen || self.texture_output.is_some() {
            return;
        }
        let format = self.config.format.add_srgb_suffix();
        let target = ColorTarget::new(
            device,
            "offscreen scene color",
            self.config.width,
            self.config.height,
            format,
        );
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present texture layout"),
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("present sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("present texture"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&target.color),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("present.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("present layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("present offscreen scene"),
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
                    format,
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
        self.texture_output = Some(TextureOutput {
            target,
            bind_group,
            pipeline,
        });
    }
    pub fn target<'a>(&'a self, frame: &'a CanvasFrame) -> RenderTarget<'a> {
        let (color, depth) = if self.offscreen {
            let output = self
                .texture_output
                .as_ref()
                .expect("ensure_target before target");
            (&output.target.color, &output.target.depth)
        } else {
            (&frame.view, &self.depth_view)
        };
        RenderTarget {
            color,
            depth,
            color_format: self.config.format.add_srgb_suffix(),
            depth_format: DEPTH_FORMAT,
            width: self.config.width,
            height: self.config.height,
            viewport: Viewport::full(self.config.width, self.config.height),
            color_load: wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.1,
                g: 0.1,
                b: 0.3,
                a: 1.0,
            }),
            depth_load: wgpu::LoadOp::Clear(1.0),
        }
    }
    pub fn present(&mut self, frame: CanvasFrame, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.offscreen {
            let output = self
                .texture_output
                .as_ref()
                .expect("ensure_target before present");
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("present encoder"),
            });
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("present texture"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&output.pipeline);
                pass.set_bind_group(0, &output.bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
            queue.submit(Some(encoder.finish()));
        }
        queue.present(frame.frame);
        if frame.reconfigure {
            self.surface.configure(device, &self.config);
        }
    }
}
