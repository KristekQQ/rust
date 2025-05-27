#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{closure::Closure, JsCast};
#[cfg(target_arch = "wasm32")]
use wgpu::util::DeviceExt;

mod math;

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex { position: [-0.5, -0.5, 0.0], color: [0.0, 1.0, 0.0] },
    Vertex { position: [0.5, -0.5, 0.0], color: [0.0, 1.0, 0.0] },
    Vertex { position: [0.0, 0.5, 0.0], color: [0.0, 1.0, 0.0] },
];

const INDICES: &[u16] = &[0, 1, 2];

#[cfg(target_arch = "wasm32")]
fn as_bytes<T: Copy>(data: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            data.as_ptr() as *const u8,
            data.len() * std::mem::size_of::<T>(),
        )
    }
}

#[cfg(target_arch = "wasm32")]
#[repr(C)]
#[derive(Clone, Copy)]
struct Uniforms {
    mvp: [[f32; 4]; 4],
}

#[derive(Default)]
struct Input {
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    mouse_down: bool,
    last_x: f32,
    last_y: f32,
    dyaw: f32,
    dpitch: f32,
}

struct Camera {
    yaw: f32,
    pitch: f32,
    distance: f32,
}

impl Camera {
    fn view(&self) -> [[f32; 4]; 4] {
        use crate::math::look_at;
        let cy = self.yaw.cos();
        let sy = self.yaw.sin();
        let cp = self.pitch.cos();
        let sp = self.pitch.sin();
        let eye = [self.distance * cy * cp, self.distance * sp, self.distance * sy * cp];
        look_at(eye, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0])
    }
}

struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    aspect: f32,
    camera: Camera,
}

#[cfg(target_arch = "wasm32")]
impl State {
    async fn new(canvas: &web_sys::HtmlCanvasElement) -> Result<Self, JsValue> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or("failed to find adapter")?;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: adapter.limits(),
                },
                None,
            )
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let caps = surface.get_capabilities(&adapter);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: caps.formats[0],
            width: canvas.width(),
            height: canvas.height(),
            present_mode: caps.present_modes[0],
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let aspect = config.width as f32 / config.height as f32;

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex buffer"),
            contents: as_bytes(VERTICES),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index buffer"),
            contents: as_bytes(INDICES),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bind group layout"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                front_face: wgpu::FrontFace::Cw,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });
        let uniform = Uniforms {
            mvp: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform buffer"),
            contents: as_bytes(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("bind group"),
        });
        let camera = Camera {
            yaw: 0.0,
            pitch: 0.0,
            distance: 3.0,
        };

        Ok(Self {
            surface,
            device,
            queue,
            pipeline,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            bind_group,
            aspect,
            camera,
        })
    }

    fn update(&mut self, angle: f32) {
        use crate::math::{mat4_mul, perspective_lh, rotation_z, transpose};
        let model = rotation_z(angle);
        let view = self.camera.view();
        let proj = perspective_lh(self.aspect, std::f32::consts::FRAC_PI_4, 0.1, 10.0);
        let m = transpose(mat4_mul(proj, mat4_mul(view, model)));
        let uniform = Uniforms { mvp: m };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, as_bytes(&[uniform]));
        web_sys::console::log_1(&format!("view: {:?}", view).into());
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.1, b: 0.3, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            rp.set_pipeline(&self.pipeline);
            rp.set_bind_group(0, &self.bind_group, &[]);
            rp.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            rp.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            rp.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
#[cfg(target_arch = "wasm32")]
pub async fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"Test 321".into());
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document
        .get_element_by_id("gpu-canvas")
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let state = Rc::new(RefCell::new(State::new(&canvas).await?));
    let input = Rc::new(RefCell::new(Input::default()));
    {
        let inp = input.clone();
        let keydown = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
            let mut i = inp.borrow_mut();
            match e.key_code() {
                37 => i.left = true,
                38 => i.up = true,
                39 => i.right = true,
                40 => i.down = true,
                _ => {}
            }
        }) as Box<dyn FnMut(_)>);
        window.add_event_listener_with_callback("keydown", keydown.as_ref().unchecked_ref())?;
        keydown.forget();
    }
    {
        let inp = input.clone();
        let keyup = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
            let mut i = inp.borrow_mut();
            match e.key_code() {
                37 => i.left = false,
                38 => i.up = false,
                39 => i.right = false,
                40 => i.down = false,
                _ => {}
            }
        }) as Box<dyn FnMut(_)>);
        window.add_event_listener_with_callback("keyup", keyup.as_ref().unchecked_ref())?;
        keyup.forget();
    }
    {
        let inp = input.clone();
        let md = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
            let mut i = inp.borrow_mut();
            i.mouse_down = true;
            i.last_x = e.client_x() as f32;
            i.last_y = e.client_y() as f32;
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("mousedown", md.as_ref().unchecked_ref())?;
        md.forget();
    }
    {
        let inp = input.clone();
        let mu = Closure::wrap(Box::new(move |_e: web_sys::MouseEvent| {
            inp.borrow_mut().mouse_down = false;
        }) as Box<dyn FnMut(_)>);
        window.add_event_listener_with_callback("mouseup", mu.as_ref().unchecked_ref())?;
        mu.forget();
    }
    {
        let inp = input.clone();
        let mm = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
            let mut i = inp.borrow_mut();
            if i.mouse_down {
                let x = e.client_x() as f32;
                let y = e.client_y() as f32;
                i.dyaw += (x - i.last_x) * 0.005;
                i.dpitch += (y - i.last_y) * 0.005;
                i.last_x = x;
                i.last_y = y;
            }
        }) as Box<dyn FnMut(_)>);
        window.add_event_listener_with_callback("mousemove", mm.as_ref().unchecked_ref())?;
        mm.forget();
    }

    let performance = window.performance().unwrap();
    let start_time = performance.now();
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    let window_c = window.clone();
    let perf_c = performance.clone();
    let state_c = state.clone();
    let input_c = input.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let elapsed = (perf_c.now() - start_time) as f32 / 1000.0;
        let angle = elapsed / 5.0 * (2.0 * std::f32::consts::PI);
        {
            let mut st = state_c.borrow_mut();
            let mut inp = input_c.borrow_mut();
            if inp.left {
                st.camera.yaw -= 0.02;
            }
            if inp.right {
                st.camera.yaw += 0.02;
            }
            if inp.up {
                st.camera.pitch += 0.02;
            }
            if inp.down {
                st.camera.pitch -= 0.02;
            }
            st.camera.yaw += inp.dyaw;
            st.camera.pitch -= inp.dpitch;
            inp.dyaw = 0.0;
            inp.dpitch = 0.0;
            st.update(angle);
            if st.render().is_err() {
                return;
            }
        }
        window_c
            .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
            .unwrap();
    }) as Box<dyn FnMut()>));

    window.request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref())?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn start() -> Result<(), ()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_vertex_count() {
        assert_eq!(VERTICES.len(), 3);
    }

    #[test]
    fn triangle_index_count() {
        assert_eq!(INDICES.len(), 3);
    }
}
