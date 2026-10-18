/// Generic opt-in XR round timer, advanced once per platform frame.
#[derive(Clone, Copy)]
pub struct RoundClock {
    pub duration: f32,
    pub remaining: f32,
}
impl RoundClock {
    pub fn new(seconds: f32) -> Option<Self> {
        (seconds.is_finite() && (1.0..=3600.0).contains(&seconds)).then_some(Self {
            duration: seconds,
            remaining: seconds,
        })
    }
    pub fn advance(&mut self, dt: f32) {
        if dt.is_finite() && dt > 0.0 {
            self.remaining = (self.remaining - dt).max(0.0);
        }
    }
    pub fn finished(&self) -> bool {
        self.remaining <= 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_duration() {
        for seconds in [f32::NAN, f32::INFINITY, -1.0, 0.0, 0.99, 3600.1] {
            assert!(RoundClock::new(seconds).is_none());
        }
        for seconds in [1.0, 30.0, 3600.0] {
            let round = RoundClock::new(seconds).unwrap();
            assert_eq!(round.remaining, seconds);
            assert!(!round.finished());
        }
    }
    #[test]
    fn counts_frames_clamps_finish_and_restarts() {
        let mut round = RoundClock::new(30.0).unwrap();
        round.advance(0.25);
        assert_eq!(round.remaining, 29.75);
        for dt in [-1.0, f32::NAN, f32::INFINITY] {
            round.advance(dt);
        }
        assert_eq!(round.remaining, 29.75);
        round.advance(29.75);
        assert!(round.finished());
        round.advance(10.0);
        assert_eq!(round.remaining, 0.0);
        let restarted = RoundClock::new(30.0).unwrap();
        assert_eq!(restarted.duration, 30.0);
        assert_eq!(restarted.remaining, 30.0);
        assert!(!restarted.finished());
    }
}
