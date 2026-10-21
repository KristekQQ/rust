//! Browser host connecting the scene renderer to a canvas output.
use super::{
    output::CanvasOutput, postprocess::PostProcessor, renderer::SceneRenderer, target::RenderView,
};
use glam::{Mat4, Vec3};
use std::ops::{Deref, DerefMut};
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

struct ComboIndicator {
    id: u32,
    kind: String,
    slot: u32,
    scale: Vec3,
}

pub struct State {
    renderer: SceneRenderer,
    output: CanvasOutput,
    postprocess: PostProcessor,
    xr_depth_resolve: Option<super::depth_resolve::DepthResolve>,
    xr_present: Option<super::xr_present::XrPresent>,
    environment: Option<super::environment::EnvironmentProbe>,
    background: wgpu::Color,
    pub backend: String,
    pub aspect: f32,
    view: RenderView,
    model_camera: Option<(u32, usize)>,
    debug_enabled: bool,
    debug_hud: Option<super::hud::DebugHud>,
    frame_dt: f32,
    xr_rendering: bool,
    xr_hud_pending: bool,
    xr_scene_pending: bool,
    xr_view_slot: u32,
    xr_eye_count: u32,
    xr_cuts: u64,
    pub combo_score: crate::xr_round::ComboScore,
    combo_indicators: Vec<ComboIndicator>,
    combo_display: Option<crate::combo_display::ComboDisplay>,
    info_panel_elapsed: f32,
    info_round_dirty: bool,
    xr_round: Option<crate::xr_round::RoundClock>,
    cutting_compute: Option<super::cutting_compute::ComputeCutter>,
    resident_cutter: Option<super::cutting_resident::ResidentCutter>,
    resident_sources:
        std::collections::BTreeMap<u32, std::rc::Rc<super::cutting_resident::ResidentSource>>,
    pub cutting_busy: std::rc::Rc<std::cell::Cell<bool>>,
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
    pub async fn new(
        canvas: &HtmlCanvasElement,
        check_current: impl Fn() -> Result<(), JsValue>,
    ) -> Result<Self, JsValue> {
        let (output, device, queue, backend) = CanvasOutput::new(canvas, &check_current).await?;
        check_current()?;
        let aspect = output.aspect();
        super::profiler::reset();
        if canvas.get_attribute("data-frame-profile").as_deref() == Some("true") {
            super::profiler::initialize(&device);
        }
        let renderer = SceneRenderer::new(device, queue);
        let mut postprocess = PostProcessor::new(&renderer.device, output.color_format());
        let (static_map, dynamic_map) = renderer.shadow_maps();
        postprocess.set_shadow_maps(&renderer.device, static_map, dynamic_map);
        Ok(Self {
            renderer,
            output,
            postprocess,
            xr_depth_resolve: None,
            xr_present: None,
            environment: None,
            background: wgpu::Color {
                r: 0.1,
                g: 0.1,
                b: 0.3,
                a: 1.0,
            },
            backend,
            aspect,
            model_camera: None,
            debug_enabled: false,
            debug_hud: None,
            frame_dt: 0.0,
            xr_rendering: false,
            xr_hud_pending: false,
            xr_scene_pending: false,
            xr_view_slot: 0,
            xr_eye_count: 0,
            xr_cuts: 0,
            combo_score: Default::default(),
            combo_indicators: Vec::new(),
            combo_display: None,
            info_panel_elapsed: 0.25,
            info_round_dirty: true,
            xr_round: None,
            cutting_compute: None,
            resident_cutter: None,
            resident_sources: Default::default(),
            cutting_busy: Default::default(),
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
    pub fn dispatch_cut_geometry(
        &mut self,
        inputs: &[crate::cutting::CutGeometryInput],
    ) -> super::cutting_compute::PendingCutGeometry {
        let renderer = &self.renderer;
        self.cutting_compute
            .get_or_insert_with(|| super::cutting_compute::ComputeCutter::new(&renderer.device))
            .dispatch(&renderer.device, &renderer.queue, inputs)
    }
    pub fn resident_devices(&self) -> (wgpu::Device, wgpu::Queue) {
        (self.renderer.device.clone(), self.renderer.queue.clone())
    }
    pub fn dispatch_resident(
        &mut self,
        candidate: &crate::scene::ResidentCandidate,
    ) -> Result<super::cutting_resident::PendingResidentCut, String> {
        use super::cutting_resident::*;
        use wgpu::util::DeviceExt;
        let renderer = &self.renderer;
        if let std::collections::btree_map::Entry::Vacant(entry) =
            self.resident_sources.entry(candidate.id)
        {
            let mesh = candidate.mesh.cpu()?;
            if !mesh.resident_eligible() {
                return Err("Resident cutting requires one connected genus-zero solid".into());
            }
            let vertices: Vec<_> = mesh
                .render_mesh()
                .vertices
                .iter()
                .map(|v| ResidentVertex {
                    position: [v.position[0], v.position[1], v.position[2], 0.],
                    normal: [v.normal[0], v.normal[1], v.normal[2], 0.],
                    color: [v.color[0], v.color[1], v.color[2], 0.],
                })
                .collect();
            let buffer = renderer
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("resident original soup"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                });
            entry.insert(std::rc::Rc::new(ResidentSource {
                vertices: buffer,
                vertex_count: vertices.len() as u32,
            }));
        }
        let params = ResidentCutParams {
            a: candidate.a,
            b: candidate.b,
            motion: candidate.motion,
            cap_color: candidate.color,
            source_volume: candidate.mesh.volume() as f32,
            support_dirs: candidate.support_dirs(),
        };
        self.resident_cutter
            .get_or_insert_with(|| ResidentCutter::new(&renderer.device))
            .dispatch(
                &renderer.device,
                &renderer.queue,
                &self.resident_sources[&candidate.id],
                &params,
            )
    }
    pub fn install_resident(&mut self, id: u32, piece: super::cutting_resident::ResidentPiece) {
        let source = self.renderer.install_resident(id, piece);
        self.resident_sources.insert(id, std::rc::Rc::new(source));
    }
    pub fn resident_source(
        &self,
        id: u32,
    ) -> Option<std::rc::Rc<super::cutting_resident::ResidentSource>> {
        self.resident_sources.get(&id).cloned()
    }
    pub fn configure_god_rays(&mut self, density: f32, exposure: f32) -> bool {
        self.postprocess.configure_volume(density, exposure)
    }
    pub fn set_background(&mut self, color: Vec3) -> bool {
        if !color.is_finite() || color.min_element() < 0.0 || color.max_element() > 1.0 {
            return false;
        }
        self.background = wgpu::Color {
            r: color.x as f64,
            g: color.y as f64,
            b: color.z as f64,
            a: 1.0,
        };
        true
    }
    pub fn set_environment_cube_map(
        &mut self,
        size: u32,
        color: &[u8],
        emission: &[u8],
        intensity: f32,
        projection: &[f32],
    ) -> bool {
        self.postprocess.set_environment_cube_map(
            &self.renderer.device,
            &self.renderer.queue,
            size,
            color,
            emission,
            intensity,
            projection,
        )
    }
    pub fn clear_environment_cube_map(&mut self) {
        self.postprocess.clear_environment_cube_map();
    }
    pub fn set_environment_intensity(&mut self, intensity: f32) -> bool {
        self.postprocess.set_environment_intensity(intensity)
    }
    pub fn configure_environment(
        &mut self,
        enabled: bool,
        center: Vec3,
        half_extents: Vec3,
    ) -> bool {
        if !center.is_finite() || !half_extents.is_finite() || half_extents.min_element() <= 0.1 {
            return false;
        }
        self.environment = enabled.then(|| {
            super::environment::EnvironmentProbe::new(&self.renderer.device, center, half_extents)
        });
        self.postprocess.set_environment(
            self.environment.as_ref().map(|p| p.view.clone()),
            center,
            half_extents,
        );
        true
    }
    pub fn environment_captures(&self) -> u32 {
        self.environment.as_ref().map_or(0, |p| p.captures)
    }
    pub fn set_postprocess(&mut self, name: &str, enabled: bool, strength: f32) -> bool {
        self.postprocess.set_effect(name, enabled, strength)
    }
    pub fn configure_ssr(&mut self, resolution_scale: f32) -> bool {
        self.postprocess.configure_ssr(resolution_scale)
    }
    pub fn ssr_stats(&self) -> [f64; 3] {
        self.postprocess.ssr_stats()
    }
    pub fn configure_ssr_region(&mut self, min: Vec3, max: Vec3, enabled: bool) -> bool {
        self.postprocess.configure_ssr_region(min, max, enabled)
    }

    pub fn ssr_work_stats(&self) -> [f64; 2] {
        self.postprocess.ssr_work_stats()
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
    pub fn postprocess_graph(&self) -> serde_json::Value {
        self.postprocess.graph_stats()
    }
    pub fn graph_probe_handles(&self) -> (wgpu::Device, wgpu::Queue) {
        (self.renderer.device.clone(), self.renderer.queue.clone())
    }
    pub fn finish_xr_frame(&self) {
        super::profiler::end_frame(&self.renderer.device, &self.renderer.queue);
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
        super::profiler::begin_frame();
        self.frame_dt = dt;
        if let Some(display) = &mut self.combo_display {
            display.advance(&mut self.renderer.scene, dt);
        }
        self.renderer.advance_scene(dt);
        self.refresh_info_panels(dt);
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
    pub fn screen_to_plane(&self, u: f32, v: f32, height: f32) -> Option<Vec3> {
        crate::render_math::screen_to_horizontal_plane(self.view.view_projection, u, v, height)
    }
    pub fn world_to_screen(&self, point: Vec3) -> Option<[f32; 2]> {
        let p = self.view.view_projection * point.extend(1.0);
        if !p.is_finite() || p.w <= 0.0 {
            return None;
        }
        let p = p.truncate() / p.w;
        (p.x.abs() <= 1.0 && p.y.abs() <= 1.0 && (0.0..=1.0).contains(&p.z))
            .then_some([(p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5])
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
    pub fn set_debug_overlay(&mut self, enabled: bool) {
        self.debug_enabled = enabled;
        self.sync_info_diagnostics();
        if enabled {
            if let Some(hud) = &mut self.debug_hud {
                hud.invalidate();
            }
        }
    }
    fn sync_info_diagnostics(&mut self) {
        let scene_panel = self.renderer.scene.info_panels.values().any(|p| {
            matches!(
                p.kind,
                crate::info_panel::PanelKind::Scene | crate::info_panel::PanelKind::Diagnostics
            )
        });
        self.renderer
            .set_debug_diagnostics(self.debug_enabled || scene_panel);
    }
    pub fn add_info_panel(&mut self, kind: &str, width: f32, height: f32) -> Result<u32, JsValue> {
        let id = self
            .renderer
            .scene
            .add_info_panel(kind, width, height)
            .map_err(|e| JsValue::from_str(&e))?;
        self.info_panel_elapsed = 0.25;
        self.info_round_dirty = true;
        self.sync_info_diagnostics();
        self.refresh_info_panels(0.0);
        Ok(id)
    }
    fn refresh_info_panels(&mut self, dt: f32) {
        use crate::info_panel::PanelKind;
        self.sync_info_diagnostics();
        if self.renderer.scene.info_panels.is_empty() {
            return;
        }
        if dt.is_finite() && dt > 0. {
            self.info_panel_elapsed += dt;
        }
        if self.info_panel_elapsed >= 0.25 {
            self.info_panel_elapsed = 0.;
            let rows = self
                .renderer
                .xr_debug_content(self.xr_cuts, self.xr_round)
                .scene;
            self.renderer
                .scene
                .update_info_panels(PanelKind::Scene, &rows);
            let diagnostics = self.renderer.info_diagnostic_rows();
            self.renderer
                .scene
                .update_info_panels(PanelKind::Diagnostics, &diagnostics);
        }
        self.renderer.scene.update_info_panels(
            PanelKind::Countdown,
            &[(
                "",
                self.xr_round
                    .map_or(30, |r| r.remaining.ceil() as u32)
                    .to_string(),
            )],
        );
        self.renderer.scene.update_info_panels(
            PanelKind::Score,
            &[("", self.combo_score.score.to_string())],
        );
        if self.info_round_dirty {
            let mut rows = if self.combo_score.enabled {
                vec![
                    ("SKORE", self.combo_score.score.to_string()),
                    (
                        "SERIE",
                        format!(
                            "{} {}/3",
                            if self.combo_score.kind.is_empty() {
                                if self.combo_score.streak > 0 {
                                    "WILD"
                                } else {
                                    "CEKA"
                                }
                            } else {
                                &self.combo_score.kind
                            },
                            self.combo_score.streak
                        ),
                    ),
                ]
            } else {
                vec![("ROZKROJENO", self.xr_cuts.to_string())]
            };
            if let Some(round) = self.xr_round {
                rows.push(("CAS", format!("{} S", round.remaining.ceil() as u32)));
                rows.push((
                    "STAV",
                    if round.finished() { "HOTOVO" } else { "HRA" }.into(),
                ));
            }
            self.renderer
                .scene
                .update_info_panels(PanelKind::Round, &rows);
            self.info_round_dirty = false;
        }
        self.renderer.sync_numeric_font_meshes();
    }
    pub fn debug_stats(&self) -> String {
        let mut value = self.renderer.debug_values();
        value["enabled"] = self.debug_enabled.into();
        value["hudFrames"] = self.debug_hud.as_ref().map_or(0, |h| h.frames).into();
        value["hudRefreshes"] = self.debug_hud.as_ref().map_or(0, |h| h.refreshes).into();
        value["xrCuts"] = self.xr_cuts.into();
        value["xrRoundRemaining"] = self.xr_round.map(|r| r.remaining).into();
        value.to_string()
    }
    pub fn xr_round_finished(&self) -> bool {
        self.xr_round.is_some_and(|round| round.finished())
    }
    pub fn configure_combo_display(&mut self, input: &str) -> bool {
        let Some(mut display) =
            crate::combo_display::ComboDisplay::configure(&self.renderer.scene, input)
        else {
            return false;
        };
        self.clear_combo_display();
        display.reset(&mut self.renderer.scene);
        self.combo_display = Some(display);
        true
    }
    pub fn clear_combo_display(&mut self) {
        if let Some(mut display) = self.combo_display.take() {
            display.reset(&mut self.renderer.scene);
        }
    }
    pub fn combo_display_state(&self) -> String {
        self.combo_display
            .as_ref()
            .map_or_else(|| "null".into(), |d| d.state())
    }
    pub fn clear_combo_indicators(&mut self) {
        self.combo_indicators.clear();
    }
    pub fn bind_combo_indicator(&mut self, id: u32, kind: &str, slot: u32) -> bool {
        if slot > 2
            || kind.is_empty()
            || kind.len() > 16
            || !kind.bytes().all(|c| c.is_ascii_alphanumeric() || c == b' ')
        {
            return false;
        }
        let Some(object) = self.renderer.scene.objects().get(&id) else {
            return false;
        };
        let scale = self
            .combo_indicators
            .iter()
            .find(|item| item.id == id)
            .map_or(object.transform.scale, |item| item.scale);
        self.combo_indicators.retain(|item| item.id != id);
        self.combo_indicators.push(ComboIndicator {
            id,
            kind: kind.into(),
            slot,
            scale,
        });
        self.refresh_combo_indicators();
        true
    }
    fn refresh_combo_indicators(&mut self) {
        let (kind, count) = self.combo_score.display_progress();
        self.combo_indicators
            .retain(|item| self.renderer.scene.object_exists(item.id));
        for item in &self.combo_indicators {
            let active = item.kind == kind && item.slot < count;
            self.renderer
                .scene
                .set_emission(item.id, if active { 8.0 } else { 0.65 });
            self.renderer
                .scene
                .set_object_scale(item.id, item.scale * if active { 1.32 } else { 1.0 });
        }
    }
    pub fn start_xr_round(&mut self, seconds: f32) -> Result<(), JsValue> {
        let round = crate::xr_round::RoundClock::new(seconds)
            .ok_or_else(|| JsValue::from_str("Round duration must be 1..3600 seconds"))?;
        self.xr_round = Some(round);
        self.xr_cuts = 0;
        self.combo_score.reset();
        if let Some(display) = &mut self.combo_display {
            display.reset(&mut self.renderer.scene);
        }
        let live: std::collections::BTreeSet<_> =
            self.renderer.scene.objects().keys().copied().collect();
        self.combo_score.targets.retain(|id, _| live.contains(id));
        self.invalidate_xr_hud();
        Ok(())
    }
    pub fn xr_round_state(&self) -> String {
        self.xr_round
            .map(|round| {
                serde_json::json!({
                    "duration": round.duration, "remaining": round.remaining,
                    "finished": round.finished(), "cuts": self.xr_cuts,
                    "score": self.combo_score.score, "streak": self.combo_score.streak,
                    "fruit": self.combo_score.kind, "triples": self.combo_score.triples,
                })
            })
            .unwrap_or(serde_json::Value::Null)
            .to_string()
    }
    fn invalidate_xr_hud(&mut self) {
        self.refresh_combo_indicators();
        self.info_round_dirty = true;
        if let Some(hud) = &mut self.debug_hud {
            hud.invalidate();
        }
    }
    pub fn record_xr_cuts(&mut self, removed: &[u32]) {
        let count = removed.len();
        if count > 0 {
            // Controller input follows advance_xr; new halves must be visible in
            // this same stereo frame instead of disappearing for one frame.
            self.xr_scene_pending = true;
        }
        if count > 0 && !self.xr_round_finished() {
            self.xr_cuts = self.xr_cuts.saturating_add(count as u64);
            for id in removed {
                if let Some(event) = self.combo_score.hit_event(*id) {
                    if let Some(display) = &mut self.combo_display {
                        display.event(&mut self.renderer.scene, event);
                    }
                }
            }
            self.invalidate_xr_hud();
        }
    }
    pub fn frame_dt(&self) -> f32 {
        self.frame_dt
    }
    pub fn advance_xr(&mut self, dt: f32) {
        self.frame_dt = dt;
        self.xr_hud_pending = true;
        self.xr_eye_count = 0;
        self.xr_view_slot = 0;
        if let Some(round) = &mut self.xr_round {
            let before = round.remaining.ceil();
            round.advance(dt);
            if before != round.remaining.ceil() {
                self.invalidate_xr_hud();
            }
        }
        // FPS and the clock use elapsed time; only physics has a bounded step.
        if let Some(present) = &mut self.xr_present {
            present.reset_frame();
        }
        super::profiler::begin_frame();
        if let Some(display) = &mut self.combo_display {
            display.advance(&mut self.renderer.scene, dt);
        }
        self.renderer.advance_xr_scene(dt);
        self.xr_scene_pending = true;
        self.refresh_info_panels(dt);
    }
    pub fn render_xr_eye(&mut self, matrix: Mat4, position: Vec3) -> Result<(), JsValue> {
        if self.xr_eye_count >= 2 {
            return Err(JsValue::from_str("XR frame supports at most two eyes"));
        }
        self.xr_view_slot = self.xr_eye_count + 1;
        self.xr_eye_count += 1;
        if self.xr_scene_pending {
            self.renderer.finish_scene_updates();
            self.xr_scene_pending = false;
        }
        let saved = self.view;
        let saved_xr = self.xr_rendering;
        self.xr_rendering = true;
        self.view = RenderView {
            view_projection: matrix,
            camera_position: position,
        };
        let result = self.render();
        self.view = saved;
        self.xr_rendering = saved_xr;
        result
    }
    pub fn render(&mut self) -> Result<(), JsValue> {
        self.render_to(None, true)
    }
    pub fn xr_gpu_supported(&self) -> bool {
        self.output.xr_compatible
    }
    pub fn xr_device(&self) -> Result<JsValue, JsValue> {
        if !self.output.xr_compatible {
            return Err(JsValue::from_str(
                "XR-compatible WebGPU adapter unavailable; use WebGL",
            ));
        }
        self.renderer
            .device
            .as_webgpu()
            .map(|device| device.clone().into())
            .ok_or_else(|| JsValue::from_str("WebGPU device unavailable"))
    }
    pub fn xr_color_format(&self) -> String {
        match self.output.color_format().remove_srgb_suffix() {
            wgpu::TextureFormat::Rgba8Unorm => "rgba8unorm",
            wgpu::TextureFormat::Bgra8Unorm => "bgra8unorm",
            _ => "unsupported",
        }
        .into()
    }
    pub fn render_xr_gpu_image(
        &mut self,
        matrix: Mat4,
        position: Vec3,
        image: &JsValue,
    ) -> Result<(), JsValue> {
        // Validate the frame-scoped image without importing browser textures:
        // the staged path only needs dimensions/format until final presentation.
        let write_depth = js_sys::Reflect::get(image, &JsValue::from_str("ignoreDepthValues"))?
            .as_bool()
            != Some(true);
        // Even without effects, keep linear blending + sRGB output encoding in
        // owned targets; browser UNORM views cannot safely be reinterpreted.
        let info = super::xr_output::XrAttachments::inspect(image)?;
        let index = info.layer as usize;
        if index >= 2 {
            return Err(JsValue::from_str("XR frame supports at most two eyes"));
        }
        self.xr_view_slot = index as u32 + 1;
        // Own the complete output of each eye until both are ready. Browser
        // attachments are never retained here or touched by intermediate passes.
        let mut present = self.xr_present.take().unwrap_or_else(|| {
            super::xr_present::XrPresent::new(&self.renderer.device, info.format)
        });
        let result = (|| {
            present
                .ensure_eye(&self.renderer.device, index, info.width, info.height)
                .map_err(|e| JsValue::from_str(&e))?;
            self.render_xr_gpu_eye(
                matrix,
                position,
                &present.target(index).map_err(|e| JsValue::from_str(&e))?,
                write_depth,
            )?;
            present
                .mark_ready(index, write_depth)
                .map_err(|e| JsValue::from_str(&e))
        })();
        self.xr_present = Some(present);
        result
    }
    pub fn present_xr_gpu_frame(&mut self, images: &js_sys::Array) -> Result<(), JsValue> {
        let Some(present) = &mut self.xr_present else {
            return Ok(());
        };
        let mut attachments = Vec::with_capacity(images.length() as usize);
        let mut indices = Vec::with_capacity(images.length() as usize);
        if images.length() == 0 || images.length() > 2 {
            return Err(JsValue::from_str(
                "XR presentation requires one or two eyes",
            ));
        }
        for image in images.iter() {
            let descriptor = js_sys::Reflect::get(&image, &JsValue::from_str("viewDescriptor"))?;
            let index = js_sys::Reflect::get(&descriptor, &JsValue::from_str("baseArrayLayer"))?
                .as_f64()
                .ok_or_else(|| JsValue::from_str("Missing XR eye layer"))?
                as usize;
            if !present.is_ready(index) {
                return Err(JsValue::from_str("XR eye has not been rendered"));
            }
            attachments.push(super::xr_output::XrAttachments::new(
                &self.renderer.device,
                &image,
            )?);
            indices.push(index);
        }
        let targets: Vec<_> = indices
            .into_iter()
            .zip(attachments.iter().map(|a| a.target()))
            .collect();
        self.renderer.draw_calls += present
            .present(&self.renderer.device, &self.renderer.queue, &targets)
            .map_err(|e| JsValue::from_str(&e))?;
        Ok(())
    }
    pub fn render_xr_gpu_eye(
        &mut self,
        matrix: Mat4,
        position: Vec3,
        target: &super::target::RenderTarget<'_>,
        write_depth: bool,
    ) -> Result<(), JsValue> {
        if target.color_format.remove_srgb_suffix()
            != self.output.color_format().remove_srgb_suffix()
        {
            return Err(JsValue::from_str(
                "XR layer format must match xr_gpu_color_format",
            ));
        }
        if self.xr_scene_pending {
            self.renderer.finish_scene_updates();
            self.xr_scene_pending = false;
        }
        let saved = self.view;
        let saved_xr = self.xr_rendering;
        self.xr_rendering = true;
        self.view = RenderView {
            view_projection: matrix,
            camera_position: position,
        };
        let result = self.render_to(Some(target), write_depth);
        self.view = saved;
        self.xr_rendering = saved_xr;
        result
    }
    fn render_to(
        &mut self,
        external: Option<&super::target::RenderTarget<'_>>,
        write_external_depth: bool,
    ) -> Result<(), JsValue> {
        self.refresh_info_panels(0.0);
        self.renderer.draw_calls = 0;
        self.renderer.main_draw_calls = 0;
        self.renderer.reset_translucency_metrics();
        if let Some(probe) = &mut self.environment {
            probe
                .update(&mut self.renderer)
                .map_err(|e| JsValue::from_str(&e))?;
        }
        let frame = if external.is_none() {
            let Some(frame) = self.output.acquire(&self.renderer.device)? else {
                return Ok(());
            };
            self.output.ensure_target(&self.renderer.device);
            Some(frame)
        } else {
            None
        };
        if let Some(compute) = &mut self.cutting_compute {
            compute.retain(|id| self.renderer.scene.triangle_meshes().contains_key(&id));
        }
        self.resident_sources
            .retain(|id, _| self.renderer.scene.object_exists(*id));
        let canvas_target = frame.as_ref().map(|frame| self.output.target(frame));
        let target = external.or(canvas_target.as_ref()).expect("render target");
        let target = super::target::RenderTarget {
            color_load: wgpu::LoadOp::Clear(self.background),
            ..*target
        };
        // Collect only the main view. Auxiliary shadow/reflection passes still
        // submit before shared uniforms are rewritten for this view.
        let main_view_slot = if self.xr_rendering {
            self.xr_view_slot
        } else {
            0
        };
        let mut commands = Vec::with_capacity(3);
        let mut postprocessed = false;
        if self
            .postprocess
            .needs_render(self.view, target.width, target.height)
        {
            self.postprocess
                .ensure_size(&self.renderer.device, target.width, target.height)
                .map_err(|e| JsValue::from_str(&e))?;
            let mut scene_target = self.postprocess.target();
            scene_target.color_load = wgpu::LoadOp::Clear(self.background);
            commands.push(
                self.renderer
                    .render_view_slot(self.view, &scene_target, main_view_slot)
                    .map_err(|e| JsValue::from_str(&e))?,
            );
            // Effects lazily replaces fallback maps when shadows are first enabled.
            // Rebind by resource identity; unchanged maps do not allocate a new bind group.
            let (static_map, dynamic_map) = self.renderer.shadow_maps();
            self.postprocess
                .set_shadow_maps(&self.renderer.device, static_map, dynamic_map);
            let light = self.renderer.scene.lights().values().next();
            let (position, color) = light
                .map(|l| (l.position, l.color))
                .unwrap_or((Vec3::ZERO, Vec3::ZERO));
            let (passes, postprocess_command) = self.postprocess.render(
                &self.renderer.device,
                &self.renderer.queue,
                self.view,
                position,
                color,
                &target,
                self.renderer.shadows_enabled(),
                self.xr_rendering,
                if self.xr_rendering {
                    self.xr_view_slot
                } else {
                    0
                },
            );
            self.renderer.draw_calls += passes;
            if let Some(command) = postprocess_command {
                commands.push(command);
                postprocessed = true;
            }
            let overlays = super::target::RenderTarget {
                depth: self.postprocess.target().depth,
                auxiliary: None,
                material_mask: false,
                ..target
            };
            if let Some(command) = self
                .renderer
                .draw_postprocess_overlays(&overlays)
                .map_err(|e| JsValue::from_str(&e))?
            {
                commands.push(command);
            }
        } else {
            self.postprocess.skip_render();
            if !self.postprocess.enabled() {
                self.xr_depth_resolve = None;
            }
            commands.push(
                self.renderer
                    .render_view_slot(self.view, &target, main_view_slot)
                    .map_err(|e| JsValue::from_str(&e))?,
            );
        }
        // Flush before depth resolve, HUD, presentation or preparing another eye:
        // those steps may read these attachments or overwrite shared uniforms.
        self.renderer.queue.submit(commands);
        if postprocessed {
            self.postprocess.record_submission();
        }
        if postprocessed && external.is_some() && write_external_depth {
            // Postprocessing writes only color. Supply the compositor with the
            // actual scene depth, including any depth-writing overlays above.
            let resolve = self.xr_depth_resolve.get_or_insert_with(|| {
                super::depth_resolve::DepthResolve::new(&self.renderer.device)
            });
            self.renderer.draw_calls += resolve.render(
                &self.renderer.device,
                &self.renderer.queue,
                self.postprocess.target().depth,
                target.depth,
            );
        }
        // Include the imminent HUD and final texture blit in the displayed total.
        self.renderer.draw_calls +=
            usize::from(self.debug_enabled) + usize::from(frame.is_some() && self.output.offscreen);
        if self.debug_enabled {
            let renderer = &self.renderer;
            let xr = self.xr_rendering;
            let cuts = self.xr_cuts;
            let round = self.xr_round;
            self.renderer.upload_bytes +=
                self.debug_hud.get_or_insert_with(Default::default).render(
                    &renderer.device,
                    &renderer.queue,
                    &target,
                    if !self.xr_rendering || self.xr_hud_pending {
                        self.frame_dt
                    } else {
                        0.0
                    },
                    self.xr_rendering,
                    || {
                        if xr {
                            crate::debug_overlay::HudContent::Xr(
                                renderer.xr_debug_content(cuts, round),
                            )
                        } else {
                            crate::debug_overlay::HudContent::Desktop(renderer.debug_lines())
                        }
                    },
                );
            self.xr_hud_pending = false;
        }
        if let Some(frame) = frame {
            self.output
                .present(frame, &self.renderer.device, &self.renderer.queue);
        }
        if !self.xr_rendering {
            super::profiler::end_frame(&self.renderer.device, &self.renderer.queue);
        }
        Ok(())
    }
}
