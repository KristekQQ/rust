//! Browser frame-loop timing, not GPU execution timing.
#[derive(Default)]
pub struct FrameMetrics {
    elapsed: f64,
    samples: std::collections::VecDeque<f64>,
    pub frame_p95_ms: f64,
    frames: u32,
    pub fps: f64,
    pub frame_ms: f64,
}
impl FrameMetrics {
    pub fn record(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        // Count slow sub-second frames too; discard longer stalls.
        if dt > 1.0 {
            *self = Self::default();
            return;
        }
        if self.samples.len() == 120 {
            self.samples.pop_front();
        }
        self.samples.push_back(dt as f64 * 1000.);
        self.elapsed += dt as f64;
        self.frames += 1;
        if self.elapsed >= 0.5 {
            let mut sorted: Vec<_> = self.samples.iter().copied().collect();
            sorted.sort_by(f64::total_cmp);
            self.frame_p95_ms = sorted[(sorted.len() as f64 * 0.95).ceil() as usize - 1];
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
        assert!((metrics.frame_p95_ms - 1000.0 / 60.0).abs() < 0.001);
        metrics.record(f32::NAN);
        assert!(metrics.fps.is_finite());
        metrics.record(2.0);
        assert_eq!(metrics.fps, 0.0);
        for _ in 0..30 {
            metrics.record(1.0 / 30.0);
        }
        assert!((metrics.fps - 30.0).abs() < 0.001);
    }
    #[test]
    fn slow_frames_still_report_fps_instead_of_zero() {
        let mut metrics = FrameMetrics::default();
        metrics.record(0.35);
        metrics.record(0.35);
        assert!((metrics.fps - 1.0 / 0.35).abs() < 0.001);
        assert!((metrics.frame_ms - 350.0).abs() < 0.001);
    }
}
