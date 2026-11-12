//! Host glue only: original scene, renderer, XR player and combat remain engine-owned.
use crate::content::{self, RimContent};
use glam::{Mat4, Vec3};
use webgpu_wasm::render::{
    postprocess::PostProcessor,
    renderer::SceneRenderer,
    target::{RenderTarget, RenderView},
};

#[derive(Default)]
struct Phases {
    poll: f64,
    movement: f64,
    tick: f64,
    advance: f64,
    prepare: f64,
    render: f64,
    job_count: usize,
    accepted: usize,
}

pub struct App {
    pub renderer: SceneRenderer,
    pub content: RimContent,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    post: PostProcessor,
    pub info: crate::info::Info,
    last_frame: Option<std::time::Instant>,
    cuts: crate::cut_worker::CutWorker,
    phases: Phases,
}
impl App {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        read: &dyn Fn(&str) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let mut renderer = SceneRenderer::new(device.clone(), queue.clone());
        let mut content = content::load(&mut renderer.scene, read)?;
        let models: Vec<_> = renderer.scene.models().keys().copied().collect();
        for id in models {
            renderer.register_gltf_model(id)?;
        }
        renderer.set_grid_visible(false);
        renderer.set_light_helpers_visible(false);
        renderer.set_effects(true, false);
        renderer.set_appearance(webgpu_wasm::render_appearance::Appearance::from_config(
            content::appearance(),
        )?);
        let mut post = PostProcessor::new(&device, format);
        post.set_effect("fxaa", true, 1.0);
        let info = crate::info::Info::new(&mut renderer.scene, &mut content.player)?;
        renderer.set_debug_diagnostics(true);
        let mut app = Self {
            renderer,
            content,
            device,
            queue,
            post,
            info,
            last_frame: None,
            cuts: crate::cut_worker::CutWorker::new()?,
            phases: Phases::default(),
        };
        app.content.game.input(
            webgpu_wasm::rim_game::Input {
                start: true,
                ..Default::default()
            },
            &mut app.renderer.scene,
        )?;
        Ok(app)
    }
    pub fn tick_xr(
        &mut self,
        dt: f32,
        head: Option<Mat4>,
        hands: [Option<Mat4>; 2],
        ray: Option<Mat4>,
        axes: [f32; 3],
        aim: bool,
        dash: bool,
    ) -> Result<(), String> {
        let update_started = std::time::Instant::now();
        self.phases = Phases::default();
        let game = &mut self.content.game;
        loop {
            match self.cuts.poll() {
                Ok(Some(result)) => {
                    self.phases.accepted += 1;
                    if let Err(error) = game.accept_prepared_cut(&mut self.renderer.scene, result) {
                        game.cut_failed();
                        return Err(error);
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    game.cut_failed();
                    return Err(error);
                }
            }
        }
        self.phases.poll = update_started.elapsed().as_secs_f64() * 1000.;
        let movement_started = std::time::Instant::now();
        game.xr_buttons(aim, dash);
        // Bound locomotion integration, retain real tracking time for blade speed.
        game.xr_move(head, ray, axes, false, dt.min(0.05));
        let origin = game.xr_origin;
        let world_head = head.map(|p| origin * p);
        let world_hands = hands.map(|p| p.map(|p| origin * p));
        self.content.player.set_reference(origin);
        self.content
            .player
            .update_head(&mut self.renderer.scene, world_head)?;
        for (i, pose) in world_hands.into_iter().enumerate() {
            self.content.player.update_hand(
                &mut self.renderer.scene,
                i,
                pose,
                pose.is_some(),
                dt,
            )?;
        }
        self.phases.movement = movement_started.elapsed().as_secs_f64() * 1000.;
        let tick_started = std::time::Instant::now();
        let sim = game.tick(&mut self.renderer.scene, dt, world_head, world_hands);
        self.phases.tick = tick_started.elapsed().as_secs_f64() * 1000.;
        if game.is_dead() {
            self.cuts.invalidate();
            self.content
                .restart_if_dead(&mut self.renderer.scene, head)?;
            self.info.refresh(&mut self.renderer);
            self.renderer.draw_calls = 0;
            self.renderer.main_draw_calls = 0;
            self.renderer.advance_scene_with_elapsed(dt, 0.);
            self.info.update_ms = update_started.elapsed().as_secs_f64() * 1000.;
            log::info!(
                "Arena reset after HP reached zero; fresh wave, safe spawn, existing GPU assets"
            );
            return Ok(());
        }
        self.info.refresh(&mut self.renderer);
        let advance_started = std::time::Instant::now();
        self.renderer.draw_calls = 0;
        self.renderer.main_draw_calls = 0;
        self.renderer.advance_scene_with_elapsed(dt, sim);
        self.phases.advance = advance_started.elapsed().as_secs_f64() * 1000.;
        // Match web: one bounded preparation chunk per frame, at most two
        // outstanding jobs; clipping runs on the worker without XR/GPU access.
        if self.cuts.available() {
            let prepare_started = std::time::Instant::now();
            let job = game.take_cut_work()?;
            self.phases.prepare = prepare_started.elapsed().as_secs_f64() * 1000.;
            self.phases.job_count = usize::from(job.is_some());
            if let Some(job) = job {
                if let Err(error) = self.cuts.submit(job) {
                    game.cut_failed();
                    return Err(error);
                }
            }
        }
        self.info.update_ms = update_started.elapsed().as_secs_f64() * 1000.;
        Ok(())
    }
    pub fn toggle_info(&mut self) -> Result<(), String> {
        self.info
            .toggle(&mut self.renderer.scene, &mut self.content.player)?;
        self.renderer.set_debug_diagnostics(self.info.enabled);
        Ok(())
    }
    pub fn reset_frame_clock(&mut self) {
        self.last_frame = None;
        self.info.reset();
    }
    pub fn frame_submitted(&mut self, work: std::time::Duration) {
        let now = std::time::Instant::now();
        if self.phases.job_count > 0 || self.phases.accepted > 0 || work.as_secs_f64() > 0.020 {
            let p = &self.phases;
            log::info!("Native phases: work_ms={:.2} update_ms={:.2} poll_ms={:.2} movement_ms={:.2} tick_ms={:.2} advance_ms={:.2} prepare_ms={:.2} render_ms={:.2} jobs={} accepted={} upload_bytes={} draws={}",work.as_secs_f64()*1000.,self.info.update_ms,p.poll,p.movement,p.tick,p.advance,p.prepare,p.render,p.job_count,p.accepted,self.renderer.upload_bytes,self.renderer.draw_calls);
        }
        if let Some(last) = self.last_frame {
            self.info.record(
                now.duration_since(last).as_secs_f32(),
                work.as_secs_f64() * 1000.,
            );
        }
        self.last_frame = Some(now);
    }
    pub fn render(
        &mut self,
        view: RenderView,
        target: &RenderTarget<'_>,
        slot: u32,
    ) -> Result<(), String> {
        let render_started = std::time::Instant::now();
        crate::presentation::render_scene(
            &mut self.renderer,
            &mut self.post,
            &self.device,
            &self.queue,
            view,
            target,
            slot,
            content::background(),
        )?;
        let ready: Vec<_> = self
            .content
            .game
            .staged_cut_ids()
            .filter(|id| self.renderer.triangle_mesh_uploaded(*id))
            .collect();
        self.content.game.ack_cut_uploads(&ready);
        self.phases.render += render_started.elapsed().as_secs_f64() * 1000.;
        Ok(())
    }
}

pub fn pose(p: openxr::Posef) -> Mat4 {
    Mat4::from_rotation_translation(
        glam::Quat::from_xyzw(
            p.orientation.x,
            p.orientation.y,
            p.orientation.z,
            p.orientation.w,
        ),
        Vec3::new(p.position.x, p.position.y, p.position.z),
    )
}
/// OpenXR asymmetric tangents, right-handed world, wgpu 0..1 depth.
pub fn projection(f: openxr::Fovf, near: f32, far: f32) -> Mat4 {
    let (l, r, b, t) = (
        f.angle_left.tan(),
        f.angle_right.tan(),
        f.angle_down.tan(),
        f.angle_up.tan(),
    );
    Mat4::from_cols_array(&[
        2. / (r - l),
        0.,
        0.,
        0.,
        0.,
        2. / (t - b),
        0.,
        0.,
        (r + l) / (r - l),
        (t + b) / (t - b),
        -far / (far - near),
        -1.,
        0.,
        0.,
        -far * near / (far - near),
        0.,
    ])
}
