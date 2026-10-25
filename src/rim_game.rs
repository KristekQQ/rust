//! Aurelia encounter and equipment simulation. Browser code only supplies input and worker messages.
use crate::{
    rim_movement::{Player, Shape, World},
    scene::SceneManager,
};
use glam::{Mat4, Quat, Vec2, Vec3};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
const HIDDEN: Vec3 = Vec3::new(0., -10000., 0.);
#[derive(Deserialize)]
pub struct EnemySpec {
    pub id: u32,
    pub kind: String,
    pub height: f32,
    pub offset: [f32; 3],
    pub scale: [f32; 3],
}
#[derive(Deserialize)]
pub struct Config {
    pub enemies: Vec<EnemySpec>,
    pub swords: [u32; 2],
    pub bolts: Vec<u32>,
    pub beams: Vec<u32>,
    pub charges: Vec<u32>,
    #[serde(default, rename = "healthBars")]
    pub health_bars: Vec<[u32; 2]>,
    pub hud: Vec<u32>,
    pub marker: u32,
    pub shapes: Vec<Shape>,
    pub obstacles: Vec<[f32; 4]>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Input {
    pub start: bool,
    pub pause: Option<bool>,
    pub reset: bool,
    pub movement: Option<[f32; 2]>,
    pub sprint: Option<bool>,
    pub aim: Option<bool>,
    pub dash: bool,
    pub slash: Option<usize>,
    pub look: Option<[f32; 2]>,
    pub teleport: Option<[f32; 3]>,
    pub death_mode: Option<String>,
}
struct Enemy {
    spec: EnemySpec,
    position: Vec3,
    yaw: f32,
    phase: u8,
    age: f32,
    cut_idle: f32,
    shot: f32,
    salvos: u32,
    beam_age: f32,
    beam_from: Vec3,
    beam_target: Vec3,
    waypoint: Option<Vec2>,
    nav_age: f32,
    serial: u32,
    hp: f32,
    max_hp: f32,
    contacts: [bool; 2],
    cooldowns: [f32; 2],
    last_strikes: [u64; 2],
}
#[derive(Clone, Copy)]
struct Blade {
    a: Vec3,
    b: Vec3,
    grip: Vec3,
    rotation: Quat,
}
struct Bolt {
    id: u32,
    position: Vec3,
    velocity: Vec3,
    owner: usize,
    life: f32,
    friendly: bool,
}
struct CutTask {
    model: crate::gltf_model::ModelInstance,
    root: Mat4,
    parts: Vec<(usize, usize)>,
    triangle_offset: usize,
    normal: Vec3,
    point: Vec3,
    owner: usize,
    serial: u32,
}
#[derive(Clone, Copy)]
struct Pending {
    owner: usize,
    serial: u32,
    normal: Vec3,
}
const PREPARE_TRIANGLES: usize = 3500;
const INSTALL_VERTICES: usize = 9000;
struct CompletedCut {
    pending: Pending,
    bytes: Vec<u8>,
    counts: [usize; 2],
    side: usize,
    offset: usize,
}
struct PieceMotion {
    direction: Vec3,
    age: f32,
}
struct StagedCut {
    pending: Pending,
    ids: Vec<(u32, usize)>,
    uploaded: BTreeSet<u32>,
}
type ConfigIds = ([u32; 2], Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>, u32);

pub struct Game {
    pub player: Player,
    world: World,
    config_ids: ConfigIds,
    enemies: Vec<Enemy>,
    health_bars: Vec<[u32; 2]>,
    obstacles: Vec<[f32; 4]>,
    corners: Vec<Vec2>,
    edges: Vec<Vec<f32>>,
    active: bool,
    paused: bool,
    wave: usize,
    health: f32,
    kills: u32,
    deaths: u32,
    hits: u32,
    dead_age: f32,
    age: f32,
    axis: [f32; 2],
    sprint: bool,
    aim: bool,
    dash: bool,
    swing: [f32; 2],
    strikes: [u64; 2],
    blades: [Option<Blade>; 2],
    bolts: Vec<Bolt>,
    fired: u32,
    reflected: u32,
    returned: u32,
    blocked: u32,
    frame: u64,
    visual_frame: u64,
    death_mode: String,
    tasks: VecDeque<CutTask>,
    pending: BTreeMap<u32, Pending>,
    completed: VecDeque<CompletedCut>,
    staged: Vec<StagedCut>,
    abandoned: Vec<u32>,
    prepared_triangles: usize,
    staged_vertices: usize,
    commits: u32,
    stage_ms: f64,
    stage_max_ms: f64,
    token: u32,
    pieces: Vec<u32>,
    piece_motion: BTreeMap<u32, PieceMotion>,
    cut_errors: u32,
    blade_contacts: u64,
    blade_hits: u64,
    blade_slow: u64,
    blade_breaks: u64,
    blade_speed: f32,
    xr: bool,
    pub xr_origin: Mat4,
    xr_aim: bool,
    xr_dash: bool,
    turn_latched: bool,
    pub sim_dt: f32,
}
impl Game {
    pub fn new(config: Config, scene: &mut SceneManager) -> Result<Self, String> {
        if config.enemies.len() != 14
            || config.bolts.len() != 32
            || config.beams.len() != 14
            || config.charges.len() != 14
            || config.hud.len() != 5
            || (!config.health_bars.is_empty() && config.health_bars.len() != config.enemies.len())
            || [("drone-orbiter", 8), ("core-tank", 3), ("blade-wraith", 3)]
                .iter()
                .any(|(kind, count)| {
                    config.enemies.iter().filter(|e| e.kind == *kind).count() != *count
                })
        {
            return Err(
                "Aurelia needs 8/3/3 enemies, 32 bolts, 14 beams/charges, five HUD parts and paired optional health bars"
                    .into(),
            );
        }
        let mut ids = Vec::new();
        for id in config
            .enemies
            .iter()
            .map(|e| e.id)
            .chain(config.swords)
            .chain(config.bolts.iter().copied())
            .chain(config.beams.iter().copied())
            .chain(config.charges.iter().copied())
            .chain(config.hud.iter().copied())
            .chain(config.health_bars.iter().flatten().copied())
            .chain([config.marker])
        {
            if ids.contains(&id) || !scene.object_exists(id) {
                return Err("Distinct scene-owned Aurelia objects required".into());
            }
            ids.push(id);
        }
        if config.enemies.iter().any(|e| {
            !matches!(
                e.kind.as_str(),
                "drone-orbiter" | "core-tank" | "blade-wraith"
            ) || !e.height.is_finite()
                || e.height <= 0.
                || e.scale
                    .iter()
                    .chain(e.offset.iter())
                    .any(|v| !v.is_finite())
                || scene.model(e.id).is_none()
        }) || config
            .obstacles
            .iter()
            .any(|r| r.iter().any(|x| !x.is_finite()) || r[0] >= r[1] || r[2] >= r[3])
        {
            return Err("Invalid Aurelia enemy/navigation configuration".into());
        }
        let world = World::new(config.shapes)?;
        let obstacles: Vec<_> = config
            .obstacles
            .iter()
            .map(|r| [r[0] - 0.55, r[1] + 0.55, r[2] - 0.55, r[3] + 0.55])
            .collect();
        let mut corners = Vec::new();
        for r in &obstacles {
            for x in [r[0] - 0.04, r[1] + 0.04] {
                for z in [r[2] - 0.04, r[3] + 0.04] {
                    let p = Vec2::new(x, z);
                    if free(p, &obstacles) {
                        corners.push(p);
                    }
                }
            }
        }
        let edges = corners
            .iter()
            .enumerate()
            .map(|(i, a)| {
                corners
                    .iter()
                    .enumerate()
                    .map(|(j, b)| {
                        if i != j && !blocked(*a, *b, &obstacles) {
                            a.distance(*b)
                        } else {
                            f32::INFINITY
                        }
                    })
                    .collect()
            })
            .collect();
        let enemies = config
            .enemies
            .into_iter()
            .map(|spec| Enemy {
                hp: max_hp(&spec.kind),
                max_hp: max_hp(&spec.kind),
                contacts: [false; 2],
                cooldowns: [0.; 2],
                last_strikes: [0; 2],
                spec,
                position: HIDDEN,
                yaw: 0.,
                phase: 0,
                age: 0.,
                cut_idle: 0.,
                shot: 0.,
                salvos: 0,
                beam_age: -1.,
                beam_from: Vec3::ZERO,
                beam_target: Vec3::ZERO,
                waypoint: None,
                nav_age: 0.,
                serial: 0,
            })
            .collect();
        let mut g = Self {
            player: Player::new(Vec3::new(0., 0., 42.)),
            world,
            config_ids: (
                config.swords,
                config.bolts,
                config.beams,
                config.charges,
                config.hud,
                config.marker,
            ),
            enemies,
            health_bars: config.health_bars,
            obstacles,
            corners,
            edges,
            active: false,
            paused: false,
            wave: 0,
            health: 100.,
            kills: 0,
            deaths: 0,
            hits: 0,
            dead_age: 0.,
            age: 0.,
            axis: [0.; 2],
            sprint: false,
            aim: false,
            dash: false,
            swing: [-1.; 2],
            strikes: [0; 2],
            blades: [None; 2],
            bolts: Vec::new(),
            fired: 0,
            reflected: 0,
            returned: 0,
            blocked: 0,
            frame: 0,
            visual_frame: 0,
            death_mode: "cut".into(),
            tasks: VecDeque::new(),
            pending: BTreeMap::new(),
            completed: VecDeque::new(),
            staged: Vec::new(),
            abandoned: Vec::new(),
            prepared_triangles: 0,
            staged_vertices: 0,
            commits: 0,
            stage_ms: 0.,
            stage_max_ms: 0.,
            token: 0,
            pieces: Vec::new(),
            piece_motion: BTreeMap::new(),
            cut_errors: 0,
            blade_contacts: 0,
            blade_hits: 0,
            blade_slow: 0,
            blade_breaks: 0,
            blade_speed: 0.,
            xr: false,
            xr_origin: Mat4::IDENTITY,
            xr_aim: false,
            xr_dash: false,
            turn_latched: false,
            sim_dt: 0.,
        };
        for id in ids {
            scene.set_object_position(id, HIDDEN);
        }
        for e in &g.enemies {
            hide_enemy(scene, e.spec.id);
        }
        g.sync(scene, None);
        Ok(g)
    }
    /// Release pooled visuals and asynchronous work before removing this encounter.
    pub fn teardown(&mut self, scene: &mut SceneManager) {
        self.active = false;
        self.tasks.clear();
        self.pending.clear();
        self.completed.clear();
        self.clear_staged(scene);
        self.piece_motion.clear();
        for id in self.pieces.drain(..) {
            scene.remove_object(id);
        }
        self.bolts.clear();
        self.blades = [None; 2];
        for e in &mut self.enemies {
            e.phase = 0;
            e.beam_age = -1.;
            e.serial = e.serial.wrapping_add(1);
            e.hp = e.max_hp;
            e.contacts = [false; 2];
            e.cooldowns = [0.; 2];
            e.last_strikes = [0; 2];
            hide_enemy(scene, e.spec.id);
        }
        for id in self.health_bars.iter().flatten() {
            scene.set_object_position(*id, HIDDEN);
        }
        for id in self
            .config_ids
            .0
            .iter()
            .chain(&self.config_ids.1)
            .chain(&self.config_ids.2)
            .chain(&self.config_ids.3)
            .chain(&self.config_ids.4)
            .chain(std::iter::once(&self.config_ids.5))
        {
            scene.set_object_position(*id, HIDDEN);
        }
    }
    pub fn input(&mut self, input: Input, scene: &mut SceneManager) -> Result<(), String> {
        if input
            .movement
            .is_some_and(|a| a.iter().any(|v| !v.is_finite() || v.abs() > 1.))
            || input
                .look
                .is_some_and(|a| a.iter().any(|v| !v.is_finite() || v.abs() > 10000.))
            || input
                .teleport
                .is_some_and(|a| a.iter().any(|v| !v.is_finite()))
            || input.slash.is_some_and(|h| h > 1)
            || input
                .death_mode
                .as_ref()
                .is_some_and(|s| !matches!(s.as_str(), "cut" | "dissolve"))
        {
            return Err("Invalid Aurelia input".into());
        }
        if input.reset {
            self.axis = [0.; 2];
            self.aim = false;
            self.dash = false;
            self.blades = [None; 2];
            self.player.reset_input();
        }
        if let Some(v) = input.pause {
            self.paused = v;
            self.blades = [None; 2];
        }
        if let Some(v) = input.movement {
            self.axis = v;
        }
        if let Some(v) = input.sprint {
            self.sprint = v;
        }
        if let Some(v) = input.aim {
            self.aim = v;
        }
        self.dash |= input.dash;
        if let Some([x, y]) = input.look {
            self.player.yaw -= x * 0.003;
            self.player.pitch = (self.player.pitch - y * 0.003).clamp(-1.35, 1.35);
        }
        if let Some(mode) = input.death_mode {
            self.death_mode = mode;
        }
        if let Some(p) = input.teleport {
            let before = self.player.position;
            if !self.player.teleport(&self.world, Vec3::from_array(p)) {
                return Err("Destination is obstructed".into());
            }
            if self.xr {
                self.xr_origin.w_axis += (self.player.position - before).extend(0.);
            }
            self.blades = [None; 2];
        }
        if input.start {
            self.paused = false;
            if !self.active {
                self.active = true;
                self.spawn(scene);
            }
        }
        if self.active && self.health > 0. {
            if let Some(hand) = input.slash {
                if self.swing[hand] < 0. {
                    self.swing[hand] = 0.;
                    self.strikes[hand] = self.strikes[hand].wrapping_add(1);
                }
            }
        }
        Ok(())
    }
    pub fn camera(&self, aspect: f32) -> (Mat4, Vec3) {
        let p = self.player.head(1.7);
        let proj =
            glam::camera::rh::proj::directx::perspective(65f32.to_radians(), aspect, 0.08, 350.);
        (
            proj * glam::camera::rh::view::look_at_mat4(p, p + self.player.forward(), Vec3::Y),
            p,
        )
    }
    pub fn playing(&self) -> bool {
        self.active && !self.paused
    }
    pub fn set_xr(&mut self, value: bool) {
        self.xr = value;
        self.blades = [None; 2];
        self.axis = [0.; 2];
        self.aim = false;
        self.player.reset_input();
        self.xr_dash = true;
        if value {
            self.xr_origin = Mat4::from_translation(self.player.position)
                * Mat4::from_rotation_y(self.player.yaw);
        }
    }
    pub fn xr_buttons(&mut self, aim: bool, dash: bool) {
        self.xr_aim = aim;
        self.dash |= dash && !self.xr_dash;
        self.xr_dash = dash;
    }
    pub fn xr_move(
        &mut self,
        head: Option<Mat4>,
        ray: Option<Mat4>,
        axes: [f32; 3],
        cancel: bool,
        dt: f32,
    ) {
        if !self.playing() {
            self.sim_dt = 0.;
            return;
        }
        let Some(head) = head else {
            self.player.reset_input();
            self.blades = [None; 2];
            self.sim_dt = 0.;
            return;
        };
        if !head.is_finite() || !axes.iter().all(|x| x.is_finite()) {
            return;
        }
        let physical = head.w_axis.truncate();
        let old_head = self.xr_origin.transform_point3(physical);
        let physical_delta = Vec3::new(
            old_head.x - self.player.position.x,
            0.,
            old_head.z - self.player.position.z,
        );
        self.player.position = self
            .world
            .move_body(
                self.player.position,
                physical_delta,
                physical.y.clamp(0.6, 2.5),
            )
            .position;
        if axes[2].abs() > 0.65 && !self.turn_latched {
            let old = self.player.yaw;
            self.player.yaw -= axes[2].signum() * std::f32::consts::FRAC_PI_6;
            let _ = old;
            self.blades = [None; 2];
            self.turn_latched = true;
        }
        if axes[2].abs() < 0.25 {
            self.turn_latched = false;
        }
        let yaw = self.player.yaw;
        let forward = -(Mat4::from_rotation_y(yaw) * head).z_axis.truncate();
        self.player.yaw = (-forward.x).atan2(-forward.z);
        let origin = Mat4::from_rotation_y(yaw);
        let current_origin = Mat4::from_translation(
            self.player.position - origin.transform_vector3(Vec3::new(physical.x, 0., physical.z)),
        ) * origin;
        let ray = ray.filter(|matrix| matrix.is_finite());
        let aim_ray = ray.map(|r| {
            let r = current_origin * r;
            (r.w_axis.truncate(), -r.z_axis.truncate())
        });
        let dash = std::mem::take(&mut self.dash);
        let stick = Vec2::new(
            if axes[0].abs() > 0.15 { axes[0] } else { 0. },
            if axes[1].abs() > 0.15 { -axes[1] } else { 0. },
        );
        let stick = stick / stick.length().max(1.) * (1.8 / 2.5);
        let alive = self.health > 0.;
        // Losing/cancelling a tracked ray must not look like a trigger release:
        // Player::step launches when an existing aim becomes false.
        let cancel_aim = cancel || ray.is_none() || !alive;
        if cancel_aim {
            self.player.aiming = false;
            self.player.target = None;
        }
        self.sim_dt = self.player.step(
            &self.world,
            dt,
            if alive { stick.to_array() } else { [0.; 2] },
            false,
            self.xr_aim && !cancel_aim,
            dash && alive && !cancel_aim,
            physical.y.clamp(0.6, 2.5),
            aim_ray,
        );
        self.player.yaw = yaw;
        let shift =
            self.player.position - origin.transform_vector3(Vec3::new(physical.x, 0., physical.z));
        self.xr_origin = Mat4::from_translation(shift) * origin;
    }
    pub fn tick(
        &mut self,
        scene: &mut SceneManager,
        real_dt: f32,
        head: Option<Mat4>,
        hands: [Option<Mat4>; 2],
    ) -> f32 {
        let stage_start = cut_clock();
        if self.advance_cuts(scene).is_err() {
            self.cut_failed();
            self.clear_staged(scene);
        }
        self.stage_ms = cut_clock() - stage_start;
        self.stage_max_ms = self.stage_max_ms.max(self.stage_ms);
        let dt = real_dt.clamp(0., 0.05);
        if !dt.is_finite() || dt <= 0. {
            self.blades = [None; 2];
            return 0.;
        }
        if !self.playing() {
            self.sync(scene, head);
            return dt;
        }
        let sim = if self.xr {
            self.sim_dt
        } else {
            let dash = std::mem::take(&mut self.dash);
            self.player.step(
                &self.world,
                dt,
                if self.health > 0. { self.axis } else { [0.; 2] },
                self.sprint,
                self.aim && self.health > 0.,
                dash && self.health > 0.,
                1.7,
                None,
            )
        };
        self.sim_dt = sim;
        if self.xr && head.is_none() {
            self.blades = [None; 2];
            return 0.;
        }
        self.advance_piece_motion(scene, sim);
        self.frame += 1;
        self.age += sim;
        let head = head.unwrap_or_else(|| {
            let (_, p) = self.camera(1.);
            Mat4::from_rotation_translation(
                Quat::from_rotation_y(self.player.yaw) * Quat::from_rotation_x(self.player.pitch),
                p,
            )
        });
        let hp = head.w_axis.truncate();
        if self.health <= 0. {
            self.dead_age += dt;
            if self.dead_age >= 3. {
                self.health = 100.;
                self.spawn(scene);
            }
            self.sync(scene, Some(head));
            self.visual_frame = self.frame;
            return sim;
        }
        let samples = self.sample_blades(scene, head, hands, dt);
        self.blade_speed = 0.;
        let mut sword_hits = Vec::new();
        let mut shots = Vec::new();
        let mut damage = 0.;
        for i in 0..self.enemies.len() {
            if self.enemies[i].phase == 0 {
                continue;
            }
            if self.enemies[i].phase == 3 {
                self.enemies[i].age += sim;
                self.enemies[i].cut_idle += dt;
                let mut cutting = self.tasks.iter().any(|t| t.owner == i)
                    || self.pending.values().any(|p| p.owner == i)
                    || self.completed.iter().any(|c| c.pending.owner == i)
                    || self.staged.iter().any(|s| s.pending.owner == i);
                // Lost worker replies or GPU readiness must not hold a wave forever.
                // This measures active real time, independently of bullet time.
                let expired = cutting && self.enemies[i].cut_idle > 10.;
                if expired {
                    self.expire_cut(scene, i);
                    cutting = false;
                }
                let e = &mut self.enemies[i];
                if expired || (e.age > 3.2 && !cutting) {
                    e.phase = 0;
                    hide_enemy(scene, e.spec.id);
                }
                continue;
            }
            self.move_enemy(scene, i, hp, sim);
            let e = &mut self.enemies[i];
            e.yaw = (hp.x - e.position.x).atan2(hp.z - e.position.z);
            if e.spec.kind != "blade-wraith" {
                if e.beam_age >= 0. {
                    e.beam_age += sim;
                    let end = self
                        .world
                        .cast(e.beam_from, e.beam_target)
                        .map_or(e.beam_target, |h| h.point);
                    let active_overlap =
                        (e.beam_age.min(2.85) - (e.beam_age - sim).max(0.85)).max(0.);
                    let direction = (end - e.beam_from).normalize_or_zero();
                    let length = end.distance(e.beam_from);
                    let radius = if e.spec.kind == "core-tank" {
                        0.34
                    } else {
                        0.23
                    };
                    let hit = [0., -0.3, -0.6, -0.9].into_iter().any(|y| {
                        let p = hp + Vec3::Y * y;
                        let t = (p - e.beam_from).dot(direction);
                        t >= 0.
                            && t <= length
                            && p.distance(e.beam_from + direction * t) < radius + 0.23
                            && self.world.cast(e.beam_from, p).is_none()
                    });
                    if hit {
                        damage += active_overlap
                            * if e.spec.kind == "core-tank" {
                                170.
                            } else {
                                125.
                            };
                    }
                    if e.beam_age >= 2.85 {
                        e.beam_age = -1.;
                        play(scene, e, "Idle", true);
                    }
                } else {
                    e.shot -= sim;
                    if e.shot <= 0. {
                        let center = e.position + Vec3::Y * e.spec.height * 0.5;
                        let target = hp - Vec3::Y * 0.25;
                        let from =
                            center + (target - center).normalize_or_zero() * e.spec.height * 0.48;
                        if center.distance(target) < 12. && self.world.cast(from, target).is_none()
                        {
                            if e.salvos.is_multiple_of(2) {
                                e.beam_age = 0.;
                                e.beam_from = from;
                                e.beam_target = from + (target - from).normalize_or_zero() * 18.;
                            } else {
                                shots.push((i, from, target));
                            }
                            e.salvos += 1;
                            e.shot = if e.spec.kind == "core-tank" { 2.8 } else { 3.2 }
                                + (i % 3) as f32 * 0.25;
                            play(scene, e, "Attack4", false);
                        } else {
                            e.shot = 0.45;
                        }
                    }
                }
            }
            scene.set_object_transform(
                e.spec.id,
                e.position + Quat::from_rotation_y(e.yaw) * Vec3::from_array(e.spec.offset),
                Vec3::new(0., e.yaw, 0.),
                Vec3::from_array(e.spec.scale),
            );
            for (hand, sample) in samples.iter().enumerate() {
                e.cooldowns[hand] = (e.cooldowns[hand] - real_dt).max(0.);
                let Some((old, new, can_cut)) = sample else {
                    e.contacts[hand] = false;
                    continue;
                };
                let contact = swept_enemy(*old, *new, e);
                let travel = old.a.distance(new.a).max(old.b.distance(new.b));
                // Physical speed, not a per-frame distance: identical swings must
                // remain eligible at 72/90/120 Hz. Contact state and cooldown
                // suppress repeats while resting inside an opponent.
                let speed = travel / real_dt;
                self.blade_speed = self.blade_speed.max(speed);
                let eligible = *can_cut && speed > 0.3;
                if contact {
                    self.blade_contacts += 1;
                    if !eligible {
                        self.blade_slow += 1;
                    }
                }
                if blade_damage_allowed(
                    e,
                    hand,
                    contact,
                    eligible,
                    (!self.xr).then_some(self.strikes[hand]),
                ) {
                    self.blade_hits += 1;
                    let edge = new.b - new.a;
                    let movement = (new.a + new.b - old.a - old.b) * 0.5;
                    let normal = edge.cross(movement).try_normalize().unwrap_or(Vec3::Y);
                    sword_hits.push((i, normal));
                }
            }
        }
        if damage > 0. {
            self.damage(damage);
        }
        for (i, from, target) in shots {
            self.shoot(i, from, target);
        }
        for (i, n) in sword_hits {
            self.hurt_enemy(scene, i, 50., n);
        }
        self.update_bolts(scene, sim, hp, &samples);
        if self.enemies.iter().all(|e| e.phase == 0) {
            self.wave += 1;
            self.spawn(scene);
        }
        self.sync(scene, Some(head));
        self.visual_frame = self.frame;
        sim
    }
    fn damage(&mut self, amount: f32) {
        if self.health <= 0. {
            return;
        }
        self.health = (self.health - amount).max(0.);
        self.hits += 1;
        if self.health <= 0. {
            self.deaths += 1;
            self.dead_age = 0.;
            self.bolts.clear();
            self.aim = false;
            self.player.reset_input();
            for e in &mut self.enemies {
                e.beam_age = -1.;
            }
        }
    }
    fn spawn(&mut self, scene: &mut SceneManager) {
        self.tasks.clear();
        self.pending.clear();
        self.completed.clear();
        self.clear_staged(scene);
        self.piece_motion.clear();
        for id in self.pieces.drain(..) {
            scene.remove_object(id);
        }
        self.bolts.clear();
        self.blades = [None; 2];
        for e in &mut self.enemies {
            e.phase = 0;
            e.beam_age = -1.;
            e.serial = e.serial.wrapping_add(1);
            e.hp = e.max_hp;
            e.contacts = [false; 2];
            e.cooldowns = [0.; 2];
            e.last_strikes = [0; 2];
            hide_enemy(scene, e.spec.id);
        }
        let (kind, count) = match self.wave % 6 {
            0 => ("drone-orbiter", 6),
            1 => ("core-tank", 2),
            2 => ("blade-wraith", 2),
            3 => ("drone-orbiter", 8),
            4 => ("core-tank", 3),
            _ => ("blade-wraith", 3),
        };
        let head = self.player.head(1.7);
        let f = self.player.forward();
        let f = Vec2::new(f.x, f.z).normalize_or_zero();
        let mut slot = 0;
        for e in &mut self.enemies {
            if e.spec.kind != kind || slot >= count {
                continue;
            }
            let angle = (slot as f32 - (count - 1) as f32 * 0.5) * 0.32;
            let f = Vec2::from_angle(-angle).rotate(f);
            let mut chosen = None;
            'search: for distance in [8., 6., 10., 4., 12.] {
                for extra in [
                    0.,
                    0.25,
                    -0.25,
                    0.65,
                    -0.65,
                    1.2,
                    -1.2,
                    std::f32::consts::PI,
                ] {
                    let d = Vec2::from_angle(extra).rotate(f);
                    let p = Vec2::new(head.x, head.z) + d * distance;
                    if free(p, &self.obstacles) {
                        chosen = Some(p);
                        break 'search;
                    }
                }
            }
            if let Some(p) = chosen {
                e.position = Vec3::new(
                    p.x,
                    if kind == "blade-wraith" {
                        0.13
                    } else {
                        (head.y - 0.2 - e.spec.height * 0.5).max(0.6)
                    },
                    p.y,
                );
                e.phase = 1;
                e.age = 0.;
                e.shot = 1.8 + slot as f32 * 0.8;
                e.salvos = 0;
                e.waypoint = None;
                e.nav_age = 1.;
                e.yaw = 0.;
                play(
                    scene,
                    e,
                    if kind == "blade-wraith" {
                        "Run"
                    } else {
                        "Idle"
                    },
                    true,
                );
                scene.set_object_position(e.spec.id, e.position + Vec3::from_array(e.spec.offset));
            }
            slot += 1;
        }
    }
    fn move_enemy(&mut self, scene: &mut SceneManager, i: usize, head: Vec3, dt: f32) {
        let e = &mut self.enemies[i];
        if e.beam_age >= 0. {
            return;
        }
        let flying = e.spec.kind != "blade-wraith";
        let p = Vec2::new(e.position.x, e.position.z);
        let h = Vec2::new(head.x, head.z);
        let distance = p.distance(h);
        let goal = if flying {
            let direction = (p - h).normalize_or_zero();
            h + direction * 5.3
        } else {
            h
        };
        if flying {
            let desired = (head.y - 0.2 - e.spec.height * 0.5).max(0.6)
                + (self.age * 1.7 + i as f32).sin() * 0.13;
            e.position.y += (desired - e.position.y).clamp(-dt, dt);
        } else if distance < 1.25 {
            if e.phase != 2 {
                e.phase = 2;
                play(scene, e, "Attack", true);
            }
            return;
        } else if e.phase != 1 {
            e.phase = 1;
            play(scene, e, "Run", true);
        }
        e.nav_age += dt;
        if e.waypoint.is_none_or(|w| p.distance(w) < 0.08) || e.nav_age > 0.3 {
            e.waypoint = waypoint(p, goal, &self.obstacles, &self.corners, &self.edges);
            e.nav_age = 0.;
        }
        if let Some(w) = e.waypoint {
            let delta = w - p;
            let step = (dt * if flying { 2.2 } else { 1.35 })
                .min(delta.length())
                .min(if flying {
                    100.
                } else {
                    (distance - 1.2).max(0.)
                });
            let direction = delta.normalize_or_zero();
            let next = e.position + Vec3::new(direction.x, 0., direction.y) * step;
            let from = e.position + Vec3::Y * e.spec.height * 0.5;
            let to = next + Vec3::Y * e.spec.height * 0.5;
            if self.world.cast(from, to).is_none() {
                e.position = next;
            }
        }
    }
    fn sample_blades(
        &mut self,
        scene: &mut SceneManager,
        head: Mat4,
        hands: [Option<Mat4>; 2],
        dt: f32,
    ) -> [Option<(Blade, Blade, bool)>; 2] {
        std::array::from_fn(|h| {
            if self.swing[h] >= 0. {
                self.swing[h] += dt;
                if self.swing[h] > 0.38 {
                    self.swing[h] = -1.;
                }
            }
            let side = if h == 0 { -1. } else { 1. };
            let w = if self.swing[h] < 0. {
                0.
            } else {
                (std::f32::consts::PI * self.swing[h] / 0.38).sin()
            };
            let matrix = if self.xr {
                let Some(matrix) = hands[h] else {
                    self.blades[h] = None;
                    return None;
                };
                matrix
            } else {
                head * Mat4::from_rotation_translation(
                    Quat::from_euler(
                        glam::EulerRot::YXZ,
                        side * (0.07 + 0.95 * w),
                        -0.08 - 0.28 * w,
                        side * 0.12 * w,
                    ),
                    Vec3::new(
                        side * 0.31 - side * 0.32 * w,
                        -0.29 + 0.13 * w,
                        -0.36 - 0.18 * w,
                    ),
                )
            };
            let (scale, q, p) = matrix.to_scale_rotation_translation();
            let (y, x, z) = q.to_euler(glam::EulerRot::YXZ);
            scene.set_object_transform(self.config_ids.0[h], p, Vec3::new(x, y, z), scale);
            let new = Blade {
                a: matrix.transform_point3(Vec3::new(0., 0., -0.14)),
                b: matrix.transform_point3(Vec3::new(0., 0., -1.02)),
                grip: p,
                rotation: q,
            };
            let old = self.blades[h].replace(new)?;
            if real_continuity(old, new, dt) {
                Some((old, new, self.xr || self.swing[h] >= 0.))
            } else {
                self.blade_breaks += 1;
                None
            }
        })
    }
    fn shoot(&mut self, owner: usize, from: Vec3, to: Vec3) {
        if self.bolts.len() >= 32 {
            return;
        }
        if let Some(&id) = self
            .config_ids
            .1
            .iter()
            .find(|id| !self.bolts.iter().any(|b| b.id == **id))
        {
            self.bolts.push(Bolt {
                id,
                position: from,
                velocity: (to - from).normalize_or_zero() * 3.2,
                owner,
                life: 8.,
                friendly: false,
            });
            self.fired += 1;
        }
    }
    fn update_bolts(
        &mut self,
        scene: &mut SceneManager,
        dt: f32,
        head: Vec3,
        blades: &[Option<(Blade, Blade, bool)>; 2],
    ) {
        let mut defeats = Vec::new();
        let mut damage = 0.;
        for b in &mut self.bolts {
            if b.life <= 0. {
                continue;
            }
            let from = b.position;
            let to = from + b.velocity * dt;
            let mut nearest = self
                .world
                .cast(from, to)
                .map(|h| (h.fraction, 0usize, 0usize));
            let mut consider = |t: Option<f32>, kind: usize, index: usize| {
                if let Some(t) = t {
                    if nearest.is_none_or(|h| t < h.0 - 1e-7) {
                        nearest = Some((t, kind, index));
                    }
                }
            };
            if b.friendly {
                for (i, e) in self.enemies.iter().enumerate() {
                    if e.phase == 1 || e.phase == 2 {
                        consider(
                            sphere_time(
                                from,
                                to,
                                e.position + Vec3::Y * e.spec.height * 0.5,
                                if e.spec.kind == "blade-wraith" {
                                    0.705
                                } else {
                                    e.spec.height * 0.48 + 0.055
                                },
                            ),
                            2,
                            i,
                        );
                    }
                }
            } else {
                for (i, blade) in blades.iter().enumerate() {
                    if let Some((old, new, _)) = blade {
                        consider(blade_time(from, to, *old, *new), 1, i);
                    }
                }
                consider(sphere_time(from, to, head - Vec3::Y * 0.25, 0.355), 3, 0);
            }
            b.position = from.lerp(to, nearest.map_or(1., |h| h.0));
            b.life -= dt;
            if let Some((_, kind, index)) = nearest {
                match kind {
                    0 => {
                        b.life = 0.;
                        self.blocked += 1;
                    }
                    1 => {
                        let target = self
                            .enemies
                            .get(b.owner)
                            .filter(|e| e.phase == 1 || e.phase == 2)
                            .or_else(|| self.enemies.iter().find(|e| e.phase == 1 || e.phase == 2));
                        b.velocity = target
                            .map_or(-b.velocity, |e| {
                                e.position + Vec3::Y * e.spec.height * 0.5 - b.position
                            })
                            .normalize_or_zero()
                            * 5.5;
                        b.friendly = true;
                        self.reflected += 1;
                    }
                    2 => {
                        b.life = 0.;
                        defeats.push(index);
                        self.returned += 1;
                    }
                    _ => {
                        b.life = 0.;
                        damage += 25.;
                    }
                }
            }
        }
        self.bolts.retain(|b| b.life > 0.);
        for i in defeats {
            self.hurt_enemy(scene, i, 100., Vec3::Y);
        }
        if damage > 0. {
            self.damage(damage);
        }
    }
    fn hurt_enemy(&mut self, scene: &mut SceneManager, i: usize, amount: f32, normal: Vec3) {
        let e = &mut self.enemies[i];
        if !matches!(e.phase, 1 | 2) || !amount.is_finite() || amount <= 0. || e.hp <= 0. {
            return;
        }
        e.hp = (e.hp - amount).max(0.);
        if e.hp == 0. {
            self.defeat(scene, i, normal);
        }
    }
    fn defeat(&mut self, scene: &mut SceneManager, i: usize, normal: Vec3) {
        let e = &mut self.enemies[i];
        if e.phase == 0 || e.phase == 3 {
            return;
        }
        e.hp = 0.;
        e.phase = 3;
        e.age = 0.;
        e.cut_idle = 0.;
        e.beam_age = -1.;
        self.kills += 1;
        // Freeze the exact impact pose while the bounded worker transaction runs.
        // Explode moves individual nodes apart before slicing and destroys the silhouette.
        let _ = scene.pause_model_animation(e.spec.id, true);
        if let Some(model) = scene.model(e.spec.id) {
            let model = model.clone();
            let parts = model
                .pose
                .active_nodes
                .iter()
                .flat_map(|&node| {
                    model.asset.nodes[node]
                        .mesh
                        .map(|m| {
                            model.asset.meshes[m]
                                .primitives
                                .iter()
                                .map(move |&p| (node, p))
                        })
                        .into_iter()
                        .flatten()
                })
                .collect();
            self.tasks.push_back(CutTask {
                model,
                root: scene.objects()[&e.spec.id].transform.model(),
                parts,
                triangle_offset: 0,
                normal,
                point: e.position + Vec3::Y * e.spec.height * 0.5,
                owner: i,
                serial: e.serial,
            });
        }
    }
    pub fn take_cut_job(&mut self) -> Result<Vec<u8>, String> {
        let Some(task) = self.tasks.front_mut() else {
            return Ok(Vec::new());
        };
        let mut data = Vec::new();
        let mut triangles = 0;
        while triangles < PREPARE_TRIANGLES {
            let Some(&(node, primitive)) = task.parts.last() else {
                break;
            };
            let p = &task.model.asset.primitives[primitive];
            if p.mode != crate::gltf_model::PrimitiveMode::Triangles {
                task.parts.pop();
                task.triangle_offset = 0;
                continue;
            }
            let start = task.triangle_offset * 3;
            let end = (start + (PREPARE_TRIANGLES - triangles) * 3).min(p.indices.len() / 3 * 3);
            let vertices = crate::gltf_animation::deformed_cut_vertex_subset(
                &task.model.asset,
                &task.model.pose,
                node,
                primitive,
                p.indices[start..end].iter().copied(),
            )?;
            let matrix =
                task.root * crate::gltf_animation::node_render_matrix(&task.model.pose, node);
            let nm = matrix.inverse().transpose();
            let material = p.material.map(|m| &task.model.asset.materials[m]);
            for v in &vertices {
                let position = matrix.transform_point3(Vec3::from_array(v.position));
                let normal = nm
                    .transform_vector3(Vec3::from_array(v.normal))
                    .normalize_or_zero();
                let color = Vec3::from_slice(&v.color)
                    * material.map_or(Vec3::ONE, |m| Vec3::from_slice(&m.base_color_factor));
                data.extend_from_slice(&[
                    position.x, position.y, position.z, normal.x, normal.y, normal.z, color.x,
                    color.y, color.z,
                ]);
            }
            triangles += (end - start) / 3;
            if end == p.indices.len() / 3 * 3 {
                task.parts.pop();
                task.triangle_offset = 0;
            } else {
                task.triangle_offset = end / 3;
            }
        }
        self.prepared_triangles = triangles;
        self.token = self.token.wrapping_add(1);
        let token = self.token;
        self.enemies[task.owner].cut_idle = 0.;
        let normal = if self.death_mode == "cut" {
            task.normal
        } else {
            Vec3::Y
        };
        let point = if self.death_mode == "cut" {
            task.point
        } else {
            Vec3::new(0., -9999., 0.)
        };
        self.pending.insert(
            token,
            Pending {
                owner: task.owner,
                serial: task.serial,
                normal,
            },
        );
        let mut output = Vec::with_capacity(32 + data.len() * 4);
        output.extend_from_slice(&token.to_le_bytes());
        output.extend_from_slice(&(triangles as u32).to_le_bytes());
        for v in [normal.x, normal.y, normal.z, point.x, point.y, point.z]
            .into_iter()
            .chain(data)
        {
            output.extend_from_slice(&v.to_le_bytes());
        }
        if task.parts.is_empty() {
            self.tasks.pop_front();
        }
        Ok(output)
    }
    /// Worker callbacks only validate framing and enqueue; geometry work runs in tick.
    pub fn accept_cut(&mut self, _scene: &mut SceneManager, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() < 12 || bytes.len() > 16 * 1024 * 1024 || !bytes.len().is_multiple_of(4) {
            self.cut_failed();
            return Err("Invalid cut result".into());
        }
        let token = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let Some(pending) = self.pending.remove(&token) else {
            return Ok(());
        };
        let e = &self.enemies[pending.owner];
        if e.serial != pending.serial || e.phase != 3 {
            return Ok(());
        }
        let counts = [
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize,
            u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize,
        ];
        if counts
            .iter()
            .any(|n| !n.is_multiple_of(3) || *n > (bytes.len() - 12) / 36)
            || bytes.len() != 12 + (counts[0] + counts[1]) * 36
        {
            self.cut_failed();
            return Err("Invalid cut result length".into());
        }
        if !self
            .staged
            .iter()
            .any(|s| s.pending.owner == pending.owner && s.pending.serial == pending.serial)
        {
            self.staged.push(StagedCut {
                pending,
                ids: Vec::new(),
                uploaded: BTreeSet::new(),
            });
        }
        self.enemies[pending.owner].cut_idle = 0.;
        self.completed.push_back(CompletedCut {
            pending,
            bytes: bytes.to_vec(),
            counts,
            side: 0,
            offset: 0,
        });
        Ok(())
    }
    /// Prepared hidden meshes can be uploaded before their atomic publication.
    pub fn staged_cut_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.staged
            .iter()
            .flat_map(|s| s.ids.iter().map(|&(id, _)| id))
    }
    /// Called after renderer synchronization, only for meshes whose GPU upload succeeded.
    pub fn ack_cut_uploads(&mut self, ids: &[u32]) {
        for stage in &mut self.staged {
            for &id in ids {
                if stage.ids.iter().any(|&(staged, _)| staged == id) && stage.uploaded.insert(id) {
                    self.enemies[stage.pending.owner].cut_idle = 0.;
                }
            }
        }
    }
    fn clear_staged(&mut self, scene: &mut SceneManager) {
        for s in self.staged.drain(..) {
            for (id, _) in s.ids {
                scene.remove_object(id);
            }
        }
        for id in self.abandoned.drain(..) {
            scene.remove_object(id);
        }
    }
    fn advance_cuts(&mut self, scene: &mut SceneManager) -> Result<(), String> {
        self.staged_vertices = 0;
        if self
            .staged
            .iter()
            .flat_map(|s| &s.ids)
            .any(|&(id, _)| !scene.object_exists(id))
        {
            return Err("Staged cut object removed before publication".into());
        }
        for id in self.abandoned.drain(..) {
            scene.remove_object(id);
        }
        // Commit only stages completed on a previous tick: renderer gets a warmup frame.
        let mut i = 0;
        while i < self.staged.len() {
            let p = self.staged[i].pending;
            let valid =
                self.enemies[p.owner].serial == p.serial && self.enemies[p.owner].phase == 3;
            let busy = self
                .tasks
                .iter()
                .any(|t| t.owner == p.owner && t.serial == p.serial)
                || self
                    .pending
                    .values()
                    .any(|t| t.owner == p.owner && t.serial == p.serial)
                || self
                    .completed
                    .iter()
                    .any(|t| t.pending.owner == p.owner && t.pending.serial == p.serial);
            let uploaded = self.staged[i]
                .ids
                .iter()
                .all(|&(id, _)| self.staged[i].uploaded.contains(&id));
            if valid && (busy || !uploaded) {
                i += 1;
                continue;
            }
            let stage = self.staged.remove(i);
            if !valid {
                for (id, _) in stage.ids {
                    scene.remove_object(id);
                }
                continue;
            }
            // All fallible geometry creation has succeeded. Publish all halves together.
            for &(id, side) in &stage.ids {
                scene.set_object_position(id, Vec3::ZERO);
                let sign = if side == 0 { 1. } else { -1. };
                // Generic fading pieces deliberately skip projectile physics. Own this
                // visual motion here so both halves actually separate during dissolve.
                self.piece_motion.insert(
                    id,
                    PieceMotion {
                        direction: if self.death_mode == "cut" {
                            p.normal * sign
                        } else {
                            Vec3::ZERO
                        },
                        age: 0.,
                    },
                );
                scene.dissolve_cut_pieces(&[id], 1.26, 1.74)?;
                self.pieces.push(id);
            }
            hide_enemy(scene, self.enemies[p.owner].spec.id);
            // Worker latency must not consume the visible death effect lifetime.
            self.enemies[p.owner].age = 0.;
            self.commits += 1;
        }
        let mut budget = INSTALL_VERTICES;
        while budget > 0 {
            let Some(result) = self.completed.front_mut() else {
                break;
            };
            if self.enemies[result.pending.owner].serial != result.pending.serial
                || self.enemies[result.pending.owner].phase != 3
            {
                self.completed.pop_front();
                continue;
            }
            if result.side == 2 {
                self.completed.pop_front();
                continue;
            }
            let count = (result.counts[result.side] - result.offset).min(budget);
            if count == 0 {
                result.side += 1;
                result.offset = 0;
                continue;
            }
            let start = 12
                + (if result.side == 1 {
                    result.counts[0]
                } else {
                    0
                } + result.offset)
                    * 36;
            let mut vertices = Vec::with_capacity(count);
            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);
            for bytes in result.bytes[start..start + count * 36].as_chunks::<36>().0 {
                let floats: [f32; 9] = std::array::from_fn(|i| {
                    f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
                });
                if floats.iter().any(|v| !v.is_finite()) {
                    return Err("Nonfinite cut result".into());
                }
                let p = Vec3::from_slice(&floats);
                min = min.min(p);
                max = max.max(p);
                vertices.push(crate::gltf_loader::ImportedVertex {
                    position: p.to_array(),
                    normal: [floats[3], floats[4], floats[5]],
                    color: [floats[6], floats[7], floats[8]],
                });
            }
            let mesh = crate::gltf_loader::ImportedMesh {
                indices: (0..count as u32).collect(),
                vertices,
                bounds_min: min.to_array(),
                bounds_max: max.to_array(),
                warnings: Vec::new(),
            };
            let id =
                scene.add_visual_triangle_mesh(crate::cutting::CutMesh::from_render_mesh(&mesh))?;
            scene.set_object_position(id, HIDDEN);
            self.staged
                .iter_mut()
                .find(|s| {
                    s.pending.owner == result.pending.owner
                        && s.pending.serial == result.pending.serial
                })
                .expect("accepted cut has stage")
                .ids
                .push((id, result.side));
            self.enemies[result.pending.owner].cut_idle = 0.;
            result.offset += count;
            budget -= count;
            self.staged_vertices += count;
        }
        Ok(())
    }
    fn advance_piece_motion(&mut self, scene: &mut SceneManager, dt: f32) {
        self.pieces.retain(|id| scene.object_exists(*id));
        self.piece_motion.retain(|id, motion| {
            if !scene.object_exists(*id) {
                return false;
            }
            motion.age += dt;
            let distance = (motion.age * 0.52).min(0.65);
            let falling = (motion.age - 0.45).max(0.);
            let drop = if motion.direction == Vec3::ZERO {
                0.
            } else {
                (falling * falling * 0.1).min(0.38)
            };
            scene.set_object_position(*id, motion.direction * distance - Vec3::Y * drop);
            true
        });
    }
    fn expire_cut(&mut self, scene: &mut SceneManager, owner: usize) {
        let serial = self.enemies[owner].serial;
        let matches = |p: &Pending| p.owner == owner && p.serial == serial;
        self.tasks
            .retain(|t| t.owner != owner || t.serial != serial);
        self.pending.retain(|_, p| !matches(p));
        self.completed.retain(|c| !matches(&c.pending));
        let mut i = 0;
        while i < self.staged.len() {
            if matches(&self.staged[i].pending) {
                for (id, _) in self.staged.remove(i).ids {
                    scene.remove_object(id);
                }
            } else {
                i += 1;
            }
        }
        self.cut_errors += 1;
    }
    pub fn cut_failed(&mut self) {
        self.cut_errors += 1;
        for stage in self.staged.drain(..) {
            self.abandoned
                .extend(stage.ids.into_iter().map(|(id, _)| id));
        }
        self.tasks.clear();
        self.pending.clear();
        self.completed.clear();
    }
    fn sync(&mut self, scene: &mut SceneManager, head: Option<Mat4>) {
        for &id in &self.config_ids.1 {
            scene.set_object_position(id, HIDDEN);
        }
        for b in &self.bolts {
            scene.set_object_position(b.id, b.position);
            scene.set_emission(b.id, if b.friendly { 3. } else { 1.5 });
        }
        for (i, e) in self.enemies.iter().enumerate() {
            let beam = self.config_ids.2[i];
            let charge = self.config_ids.3[i];
            scene.set_object_position(beam, HIDDEN);
            scene.set_object_position(charge, HIDDEN);
            if self.health > 0. && (e.phase == 1 || e.phase == 2) {
                if e.beam_age >= 0. {
                    let end = self
                        .world
                        .cast(e.beam_from, e.beam_target)
                        .map_or(e.beam_target, |h| h.point);
                    let dir = end - e.beam_from;
                    let q = Quat::from_rotation_arc(Vec3::Z, dir.normalize_or_zero());
                    let (y, x, z) = q.to_euler(glam::EulerRot::YXZ);
                    let radius = if e.beam_age < 0.85 {
                        0.014
                    } else if e.spec.kind == "core-tank" {
                        0.13
                    } else {
                        0.085
                    };
                    scene.set_object_transform(
                        beam,
                        (e.beam_from + end) * 0.5,
                        Vec3::new(x, y, z),
                        Vec3::new(radius, radius, dir.length()),
                    );
                }
                if e.shot < 0.4 && e.beam_age < 0. {
                    scene.set_object_transform(
                        charge,
                        e.position + Vec3::Y * e.spec.height * 0.5,
                        Vec3::ZERO,
                        Vec3::splat(0.12 + (0.4 - e.shot) * 0.25),
                    );
                }
            }
        }
        let head = head.unwrap_or_else(|| {
            Mat4::from_rotation_translation(
                Quat::from_rotation_y(self.player.yaw) * Quat::from_rotation_x(self.player.pitch),
                self.player.head(1.7),
            )
        });
        let head_position = head.w_axis.truncate();
        for (e, &[background, fill]) in self.enemies.iter().zip(&self.health_bars) {
            if !self.playing() || self.health <= 0. || !matches!(e.phase, 1 | 2) {
                scene.set_object_position(background, HIDDEN);
                scene.set_object_position(fill, HIDDEN);
                continue;
            }
            let yaw = (head_position.x - e.position.x).atan2(head_position.z - e.position.z);
            let q = Quat::from_rotation_y(yaw);
            let width = e.spec.height.clamp(0.7, 1.4);
            let ratio = (e.hp / e.max_hp).clamp(0., 1.);
            let center = e.position + Vec3::Y * (e.spec.height + 0.2);
            scene.set_object_transform(
                background,
                center,
                Vec3::new(0., yaw, 0.),
                Vec3::new(width + 0.06, 0.10, 0.025),
            );
            scene.set_object_transform(
                fill,
                center + q * Vec3::new(width * (ratio - 1.) * 0.5, 0., 0.025),
                Vec3::new(0., yaw, 0.),
                Vec3::new(width * ratio, 0.065, 0.018),
            );
        }
        for h in 0..2 {
            if !self.playing() || self.health <= 0. || self.xr && self.blades[h].is_none() {
                scene.set_object_position(self.config_ids.0[h], HIDDEN);
            }
        }
        let q = Quat::from_mat4(&head);
        let (y, x, z) = q.to_euler(glam::EulerRot::YXZ);
        for (i, &id) in self.config_ids.4.iter().enumerate() {
            let value = if i == 0 {
                self.health / 100.
            } else {
                (self.player.stamina - (i - 1) as f32).clamp(0., 1.)
            };
            let offset = if i == 0 {
                Vec3::new(0., -0.26, -0.65)
            } else {
                Vec3::new((i as f32 - 2.5) * 0.045, -0.295, -0.65)
            };
            let scale = if i == 0 {
                Vec3::new(0.16 * value.max(0.001), 0.008, 0.003)
            } else {
                Vec3::new(0.034 * value.max(0.001), 0.012, 0.003)
            };
            scene.set_object_transform(
                id,
                if self.playing() {
                    head.transform_point3(offset)
                } else {
                    HIDDEN
                },
                Vec3::new(x, y, z),
                scale,
            );
        }
        scene.set_object_position(
            self.config_ids.5,
            if self.player.aiming {
                self.player.target.unwrap_or(HIDDEN) + Vec3::Y * 0.03
            } else {
                HIDDEN
            },
        );
    }
    pub fn state(&self) -> Value {
        json!({"blade":{"contactSamples":self.blade_contacts,"hits":self.blade_hits,"slowSamples":self.blade_slow,"discontinuities":self.blade_breaks,"maxSpeedForFrame":self.blade_speed},"input":{"movement":self.axis,"aim":self.aim,"sprint":self.sprint},"simDt":self.sim_dt,"active":self.active,"paused":self.paused,"health":self.health,"maxHealth":100,"dead":self.health<=0.,"wave":self.wave+1,"kills":self.kills,"deaths":self.deaths,"hitsTaken":self.hits,"player":self.player.describe(),"frame":{"simulation":self.frame,"visual":self.visual_frame},"deathMode":self.death_mode,"enemies":self.enemies.iter().filter(|e|e.phase!=0).map(|e|json!({"id":e.spec.id,"slug":e.spec.kind,"position":e.position.to_array(),"height":e.spec.height,"hp":e.hp,"maxHp":e.max_hp,"phase":e.phase,"shotTimer":e.shot,"plasma":e.beam_age})).collect::<Vec<_>>(),"projectiles":{"active":self.bolts.len(),"fired":self.fired,"reflected":self.reflected,"hitEnemies":self.returned,"blockedWorld":self.blocked,"bolts":self.bolts.iter().map(|b|json!({"position":b.position.to_array(),"velocity":b.velocity.to_array(),"friendly":b.friendly})).collect::<Vec<_>>()},"cuts":{"queued":self.tasks.len(),"pending":self.pending.len(),"pieces":self.pieces.len(),"motion":self.piece_motion.iter().map(|(id,m)|json!({"id":id,"age":m.age,"direction":m.direction.to_array(),"distance":(m.age*0.52).min(0.65)})).collect::<Vec<_>>(),"errors":self.cut_errors,"staging":self.staged.len(),"stagedMeshes":self.staged_cut_ids().count(),"preparedTriangles":self.prepared_triangles,"stagedVertices":self.staged_vertices,"commits":self.commits,"stageMs":self.stage_ms,"stageMaxMs":self.stage_max_ms},"xr":self.xr})
    }
}
fn max_hp(kind: &str) -> f32 {
    match kind {
        "core-tank" => 200.,
        "blade-wraith" => 150.,
        _ => 100.,
    }
}
fn blade_damage_allowed(
    e: &mut Enemy,
    hand: usize,
    contact: bool,
    eligible: bool,
    strike: Option<u64>,
) -> bool {
    if !contact {
        e.contacts[hand] = false;
        return false;
    }
    if e.contacts[hand]
        || !eligible
        || e.hp <= 0.
        || e.cooldowns[hand] > 0.
        || strike.is_some_and(|id| id == e.last_strikes[hand])
    {
        return false;
    }
    if let Some(id) = strike {
        e.last_strikes[hand] = id;
    }
    e.contacts[hand] = true;
    e.cooldowns[hand] = 0.28;
    true
}
fn hide_enemy(scene: &mut SceneManager, id: u32) {
    scene.set_object_position(id, HIDDEN);
    // Preserve the last pose for queued cuts; stop() would reset it and rebuild meshes.
    let _ = scene.pause_model_animation(id, true);
}
fn play(scene: &mut SceneManager, e: &Enemy, name: &str, looping: bool) {
    if let Some(model) = scene.model(e.spec.id) {
        if let Some(clip) = model.asset.animations.iter().position(|c| {
            c.name
                .as_deref()
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        }) {
            let _ = scene.play_model_animation(e.spec.id, clip, looping, 1.);
        }
    }
}
fn free(p: Vec2, obs: &[[f32; 4]]) -> bool {
    p.x > -77.
        && p.x < 77.
        && p.y > -81.
        && p.y < 59.
        && !obs
            .iter()
            .any(|r| p.x >= r[0] && p.x <= r[1] && p.y >= r[2] && p.y <= r[3])
}
fn blocked(a: Vec2, b: Vec2, obs: &[[f32; 4]]) -> bool {
    obs.iter().any(|r| {
        let d = b - a;
        let mut lo = 0f32;
        let mut hi = 1f32;
        for (v, delta, min, max) in [(a.x, d.x, r[0], r[1]), (a.y, d.y, r[2], r[3])] {
            if delta.abs() < 1e-8 {
                if v < min || v > max {
                    return false;
                }
            } else {
                let x = (min - v) / delta;
                let y = (max - v) / delta;
                lo = lo.max(x.min(y));
                hi = hi.min(x.max(y));
                if lo > hi {
                    return false;
                }
            }
        }
        true
    })
}
fn waypoint(
    start: Vec2,
    goal: Vec2,
    obs: &[[f32; 4]],
    corners: &[Vec2],
    edges: &[Vec<f32>],
) -> Option<Vec2> {
    if !blocked(start, goal, obs) {
        return Some(goal);
    }
    let mut nodes = vec![start, goal];
    nodes.extend_from_slice(corners);
    let n = nodes.len();
    let mut dist = vec![f32::INFINITY; n];
    let mut prev = vec![0; n];
    let mut seen = vec![false; n];
    dist[0] = 0.;
    for _ in 0..n {
        let i = (0..n)
            .filter(|&i| !seen[i])
            .min_by(|&a, &b| dist[a].total_cmp(&dist[b]))?;
        if !dist[i].is_finite() {
            return None;
        }
        if i == 1 {
            let mut j = 1;
            while prev[j] > 0 {
                j = prev[j];
            }
            return Some(nodes[j]);
        }
        seen[i] = true;
        for j in 1..n {
            if !seen[j] {
                let cost = if i > 1 && j > 1 {
                    edges[i - 2][j - 2]
                } else if blocked(nodes[i], nodes[j], obs) {
                    f32::INFINITY
                } else {
                    nodes[i].distance(nodes[j])
                };
                let d = dist[i] + cost;
                if d < dist[j] {
                    dist[j] = d;
                    prev[j] = i;
                }
            }
        }
    }
    None
}
fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let d = b - a;
    let t = ((p - a).dot(d) / d.length_squared().max(1e-10)).clamp(0., 1.);
    p.distance(a + d * t)
}
fn real_continuity(a: Blade, b: Blade, dt: f32) -> bool {
    dt > 0.
        && dt <= 0.15
        && a.grip.distance(b.grip) < (15. * dt + 0.025).min(1.)
        && a.rotation.angle_between(b.rotation) < (45. * dt + 0.03).min(1.5)
        && a.a.distance(b.a).max(a.b.distance(b.b)) < 1.5
}
fn swept_enemy(a: Blade, b: Blade, e: &Enemy) -> bool {
    let travel = a.a.distance(b.a).max(a.b.distance(b.b));
    let count = (travel / 0.035).ceil() as usize;
    let flying = e.spec.kind != "blade-wraith";
    for i in 0..=count {
        let f = i as f32 / count.max(1) as f32;
        let pa = a.a.lerp(b.a, f);
        let pb = a.b.lerp(b.b, f);
        let mut y = if flying { e.spec.height * 0.25 } else { 0.35 };
        while y <= e.spec.height * 0.75 {
            if segment_distance(e.position + Vec3::Y * y, pa, pb)
                < if flying { e.spec.height * 0.5 } else { 0.47 }
            {
                return true;
            }
            y += 0.08;
        }
    }
    false
}
fn sphere_time(a: Vec3, b: Vec3, c: Vec3, r: f32) -> Option<f32> {
    let d = b - a;
    let m = a - c;
    let c = m.length_squared() - r * r;
    if c <= 0. {
        return Some(0.);
    }
    let aa = d.length_squared();
    let bb = m.dot(d);
    let disc = bb * bb - aa * c;
    if aa < 1e-7 || bb >= 0. || disc < 0. {
        return None;
    }
    let t = (-bb - disc.sqrt()) / aa;
    (0. ..=1.).contains(&t).then_some(t)
}
fn blade_time(from: Vec3, to: Vec3, a: Blade, b: Blade) -> Option<f32> {
    let speed = from.distance(to) + a.a.distance(b.a).max(a.b.distance(b.b));
    let mut t = 0.;
    for _ in 0..256 {
        let gap = segment_distance(from.lerp(to, t), a.a.lerp(b.a, t), a.b.lerp(b.b, t)) - 0.075;
        if gap <= 1e-5 {
            return Some(t);
        }
        if speed <= 1e-8 {
            return None;
        }
        t += gap / speed;
        if t > 1. {
            return None;
        }
    }
    None
}

#[cfg(test)]
#[path = "rim_game_tests.rs"]
mod tests;

#[cfg(target_arch = "wasm32")]
fn cut_clock() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map_or(0., |p| p.now())
}
#[cfg(not(target_arch = "wasm32"))]
fn cut_clock() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs_f64()
        * 1000.
}
