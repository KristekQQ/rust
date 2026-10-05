//! Browser frame-loop timing, not GPU execution timing.
#[derive(Default)]
pub struct FrameMetrics {
    elapsed: f64,
    frames: u32,
    pub fps: f64,
    pub frame_ms: f64,
}
impl FrameMetrics {
    pub fn record(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        // A background-tab pause is not a useful rendering sample.
        if dt > 0.25 {
            *self = Self::default();
            return;
        }
        self.elapsed += dt as f64;
        self.frames += 1;
        if self.elapsed >= 0.5 {
            self.fps = self.frames as f64 / self.elapsed;
            self.frame_ms = self.elapsed * 1000.0 / self.frames as f64;
            self.elapsed = 0.0;
            self.frames = 0;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn averages_frame_intervals_and_discards_background_pauses() {
        let mut metrics = FrameMetrics::default();
        for _ in 0..60 {
            metrics.record(1.0 / 60.0);
        }
        assert!((metrics.fps - 60.0).abs() < 0.001);
        assert!((metrics.frame_ms - 1000.0 / 60.0).abs() < 0.001);
        metrics.record(f32::NAN);
        assert!(metrics.fps.is_finite());
        metrics.record(2.0);
        assert_eq!(metrics.fps, 0.0);
        for _ in 0..30 {
            metrics.record(1.0 / 30.0);
        }
        assert!((metrics.fps - 30.0).abs() < 0.001);
    }
}
