//! Deterministic source recording: scene transforms are authored by Rust at60Hz.
use glam::Vec3;
use serde_json::json;
use webgpu_wasm::scene::{wire::WirePublisher, SceneManager};
fn main() -> Result<(), String> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let mut scene = SceneManager::default();
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    const RINGS: usize = 8;
    const SIDES: usize = 12;
    positions.extend([0., 0.34, 0.]);
    for ring in 1..RINGS {
        let a = std::f32::consts::PI * ring as f32 / RINGS as f32;
        for side in 0..SIDES {
            let b = std::f32::consts::TAU * side as f32 / SIDES as f32;
            let radius = 0.36 * a.sin() * (1. + 0.08 * (b * 5.).cos() * a.sin());
            positions.extend([radius * b.cos(), 0.39 * a.cos(), radius * b.sin()]);
        }
    }
    let bottom = (positions.len() / 3) as u32;
    positions.extend([0., -0.36, 0.]);
    let vertex = |ring: usize, side: usize| 1 + ((ring - 1) * SIDES + side % SIDES) as u32;
    for side in 0..SIDES {
        indices.extend([0, vertex(1, side + 1), vertex(1, side)]);
        for ring in 1..RINGS - 1 {
            let a = vertex(ring, side);
            let b = vertex(ring, side + 1);
            let c = vertex(ring + 1, side);
            let d = vertex(ring + 1, side + 1);
            indices.extend([a, b, c, b, d, c]);
        }
        indices.extend([vertex(RINGS - 1, side), vertex(RINGS - 1, side + 1), bottom]);
    }
    let count = indices.len() / 3;
    let mut colors = Vec::new();
    for palette in 0..2 {
        for triangle in 0..count {
            let shade = 0.70 + ((triangle * 17) % 23) as f32 / 100.;
            let base = if palette == 0 {
                [0.95, 0.12, 0.065]
            } else {
                [0.18, 0.85, 0.32]
            };
            colors.extend(base.map(|v| v * shade));
        }
    }
    let red_model =
        scene.register_mesh_variants(&positions, &indices, &colors[..count * 3], true)?;
    let green_model =
        scene.register_mesh_variants(&positions, &indices, &colors[count * 3..], true)?;
    let mut fruit: Vec<_> = (0..5)
        .map(|i| scene.add_mesh_variant(if i % 2 == 0 { red_model } else { green_model }, 0))
        .collect::<Result<_, _>>()?;
    for &id in &fruit {
        scene.set_emission(id, 0.25);
        scene.set_object_scale(id, Vec3::splat(0.55));
    }
    let swords = [
        sword(&mut scene, [0.10, 0.75, 1.0])?,
        sword(&mut scene, [1.0, 0.28, 0.65])?,
    ];
    scene.add_light(Vec3::new(0., 4., 2.), Vec3::splat(1.8));
    let mut publisher = WirePublisher::default();
    let mut bytes = Vec::new();
    let mut frames = Vec::new();
    const FRAME_COUNT: usize = 2160;
    let mut cycles = [usize::MAX; 5];
    let mut halves: Vec<Vec<u32>> = vec![Vec::new(); 5];
    let mut cuts = Vec::new();
    // This hidden blade invokes the real engine CPU cutter, independently of presentation.
    let cutter = scene.add_cube(Vec3::new(0., -10., 0.));
    scene.set_object_scale(cutter, Vec3::splat(0.001));
    for frame in 0..FRAME_COUNT {
        let t = frame as f32 / 60.;
        let mut strikes: [Option<(f32, Vec3)>; 2] = [None, None];
        for i in 0..fruit.len() {
            let mut id = fruit[i];
            // Different launch times, ballistic arcs and spin; waiting fruit stays below the floor.
            let mut launch = -0.15 + i as f32 * 0.43;
            let mut n = 0;
            loop {
                let flight = 1.55 + noise(i * 71 + n * 13) * 0.85;
                let pause = 0.16 + noise(i * 19 + n * 23 + 5) * 0.85;
                if t < launch + flight + pause {
                    let age = t - launch;
                    let direction = if noise(i * 17 + n * 31) > 0.5 {
                        1.0
                    } else {
                        -1.0
                    };
                    let x0 = -direction * (0.7 + noise(n * 37 + i) * 0.65);
                    let vx = direction * (0.65 + noise(n * 29 + i * 11) * 0.8);
                    let g = 4.0;
                    let vy = g * flight * 0.5;
                    let position = if age >= 0.0 && age < flight {
                        Vec3::new(
                            x0 + vx * age,
                            0.18 + vy * age - 0.5 * g * age * age,
                            -1.65 + noise(i * 43 + n) * 0.7,
                        )
                    } else {
                        Vec3::new(0., -3., -1.5)
                    };
                    let rotation = Vec3::new(
                        age * (1.2 + noise(i + n) * 4.),
                        age * (noise(i * 3 + n) * 6. - 3.),
                        age * (noise(i + n * 7) * 4. - 2.),
                    );
                    if cycles[i] != n {
                        for piece in halves[i].drain(..) {
                            scene.remove_object(piece);
                        }
                        if cycles[i] != usize::MAX {
                            scene.remove_object(id);
                            id = scene.add_mesh_variant(
                                if i % 2 == 0 { red_model } else { green_model },
                                0,
                            )?;
                            fruit[i] = id;
                            scene.set_object_scale(id, Vec3::splat(0.55));
                            scene.set_emission(id, 0.25);
                        }
                        cycles[i] = n;
                    }
                    let cut_age = flight * (0.42 + noise(i * 7 + n) * 0.16);
                    let cut_position = Vec3::new(
                        x0 + vx * cut_age,
                        0.18 + vy * cut_age - 0.5 * g * cut_age * cut_age,
                        -1.65 + noise(i * 43 + n) * 0.7,
                    );
                    // Only one scheduled strike per hand at once; one fruit is left uncut for comparison.
                    if i < 4 && (age - cut_age).abs() < 0.32 {
                        let hand = i % 2;
                        let distance = age - cut_age;
                        if strikes[hand].is_none_or(|(old, _)| distance.abs() < old.abs()) {
                            strikes[hand] = Some((distance, cut_position));
                        }
                    }
                    if halves[i].is_empty() {
                        scene.set_object_position(id, position);
                        scene.set_object_rotation(id, rotation);
                        if i < 4 && age >= cut_age && age < flight {
                            scene.set_cuttable(id, true, Vec3::new(1., 0.78, 0.38))?;
                            scene.set_object_position(cutter, position);
                            scene.set_cutter(
                                cutter,
                                Some([Vec3::new(0., -800., 600.), Vec3::new(0., 800., 600.)]),
                            )?;
                            let result = scene.cut_with(cutter, Vec3::new(0., 0., -1.2), 0.)?;
                            if result.pieces.len() != 2 || result.removed != vec![id] {
                                return Err(format!(
                                    "Expected real two-half cut for fruit {id}, got {}",
                                    result.pieces.len()
                                ));
                            }
                            for &piece in &result.pieces {
                                scene.set_cuttable(piece, false, Vec3::ONE)?;
                            }
                            cuts.push(json!({"frame":frame,"removed":id,"pieces":result.pieces}));
                            halves[i] = result.pieces;
                        }
                    }
                    if !halves[i].is_empty() {
                        let elapsed = (age - cut_age).max(0.);
                        for (j, &piece) in halves[i].iter().enumerate() {
                            let side = if j == 0 { -1. } else { 1. };
                            // Continue the throw, separate the actual capped halves and give them independent spin.
                            scene.set_object_position(
                                piece,
                                position
                                    + Vec3::new(
                                        side * elapsed * 0.8,
                                        -elapsed * elapsed * 0.35,
                                        side * elapsed * 0.25,
                                    ),
                            );
                            scene.set_object_rotation(
                                piece,
                                rotation
                                    + Vec3::new(
                                        side * elapsed * 2.4,
                                        elapsed * 0.8,
                                        side * elapsed * 1.8,
                                    ),
                            );
                        }
                    }
                    break;
                }
                launch += flight + pause;
                n += 1;
            }
        }
        for (hand, &id) in swords.iter().enumerate() {
            // Uneven holds and quick swings, with independently generated poses for each hand.
            let mut at = 0.;
            let mut segment = 0;
            loop {
                let span = 0.16 + noise(segment * 53 + hand * 109) * 0.70;
                if t < at + span {
                    let u = ((t - at) / span).clamp(0., 1.);
                    let u = u * u * (3. - 2. * u);
                    let (pa, ra) = sword_pose(hand, segment);
                    let (pb, rb) = sword_pose(hand, segment + 1);
                    scene.set_object_position(id, pa.lerp(pb, u));
                    scene.set_object_rotation(id, ra.lerp(rb, u));
                    break;
                }
                at += span;
                segment += 1;
            }
        }
        for (hand, strike) in strikes.iter().enumerate() {
            if let Some((age, center)) = strike {
                // The blade sweeps through the fruit at its authored cut time; the source controls this event.
                scene
                    .set_object_position(swords[hand], *center + Vec3::new(0., -0.56, -age * 3.75));
                scene.set_object_rotation(swords[hand], Vec3::ZERO);
            }
        }
        let wire = publisher.capture(&mut scene)?;
        frames.push(json!({"offset":bytes.len(),"length":wire.len()}));
        bytes.extend(wire);
    }
    std::fs::write(out.join("frames.bin"), &bytes).map_err(|e| e.to_string())?;
    std::fs::write(out.join("manifest.json"),json!({"sourceHz":60,"durationSeconds":36,"frames":frames,"objects":{"fruit":fruit,"swords":swords},"cuts":cuts,"description":"Rust-authored irregular throws and real CPU mesh cuts; no headset"}).to_string()).map_err(|e|e.to_string())?;
    println!("Generated2160 Rust scene states, {} bytes", bytes.len());
    Ok(())
}

// Stable pseudo-random samples keep the source reproducible without cyclic sine motion.
fn noise(seed: usize) -> f32 {
    let mut x = (seed as u32).wrapping_add(0x9e3779b9);
    x = (x ^ (x >> 16)).wrapping_mul(0x85ebca6b);
    x = (x ^ (x >> 13)).wrapping_mul(0xc2b2ae35);
    ((x ^ (x >> 16)) & 0xffff) as f32 / 65535.
}
fn sword_pose(hand: usize, n: usize) -> (Vec3, Vec3) {
    let s = n * 17 + hand * 991;
    let side = if hand == 0 { -1. } else { 1. };
    (
        Vec3::new(
            side * (0.35 + noise(s) * 0.8),
            0.55 + noise(s + 1) * 0.6,
            -0.65 - noise(s + 2) * 0.35,
        ),
        Vec3::new(
            (noise(s + 3) - 0.5) * 1.9,
            (noise(s + 4) - 0.5) * 1.5,
            (noise(s + 5) - 0.5) * 2.9,
        ),
    )
}
fn sword(scene: &mut SceneManager, color: [f32; 3]) -> Result<u32, String> {
    let mut p = Vec::new();
    let mut idx = Vec::new();
    let mut colors = Vec::new();
    for (lo, hi, c) in [
        ([-0.045, 0., -0.025], [0.045, 1.12, 0.025], color),
        (
            [-0.15, -0.035, -0.055],
            [0.15, 0.035, 0.055],
            [0.35, 0.4, 0.5],
        ),
        (
            [-0.035, -0.26, -0.035],
            [0.035, 0., 0.035],
            [0.12, 0.15, 0.21],
        ),
    ] {
        let b = (p.len() / 3) as u32;
        for v in [
            [lo[0], lo[1], lo[2]],
            [hi[0], lo[1], lo[2]],
            [hi[0], hi[1], lo[2]],
            [lo[0], hi[1], lo[2]],
            [lo[0], lo[1], hi[2]],
            [hi[0], lo[1], hi[2]],
            [hi[0], hi[1], hi[2]],
            [lo[0], hi[1], hi[2]],
        ] {
            p.extend(v);
        }
        for face in [
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [3, 7, 6],
            [3, 6, 2],
            [0, 4, 7],
            [0, 7, 3],
            [1, 2, 6],
            [1, 6, 5],
        ] {
            idx.extend(face.map(|v| b + v));
            colors.extend(c);
        }
    }
    let model = scene.register_mesh_variants(&p, &idx, &colors, true)?;
    let id = scene.add_mesh_variant(model, 0)?;
    scene.set_emission(id, 0.65);
    Ok(id)
}
