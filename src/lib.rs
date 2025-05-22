#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
#[cfg(target_arch = "wasm32")]
pub async fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    // Access the canvas element from the document
    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    let canvas = document
        .get_element_by_id("gpu-canvas")
        .expect("Canvas with id 'gpu-canvas' not found")
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    // Initialize wgpu
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
        .ok_or("failed to find an appropriate adapter")?;

    // Po odstranění starého limitu `maxInterStageShaderComponents` musí být
    // vytvoření zařízení explicitní, abychom nepožadovali neznámé limity.
    let (device, queue) = {
        let desc = wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
        };
        adapter
            .request_device(&desc, None)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?
    };

    let caps = surface.get_capabilities(&adapter);
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: caps.formats[0],
        width: canvas.width(),
        height: canvas.height(),
        desired_maximum_frame_latency: 2,
        present_mode: caps.present_modes[0],
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
    };
    surface.configure(&device, &config);

    let frame = surface
        .get_current_texture()
        .expect("Failed to acquire next surface texture");
    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("render encoder"),
    });

    {
        let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
    }

    queue.submit(std::iter::once(encoder.finish()));
    frame.present();

    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn start() -> Result<(), ()> {
    // No-op when not targeting WebAssembly
    Ok(())
}

