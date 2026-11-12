//! Bounded optional frame capture. Serialization and disk access belong to the writer.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
    sync::mpsc::{sync_channel, SyncSender},
    thread::{self, JoinHandle},
};

pub const MAX_FRAMES: usize = 60_000;
pub const STAGE_NAMES: [&str; 10] = [
    "wait_frame",
    "begin_tracking_actions",
    "update",
    "haptics",
    "acquire_wait",
    "left_encode_submit",
    "right_encode_submit",
    "transition_poll_release",
    "end_frame",
    "post_end_bookkeeping",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub run_id: String,
    pub warmup_s: f64,
    pub duration_s: f64,
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.run_id.is_empty()
            || self.run_id.len() > 64
            || !self
                .run_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(
                "run_id must contain 1..64 ASCII letters, digits, underscores or hyphens".into(),
            );
        }
        if !self.warmup_s.is_finite() || !(0.0..=120.0).contains(&self.warmup_s) {
            return Err("warmup_s must be finite and within 0..120".into());
        }
        if !self.duration_s.is_finite() || !(1.0..=300.0).contains(&self.duration_s) {
            return Err("duration_s must be finite and within 1..300".into());
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct Setup {
    pub config: Config,
    pub output_dir: PathBuf,
}
#[derive(Clone, Default, Serialize)]
pub struct ReplayMetrics {
    pub active: bool,
    pub phase: &'static str,
    pub cutting: bool,
    pub elapsed_s: f32,
    pub head_world: [[f32; 4]; 4],
}
#[derive(Clone, Default, Serialize)]
pub struct AppMetrics {
    pub replay: Option<ReplayMetrics>,
    pub input_sequence: u64,
    pub applied_sequence: u64,
    pub worker_ms: f64,
    pub simulation_age_ms: f64,
    pub publication_age_ms: f64,
    pub worker_samples: u32,
    pub worker_queue_ms: f64,
    pub worker_game_ms: f64,
    pub worker_publication_ms: f64,
    pub worker_retirement_ms: f64,
    pub draws: u64,
    pub upload_bytes: u64,
    pub cuts: u64,
    pub shots: u64,
    pub cue_index: u64,
    pub flying: u32,
    pub fragments: u32,
    pub batched: u64,
    pub dropped: u64,
}
#[derive(Serialize)]
pub struct FrameRecord {
    pub frame_id: u64,
    pub monotonic_ns: u64,
    pub predicted_ns: i64,
    pub period_ns: i64,
    pub display_hz: Option<f32>,
    pub should_render: bool,
    pub focused: bool,
    pub tracking_valid: bool,
    pub stages_ns: [u64; 10],
    pub app_interval_ns: Option<u64>,
    pub app: AppMetrics,
}
#[derive(Serialize)]
struct ProfileSnapshot {
    frame_id: u64,
    data: serde_json::Value,
}
#[derive(Serialize)]
struct Run {
    schema: &'static str,
    config: Config,
    complete: bool,
    valid: bool,
    reason: &'static str,
    stage_names: [&'static str; 10],
    warmup_anchor_ns: Option<u64>,
    measurement_start_ns: Option<u64>,
    end_ns: Option<u64>,
    frames: Vec<FrameRecord>,
    profile_kind: &'static str,
    profiles: Vec<ProfileSnapshot>,
}

pub struct Recorder {
    run: Option<Run>,
    warmup_ns: u64,
    duration_ns: u64,
    profile_limit: usize,
    last_ns: Option<u64>,
    sender: Option<SyncSender<Run>>,
    writer: Option<JoinHandle<()>>,
}
impl Recorder {
    pub fn new(setup: Setup) -> Result<Self, String> {
        setup.config.validate()?;
        // Reserve this run before rendering. A second recorder cannot overwrite it.
        fs::create_dir_all(&setup.output_dir).map_err(|e| e.to_string())?;
        let result = setup
            .output_dir
            .join(format!("{}.json", setup.config.run_id));
        if result.exists() {
            return Err(format!("result already exists: {}", result.display()));
        }
        let lock = setup
            .output_dir
            .join(format!(".{}.lock", setup.config.run_id));
        let reservation = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
            .map_err(|e| format!("cannot reserve run: {e}"))?;
        if result.exists() {
            let _ = fs::remove_file(&lock);
            return Err("result already exists".into());
        }
        let mut recorder = Self::buffer(setup.config);
        let (sender, receiver) = sync_channel::<Run>(1);
        let cleanup_lock = lock.clone();
        let writer = thread::Builder::new()
            .name("frame-measurement-writer".into())
            .spawn(move || {
                let _reservation = reservation;
                if let Ok(run) = receiver.recv() {
                    match write_run(&result, &run) {
                        Ok(()) => log::info!(
                            "frame measurement exported: {} (complete={}, valid={}, reason={})",
                            result.display(),
                            run.complete,
                            run.valid,
                            run.reason
                        ),
                        Err(error) => log::error!("frame measurement export failed: {error}"),
                    }
                }
                let _ = fs::remove_file(lock);
            })
            .map_err(|e| {
                let _ = fs::remove_file(cleanup_lock);
                e.to_string()
            })?;
        recorder.sender = Some(sender);
        recorder.writer = Some(writer);
        Ok(recorder)
    }
    fn buffer(config: Config) -> Self {
        let warmup_ns = (config.warmup_s * 1e9) as u64;
        let duration_ns = (config.duration_s * 1e9) as u64;
        let profile_limit = (config.warmup_s + config.duration_s).ceil() as usize + 10;
        Self {
            run: Some(Run {
                schema: "native-frame-measurement-v1",
                config,
                complete: false,
                valid: false,
                reason: "in_progress",
                stage_names: STAGE_NAMES,
                warmup_anchor_ns: None,
                measurement_start_ns: None,
                end_ns: None,
                frames: Vec::with_capacity(MAX_FRAMES),
                profile_kind: "sparse diagnostic snapshots; not all GPU samples",
                profiles: Vec::with_capacity(profile_limit),
            }),
            warmup_ns,
            duration_ns,
            profile_limit,
            last_ns: None,
            sender: None,
            writer: None,
        }
    }
    pub fn record(&mut self, record: FrameRecord) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        if self.last_ns.is_some_and(|last| record.monotonic_ns < last) {
            self.finish(false, "non_monotonic_timestamp");
            return;
        }
        self.last_ns = Some(record.monotonic_ns);
        if run.warmup_anchor_ns.is_none() {
            // A scripted workload first waits for a stable native session.
            // Startup focus gaps must not consume its measurement window.
            if record
                .app
                .replay
                .as_ref()
                .is_some_and(|replay| !replay.active)
            {
                return;
            }
            if !(record.focused && record.tracking_valid && record.should_render) {
                return;
            }
            run.warmup_anchor_ns = Some(record.monotonic_ns);
        }
        let start = run.warmup_anchor_ns.unwrap().saturating_add(self.warmup_ns);
        if record.monotonic_ns < start {
            return;
        }
        // Use the requested timeline boundary, not the first frame after a stall.
        run.measurement_start_ns = Some(start);
        if run.frames.len() == MAX_FRAMES {
            self.finish(false, "frame_capacity_exceeded");
            return;
        }
        let now = record.monotonic_ns;
        run.end_ns = Some(now);
        run.frames.push(record);
        if now.saturating_sub(start) >= self.duration_ns {
            self.finish(true, "duration_elapsed");
        }
    }
    pub fn profile(&mut self, frame_id: u64, data: serde_json::Value) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        // Initial frame-zero setup metadata is valid before the warmup anchor.
        if run.profiles.len() == self.profile_limit {
            self.finish(false, "profile_capacity_exceeded");
            return;
        }
        run.profiles.push(ProfileSnapshot { frame_id, data });
    }
    fn finish(&mut self, complete: bool, reason: &'static str) {
        let Some(mut run) = self.run.take() else {
            return;
        };
        run.complete = complete;
        run.valid = complete;
        run.reason = reason;
        if let Some(sender) = self.sender.take() {
            // One preallocated channel slot, one export: this never waits for disk I/O.
            if sender.try_send(run).is_err() {
                log::error!("frame measurement writer unavailable");
            }
        }
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        if self
            .run
            .as_ref()
            .is_some_and(|r| r.warmup_anchor_ns.is_some())
        {
            self.finish(false, "recorder_dropped");
        }
        self.sender.take();
        if let Some(writer) = self.writer.take() {
            if writer.join().is_err() {
                log::error!("frame measurement writer panicked");
            }
        }
    }
}
fn write_run(result: &std::path::Path, run: &Run) -> Result<(), String> {
    let temporary = result.with_extension("json.tmp");
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let write = || -> Result<(), String> {
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, run).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
        writer.get_ref().sync_all().map_err(|e| e.to_string())?;
        if result.exists() {
            return Err("refusing existing result".into());
        }
        fs::rename(&temporary, result).map_err(|e| e.to_string())
    };
    let outcome = write();
    if outcome.is_err() {
        let _ = fs::remove_file(temporary);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(warmup_s: f64) -> Config {
        Config {
            run_id: "test-run_1".into(),
            warmup_s,
            duration_s: 2.0,
        }
    }
    fn frame(monotonic_ns: u64) -> FrameRecord {
        FrameRecord {
            frame_id: monotonic_ns,
            monotonic_ns,
            predicted_ns: 0,
            period_ns: 0,
            display_hz: None,
            should_render: true,
            focused: true,
            tracking_valid: true,
            stages_ns: [0; 10],
            app_interval_ns: None,
            app: AppMetrics::default(),
        }
    }
    fn collecting(warmup_s: f64) -> (Recorder, std::sync::mpsc::Receiver<Run>) {
        let mut r = Recorder::buffer(config(warmup_s));
        let (s, rx) = sync_channel(1);
        r.sender = Some(s);
        (r, rx)
    }

    #[test]
    fn scripted_workload_anchors_after_settling_and_keeps_finished_endpoint() {
        let (mut recorder, receiver) = collecting(0.);
        let mut settling = frame(0);
        settling.app.replay = Some(ReplayMetrics {
            phase: "settling",
            ..Default::default()
        });
        recorder.record(settling);
        assert!(recorder.run.as_ref().unwrap().warmup_anchor_ns.is_none());
        let mut playing = frame(50_000_000_000);
        playing.app.replay = Some(ReplayMetrics {
            active: true,
            phase: "playing",
            ..Default::default()
        });
        recorder.record(playing);
        let mut finished = frame(52_000_000_000);
        finished.app.replay = Some(ReplayMetrics {
            phase: "finished",
            ..Default::default()
        });
        recorder.record(finished);
        let run = receiver.recv().unwrap();
        assert!(run.complete && run.valid);
        assert_eq!(run.measurement_start_ns, Some(50_000_000_000));
        assert_eq!(run.frames.len(), 2);
    }
    #[test]
    fn config_safety() {
        assert!(config(120.0).validate().is_ok());
        for id in ["", "../bad", "a/b", "ž", "a.b"] {
            let mut c = config(0.0);
            c.run_id = id.into();
            assert!(c.validate().is_err());
        }
        let mut c = config(0.0);
        c.run_id = "a".repeat(65);
        assert!(c.validate().is_err());
        for n in [f64::NAN, f64::INFINITY, -1.0, 121.0] {
            assert!(config(n).validate().is_err());
        }
        for n in [f64::NAN, f64::INFINITY, 0.0, 301.0] {
            let mut c = config(0.0);
            c.duration_s = n;
            assert!(c.validate().is_err());
        }
    }
    #[test]
    fn warmup_anchor_and_gaps_preserved() {
        let (mut r, rx) = collecting(1.0);
        r.profile(0, serde_json::json!({"kind": "setup", "eye_width": 1024}));
        let mut invalid = frame(0);
        invalid.focused = false;
        r.record(invalid);
        assert!(r.run.as_ref().unwrap().warmup_anchor_ns.is_none());
        r.record(frame(100));
        r.record(frame(500_000_100));
        let mut gap = frame(1_000_000_100);
        gap.focused = false;
        gap.tracking_valid = false;
        gap.should_render = false;
        r.record(gap);
        r.record(frame(3_000_000_100));
        let run = rx.try_recv().unwrap();
        assert!(run.complete && run.valid);
        assert_eq!(run.measurement_start_ns, Some(1_000_000_100));
        assert_eq!(run.frames.len(), 2);
        assert_eq!(run.profiles.len(), 1);
        assert_eq!(run.profiles[0].frame_id, 0);
        assert_eq!(run.profiles[0].data["eye_width"], 1024);
        assert!(
            !run.frames[0].focused && !run.frames[0].tracking_valid && !run.frames[0].should_render
        );
    }
    #[test]
    fn long_stall_is_not_censored_or_reanchored() {
        let (mut r, rx) = collecting(1.0);
        r.record(frame(0));
        r.record(frame(10_000_000_000));
        let run = rx.try_recv().unwrap();
        assert!(run.complete);
        assert_eq!(run.frames.len(), 1);
        assert_eq!(run.measurement_start_ns, Some(1_000_000_000));
        assert_eq!(run.end_ns, Some(10_000_000_000));
    }
    #[test]
    fn overflow_exports_incomplete() {
        let (mut r, rx) = collecting(0.0);
        for n in 0..=MAX_FRAMES {
            r.record(frame(n as u64));
        }
        let run = rx.try_recv().unwrap();
        assert!(!run.complete && !run.valid);
        assert_eq!(run.reason, "frame_capacity_exceeded");
        assert_eq!(run.frames.len(), MAX_FRAMES);
    }
    #[test]
    fn drop_exports_partial_and_profile_overflow_invalidates() {
        let (mut r, rx) = collecting(0.0);
        r.record(frame(1));
        drop(r);
        let run = rx.try_recv().unwrap();
        assert!(!run.complete);
        assert_eq!(run.reason, "recorder_dropped");
        let (mut r, rx) = collecting(0.0);
        r.record(frame(1));
        for n in 0..=r.profile_limit {
            r.profile(n as u64, serde_json::Value::Null);
        }
        let run = rx.try_recv().unwrap();
        assert!(!run.valid);
        assert_eq!(run.reason, "profile_capacity_exceeded");
    }
    #[test]
    fn backward_time_invalidates_without_hiding_recorded_frames() {
        let (mut r, rx) = collecting(0.0);
        r.record(frame(100));
        r.record(frame(50));
        let run = rx.try_recv().unwrap();
        assert!(!run.valid);
        assert_eq!(run.reason, "non_monotonic_timestamp");
        assert_eq!(run.frames.len(), 1);
    }
    #[test]
    fn background_export_is_complete_before_drop_returns_and_refuses_reuse() {
        let directory =
            std::env::temp_dir().join(format!("native-frame-recorder-test-{}", std::process::id()));
        // Unique test name in this process; create_dir catches stale/interfering files.
        fs::create_dir(&directory).unwrap();
        let setup = Setup {
            config: config(0.0),
            output_dir: directory.clone(),
        };
        let mut r = Recorder::new(setup.clone()).unwrap();
        assert!(Recorder::new(setup.clone()).is_err());
        r.record(frame(10));
        drop(r);
        let result = directory.join("test-run_1.json");
        let exported: serde_json::Value =
            serde_json::from_slice(&fs::read(&result).unwrap()).unwrap();
        assert_eq!(exported["complete"], false);
        assert_eq!(exported["reason"], "recorder_dropped");
        assert_eq!(exported["frames"][0]["monotonic_ns"], 10);
        assert!(Recorder::new(setup).is_err());
        assert!(!directory.join("test-run_1.json.tmp").exists());
        fs::remove_file(result).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
