//! Quest output is owned by one blocking-write thread, never the XR/render thread.
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
#[cfg(target_os = "android")]
use std::{
    sync::mpsc::{self, SyncSender},
    thread::JoinHandle,
    time::Instant,
};
use webgpu_wasm::{
    audio::{AudioEvent, AudioPolicy, AudioSnapshot},
    audio_mixer::EventCursor,
};

/// First activation accepts fresh startup cues; subsequent resumes baseline retained history.
#[derive(Default)]
struct SessionGate {
    active: bool,
    ever_active: bool,
    startup: bool,
    baseline: bool,
}
impl SessionGate {
    fn set_active(&mut self, active: bool) -> bool {
        if self.active == active {
            return false;
        }
        self.active = active;
        if active {
            self.startup = !self.ever_active;
            self.baseline = self.ever_active;
            self.ever_active = true;
        }
        true
    }
    fn prepare(&mut self, cursor: &mut EventCursor, snapshot: &mut AudioSnapshot) -> bool {
        if !self.active || self.baseline {
            cursor.baseline(snapshot);
            self.baseline = false;
            return false;
        }
        if self.startup {
            self.startup = false;
            let latest = snapshot.latest;
            snapshot.latest = 0;
            cursor.baseline(snapshot);
            snapshot.latest = latest;
        }
        true
    }
}

pub struct Audio {
    cursor: EventCursor,
    session: SessionGate,
    epoch: Option<u32>,
    policy: Option<AudioPolicy>,
    running: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    #[cfg(target_os = "android")]
    stop: Arc<AtomicBool>,
    #[cfg(target_os = "android")]
    sender: SyncSender<Command>,
    #[cfg(target_os = "android")]
    thread: Option<JoinHandle<()>>,
}
#[cfg(target_os = "android")]
enum Command {
    Play(AudioEvent, Instant, u64),
    Listener([f32; 16]),
    Policy(AudioPolicy),
}
impl Audio {
    pub fn new() -> Self {
        let running = Arc::new(AtomicBool::new(false));
        let generation = Arc::new(AtomicU64::new(0));
        #[cfg(target_os = "android")]
        {
            let stop = Arc::new(AtomicBool::new(false));
            let (sender, receiver) = mpsc::sync_channel(64);
            let (run, quit, epoch) = (running.clone(), stop.clone(), generation.clone());
            let thread = std::thread::Builder::new()
                .name("neon-audio".into())
                .spawn(move || {
                    if let Err(error) = android::run(receiver, run, quit, epoch) {
                        log::error!("Neon audio disabled: {error}");
                    }
                });
            let thread = match thread {
                Ok(t) => Some(t),
                Err(e) => {
                    log::error!("Neon audio worker unavailable: {e}");
                    None
                }
            };
            Self {
                cursor: EventCursor::default(),
                session: SessionGate::default(),
                epoch: None,
                policy: None,
                running,
                generation,
                stop,
                sender,
                thread,
            }
        }
        #[cfg(not(target_os = "android"))]
        {
            log::info!(
                "Neon audio output disabled: AAudio requires Android (PCM mixer remains available)"
            );
            Self {
                cursor: EventCursor::default(),
                session: SessionGate::default(),
                epoch: None,
                policy: None,
                running,
                generation,
            }
        }
    }
    pub fn set_active(&mut self, active: bool) {
        if self.session.set_active(active) {
            self.generation.fetch_add(1, Ordering::AcqRel);
            self.running.store(active, Ordering::Release);
        }
    }
    pub fn listener(&self, matrix: [f32; 16]) {
        #[cfg(target_os = "android")]
        let _ = self.sender.try_send(Command::Listener(matrix));
        #[cfg(not(target_os = "android"))]
        let _ = matrix;
    }
    pub fn snapshot(&mut self, mut snapshot: AudioSnapshot, publication_age: f64) {
        if self.epoch != Some(snapshot.epoch) {
            self.epoch = Some(snapshot.epoch);
            self.generation.fetch_add(1, Ordering::AcqRel);
        }
        if self.policy.as_ref() != Some(&snapshot.policy) {
            #[cfg(target_os = "android")]
            if self
                .sender
                .try_send(Command::Policy(snapshot.policy.clone()))
                .is_ok()
            {
                self.policy = Some(snapshot.policy.clone());
            }
            #[cfg(not(target_os = "android"))]
            {
                self.policy = Some(snapshot.policy.clone());
            }
        }
        if !self.session.prepare(&mut self.cursor, &mut snapshot) {
            return;
        }
        // The snapshot clock predates local arrival by the worker publication age.
        if !publication_age.is_finite() || publication_age < 0. {
            return;
        }
        snapshot.clock += publication_age;
        let _generation = self.generation.load(Ordering::Acquire);
        let _sender = ();
        #[cfg(target_os = "android")]
        let _sender = &self.sender;
        let _clock = snapshot.clock;
        self.cursor.consume(&snapshot, |event: &AudioEvent| {
            #[cfg(target_os = "android")]
            let _ = _sender.try_send(Command::Play(
                event.clone(),
                Instant::now()
                    + std::time::Duration::from_secs_f64(
                        (0.25 - (_clock - event.at)).clamp(0., 0.25),
                    ),
                _generation,
            ));
            #[cfg(not(target_os = "android"))]
            let _ = event;
        });
    }
}
impl Drop for Audio {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        #[cfg(target_os = "android")]
        {
            self.stop.store(true, Ordering::Release);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

#[cfg(target_os = "android")]
mod android {
    use super::*;
    use std::{ffi::c_void, ptr, sync::mpsc::Receiver, time::Duration};
    use webgpu_wasm::audio_mixer::{Clip, Mixer, SAMPLE_RATE};
    #[repr(C)]
    struct Builder {
        _opaque: [u8; 0],
    }
    #[repr(C)]
    struct Stream {
        _opaque: [u8; 0],
    }
    // Signatures/constants checked against NDK 27.2 aaudio/AAudio.h (API 26+).
    #[link(name = "aaudio")]
    unsafe extern "C" {
        fn AAudio_createStreamBuilder(builder: *mut *mut Builder) -> i32;
        fn AAudioStreamBuilder_delete(builder: *mut Builder) -> i32;
        fn AAudioStreamBuilder_setDirection(builder: *mut Builder, value: i32);
        fn AAudioStreamBuilder_setFormat(builder: *mut Builder, value: i32);
        fn AAudioStreamBuilder_setChannelCount(builder: *mut Builder, value: i32);
        fn AAudioStreamBuilder_setSampleRate(builder: *mut Builder, value: i32);
        fn AAudioStreamBuilder_setPerformanceMode(builder: *mut Builder, value: i32);
        fn AAudioStreamBuilder_setUsage(builder: *mut Builder, value: i32);
        fn AAudioStreamBuilder_openStream(builder: *mut Builder, stream: *mut *mut Stream) -> i32;
        fn AAudioStream_getFormat(stream: *mut Stream) -> i32;
        fn AAudioStream_getChannelCount(stream: *mut Stream) -> i32;
        fn AAudioStream_getSampleRate(stream: *mut Stream) -> i32;
        fn AAudioStream_requestStart(stream: *mut Stream) -> i32;
        fn AAudioStream_write(
            stream: *mut Stream,
            data: *const c_void,
            frames: i32,
            timeout: i64,
        ) -> i32;
        fn AAudioStream_close(stream: *mut Stream) -> i32;
    }
    struct Output(*mut Stream);
    impl Output {
        fn open() -> Result<Self, String> {
            unsafe {
                let mut builder = ptr::null_mut();
                let result = AAudio_createStreamBuilder(&mut builder);
                if result < 0 || builder.is_null() {
                    return Err(format!("AAudio builder: {result}"));
                }
                AAudioStreamBuilder_setDirection(builder, 0); // OUTPUT
                AAudioStreamBuilder_setFormat(builder, 2); // PCM_FLOAT
                AAudioStreamBuilder_setChannelCount(builder, 2);
                AAudioStreamBuilder_setSampleRate(builder, SAMPLE_RATE as i32);
                AAudioStreamBuilder_setPerformanceMode(builder, 12); // LOW_LATENCY
                AAudioStreamBuilder_setUsage(builder, 14); // GAME, API 28 (Quest min 29)
                let mut stream = ptr::null_mut();
                let result = AAudioStreamBuilder_openStream(builder, &mut stream);
                let deleted = AAudioStreamBuilder_delete(builder);
                if deleted < 0 {
                    log::warn!("AAudio builder delete: {deleted}");
                }
                if result < 0 || stream.is_null() {
                    return Err(format!("AAudio open: {result}"));
                }
                let output = Self(stream);
                if AAudioStream_getFormat(stream) != 2
                    || AAudioStream_getChannelCount(stream) != 2
                    || AAudioStream_getSampleRate(stream) != SAMPLE_RATE as i32
                {
                    return Err("AAudio negotiated unsupported format".into());
                }
                let result = AAudioStream_requestStart(stream);
                if result < 0 {
                    return Err(format!("AAudio start: {result}"));
                }
                log::info!("Neon AAudio stream started: 48000 Hz stereo float, blocking worker");
                Ok(output)
            }
        }
        fn write(
            &self,
            samples: &[f32],
            stop: &AtomicBool,
            running: &AtomicBool,
        ) -> Result<(), String> {
            let mut frames = 0;
            let deadline = Instant::now() + Duration::from_millis(40);
            while frames < samples.len() / 2 {
                if stop.load(Ordering::Acquire) || !running.load(Ordering::Acquire) {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err("AAudio block write deadline exceeded".into());
                }
                let result = unsafe {
                    AAudioStream_write(
                        self.0,
                        samples[frames * 2..].as_ptr().cast(),
                        (samples.len() / 2 - frames) as i32,
                        20_000_000,
                    )
                };
                if result <= 0 {
                    return Err(format!("AAudio write: {result}"));
                }
                if result as usize > samples.len() / 2 - frames {
                    return Err("AAudio invalid write length".into());
                }
                frames += result as usize;
            }
            Ok(())
        }
    }
    impl Drop for Output {
        fn drop(&mut self) {
            let result = unsafe { AAudioStream_close(self.0) };
            if result < 0 {
                log::warn!("AAudio close: {result}");
            }
        }
    }
    fn clips() -> Result<Vec<Clip>, String> {
        macro_rules! asset {
            ($name:literal) => {
                Clip::wav(
                    $name,
                    include_bytes!(concat!("../../../assets/audio/neon/", $name, ".wav")),
                )
                .map_err(|e| format!("{}: {}", $name, e))?
            };
        }
        Ok(vec![
            asset!("fruit_cut_soft_1"),
            asset!("fruit_cut_soft_2"),
            asset!("fruit_cut_soft_3"),
            asset!("fruit_cut_hard_1"),
            asset!("fruit_cut_hard_2"),
            asset!("fruit_cut_hard_3"),
            asset!("blade_swing_1"),
            asset!("blade_swing_2"),
            asset!("fruit_spawn"),
            asset!("fruit_miss"),
            asset!("combo_triple"),
            asset!("chain_bank"),
            asset!("countdown_tick"),
            asset!("round_start"),
            asset!("round_end"),
            asset!("ui_confirm"),
        ])
    }
    pub(super) fn run(
        receiver: Receiver<Command>,
        running: Arc<AtomicBool>,
        stop: Arc<AtomicBool>,
        generation: Arc<AtomicU64>,
    ) -> Result<(), String> {
        let mut mixer = Mixer::new(clips()?);
        let mut stream = None;
        let mut buffer = [0f32; 480]; // 5ms stereo block
        let mut last_generation = 0;
        let mut retry_at = Instant::now();
        let mut retry_delay = Duration::from_millis(100);
        while !stop.load(Ordering::Acquire) {
            let current = generation.load(Ordering::Acquire);
            if current != last_generation {
                mixer.clear();
                stream = None;
                last_generation = current;
                retry_at = Instant::now();
                retry_delay = Duration::from_millis(100);
            }
            for _ in 0..64 {
                match receiver.try_recv() {
                    Ok(Command::Listener(matrix)) => mixer.listener(matrix),
                    Ok(Command::Policy(policy)) => mixer.set_policy(&policy),
                    Ok(Command::Play(event, arrived, epoch)) => {
                        let current = generation.load(Ordering::Acquire);
                        if current != last_generation {
                            mixer.clear();
                            stream = None;
                            last_generation = current;
                            retry_at = Instant::now();
                            retry_delay = Duration::from_millis(100);
                        }
                        if Instant::now() >= retry_at
                            && running.load(Ordering::Acquire)
                            && epoch == current
                            && Instant::now() <= arrived
                        {
                            mixer.play(&event);
                        }
                    }
                    Err(_) => break,
                }
            }
            if !running.load(Ordering::Acquire) {
                mixer.clear();
                stream = None;
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            if Instant::now() < retry_at {
                mixer.clear();
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            if stream.is_none() {
                match Output::open() {
                    Ok(output) => stream = Some(output),
                    Err(error) => {
                        log::warn!(
                            "Neon AAudio open failed; retry in {}ms: {error}",
                            retry_delay.as_millis()
                        );
                        mixer.clear();
                        retry_at = Instant::now() + retry_delay;
                        retry_delay = (retry_delay * 2).min(Duration::from_secs(2));
                        continue;
                    }
                }
            }
            mixer.render(&mut buffer);
            if let Err(error) = stream.as_ref().unwrap().write(&buffer, &stop, &running) {
                log::warn!(
                    "Neon AAudio write failed; retry in {}ms: {error}",
                    retry_delay.as_millis()
                );
                stream = None;
                mixer.clear();
                retry_at = Instant::now() + retry_delay;
                retry_delay = (retry_delay * 2).min(Duration::from_secs(2));
            } else {
                retry_delay = Duration::from_millis(100);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_start_plays_fresh_cue_resume_suppresses_history() {
        let mut gate = SessionGate::default();
        let mut cursor = EventCursor::default();
        let mut snapshot = AudioSnapshot {
            epoch: 1,
            clock: 1.,
            latest: 1,
            policy: Default::default(),
            events: vec![AudioEvent {
                id: 1,
                at: 1.,
                clip: "round_start".into(),
                position: None,
                gain: 1.,
                pitch: 1.,
                priority: 1,
            }],
        };
        assert!(gate.set_active(true));
        let mut count = 0;
        assert!(gate.prepare(&mut cursor, &mut snapshot));
        cursor.consume(&snapshot, |_| count += 1);
        assert_eq!(count, 1);
        assert!(gate.set_active(false));
        snapshot.latest = 2;
        snapshot.events[0].id = 2;
        assert!(gate.set_active(true));
        assert!(!gate.prepare(&mut cursor, &mut snapshot));
        cursor.consume(&snapshot, |_| count += 1);
        assert_eq!(count, 1);
        snapshot.latest = 3;
        snapshot.events[0].id = 3;
        assert!(gate.prepare(&mut cursor, &mut snapshot));
        cursor.consume(&snapshot, |_| count += 1);
        assert_eq!(count, 2);
    }
}
