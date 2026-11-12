//! Diagnostic player input, never scene authoring or a substitute for device tracking.
//!
//! The host supplies only live original fruit from its latest scene publication.
//! Strokes lock onto a fruit while preparing, then follow through even after a hit.
use glam::{Mat4, Quat, Vec3};
use webgpu_wasm::frame_input::FrameInput;

const SPEED: f32 = 3.0;
const REST: [Vec3; 2] = [Vec3::new(-0.25, 1.05, -0.38), Vec3::new(0.25, 1.05, -0.38)];

struct Stroke {
    target: u32,
    hand: usize,
    diagonal: f32,
    endpoint: Vec3,
    swinging: bool,
    age: f32,
}

pub struct Replay {
    cutting: bool,
    time: f64,
    positions: [Vec3; 2],
    stroke: Option<Stroke>,
    next_hand: usize,
    stroke_count: usize,
    cooldown: f32,
}

impl Replay {
    pub fn new(cutting: bool) -> Self {
        Self {
            cutting,
            time: 0.,
            positions: REST,
            stroke: None,
            next_hand: 1,
            stroke_count: 0,
            cooldown: 0.25,
        }
    }

    pub fn elapsed(&self) -> f32 {
        self.time as f32
    }

    pub fn head(&self) -> Mat4 {
        let t = self.elapsed();
        let yaw = 0.10 * (t * 0.27).sin();
        Mat4::from_rotation_translation(
            Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-0.14),
            Vec3::new(0., 1.65, 0.),
        )
    }

    pub fn frame(&mut self, dt: f32, targets: &[(u32, Vec3)]) -> FrameInput {
        // Match the production worker's effective interval, including slow-frame
        // tails; the same interval bounds movement and advances the game clock.
        let step = if dt.is_finite() && dt > 0. {
            dt.min(0.1)
        } else {
            0.
        };
        self.time += f64::from(step);
        self.cooldown = (self.cooldown - step).max(0.);
        if self.cutting && self.stroke.is_none() && self.cooldown == 0. {
            let hand = self.next_hand;
            let target = targets
                .iter()
                .filter(|(_, p)| reachable(*p))
                .min_by(|a, b| {
                    let cost = |p: Vec3| {
                        // Prefer the active hand's side, while allowing a modest crossover.
                        let side = if hand == 0 { -1. } else { 1. };
                        self.positions[hand].distance_squared(grip(p)) + (-side * p.x).max(0.) * 0.4
                    };
                    cost(a.1).total_cmp(&cost(b.1)).then(a.0.cmp(&b.0))
                });
            if let Some(&(target, center)) = target {
                self.stroke = Some(Stroke {
                    target,
                    hand,
                    diagonal: if self.stroke_count % 2 == 0 {
                        0.
                    } else if hand == 0 {
                        0.18
                    } else {
                        -0.18
                    },
                    endpoint: grip(center),
                    swinging: false,
                    age: 0.,
                });
            }
        }
        let mut goals = REST;
        let mut finished = false;
        if let Some(stroke) = &mut self.stroke {
            stroke.age += step;
            if !stroke.swinging {
                if let Some((_, center)) = targets
                    .iter()
                    .find(|(id, p)| *id == stroke.target && reachable(*p))
                {
                    let center = grip(*center);
                    let preparation = center + Vec3::new(-stroke.diagonal, 0.30, 0.);
                    goals[stroke.hand] = preparation;
                    if self.positions[stroke.hand].distance(preparation) <= 0.05 {
                        stroke.swinging = true;
                        stroke.endpoint = center + Vec3::new(stroke.diagonal, -0.38, 0.);
                    }
                } else {
                    finished = true;
                }
            }
            if stroke.swinging {
                goals[stroke.hand] = stroke.endpoint;
                finished = self.positions[stroke.hand].distance(stroke.endpoint) <= 0.025;
            }
            // A missed/falling target cannot hold the controller in pursuit forever.
            finished |= stroke.age > 0.85;
        }
        for (position, goal) in self.positions.iter_mut().zip(goals) {
            *position += (goal - *position).clamp_length_max(SPEED * step);
        }
        if finished {
            self.stroke = None;
            self.next_hand = 1 - self.next_hand;
            self.stroke_count += 1;
            self.cooldown = 0.12;
        }
        let rotation = if self.cutting {
            Quat::IDENTITY
        } else {
            Quat::from_rotation_x(-1.0)
        };
        FrameInput {
            dt: step,
            head: Some(self.head()),
            hands: self
                .positions
                .map(|p| Some(Mat4::from_rotation_translation(rotation, p))),
            // Ordinary trigger edge banks stock occasionally; no restart or UI ray.
            triggers: [self.cutting && (self.elapsed() % 4.0) < 0.04; 2],
            primary: false,
            ..Default::default()
        }
    }
}

fn reachable(p: Vec3) -> bool {
    p.is_finite()
        && p.x.abs() <= 0.72
        && (0.85..=1.85).contains(&p.y)
        && (-1.50..=-0.80).contains(&p.z)
}

fn grip(p: Vec3) -> Vec3 {
    // The original blade occupies local -Z from -.10 to -.95: center it .67m out.
    Vec3::new(
        p.x.clamp(-0.65, 0.65),
        p.y.clamp(0.85, 1.85),
        (p.z + 0.67).clamp(-0.55, -0.25),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{content::Content, game::Game};
    use std::collections::BTreeSet;
    use webgpu_wasm::scene::SceneManager;

    #[test]
    fn finite_bounded_motion_with_disappearing_and_jumping_targets() {
        for hz in [72, 120] {
            let dt = 1. / hz as f32;
            let mut replay = Replay::new(true);
            let mut baseline = Replay::new(false);
            let mut previous = replay.positions;
            for frame in 0..hz * 12 {
                let targets = if frame % 23 < 15 {
                    vec![
                        (
                            frame / 23,
                            Vec3::new(if frame % 2 == 0 { -0.6 } else { 0.6 }, 1.3, -1.1),
                        ),
                        (99, Vec3::splat(f32::NAN)),
                    ]
                } else {
                    vec![]
                };
                let input = replay.frame(dt, &targets);
                let idle = baseline.frame(dt, &targets);
                assert_eq!(input.head, idle.head);
                assert!(!input.primary);
                for (i, pose) in input.hands.iter().enumerate() {
                    let pose = pose.unwrap();
                    assert!(pose.is_finite());
                    let position = pose.w_axis.truncate();
                    assert!(position.distance(previous[i]) <= SPEED * dt + 1e-6);
                    assert!(position.x.abs() <= 0.66 && (0.65..=2.2).contains(&position.y));
                    assert!((-0.56..=-0.24).contains(&position.z));
                    previous[i] = position;
                }
                assert_eq!(baseline.positions, REST);
            }
            for dt in [f32::NAN, f32::INFINITY, -1., 0., 10.] {
                assert!(replay.frame(dt, &[]).head.unwrap().is_finite());
            }
        }
    }

    #[test]
    fn slow_frame_tails_preserve_production_time_and_bounded_motion() {
        let mut replay = Replay::new(true);
        let targets = [(1, Vec3::new(0.55, 1.4, -1.1))];
        let mut time = 0.;
        for dt in [0.075_f32, 0.1, 0.075, 0.1, 0.2] {
            let previous = replay.positions;
            let frame = replay.frame(dt, &targets);
            let effective = dt.min(0.1);
            time += effective;
            assert_eq!(frame.dt, effective);
            assert!((replay.elapsed() - time).abs() < 1e-6);
            for (i, hand) in frame.hands.iter().enumerate() {
                let pose = hand.unwrap();
                assert!(pose.is_finite());
                assert!(pose.w_axis.truncate().distance(previous[i]) <= SPEED * effective + 1e-6);
            }
        }
    }

    #[test]
    fn worker_minute_with_stale_publication_targets_and_variable_cadence() {
        use crate::simulation::Simulation;
        use std::collections::VecDeque;

        let mut failures = Vec::new();
        for variable in [false, true] {
            'case: for stale_frames in [1, 2] {
                let mut scene = SceneManager::default();
                let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
                let content = Content::load(&mut scene, &|name| {
                    std::fs::read(root.join(name)).map_err(|e| e.to_string())
                })
                .unwrap();
                let game = Game::new(&mut scene, &content, None).unwrap();
                let mut simulation = Simulation::new(scene, game).unwrap();
                let initial = simulation.wait_initial().unwrap();
                let mut mirror = SceneManager::default();
                simulation.retire(mirror.apply_render_publication(initial.publication, &[]));
                let mut replay = Replay::new(true);
                let mut history = VecDeque::from(vec![Vec::new(); stale_frames]);
                let mut frame_index = 0;
                let mut state = initial.state;
                let mut haptics_seen = false;
                while replay.elapsed() < 60.1 {
                    let dt = if variable {
                        match frame_index % 97 {
                            47 => 0.075,
                            96 => 0.1,
                            n if n % 7 == 0 => 0.033,
                            n if n % 3 == 0 => 0.020,
                            _ => 1. / 60.,
                        }
                    } else {
                        1. / 60.
                    };
                    let targets = history.pop_front().unwrap();
                    let frame = replay.frame(dt, &targets);
                    simulation.submit(frame).unwrap_or_else(|error| {
                        panic!("submit: variable={variable}, stale={stale_frames}, frame={frame_index}, elapsed={}, state={state}, error={error}", replay.elapsed())
                    });
                    let snapshot = match simulation.wait_replay() {
                        Ok(snapshot) => snapshot,
                        Err(error) => {
                            let evidence = format!("worker: variable={variable}, stale={stale_frames}, frame={frame_index}, elapsed={}, state={state}, error={error}", replay.elapsed());
                            println!("{evidence}");
                            failures.push(evidence);
                            continue 'case;
                        }
                    };
                    state = snapshot.state;
                    simulation.retire(mirror.apply_render_publication(snapshot.publication, &[]));
                    let latest = state["flying"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|id| id.as_u64())
                        .filter_map(|id| {
                            mirror
                                .objects()
                                .get(&(id as u32))
                                .map(|object| (id as u32, object.transform.position))
                        })
                        .collect();
                    history.push_back(latest);
                    haptics_seen |= simulation.take_haptics().iter().any(|value| *value > 0.);
                    frame_index += 1;
                }
                println!("worker minute: variable={variable}, stale={stale_frames}, frames={frame_index}, haptics={haptics_seen}, state={state}");
                assert_eq!(state["cueIndex"], 24);
                assert_eq!(state["round"]["finished"], true);
                assert!((100..=140).contains(&state["shots"].as_u64().unwrap()));
                assert!(state["cuts"].as_u64().unwrap() > 10);
                assert!(haptics_seen);
                assert_eq!(simulation.dropped, 0);
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn authored_minute_replay_makes_real_cuts_and_feedback() {
        let mut scene = SceneManager::default();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let content = Content::load(&mut scene, &|name| {
            std::fs::read(root.join(name)).map_err(|e| e.to_string())
        })
        .unwrap();
        let mut game = Game::new(&mut scene, &content, None).unwrap();
        let original_ids: BTreeSet<_> = scene.objects().keys().copied().collect();
        let mut replay = Replay::new(true);
        let mut pieces_seen = false;
        let mut feedback_seen = false;
        let mut haptics_seen = false;
        for _ in 0..7205 {
            let dt = 1. / 120.;
            scene.update(dt);
            let targets: Vec<_> = game
                .flying
                .iter()
                .filter_map(|id| scene.objects().get(id).map(|o| (*id, o.transform.position)))
                .collect();
            let frame = replay.frame(dt, &targets);
            game.haptics = [0.; 2];
            game.tick(&mut scene, frame).unwrap();
            haptics_seen |= game.haptics.iter().any(|v| *v > 0.);
            pieces_seen |= game.state()["fragments"].as_u64().unwrap_or(0) >= 2;
            // Real pooled impact annulus: 128 vertices/256 faces, emitted at the cut.
            // Original authored scene, fruit, debris and blade trails are excluded.
            feedback_seen |= scene.triangle_meshes().iter().any(|(id, triangle)| {
                !original_ids.contains(id)
                    && scene.objects().get(id).is_some_and(|o| o.emission > 2.0)
                    && triangle
                        .mesh
                        .cpu()
                        .is_ok_and(|m| m.positions.len() == 128 && m.triangles.len() == 256)
            });
            assert!(game.flying.len() <= 10);
        }
        println!(
            "ordinary minute: shots={}, cuts={}, cues={}, pieces={pieces_seen}, impact_ring={feedback_seen}, haptics={haptics_seen}",
            game.shots, game.cuts, game.cue_index
        );
        assert_eq!(game.cue_index, 24);
        // All 140 recipes remain authored; ordinary gameplay may skip a whole
        // cue when surviving fruit plus its recipes exceed the ten-fruit cap.
        assert!((100..=140).contains(&game.shots), "{}", game.state());
        assert!(game.round.unwrap().finished());
        assert!(
            game.cuts > 10,
            "replay did not produce meaningful real cuts: {}",
            game.state()
        );
        assert!(
            pieces_seen && feedback_seen && haptics_seen,
            "pieces={pieces_seen}, impact_ring={feedback_seen}, haptics={haptics_seen}"
        );
    }
}
