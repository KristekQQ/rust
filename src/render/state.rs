//! Browser host connecting the scene renderer to a canvas output.
use super::{
    output::CanvasOutput, postprocess::PostProcessor, renderer::SceneRenderer, target::RenderView,
};
use glam::{Mat4, Vec3};
use std::ops::{Deref, DerefMut};
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

pub struct State {
    renderer: SceneRenderer,
    output: CanvasOutput,
    postprocess: PostProcessor,
    pub backend: String,
    pub aspect: f32,
    view: RenderView,
    model_camera: Option<(u32, usize)>,
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
        let renderer = SceneRenderer::new(device, queue);
        let mut postprocess = PostProcessor::new(&renderer.device, output.color_format());
        let (static_map, dynamic_map) = renderer.shadow_maps();
        postprocess.set_shadow_maps(&renderer.device, static_map, dynamic_map);
        Ok(Self {
            renderer,
            output,
            postprocess,
            backend,
            aspect,
            model_camera: None,
            view: RenderView {
                view_projection: Mat4::IDENTITY,
                camera_position: Vec3::ZERO,
            },
        })
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        self.output.resize(&self.renderer.device, width, height);
        self.aspect = self.output.aspect();
    }
    pub fn configure_god_rays(&mut self, density: f32, exposure: f32) -> bool {
        self.postprocess.configure_volume(density, exposure)
    }
    pub fn set_postprocess(&mut self, name: &str, enabled: bool, strength: f32) -> bool {
        self.postprocess.set_effect(name, enabled, strength)
    }
    pub fn set_reflection_mask(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u8],
        center: glam::Vec2,
        size: glam::Vec2,
    ) -> bool {
        self.postprocess.set_reflection_mask(
            &self.renderer.device,
            &self.renderer.queue,
            width,
            height,
            pixels,
            center,
            size,
        )
    }
    pub fn postprocess_stats(&self) -> [f64; 5] {
        self.postprocess.stats()
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
        self.view = RenderView {
            view_projection: camera_matrix,
            camera_position: camera_pos,
        };
        if let Some((id, node)) = self.model_camera {
            if let Some(view) = self.imported_camera_view(id, node) {
                self.view = view;
            } else {
                self.model_camera = None;
            }
        }
    }
    pub fn clear_model_camera(&mut self) {
        self.model_camera = None;
    }
    pub fn use_model_camera(&mut self, id: u32, node: usize) -> bool {
        if self.imported_camera_view(id, node).is_none() {
            return false;
        }
        self.model_camera = Some((id, node));
        true
    }
    fn imported_camera_view(&self, id: u32, node: usize) -> Option<RenderView> {
        let model = self.scene.model(id)?;
        if !model.pose.active_nodes.contains(&node) {
            return None;
        }
        let camera = model.asset.nodes.get(node)?.camera?;
        let root = self.scene.objects().get(&id)?.transform.model();
        let world = root * model.pose.world_matrices[node];
        if world.determinant().abs() < 1e-10 {
            return None;
        }
        let projection = match &model.asset.cameras[camera] {
            crate::gltf_model::ModelCamera::Perspective {
                yfov,
                aspect_ratio,
                znear,
                zfar,
                ..
            } => match zfar {
                Some(far) => glam::camera::rh::proj::directx::perspective(
                    *yfov,
                    aspect_ratio.unwrap_or(self.aspect),
                    *znear,
                    *far,
                ),
                None => glam::camera::rh::proj::directx::perspective_infinite(
                    *yfov,
                    aspect_ratio.unwrap_or(self.aspect),
                    *znear,
                ),
            },
            crate::gltf_model::ModelCamera::Orthographic {
                xmag,
                ymag,
                znear,
                zfar,
                ..
            } => glam::camera::rh::proj::directx::orthographic(
                -*xmag, *xmag, -*ymag, *ymag, *znear, *zfar,
            ),
        };
        Some(RenderView {
            view_projection: projection * world.inverse(),
            camera_position: world.transform_point3(Vec3::ZERO),
        })
    }
    pub fn render(&mut self) -> Result<(), JsValue> {
        let Some(frame) = self.output.acquire(&self.renderer.device)? else {
            return Ok(());
        };
        self.output.ensure_target(&self.renderer.device);
        let target = self.output.target(&frame);
        if self.postprocess.enabled() {
            self.postprocess
                .ensure_size(&self.renderer.device, target.width, target.height);
            self.renderer
                .render_view(self.view, &self.postprocess.target())
                .map_err(|e| JsValue::from_str(&e))?;
            // Effects lazily replaces fallback maps when shadows are first enabled.
            // Rebind by resource identity; unchanged maps do not allocate a new bind group.
            let (static_map, dynamic_map) = self.renderer.shadow_maps();
            self.postprocess
                .set_shadow_maps(&self.renderer.device, static_map, dynamic_map);
            let light = self.renderer.scene.lights().values().next();
            let (position, color) = light
                .map(|l| (l.position, l.color))
                .unwrap_or((Vec3::ZERO, Vec3::ZERO));
            self.postprocess.render(
                &self.renderer.device,
                &self.renderer.queue,
                self.view,
                position,
                color,
                target.color,
                self.renderer.shadows_enabled(),
            );
            let overlays = super::target::RenderTarget {
                depth: self.postprocess.target().depth,
                auxiliary: None,
                ..target
            };
            self.renderer
                .draw_transparent(&overlays)
                .map_err(|e| JsValue::from_str(&e))?;
            self.renderer.draw_helpers(&overlays);
        } else {
            self.renderer
                .render_view(self.view, &target)
                .map_err(|e| JsValue::from_str(&e))?;
        }
        self.output
            .present(frame, &self.renderer.device, &self.renderer.queue);
        Ok(())
    }
}
