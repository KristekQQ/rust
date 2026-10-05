//! Browser host connecting the scene renderer to a canvas output.
use super::{output::CanvasOutput, renderer::SceneRenderer, target::RenderView};
use glam::{Mat4, Vec3};
use std::ops::{Deref, DerefMut};
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

pub struct State {
    renderer: SceneRenderer,
    output: CanvasOutput,
    pub backend: String,
    pub aspect: f32,
}
impl Deref for State {
    type Target = SceneRenderer;
    fn deref(&self) -> &Self::Target {
        &self.renderer
    }
}
impl DerefMut for State {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.renderer
    }
}
impl State {
    pub async fn new(canvas: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let (output, device, queue, backend) = CanvasOutput::new(canvas).await?;
        let aspect = output.aspect();
        Ok(Self {
            renderer: SceneRenderer::new(device, queue),
            output,
            backend,
            aspect,
        })
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        self.output.resize(&self.renderer.device, width, height);
        self.aspect = self.output.aspect();
    }
    pub fn set_output_mode(&mut self, mode: &str) -> bool {
        match mode {
            "canvas" => self.output.offscreen = false,
            "texture" => self.output.offscreen = true,
            _ => return false,
        }
        true
    }
    pub fn update(&mut self, dt: f32, camera_matrix: Mat4, camera_pos: Vec3) {
        self.renderer.advance_scene(dt);
        self.renderer.prepare_view(RenderView {
            view_projection: camera_matrix,
            camera_position: camera_pos,
        });
    }
    pub fn render(&mut self) -> Result<(), JsValue> {
        let Some(frame) = self.output.acquire(&self.renderer.device)? else {
            return Ok(());
        };
        self.output.ensure_target(&self.renderer.device);
        let target = self.output.target(&frame);
        self.renderer
            .draw(&target)
            .map_err(|e| JsValue::from_str(&e))?;
        self.output
            .present(frame, &self.renderer.device, &self.renderer.queue);
        Ok(())
    }
}
