//! Single owner of gameplay/physics/cuts; XR never takes its scene lock or waits.
use crate::{game::Game, scene_observer::SceneObserver};
use kristek_rim_native::xr::FrameInput;
use serde_json::Value;
use std::{
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc::{self, SyncSender, TrySendError},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use webgpu_wasm::scene::{
    publication::{RenderPublication, RenderPublisher, RenderRetirement},
    SceneManager,
};
const INPUT_CAPACITY: usize = 8;
const MAX_PENDING_SAMPLES: usize = 128;
struct Sample {
    frame: FrameInput,
    reset: bool,
    session: bool,
    sampled: Instant,
}
struct Input {
    samples: Vec<Sample>,
    sequence: u64,
    retired: Vec<RenderRetirement>,
}
#[derive(Clone, Copy, Default)]
pub struct WorkerMetrics {
    pub samples: u32,
    pub queue_ms: f64,
    pub game_ms: f64,
    pub publication_ms: f64,
    pub retirement_ms: f64,
}
pub struct Snapshot {
    pub publication: RenderPublication,
    pub state: Value,
    pub audio: webgpu_wasm::audio::AudioSnapshot,
    pub sequence: u64,
    pub simulation_ms: f64,
    pub measurement: WorkerMetrics,
    pub published: Instant,
    pub sampled: Instant,
    pub execution_thread: std::thread::ThreadId,
}
#[derive(Default)]
struct Output {
    latest: Option<Snapshot>,
    error: Option<String>,
}
pub struct Simulation {
    sender: Option<SyncSender<Input>>,
    output: Arc<Mutex<Output>>,
    haptics: Arc<[AtomicU32; 2]>,
    thread: Option<JoinHandle<()>>,
    pending: Option<Input>,
    reset: bool,
    session: bool,
    pub sequence: u64,
    pub batched: u64,
    pub dropped: u64,
    retired: Vec<RenderRetirement>,
}
impl Simulation {
    pub fn new(scene: SceneManager, game: Game) -> Result<Self, String> {
        Self::new_with_measurement(scene, game, false)
    }
    pub fn new_with_measurement(
        scene: SceneManager,
        game: Game,
        measure: bool,
    ) -> Result<Self, String> {
        Self::new_with_observer(scene, game, measure, None)
    }
    pub(crate) fn new_with_observer(
        mut scene: SceneManager,
        mut game: Game,
        measure: bool,
        mut observer: Option<SceneObserver>,
    ) -> Result<Self, String> {
        let (sender, inputs) = mpsc::sync_channel::<Input>(INPUT_CAPACITY);
        let output = Arc::new(Mutex::new(Output::default()));
        let shared = output.clone();
        let haptics = Arc::new([AtomicU32::new(0), AtomicU32::new(0)]);
        let vibration = haptics.clone();
        let thread = std::thread::Builder::new()
            .name("neon-simulation".into())
            .spawn(move || {
                log::info!(
                    "Neon simulation worker started: {:?}",
                    std::thread::current().id()
                );
                let mut publisher = RenderPublisher::default();
                // Startup prewarming is finished before entering the XR frame loop.
                let start = Instant::now();
                let state = game.state();
                if let Some(observer) = &mut observer {
                    observer.publish_scene(&mut scene, &state);
                }
                let publication = publisher.capture(&mut scene);
                shared.lock().unwrap_or_else(|e| e.into_inner()).latest = Some(Snapshot {
                    publication,
                    state,
                    audio: game.audio.snapshot().clone(),
                    sequence: 0,
                    simulation_ms: start.elapsed().as_secs_f64() * 1000.,
                    measurement: WorkerMetrics::default(),
                    published: Instant::now(),
                    sampled: start,
                    execution_thread: std::thread::current().id(),
                });
                while let Ok(input) = inputs.recv() {
                    // Old render maps are deallocated by the simulation owner, not XR.
                    let retirement = measure.then(Instant::now);
                    let queue_ms = retirement.map_or(0., |t| {
                        t.duration_since(input.samples[0].sampled).as_secs_f64() * 1000.
                    });
                    drop(input.retired);
                    let retirement_ms =
                        retirement.map_or(0., |t| t.elapsed().as_secs_f64() * 1000.);
                    let start = Instant::now();
                    let sampled = input.samples.last().expect("nonempty input batch").sampled;
                    let mut measurement = WorkerMetrics {
                        samples: input.samples.len() as u32,
                        queue_ms,
                        retirement_ms,
                        ..Default::default()
                    };
                    for sample in input.samples {
                        if sample.session {
                            game.active = false;
                        }
                        if sample.reset {
                            game.reset_tracking(&mut scene);
                        }
                        let mut frame = sample.frame;
                        // Every original sample keeps its own pose, button states,
                        // aim ray and pause boundary. Stall clamping is identical for
                        // gameplay and physics; ordinary 120Hz samples are unmodified.
                        frame.dt = if frame.dt.is_finite() && frame.dt > 0. && frame.head.is_some()
                        {
                            frame.dt.min(0.1)
                        } else {
                            0.
                        };
                        scene.update(frame.dt);
                        if let Err(error) = game.tick(&mut scene, frame) {
                            log::error!(
                                "Neon simulation tick failed at input {}: {error}",
                                input.sequence
                            );
                            shared.lock().unwrap_or_else(|e| e.into_inner()).error = Some(error);
                            return;
                        }
                        for (hand, strength) in std::mem::replace(&mut game.haptics, [0.; 2])
                            .into_iter()
                            .enumerate()
                        {
                            if strength > 0. {
                                vibration[hand]
                                    .fetch_max((strength * 1000.) as u32, Ordering::Release);
                            }
                        }
                    }
                    let publication_start = measure.then(Instant::now);
                    measurement.game_ms = publication_start
                        .map_or(0., |t| t.duration_since(start).as_secs_f64() * 1000.);
                    let state = game.state();
                    if let Some(observer) = &mut observer {
                        observer.publish_scene(&mut scene, &state);
                    }
                    let publication = publisher.capture(&mut scene);
                    measurement.publication_ms =
                        publication_start.map_or(0., |t| t.elapsed().as_secs_f64() * 1000.);
                    let snapshot = Snapshot {
                        publication,
                        state,
                        audio: game.audio.snapshot().clone(),
                        sequence: input.sequence,
                        simulation_ms: start.elapsed().as_secs_f64() * 1000.,
                        measurement,
                        published: Instant::now(),
                        sampled,
                        execution_thread: std::thread::current().id(),
                    };
                    // Replaced snapshots are destroyed here, outside the mailbox lock.
                    let retired = shared
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .latest
                        .replace(snapshot);
                    drop(retired);
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            sender: Some(sender),
            output,
            haptics,
            thread: Some(thread),
            pending: None,
            reset: false,
            session: false,
            sequence: 0,
            batched: 0,
            dropped: 0,
            retired: Vec::new(),
        })
    }
    pub fn retire(&mut self, retired: RenderRetirement) {
        self.retired.push(retired);
    }
    pub fn reset_tracking(&mut self) {
        self.reset = true;
    }
    pub fn new_session(&mut self) {
        self.session = true;
        self.reset = true;
    }
    fn try_flush(&mut self) -> Result<(), String> {
        if let Some(input) = self.pending.take() {
            match self
                .sender
                .as_ref()
                .ok_or("Simulation closed")?
                .try_send(input)
            {
                Ok(()) => {}
                Err(TrySendError::Full(input)) => self.pending = Some(input),
                Err(TrySendError::Disconnected(_)) => return Err(self.worker_stopped_error()),
            }
        }
        Ok(())
    }
    fn worker_stopped_error(&self) -> String {
        // A stopped worker cannot hold this lock doing gameplay. Keep even this
        // failure path nonblocking, and retain the original tick error for diagnosis.
        self.output
            .try_lock()
            .ok()
            .and_then(|output| output.error.clone())
            .map_or_else(
                || "Neon simulation worker stopped".into(),
                |error| format!("Neon simulation worker failed: {error}"),
            )
    }
    /// Bounded FIFO. Preserve raw samples and their discrete control transitions;
    /// batch only publication when busy, never OR states or sum elapsed times.
    pub fn submit(&mut self, frame: FrameInput) -> Result<u64, String> {
        self.try_flush()?;
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.samples.len() >= MAX_PENDING_SAMPLES)
        {
            // Explicit extreme overload policy: bounded memory, never synthesize
            // a long blade swing from skipped history. Telemetry exposes losses.
            self.dropped += 1;
            self.reset = true;
            return Ok(self.sequence);
        }
        self.sequence += 1;
        let sample = Sample {
            frame,
            reset: std::mem::take(&mut self.reset),
            session: std::mem::take(&mut self.session),
            sampled: Instant::now(),
        };
        if let Some(input) = &mut self.pending {
            input.samples.push(sample);
            input.sequence = self.sequence;
            input.retired.append(&mut self.retired);
            self.batched += 1;
        } else {
            self.pending = Some(Input {
                samples: vec![sample],
                sequence: self.sequence,
                retired: std::mem::take(&mut self.retired),
            });
        }
        self.try_flush()?;
        Ok(self.sequence)
    }
    pub fn poll(&self) -> Result<Option<Snapshot>, String> {
        // A collision/cut never holds this mutex. XR does not wait even on the
        // short publication exchange; keep drawing the previously applied state.
        let Ok(mut output) = self.output.try_lock() else {
            return Ok(None);
        };
        if let Some(error) = output.error.take() {
            return Err(error);
        }
        Ok(output.latest.take())
    }
    pub fn take_haptics(&self) -> [f32; 2] {
        std::array::from_fn(|i| self.haptics[i].swap(0, Ordering::AcqRel) as f32 / 1000.)
    }
    /// Constructor-only prewarming, before the first OpenXR frame.
    pub fn wait_initial(&mut self) -> Result<Snapshot, String> {
        if self.sequence != 0 {
            return Err("Initial publication already passed".into());
        }
        self.wait_replay()
    }
    /// Constructor or deterministic OFFSCREEN replay only. Never in an XR tick.
    pub fn wait_replay(&mut self) -> Result<Snapshot, String> {
        if let Some(input) = self.pending.take() {
            self.sender
                .as_ref()
                .ok_or("Simulation closed")?
                .send(input)
                .map_err(|e| e.to_string())?;
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(snapshot) = self.poll()? {
                if snapshot.sequence >= self.sequence {
                    return Ok(snapshot);
                }
            }
            if Instant::now() > deadline {
                return Err("Simulation replay timed out".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
impl Drop for Simulation {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{content::Content, game::Recipe};
    use glam::{Mat4, Vec3};
    fn fixture() -> (SceneManager, Game) {
        let mut scene = SceneManager::default();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let content = Content::load(&mut scene, &|name| {
            std::fs::read(root.join(name)).map_err(|e| e.to_string())
        })
        .unwrap();
        let game = Game::new(&mut scene, &content, None).unwrap();
        (scene, game)
    }
    fn input() -> FrameInput {
        FrameInput {
            dt: 1. / 120.,
            head: Some(Mat4::from_translation(Vec3::new(0., 1.55, 0.))),
            ..Default::default()
        }
    }
    #[test]
    fn worker_owns_flight_and_tracking_pause_off_render_thread() {
        let (scene, game) = fixture();
        let mut sim = Simulation::new(scene, game).unwrap();
        sim.submit(input()).unwrap();
        let first = sim.wait_replay().unwrap();
        assert_ne!(first.execution_thread, std::thread::current().id());
        let id = first.state["flying"][0].as_u64().unwrap() as u32;
        let mut mirror = SceneManager::default();
        mirror.apply_render_publication(first.publication, &[]);
        let position = mirror.objects()[&id].transform.position;
        sim.submit(FrameInput { dt: 0.1, ..input() }).unwrap();
        let second = sim.wait_replay().unwrap();
        let remaining = second.state["round"]["remaining"].as_f64().unwrap();
        mirror.apply_render_publication(second.publication, &[]);
        let moved = mirror.objects()[&id].transform.position;
        assert_ne!(position, moved);
        sim.submit(FrameInput {
            dt: 0.5,
            head: None,
            ..input()
        })
        .unwrap();
        let paused = sim.wait_replay().unwrap();
        assert_eq!(
            paused.state["round"]["remaining"].as_f64().unwrap(),
            remaining
        );
        mirror.apply_render_publication(paused.publication, &[]);
        assert_eq!(mirror.objects()[&id].transform.position, moved);
        mirror.update(1.);
        assert_eq!(
            mirror.objects()[&id].transform.position,
            moved,
            "render mirror cannot simulate"
        );
    }
    #[test]
    fn worker_real_sword_cut_scores_and_emits_haptic_off_xr() {
        let (mut scene, mut game) = fixture();
        game.start(&mut scene).unwrap();
        game.spawn(
            &mut scene,
            &[Recipe {
                fruit: "watermelon".into(),
                scale: 0.126,
                position: [0.45, 1.3, -1.],
                velocity: [0.; 3],
                gravity: 0.,
                lifetime: 5.,
            }],
            false,
        )
        .unwrap();
        let mut sim = Simulation::new(scene, game).unwrap();
        let mut cuts = 0;
        let mut haptic: f32 = 0.;
        for step in 0..25 {
            let mut frame = input();
            frame.hands[1] = Some(Mat4::from_translation(Vec3::new(
                0.45,
                1. + step as f32 * 0.025,
                -0.45,
            )));
            sim.submit(frame).unwrap();
            let snapshot = sim.wait_replay().unwrap();
            assert_ne!(snapshot.execution_thread, std::thread::current().id());
            cuts = snapshot.state["cuts"].as_u64().unwrap();
            haptic = haptic.max(sim.take_haptics()[1]);
            if cuts > 0 {
                assert_eq!(snapshot.state["score"], 10);
                break;
            }
        }
        assert_eq!(cuts, 1);
        assert!(haptic > 0.);
    }
    #[test]
    fn xr_submission_and_poll_never_wait_for_busy_publication() {
        let (scene, game) = fixture();
        let mut sim = Simulation::new(scene, game).unwrap();
        let output = sim.output.clone();
        let guard = output.lock().unwrap();
        // Worker is deliberately prevented from publishing. All calls still
        // complete; bounded queue batches instead of waiting or growing.
        for _ in 0..64 {
            sim.submit(input()).unwrap();
            assert!(sim.poll().unwrap().is_none());
        }
        assert!(sim.pending.is_some());
        assert!(sim.batched > 0);
        drop(guard);
        let snapshot = sim.wait_replay().unwrap();
        assert_eq!(snapshot.sequence, 64);
    }
    #[test]
    fn queued_pause_and_primary_release_press_boundaries_survive_batching() {
        let (scene, game) = fixture();
        let mut sim = Simulation::new(scene, game).unwrap();
        let output = sim.output.clone();
        let guard = output.lock().unwrap();
        for _ in 0..16 {
            sim.submit(input()).unwrap();
        }
        sim.submit(FrameInput {
            primary: true,
            ..input()
        })
        .unwrap();
        sim.submit(input()).unwrap(); // Release must not be merged with press.
        sim.submit(FrameInput {
            primary: true,
            ..input()
        })
        .unwrap();
        sim.submit(FrameInput { dt: 0., ..input() }).unwrap(); // Head still valid while unfocused.
        drop(guard);
        let snapshot = sim.wait_replay().unwrap();
        let remaining = snapshot.state["round"]["remaining"].as_f64().unwrap();
        assert!(
            (remaining - (60. - 1. / 120.)).abs() < 0.00001,
            "last press restarts; pause advances no time: {remaining}"
        );
        assert_eq!(sim.dropped, 0);
    }
}
