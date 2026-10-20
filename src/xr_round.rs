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

/// Optional object-tagged triples. Only original targets can award points;
/// fragments never inherit a target registration.
#[derive(Default)]
pub struct ComboScore {
    pub enabled: bool,
    pub score: u64,
    pub triples: u32,
    pub streak: u32,
    pub kind: String,
    pub targets: std::collections::BTreeMap<u32, (String, u32, bool)>,
    streak_points: u32,
    display_kind: String,
    display_count: u32,
}
impl ComboScore {
    pub fn register(&mut self, id: u32, kind: &str, points: u32, wild: bool) -> bool {
        if kind.is_empty()
            || kind.len() > 16
            || !kind.bytes().all(|c| c.is_ascii_alphanumeric() || c == b' ')
            || !(1..=1_000_000).contains(&points)
            || self
                .targets
                .values()
                .any(|(k, p, _)| k == kind && *p != points)
        {
            return false;
        }
        self.enabled = true;
        self.targets.insert(id, (kind.into(), points, wild));
        true
    }
    pub fn reset(&mut self) {
        *self = Self {
            enabled: self.enabled,
            targets: std::mem::take(&mut self.targets),
            ..Self::default()
        };
    }
    /// Kind and number of matching icons to highlight in the combo display.
    /// The completed triple remains visible until the next registered hit.
    pub fn display_progress(&self) -> (&str, u32) {
        (&self.display_kind, self.display_count)
    }
    pub fn hit(&mut self, id: u32) {
        self.hit_event(id);
    }
    pub fn hit_event(&mut self, id: u32) -> Option<ComboEvent> {
        let (kind, points, wild) = self.targets.remove(&id)?;
        let previous = (self.display_kind.clone(), self.streak);
        let mismatch = !wild && !self.kind.is_empty() && self.kind != kind;
        if self.streak == 0 {
            self.display_kind = kind.clone();
            self.display_count = 0;
        }
        if !wild {
            if !self.kind.is_empty() && self.kind != kind {
                self.streak = 0;
            }
            self.kind = kind;
            self.display_kind = self.kind.clone();
            self.streak_points = points;
        } else if self.kind.is_empty() {
            // Until ordinary fruit fixes the kind, an all-wild triple uses
            // the wildcard target's configured reward.
            self.streak_points = points;
        }
        self.streak += 1;
        self.display_count = self.streak;
        let completed = self.streak == 3;
        let award = self.streak_points;
        if completed {
            self.score = self.score.saturating_add(u64::from(self.streak_points));
            self.triples = self.triples.saturating_add(1);
            self.display_count = 3;
            if !self.kind.is_empty() {
                self.display_kind = self.kind.clone();
            }
            self.streak = 0;
            self.kind.clear();
            self.streak_points = 0;
        }
        Some(ComboEvent {
            failed: mismatch.then_some(previous),
            kind: self.display_kind.clone(),
            count: self.display_count,
            success: completed,
            points: if completed { award } else { 0 },
        })
    }
}
#[derive(Debug)]
pub struct ComboEvent {
    pub failed: Option<(String, u32)>,
    pub kind: String,
    pub count: u32,
    pub success: bool,
    pub points: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combo_triples_require_consecutive_original_targets() {
        let mut score = ComboScore::default();
        for id in 1..=9 {
            assert!(score.register(id, "CHERRY", 10, false));
        }
        assert!(score.register(10, "MELON", 100, false));
        score.hit(1);
        score.hit(2);
        assert_eq!((score.score, score.streak), (0, 2));
        score.hit(10); // Wrong fruit starts its own streak.
        score.hit(3);
        score.hit(4);
        assert_eq!((score.score, score.streak), (0, 2));
        score.hit(5);
        assert_eq!((score.score, score.streak, score.triples), (10, 0, 1));
        score.hit(5);
        score.hit(999); // Repeated source / untagged fragments.
        assert_eq!((score.score, score.streak), (10, 0));
        score.hit(6);
        score.hit(7);
        score.hit(8);
        assert_eq!((score.score, score.triples), (20, 2));
        score.reset();
        assert!(score.enabled && score.targets.contains_key(&9));
        assert_eq!((score.score, score.streak, score.triples), (0, 0, 0));
        score.hit(9); // A live target's metadata survives the round restart.
        assert_eq!((score.score, score.streak), (0, 1));
    }
    #[test]
    fn combo_validates_tags() {
        let mut score = ComboScore::default();
        assert!(!score.register(1, "", 10, false));
        assert!(!score.register(1, "A", 0, false));
        assert!(!score.register(1, "A\n", 10, false));
        assert!(score.register(1, "A", 10, false));
        assert!(!score.register(2, "A", 20, false));
    }
    #[test]
    fn combo_wildcard_permutations_use_the_ordinary_fruit_reward() {
        for wild_mask in 0..7 {
            let mut score = ComboScore::default();
            for id in 0..3 {
                let wild = wild_mask & (1 << id) != 0;
                assert!(score.register(
                    id,
                    if wild { "WILD" } else { "CHERRY" },
                    if wild { 120 } else { 10 },
                    wild,
                ));
                score.hit(id);
                if id < 2 && wild_mask & ((1 << (id + 1)) - 1) == (1 << (id + 1)) - 1 {
                    assert!(score.kind.is_empty());
                }
            }
            assert_eq!((score.score, score.triples, score.streak), (10, 1, 0));
            assert!(score.kind.is_empty());
        }
    }
    #[test]
    fn combo_three_wildcards_pay_and_start_a_fresh_triple() {
        let mut score = ComboScore::default();
        for id in 0..3 {
            assert!(score.register(id, "WILD", 120, true));
            score.hit(id);
        }
        assert_eq!((score.score, score.triples, score.streak), (120, 1, 0));
        assert!(score.kind.is_empty());
        for (id, kind, points, wild) in [
            (3, "CHERRY", 10, false),
            (4, "WILD", 120, true),
            (5, "WILD", 120, true),
            (6, "WILD", 120, true),
            (7, "WILD", 120, true),
            (8, "MELON", 100, false),
        ] {
            assert!(score.register(id, kind, points, wild));
            score.hit(id);
        }
        assert_eq!((score.score, score.triples, score.streak), (230, 3, 0));
        assert!(score.kind.is_empty());
    }
    #[test]
    fn combo_wrong_fruit_breaks_a_wildcard_streak() {
        let mut score = ComboScore::default();
        for (id, kind, points, wild) in [
            (1, "CHERRY", 10, false),
            (2, "WILD", 120, true),
            (3, "MELON", 100, false),
            (4, "WILD", 120, true),
            (5, "MELON", 100, false),
        ] {
            assert!(score.register(id, kind, points, wild));
        }
        for id in 1..=3 {
            score.hit(id);
        }
        assert_eq!(
            (score.score, score.streak, score.kind.as_str()),
            (0, 1, "MELON")
        );
        score.hit(4);
        score.hit(5);
        assert_eq!((score.score, score.triples), (100, 1));
    }
    #[test]
    fn combo_reset_keeps_live_wildcards_and_ignores_duplicate_hits() {
        let mut score = ComboScore::default();
        for id in 0..5 {
            assert!(score.register(id, "WILD", 120, true));
        }
        for id in 0..3 {
            score.hit(id);
        }
        score.hit(3);
        score.hit(3);
        assert_eq!((score.score, score.triples, score.streak), (120, 1, 1));
        score.reset();
        assert_eq!((score.score, score.triples, score.streak), (0, 0, 0));
        assert!(score.enabled && score.kind.is_empty());
        assert_eq!(score.targets.len(), 1);
        assert_eq!(score.targets.get(&4), Some(&("WILD".into(), 120, true)));
        score.hit(3);
        score.hit(4);
        score.hit(4);
        assert_eq!((score.score, score.triples, score.streak), (0, 0, 1));
    }
    #[test]
    fn combo_display_tracks_wildcards_payout_and_reset() {
        let mut score = ComboScore::default();
        for (id, kind, wild) in [
            (1, "ZOLIK", true),
            (2, "TRESEN", false),
            (3, "ZOLIK", true),
            (4, "MELOUN", false),
            (5, "ZOLIK", true),
            (6, "ZOLIK", true),
            (7, "ZOLIK", true),
            (8, "ZOLIK", true),
            (9, "ZOLIK", true),
            (10, "ZOLIK", true),
        ] {
            assert!(score.register(id, kind, 10, wild));
        }
        assert_eq!(score.display_progress(), ("", 0));
        score.hit(1);
        assert_eq!(score.display_progress(), ("ZOLIK", 1));
        score.hit(2);
        assert_eq!(score.display_progress(), ("TRESEN", 2));
        score.hit(3);
        assert_eq!(score.display_progress(), ("TRESEN", 3));
        // Payout remains visible through unregistered and duplicate hits.
        score.hit(999);
        score.hit(3);
        assert_eq!(score.display_progress(), ("TRESEN", 3));
        score.hit(4);
        assert_eq!(score.display_progress(), ("MELOUN", 1));
        score.hit(5);
        assert_eq!(score.display_progress(), ("MELOUN", 2));
        score.hit(6);
        assert_eq!(score.display_progress(), ("MELOUN", 3));
        score.hit(7);
        assert_eq!(score.display_progress(), ("ZOLIK", 1));
        let mut all_wild = ComboScore::default();
        for id in 20..23 {
            assert!(all_wild.register(id, "ZOLIK", 10, true));
            all_wild.hit(id);
        }
        assert_eq!(all_wild.display_progress(), ("ZOLIK", 3));
        score.reset();
        assert_eq!(score.display_progress(), ("", 0));
    }
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
