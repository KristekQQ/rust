//! Native host choreography over the shared Rust player, cutting, scoring and UI.
use crate::{content::Content, scores::Scores};
use glam::{Mat4, Vec3};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use webgpu_wasm::frame_input::FrameInput;
use webgpu_wasm::{
    combo_display::ComboDisplay,
    scene::{RenderOptions, SceneManager},
    xr_player::XrPlayer,
    xr_round::{ComboScore, RoundClock},
};

const HIDDEN: Vec3 = Vec3::new(100., -10000., 100.);
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
    fragments: BTreeSet<u32>,
    indicators: Vec<(u32, String, u32, Vec3)>,
    previous: [Option<Mat4>; 2],
    triggers: [bool; 2],
    primary: bool,
    button_touch: [bool; 2],
    last_second: u32,
    scores: Scores,
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
        if cues.len() != 24 || catalog.len() != 10 {
            return Err("Incomplete native Neon program".into());
        }
        let mut player = XrPlayer::default();
        player.bind_hand(scene, 0, Some(id(&refs["leftSword"])?), true)?;
        player.bind_hand(scene, 1, Some(id(&refs["sword"])?), true)?;
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
            player,
            score,
            round: None,
            cuts: 0,
            shots: 0,
            cue_index: 0,
            display,
            refs,
            catalog: catalog.into_iter().map(|f| (f.name.clone(), f)).collect(),
            cues,
            cursors: BTreeMap::new(),
            flying: Vec::new(),
            fragments: BTreeSet::new(),
            indicators,
            previous: [None; 2],
            triggers: [true; 2],
            primary: true,
            button_touch: [false; 2],
            last_second: u32::MAX,
            scores: Scores::load(score_path),
            scoreboard_rows,
            glints: Vec::new(),
            active: false,
            solid: true,
            haptics: [0.; 2],
        };
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
        let mut values: Vec<_> = scores
            .iter()
            .copied()
            .filter(|v| *v <= 9_007_199_254_740_991)
            .collect();
        values.sort_unstable_by(|a, b| b.cmp(a));
        values.truncate(5);
        self.scores.top = values;
        self.refresh_scoreboard(scene)
    }
    pub fn ref_id(&self, name: &str) -> Result<u32, String> {
        id(&self.refs[name])
    }
    pub fn reset_tracking(&mut self, scene: &mut SceneManager) {
        self.player.reset_tracking(scene);
        self.previous = [None; 2];
        self.triggers = [true; 2];
        self.primary = true;
        self.button_touch = [false; 2];
        self.haptics = [0.; 2];
        if let Some(pointers) = self.refs["pointers"].as_array() {
            for pointer in pointers {
                if let Ok(id) = id(pointer) {
                    scene.set_object_position(id, HIDDEN);
                }
            }
        }
    }
    pub fn start(&mut self, scene: &mut SceneManager) -> Result<(), String> {
        for id in self
            .flying
            .drain(..)
            .chain(std::mem::take(&mut self.fragments))
            .chain(self.glints.drain(..))
        {
            scene.remove_object(id);
        }
        scene.clear_cut_feedback();
        self.round = RoundClock::new(60.);
        self.score.reset();
        self.score.set_chain_mode(true);
        self.cuts = 0;
        self.shots = 0;
        self.cue_index = 0;
        self.last_second = u32::MAX;
        self.active = true;
        self.display.reset(scene);
        self.display.sync_chains(scene, &self.score.chains);
        self.reset_tracking(scene);
        // A blade already intersecting the parked button must first leave it.
        self.button_touch = [true; 2];
        self.refresh_indicators(scene);
        scene.set_object_position(self.ref_id("restartButton")?, Vec3::new(0., 0.2, -1.45));
        scene.set_object_rotation(self.ref_id("restartButton")?, Vec3::ZERO);
        self.show_phase(scene, "");
        self.update_panels(scene);
        Ok(())
    }
    pub fn tick(&mut self, scene: &mut SceneManager, input: FrameInput) -> Result<(), String> {
        if !input.dt.is_finite() || input.dt <= 0. || input.head.is_none() {
            self.reset_tracking(scene);
            return Ok(());
        }
        let dt = input.dt;
        self.player.set_reference(Mat4::IDENTITY);
        self.player.update_head(scene, input.head)?;
        if !self.active {
            self.start(scene)?;
            self.player.update_head(scene, input.head)?;
        }
        let restart = self.ref_id("restartButton")?;
        let panel = scene.objects()[&restart].transform.model();
        let primary = input.primary && !self.primary;
        self.primary = input.primary;
        let mut restart_requested = primary;
        for i in 0..2 {
            let ray_hit = input.rays[i].and_then(|ray| {
                webgpu_wasm::xr_buttons::ray_panel_intersection(ray, panel, 0.5, 0.22, 8.)
            });
            if let Some(pointer) = self.refs["pointers"].get(i).and_then(Value::as_u64) {
                scene.set_object_position(pointer as u32, ray_hit.unwrap_or(HIDDEN));
            }
            let pressed = input.triggers[i] && !self.triggers[i];
            self.triggers[i] = input.triggers[i];
            if pressed {
                if ray_hit.is_some() {
                    restart_requested = true;
                } else if self.round.is_some_and(|r| !r.finished()) {
                    self.bank(scene, i);
                }
            }
            let [crossed, touch] = input.hands[i]
                .map(|pose| {
                    webgpu_wasm::xr_buttons::blade_panel_contact(
                        self.previous[i],
                        pose,
                        [Vec3::new(0., 0., -0.1), Vec3::new(0., 0., -0.95)],
                        Vec3::ONE,
                        panel,
                        [0.5, 0.22],
                        dt,
                    )
                })
                .unwrap_or([false; 2]);
            if crossed && !self.button_touch[i] {
                restart_requested = true;
            }
            self.button_touch[i] = touch;
        }
        if restart_requested {
            self.start(scene)?;
            self.player.update_head(scene, input.head)?;
        }
        let was_finished = self.round.is_some_and(|r| r.finished());
        if let Some(round) = &mut self.round {
            round.advance(dt);
        }
        if !was_finished && self.round.is_some_and(|r| r.finished()) {
            self.score.bank_all();
            self.display.sync_chains(scene, &self.score.chains);
            if let Err(e) = self.scores.record(self.score.score) {
                log::warn!("TOP5 save failed: {e}");
            }
            self.refresh_scoreboard(scene)?;
            if let Some(head) = input.head {
                let position = head.w_axis.truncate();
                let forward = (-head.z_axis.truncate()).with_y(0.).normalize_or_zero();
                let p = position + forward * 0.95;
                scene.set_object_position(
                    restart,
                    Vec3::new(p.x, (position.y - 0.35).max(0.85), p.z),
                );
                scene.set_object_rotation(
                    restart,
                    Vec3::new(0., (-forward.x).atan2(-forward.z), 0.),
                );
            }
        }
        self.display.advance(scene, dt);
        if !self.round.is_some_and(|r| r.finished()) {
            let elapsed = 60. - self.round.map_or(60., |r| r.remaining);
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
        for i in 0..2 {
            let report = self
                .player
                .update_hand(scene, i, input.hands[i], true, dt)?;
            if !report.removed.is_empty() {
                self.haptics[i] = 0.6;
                let mut rewards = Vec::new();
                if !self.round.is_some_and(|r| r.finished()) {
                    self.cuts = self.cuts.saturating_add(report.removed.len() as u64);
                    for &original in &report.removed {
                        if let Some(accepted) = self.score.hit_chain(original) {
                            rewards.push((original, self.score.last_chain_total_points()));
                            if !accepted {
                                self.display.notice(scene, "PLNO".into());
                            }
                        }
                    }
                    self.display.sync_chains(scene, &self.score.chains);
                    self.refresh_indicators(scene);
                }
                for &piece in &report.pieces {
                    scene.set_emission(piece, 1.1);
                    self.fragments.insert(piece);
                }
                for pieces in report.pieces.chunks(32).filter(|p| p.len() >= 2) {
                    scene.play_cut_feedback_scored(pieces, false, 0.15, &rewards)?;
                    // Original web presentation: physical halves fall/spin until
                    // the feedback pool retires them; no immediate dissolve.
                }
            }
        }
        self.previous = input.hands;
        self.flying.retain(|id| scene.object_exists(*id));
        self.fragments.retain(|id| scene.object_exists(*id));
        self.glints.retain(|id| scene.object_exists(*id));
        self.score.targets.retain(|id, _| scene.object_exists(*id));
        self.update_panels(scene);
        Ok(())
    }
    pub fn spawn(
        &mut self,
        scene: &mut SceneManager,
        recipes: &[Recipe],
        glints: bool,
    ) -> Result<usize, String> {
        self.flying.retain(|id| scene.object_exists(*id));
        if recipes.len() + self.flying.len() > 10 {
            return Ok(0);
        }
        for (index, r) in recipes.iter().enumerate() {
            let fruit = self.catalog.get(&r.fruit).ok_or("Unknown Neon fruit")?;
            let cursor = self.cursors.entry(r.fruit.clone()).or_default();
            let variant = &fruit.variants[*cursor % fruit.variants.len()];
            *cursor = (*cursor + 1) % fruit.variants.len();
            let id = scene.add_mesh_variant(fruit.asset, variant.variant)?;
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
            scene.set_cuttable(
                id,
                true,
                if self.solid {
                    Vec3::from_array(fruit.cap_color) * 0.22
                } else {
                    Vec3::ZERO
                },
            )?;
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
            if glints {
                if let Some(asset) = self.refs["launchGlintAssets"]
                    .get(index % 3)
                    .and_then(Value::as_u64)
                {
                    for sign in [-1., 1.] {
                        let glint = scene.add_mesh_variant(asset as u32, 0)?;
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
        Ok(recipes.len())
    }
    fn bank(&mut self, scene: &mut SceneManager, index: usize) {
        if self.score.bank_chain(index).is_some() {
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
                .map(|(i, s)| format!("{}. {s}", i + 1))
                .collect()
        };
        for (i, label) in labels.iter().enumerate() {
            let id = scene.add_text_panel(label, &[], 1.22, 0.16)?;
            scene.set_object_position(id, Vec3::new(1.6, 1.94 - i as f32 * 0.22, -2.2));
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
        json!({"active":self.active,"round":self.round.map(|r|json!({"duration":r.duration,"remaining":r.remaining,"finished":r.finished()})),"score":self.score.score,"cuts":self.cuts,"shots":self.shots,"cueIndex":self.cue_index,"flying":self.flying,"fragments":self.fragments.len(),"stocks":self.score.chains.iter().map(|c|c.as_ref().map(|c|json!({"kind":c.kind,"count":c.count,"points":c.total_points()}))).collect::<Vec<_>>(),"top5":self.scores.top})
    }
}
fn id(v: &Value) -> Result<u32, String> {
    v.as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| format!("Missing native scene id: {v}"))
}
