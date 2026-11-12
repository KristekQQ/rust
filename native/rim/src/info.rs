//! Existing engine panel and player binding; wall-clock application cadence is
//! deliberately separate from OpenXR's requested/active display frequency.
use glam::Vec3;
use webgpu_wasm::{
    frame_metrics::FrameMetrics, info_panel::PanelKind, scene::SceneManager, xr_player::XrPlayer,
};
pub const OFFSET: Vec3 = Vec3::new(0.9, 0.35, -1.0);
pub const WIDTH: f32 = 0.66;
pub const HEIGHT: f32 = 0.40;
pub struct Info {
    pub panel: u32,
    pub enabled: bool,
    pub timing: FrameMetrics,
    pub refresh_hz: Option<f32>,
    pub work_ms: f64,
    pub update_ms: f64,
    pub simulation: Option<(f64, f64)>,
    refresh_elapsed: f32,
    log_elapsed: f32,
}
impl Info {
    pub fn new(scene: &mut SceneManager, player: &mut XrPlayer) -> Result<Self, String> {
        let panel = scene.add_info_panel("diagnostics", WIDTH, HEIGHT)?;
        Self::from_panel(scene, player, panel)
    }
    pub fn from_panel(
        scene: &mut SceneManager,
        player: &mut XrPlayer,
        panel: u32,
    ) -> Result<Self, String> {
        player.bind_head_offset(scene, Some(panel), OFFSET, true)?;
        Ok(Self {
            panel,
            enabled: true,
            timing: FrameMetrics::default(),
            refresh_hz: None,
            work_ms: 0.,
            update_ms: 0.,
            simulation: None,
            refresh_elapsed: 0.25,
            log_elapsed: 0.,
        })
    }
    pub fn toggle(
        &mut self,
        scene: &mut SceneManager,
        player: &mut XrPlayer,
    ) -> Result<(), String> {
        let enabled = !self.enabled;
        player.bind_head_offset(scene, enabled.then_some(self.panel), OFFSET, true)?;
        self.enabled = enabled;
        self.refresh_elapsed = 0.25;
        Ok(())
    }
    pub fn record(&mut self, elapsed: f32, work_ms: f64) {
        self.timing.record(elapsed);
        self.work_ms = work_ms;
        self.refresh_elapsed += elapsed;
        self.log_elapsed += elapsed;
    }
    pub fn reset(&mut self) {
        self.timing = FrameMetrics::default();
        self.refresh_elapsed = 0.25;
        self.log_elapsed = 0.;
    }
    pub fn refresh(&mut self, renderer: &mut webgpu_wasm::render::renderer::SceneRenderer) {
        if !self.enabled || self.refresh_elapsed < 0.25 {
            return;
        }
        self.refresh_elapsed = 0.;
        let diagnostic = renderer.info_diagnostic_rows();
        let value = |key| {
            diagnostic
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| "N/A".into())
        };
        let mut rows = vec![
            ("RENDER", "RUST VULKAN".into()),
            ("FPS APP", format!("{:.1}", self.timing.fps)),
            (
                "DISPLAY HZ",
                self.refresh_hz
                    .map_or_else(|| "N/A".into(), |v| format!("{v:.0}")),
            ),
            ("FRAME MS", format!("{:.2}", self.timing.frame_ms)),
            ("FRAME P95", format!("{:.2}", self.timing.frame_p95_ms)),
            ("APP+WAIT MS", format!("{:.2}", self.work_ms)),
            ("UPDATE MS", format!("{:.2}", self.update_ms)),
            if let Some((simulation_ms, _)) = self.simulation {
                ("SIM MS", format!("{simulation_ms:.2}"))
            } else {
                ("GPU MS", "N/A".into())
            },
            ("UPLOAD B", renderer.upload_bytes.to_string()),
            ("VISIBLE", value("VISIBLE")),
            ("OBJECTS", value("OBJECTS")),
            ("DRAWS", renderer.draw_calls.to_string()),
        ];
        if let Some((_, age_ms)) = self.simulation {
            rows.push(("SIM AGE MS", format!("{age_ms:.2}")));
        }
        renderer
            .scene
            .update_info_panels(PanelKind::Diagnostics, &rows);
        if self.log_elapsed >= 1. {
            self.log_elapsed = 0.;
            log::info!("Native frame: app_fps={:.1} display_hz={:?} frame_ms={:.2} p95_ms={:.2} app_work_plus_gpu_wait_ms={:.2} draws={}",self.timing.fps,self.refresh_hz,self.timing.frame_ms,self.timing.frame_p95_ms,self.work_ms,renderer.draw_calls);
        }
    }
}
