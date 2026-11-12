//! Platform-neutral, bounded sound journal. Gameplay is the only event authority.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioEvent {
    pub id: u64,
    pub at: f64,
    pub clip: String,
    pub position: Option<[f32; 3]>,
    pub gain: f32,
    pub pitch: f32,
    pub priority: u8,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioGroupPolicy {
    pub prefix: String,
    pub limit: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioPolicy {
    pub max_voices: usize,
    pub master: f32,
    pub groups: Vec<AudioGroupPolicy>,
}
impl Default for AudioPolicy {
    fn default() -> Self {
        Self {
            max_voices: 24,
            master: 0.6,
            groups: Vec::new(),
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSnapshot {
    #[serde(default)]
    pub policy: AudioPolicy,
    pub epoch: u32,
    pub clock: f64,
    pub latest: u64,
    pub events: Vec<AudioEvent>,
}
/// Retain events across coalesced publications, but never accumulate an unbounded queue.
/// Epoch changes on round restart. IDs are monotonic within each epoch.
pub struct AudioScene {
    snapshot: AudioSnapshot,
    pub dropped: u64,
}
impl Default for AudioScene {
    fn default() -> Self {
        Self {
            snapshot: AudioSnapshot {
                events: Vec::with_capacity(64),
                ..Default::default()
            },
            dropped: 0,
        }
    }
}
impl AudioScene {
    pub fn set_policy(&mut self, policy: AudioPolicy) {
        self.snapshot.policy = policy;
    }
    pub fn snapshot(&self) -> &AudioSnapshot {
        &self.snapshot
    }
    pub fn clock(&self) -> f64 {
        self.snapshot.clock
    }
    pub fn restart(&mut self) {
        self.snapshot.epoch = self.snapshot.epoch.wrapping_add(1);
        self.snapshot.latest = 0;
        self.snapshot.clock = 0.;
        self.snapshot.events.clear();
    }
    pub fn advance(&mut self, dt: f32) {
        if dt.is_finite() && dt > 0. {
            self.snapshot.clock += f64::from(dt.min(0.1));
            let oldest = self.snapshot.clock - 1.;
            self.snapshot.events.retain(|e| e.at >= oldest);
        }
    }
    pub fn emit(
        &mut self,
        clip: &str,
        position: Option<[f32; 3]>,
        gain: f32,
        pitch: f32,
        priority: u8,
    ) {
        if clip.is_empty()
            || clip.len() > 64
            || !gain.is_finite()
            || gain <= 0.
            || !pitch.is_finite()
            || position.is_some_and(|p| !p.into_iter().all(f32::is_finite))
        {
            return;
        }
        if self.snapshot.events.len() == 64 {
            self.snapshot.events.remove(0);
            self.dropped += 1;
        }
        self.snapshot.latest += 1;
        self.snapshot.events.push(AudioEvent {
            id: self.snapshot.latest,
            at: self.snapshot.clock,
            clip: clip.into(),
            position,
            gain: gain.clamp(0., 1.),
            pitch: pitch.clamp(0.5, 2.),
            priority,
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn journal_survives_coalescing_and_expires() {
        let mut a = AudioScene::default();
        a.restart();
        for _ in 0..5 {
            a.advance(0.01);
            a.emit("hit", None, 0.5, 1., 1);
        }
        assert_eq!(a.snapshot().events.len(), 5);
        for _ in 0..11 {
            a.advance(0.1);
        }
        assert!(a.snapshot().events.is_empty());
        assert_eq!(a.snapshot().latest, 5);
    }
    #[test]
    fn bounded_restart_and_invalid_values() {
        let mut a = AudioScene::default();
        for _ in 0..100 {
            a.emit("hit", None, 0.5, 1., 1);
        }
        assert_eq!(a.snapshot().events.len(), 64);
        assert_eq!(a.dropped, 36);
        a.emit("bad", Some([f32::NAN, 0., 0.]), 1., 1., 1);
        assert_eq!(a.snapshot().latest, 100);
        a.restart();
        assert_eq!(a.snapshot().epoch, 1);
        assert_eq!(a.snapshot().latest, 0);
        assert!(a.snapshot().events.is_empty());
    }
}
