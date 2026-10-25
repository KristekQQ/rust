//! Native table tennis: physical moving-surface impacts, spin, rules and free practice.
use crate::{
    scene::SceneManager,
    table_tennis_contact::{self as contact, Contact, Motion, Pose},
};
use glam::{EulerRot, Quat, Vec3};
use serde_json::{json, Value};
const STEP: f32 = 1. / 240.;
const RADIUS: f32 = 0.02;
const SURFACE: f32 = 0.76 + RADIUS;
const NET_Z: f32 = -2.2;
const PADDLE_RADIUS: f32 = 0.085;
const PADDLE_HALF: f32 = 0.006;
// WebXR: shaft -Z, face normal X. The grip stays at the handle's centre.
const PADDLE_FACE_OFFSET: Vec3 = Vec3::new(0., 0., -0.135);
#[cfg(test)]
const DESKTOP_FACE_OFFSET: Vec3 = Vec3::new(0., 0.135, 0.);
const DESKTOP_ROTATION: Vec3 =
    Vec3::new(std::f32::consts::FRAC_PI_2, 0., std::f32::consts::FRAC_PI_2);
const PRACTICE_ROTATION: Vec3 = Vec3::new(0., 0., std::f32::consts::FRAC_PI_2);
fn rotation(v: Vec3) -> Quat {
    Quat::from_euler(EulerRot::YXZ, v.y, v.x, v.z)
}
fn pose_lerp(a: Pose, b: Pose, t: f32) -> Pose {
    Pose {
        center: a.center.lerp(b.center, t),
        orientation: a.orientation.slerp(b.orientation, t).normalize(),
    }
}

pub struct Game {
    pub score_panel: Option<u32>,
    ids: [u32; 3],
    position: Vec3,
    velocity: Vec3,
    spin: Vec3,
    ball_rotation: Quat,
    paddles: [Vec3; 2],
    orientations: [Quat; 2],
    paddle_velocity: [Vec3; 2],
    command: Option<Vec3>,
    player_target: Vec3,
    player_orientation: Quat,
    tracking_ready: bool,
    accumulator: f64,
    score: [u32; 2],
    server: usize,
    last_hit: usize,
    own_bounce: bool,
    bounces: u32,
    rally_hits: u32,
    phase: &'static str,
    event: &'static str,
    delay: f32,
    sequence: u32,
    winner: Option<usize>,
    practice: bool,
    drop_pending: bool,
    time: f32,
    last_contact: [f32; 2],
}
impl Game {
    pub fn new(ball: u32, player: u32, opponent: u32) -> Self {
        let q = rotation(DESKTOP_ROTATION);
        Self {
            score_panel: None,
            ids: [ball, player, opponent],
            position: Vec3::new(0., 1.1, -1.08),
            velocity: Vec3::ZERO,
            spin: Vec3::ZERO,
            ball_rotation: Quat::IDENTITY,
            paddles: [Vec3::new(0., 1., -0.65), Vec3::new(0., 1., -3.75)],
            orientations: [q; 2],
            paddle_velocity: [Vec3::ZERO; 2],
            command: None,
            player_target: Vec3::new(0., 1., -0.65),
            player_orientation: q,
            tracking_ready: false,
            accumulator: 0.,
            score: [0; 2],
            server: 0,
            last_hit: 0,
            own_bounce: true,
            bounces: 0,
            rally_hits: 0,
            phase: "ready",
            event: "ready",
            delay: 0.7,
            sequence: 0,
            winner: None,
            practice: false,
            drop_pending: false,
            time: 0.,
            last_contact: [-10.; 2],
        }
    }
    pub fn reset_tracking(&mut self) {
        self.tracking_ready = false;
        self.accumulator = 0.;
        self.paddle_velocity = [Vec3::ZERO; 2];
    }
    pub fn reset(&mut self) {
        let panel = self.score_panel;
        let practice = self.practice;
        *self = Self::new(self.ids[0], self.ids[1], self.ids[2]);
        self.score_panel = panel;
        self.practice = practice;
    }
    pub fn set_practice(&mut self, enabled: bool) {
        self.practice = enabled;
        self.reset();
    }
    pub fn move_player(&mut self, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            self.command = Some(Vec3::new(x.clamp(-1.1, 1.1), y.clamp(0.82, 1.8), -0.65));
        }
    }
    pub fn serve(&mut self) {
        if self.practice {
            self.drop_pending = true;
            return;
        }
        if self.phase == "rally" || self.winner.is_some() {
            return;
        }
        self.sequence += 1;
        let direction = if self.server == 0 { -1. } else { 1. };
        let x = match self.sequence % 3 {
            0 => -0.28,
            1 => 0.,
            _ => 0.28,
        };
        self.position = Vec3::new(x, 1.1, NET_Z - direction * 1.12);
        self.velocity = Vec3::new(-x * 0.5, -1.5, direction * 3.5);
        self.spin = Vec3::ZERO;
        self.last_hit = self.server;
        self.own_bounce = true;
        self.bounces = 0;
        self.rally_hits = 0;
        self.phase = "rally";
        self.event = "serve";
    }
    pub fn state(&self) -> Value {
        json!({"score":self.score,"server":self.server,"winner":self.winner,"phase":self.phase,"event":self.event,
        "rallyHits":self.rally_hits,"ballPosition":self.position.to_array(),"ballVelocity":self.velocity.to_array(),"ballSpin":self.spin.to_array(),
        "practice":self.practice,"paddleVelocity":self.paddle_velocity[0].to_array(),"ballRadius":RADIUS,"paddleFacePosition":self.paddles[0].to_array(),"paddleNormal":(self.orientations[0]*Vec3::X).to_array()})
    }
    pub fn tick(&mut self, scene: &mut SceneManager, dt: f32) {
        if !dt.is_finite() || dt <= 0. || self.ids.iter().any(|id| !scene.object_exists(*id)) {
            return;
        }
        let old = Pose {
            center: self.paddles[0],
            orientation: self.orientations[0],
        };
        let target = if let Some(p) = self.command.take() {
            let q = rotation(if self.practice {
                PRACTICE_ROTATION
            } else {
                DESKTOP_ROTATION
            });
            let grip = p - q * PADDLE_FACE_OFFSET;
            let (y, x, z) = q.to_euler(EulerRot::YXZ);
            scene.set_object_transform(self.ids[1], grip, Vec3::new(x, y, z), Vec3::ONE);
            Pose {
                center: p,
                orientation: q,
            }
        } else {
            let t = scene.objects()[&self.ids[1]].transform;
            Pose {
                center: t.model().transform_point3(PADDLE_FACE_OFFSET),
                orientation: rotation(t.rotation),
            }
        };
        let valid = target.center.abs().max_element() < 100.;
        let continuous = valid
            && self.tracking_ready
            && dt <= 0.1
            && old.center.distance(target.center) <= 1.
            && old.orientation.angle_between(target.orientation) < 2.4;
        let start = if continuous { old } else { target };
        self.tracking_ready = valid;
        self.player_target = target.center;
        self.player_orientation = target.orientation;
        self.paddles[0] = start.center;
        self.orientations[0] = start.orientation;
        if self.drop_pending && valid {
            self.drop_pending = false;
            self.position = target.center + Vec3::Y * 0.42;
            self.velocity = Vec3::ZERO;
            self.spin = Vec3::ZERO;
            self.rally_hits = 0;
            self.phase = "rally";
            self.event = "drop";
            self.last_contact = [-10.; 2];
        }
        if !continuous {
            self.accumulator = 0.;
        }
        self.accumulator += f64::from(dt.min(0.1));
        let horizon = self.accumulator as f32;
        let steps = (self.accumulator / f64::from(STEP) + 1e-7).floor() as u32;
        for i in 0..steps {
            let alpha = (((i + 1) as f32 * STEP) / horizon).min(1.);
            let orientation = start
                .orientation
                .slerp(target.orientation, alpha)
                .normalize();
            let grip_start = start.center - start.orientation * PADDLE_FACE_OFFSET;
            let grip_end = target.center - target.orientation * PADDLE_FACE_OFFSET;
            let end = Pose {
                center: grip_start.lerp(grip_end, alpha) + orientation * PADDLE_FACE_OFFSET,
                orientation,
            };
            self.step_to(STEP, end);
            self.accumulator -= f64::from(STEP);
        }
        // Render exactly the integrated orientation, including the visible spin marks.
        let (y, x, z) = self.ball_rotation.to_euler(EulerRot::YXZ);
        scene.set_object_position(self.ids[0], self.position);
        scene.set_object_rotation(self.ids[0], Vec3::new(x, y, z));
        let ai = self.orientations[1];
        let (y, x, z) = ai.to_euler(EulerRot::YXZ);
        scene.set_object_transform(
            self.ids[2],
            if self.practice {
                Vec3::splat(-10000.)
            } else {
                self.paddles[1] - ai * PADDLE_FACE_OFFSET
            },
            Vec3::new(x, y, z),
            Vec3::ONE,
        );
        if let Some(p) = self
            .score_panel
            .and_then(|id| scene.info_panels.get_mut(&id))
        {
            p.update(vec![(
                "",
                if self.practice {
                    format!("ODRAZY {}", self.rally_hits)
                } else {
                    format!("{} : {}", self.score[0], self.score[1])
                },
            )]);
        }
    }
    fn point(&mut self, winner: usize, reason: &'static str) {
        if self.practice || self.phase != "rally" {
            return;
        }
        self.score[winner] += 1;
        self.event = reason;
        self.velocity = Vec3::ZERO;
        if self.score[winner] >= 11 && self.score[winner] >= self.score[1 - winner] + 2 {
            self.winner = Some(winner);
            self.phase = "finished";
        } else {
            self.phase = "point";
            self.delay = 1.15;
            let total = self.score[0] + self.score[1];
            self.server = if total >= 20 {
                (total % 2) as usize
            } else {
                ((total / 2) % 2) as usize
            };
        }
    }
    fn bounce(&mut self) {
        if self.practice {
            return;
        }
        let side = if self.position.z > NET_Z { 0 } else { 1 };
        if self.own_bounce {
            if side != self.last_hit {
                self.point(1 - self.last_hit, "serve_fault");
                return;
            }
            self.own_bounce = false;
        } else if side == self.last_hit {
            self.point(1 - self.last_hit, "wrong_side");
        } else {
            self.bounces += 1;
            if self.bounces >= 2 {
                self.point(self.last_hit, "double_bounce");
            }
        }
    }
    fn ai_pose(&self, dt: f32) -> Pose {
        if self.practice {
            return Pose {
                center: self.paddles[1],
                orientation: self.orientations[1],
            };
        }
        let travel = if self.velocity.z < -0.1 {
            ((-3.7 - self.position.z) / self.velocity.z).clamp(0., 0.45)
        } else {
            0.
        };
        let striking = self.velocity.z < -0.1 && (self.position.z - self.paddles[1].z).abs() < 0.45;
        let target = Vec3::new(
            (self.position.x + self.velocity.x * travel).clamp(-0.9, 0.9),
            (self.position.y + self.velocity.y * travel - 4.905 * travel * travel).clamp(0.8, 1.6),
            if striking { -3.35 } else { -3.75 },
        );
        let center = self.paddles[1]
            + (target - self.paddles[1]).clamp_length_max(if striking {
                1.3 * dt
            } else {
                2.2 * dt
            });
        let desired = Vec3::new(
            (-self.position.x / 0.65).clamp(-1., 1.),
            (SURFACE - self.position.y) / 0.65 + 4.905 * 0.65,
            (-1.35 - self.position.z) / 0.65,
        );
        let n = if self.velocity.z < -0.1 {
            (desired - self.velocity).normalize_or_zero()
        } else {
            Vec3::Z
        };
        let q =
            Quat::from_rotation_arc(Vec3::X, if n.length_squared() > 0.5 { n } else { Vec3::Z });
        let max_angle = 6. * dt;
        let angle = self.orientations[1].angle_between(q);
        Pose {
            center,
            orientation: self.orientations[1]
                .slerp(
                    q,
                    if angle > max_angle {
                        max_angle / angle
                    } else {
                        1.
                    },
                )
                .normalize(),
        }
    }
    #[cfg(test)]
    fn step(&mut self, dt: f32) {
        self.step_to(
            dt,
            Pose {
                center: self.paddles[0],
                orientation: self.orientations[0],
            },
        );
    }
    fn step_to(&mut self, dt: f32, human_end: Pose) {
        self.time += dt;
        let motions = [
            Motion {
                start: Pose {
                    center: self.paddles[0],
                    orientation: self.orientations[0],
                },
                end: human_end,
                duration: dt,
            },
            Motion {
                start: Pose {
                    center: self.paddles[1],
                    orientation: self.orientations[1],
                },
                end: self.ai_pose(dt),
                duration: dt,
            },
        ];
        for (i, m) in motions.iter().enumerate() {
            self.paddles[i] = m.end.center;
            self.orientations[i] = m.end.orientation;
            self.paddle_velocity[i] = (m.end.center - m.start.center) / dt;
        }
        if self.phase != "rally" {
            if !self.practice && self.winner.is_none() {
                self.delay -= dt;
                if self.delay <= 0. {
                    self.serve();
                }
            }
            return;
        }
        // Approximate quadratic air drag and Magnus lift; coefficients are exposed here for calibration.
        let acceleration = -Vec3::Y * 9.81 - self.velocity * self.velocity.length() * 0.14
            + self.spin.cross(self.velocity) * 0.0008;
        self.velocity += acceleration * dt;
        self.spin *= (-0.35 * dt).exp();
        self.ball_rotation =
            (Quat::from_scaled_axis(self.spin * dt) * self.ball_rotation).normalize();
        let mut remaining = dt;
        for _ in 0..8 {
            if remaining < 1e-7 || self.phase != "rally" {
                break;
            }
            let start = self.position;
            let end = start + self.velocity * remaining;
            let elapsed = dt - remaining;
            let mut hit = 0;
            let mut fraction = 1.;
            let mut paddle_contact = None;
            if self.velocity.y < 0. {
                for (plane, kind) in [(SURFACE, 1), (RADIUS, 2)] {
                    if start.y >= plane && end.y <= plane {
                        let t = (start.y - plane) / (start.y - end.y);
                        let p = start.lerp(end, t);
                        if t <= fraction
                            && (kind == 2 || (p.x.abs() <= 0.7625 && p.z >= -3.57 && p.z <= -0.83))
                        {
                            fraction = t;
                            hit = kind;
                        }
                    }
                }
            }
            if self.velocity.z.abs() > 1e-6 {
                let face = NET_Z - self.velocity.z.signum() * (RADIUS + 0.008);
                let t = (face - start.z) / (end.z - start.z);
                if t >= 0. && t <= fraction {
                    let p = start.lerp(end, t);
                    if p.x.abs() <= 0.78 + RADIUS && p.y <= 0.9125 + RADIUS && p.y >= 0.76 - RADIUS
                    {
                        fraction = t;
                        hit = 3;
                    }
                }
            }
            for (i, motion) in motions.iter().enumerate() {
                if (i == 0 && !self.tracking_ready) || (i == 1 && self.practice) {
                    continue;
                }
                let slice = Motion {
                    start: pose_lerp(motion.start, motion.end, elapsed / dt),
                    end: motion.end,
                    duration: remaining,
                };
                if let Some(c) =
                    contact::sweep_ball(start, end, RADIUS, slice, PADDLE_RADIUS, PADDLE_HALF)
                {
                    if c.fraction <= fraction {
                        fraction = c.fraction;
                        hit = 4 + i;
                        paddle_contact = Some(c);
                    }
                }
            }
            self.position = start.lerp(end, fraction);
            remaining *= 1. - fraction;
            match hit {
                0 => break,
                1 | 2 => {
                    if hit == 1 {
                        self.bounce();
                    } else if !self.practice {
                        let winner = if !self.own_bounce && self.bounces > 0 {
                            self.last_hit
                        } else {
                            1 - self.last_hit
                        };
                        self.point(winner, "floor");
                    }
                    if self.phase == "rally" {
                        let c = Contact {
                            fraction,
                            normal: Vec3::Y,
                            point: self.position - Vec3::Y * RADIUS,
                            surface_velocity: Vec3::ZERO,
                            corrected_center: self.position + Vec3::Y * 0.00001,
                        };
                        let e = if self.velocity.y.abs() < 0.25 {
                            0.
                        } else if hit == 1 {
                            0.89
                        } else {
                            0.55
                        };
                        (self.velocity, self.spin) =
                            contact::resolve(self.velocity, self.spin, &c, RADIUS, e, 0.16);
                        self.position = c.corrected_center;
                        self.event = if hit == 1 { "bounce" } else { "floor" };
                    }
                }
                3 => {
                    if self.practice {
                        self.velocity.z = -self.velocity.z * 0.25;
                        self.position.z += self.velocity.z.signum() * 0.0001;
                        self.event = "net";
                    } else {
                        self.point(1 - self.last_hit, "net");
                    }
                }
                _ => {
                    let i = hit - 4;
                    let c = paddle_contact.unwrap();
                    let closing = -(self.velocity - c.surface_velocity).dot(c.normal);
                    self.position = c.corrected_center;
                    let e = if closing < 0.25 { 0. } else { 0.85 };
                    (self.velocity, self.spin) =
                        contact::resolve(self.velocity, self.spin, &c, RADIUS, e, 0.22);
                    if closing > 0.25 && self.time - self.last_contact[i] > 0.08 {
                        self.last_contact[i] = self.time;
                        if !self.practice
                            && (i == self.last_hit || self.own_bounce || self.bounces == 0)
                        {
                            self.point(1 - i, "volley");
                            break;
                        }
                        self.last_hit = i;
                        self.bounces = 0;
                        self.rally_hits += 1;
                        self.event = "paddle";
                    }
                }
            }
        }
        if self.phase == "rally"
            && (self.position.y < -0.1
                || self.position.z < -6.
                || self.position.z > 2.
                || self.position.x.abs() > 4.)
        {
            if self.practice {
                self.phase = "ready";
                self.velocity = Vec3::ZERO;
                self.event = "out";
            } else {
                let w = if !self.own_bounce && self.bounces > 0 {
                    self.last_hit
                } else {
                    1 - self.last_hit
                };
                self.point(w, "out");
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rally() -> Game {
        let mut g = Game::new(1, 2, 3);
        g.tracking_ready = true;
        g.serve();
        g
    }
    #[test]
    fn serve_bounces_on_both_sides() {
        let mut g = rally();
        for _ in 0..170 {
            g.step(STEP);
        }
        assert!(!g.own_bounce);
        assert!(g.bounces > 0 || g.rally_hits > 0);
        assert_eq!(g.score, [0, 0]);
    }
    #[test]
    fn fast_ball_cannot_tunnel_table_or_net() {
        let mut g = rally();
        g.own_bounce = false;
        g.position = Vec3::new(0., 1., -3.);
        g.velocity = Vec3::new(0., -150., 0.);
        g.step(STEP);
        assert!(g.velocity.y > 0.);
        assert_eq!(g.bounces, 1);
        let mut g = rally();
        g.position = Vec3::new(0., 0.86, -2.);
        g.velocity = Vec3::new(0., 0., -150.);
        g.step(STEP);
        assert_eq!(g.score, [0, 1]);
        assert_eq!(g.event, "net");
    }
    #[test]
    fn fast_ball_hits_finite_paddle() {
        let mut g = rally();
        g.last_hit = 1;
        g.own_bounce = false;
        g.bounces = 1;
        g.position = Vec3::new(0., 1., -0.95);
        g.velocity = Vec3::new(0., 0., 150.);
        g.step(STEP);
        assert_eq!(g.last_hit, 0);
        assert!(g.velocity.z < 0.);
        let mut g = rally();
        g.last_hit = 1;
        g.own_bounce = false;
        g.bounces = 1;
        g.position = Vec3::new(0.6, 1., -0.95);
        g.velocity = Vec3::new(0., 0., 150.);
        g.step(STEP);
        assert_eq!(g.last_hit, 1);
    }
    #[test]
    fn legality_scoring_and_win_by_two() {
        let mut g = rally();
        g.own_bounce = false;
        g.position.z = -3.;
        g.bounce();
        g.bounce();
        assert_eq!(g.score, [1, 0]);
        g.score = [10, 10];
        g.phase = "rally";
        g.point(0, "out");
        assert!(g.winner.is_none());
        g.phase = "rally";
        g.point(0, "out");
        assert_eq!(g.winner, Some(0));
        g.reset();
        assert_eq!(g.score, [0, 0]);
        assert_eq!(g.phase, "ready");
        assert_eq!(g.ids, [1, 2, 3]);
    }
    #[test]
    fn frame_splitting_is_stable() {
        let mut a = rally();
        let mut b = rally();
        // Same fixed-step accumulator path as tick, without scene fixtures.
        for _ in 0..60 {
            a.accumulator += 1. / 60.;
            while a.accumulator + 1e-10 >= f64::from(STEP) {
                a.step(STEP);
                a.accumulator -= f64::from(STEP);
            }
        }
        for _ in 0..120 {
            b.accumulator += 1. / 120.;
            while b.accumulator + 1e-10 >= f64::from(STEP) {
                b.step(STEP);
                b.accumulator -= f64::from(STEP);
            }
        }
        assert_eq!(a.score, b.score);
        assert!((a.position - b.position).length() < 1e-6);
    }
    fn scene_game() -> (SceneManager, Game) {
        let mut scene = SceneManager::default();
        let ball = scene.add_sphere(Vec3::ZERO);
        let player = scene.add_cube(Vec3::new(0., 1., -0.4) - DESKTOP_FACE_OFFSET);
        scene.set_object_rotation(player, DESKTOP_ROTATION);
        let ai = scene.add_cube(Vec3::new(0., 1., -3.75));
        let mut game = Game::new(ball, player, ai);
        game.paddles[0] = Vec3::new(0., 1., -0.4);
        game.serve();
        game.last_hit = 1;
        game.own_bounce = false;
        game.bounces = 1;
        game.position = Vec3::new(0., 1., -0.7);
        game.velocity = Vec3::ZERO;
        (scene, game)
    }
    #[test]
    fn actual_tick_sweeps_moving_paddle_at_30_and_120_hz() {
        for hz in [30, 120] {
            let (mut scene, mut game) = scene_game();
            for frame in 1..=hz / 10 {
                scene.set_object_position(
                    game.ids[1],
                    Vec3::new(0., 1., -0.4 - 5. * frame as f32 / hz as f32) - DESKTOP_FACE_OFFSET,
                );
                game.tick(&mut scene, 1. / hz as f32);
            }
            assert_eq!(game.last_hit, 0, "rate {hz}");
            assert_eq!(game.rally_hits, 1);
        }
    }
    #[test]
    fn rotated_grip_uses_offset_face_instead_of_hand_as_contact_center() {
        let (mut scene, mut game) = scene_game();
        let grip = Vec3::new(0.4, 1., -0.4);
        scene.set_object_transform(
            game.ids[1],
            grip,
            Vec3::new(0., std::f32::consts::FRAC_PI_2, 0.),
            Vec3::ONE,
        );
        game.tick(&mut scene, 1. / 60.);
        assert!((game.player_target - (grip - Vec3::X * 0.135)).length() < 1e-5);
        // Ball crosses the face at a point outside a hypothetical disk at the hand.
        game.paddles[0] = game.player_target;
        game.position = game.player_target + Vec3::new(-0.05, 0., -0.2);
        game.velocity = Vec3::Z * 100.;
        game.step(STEP);
        assert_eq!(game.rally_hits, 1);
    }
    #[test]
    fn paddle_does_not_extrapolate_or_sweep_tracking_loss() {
        let (mut scene, mut game) = scene_game();
        game.position.z = -0.85;
        scene.set_object_position(game.ids[1], Vec3::new(0., 1., -0.6) - DESKTOP_FACE_OFFSET);
        game.tick(&mut scene, 1. / 30.);
        assert_eq!(game.rally_hits, 0);
        scene.set_object_position(game.ids[1], Vec3::splat(100.));
        game.tick(&mut scene, 1. / 30.);
        scene.set_object_position(game.ids[1], Vec3::new(0., 1., -0.9) - DESKTOP_FACE_OFFSET);
        game.tick(&mut scene, 1. / 30.);
        assert_eq!(game.rally_hits, 0);
    }
    #[test]
    fn stationary_horizontal_paddle_bounces_drop_without_table_rule_or_auto_aim() {
        let (mut scene, mut g) = scene_game();
        g.set_practice(true);
        g.move_player(0., 1.);
        g.tick(&mut scene, 1. / 120.);
        g.serve();
        let mut upward = false;
        for _ in 0..140 {
            g.tick(&mut scene, 1. / 240.);
            if g.event == "paddle" && g.velocity.y > 0. {
                upward = true;
                break;
            }
        }
        assert!(upward, "{:?}", g.state());
        assert_eq!(g.rally_hits, 1);
        assert_eq!(g.score, [0, 0]);
        assert!(
            g.velocity.x.abs() < 0.001 && g.velocity.z.abs() < 0.001,
            "no forced opponent direction"
        );
        assert!(g.velocity.y > 1. && g.velocity.y < 3.5);
    }
    #[test]
    fn practice_reset_and_mode_commands_preserve_bindings_and_clear_spin() {
        let mut g = Game::new(1, 2, 3);
        g.score_panel = Some(4);
        g.set_practice(true);
        g.spin = Vec3::ONE * 100.;
        g.reset();
        assert!(g.practice);
        assert_eq!(g.score_panel, Some(4));
        assert_eq!(g.spin, Vec3::ZERO);
        g.set_practice(false);
        assert!(!g.practice);
        assert_eq!(g.phase, "ready");
    }
    #[test]
    fn tracked_speed_is_constant_at_72_90_and_120_hz() {
        for hz in [72, 90, 120] {
            let (mut scene, mut g) = scene_game();
            g.set_practice(true);
            g.move_player(0., 1.);
            g.tick(&mut scene, 1. / 120.);
            let base = scene.objects()[&g.ids[1]].transform.position;
            for frame in 1..=30 {
                scene.set_object_position(
                    g.ids[1],
                    base + Vec3::X * (0.5 * frame as f32 / hz as f32),
                );
                g.tick(&mut scene, 1. / hz as f32);
                assert!(
                    (g.paddle_velocity[0].x - 0.5).abs() < 0.0005,
                    "hz {hz}, frame {frame}, {:?}",
                    g.paddle_velocity[0]
                );
            }
        }
    }
    #[test]
    fn standard_table_drop_rebounds_about_23cm_from_30cm() {
        let mut g = Game::new(1, 2, 3);
        g.practice = true;
        g.phase = "rally";
        g.position = Vec3::new(0., SURFACE + 0.3, -2.8);
        let mut bounced = false;
        let mut apex = SURFACE;
        for _ in 0..220 {
            g.step(STEP);
            if g.event == "bounce" {
                bounced = true;
            }
            if bounced {
                apex = apex.max(g.position.y);
            }
        }
        assert!(bounced);
        assert!(
            (apex - SURFACE - 0.23).abs() < 0.015,
            "height {}",
            apex - SURFACE
        );
    }
}
