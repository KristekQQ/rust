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

/// Guaranteed reward for each registered original cut in chain mode.
pub const CHAIN_HIT_POINTS: u64 = 10;

/// Pending bonus; guaranteed hit points are already in ComboScore::score.
#[derive(Clone, Debug)]
pub struct FruitChain {
    pub kind: String,
    pub count: u32,
    pub points: u32,
}
impl FruitChain {
    /// Total earned by the stored fruit: guaranteed hits and chain bonus.
    pub fn total_points(&self) -> u64 {
        u64::from(self.count)
            .saturating_mul(CHAIN_HIT_POINTS)
            .saturating_add(self.value())
    }
    pub fn value(&self) -> u64 {
        if self.count < 3 {
            return 0;
        }
        let n = u64::from(self.count);
        // The triangular factor fits u64 for every u32 count. Divide before
        // multiplying by points so only the final reward saturates.
        ((n - 2) * (n - 1) / 2).saturating_mul(u64::from(self.points))
    }
}

/// Optional object-tagged scoring. Only original targets can award points;
/// fragments never inherit a target registration.
#[derive(Default)]
pub struct ComboScore {
    pub enabled: bool,
    pub chain_mode: bool,
    pub chains: [Option<FruitChain>; 2],
    pub rejected: u32,
    pub last_lost_bonus: u64,
    pub score: u64,
    pub triples: u32,
    pub streak: u32,
    pub kind: String,
    pub targets: std::collections::BTreeMap<u32, (String, u32, bool)>,
    streak_points: u32,
    display_kind: String,
    display_count: u32,
    chain_wild: [bool; 2],
    last_chain_index: Option<usize>,
    // Relative creation order: 0 = vacant, 1 = older, 2 = newer.
    // Bounded ranks avoid counter overflow during arbitrarily long rounds.
    chain_age: [u8; 2],
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
            chain_mode: self.chain_mode,
            targets: std::mem::take(&mut self.targets),
            ..Self::default()
        };
    }
    pub fn set_chain_mode(&mut self, enabled: bool) {
        if self.chain_mode == enabled {
            return;
        }
        self.chain_mode = enabled;
        self.reset();
    }
    /// Consume an original target once. A third ordinary kind replaces the
    /// oldest chain; its previously earned reward remains in the score.
    pub fn hit_chain(&mut self, id: u32) -> Option<bool> {
        let (kind, points, wild) = self.targets.remove(&id)?;
        // Only registered originals get the guaranteed reward, exactly once.
        self.score = self.score.saturating_add(CHAIN_HIT_POINTS);
        self.last_lost_bonus = 0;
        let slot = if wild {
            self.chains
                .iter()
                .enumerate()
                .filter_map(|(index, chain)| chain.as_ref().map(|c| (index, c.count)))
                .min_by_key(|&(index, count)| (count, index))
                .map(|(index, _)| index)
                .or(Some(0))
        } else {
            self.chains
                .iter()
                .position(|chain| chain.as_ref().is_some_and(|c| c.kind == kind))
                .or_else(|| {
                    self.chain_wild
                        .iter()
                        .enumerate()
                        .position(|(index, wild)| *wild && self.chains[index].is_some())
                })
                .or_else(|| self.chains.iter().position(Option::is_none))
                .or_else(|| (0..self.chains.len()).min_by_key(|&index| self.chain_age[index]))
        };
        let index = slot?;
        if !wild
            && !self.chain_wild[index]
            && self.chains[index]
                .as_ref()
                .is_some_and(|chain| chain.kind != kind)
        {
            // Chain growth is credited immediately, so replacing its display
            // row cannot remove points already earned.
            self.last_lost_bonus = 0;
            self.chains[index] = None;
        }
        let old_value = self.chains[index].as_ref().map_or(0, FruitChain::value);
        let old_count = self.chains[index].as_ref().map_or(0, |chain| chain.count);
        if let Some(chain) = self.chains[index].as_mut() {
            if !wild && self.chain_wild[index] {
                chain.kind = kind;
                chain.points = points;
                self.chain_wild[index] = false;
            }
            chain.count = chain.count.saturating_add(1);
        } else {
            let other = 1 - index;
            self.chain_age[other] = u8::from(self.chains[other].is_some());
            self.chain_age[index] = 2;
            self.chains[index] = Some(FruitChain {
                kind: if wild { "ZOLIK".into() } else { kind },
                count: 1,
                points,
            });
            self.chain_wild[index] = wild;
        }
        let chain = self.chains[index]
            .as_ref()
            .expect("chain was inserted or grown");
        let new_value = chain.value();
        self.score = self
            .score
            .saturating_add(new_value.saturating_sub(old_value));
        let new_triples = chain.count / 3;
        self.triples = self
            .triples
            .saturating_add(new_triples.saturating_sub(old_count / 3));
        self.last_chain_index = Some(index);
        Some(true)
    }
    pub fn last_chain_total_points(&self) -> u64 {
        self.last_chain_index
            .and_then(|index| self.chains[index].as_ref())
            .map_or(0, FruitChain::total_points)
    }
    pub fn bank_chain(&mut self, index: usize) -> Option<u64> {
        let chain = self.chains.get_mut(index)?.take()?;
        self.chain_wild[index] = false;
        self.chain_age[index] = 0;
        Some(chain.value())
    }
    pub fn pending_bonus(&self) -> u64 {
        self.chains
            .iter()
            .flatten()
            .fold(0_u64, |sum, chain| sum.saturating_add(chain.value()))
    }
    pub fn bank_all(&mut self) -> u64 {
        (0..self.chains.len()).fold(0_u64, |total, index| {
            total.saturating_add(self.bank_chain(index).unwrap_or(0))
        })
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
    fn chain_hit(score: &mut ComboScore, id: u32, kind: &str, points: u32, wild: bool) -> bool {
        assert!(score.register(id, kind, points, wild));
        score.hit_chain(id).unwrap()
    }
    #[test]
    fn growing_chains_are_independent_and_credit_bonus_growth_immediately() {
        let mut score = ComboScore::default();
        score.set_chain_mode(true);
        for id in 0..5 {
            assert!(chain_hit(&mut score, id, "CHERRY", 10, false));
            if id < 4 {
                assert!(chain_hit(&mut score, id + 10, "MELON", 100, false));
            }
        }
        assert_eq!(score.score, 450);
        assert_eq!(score.triples, 2);
        assert_eq!(score.chains[0].as_ref().unwrap().count, 5);
        assert_eq!(score.chains[1].as_ref().unwrap().count, 4);
        assert_eq!(score.bank_chain(0), Some(60));
        assert_eq!(score.bank_chain(1), Some(300));
        assert_eq!((score.score, score.triples), (450, 2));
        assert_eq!(score.bank_all(), 0);
        assert_eq!(score.bank_chain(2), None);
        assert_eq!(score.hit_chain(0), None);
        assert_eq!(score.hit_chain(999), None);
    }
    #[test]
    fn chain_bonus_is_credited_on_each_hit_and_banking_never_repeats_it() {
        let mut score = ComboScore::default();
        score.set_chain_mode(true);
        for (id, expected_score, expected_triples) in
            [(1, 10, 0), (2, 20, 0), (3, 40, 1), (4, 70, 1), (5, 110, 1)]
        {
            chain_hit(&mut score, id, "A", 10, false);
            assert_eq!(
                (score.score, score.triples),
                (expected_score, expected_triples)
            );
            assert_eq!(score.last_chain_total_points(), expected_score);
        }
        assert_eq!(score.bank_chain(0), Some(60));
        assert_eq!((score.score, score.triples), (110, 1));
        assert_eq!(score.bank_all(), 0);
        assert_eq!((score.score, score.triples), (110, 1));
    }
    #[test]
    fn third_kind_replaces_oldest_creation_without_refreshing_on_matches() {
        let mut score = ComboScore::default();
        chain_hit(&mut score, 1, "A", 10, false);
        chain_hit(&mut score, 2, "B", 20, false);
        chain_hit(&mut score, 3, "A", 10, false);
        assert!(chain_hit(&mut score, 4, "C", 30, false));
        assert_eq!(score.rejected, 0);
        assert_eq!(score.hit_chain(4), None);
        assert_eq!(score.chains[0].as_ref().unwrap().kind, "C");
        assert_eq!(score.chains[0].as_ref().unwrap().count, 1);
        assert_eq!(score.chains[1].as_ref().unwrap().count, 1);
        chain_hit(&mut score, 5, "B", 20, false);
        assert!(chain_hit(&mut score, 6, "D", 40, false));
        assert_eq!(score.chains[0].as_ref().unwrap().kind, "C");
        assert_eq!(score.chains[1].as_ref().unwrap().kind, "D");
        assert_eq!(score.chains[1].as_ref().unwrap().count, 1);
    }
    #[test]
    fn replacement_preserves_earned_rewards_and_banking_refill_has_new_creation_age() {
        let mut score = ComboScore::default();
        for id in 0..3 {
            chain_hit(&mut score, id, "A", 10, false);
        }
        chain_hit(&mut score, 3, "B", 20, false);
        assert_eq!(score.chains[0].as_ref().unwrap().value(), 10);
        chain_hit(&mut score, 4, "C", 30, false);
        assert_eq!((score.score, score.triples), (60, 1));
        assert_eq!(score.last_chain_total_points(), 10);
        assert_eq!(score.bank_chain(0), Some(0));
        assert_eq!(score.chain_age[0], 0);
        chain_hit(&mut score, 5, "D", 40, false);
        chain_hit(&mut score, 6, "E", 50, false);
        assert_eq!(score.chains[0].as_ref().unwrap().kind, "D");
        assert_eq!(score.chains[1].as_ref().unwrap().kind, "E");
        assert_eq!(score.bank_all(), 0);
        assert_eq!(score.chain_age, [0, 0]);
        // Refilling slot 1 while slot 0 remains makes slot 0 the oldest.
        chain_hit(&mut score, 7, "A", 10, false);
        chain_hit(&mut score, 8, "B", 20, false);
        score.bank_chain(1);
        chain_hit(&mut score, 9, "C", 30, false);
        chain_hit(&mut score, 10, "D", 40, false);
        assert_eq!(score.chains[0].as_ref().unwrap().kind, "D");
        assert_eq!(score.chains[1].as_ref().unwrap().kind, "C");
    }
    #[test]
    fn wildcard_adoption_and_same_mode_keep_creation_age_until_reset() {
        let mut score = ComboScore::default();
        score.set_chain_mode(true);
        chain_hit(&mut score, 1, "WILD", 120, true);
        chain_hit(&mut score, 2, "A", 10, false);
        chain_hit(&mut score, 3, "B", 20, false);
        chain_hit(&mut score, 4, "WILD", 120, true);
        score.set_chain_mode(true);
        assert_eq!(score.chains[0].as_ref().unwrap().count, 2);
        assert_eq!(score.chains[1].as_ref().unwrap().count, 2);
        chain_hit(&mut score, 5, "C", 30, false);
        assert_eq!(score.chains[0].as_ref().unwrap().kind, "C");
        assert_eq!(score.chains[1].as_ref().unwrap().kind, "B");
        score.reset();
        assert_eq!(score.chain_age, [0, 0]);
        chain_hit(&mut score, 6, "D", 40, false);
        chain_hit(&mut score, 7, "E", 50, false);
        chain_hit(&mut score, 8, "F", 60, false);
        assert_eq!(score.chains[0].as_ref().unwrap().kind, "F");
        assert_eq!(score.chains[1].as_ref().unwrap().kind, "E");
    }
    #[test]
    fn chain_wildcards_choose_shortest_then_adopt_ordinary_kind() {
        let mut score = ComboScore::default();
        chain_hit(&mut score, 1, "WILD", 120, true);
        chain_hit(&mut score, 2, "WILD", 120, true);
        assert_eq!(score.chains[0].as_ref().unwrap().kind, "ZOLIK");
        chain_hit(&mut score, 3, "A", 10, false);
        assert_eq!(score.chains[0].as_ref().unwrap().kind, "A");
        assert_eq!(score.chains[0].as_ref().unwrap().value(), 10);
        chain_hit(&mut score, 4, "B", 20, false);
        chain_hit(&mut score, 5, "WILD", 120, true);
        chain_hit(&mut score, 6, "WILD", 120, true);
        // At a tie, slot zero wins deterministically.
        chain_hit(&mut score, 7, "WILD", 120, true);
        assert_eq!(score.chains[0].as_ref().unwrap().count, 4);
        assert_eq!(score.chains[1].as_ref().unwrap().count, 3);
        assert_eq!(score.bank_all(), 50);
        chain_hit(&mut score, 8, "WILD", 120, true);
        chain_hit(&mut score, 9, "WILD", 120, true);
        chain_hit(&mut score, 10, "WILD", 120, true);
        assert_eq!(score.bank_all(), 120);
    }
    #[test]
    fn chain_reset_mode_and_end_banking_are_idempotent_and_saturating() {
        let mut score = ComboScore::default();
        score.set_chain_mode(true);
        assert!(score.register(50, "A", 10, false));
        for id in 0..3 {
            chain_hit(&mut score, id, "A", 10, false);
        }
        assert_eq!(score.bank_all(), 10);
        assert_eq!(score.bank_all(), 0);
        assert_eq!(score.score, 40);
        score.reset();
        assert!(score.chain_mode && score.enabled && score.targets.contains_key(&50));
        assert!(score.chains.iter().all(Option::is_none));
        assert_eq!((score.score, score.triples, score.rejected), (0, 0, 0));
        let maximal = FruitChain {
            kind: "A".into(),
            count: u32::MAX,
            points: u32::MAX,
        };
        assert_eq!(maximal.value(), u64::MAX);
        score.chains = [Some(maximal.clone()), Some(maximal)];
        assert_eq!(score.pending_bonus(), u64::MAX);
        score.score = 1;
        score.triples = u32::MAX;
        assert_eq!(score.bank_all(), u64::MAX);
        assert_eq!((score.score, score.triples), (1, u32::MAX));
        assert_eq!(score.pending_bonus(), 0);
        score.set_chain_mode(false);
        assert!(!score.chain_mode);
        assert_eq!(score.score, 0);
        assert!(score.targets.contains_key(&50));
    }
    #[test]
    fn guaranteed_hits_survive_short_banks_eviction_and_duplicate_fragments() {
        let mut score = ComboScore::default();
        score.set_chain_mode(true);
        chain_hit(&mut score, 1, "A", 120, false);
        assert_eq!(score.score, 10);
        assert_eq!(score.bank_chain(0), Some(0));
        assert_eq!(score.score, 10);
        for id in 2..=6 {
            chain_hit(&mut score, id, "A", 120, false);
        }
        chain_hit(&mut score, 7, "B", 20, false);
        chain_hit(&mut score, 8, "C", 30, false);
        assert_eq!(score.last_lost_bonus, 0);
        assert_eq!(score.score, 800);
        assert_eq!(score.hit_chain(8), None);
        assert_eq!(score.hit_chain(99), None);
        assert_eq!(score.score, 800);
        assert_eq!(score.bank_all(), 0);
        chain_hit(&mut score, 9, "WILD", 120, true);
        assert_eq!(score.last_lost_bonus, 0);
        assert_eq!(score.score, 810);
        score.score = u64::MAX - 5;
        chain_hit(&mut score, 10, "WILD", 120, true);
        assert_eq!(score.score, u64::MAX);
    }
    #[test]
    fn saving_a_long_chain_beats_two_safe_triples_but_every_player_gets_hits() {
        let mut long = ComboScore::default();
        let mut safe = ComboScore::default();
        let mut beginner = ComboScore::default();
        for id in 0..6 {
            chain_hit(&mut long, id, "A", 10, false);
            chain_hit(&mut safe, id, "A", 10, false);
            if id % 3 == 2 {
                safe.bank_chain(0);
            }
            chain_hit(&mut beginner, id, &format!("K{id}"), 10, false);
        }
        long.bank_all();
        beginner.bank_all();
        assert_eq!((long.score, safe.score, beginner.score), (160, 80, 60));
    }
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
