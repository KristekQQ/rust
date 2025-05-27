use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{closure::Closure, JsCast};
use wgpu::util::DeviceExt;

mod math;

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 3],
}

impl Vertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex { position: [-0.5, -0.5, 0.0] },
    Vertex { position: [0.5, -0.5, 0.0] },
    Vertex { position: [0.0, 0.5, 0.0] },
];

fn as_bytes<T: Copy>(data: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * std::mem::size_of::<T>()) }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Uniforms {
    mvp: [[f32; 4]; 4],
}

struct Camera {
    pos: [f32; 3],
    yaw: f32,
    pitch: f32,
}

impl Camera {
    fn new() -> Self {
        Self { pos: [0.0, 0.0, 2.0], yaw: -std::f32::consts::FRAC_PI_2, pitch: 0.0 }
    }

    fn view_matrix(&self) -> [[f32; 4]; 4] {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let forward = [cp * cy, sp, cp * sy];
        let target = [
            self.pos[0] + forward[0],
            self.pos[1] + forward[1],
            self.pos[2] + forward[2],
        ];
        math::look_at(self.pos, target, [0.0, 1.0, 0.0])
    }

    fn update(&mut self, input: &InputState, dt: f32) {
        let speed = 2.0 * dt;
        let (sy, cy) = self.yaw.sin_cos();
        let forward = [cy, 0.0, sy];
        let right = [forward[2], 0.0, -forward[0]];
        if input.forward {
            self.pos[0] += forward[0] * speed;
            self.pos[2] += forward[2] * speed;
        }
        if input.back {
            self.pos[0] -= forward[0] * speed;
            self.pos[2] -= forward[2] * speed;
        }
        if input.left {
            self.pos[0] -= right[0] * speed;
            self.pos[2] -= right[2] * speed;
        }
        if input.right {
            self.pos[0] += right[0] * speed;
            self.pos[2] += right[2] * speed;
        }
    }

    fn add_mouse_delta(&mut self, dx: f32, dy: f32) {
        const SENS: f32 = 0.002;
        self.yaw += dx * SENS;
        self.pitch -= dy * SENS;
        self.pitch = self.pitch.clamp(-1.55, 1.55);
    }
}

#[derive(Default)]
struct InputState {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
}

struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    aspect: f32,
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
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });
        let uniform = Uniforms { mvp: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]] };
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

        Ok(Self { surface, device, queue, pipeline, vertex_buffer, uniform_buffer, bind_group, aspect })
    }

    fn update(&mut self, angle: f32, camera: &Camera) {
        use crate::math::{mat4_mul, perspective, rotation_y};
        let model = rotation_y(angle);
        let view = camera.view_matrix();
        let proj = perspective(self.aspect, std::f32::consts::FRAC_PI_4, 0.1, 10.0);
        let m = mat4_mul(proj, mat4_mul(view, model));
        let uniform = Uniforms { mvp: m };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, as_bytes(&[uniform]));
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("encoder"),
        });
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
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
            rp.draw(0..3, 0..1);
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
    let camera = Rc::new(RefCell::new(Camera::new()));
    let input = Rc::new(RefCell::new(InputState::default()));
    let performance = window.performance().unwrap();
    let start_time = performance.now();
    let last_time = Rc::new(RefCell::new(start_time));

    {
        let inp = input.clone();
        let keydown = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
            match e.key().as_str() {
                "w" | "W" => inp.borrow_mut().forward = true,
                "s" | "S" => inp.borrow_mut().back = true,
                "a" | "A" => inp.borrow_mut().left = true,
                "d" | "D" => inp.borrow_mut().right = true,
                _ => {}
            }
        }) as Box<dyn FnMut(_)>);
        window.add_event_listener_with_callback("keydown", keydown.as_ref().unchecked_ref())?;
        keydown.forget();
    }
    {
        let inp = input.clone();
        let keyup = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
            match e.key().as_str() {
                "w" | "W" => inp.borrow_mut().forward = false,
                "s" | "S" => inp.borrow_mut().back = false,
                "a" | "A" => inp.borrow_mut().left = false,
                "d" | "D" => inp.borrow_mut().right = false,
                _ => {}
            }
        }) as Box<dyn FnMut(_)>);
        window.add_event_listener_with_callback("keyup", keyup.as_ref().unchecked_ref())?;
        keyup.forget();
    }
    {
        let cam = camera.clone();
        let mouse = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
            cam.borrow_mut().add_mouse_delta(e.movement_x() as f32, e.movement_y() as f32);
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("mousemove", mouse.as_ref().unchecked_ref())?;
        mouse.forget();
    }
    {
        let c = canvas.clone();
        let click = Closure::wrap(Box::new(move |_e: web_sys::MouseEvent| {
            c.request_pointer_lock();
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("click", click.as_ref().unchecked_ref())?;
        click.forget();
    }
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    let window_c = window.clone();
    let perf_c = performance.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let now = perf_c.now();
        let elapsed = (now - start_time) as f32 / 1000.0;
        let dt = (now - *last_time.borrow()) as f32 / 1000.0;
        *last_time.borrow_mut() = now;
        camera.borrow_mut().update(&input.borrow(), dt);
        let angle = elapsed / 5.0 * (2.0 * std::f32::consts::PI);
        {
            let mut st = state.borrow_mut();
            st.update(angle, &camera.borrow());
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
}

