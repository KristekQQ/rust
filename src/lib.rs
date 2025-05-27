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

#[cfg(target_arch = "wasm32")]
struct Input {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    mouse_down: bool,
    last_x: f32,
    last_y: f32,
    dx: f32,
    dy: f32,
}

#[cfg(target_arch = "wasm32")]
impl Input {
    fn new() -> Self {
        Self {
            forward: false,
            back: false,
            left: false,
            right: false,
            mouse_down: false,
            last_x: 0.0,
            last_y: 0.0,
            dx: 0.0,
            dy: 0.0,
        }
    }

    fn consume_mouse_delta(&mut self) -> (f32, f32) {
        let d = (self.dx, self.dy);
        self.dx = 0.0;
        self.dy = 0.0;
        d
    }
}

#[cfg(target_arch = "wasm32")]
struct Camera {
    position: [f32; 3],
    yaw: f32,
    pitch: f32,
}

#[cfg(target_arch = "wasm32")]
impl Camera {
    fn new() -> Self {
        Self {
            position: [0.0, 0.0, 2.0],
            yaw: 0.0,
            pitch: 0.0,
        }
    }

    fn update(&mut self, input: &mut Input, dt: f32) {
        let speed = 2.0 * dt;
        let (dx, dy) = input.consume_mouse_delta();
        if input.mouse_down {
            self.yaw += dx * 0.005;
            self.pitch += dy * 0.005;
        }

        let (cy, sy) = (self.yaw.cos(), self.yaw.sin());
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        let forward = [cy * cp, sp, sy * cp];
        let right = [cy, 0.0, sy * -1.0];

        if input.forward {
            for i in 0..3 {
                self.position[i] += forward[i] * speed;
            }
        }
        if input.back {
            for i in 0..3 {
                self.position[i] -= forward[i] * speed;
            }
        }
        if input.right {
            for i in 0..3 {
                self.position[i] += right[i] * speed;
            }
        }
        if input.left {
            for i in 0..3 {
                self.position[i] -= right[i] * speed;
            }
        }
    }

    fn view(&self) -> [[f32; 4]; 4] {
        use crate::math::look_at;
        let (cy, sy) = (self.yaw.cos(), self.yaw.sin());
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        let forward = [cy * cp, sp, sy * cp];
        let target = [
            self.position[0] + forward[0],
            self.position[1] + forward[1],
            self.position[2] + forward[2],
        ];
        look_at(self.position, target, [0.0, 1.0, 0.0])
    }
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

#[cfg(target_arch = "wasm32")]
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
    input: Input,
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
            camera: Camera::new(),
            input: Input::new(),
        })
    }

    fn update(&mut self, angle: f32, dt: f32) {
        use crate::math::{mat4_mul, perspective_lh, rotation_z, transpose};
        self.camera.update(&mut self.input, dt);
        let model = rotation_z(angle);
        let view = self.camera.view();
        web_sys::console::log_1(&format!("{:?}", view).into());
        let proj = perspective_lh(self.aspect, std::f32::consts::FRAC_PI_4, 0.1, 10.0);
        let m = transpose(mat4_mul(proj, mat4_mul(view, model)));
        let uniform = Uniforms { mvp: m };
        self.queue.write_buffer(&self.uniform_buffer, 0, as_bytes(&[uniform]));
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
    let performance = window.performance().unwrap();
    let start_time = performance.now();
    let last_time = Rc::new(RefCell::new(start_time));

    {
        let st = state.clone();
        let cb = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
            let mut s = st.borrow_mut();
            match e.key().as_str() {
                "w" | "W" => s.input.forward = true,
                "s" | "S" => s.input.back = true,
                "a" | "A" => s.input.left = true,
                "d" | "D" => s.input.right = true,
                _ => {}
            }
        }) as Box<dyn FnMut(_)>);
        window.add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }
    {
        let st = state.clone();
        let cb = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
            let mut s = st.borrow_mut();
            match e.key().as_str() {
                "w" | "W" => s.input.forward = false,
                "s" | "S" => s.input.back = false,
                "a" | "A" => s.input.left = false,
                "d" | "D" => s.input.right = false,
                _ => {}
            }
        }) as Box<dyn FnMut(_)>);
        window.add_event_listener_with_callback("keyup", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }
    {
        let st = state.clone();
        let cb = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
            let mut s = st.borrow_mut();
            s.input.mouse_down = true;
            s.input.last_x = e.client_x() as f32;
            s.input.last_y = e.client_y() as f32;
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("mousedown", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }
    {
        let st = state.clone();
        let cb = Closure::wrap(Box::new(move |_e: web_sys::MouseEvent| {
            let mut s = st.borrow_mut();
            s.input.mouse_down = false;
        }) as Box<dyn FnMut(_)>);
        window.add_event_listener_with_callback("mouseup", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }
    {
        let st = state.clone();
        let cb = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
            let mut s = st.borrow_mut();
            let x = e.client_x() as f32;
            let y = e.client_y() as f32;
            s.input.dx += x - s.input.last_x;
            s.input.dy += y - s.input.last_y;
            s.input.last_x = x;
            s.input.last_y = y;
        }) as Box<dyn FnMut(_)>);
        window.add_event_listener_with_callback("mousemove", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    let window_c = window.clone();
    let perf_c = performance.clone();
    let last_c = last_time.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let now = perf_c.now();
        let elapsed = (now - start_time) as f32 / 1000.0;
        let dt = (now - *last_c.borrow()) as f32 / 1000.0;
        *last_c.borrow_mut() = now;
        let angle = elapsed / 5.0 * (2.0 * std::f32::consts::PI);
        {
            let mut st = state.borrow_mut();
            st.update(angle, dt);
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
