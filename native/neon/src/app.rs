//! XR owns rendering and late hand/head poses; gameplay is simulation-thread owned.
pub const NEON_GLOW_STRENGTH: f32 = 0.4;
pub const NEON_GLOW_RADIUS: f32 = 0.045;
pub const NEON_SSR_STRENGTH: f32 = 0.92;
pub const NEON_SSR_SCALE: f32 = 0.5;
use crate::{
    benchmark_replay::Replay,
    content::Content,
    game::Game,
    observer::Observer,
    settings::ReferenceMode,
    simulation::{Simulation, Snapshot, WorkerMetrics},
};
use glam::{Mat4, Vec3};
use kristek_rim_native::{
    info::Info,
    measurement::{AppMetrics, Config, ReplayMetrics, Setup},
    presentation::render_scene,
    xr::{FrameInput, HostApp},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use webgpu_wasm::{
    render::{
        postprocess::PostProcessor,
        renderer::SceneRenderer,
        target::{RenderTarget, RenderView},
    },
    scene::SceneManager,
    xr_player::XrPlayer,
};
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplayConfig {
    cutting: bool,
    duration_s: f32,
}
struct PlayerReplay {
    config: ReplayConfig,
    driver: Replay,
    settle_s: f32,
    last_tick: Option<Instant>,
    started: Option<Instant>,
    finished: bool,
}
impl PlayerReplay {
    fn load(score_path: Option<&std::path::Path>) -> Result<Option<Self>, String> {
        let Some(dir) = score_path.and_then(|p| p.parent()) else {
            return Ok(None);
        };
        let bytes = match std::fs::read(dir.join("neon-replay.json")) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("Replay config: {error}")),
        };
        let config: ReplayConfig = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if !config.duration_s.is_finite() || !(10.0..=120.0).contains(&config.duration_s) {
            return Err("Replay duration must be10..120 seconds".into());
        }
        log::info!("Diagnostic player replay: cutting={} duration={}s; ordinary game/worker; temporary score store",config.cutting,config.duration_s);
        Ok(Some(Self {
            driver: Replay::new(config.cutting),
            config,
            settle_s: 0.,
            last_tick: None,
            started: None,
            finished: false,
        }))
    }
    fn metrics(&self) -> ReplayMetrics {
        ReplayMetrics {
            active: self.started.is_some() && !self.finished,
            phase: if self.finished {
                "finished"
            } else if self.started.is_some() {
                "playing"
            } else {
                "settling"
            },
            cutting: self.config.cutting,
            elapsed_s: self.driver.elapsed(),
            head_world: self.driver.head().to_cols_array_2d(),
        }
    }
    fn input(&mut self, actual: FrameInput, targets: &[(u32, Vec3)]) -> (FrameInput, Mat4, bool) {
        if self.finished {
            return (actual, Mat4::IDENTITY, false);
        }
        let now = Instant::now();
        if self
            .started
            .is_some_and(|t| t.elapsed().as_secs_f32() > self.config.duration_s + 30.)
            || self.driver.elapsed() >= self.config.duration_s
        {
            self.finished = true;
            log::info!("Diagnostic player replay finished; physical head/hands restored");
            return (actual, Mat4::IDENTITY, true);
        }
        let valid = actual.head.is_some() && actual.dt.is_finite() && actual.dt > 0.;
        let uninterrupted = self
            .last_tick
            .is_none_or(|t| now.duration_since(t).as_secs_f32() < 0.25);
        self.last_tick = Some(now);
        let stable = valid && uninterrupted && actual.dt <= 0.1;
        if !stable {
            self.settle_s = 0.;
        }
        if stable && self.started.is_none() {
            self.settle_s += actual.dt;
        }
        let just_started = self.started.is_none() && self.settle_s >= 8.;
        if just_started {
            self.started = Some(now);
            log::info!(
                "Diagnostic player replay START after8s stable frames; starting ordinary minute"
            );
        }
        let mut scripted = if self.started.is_some() && valid {
            self.driver.frame(actual.dt, targets)
        } else {
            FrameInput {
                // Pre-measurement settling must not consume cues/variant cursors.
                dt: 0.,
                head: actual.head.map(|_| self.driver.head()),
                ..Default::default()
            }
        };
        // Game's ordinary first valid tick starts its authored minute.
        scripted.primary = false;
        let reference = match (actual.head, scripted.head) {
            (Some(physical), Some(virtual_head)) => virtual_head * physical.inverse(),
            _ => Mat4::IDENTITY,
        };
        (scripted, reference, false)
    }
}
pub struct App {
    pub renderer: SceneRenderer,
    pub player: XrPlayer,
    pub simulation: Simulation,
    pub simulation_state: serde_json::Value,
    pub info: Info,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub applied_sequence: u64,
    local_ids: [u32; 3],
    post: PostProcessor,
    last_frame: Option<Instant>,
    published: Option<Instant>,
    simulation_ms: f64,
    log_elapsed: f32,
    reference_mode: ReferenceMode,
    measurement_setup: Option<Setup>,
    worker_measurement: WorkerMetrics,
    snapshot_published: Option<Instant>,
    observer: Option<Observer>,
    replay: Option<PlayerReplay>,
    replay_targets: Vec<(u32, Vec3)>,
    view_reference: Mat4,
    glow_logged_views: u8,
    audio: crate::audio::Audio,
}
impl App {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        read: &dyn Fn(&str) -> Result<Vec<u8>, String>,
        score_path: Option<PathBuf>,
    ) -> Result<Self, String> {
        let observer = Observer::load(score_path.as_deref());
        let scene_observer = crate::scene_observer::SceneObserver::load(score_path.as_deref());
        let replay = PlayerReplay::load(score_path.as_deref())?;
        // Preserve the user's TOP5 content, but write synthetic scores only to a
        // private diagnostic copy. Normal startup still uses the original path.
        let score_path = if replay.is_some() {
            match score_path {
                Some(original) => {
                    let temporary = original.with_file_name("neon-replay-top5.json");
                    match std::fs::copy(&original, &temporary) {
                        Ok(_) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            if temporary.exists() {
                                std::fs::remove_file(&temporary).map_err(|e| e.to_string())?;
                            }
                        }
                        Err(e) => return Err(format!("Cannot isolate replay scores: {e}")),
                    }
                    Some(temporary)
                }
                None => None,
            }
        } else {
            score_path
        };
        let measurement_setup = score_path
            .as_ref()
            .and_then(|p| p.parent())
            .and_then(|dir| {
                let path = dir.join("neon-measure.json");
                match std::fs::read(&path) {
                    Ok(bytes) => match serde_json::from_slice::<Config>(&bytes) {
                        Ok(config) => match config.validate() {
                            Ok(()) => Some(Setup {
                                config,
                                output_dir: dir.join("measurements"),
                            }),
                            Err(error) => {
                                log::warn!("Measurement config rejected: {error}");
                                None
                            }
                        },
                        Err(error) => {
                            log::warn!("Measurement config invalid: {error}");
                            None
                        }
                    },
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => {
                        log::warn!("Measurement config unreadable: {error}");
                        None
                    }
                }
            });
        let mut renderer = SceneRenderer::new(device.clone(), queue.clone());
        let content = Content::load(&mut renderer.scene, read)?;
        // Independent startup mirrors share stable authored IDs. Only the worker
        // owns physics/contact history, timers, scoring and CPU cut geometry.
        let mut scene = SceneManager::default();
        let simulation_content = Content::load(&mut scene, read)?;
        let game = Game::new(&mut scene, &simulation_content, score_path)?;
        let local_ids = [
            game.ref_id("infoPanel")?,
            game.ref_id("leftSword")?,
            game.ref_id("sword")?,
        ];
        if content.refs != simulation_content.refs {
            return Err("Simulation/render authored IDs differ".into());
        }
        for id in renderer.scene.models().keys().copied().collect::<Vec<_>>() {
            renderer.register_gltf_model(id)?;
        }
        renderer.set_grid_visible(false);
        renderer.set_light_helpers_visible(false);
        renderer.set_effects(false, false);
        renderer.configure_neon_glow(NEON_GLOW_STRENGTH, NEON_GLOW_RADIUS);
        // Soft ordinary halos use half-resolution rendering; sharp surfaces and
        // glyph atlases retain full resolution. Unsupported targets fall back.
        renderer.configure_neon_glow_resolution(0.5);
        renderer.configure_font_glow_atlas(true);
        renderer.prewarm_font_glow_atlas("0123456789+-");
        renderer.set_debug_diagnostics(true);
        let mut player = XrPlayer::default();
        player.bind_hand(&mut renderer.scene, 0, Some(local_ids[1]), false)?;
        player.bind_hand(&mut renderer.scene, 1, Some(local_ids[2]), false)?;
        let info = Info::from_panel(&mut renderer.scene, &mut player, local_ids[0])?;
        let simulation_state = game.state();
        let simulation = Simulation::new_with_observer(
            scene,
            game,
            measurement_setup.is_some(),
            scene_observer,
        )?;
        let mut post = PostProcessor::new(&device, format);
        post.set_effect("fxaa", false, 1.);
        // Match the original browser arena: half-resolution floor SSR at .92.
        post.configure_ssr(NEON_SSR_SCALE);
        post.configure_ssr_region(
            Vec3::new(-1.5, -0.03, -3.),
            Vec3::new(1.5, 0.03, 0.75),
            true,
        );
        post.set_effect("ssr", true, NEON_SSR_STRENGTH);
        let mut app = Self {
            renderer,
            player,
            simulation,
            simulation_state,
            info,
            device,
            queue,
            local_ids,
            post,
            last_frame: None,
            applied_sequence: 0,
            published: None,
            simulation_ms: 0.,
            log_elapsed: 0.,
            reference_mode: ReferenceMode::Recenterable,
            measurement_setup,
            worker_measurement: WorkerMetrics::default(),
            snapshot_published: None,
            observer,
            replay,
            replay_targets: Vec::with_capacity(16),
            view_reference: Mat4::IDENTITY,
            glow_logged_views: 0,
            audio: crate::audio::Audio::new(),
        };
        let initial = app.simulation.wait_initial()?;
        app.apply(initial);
        app.renderer.finish_scene_updates();
        // OpenXR's right-handed perspective has a negative determinant. Compile
        // the same mesh/alpha/glow variants before entering the XR frame loop.
        let preparation = Instant::now();
        app.renderer
            .prewarm_main_pipelines(format, wgpu::TextureFormat::Depth32Float, true);
        log::info!(
            "Neon pipeline preparation: {:.2} ms",
            preparation.elapsed().as_secs_f64() * 1000.
        );
        Ok(app)
    }
    /// Startup-only choice: the host selects its reference space after construction.
    pub fn with_reference_mode(mut self, mode: ReferenceMode) -> Self {
        self.reference_mode = mode;
        log::info!("Neon reference mode: {mode:?}");
        self
    }
    fn apply(&mut self, snapshot: Snapshot) {
        self.applied_sequence = snapshot.sequence;
        self.simulation_ms = snapshot.simulation_ms;
        self.worker_measurement = snapshot.measurement;
        self.snapshot_published = Some(snapshot.published);
        self.published = Some(snapshot.sampled);
        self.audio
            .snapshot(snapshot.audio, snapshot.sampled.elapsed().as_secs_f64());
        self.simulation_state = snapshot.state;
        let retired = self
            .renderer
            .scene
            .apply_render_publication(snapshot.publication, &self.local_ids);
        self.simulation.retire(retired);
    }
    /// Offscreen acceptance only, never called from the real OpenXR frame path.
    pub fn replay_ssr(&mut self, enabled: bool) {
        self.post.set_effect("ssr", enabled, NEON_SSR_STRENGTH);
    }
    pub fn reflection_work(&self) -> [f64; 2] {
        self.post.ssr_work_stats()
    }
    pub fn tick_replay(&mut self, input: FrameInput) -> Result<(), String> {
        self.tick_input(input)?;
        // tick_input may already consume the completed publication. Waiting
        // again would time out: the worker publishes each sequence only once.
        if self.applied_sequence < self.simulation.sequence {
            let snapshot = self.simulation.wait_replay()?;
            self.apply(snapshot);
        }
        self.renderer.finish_scene_updates();
        Ok(())
    }
}
impl HostApp for App {
    fn measurement_setup(&self) -> Option<Setup> {
        self.measurement_setup.clone()
    }
    fn measurement_metrics(&self) -> AppMetrics {
        let w = self.worker_measurement;
        AppMetrics {
            replay: self.replay.as_ref().map(PlayerReplay::metrics),
            input_sequence: self.simulation.sequence,
            applied_sequence: self.applied_sequence,
            worker_ms: self.simulation_ms,
            simulation_age_ms: self
                .published
                .map_or(0., |t| t.elapsed().as_secs_f64() * 1000.),
            publication_age_ms: self
                .snapshot_published
                .map_or(0., |t| t.elapsed().as_secs_f64() * 1000.),
            worker_samples: w.samples,
            worker_queue_ms: w.queue_ms,
            worker_game_ms: w.game_ms,
            worker_publication_ms: w.publication_ms,
            worker_retirement_ms: w.retirement_ms,
            draws: self.renderer.draw_calls as u64,
            upload_bytes: self.renderer.upload_bytes as u64,
            cuts: self.simulation_state["cuts"].as_u64().unwrap_or(0),
            shots: self.simulation_state["shots"].as_u64().unwrap_or(0),
            cue_index: self.simulation_state["cueIndex"].as_u64().unwrap_or(0),
            flying: self.simulation_state["flying"]
                .as_array()
                .map_or(0, |a| a.len() as u32),
            fragments: self.simulation_state["fragments"].as_u64().unwrap_or(0) as u32,
            batched: self.simulation.batched,
            dropped: self.simulation.dropped,
        }
    }
    fn recenterable_floor(&self) -> bool {
        self.reference_mode == ReferenceMode::Recenterable
    }
    fn set_xr(&mut self, active: bool) {
        self.audio.set_active(active);
        if active {
            self.simulation.new_session();
        }
    }
    fn set_refresh_hz(&mut self, hz: Option<f32>) {
        self.info.refresh_hz = hz;
    }
    fn reset_tracking(&mut self) {
        self.player.reset_tracking(&mut self.renderer.scene);
        self.simulation.reset_tracking();
    }
    fn reset_frame_clock(&mut self) {
        self.last_frame = None;
        self.info.reset();
    }
    fn toggle_info(&mut self) -> Result<(), String> {
        self.info
            .toggle(&mut self.renderer.scene, &mut self.player)?;
        self.renderer.set_debug_diagnostics(self.info.enabled);
        Ok(())
    }
    fn reference(&self) -> Mat4 {
        self.view_reference
    }
    fn tick_input(&mut self, input: FrameInput) -> Result<(), String> {
        let start = Instant::now();
        let input = if let Some(replay) = &mut self.replay {
            self.replay_targets.clear();
            if let Some(flying) = self.simulation_state["flying"].as_array() {
                for id in flying.iter().filter_map(|id| id.as_u64()).take(16) {
                    if let Some(object) = self.renderer.scene.objects().get(&(id as u32)) {
                        self.replay_targets
                            .push((id as u32, object.transform.position));
                    }
                }
            }
            let (scripted, reference, reset) = replay.input(input, &self.replay_targets);
            self.view_reference = reference;
            if reset {
                self.player.reset_tracking(&mut self.renderer.scene);
                self.simulation.reset_tracking();
            }
            scripted
        } else {
            input
        };
        if let Some(head) = input.head {
            self.audio.listener(head.to_cols_array());
        }
        if let Some(observer) = &self.observer {
            observer.publish(input.dt, input.head, input.hands);
        }
        self.info.simulation = Some((
            self.simulation_ms,
            self.published
                .map_or(0., |t| t.elapsed().as_secs_f64() * 1000.),
        ));
        self.info.refresh(&mut self.renderer);
        self.renderer.draw_calls = 0;
        self.renderer.main_draw_calls = 0;
        self.renderer.begin_external_scene_frame(input.dt);
        self.simulation.submit(input)?;
        if let Some(snapshot) = self.simulation.poll()? {
            self.apply(snapshot);
        }
        // No collision/cutting on these render-only equipment controllers.
        if input.head.is_none() || !input.dt.is_finite() || input.dt <= 0. {
            self.player.reset_tracking(&mut self.renderer.scene);
        } else {
            self.player
                .update_head(&mut self.renderer.scene, input.head)?;
            for hand in 0..2 {
                self.player.update_hand(
                    &mut self.renderer.scene,
                    hand,
                    input.hands[hand],
                    false,
                    input.dt,
                )?;
            }
        }
        self.renderer.finish_scene_updates();
        self.info.update_ms = start.elapsed().as_secs_f64() * 1000.;
        self.log_elapsed += input.dt.max(0.);
        if self.log_elapsed >= 1. {
            self.log_elapsed = 0.;
            log::info!("Neon simulation: worker_ms={:.2} snapshot_age_ms={:.2} xr_update_ms={:.2} sequence={} input_sequence={} batched={} dropped={}",
                self.simulation_ms, self.published.map_or(0., |t|t.elapsed().as_secs_f64()*1000.), self.info.update_ms,
                self.applied_sequence, self.simulation.sequence, self.simulation.batched, self.simulation.dropped);
        }
        Ok(())
    }
    fn render(
        &mut self,
        view: RenderView,
        target: &RenderTarget<'_>,
        slot: u32,
    ) -> Result<(), String> {
        render_scene(
            &mut self.renderer,
            &mut self.post,
            &self.device,
            &self.queue,
            view,
            target,
            slot,
            [0., 0., 0.],
        )?;
        let view_bit = 1u8 << slot.min(7);
        if self.glow_logged_views & view_bit == 0 {
            log::info!(
                "Neon halo view {slot}: {}",
                self.renderer.debug_values()["halfGlow"]
            );
            self.glow_logged_views |= view_bit;
        }
        Ok(())
    }
    fn frame_submitted(&mut self, work: Duration) {
        let now = Instant::now();
        if let Some(last) = self.last_frame {
            self.info.record(
                now.duration_since(last).as_secs_f32(),
                work.as_secs_f64() * 1000.,
            );
        }
        self.last_frame = Some(now);
    }
    fn take_haptics(&mut self) -> [f32; 2] {
        self.simulation.take_haptics()
    }
}

#[cfg(test)]
mod replay_host_tests {
    use super::*;
    fn replay() -> PlayerReplay {
        PlayerReplay {
            config: ReplayConfig {
                cutting: true,
                duration_s: 65.,
            },
            driver: Replay::new(true),
            settle_s: 0.,
            last_tick: None,
            started: None,
            finished: false,
        }
    }
    #[test]
    fn player_view_preserves_both_physical_eye_offsets() {
        let mut replay = replay();
        let physical = Mat4::from_rotation_translation(
            glam::Quat::from_rotation_y(1.3),
            Vec3::new(4., 0.3, -7.),
        );
        let input = FrameInput {
            dt: 1. / 90.,
            head: Some(physical),
            ..Default::default()
        };
        let (scripted, reference, reset) = replay.input(input, &[]);
        assert!(!reset);
        for x in [-0.032, 0.032] {
            let eye = physical * Mat4::from_translation(Vec3::new(x, 0., 0.));
            let actual = reference * eye;
            let expected = scripted
                .head
                .unwrap()
                .transform_point3(Vec3::new(x, 0., 0.));
            assert!(actual.w_axis.truncate().distance(expected) < 1e-5);
        }
        assert!(
            !replay.metrics().active,
            "settling must not arm the recorder"
        );
        assert!(scripted.hands.iter().all(Option::is_none));
        assert_eq!(
            scripted.dt, 0.,
            "settling must not consume the minute or its variant cursor"
        );
    }
    #[test]
    fn bounded_replay_restores_physical_input_and_resets_tracking_once() {
        let mut replay = replay();
        replay.started = Some(Instant::now() - Duration::from_secs(100));
        let actual = FrameInput {
            dt: 1. / 90.,
            head: Some(Mat4::from_translation(Vec3::new(2., 1.5, 1.))),
            ..Default::default()
        };
        let (restored, reference, reset) = replay.input(actual, &[]);
        assert!(reset && replay.finished);
        assert_eq!(restored.head, actual.head);
        assert_eq!(reference, Mat4::IDENTITY);
        assert!(!replay.input(actual, &[]).2);
        assert_eq!(replay.metrics().phase, "finished");
    }
    #[test]
    fn active_replay_uses_production_stall_clamp_instead_of_pausing_game_time() {
        let mut replay = replay();
        replay.started = Some(Instant::now());
        for dt in [0.075, 0.1, 0.2] {
            let input = FrameInput {
                dt,
                head: Some(Mat4::IDENTITY),
                ..Default::default()
            };
            let (scripted, _, reset) = replay.input(input, &[]);
            assert!(!reset);
            assert_eq!(scripted.dt, dt.min(0.1));
        }
    }
}
