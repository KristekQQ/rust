//! Serial melee encounters. Tracking, navigation and hit decisions remain native.
use crate::scene::{SceneManager, Transform};
use glam::{Mat4, Quat, Vec2, Vec3};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub enemies: Vec<u32>,
    pub swords: [u32; 2],
    pub spawns: Vec<[f32; 3]>,
    /// [minimum X, maximum X, minimum Z, maximum Z], in world meters.
    pub bounds: [f32; 4],
    pub obstacles: Vec<[f32; 4]>,
}

struct Enemy {
    id: u32,
    original: Transform,
    // Idle, Run, Attack, Explode.
    clips: [usize; 4],
}
#[derive(Clone, Copy)]
struct BladeSample {
    points: [Vec3; 2],
    hand: Vec3,
    orientation: Quat,
}
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Chase,
    Attack,
    Dying,
    Stopped,
}

pub struct Encounter {
    config: Config,
    enemies: Vec<Enemy>,
    active: usize,
    phase: Phase,
    position: Vec3,
    kills: u32,
    death_time: f32,
    blades: [Option<BladeSample>; 2],
    last_head: Option<Vec3>,
    nav_obstacles: Vec<[f32; 4]>,
    nav_corners: Vec<Vec2>,
    nav_waypoint: Option<Vec2>,
    nav_goal: Vec2,
    nav_age: f32,
}

impl Config {
    pub fn validate(&self, scene: &SceneManager) -> Result<(), String> {
        self.prepare(scene).map(|_| ())
    }
    fn prepare(&self, scene: &SceneManager) -> Result<Vec<Enemy>, String> {
        if self.enemies.is_empty() || self.spawns.len() != self.enemies.len() {
            return Err("Encounter needs one spawn per enemy".into());
        }
        if !valid_rect(self.bounds)
            || self.obstacles.iter().any(|r| !valid_rect(*r))
            || self.spawns.iter().flatten().any(|v| !v.is_finite())
        {
            return Err("Encounter bounds, obstacles and spawns must be finite and ordered".into());
        }
        if self.swords[0] == self.swords[1]
            || self
                .swords
                .iter()
                .any(|id| !scene.object_exists(*id) || scene.tracked_blade(*id).is_none())
        {
            return Err("Encounter requires two distinct tracked sword objects".into());
        }
        let mut enemies = Vec::new();
        for &id in &self.enemies {
            if enemies.iter().any(|e: &Enemy| e.id == id) || self.swords.contains(&id) {
                return Err("Encounter enemy IDs must be distinct".into());
            }
            let model = scene
                .model(id)
                .ok_or("Encounter enemy must be a loaded model")?;
            let mut clips = [0; 4];
            for (slot, name) in ["Idle", "Run", "Attack", "Explode"].iter().enumerate() {
                clips[slot] = model
                    .asset
                    .animations
                    .iter()
                    .position(|c| {
                        c.name
                            .as_deref()
                            .is_some_and(|n| n.eq_ignore_ascii_case(name))
                    })
                    .ok_or_else(|| format!("Enemy {id} has no {name} clip"))?;
            }
            enemies.push(Enemy {
                id,
                original: scene.objects()[&id].transform,
                clips,
            });
        }
        Ok(enemies)
    }
}

impl Encounter {
    pub fn new(config: Config, scene: &mut SceneManager) -> Result<Self, String> {
        let enemies = config.prepare(scene)?;
        // Inflate geometry for an approximately 80cm-wide enemy. Visibility graph
        // corners are cached; shortest paths avoid the local-steering barricade trap.
        let nav_obstacles: Vec<_> = config
            .obstacles
            .iter()
            .map(|r| [r[0] - 0.4, r[1] + 0.4, r[2] - 0.4, r[3] + 0.4])
            .collect();
        let mut nav_corners = Vec::new();
        for r in &nav_obstacles {
            for x in [r[0] - 0.02, r[1] + 0.02] {
                for z in [r[2] - 0.02, r[3] + 0.02] {
                    let p = Vec2::new(x, z);
                    if inside(p, config.bounds) && !nav_obstacles.iter().any(|b| inside(p, *b)) {
                        nav_corners.push(p);
                    }
                }
            }
        }
        let mut encounter = Self {
            position: Vec3::from_array(config.spawns[0]),
            config,
            enemies,
            active: 0,
            phase: Phase::Chase,
            kills: 0,
            death_time: 0.,
            blades: [None; 2],
            last_head: None,
            nav_obstacles,
            nav_corners,
            nav_waypoint: None,
            nav_goal: Vec2::ZERO,
            nav_age: 0.,
        };
        for i in 0..encounter.enemies.len() {
            encounter.hide(scene, i);
        }
        encounter.spawn(scene);
        Ok(encounter)
    }

    pub fn reset_tracking(&mut self) {
        self.blades = [None; 2];
        self.last_head = None;
    }
    pub fn state(&self) -> Value {
        json!({"phase": match self.phase {Phase::Chase=>"chase", Phase::Attack=>"attack",
            Phase::Dying=>"dying", Phase::Stopped=>"stopped"},
            "active":self.active, "id": self.enemies[self.active].id, "kills":self.kills,
            "position": self.position.to_array(), "hp": if self.phase == Phase::Dying {0} else {1}})
    }
    pub fn stop(&mut self, scene: &mut SceneManager) {
        for enemy in &self.enemies {
            let t = enemy.original;
            scene.set_object_transform(enemy.id, t.position, t.rotation, t.scale);
            let _ = scene.play_model_animation(enemy.id, enemy.clips[0], true, 1.);
        }
        self.phase = Phase::Stopped;
        self.reset_tracking();
    }
    fn hide(&self, scene: &mut SceneManager, i: usize) {
        let enemy = &self.enemies[i];
        scene.set_object_position(enemy.id, Vec3::new(0., -10000., 0.));
        // Start then pause even if the caller had no playback yet.
        let _ = scene.play_model_animation(enemy.id, enemy.clips[0], true, 1.);
        let _ = scene.pause_model_animation(enemy.id, true);
    }
    fn spawn(&mut self, scene: &mut SceneManager) {
        self.position = Vec3::from_array(self.config.spawns[self.active]);
        let enemy = &self.enemies[self.active];
        scene.set_object_transform(
            enemy.id,
            self.position,
            enemy.original.rotation,
            enemy.original.scale,
        );
        self.phase = Phase::Chase;
        self.death_time = 0.;
        self.nav_waypoint = None;
        self.nav_age = 0.25;
        self.reset_tracking();
        self.play(scene, 1, true);
    }
    fn play(&self, scene: &mut SceneManager, slot: usize, looping: bool) {
        let enemy = &self.enemies[self.active];
        let _ = scene.play_model_animation(enemy.id, enemy.clips[slot], looping, 1.);
    }
    pub fn tick(
        &mut self,
        scene: &mut SceneManager,
        dt: f32,
        head: Option<Mat4>,
        hands: [Option<Mat4>; 2],
    ) {
        self.tick_timed(scene, dt, dt, head, hands);
    }
    /// Simulation can be slowed; tracking continuity always uses real elapsed time.
    pub fn tick_timed(
        &mut self,
        scene: &mut SceneManager,
        dt: f32,
        tracking_dt: f32,
        head: Option<Mat4>,
        hands: [Option<Mat4>; 2],
    ) {
        if self.phase == Phase::Stopped {
            return;
        }
        let Some(head) = head.filter(|h| h.is_finite()) else {
            self.reset_tracking();
            return;
        };
        if !dt.is_finite()
            || dt <= 0.
            || dt > 0.25
            || !tracking_dt.is_finite()
            || tracking_dt <= 0.
            || tracking_dt > 0.25
        {
            self.reset_tracking();
            return;
        }
        let head = head.w_axis.truncate();
        if self.last_head.is_some_and(|p| p.distance(head) > 1.5) {
            self.reset_tracking();
        }
        self.last_head = Some(head);
        if self.phase == Phase::Dying {
            self.death_time += dt;
            if self.death_time >= 3. {
                self.hide(scene, self.active);
                self.active = (self.active + 1) % self.enemies.len();
                self.spawn(scene);
            }
            return;
        }
        let p = Vec2::new(self.position.x, self.position.z);
        let goal = Vec2::new(
            head.x.clamp(self.config.bounds[0], self.config.bounds[1]),
            head.z.clamp(self.config.bounds[2], self.config.bounds[3]),
        );
        let near = p.distance(goal) <= 1.1;
        let next_phase = if near { Phase::Attack } else { Phase::Chase };
        if self.phase != next_phase {
            self.phase = next_phase;
            self.play(scene, if near { 2 } else { 1 }, true);
        }
        if !near {
            self.nav_age += dt;
            let reached_or_blocked = self
                .nav_waypoint
                .is_some_and(|w| p.distance(w) < 0.04 || blocked(p, w, &self.nav_obstacles));
            if reached_or_blocked
                || self.nav_waypoint.is_none() && self.nav_age >= 0.25
                || self.nav_age >= 0.25 && self.nav_goal.distance(goal) > 0.25
            {
                self.nav_waypoint = next_waypoint(p, goal, &self.nav_obstacles, &self.nav_corners);
                self.nav_goal = goal;
                self.nav_age = 0.;
            }
            if let Some(waypoint) = self.nav_waypoint {
                let delta = waypoint - p;
                let step = delta.normalize_or_zero()
                    * dt.min(delta.length()).min((p.distance(goal) - 1.1).max(0.));
                self.position.x += step.x;
                self.position.z += step.y;
            }
        }
        let id = self.enemies[self.active].id;
        scene.set_object_position(id, self.position);
        let direction = goal - Vec2::new(self.position.x, self.position.z);
        if direction.length_squared() > 0.0001 {
            let mut rotation = self.enemies[self.active].original.rotation;
            rotation.y = direction.x.atan2(direction.y);
            scene.set_object_rotation(id, rotation);
        }
        let mut hit = false;
        for (i, hand) in hands.into_iter().enumerate() {
            let sample = hand.filter(|h| h.is_finite()).and_then(|h| {
                let sword = self.config.swords[i];
                let matrix = scene.objects().get(&sword)?.transform.model();
                let points = scene
                    .tracked_blade(sword)?
                    .map(|p| matrix.transform_point3(p));
                points.iter().all(|p| p.is_finite()).then_some(BladeSample {
                    points,
                    hand: h.w_axis.truncate(),
                    orientation: Quat::from_mat4(&h).normalize(),
                })
            });
            if let (Some(previous), Some(current)) = (self.blades[i], sample) {
                hit |= swing_hit(previous, current, self.position, tracking_dt);
            }
            self.blades[i] = sample;
        }
        if hit {
            self.kills = self.kills.saturating_add(1);
            self.phase = Phase::Dying;
            self.death_time = 0.;
            self.blades = [None; 2];
            self.play(scene, 3, false);
        }
    }
}
fn valid_rect(r: [f32; 4]) -> bool {
    r.iter().all(|x| x.is_finite()) && r[0] < r[1] && r[2] < r[3]
}
fn inside(p: Vec2, r: [f32; 4]) -> bool {
    p.x >= r[0] && p.x <= r[1] && p.y >= r[2] && p.y <= r[3]
}
fn blocked(a: Vec2, b: Vec2, obstacles: &[[f32; 4]]) -> bool {
    obstacles.iter().any(|r| {
        let mut lo: f32 = 0.;
        let mut hi: f32 = 1.;
        for (start, delta, min, max) in [(a.x, b.x - a.x, r[0], r[1]), (a.y, b.y - a.y, r[2], r[3])]
        {
            if delta.abs() < 1e-6 {
                if start < min || start > max {
                    return false;
                }
            } else {
                let u = (min - start) / delta;
                let v = (max - start) / delta;
                lo = lo.max(u.min(v));
                hi = hi.min(u.max(v));
            }
        }
        lo <= hi
    })
}
fn next_waypoint(
    start: Vec2,
    goal: Vec2,
    obstacles: &[[f32; 4]],
    corners: &[Vec2],
) -> Option<Vec2> {
    if !blocked(start, goal, obstacles) {
        return Some(goal);
    }
    let nodes: Vec<_> = [start, goal]
        .into_iter()
        .chain(corners.iter().copied())
        .collect();
    let mut distances = vec![f32::INFINITY; nodes.len()];
    let mut previous = vec![0; nodes.len()];
    let mut visited = vec![false; nodes.len()];
    distances[0] = 0.;
    for _ in 0..nodes.len() {
        let i = (0..nodes.len())
            .filter(|&i| !visited[i])
            .min_by(|&a, &b| distances[a].total_cmp(&distances[b]))?;
        if !distances[i].is_finite() {
            return None;
        }
        if i == 1 {
            let mut j = 1;
            while previous[j] != 0 {
                j = previous[j];
            }
            return Some(nodes[j]);
        }
        visited[i] = true;
        for j in 1..nodes.len() {
            if visited[j] || blocked(nodes[i], nodes[j], obstacles) {
                continue;
            }
            let d = distances[i] + nodes[i].distance(nodes[j]);
            if d < distances[j] {
                distances[j] = d;
                previous[j] = i;
            }
        }
    }
    None
}
fn point_segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let d = b - a;
    p.distance(a + d * ((p - a).dot(d) / d.length_squared().max(1e-9)).clamp(0., 1.))
}
fn swing_hit(previous: BladeSample, current: BladeSample, enemy: Vec3, dt: f32) -> bool {
    let travel = previous.points[0]
        .distance(current.points[0])
        .max(previous.points[1].distance(current.points[1]));
    let hand_travel = previous.hand.distance(current.hand);
    let angle = previous
        .orientation
        .angle_between(current.orientation)
        .abs();
    if hand_travel > (15. * dt + 0.025).min(1.)
        || angle > (45. * dt + 0.03).min(1.5)
        || travel < 0.025
        || travel / dt < 0.35
        || travel > 1.5
        || previous.hand.distance(current.hand) > 1.5
    {
        return false;
    }
    // Sample the swept ruled surface at <= 3cm, then test the enemy's vertical
    // torso capsule. This covers both longitudinal and transverse fast swings.
    let steps = (travel / 0.03).ceil() as usize;
    (0..=steps).any(|i| {
        let t = i as f32 / steps as f32;
        let a = previous.points[0].lerp(current.points[0], t);
        let b = previous.points[1].lerp(current.points[1], t);
        (0..=12)
            .any(|j| point_segment_distance(enemy + Vec3::Y * (0.3 + j as f32 * 0.1), a, b) <= 0.43)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::MeshKind;
    fn sample(x: f32) -> BladeSample {
        BladeSample {
            points: [Vec3::new(x, 0.8, -0.8), Vec3::new(x, 0.8, 0.8)],
            hand: Vec3::new(x, 1., 0.),
            orientation: Quat::IDENTITY,
        }
    }
    fn fixture() -> (SceneManager, Encounter) {
        let mut scene = SceneManager::default();
        let asset = std::sync::Arc::new(
            crate::gltf_loader::parse_model(
                include_bytes!("../assets/models/neon-enemies/drone-orbiter-pink.glb"),
                &[],
                &[],
            )
            .unwrap(),
        );
        let enemies = (0..3)
            .map(|_| scene.add_model(asset.clone()).unwrap())
            .collect();
        let swords = std::array::from_fn(|_| {
            let id = scene.add_object(
                MeshKind::Cube,
                Transform::new(Vec3::new(-0.7, 0., 0.), Vec3::ZERO, Vec3::ONE),
            );
            scene
                .set_cutter(
                    id,
                    Some([Vec3::new(0., 0.8, -0.8), Vec3::new(0., 0.8, 0.8)]),
                )
                .unwrap();
            id
        });
        let encounter = Encounter::new(
            Config {
                enemies,
                swords,
                spawns: vec![[0., 0., 0.]; 3],
                bounds: [-10., 10., -10., 10.],
                obstacles: vec![],
            },
            &mut scene,
        )
        .unwrap();
        (scene, encounter)
    }
    fn tick(scene: &mut SceneManager, e: &mut Encounter) {
        e.tick(
            scene,
            0.1,
            Some(Mat4::from_translation(Vec3::new(0., 1.7, 1.))),
            [Some(Mat4::IDENTITY); 2],
        );
    }
    #[test]
    fn stationary_sword_and_teleport_never_hit_fast_sweep_does() {
        assert!(!swing_hit(sample(0.), sample(0.), Vec3::ZERO, 0.01));
        assert!(!swing_hit(sample(-2.), sample(2.), Vec3::ZERO, 0.01));
        let mut jumped = sample(0.7);
        jumped.hand = Vec3::splat(10.);
        assert!(!swing_hit(sample(-0.7), jumped, Vec3::ZERO, 0.02));
        assert!(swing_hit(sample(-0.5), sample(0.5), Vec3::ZERO, 0.1));
        assert!(!swing_hit(sample(-0.7), sample(0.3), Vec3::ZERO, 1. / 72.));
        let mut rotated = sample(0.3);
        rotated.orientation = Quat::from_rotation_y(2.0);
        assert!(!swing_hit(sample(-0.7), rotated, Vec3::ZERO, 0.1));
    }
    #[test]
    fn slow_simulation_keeps_real_tracking_time() {
        let (mut scene, mut e) = fixture();
        let head = Some(Mat4::from_translation(Vec3::new(0., 1.7, 1.)));
        e.tick_timed(
            &mut scene,
            0.1 / 72.,
            1. / 72.,
            head,
            [Some(Mat4::IDENTITY); 2],
        );
        scene.set_object_position(e.config.swords[0], Vec3::new(-0.5, 0., 0.));
        e.tick_timed(
            &mut scene,
            0.1 / 72.,
            1. / 72.,
            head,
            [
                Some(Mat4::from_translation(Vec3::new(0.2, 0., 0.))),
                Some(Mat4::IDENTITY),
            ],
        );
        scene.set_object_position(e.config.swords[0], Vec3::new(-0.3, 0., 0.));
        e.tick_timed(
            &mut scene,
            0.1 / 72.,
            1. / 72.,
            head,
            [
                Some(Mat4::from_translation(Vec3::new(0.4, 0., 0.))),
                Some(Mat4::IDENTITY),
            ],
        );
        assert_eq!(e.kills, 1);
    }
    #[test]
    fn serial_death_waits_three_seconds_and_wraps() {
        let (mut scene, mut e) = fixture();
        for expected in [0, 1, 2, 0] {
            assert_eq!(e.active, expected);
            for i in 0..3 {
                assert_eq!(
                    scene.objects()[&e.enemies[i].id].transform.position.y < -9000.,
                    i != expected
                );
            }
            tick(&mut scene, &mut e);
            for sword in e.config.swords {
                scene.set_object_position(sword, Vec3::new(0.7, 0., 0.));
            }
            tick(&mut scene, &mut e);
            assert_eq!(e.state()["phase"], "dying");
            for _ in 0..29 {
                tick(&mut scene, &mut e);
            }
            assert_eq!(e.active, expected);
            assert_eq!(e.state()["phase"], "dying");
            tick(&mut scene, &mut e);
            tick(&mut scene, &mut e);
            assert_eq!(e.active, (expected + 1) % 3);
            for sword in e.config.swords {
                scene.set_object_position(sword, Vec3::new(-0.7, 0., 0.));
            }
        }
        assert_eq!(e.kills, 4);
        e.stop(&mut scene);
        assert!(e
            .enemies
            .iter()
            .all(|enemy| scene.objects()[&enemy.id].transform.position == enemy.original.position));
    }
    #[test]
    fn each_sword_can_hit_and_tracking_loss_resets_history() {
        for hand in 0..2 {
            let (mut scene, mut e) = fixture();
            tick(&mut scene, &mut e);
            let mut hands = [Some(Mat4::IDENTITY); 2];
            hands[hand] = None;
            e.tick(&mut scene, 0.1, Some(Mat4::IDENTITY), hands);
            scene.set_object_position(e.config.swords[hand], Vec3::new(0.7, 0., 0.));
            tick(&mut scene, &mut e); // Reacquisition does not sweep across the gap.
            assert_eq!(e.kills, 0);
            scene.set_object_position(e.config.swords[hand], Vec3::new(-0.7, 0., 0.));
            tick(&mut scene, &mut e);
            assert_eq!(e.kills, 1);
        }
    }
    #[test]
    fn missing_head_and_head_teleport_discard_sweep() {
        let (mut scene, mut e) = fixture();
        tick(&mut scene, &mut e);
        e.tick(&mut scene, 0.1, None, [Some(Mat4::IDENTITY); 2]);
        scene.set_object_position(e.config.swords[0], Vec3::new(0.7, 0., 0.));
        tick(&mut scene, &mut e);
        assert_eq!(e.kills, 0);
        scene.set_object_position(e.config.swords[0], Vec3::new(-0.7, 0., 0.));
        e.tick(
            &mut scene,
            0.1,
            Some(Mat4::from_translation(Vec3::new(3., 1.7, 1.))),
            [Some(Mat4::IDENTITY); 2],
        );
        assert_eq!(e.kills, 0);
    }
    #[test]
    fn navigation_walks_around_barricade_without_crossing_it() {
        let obstacles = [[-1., 1., -0.5, 0.5]];
        let corners = [
            Vec2::new(-1.02, -0.52),
            Vec2::new(1.02, -0.52),
            Vec2::new(-1.02, 0.52),
            Vec2::new(1.02, 0.52),
        ];
        let mut p = Vec2::new(0., -3.);
        let goal = Vec2::new(0., 3.);
        for _ in 0..150 {
            let next = next_waypoint(p, goal, &obstacles, &corners).unwrap();
            let delta = next - p;
            let moved = p + delta.normalize_or_zero() * 0.1f32.min(delta.length());
            assert!(!blocked(p, moved, &obstacles));
            p = moved;
        }
        assert!(p.distance(goal) < 0.01);
    }
    #[test]
    fn city_layout_spawns_reach_player_without_crossing_geometry() {
        let layout: Value =
            serde_json::from_str(include_str!("../assets/models/cyberpunk-alley/layout.json"))
                .unwrap();
        let bounds: [f32; 4] = serde_json::from_value(layout["walkableBounds"].clone()).unwrap();
        let target: [f32; 3] = serde_json::from_value(layout["spawn"].clone()).unwrap();
        let obstacles: Vec<[f32; 4]> = layout["obstacles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| {
                let c: [f32; 3] = serde_json::from_value(o["center"].clone()).unwrap();
                let h: [f32; 3] = serde_json::from_value(o["halfExtents"].clone()).unwrap();
                [c[0] - h[0], c[0] + h[0], c[2] - h[2], c[2] + h[2]]
            })
            .collect();
        for spawn in layout["modelSpawns"].as_array().unwrap() {
            let (mut scene, mut old) = fixture();
            old.stop(&mut scene);
            let mut config = old.config.clone();
            config.bounds = bounds;
            config.obstacles = obstacles.clone();
            let spawn: [f32; 3] = serde_json::from_value(spawn["position"].clone()).unwrap();
            config.spawns = vec![spawn; 3];
            let mut e = Encounter::new(config, &mut scene).unwrap();
            let head = Some(Mat4::from_translation(
                Vec3::from_array(target) + Vec3::Y * 1.7,
            ));
            for _ in 0..650 {
                let previous = Vec2::new(e.position.x, e.position.z);
                e.tick(&mut scene, 0.1, head, [None; 2]);
                assert!(!blocked(
                    previous,
                    Vec2::new(e.position.x, e.position.z),
                    &e.nav_obstacles
                ));
            }
            assert!(
                Vec2::new(e.position.x - target[0], e.position.z - target[2]).length() < 1.12,
                "spawn {spawn:?} stuck at {:?}",
                e.position
            );
        }
    }
    #[test]
    fn chase_moves_at_one_meter_per_second_and_stops_near_head() {
        let (mut scene, mut e) = fixture();
        let hands = [None; 2];
        let head = Some(Mat4::from_translation(Vec3::new(0., 1.7, 4.)));
        for _ in 0..10 {
            e.tick(&mut scene, 0.1, head, hands);
        }
        assert!((e.position.z - 1.).abs() < 0.001);
        for _ in 0..50 {
            e.tick(&mut scene, 0.1, head, hands);
        }
        assert!((e.position.z - 2.9).abs() < 0.001);
        assert_eq!(e.state()["phase"], "attack");
    }
}
