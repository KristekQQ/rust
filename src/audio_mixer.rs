//! Bounded mono-PCM voice mixer shared by native outputs. No render-time allocation.
use crate::audio::{AudioEvent, AudioPolicy, AudioSnapshot};
pub const SAMPLE_RATE: u32 = 48_000;
pub const MAX_VOICES: usize = 24;

pub struct Clip {
    pub name: &'static str,
    pub samples: Vec<f32>,
}
impl Clip {
    /// Accept only bounded, mono PCM16 48kHz RIFF/WAVE assets.
    pub fn wav(name: &'static str, bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return Err("invalid WAV header");
        }
        let mut offset = 12usize;
        let mut format = false;
        let mut data = None;
        while offset + 8 <= bytes.len() {
            let size =
                u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
            let begin = offset + 8;
            let end = begin.checked_add(size).ok_or("WAV overflow")?;
            if end > bytes.len() {
                return Err("truncated WAV");
            }
            match &bytes[offset..offset + 4] {
                b"fmt " => {
                    let f = &bytes[begin..end];
                    format = f.len() >= 16
                        && f[0..2] == [1, 0]
                        && f[2..4] == [1, 0]
                        && u32::from_le_bytes(f[4..8].try_into().unwrap()) == SAMPLE_RATE
                        && f[12..14] == [2, 0]
                        && f[14..16] == [16, 0];
                }
                b"data" => data = Some(&bytes[begin..end]),
                _ => (),
            }
            offset = end.checked_add(size & 1).ok_or("WAV overflow")?;
        }
        let data = data.ok_or("missing WAV samples")?;
        if !format
            || data.is_empty()
            || data.len() % 2 != 0
            || data.len() > SAMPLE_RATE as usize * 2 * 10
        {
            return Err("unsupported WAV format/length");
        }
        Ok(Self {
            name,
            samples: data
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.)
                .collect(),
        })
    }
}
#[derive(Clone, Copy)]
struct Voice {
    clip: usize,
    cursor: f64,
    pitch: f64,
    gain: f32,
    position: Option<[f32; 3]>,
    priority: u8,
}
pub struct Mixer {
    clips: Vec<Clip>,
    voices: [Option<Voice>; MAX_VOICES],
    listener: [f32; 3],
    right: [f32; 3],
    groups: Vec<(usize, usize)>,
    max_voices: usize,
    master: f32,
}
impl Mixer {
    pub fn new(clips: Vec<Clip>) -> Self {
        let groups = (0..clips.len()).map(|i| (i, 4)).collect();
        Self {
            clips,
            groups,
            max_voices: MAX_VOICES,
            master: 0.6,
            voices: [None; MAX_VOICES],
            listener: [0.; 3],
            right: [1., 0., 0.],
        }
    }
    pub fn set_policy(&mut self, policy: &AudioPolicy) {
        self.max_voices = policy.max_voices.clamp(1, MAX_VOICES);
        self.master = if policy.master.is_finite() {
            policy.master.clamp(0., 1.)
        } else {
            0.
        };
        for (i, clip) in self.clips.iter().enumerate() {
            self.groups[i] = policy
                .groups
                .iter()
                .take(32)
                .enumerate()
                .find(|(_, g)| clip.name.starts_with(&g.prefix))
                .map(|(j, g)| (j, g.limit.clamp(1, MAX_VOICES)))
                .unwrap_or((policy.groups.len() + i, 4));
        }
        self.clear();
    }
    pub fn clear(&mut self) {
        self.voices.fill(None);
    }
    pub fn active_voices(&self) -> usize {
        self.voices.iter().flatten().count()
    }
    /// Column-major head world transform: local +X is the listener's right.
    pub fn listener(&mut self, head: [f32; 16]) {
        if !head.iter().all(|x| x.is_finite()) {
            return;
        }
        let len = (head[0] * head[0] + head[1] * head[1] + head[2] * head[2]).sqrt();
        if len < 0.001 {
            return;
        }
        self.listener = [head[12], head[13], head[14]];
        self.right = [head[0] / len, head[1] / len, head[2] / len];
    }
    pub fn play(&mut self, event: &AudioEvent) -> bool {
        if !event.at.is_finite()
            || !event.gain.is_finite()
            || !event.pitch.is_finite()
            || event.gain <= 0.
            || !(0.5..=2.).contains(&event.pitch)
            || event
                .position
                .is_some_and(|p| !p.iter().all(|x| x.is_finite()))
        {
            return false;
        }
        let Some(clip) = self
            .clips
            .iter()
            .position(|c| c.name == event.clip && !c.samples.is_empty())
        else {
            return false;
        };
        let same = self
            .voices
            .iter()
            .flatten()
            .filter(|v| self.groups[v.clip].0 == self.groups[clip].0)
            .count();
        let slot = if same >= self.groups[clip].1 {
            self.voices
                .iter()
                .enumerate()
                .filter_map(|(i, v)| {
                    v.filter(|v| {
                        self.groups[v.clip].0 == self.groups[clip].0 && v.priority <= event.priority
                    })
                    .map(|v| (i, v))
                })
                .max_by(|a, b| a.1.cursor.total_cmp(&b.1.cursor))
                .map(|x| x.0)
        } else {
            self.voices[..self.max_voices]
                .iter()
                .position(Option::is_none)
                .or_else(|| {
                    self.voices[..self.max_voices]
                        .iter()
                        .enumerate()
                        .filter_map(|(i, v)| v.map(|v| (i, v)))
                        .filter(|(_, v)| v.priority < event.priority)
                        .min_by_key(|(_, v)| v.priority)
                        .map(|x| x.0)
                })
        };
        let Some(slot) = slot else {
            return false;
        };
        self.voices[slot] = Some(Voice {
            clip,
            cursor: 0.,
            pitch: event.pitch as f64,
            gain: event.gain.clamp(0., 1.),
            position: event.position,
            priority: event.priority,
        });
        true
    }
    /// Interleaved stereo float output, fixed headroom and final bounded limiter.
    pub fn render(&mut self, output: &mut [f32]) {
        output.fill(0.);
        for slot in &mut self.voices {
            let Some(voice) = slot.as_mut() else {
                continue;
            };
            let (pan, attenuation) = if let Some(position) = voice.position {
                let d = std::array::from_fn::<_, 3, _>(|i| position[i] - self.listener[i]);
                let distance = (d.iter().map(|x| x * x).sum::<f32>()).sqrt();
                if !distance.is_finite() {
                    *slot = None;
                    continue;
                }
                let pan = if distance > 0.001 {
                    d.iter().zip(self.right).map(|(d, r)| d * r).sum::<f32>() / distance
                } else {
                    0.
                };
                (pan.clamp(-1., 1.), 1. / distance.max(1.))
            } else {
                (0., 1.)
            };
            let angle = (pan + 1.) * std::f32::consts::FRAC_PI_4;
            let gains = [
                angle.cos() * voice.gain * attenuation * self.master,
                angle.sin() * voice.gain * attenuation * self.master,
            ];
            let samples = &self.clips[voice.clip].samples;
            for frame in output.as_chunks_mut::<2>().0 {
                let i = voice.cursor as usize;
                if i >= samples.len() {
                    break;
                }
                let fraction = (voice.cursor - i as f64) as f32;
                let s = samples[i]
                    + (samples.get(i + 1).copied().unwrap_or(0.) - samples[i]) * fraction;
                frame[0] += s * gains[0];
                frame[1] += s * gains[1];
                voice.cursor += voice.pitch;
            }
            if voice.cursor >= samples.len() as f64 {
                *slot = None;
            }
        }
        for sample in output {
            *sample = if sample.is_finite() {
                sample.clamp(-1., 1.)
            } else {
                0.
            };
        }
    }
}

/// Snapshot cursor advances even when output is muted/full; retained history never replays.
#[derive(Default)]
pub struct EventCursor {
    epoch: Option<u32>,
    latest: u64,
}
impl EventCursor {
    pub fn baseline(&mut self, snapshot: &AudioSnapshot) {
        self.epoch = Some(snapshot.epoch);
        self.latest = snapshot.latest;
    }
    pub fn consume(&mut self, snapshot: &AudioSnapshot, mut play: impl FnMut(&AudioEvent)) {
        if !snapshot.clock.is_finite() {
            return;
        }
        if self.epoch.is_none() {
            self.baseline(snapshot);
            return;
        }
        if self.epoch != Some(snapshot.epoch) {
            self.epoch = Some(snapshot.epoch);
            self.latest = 0;
        }
        for event in snapshot.events.iter().take(128) {
            if event.id <= self.latest || event.id > snapshot.latest {
                continue;
            }
            self.latest = event.id;
            let age = snapshot.clock - event.at;
            if age.is_finite() && (0. ..=0.25).contains(&age) {
                play(event);
            }
        }
        self.latest = self.latest.max(snapshot.latest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event() -> AudioEvent {
        AudioEvent {
            id: 1,
            at: 1.,
            clip: "test".into(),
            position: None,
            gain: 1.,
            pitch: 1.,
            priority: 1,
        }
    }
    fn mixer() -> Mixer {
        Mixer::new(vec![Clip {
            name: "test",
            samples: vec![0.8; 480],
        }])
    }
    #[test]
    fn actual_pcm_pan_and_rotation() {
        let mut m = mixer();
        let mut e = event();
        e.position = Some([1., 0., 0.]);
        m.play(&e);
        let mut out = [0.; 16];
        m.render(&mut out);
        assert!(out[1] > 0.1 && out[0].abs() < 0.00001);
        m.clear();
        m.listener([
            -1., 0., 0., 0., 0., 1., 0., 0., 0., 0., -1., 0., 0., 0., 0., 1.,
        ]);
        m.play(&e);
        m.render(&mut out);
        assert!(out[0] > 0.1 && out[1].abs() < 0.00001);
    }
    #[test]
    fn bounded_and_malformed() {
        let mut m = mixer();
        m.set_policy(&AudioPolicy {
            groups: vec![crate::audio::AudioGroupPolicy {
                prefix: String::new(),
                limit: 24,
            }],
            ..Default::default()
        });
        let mut e = event();
        for _ in 0..100 {
            m.play(&e);
        }
        assert_eq!(m.active_voices(), MAX_VOICES);
        e.priority = 0;
        assert!(!m.play(&e));
        e.priority = 2;
        assert!(m.play(&e));
        e.gain = f32::NAN;
        assert!(!m.play(&e));
        e.gain = 1.;
        e.position = Some([f32::INFINITY, 0., 0.]);
        assert!(!m.play(&e));
        let mut out = [0.; 1024];
        m.render(&mut out);
        assert!(out.iter().all(|x| x.is_finite() && x.abs() <= 1.));
        assert_eq!(m.active_voices(), 0);
    }
    #[test]
    fn dedupe_age_and_resume_baseline() {
        let mut c = EventCursor::default();
        let mut s = AudioSnapshot {
            epoch: 1,
            policy: Default::default(),
            clock: 1.,
            latest: 1,
            events: vec![event()],
        };
        let mut n = 0;
        c.consume(&s, |_| n += 1);
        assert_eq!(n, 0);
        s.latest = 2;
        s.events[0].id = 2;
        c.consume(&s, |_| n += 1);
        c.consume(&s, |_| n += 1);
        assert_eq!(n, 1);
        s.latest = 3;
        s.events[0].id = 3;
        s.clock = 2.;
        c.consume(&s, |_| n += 1);
        assert_eq!(n, 1);
        c.baseline(&s);
        c.consume(&s, |_| n += 1);
        assert_eq!(n, 1);
        s.epoch = 2;
        s.clock = 1.;
        s.latest = 1;
        s.events[0].id = 1;
        c.consume(&s, |_| n += 1);
        assert_eq!(n, 2);
    }
    #[test]
    fn configurable_group_and_master_limits() {
        let mut m = mixer();
        m.set_policy(&AudioPolicy {
            max_voices: 8,
            master: 0.,
            groups: vec![crate::audio::AudioGroupPolicy {
                prefix: "test".into(),
                limit: 2,
            }],
        });
        for _ in 0..10 {
            m.play(&event());
        }
        assert_eq!(m.active_voices(), 2);
        let mut pcm = [1.; 32];
        m.render(&mut pcm);
        assert!(pcm.iter().all(|x| *x == 0.));
    }
    #[test]
    fn distance_attenuation_and_pitch_completion() {
        fn energy(distance: f32) -> f32 {
            let mut m = mixer();
            let mut e = event();
            e.position = Some([0., 0., -distance]);
            m.play(&e);
            let mut out = [0.; 64];
            m.render(&mut out);
            out.iter().map(|v| v * v).sum()
        }
        assert!(energy(1.) > energy(4.) * 10.);
        let mut fast = mixer();
        let mut slow = mixer();
        let mut e = event();
        e.pitch = 2.;
        fast.play(&e);
        e.pitch = 0.5;
        slow.play(&e);
        let mut out = [0.; 480];
        fast.render(&mut out);
        slow.render(&mut out);
        assert_eq!(fast.active_voices(), 0);
        assert_eq!(slow.active_voices(), 1);
        slow.render(&mut [0.; 1440]);
        assert_eq!(slow.active_voices(), 0);
    }
    #[test]
    fn malformed_clip_format_cannot_enter_bank() {
        let valid = include_bytes!("../assets/audio/neon/ui_confirm.wav");
        assert!(Clip::wav("valid", valid).is_ok());
        for (offset, value) in [(20, 3), (22, 2), (24, 1), (34, 8)] {
            let mut bytes = valid.to_vec();
            bytes[offset] = value;
            assert!(
                Clip::wav("invalid", &bytes).is_err(),
                "header offset {offset}"
            );
        }
        assert!(Clip::wav("truncated", &valid[..valid.len() - 1]).is_err());
    }
    #[test]
    fn truncated_wav_rejected() {
        assert!(Clip::wav("bad", b"RIFF1234WAVEdata\xff\xff\xff\xff").is_err());
    }
}
