//! Native host choreography over the shared Rust player, cutting, scoring and UI.
use crate::{
    content::Content,
    initials::Keyboard,
    scores::{Entry, Scores},
};
use glam::{Mat4, Vec3};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use webgpu_wasm::frame_input::FrameInput;
use webgpu_wasm::scene::SCENE_TRIANGLE_BUDGET_EXCEEDED;
use webgpu_wasm::{
    combo_display::ComboDisplay,
    scene::{RenderOptions, SceneManager},
    xr_player::XrPlayer,
    xr_round::{ComboScore, RoundClock},
};

pub(super) const HIDDEN: Vec3 = Vec3::new(100., -10000., 100.);
#[derive(Clone, Deserialize)]
pub struct Recipe {
    pub fruit: String,
    pub scale: f32,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub gravity: f32,
    pub lifetime: f32,
}
#[derive(Clone, Deserialize)]
struct Cue {
    at: f32,
    label: String,
    recipes: Vec<Recipe>,
}
#[derive(Clone, Deserialize)]
struct Variant {
    variant: u32,
    coverage: f32,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fruit {
    name: String,
    label: String,
    points: u32,
    cap_color: [f32; 3],
    asset: u32,
    variants: Vec<Variant>,
}

pub struct Game {
    pub audio: webgpu_wasm::audio::AudioScene,
    audio_swing_at: [f64; 2],
    music_bar: Option<u32>,
    pub player: XrPlayer,
    pub score: ComboScore,
    pub round: Option<RoundClock>,
    pub cuts: u64,
    pub shots: u64,
    pub cue_index: usize,
    pub display: ComboDisplay,
    refs: Value,
    catalog: BTreeMap<String, Fruit>,
    cues: Vec<Cue>,
    cursors: BTreeMap<String, usize>,
    pub flying: Vec<u32>,
    xp_drink: webgpu_wasm::xp_drink::XpDrink,
    fragments: BTreeSet<u32>,
    indicators: Vec<(u32, String, u32, Vec3)>,
    previous: [Option<Mat4>; 2],
    triggers: [bool; 2],
    primary: bool,
    last_second: u32,
    scores: Scores,
    keyboard: Option<Keyboard>,
    aim_pointers: [u32; 2],
    last_score_entry: Option<Entry>,
    score_entry_sequence: u32,
    scoreboard_rows: Vec<u32>,
    glints: Vec<u32>,
    pub active: bool,
    pub solid: bool,
    pub haptics: [f32; 2],
}
impl Game {
    pub fn new(
        scene: &mut SceneManager,
        content: &Content,
        score_path: Option<PathBuf>,
    ) -> Result<Self, String> {
        let refs = content.refs.clone();
        let mut display = ComboDisplay::configure(scene, &refs["comboDisplay"].to_string())
            .ok_or("Invalid native combo display")?;
        display.reset(scene);
        let mut score = ComboScore::default();
        score.set_chain_mode(true);
        let catalog: Vec<Fruit> =
            serde_json::from_value(content.manifest["runtime"]["catalog"].clone())
                .map_err(|e| e.to_string())?;
        let cues: Vec<Cue> =
            serde_json::from_value(content.manifest["runtime"]["minutePlan"]["cues"].clone())
                .map_err(|e| e.to_string())?;
        if cues.is_empty() || catalog.len() != 10 {
            return Err("Incomplete native Neon program".into());
        }
        let mut player = XrPlayer::default();
        player.bind_hand(scene, 0, Some(id(&refs["leftSword"])?), true)?;
        player.bind_hand(scene, 1, Some(id(&refs["sword"])?), true)?;
        for (hand, name) in ["leftPetHand", "rightPetHand"].into_iter().enumerate() {
            if let Ok(id) = id(&refs[name]) {
                player.bind_pet_hand(scene, hand, Some(id))?;
            }
        }
        let indicators = content
            .indicator_bindings
            .iter()
            .map(|(id, kind, slot)| {
                (
                    *id,
                    kind.clone(),
                    *slot,
                    scene.objects()[id].transform.scale,
                )
            })
            .collect();
        let scoreboard_rows = refs["scoreboardPanels"]
            .as_array()
            .ok_or("Missing scoreboard")?
            .iter()
            .skip(1)
            .map(id)
            .collect::<Result<_, _>>()?;
        let mut game = Self {
            audio: Default::default(),
            audio_swing_at: [-1.; 2],
            music_bar: None,
            player,
            score,
            round: None,
            cuts: 0,
            shots: 0,
            cue_index: 0,
            display,
            aim_pointers: [id(&refs["pointers"][0])?, id(&refs["pointers"][1])?],
            refs,
            catalog: catalog.into_iter().map(|f| (f.name.clone(), f)).collect(),
            cues,
            cursors: BTreeMap::new(),
            flying: Vec::new(),
            xp_drink: Default::default(),
            fragments: BTreeSet::new(),
            indicators,
            previous: [None; 2],
            triggers: [true; 2],
            primary: true,
            last_second: u32::MAX,
            scores: Scores::load(score_path),
            keyboard: None,
            last_score_entry: None,
            score_entry_sequence: 0,
            scoreboard_rows,
            glints: Vec::new(),
            active: false,
            solid: true,
            haptics: [0.; 2],
        };
        game.audio.set_policy(webgpu_wasm::audio::AudioPolicy {
            groups: [
                ("blade_swing", 2),
                ("fruit_cut", 12),
                ("neon_bar_", 1),
                ("", 2),
            ]
            .into_iter()
            .map(|(prefix, limit)| webgpu_wasm::audio::AudioGroupPolicy {
                prefix: prefix.into(),
                limit,
            })
            .collect(),
            ..Default::default()
        });
        game.refresh_scoreboard(scene)?;
        game.apply_gallery_style(scene);
        game.refresh_indicators(scene);
        Ok(game)
    }
    /// Platform storage adapter; gameplay and scoreboard geometry remain Rust-owned.
    pub fn restore_scores(
        &mut self,
        scene: &mut SceneManager,
        scores: &[u64],
    ) -> Result<(), String> {
        self.restore_leaderboard(
            scene,
            scores
                .iter()
                .map(|&score| Entry {
                    score,
                    initials: "VR".into(),
                })
                .collect(),
        )
    }

    pub fn restore_leaderboard(
        &mut self,
        scene: &mut SceneManager,
        entries: Vec<Entry>,
    ) -> Result<(), String> {
        self.scores.restore(entries);
        self.refresh_scoreboard(scene)
    }
    pub fn ref_id(&self, name: &str) -> Result<u32, String> {
        id(&self.refs[name])
    }
    pub fn reset_tracking(&mut self, scene: &mut SceneManager) {
        scene.set_sprite_target_player(None, false);
        self.player.reset_tracking(scene);
        self.place_xp_drink(scene, None);
        self.previous = [None; 2];
        self.triggers = [true; 2];
        self.primary = true;
        self.haptics = [0.; 2];
        for pointer in self.aim_pointers {
            scene.set_object_position(pointer, HIDDEN);
        }
    }
    pub fn start(&mut self, scene: &mut SceneManager) -> Result<(), String> {
        if let Some(keyboard) = self.keyboard.take() {
            keyboard.close(scene);
        }
        for id in self
            .flying
            .drain(..)
            .chain(std::mem::take(&mut self.fragments))
            .chain(self.glints.drain(..))
        {
            scene.remove_object(id);
        }
        scene.clear_cut_feedback();
        self.audio.restart();
        self.music_bar = None;
        self.audio_swing_at = [-1.; 2];
        self.audio.emit("round_start", None, 0.55, 1., 3);
        self.round = RoundClock::new(60.);
        scene.take_pet_gift_origin();
        self.reset_xp_drink(scene);
        self.score.reset();
        self.score.set_chain_mode(true);
        scene.set_pet_gift_enabled(self.ref_id("xpDrink").is_ok());
        self.cuts = 0;
        self.shots = 0;
        self.cue_index = 0;
        self.last_second = u32::MAX;
        self.active = true;
        self.display.reset(scene);
        self.display.sync_chains(scene, &self.score.chains);
        self.reset_tracking(scene);
        self.refresh_indicators(scene);
        scene.set_object_position(self.ref_id("restartButton")?, HIDDEN);
        scene.set_object_rotation(self.ref_id("restartButton")?, Vec3::ZERO);
        self.show_phase(scene, "");
        self.update_panels(scene);
        Ok(())
    }
    pub fn tick(&mut self, scene: &mut SceneManager, input: FrameInput) -> Result<(), String> {
        scene.set_pet_gift_enabled(
            self.active
                && self.round.is_some_and(|round| !round.finished())
                && self.score.cat_wild_remaining() <= 0.
                && self.ref_id("xpDrink").is_ok(),
        );
        scene.set_sprite_target_player(
            input.head.map(|h| h.w_axis.truncate()),
            self.active
                && input.dt.is_finite()
                && input.dt > 0.
                && self.round.is_some_and(|r| !r.finished()),
        );
        // Pose submission does not advance timers, including when tracking is lost.
        scene.interact_sprites(
            input.head.map(|head| head.w_axis.truncate()),
            [0, 1].map(|i| {
                input.hands[i]
                    .filter(|_| self.xp_drink.held_hand() != Some(i))
                    .map(|pose| pose.w_axis.truncate())
            }),
        );
        if !input.dt.is_finite() || input.dt <= 0. || input.head.is_none() {
            self.reset_tracking(scene);
            return Ok(());
        }
        let dt = input.dt;
        self.audio.advance(dt);
        self.player.set_reference(Mat4::IDENTITY);
        self.player.update_head(scene, input.head)?;
        if !self.active {
            self.start(scene)?;
            self.player.update_head(scene, input.head)?;
        }
        let restart_requested = input.primary && !self.primary;
        self.primary = input.primary;
        let pressed = [0, 1].map(|i| input.triggers[i] && !self.triggers[i]);
        for i in 0..2 {
            self.triggers[i] = input.triggers[i];
            if pressed[i] && self.round.is_some_and(|r| !r.finished()) {
                self.bank(scene, i);
            }
        }
        if let Some(keyboard) = self.keyboard.as_mut() {
            if keyboard.update(scene, input.rays, pressed) {
                let entry = Entry {
                    score: self.score.score,
                    initials: keyboard.initials.clone(),
                };
                match self.scores.record(entry.clone()) {
                    Ok(()) => {
                        self.last_score_entry = Some(entry);
                        self.score_entry_sequence = self.score_entry_sequence.wrapping_add(1);
                        self.keyboard.take().unwrap().close(scene);
                        self.refresh_scoreboard(scene)?;
                    }
                    Err(error) => log::warn!("Leaderboard save failed: {error}"),
                }
            }
        }
        if restart_requested {
            self.start(scene)?;
            self.player.update_head(scene, input.head)?;
        }
        let was_finished = self.round.is_some_and(|r| r.finished());
        self.score.advance_cat_wild(dt);
        if let Some(round) = &mut self.round {
            round.advance(dt);
        }
        if !was_finished && self.round.is_some_and(|r| r.finished()) {
            self.score.advance_cat_wild(self.score.cat_wild_remaining());
            self.reset_xp_drink(scene);
            self.audio.emit("round_end", None, 0.6, 1., 3);
            self.score.bank_all();
            self.display.sync_chains(scene, &self.score.chains);
            if self.scores.qualifies(self.score.score) {
                self.keyboard = Some(Keyboard::open(scene, self.aim_pointers)?);
            }
        }
        scene.take_pet_gift_origin();
        let carrier = scene.pet_drink_carrier(input.head.map(|head| head.w_axis.truncate()));
        let enabled = self.round.is_some_and(|round| !round.finished())
            && self.score.cat_wild_remaining() <= 0.
            && self.ref_id("xpDrink").is_ok();
        self.xp_drink.follow_carrier(carrier, enabled);
        let previous_hand = self.xp_drink.held_hand();
        let (bottle, consumed) = self.xp_drink.update(dt, input.head, input.hands);
        if previous_hand.is_none() && self.xp_drink.held_hand().is_some() {
            if let Some(carrier) = carrier {
                scene.pet_after_drink_pickup(carrier.visit);
            }
        }
        scene.set_pet_drink_visual(self.xp_drink.carried_visit());
        self.place_xp_drink(scene, bottle);
        if consumed && self.round.is_some_and(|round| !round.finished()) {
            self.score.activate_cat_wild();
            if let Some(hand) = previous_hand {
                self.haptics[hand] = 0.65;
            }
        }
        self.display.sync_xp_status(
            scene,
            self.xp_drink.held_hand().is_some(),
            self.score.cat_wild_remaining(),
        );
        for hand in 0..2 {
            self.player
                .set_hand_suppressed(hand, self.xp_drink.held_hand() == Some(hand));
        }
        self.display.advance(scene, dt);
        if !self.round.is_some_and(|r| r.finished()) {
            let elapsed = 60. - self.round.map_or(60., |r| r.remaining);
            // Four quarter notes at 120 BPM. Use the fruit clock so pauses and
            // restarts affect both the score and the choreography together.
            let bar = (elapsed / 2.) as u32;
            if self.music_bar != Some(bar) {
                self.music_bar = Some(bar);
                let clip = match bar {
                    9 | 19 | 28 => "neon_bar_break",
                    0..=9 => "neon_bar_intro",
                    10..=19 => "neon_bar_drive",
                    _ => "neon_bar_final",
                };
                self.audio.emit(clip, None, 0.65, 1., 2);
            }
            while self.cue_index < self.cues.len()
                && self.cues[self.cue_index].at <= elapsed + 0.001
            {
                let cue = self.cues[self.cue_index].clone();
                self.cue_index += 1;
                if elapsed - cue.at > 0.4 {
                    continue;
                }
                self.show_phase(scene, &cue.label);
                self.spawn(scene, &cue.recipes, true)?;
            }
        }
        scene.set_sprite_target_player(
            self.player.head().map(|h| h.matrix.w_axis.truncate()),
            self.active && self.round.is_some_and(|r| !r.finished()),
        );
        for i in 0..2 {
            let speed = match (self.previous[i], input.hands[i]) {
                (Some(previous), Some(current)) => {
                    (current.w_axis.truncate() - previous.w_axis.truncate()).length() / dt
                }
                _ => 0.,
            };
            let hand_position =
                input.hands[i].map(|m| m.transform_point3(Vec3::new(0., 0., -0.55)).to_array());
            if self.xp_drink.held_hand() != Some(i)
                && !self.player.wants_pet_hand(scene, i, input.hands[i])
                && speed > 2.
                && self.audio.clock() - self.audio_swing_at[i] > 0.28
            {
                self.audio_swing_at[i] = self.audio.clock();
                self.audio.emit(
                    if i == 0 {
                        "blade_swing_1"
                    } else {
                        "blade_swing_2"
                    },
                    hand_position,
                    0.18,
                    1.,
                    0,
                );
            }
            let triples_before = self.score.triples;
            let report = self
                .player
                .update_hand(scene, i, input.hands[i], true, dt)?;
            if !report.removed.is_empty() || !report.sprite_hits.is_empty() {
                self.haptics[i] = 0.6;
                let variant = self.cuts % 3 + 1;
                let clip = format!(
                    "fruit_cut_{}_{}",
                    if speed > 2.5 { "hard" } else { "soft" },
                    variant
                );
                self.audio.emit(&clip, hand_position, 0.65, 1., 2);
                let mut rewards = Vec::new();
                for hit in &report.sprite_hits {
                    self.score.award_round_bonus(self.round, hit.points);
                }
                if !self.round.is_some_and(|r| r.finished()) {
                    self.cuts = self.cuts.saturating_add(report.removed.len() as u64);
                    for &original in &report.removed {
                        if let Some(accepted) = self.score.hit_chain(original) {
                            rewards.push((original, self.score.last_chain_total_points()));
                            if !accepted && self.score.cat_wild_remaining() <= 0. {
                                self.display.notice(scene, "PLNO".into());
                            }
                        }
                    }
                    self.display.sync_chains(scene, &self.score.chains);
                    if self.score.triples > triples_before {
                        scene.trigger_combo_sprites();
                        self.audio.emit("combo_triple", None, 0.45, 1., 3);
                    }
                    self.refresh_indicators(scene);
                }
                for &piece in &report.pieces {
                    scene.set_emission(piece, 1.1);
                    self.fragments.insert(piece);
                }
                for pieces in report.pieces.chunks(32).filter(|p| p.len() >= 2) {
                    if let Err(error) =
                        scene.play_cut_feedback_scored(pieces, false, 0.15, &rewards)
                    {
                        if error != SCENE_TRIANGLE_BUDGET_EXCEEDED {
                            return Err(error);
                        }
                    }
                    // Original web presentation: physical halves fall/spin until
                    // the feedback pool retires them; no immediate dissolve.
                }
            }
        }
        self.previous = [0, 1].map(|i| {
            input.hands[i]
                .filter(|_| !self.player.petting_hands()[i] && self.xp_drink.held_hand() != Some(i))
        });
        self.flying.retain(|id| scene.object_exists(*id));
        self.fragments.retain(|id| scene.object_exists(*id));
        self.glints.retain(|id| scene.object_exists(*id));
        self.score.targets.retain(|id, _| scene.object_exists(*id));
        self.update_panels(scene);
        Ok(())
    }
    fn place_xp_drink(&self, scene: &mut SceneManager, bottle: Option<Mat4>) {
        let Ok(id) = self.ref_id("xpDrink") else {
            return;
        };
        if let Some(matrix) = bottle {
            let (_, rotation, position) = matrix.to_scale_rotation_translation();
            let (y, x, z) = rotation.to_euler(glam::EulerRot::YXZ);
            scene.set_object_position(id, position);
            scene.set_object_rotation(id, Vec3::new(x, y, z));
        } else {
            scene.set_object_position(id, HIDDEN);
        }
    }
    fn reset_xp_drink(&mut self, scene: &mut SceneManager) {
        self.xp_drink.reset();
        scene.reset_pet_gifts();
        scene.set_pet_drink_visual(None);
        self.place_xp_drink(scene, None);
        for hand in 0..2 {
            self.player.set_hand_suppressed(hand, false);
        }
    }
    pub fn spawn(
        &mut self,
        scene: &mut SceneManager,
        recipes: &[Recipe],
        glints: bool,
    ) -> Result<usize, String> {
        self.spawn_with_limit(scene, recipes, glints, 10)
    }

    fn spawn_with_limit(
        &mut self,
        scene: &mut SceneManager,
        recipes: &[Recipe],
        glints: bool,
        limit: usize,
    ) -> Result<usize, String> {
        self.flying.retain(|id| scene.object_exists(*id));
        if recipes.len() + self.flying.len() > limit {
            return Ok(0);
        }
        let mut spawned = 0;
        for (index, r) in recipes.iter().enumerate() {
            let fruit = self.catalog.get(&r.fruit).ok_or("Unknown Neon fruit")?;
            let cursor = self.cursors.entry(r.fruit.clone()).or_default();
            let variant = &fruit.variants[*cursor % fruit.variants.len()];
            *cursor = (*cursor + 1) % fruit.variants.len();
            let cap = if self.solid {
                Vec3::from_array(fruit.cap_color) * 0.22
            } else {
                Vec3::ZERO
            };
            let id = loop {
                match scene.add_cuttable_mesh_variant(fruit.asset, variant.variant, cap) {
                    Ok(id) => break id,
                    Err(error) if error == SCENE_TRIANGLE_BUDGET_EXCEEDED => {
                        // Expired or old cut debris yields capacity to incoming fruit.
                        let Some(oldest) = self.fragments.pop_first() else {
                            return Ok(spawned);
                        };
                        scene.remove_object(oldest);
                    }
                    Err(error) => return Err(error),
                }
            };
            scene.set_object_position(id, Vec3::from_array(r.position));
            scene.set_object_scale(id, Vec3::splat(r.scale));
            scene.set_emission(id, 2.4);
            scene.set_color_cutout(id, if self.solid { 0. } else { 0.25 });
            scene.set_opacity(
                id,
                if self.solid { 0.06 } else { 1. },
                0.25,
                if self.solid { 1. } else { 0. },
                variant.coverage,
            );
            scene.set_render_options(
                id,
                RenderOptions {
                    casts_shadow: false,
                    receives_shadow: false,
                    ..Default::default()
                },
            );
            if !self
                .score
                .register(id, &fruit.label, fruit.points, fruit.name == "wild")
            {
                return Err("Invalid native combo target".into());
            }
            scene.set_projectile(id, Vec3::from_array(r.velocity), r.gravity, 0.);
            let spin = if r.gravity >= 2. {
                Vec3::new(
                    0.55 + r.velocity[0].abs() * 0.8,
                    0.8 + r.position[0].abs() * 0.7,
                    r.velocity[0] * 1.1,
                )
            } else {
                Vec3::new(0.12, 0.45, 0.06)
            };
            scene.set_object_spin(id, spin);
            scene.schedule_remove_object(id, r.lifetime);
            self.flying.push(id);
            self.shots += 1;
            spawned += 1;
            if glints {
                if let Some(asset) = self.refs["launchGlintAssets"]
                    .get(index % 3)
                    .and_then(Value::as_u64)
                {
                    for sign in [-1., 1.] {
                        let glint = match scene.add_mesh_variant(asset as u32, 0) {
                            Err(error) if error == SCENE_TRIANGLE_BUDGET_EXCEEDED => break,
                            result => result?,
                        };
                        scene.set_object_position(
                            glint,
                            Vec3::from_array(r.position) + Vec3::new(sign * 0.045, -0.04, 0.),
                        );
                        scene.set_object_scale(glint, Vec3::new(0.006, 0.09, 0.006));
                        scene.set_object_rotation(glint, Vec3::new(0., 0., sign * 0.35));
                        scene.set_emission(glint, 2.5);
                        scene.set_render_options(
                            glint,
                            RenderOptions {
                                casts_shadow: false,
                                receives_shadow: false,
                                ..Default::default()
                            },
                        );
                        scene.schedule_remove_object(glint, 0.24);
                        self.glints.push(glint);
                    }
                }
            }
        }
        Ok(spawned)
    }
    fn bank(&mut self, scene: &mut SceneManager, index: usize) {
        if self.score.bank_chain(index).is_some() {
            self.audio.emit("chain_bank", None, 0.4, 1., 3);
            self.display.sync_chains(scene, &self.score.chains);
            self.display.notice(scene, "SERIE ULOZENA".into());
            self.refresh_indicators(scene);
        }
    }
    fn refresh_indicators(&self, scene: &mut SceneManager) {
        for (id, kind, slot, scale) in &self.indicators {
            let active = self
                .score
                .chains
                .iter()
                .flatten()
                .any(|c| &c.kind == kind && *slot < c.count);
            scene.set_emission(*id, if active { 8. } else { 0.65 });
            scene.set_object_scale(*id, *scale * if active { 1.32 } else { 1. });
        }
    }
    fn apply_gallery_style(&self, scene: &mut SceneManager) {
        // The web bootstrap applies this after createScene; keep the same
        // fifteen-pattern coverage/material rules in the native bootstrap.
        let mut catalog = self
            .catalog
            .values()
            .filter(|f| f.name != "wild")
            .collect::<Vec<_>>();
        catalog.sort_by_key(|f| f.points);
        if let Some(wild) = self.catalog.get("wild") {
            catalog.push(wild);
        }
        for (row, fruit) in catalog.into_iter().enumerate() {
            for slot in 0..3 {
                if let Some(id) = self.refs["galleryIcons"]
                    .get(row * 3 + slot)
                    .and_then(Value::as_u64)
                {
                    scene.set_color_cutout(id as u32, if self.solid { 0. } else { 0.25 });
                    scene.set_opacity(
                        id as u32,
                        if self.solid { 0.06 } else { 1. },
                        0.25,
                        if self.solid { 1. } else { 0. },
                        fruit.variants[slot].coverage,
                    );
                }
            }
        }
    }
    fn show_phase(&self, scene: &mut SceneManager, label: &str) {
        if let Some(panels) = self.refs["phasePanels"].as_array() {
            for panel in panels {
                if let Ok(id) = id(&panel["object"]) {
                    scene.set_object_position(
                        id,
                        if panel["title"] == label {
                            Vec3::new(0., 2.75, -2.15)
                        } else {
                            HIDDEN
                        },
                    );
                }
            }
        }
    }
    fn update_panels(&mut self, scene: &mut SceneManager) {
        if let Ok(id) = self.ref_id("scorePanel") {
            scene.update_info_panel(id, &[("", self.score.score.to_string())]);
        }
        let second = self.round.map_or(60, |r| r.remaining.ceil() as u32);
        if second != self.last_second {
            if (1..=3).contains(&second) {
                self.audio.emit("countdown_tick", None, 0.35, 1., 3);
            }
            self.last_second = second;
            if let Ok(id) = self.ref_id("timerPanel") {
                scene.update_info_panel(id, &[("", second.to_string())]);
            }
        }
    }
    fn refresh_scoreboard(&mut self, scene: &mut SceneManager) -> Result<(), String> {
        for id in self.scoreboard_rows.drain(..) {
            scene.remove_object(id);
        }
        let labels = if self.scores.top.is_empty() {
            vec!["ZATIM BEZ SKORE".into()]
        } else {
            self.scores
                .top
                .iter()
                .enumerate()
                .map(|(i, s)| format!("{}. {}  {}", i + 1, s.initials, s.score))
                .collect()
        };
        for (i, label) in labels.iter().enumerate() {
            let id = scene.add_info_panel("debug-text", 1.22, 0.12)?;
            scene.update_info_panel(id, &[("", label.clone())]);
            scene.set_object_position(id, Vec3::new(1.6, 1.94 - i as f32 * 0.15, -2.2));
            scene.set_object_rotation(id, Vec3::new(0., (-1.6f32).atan2(2.2), 0.));
            scene.set_emission(id, 0.9);
            scene.set_render_options(
                id,
                RenderOptions {
                    is_static: true,
                    casts_shadow: false,
                    receives_shadow: false,
                    ..Default::default()
                },
            );
            self.scoreboard_rows.push(id);
        }
        Ok(())
    }
    pub fn state(&self) -> Value {
        json!({"catWildRemaining":self.score.cat_wild_remaining(),"xpDrinkActive":self.xp_drink.active(),"drinkingHands":([0,1].map(|hand|self.xp_drink.held_hand()==Some(hand))),"pettingHands":self.player.petting_hands(),"audio":self.audio.snapshot(),"active":self.active,"round":self.round.map(|r|json!({"duration":r.duration,"remaining":r.remaining,"finished":r.finished()})),"score":self.score.score,"cuts":self.cuts,"shots":self.shots,"cueIndex":self.cue_index,"flying":self.flying,"fragments":self.fragments.len(),"stocks":self.score.chains.iter().map(|c|c.as_ref().map(|c|json!({"kind":c.kind,"count":c.count,"points":c.total_points()}))).collect::<Vec<_>>(),"leaderboard":self.scores.top,"scoreEntryPending":self.keyboard.is_some(),"lastScoreEntry":self.last_score_entry,"scoreEntrySequence":self.score_entry_sequence,"top5":self.scores.top.iter().take(5).map(|e|e.score).collect::<Vec<_>>()})
    }
}
fn id(v: &Value) -> Result<u32, String> {
    v.as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| format!("Missing native scene id: {v}"))
}
