use super::*;
use std::sync::{Arc, OnceLock};

fn fixture() -> (Game, SceneManager) {
    fixture_with_playing_pool(false)
}
fn fixture_with_playing_pool(playing: bool) -> (Game, SceneManager) {
    fixture_with_shadow_pool(playing, false)
}
fn fixture_with_shadow_pool(playing: bool, blobs: bool) -> (Game, SceneManager) {
    static ASSETS: OnceLock<Vec<Arc<crate::gltf_model::ModelAsset>>> = OnceLock::new();
    let assets = ASSETS.get_or_init(|| {
        [
            include_bytes!("../assets/models/rim/drone-orbiter.glb").as_slice(),
            include_bytes!("../assets/models/rim/core-tank.glb").as_slice(),
            include_bytes!("../assets/models/rim/blade-wraith.glb").as_slice(),
        ]
        .iter()
        .map(|bytes| Arc::new(crate::gltf_loader::parse_model(bytes, &[], &[]).unwrap()))
        .collect()
    });
    let layout: Value =
        serde_json::from_str(include_str!("../assets/models/rim/layout.json")).unwrap();
    let collision: Value =
        serde_json::from_str(include_str!("../assets/models/rim/collisions.json")).unwrap();
    let mut scene = SceneManager::default();
    let mut enemies = Vec::new();
    for (index, kind) in ["drone-orbiter", "core-tank", "blade-wraith"]
        .into_iter()
        .enumerate()
    {
        let spec = layout["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["slug"] == kind)
            .unwrap();
        for _ in 0..if index == 0 { 8 } else { 3 } {
            let id = scene.add_model(assets[index].clone()).unwrap();
            let offset: [f32; 3] =
                serde_json::from_value(spec["root"]["position"].clone()).unwrap();
            let scale: [f32; 3] = serde_json::from_value(spec["root"]["scale"].clone()).unwrap();
            scene.set_object_transform(
                id,
                Vec3::from_array(offset),
                Vec3::ZERO,
                Vec3::from_array(scale),
            );
            if playing {
                let name = if kind == "blade-wraith" {
                    "Run"
                } else {
                    "Idle"
                };
                let clip = assets[index]
                    .animations
                    .iter()
                    .position(|clip| clip.name.as_deref() == Some(name))
                    .unwrap();
                scene.play_model_animation(id, clip, true, 1.).unwrap();
            }
            enemies.push(EnemySpec {
                id,
                kind: kind.into(),
                height: spec["height"].as_f64().unwrap() as f32,
                offset,
                scale,
            });
        }
    }
    let swords = std::array::from_fn(|_| scene.add_cube(HIDDEN));
    let bolts = (0..32).map(|_| scene.add_cube(HIDDEN)).collect();
    let beams = (0..14).map(|_| scene.add_cube(HIDDEN)).collect();
    let charges = (0..14).map(|_| scene.add_cube(HIDDEN)).collect();
    let health_bars = (0..14)
        .map(|_| [scene.add_cube(HIDDEN), scene.add_cube(HIDDEN)])
        .collect();
    let hud = (0..5).map(|_| scene.add_cube(HIDDEN)).collect();
    let marker = scene.add_cube(HIDDEN);
    let shapes = serde_json::from_value(collision["shapes"].clone()).unwrap();
    let obstacles = collision["obstacles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            [
                r["minX"].as_f64().unwrap() as f32,
                r["maxX"].as_f64().unwrap() as f32,
                r["minZ"].as_f64().unwrap() as f32,
                r["maxZ"].as_f64().unwrap() as f32,
            ]
        })
        .collect();
    let game = Game::new(
        Config {
            enemies,
            swords,
            bolts,
            beams,
            charges,
            health_bars,
            blob_shadows: if blobs {
                (0..14).map(|_| scene.add_cube(HIDDEN)).collect()
            } else {
                Vec::new()
            },
            hud,
            marker,
            shapes,
            obstacles,
        },
        &mut scene,
    )
    .unwrap();
    (game, scene)
}
fn start(game: &mut Game, scene: &mut SceneManager) {
    game.input(
        Input {
            start: true,
            ..Default::default()
        },
        scene,
    )
    .unwrap();
}

#[test]
fn original_models_normalize_and_wave_waits_for_every_orbiter() {
    let (mut game, mut scene) = fixture();
    for e in &game.enemies {
        let model = scene.model(e.spec.id).unwrap();
        let (min, max) = crate::gltf_animation::model_bounds(&model.asset, &model.pose).unwrap();
        assert!(((max.y - min.y) * e.spec.scale[1] - e.spec.height).abs() < 0.01);
    }
    start(&mut game, &mut scene);
    assert_eq!(game.enemies.iter().filter(|e| e.phase == 1).count(), 6);
    assert!(game
        .enemies
        .iter()
        .filter(|e| e.phase == 1)
        .all(|e| e.spec.kind == "drone-orbiter"));
    let serial = game.enemies[0].serial;
    for i in 0..5 {
        game.defeat(&mut scene, i, Vec3::X);
        game.enemies[i].age = 3.21;
    }
    game.tick(&mut scene, 0.01, None, [None; 2]);
    assert_eq!(game.wave, 0);
    assert_eq!(game.kills, 5);
    assert_eq!(game.enemies[0].serial, serial);
    assert_eq!(game.enemies[5].phase, 1);
    game.defeat(&mut scene, 5, Vec3::Y);
    game.enemies[5].age = 3.21;
    game.cut_failed(); // This test retires deaths without a worker.
    game.tick(&mut scene, 0.01, None, [None; 2]);
    assert_eq!(game.wave, 1);
    assert_eq!(game.kills, 6);
    assert!(game.enemies.iter().all(|e| e.serial == serial + 1));
    let active: Vec<_> = game.enemies.iter().filter(|e| e.phase == 1).collect();
    assert_eq!(active.len(), 2);
    assert_eq!(active[0].spec.kind, "core-tank");
}

#[test]
fn swept_bolt_reflects_and_returns_to_original_enemy() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    game.enemies[0].position = Vec3::new(0., 1.04, 36.);
    let head = Vec3::new(0., 1.7, 42.);
    game.shoot(0, Vec3::new(0., 1.45, 39.5), head - Vec3::Y * 0.25);
    let blade = Blade {
        a: Vec3::new(-0.4, 1.45, 40.2),
        b: Vec3::new(0.4, 1.45, 40.2),
        grip: Vec3::new(-0.4, 1.45, 40.2),
        rotation: Quat::IDENTITY,
    };
    game.update_bolts(&mut scene, 0.5, head, &[Some((blade, blade, true)), None]);
    assert_eq!(game.reflected, 1);
    assert_eq!(game.health, 100.);
    assert_eq!(game.bolts.len(), 1);
    assert!(game.bolts[0].friendly && game.bolts[0].velocity.z < 0.);
    game.update_bolts(&mut scene, 1., head, &[None; 2]);
    assert_eq!(game.returned, 1);
    assert_eq!(game.kills, 1);
    assert_eq!(game.enemies[0].phase, 3);
    assert!(game.bolts.is_empty());
}

#[test]
fn imported_city_roof_blocks_fast_projectiles_and_supports_body() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    let from = Vec3::new(4., 25., -40.);
    let below = Vec3::new(4., 8., -40.);
    let roof = game.world.cast(from, below).expect("original temple roof");
    assert!(roof.normal.y > 0.55 && roof.normal.x.abs() > 0.1);
    assert!(roof.point.y > 14. && roof.point.y < 16.5);
    let landing = game.world.move_body(from, below - from, 1.7);
    assert!(landing.grounded);
    assert!(landing.position.y > roof.point.y);
    assert!(game.world.clear(landing.position, 1.7));
    game.shoot(0, from, below);
    game.bolts[0].velocity = (below - from) * 10.;
    game.update_bolts(&mut scene, 0.1, game.player.head(1.7), &[None; 2]);
    assert_eq!(game.blocked, 1);
    assert!(game.bolts.is_empty());
    assert_eq!(game.health, 100.);
}

#[test]
fn death_respawns_after_three_real_seconds() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    let serial = game.enemies[0].serial;
    for _ in 0..4 {
        game.damage(25.);
    }
    assert_eq!(game.health, 0.);
    assert_eq!(game.deaths, 1);
    assert_eq!(game.hits, 4);
    game.damage(25.);
    assert_eq!(game.deaths, 1);
    game.dead_age = 2.9;
    game.tick(&mut scene, 0.05, None, [None; 2]);
    assert_eq!(game.health, 0.);
    game.dead_age = 2.99;
    game.tick(&mut scene, 0.02, None, [None; 2]);
    assert_eq!(game.health, 100.);
    assert_eq!(game.enemies[0].serial, serial + 1);
    assert_eq!(game.enemies.iter().filter(|e| e.phase == 1).count(), 6);
}

#[test]
fn xr_tracking_discontinuity_does_not_sweep_through_enemy() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    game.set_xr(true);
    game.sim_dt = 0.01;
    let e = &mut game.enemies[0];
    e.position = Vec3::new(0., 1.04, 38.);
    e.beam_age = 0.;
    let head = Mat4::from_translation(game.player.head(1.7));
    let left = Mat4::from_translation(Vec3::new(-2., 1.45, 38.5));
    let right = Mat4::from_translation(Vec3::new(2., 1.45, 38.5));
    game.tick(&mut scene, 0.01, Some(head), [Some(left), None]);
    game.tick(&mut scene, 0.01, Some(head), [Some(right), None]);
    assert_eq!(game.kills, 0);
    assert_eq!(game.enemies[0].phase, 1);
    // Returning after tracking loss must initialize a fresh sample too.
    game.xr_move(None, None, [0.; 3], false, 0.01);
    game.tick(&mut scene, 0.01, Some(head), [Some(left), None]);
    assert_eq!(game.kills, 0);
}

#[test]
fn worker_batches_cover_full_original_mesh_and_ignore_late_serials() {
    let (mut game, mut scene) = fixture();
    game.wave = 2;
    start(&mut game, &mut scene);
    let owner = 11;
    assert_eq!(game.enemies[owner].phase, 1);
    game.defeat(&mut scene, owner, Vec3::X);
    let task = game.tasks.front().unwrap();
    let expected: usize = task
        .parts
        .iter()
        .map(|&(_, p)| {
            let p = &task.model.asset.primitives[p];
            if p.mode == crate::gltf_model::PrimitiveMode::Triangles {
                p.indices.len() / 3
            } else {
                0
            }
        })
        .sum();
    assert!(
        expected > 3500,
        "fixture must exercise multiple worker batches"
    );
    let mut triangle_count = 0;
    let mut results = Vec::new();
    loop {
        let bytes = game.take_cut_job().unwrap();
        if bytes.is_empty() {
            break;
        }
        let count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        assert!(count <= PREPARE_TRIANGLES);
        triangle_count += count;
        results.push(crate::rim_cut::split(&bytes).unwrap());
    }
    assert_eq!(triangle_count, expected);
    assert!(results.len() > 1);
    let last = results.pop().unwrap();
    for result in &results {
        game.accept_cut(&mut scene, result).unwrap();
        assert!(game.pieces.is_empty());
    }
    game.accept_cut(&mut scene, &last).unwrap();
    assert!(game.pending.is_empty());
    assert!(game.pieces.is_empty());
    finish_cut_staging(&mut game, &mut scene);
    assert!(game.completed.is_empty());
    assert!(!game.pieces.is_empty());
    assert_eq!(
        scene.objects()[&game.enemies[owner].spec.id]
            .transform
            .position,
        HIDDEN
    );
    // Reuse the real task encoder, then respawn before its worker reply arrives.
    game.enemies[owner].phase = 1;
    game.defeat(&mut scene, owner, Vec3::Y);
    let late = crate::rim_cut::split(&game.take_cut_job().unwrap()).unwrap();
    let old_serial = game.enemies[owner].serial;
    game.spawn(&mut scene);
    assert_ne!(game.enemies[owner].serial, old_serial);
    assert!(game.pieces.is_empty());
    let objects = scene.objects().len();
    game.accept_cut(&mut scene, &late).unwrap();
    assert!(game.pieces.is_empty());
    assert_eq!(scene.objects().len(), objects);
}

#[test]
fn actual_game_input_and_xr_stick_advance_the_city_player() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    game.input(
        Input {
            movement: Some([0., 1.]),
            ..Default::default()
        },
        &mut scene,
    )
    .unwrap();
    for _ in 0..30 {
        game.tick(&mut scene, 1. / 60., None, [None; 2]);
    }
    assert!(
        (game.player.position.z - 40.75).abs() < 0.002,
        "{}",
        game.state()
    );
    assert!((game.player.position.y).abs() < 0.001, "{}", game.state());

    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    game.set_xr(true);
    let physical_head = Mat4::from_translation(Vec3::new(0., 1.7, 0.));
    for _ in 0..15 {
        game.xr_move(Some(physical_head), None, [0., -1., 0.], false, 1. / 72.);
        let world_head = game.xr_origin * physical_head;
        game.tick(&mut scene, 1. / 72., Some(world_head), [None; 2]);
    }
    assert!(
        (game.player.position.z - (42. - 1.8 * 15. / 72.)).abs() < 0.002,
        "{}",
        game.state()
    );
    assert!(game.player.position.y.abs() < 0.001, "{}", game.state());
}

#[test]
fn plasma_integrates_only_original_damage_window_and_dead_xr_cannot_dash() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    for e in &mut game.enemies {
        e.shot = 100.;
    }
    game.enemies[0].beam_from = Vec3::new(0., 1.4, 38.);
    game.enemies[0].beam_target = Vec3::new(0., 1.4, 56.);
    game.enemies[0].beam_age = 0.84;
    game.tick(&mut scene, 0.05, None, [None; 2]);
    assert!((game.health - 95.).abs() < 0.002, "{}", game.state());
    game.enemies[0].beam_age = 2.84;
    game.tick(&mut scene, 0.05, None, [None; 2]);
    assert!((game.health - 93.75).abs() < 0.002, "{}", game.state());
    game.health = 0.;
    game.set_xr(true);
    game.xr_buttons(false, false);
    game.xr_buttons(false, true);
    let before = game.player.position;
    let stamina = game.player.stamina;
    game.xr_move(
        Some(Mat4::from_translation(Vec3::Y * 1.7)),
        None,
        [0., -1., 0.],
        false,
        0.05,
    );
    assert!(game.player.position.distance(before) < 0.001);
    assert_eq!(game.player.dash_count, 0);
    assert_eq!(game.player.stamina, stamina);
    assert!(!game.player.teleport(&game.world, Vec3::new(100., 0., 0.)));
}

fn assert_pool_frozen(game: &Game, scene: &mut SceneManager) {
    let snapshots: Vec<_> = game
        .enemies
        .iter()
        .filter(|e| e.phase == 0)
        .map(|e| {
            let model = scene.model(e.spec.id).unwrap();
            assert!(!model.playback.as_ref().is_some_and(|p| p.playing));
            (
                e.spec.id,
                model.revision,
                model.playback.as_ref().map(|p| p.time),
            )
        })
        .collect();
    assert!(!snapshots.is_empty());
    scene.update(0.03);
    for (id, revision, time) in snapshots {
        let model = scene.model(id).unwrap();
        assert_eq!(model.revision, revision);
        assert_eq!(model.playback.as_ref().map(|p| p.time), time);
        assert_eq!(scene.objects()[&id].transform.position, HIDDEN);
    }
}

#[test]
fn pooled_animations_freeze_and_spawn_restarts_each_kind() {
    let (mut game, mut scene) = fixture_with_playing_pool(true);
    assert_pool_frozen(&game, &mut scene);
    for (wave, name) in [(0, "Idle"), (1, "Idle"), (2, "Run"), (3, "Idle")] {
        game.wave = wave;
        game.spawn(&mut scene);
        for e in game.enemies.iter().filter(|e| e.phase == 1) {
            let model = scene.model(e.spec.id).unwrap();
            let playback = model.playback.as_ref().unwrap();
            assert!(playback.playing && playback.looping);
            assert_eq!(playback.time, 0.);
            assert_eq!(
                model.asset.animations[playback.clip].name.as_deref(),
                Some(name)
            );
        }
        assert_pool_frozen(&game, &mut scene);
        // Visibility alone must not freeze an active enemy outside the camera view.
        let id = game.enemies.iter().find(|e| e.phase == 1).unwrap().spec.id;
        scene.set_object_position(id, HIDDEN);
        let before = scene.model(id).unwrap().playback.as_ref().unwrap().time;
        scene.update(0.03);
        assert!(scene.model(id).unwrap().playback.as_ref().unwrap().time > before);
    }
}

#[test]
fn death_impact_pose_stays_intact_until_atomic_cut_publication() {
    for mode in ["cut", "dissolve"] {
        let (mut game, mut scene) = fixture_with_playing_pool(true);
        start(&mut game, &mut scene);
        game.death_mode = mode.into();
        let id = game.enemies[0].spec.id;
        let impact_pose = scene.model(id).unwrap().revision;
        game.defeat(&mut scene, 0, Vec3::X);
        assert!(!scene.model(id).unwrap().playback.as_ref().unwrap().playing);
        scene.update(0.03);
        assert_eq!(scene.model(id).unwrap().revision, impact_pose);
        let mut results = Vec::new();
        loop {
            let bytes = game.take_cut_job().unwrap();
            if bytes.is_empty() {
                break;
            }
            results.push(crate::rim_cut::split(&bytes).unwrap());
        }
        let last = results.pop().unwrap();
        for bytes in results {
            game.accept_cut(&mut scene, &bytes).unwrap();
        }
        assert!(!scene.model(id).unwrap().playback.as_ref().unwrap().playing);
        assert_ne!(scene.objects()[&id].transform.position, HIDDEN);
        game.accept_cut(&mut scene, &last).unwrap();
        assert!(game.pieces.is_empty());
        finish_cut_staging(&mut game, &mut scene);
        assert_eq!(game.enemies[0].phase, 3);
        assert!(!scene.model(id).unwrap().playback.as_ref().unwrap().playing);
        let frozen = scene.model(id).unwrap().revision;
        assert!(!game.pieces.is_empty());
        scene.update(3.);
        assert_eq!(scene.model(id).unwrap().revision, frozen);
        assert!(game.pieces.iter().all(|id| !scene.object_exists(*id)));
        game.enemies[0].age = 3.21;
        game.tick(&mut scene, 0.01, None, [None; 2]);
        assert_eq!(game.enemies[0].phase, 0);
        assert_pool_frozen(&game, &mut scene);
    }
}

#[test]
fn death_timeout_and_teardown_pause_pool_and_reject_late_cut_results() {
    let (mut game, mut scene) = fixture_with_playing_pool(true);
    start(&mut game, &mut scene);
    game.defeat(&mut scene, 0, Vec3::Y);
    game.enemies[0].age = 3.21;
    game.cut_failed(); // Explicit failure releases a stalled cut; timeout then retires the enemy.
    game.tick(&mut scene, 0.01, None, [None; 2]);
    assert_eq!(game.enemies[0].phase, 0);
    assert_pool_frozen(&game, &mut scene);
    game.defeat(&mut scene, 1, Vec3::Y);
    let late = crate::rim_cut::split(&game.take_cut_job().unwrap()).unwrap();
    game.teardown(&mut scene);
    assert!(!game.playing());
    assert!(game.tasks.is_empty() && game.pending.is_empty() && game.completed.is_empty());
    game.accept_cut(&mut scene, &late).unwrap();
    assert!(game.pieces.is_empty());
    assert_pool_frozen(&game, &mut scene);
    start(&mut game, &mut scene);
    assert_eq!(game.enemies.iter().filter(|e| e.phase == 1).count(), 6);
    assert!(game.enemies.iter().filter(|e| e.phase == 1).all(|e| {
        scene
            .model(e.spec.id)
            .unwrap()
            .playback
            .as_ref()
            .unwrap()
            .playing
    }));
}

#[test]
fn health_damage_is_nonfatal_until_zero_and_resets_for_all_six_waves() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    for (wave, kind, count, hp) in [
        (0, "drone-orbiter", 6, 100.),
        (1, "core-tank", 2, 200.),
        (2, "blade-wraith", 2, 150.),
        (3, "drone-orbiter", 8, 100.),
        (4, "core-tank", 3, 200.),
        (5, "blade-wraith", 3, 150.),
    ] {
        game.wave = wave;
        game.spawn(&mut scene);
        let active: Vec<_> = game
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.phase == 1)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(active.len(), count);
        for &i in &active {
            assert_eq!(game.enemies[i].spec.kind, kind);
            assert_eq!(game.enemies[i].hp, hp);
            assert_eq!(game.enemies[i].max_hp, hp);
        }
        let i = active[0];
        let kills = game.kills;
        game.hurt_enemy(&mut scene, i, 50., Vec3::Y);
        assert_eq!(game.enemies[i].hp, hp - 50.);
        assert_eq!(game.enemies[i].phase, 1);
        assert_eq!(game.kills, kills);
        assert!(game.tasks.is_empty());
        assert_eq!(game.wave, wave);
        game.hurt_enemy(&mut scene, i, 1000., Vec3::Y);
        assert_eq!(game.enemies[i].hp, 0.);
        assert_eq!(game.enemies[i].phase, 3);
        assert_eq!(game.kills, kills + 1);
        assert!(!game.tasks.is_empty());
        game.hurt_enemy(&mut scene, i, 50., Vec3::Y);
        assert_eq!(game.kills, kills + 1);
        game.spawn(&mut scene);
        assert_eq!(game.enemies[i].hp, hp);
        assert_eq!(game.enemies[i].contacts, [false; 2]);
        assert_eq!(game.enemies[i].cooldowns, [0.; 2]);
        assert_eq!(game.state()["enemies"][0]["maxHp"], hp);
    }
}

#[test]
fn blade_entry_cooldown_two_hands_and_desktop_strike_identity() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    let e = &mut game.enemies[0];
    assert!(!blade_damage_allowed(e, 0, true, false, None));
    assert!(
        !e.contacts[0],
        "stationary initial touch must not consume the hit"
    );
    assert!(blade_damage_allowed(e, 0, true, true, None));
    e.cooldowns[0] = 0.; // Staying inside cannot damage even after cooldown expires.
    assert!(!blade_damage_allowed(e, 0, true, true, None));
    assert!(blade_damage_allowed(e, 1, true, true, None));
    assert!(!blade_damage_allowed(e, 0, false, true, None));
    e.cooldowns[0] = 0.1;
    assert!(!blade_damage_allowed(e, 0, true, true, None));
    e.cooldowns[0] = 0.;
    assert!(blade_damage_allowed(e, 0, true, true, None));
    blade_damage_allowed(e, 0, false, false, None);
    e.cooldowns[0] = 0.;
    assert!(blade_damage_allowed(e, 0, true, true, Some(1)));
    e.cooldowns[0] = 0.;
    blade_damage_allowed(e, 0, false, false, Some(1));
    assert!(
        !blade_damage_allowed(e, 0, true, true, Some(1)),
        "return stroke of same desktop swing"
    );
    blade_damage_allowed(e, 0, false, false, Some(2));
    assert!(blade_damage_allowed(e, 0, true, true, Some(2)));
}

#[test]
fn returned_bolt_removes_exactly_one_hundred_tank_hp() {
    let (mut game, mut scene) = fixture();
    game.wave = 1;
    start(&mut game, &mut scene);
    let i = 8;
    let center = game.enemies[i].position + Vec3::Y * game.enemies[i].spec.height * 0.5;
    for expected in [100., 0.] {
        game.shoot(i, center, center + Vec3::Z);
        game.bolts.last_mut().unwrap().friendly = true;
        game.update_bolts(&mut scene, 0.01, game.player.head(1.7), &[None; 2]);
        assert_eq!(game.enemies[i].hp, expected);
        assert_eq!(game.kills, if expected == 0. { 1 } else { 0 });
        assert_eq!(game.tasks.is_empty(), expected > 0.);
    }
    assert_eq!(game.returned, 2);
}

#[test]
fn health_bars_follow_head_yaw_shrink_from_left_and_hide_on_death_teardown() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    let [background, fill] = game.health_bars[0];
    let head = Mat4::from_translation(Vec3::new(3., 2., 42.));
    game.sync(&mut scene, Some(head));
    let full = scene.objects()[&fill].transform.position;
    let width = scene.objects()[&fill].transform.scale.x;
    game.hurt_enemy(&mut scene, 0, 50., Vec3::Y);
    game.sync(&mut scene, Some(head));
    let t = &scene.objects()[&fill].transform;
    assert!((t.scale.x - width * 0.5).abs() < 1e-5);
    let left = Quat::from_rotation_y(t.rotation.y) * Vec3::X;
    assert!((t.position - left * t.scale.x * 0.5).distance(full - left * width * 0.5) < 1e-5);
    assert!(
        scene.objects()[&background].transform.position.y
            > game.enemies[0].position.y + game.enemies[0].spec.height
    );
    game.hurt_enemy(&mut scene, 0, 50., Vec3::Y);
    game.sync(&mut scene, Some(head));
    assert_eq!(scene.objects()[&fill].transform.position, HIDDEN);
    assert_eq!(scene.objects()[&background].transform.position, HIDDEN);
    game.teardown(&mut scene);
    assert!(game
        .health_bars
        .iter()
        .flatten()
        .all(|id| scene.objects()[id].transform.position == HIDDEN));
}

#[test]
fn actual_desktop_gladius_requires_two_separate_swings() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    let position = game.enemies[0].position;
    game.input(
        Input {
            teleport: Some([position.x, 0., position.z + 1.1]),
            ..Default::default()
        },
        &mut scene,
    )
    .unwrap();
    for e in &mut game.enemies {
        e.beam_age = 0.;
        e.shot = 100.;
    }
    game.tick(&mut scene, 0.01, None, [None; 2]);
    for expected in [50., 0.] {
        game.input(
            Input {
                slash: Some(1),
                ..Default::default()
            },
            &mut scene,
        )
        .unwrap();
        for _ in 0..42 {
            game.tick(&mut scene, 0.01, None, [None; 2]);
        }
        assert_eq!(game.enemies[0].hp, expected, "{}", game.state());
        assert_eq!(game.kills, if expected == 0. { 1 } else { 0 });
        assert_eq!(game.tasks.is_empty(), expected > 0.);
    }
}

#[test]
fn tracked_blade_pause_inside_never_rearms_and_real_time_allows_separate_return() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    game.set_xr(true);
    game.sim_dt = 0.001; // Slow simulation must not lengthen the real-time hit cooldown.
    game.enemies[0].position = Vec3::new(0., 1.04, 38.);
    game.enemies[0].beam_age = 0.;
    let head = Mat4::from_translation(Vec3::new(0., 1.7, 42.));
    let step = |game: &mut Game, scene: &mut SceneManager, x| {
        game.tick(
            scene,
            0.01,
            Some(head),
            [None, Some(Mat4::from_translation(Vec3::new(x, 1.45, 38.5)))],
        );
    };
    for x in [0.9, 0.75, 0.6, 0.45, 0.3, 0.15, 0.] {
        step(&mut game, &mut scene, x);
    }
    assert_eq!(game.enemies[0].hp, 50.);
    for _ in 0..30 {
        step(&mut game, &mut scene, 0.);
    }
    step(&mut game, &mut scene, 0.1);
    assert_eq!(
        game.enemies[0].hp, 50.,
        "stationary blade inside must remain latched"
    );
    for x in [
        0.25, 0.4, 0.55, 0.7, 0.85, 1., 0.85, 0.7, 0.55, 0.4, 0.25, 0.1,
    ] {
        step(&mut game, &mut scene, x);
    }
    assert_eq!(game.enemies[0].hp, 0.);
    assert_eq!(game.kills, 1);
    assert!(game.sim_dt < 0.002);
}

#[test]
fn xr_aim_cancel_and_tracking_loss_do_not_launch_but_trigger_release_does() {
    let head = Mat4::from_translation(Vec3::Y * 1.7);
    let ray = head * Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_4);
    for (reason, next_ray, cancel) in [
        ("B or source replacement", Some(ray), true),
        ("right tracking loss", None, false),
    ] {
        let (mut game, mut scene) = fixture();
        start(&mut game, &mut scene);
        game.set_xr(true);
        game.xr_buttons(true, false);
        game.xr_move(Some(head), Some(ray), [0.; 3], false, 0.01);
        assert!(
            game.player.aiming && game.player.target.is_some(),
            "{reason}"
        );
        let stamina = game.player.stamina;
        // Tracking loss reports an unpressed trigger; B/source changes may
        // cancel while it remains held. Neither is an intentional release.
        game.xr_buttons(cancel, false);
        game.xr_move(Some(head), next_ray, [0.; 3], cancel, 0.01);
        assert!(!game.player.aiming, "{reason}");
        assert!(game.player.target.is_none(), "{reason}");
        assert_eq!(game.player.jump_count, 0, "{reason}");
        assert_eq!(game.player.stamina, stamina, "{reason}");
        assert_eq!(game.state()["player"]["stamina"]["spent"], 0, "{reason}");
        // A recovered ray with the trigger already up must not launch later.
        game.xr_buttons(false, false);
        game.xr_move(Some(head), Some(ray), [0.; 3], false, 0.01);
        assert_eq!(game.player.jump_count, 0, "{reason}");
    }

    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    game.set_xr(true);
    game.xr_buttons(true, false);
    game.xr_move(Some(head), Some(ray), [0.; 3], false, 0.01);
    assert!(game.player.aiming && game.player.target.is_some());
    game.xr_buttons(false, false);
    game.xr_move(Some(head), Some(ray), [0.; 3], false, 0.01);
    assert_eq!(game.player.jump_count, 1);
    assert_eq!(game.player.stamina, 3.);
    assert_eq!(game.state()["player"]["stamina"]["spent"], 1);
}

fn finish_cut_staging(game: &mut Game, scene: &mut SceneManager) {
    let initial_commits = game.commits;
    for _ in 0..200 {
        let ids: Vec<_> = game.staged_cut_ids().collect();
        game.ack_cut_uploads(&ids);
        game.tick(scene, 0.001, None, [None; 2]);
        assert!(game.staged_vertices <= INSTALL_VERTICES);
        if game.commits != initial_commits {
            assert!(game
                .pieces
                .iter()
                .all(|id| game.piece_motion.contains_key(id)));
            assert!(
                game.pieces.iter().all(|id| {
                    !scene.is_cuttable(*id)
                        && scene.triangle_meshes()[id].mesh.contact_shape().is_none()
                }),
                "dead fragments never bake collision proxies"
            );
            return;
        }
        assert!(game.pieces.is_empty(), "no partial publication");
        for id in game.staged_cut_ids() {
            assert_eq!(scene.objects()[&id].transform.position, HIDDEN);
        }
    }
    panic!("cut staging did not finish");
}

#[test]
fn cut_staging_waits_for_gpu_ack_and_cleans_up_on_failure() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    game.defeat(&mut scene, 0, Vec3::Y);
    loop {
        let bytes = game.take_cut_job().unwrap();
        if bytes.is_empty() {
            break;
        }
        game.accept_cut(&mut scene, &crate::rim_cut::split(&bytes).unwrap())
            .unwrap();
    }
    assert!(
        game.staged_cut_ids().next().is_none(),
        "onmessage does no scene installation"
    );
    for _ in 0..30 {
        game.tick(&mut scene, 0.001, None, [None; 2]);
    }
    assert!(game.completed.is_empty());
    assert!(game.pieces.is_empty());
    assert_ne!(
        scene.objects()[&game.enemies[0].spec.id].transform.position,
        HIDDEN
    );
    let ids: Vec<_> = game.staged_cut_ids().collect();
    assert!(!ids.is_empty());
    game.cut_failed();
    game.tick(&mut scene, 0.001, None, [None; 2]);
    assert!(ids.iter().all(|id| !scene.object_exists(*id)));
    assert!(game.staged.is_empty());
}

#[test]
fn single_huge_primitive_prepares_only_requested_triangles() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    game.defeat(&mut scene, 0, Vec3::X);
    let task = game.tasks.front_mut().unwrap();
    let (node, primitive) = *task
        .parts
        .iter()
        .find(|&&(_, p)| {
            let p = &task.model.asset.primitives[p];
            p.mode == crate::gltf_model::PrimitiveMode::Triangles && p.vertices.len() > 6
        })
        .unwrap();
    let p = &task.model.asset.primitives[primitive];
    let triangle = p.indices[..3].to_vec();
    let full = crate::gltf_animation::deformed_vertices(
        &task.model.asset,
        &task.model.pose,
        node,
        primitive,
    )
    .unwrap();
    let subset = crate::gltf_animation::deformed_vertex_subset(
        &task.model.asset,
        &task.model.pose,
        node,
        primitive,
        triangle.iter().copied(),
    )
    .unwrap();
    for (v, &index) in subset.iter().zip(&triangle) {
        assert_eq!(v.position, full[index as usize].position);
        assert_eq!(v.normal, full[index as usize].normal);
        assert_eq!(v.tangent, full[index as usize].tangent);
    }
    let unused = (0..p.vertices.len())
        .find(|i| !triangle.contains(&(*i as u32)))
        .unwrap();
    let first_vertex = p.vertices.start;
    let asset = Arc::make_mut(&mut task.model.asset);
    asset.primitives[primitive].indices = triangle.repeat(10_001);
    // Whole-primitive deformation now fails: bounded subset must never touch this vertex.
    asset.vertices[first_vertex + unused].position = [f32::NAN; 3];
    task.parts = vec![(node, primitive)];
    let mut counts = Vec::new();
    let mut first_triangle = Vec::new();
    loop {
        let bytes = game.take_cut_job().unwrap();
        if bytes.is_empty() {
            break;
        }
        counts.push(u32::from_le_bytes(bytes[4..8].try_into().unwrap()));
        if first_triangle.is_empty() {
            first_triangle = bytes[32..140].to_vec();
        }
        for tri in bytes[32..].as_chunks::<108>().0 {
            assert_eq!(tri.as_slice(), first_triangle);
        }
    }
    assert_eq!(counts, [3500, 3500, 3001]);
}

#[test]
fn staged_reset_and_nonfinite_result_leave_no_partial_pieces() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    for fail in [false, true] {
        game.defeat(&mut scene, 0, Vec3::Y);
        let mut results = Vec::new();
        loop {
            let bytes = game.take_cut_job().unwrap();
            if bytes.is_empty() {
                break;
            }
            results.push(crate::rim_cut::split(&bytes).unwrap());
        }
        for result in &results {
            game.accept_cut(&mut scene, result).unwrap();
        }
        game.tick(&mut scene, 0.001, None, [None; 2]);
        let ids: Vec<_> = game.staged_cut_ids().collect();
        assert!(!ids.is_empty());
        assert!(game.pieces.is_empty());
        assert!(ids
            .iter()
            .all(|id| scene.projectile_velocity(*id).is_none()));
        if fail {
            let result = game.completed.front_mut().unwrap();
            // Poison a remaining vertex, after the first bounded chunk was staged.
            let start = 12
                + (if result.side == 1 {
                    result.counts[0]
                } else {
                    0
                } + result.offset)
                    * 36;
            result.bytes[start..start + 4].copy_from_slice(&f32::NAN.to_le_bytes());
            game.tick(&mut scene, 0.001, None, [None; 2]);
            assert!(game.staged.is_empty());
            assert_eq!(game.cut_errors, 1);
        } else {
            game.spawn(&mut scene);
        }
        assert!(ids.iter().all(|id| !scene.object_exists(*id)));
        assert!(game.pieces.is_empty());
    }
}

#[test]
fn published_halves_separate_before_dissolve_then_evaporate() {
    let (mut game, mut scene) = fixture_with_playing_pool(true);
    start(&mut game, &mut scene);
    game.defeat(&mut scene, 0, Vec3::X);
    loop {
        let job = game.take_cut_job().unwrap();
        if job.is_empty() {
            break;
        }
        game.accept_cut(&mut scene, &crate::rim_cut::split(&job).unwrap())
            .unwrap();
    }
    finish_cut_staging(&mut game, &mut scene);
    let ids = game.pieces.clone();
    assert!(ids.iter().any(|id| game.piece_motion[id].direction.x > 0.));
    assert!(ids.iter().any(|id| game.piece_motion[id].direction.x < 0.));
    for _ in 0..20 {
        game.tick(&mut scene, 0.05, None, [None; 2]);
        scene.update(0.05);
    }
    for id in &ids {
        let object = &scene.objects()[id];
        let direction = game.piece_motion[id].direction.x;
        assert!(
            object.transform.position.x * direction > 0.5,
            "half must visibly separate despite being a fading piece"
        );
        assert_eq!(
            object.visual_fade, 1.,
            "full silhouette must survive the first second"
        );
        assert!(object.visual_dissolve);
    }
    for _ in 0..20 {
        game.tick(&mut scene, 0.05, None, [None; 2]);
        scene.update(0.05);
    }
    for id in &ids {
        let object = &scene.objects()[id];
        assert!((object.transform.position.x.abs() - 0.65).abs() < 1e-5);
        assert!(object.visual_fade > 0. && object.visual_fade < 1.);
    }
    for _ in 0..21 {
        game.tick(&mut scene, 0.05, None, [None; 2]);
        scene.update(0.05);
    }
    assert!(ids.iter().all(|id| !scene.object_exists(*id)));
    // Scene removes dissolved meshes after gameplay; prune its IDs next frame.
    game.tick(&mut scene, 0.05, None, [None; 2]);
    assert!(game.pieces.is_empty());
}

#[test]
fn lost_worker_and_upload_ack_expire_only_the_stalled_enemy_in_real_time() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    game.defeat(&mut scene, 0, Vec3::X);
    let stale = crate::rim_cut::split(&game.take_cut_job().unwrap()).unwrap();
    game.defeat(&mut scene, 1, Vec3::Y);
    let other_serial = game.enemies[1].serial;
    // A stale transaction must not cancel its neighbour or require aim-time progress.
    game.enemies[0].cut_idle = 10.01;
    game.enemies[0].age = 3.21;
    game.tick(&mut scene, 0.01, None, [None; 2]);
    assert_eq!(game.enemies[0].phase, 0);
    assert_eq!(game.cut_errors, 1);
    assert!(game
        .tasks
        .iter()
        .any(|t| t.owner == 1 && t.serial == other_serial));
    game.accept_cut(&mut scene, &stale).unwrap();
    assert!(
        game.completed.is_empty(),
        "expired reply cannot resurrect a death"
    );
    while !game.tasks.is_empty() {
        let result = crate::rim_cut::split(&game.take_cut_job().unwrap()).unwrap();
        game.accept_cut(&mut scene, &result).unwrap();
    }
    for _ in 0..100 {
        game.tick(&mut scene, 0.001, None, [None; 2]);
        if game.completed.is_empty() {
            break;
        }
    }
    let hidden: Vec<_> = game.staged_cut_ids().collect();
    assert!(!hidden.is_empty());
    assert!(game.pieces.is_empty());
    game.enemies[1].cut_idle = 10.01;
    game.enemies[1].age = 3.21;
    game.tick(&mut scene, 0.01, None, [None; 2]);
    assert_eq!(game.enemies[1].phase, 0);
    assert_eq!(game.cut_errors, 2);
    assert!(hidden.iter().all(|id| !scene.object_exists(*id)));
    assert!(game.staged.is_empty());
}

#[test]
fn every_kill_finishes_real_slicing_then_advances_the_wave() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    for i in 0..6 {
        game.defeat(&mut scene, i, Vec3::X);
    }
    while !game.tasks.is_empty() {
        let result = crate::rim_cut::split(&game.take_cut_job().unwrap()).unwrap();
        game.accept_cut(&mut scene, &result).unwrap();
    }
    for _ in 0..200 {
        let ids: Vec<_> = game.staged_cut_ids().collect();
        game.ack_cut_uploads(&ids);
        game.tick(&mut scene, 0.001, None, [None; 2]);
        if game.commits == 6 {
            break;
        }
    }
    assert_eq!(game.commits, 6);
    assert!(!game.pieces.is_empty());
    assert_eq!(game.wave, 0);
    for e in game.enemies.iter_mut().filter(|e| e.phase == 3) {
        e.age = 3.21;
    }
    game.tick(&mut scene, 0.01, None, [None; 2]);
    assert_eq!(game.kills, 6);
    assert_eq!(game.wave, 1);
    assert_eq!(game.enemies.iter().filter(|e| e.phase == 1).count(), 2);
    assert!(game
        .enemies
        .iter()
        .filter(|e| e.phase == 1)
        .all(|e| e.spec.kind == "core-tank"));
}

#[test]
fn failed_cuts_cannot_block_wave_during_continuous_bullet_time() {
    let (mut game, mut scene) = fixture();
    start(&mut game, &mut scene);
    for i in 0..6 {
        game.defeat(&mut scene, i, Vec3::Y);
    }
    game.set_xr(true);
    game.sim_dt = 0.0001;
    let head = Mat4::from_translation(Vec3::new(0., 1.7, 42.));
    for _ in 0..202 {
        game.tick(&mut scene, 0.05, Some(head), [None; 2]);
    }
    assert_eq!(
        game.wave, 1,
        "real-time inactivity retires deaths even when sim time barely advances"
    );
    assert_eq!(game.cut_errors, 6);
    assert_eq!(game.enemies.iter().filter(|e| e.phase == 1).count(), 2);
}

#[test]
fn same_physical_vr_swings_hit_at_72_90_and_120_hz() {
    for hz in [72., 90., 120.] {
        let (mut game, mut scene) = fixture();
        start(&mut game, &mut scene);
        game.set_xr(true);
        game.sim_dt = 0.; // Hold the target still; hand motion uses real tracking dt.
        game.enemies[0].position = Vec3::new(0., 1.04, 40.95);
        let head = Some(Mat4::from_translation(Vec3::new(0., 1.7, 42.)));
        let dt = 1. / hz;
        let sweep = |game: &mut Game, scene: &mut SceneManager, start: f32, sign: f32| {
            for frame in 0..=(0.8_f32 * hz).ceil() as usize {
                let x = start + sign * 1.5 * (frame as f32 * dt).min(0.8);
                game.tick(
                    scene,
                    dt,
                    head,
                    [None, Some(Mat4::from_translation(Vec3::new(x, 1.4, 41.6)))],
                );
            }
        };
        sweep(&mut game, &mut scene, 0.6, -1.);
        assert_eq!(
            game.enemies[0].hp, 50.,
            "same 1.5m/s swing must hit at {hz} Hz"
        );
        for _ in 0..(hz * 0.35) as usize {
            game.tick(
                &mut scene,
                dt,
                head,
                [
                    None,
                    Some(Mat4::from_translation(Vec3::new(-0.6, 1.4, 41.6))),
                ],
            );
        }
        sweep(&mut game, &mut scene, -0.6, 1.);
        assert_eq!(game.enemies[0].hp, 0., "return swing kills at {hz} Hz");
        assert_eq!(game.enemies[0].phase, 3);
        assert!(
            !game.tasks.is_empty(),
            "HP zero queues actual worker geometry at {hz} Hz"
        );
    }
}

#[test]
fn blob_shadows_follow_living_npcs_without_invalidating_shadow_maps() {
    let (mut game, mut scene) = fixture_with_shadow_pool(false, true);
    assert!(game
        .enemies
        .iter()
        .all(|e| !scene.objects()[&e.spec.id].render_options.casts_shadow));
    // Match the authored encounter's non-casters; retain one unrelated caster in
    // each layer to catch accidental broad invalidation during NPC movement.
    let ids: Vec<_> = scene.objects().keys().copied().collect();
    for id in ids {
        scene.set_render_options(
            id,
            crate::scene::RenderOptions {
                casts_shadow: false,
                ..Default::default()
            },
        );
    }
    let city = scene.add_cube(Vec3::new(5., 0., 5.));
    scene.set_render_options(
        city,
        crate::scene::RenderOptions {
            is_static: true,
            ..Default::default()
        },
    );
    let dynamic = scene.add_cube(Vec3::new(-5., 0., 5.));
    start(&mut game, &mut scene);
    let revisions = (scene.shadow_revision(true), scene.shadow_revision(false));
    game.enemies[0].position = Vec3::new(3., 2., -7.);
    game.sync(&mut scene, None);
    let id = game.blob_shadows[0];
    let t = &scene.objects()[&id].transform;
    assert_eq!(t.position, Vec3::new(3., 0.136, -7.));
    assert!(t.scale.x > 0. && t.scale.x <= 0.9);
    assert_eq!(
        revisions,
        (scene.shadow_revision(true), scene.shadow_revision(false))
    );
    game.hurt_enemy(&mut scene, 0, 50., Vec3::Y);
    game.sync(&mut scene, None);
    assert_ne!(scene.objects()[&id].transform.position, HIDDEN);
    game.hurt_enemy(&mut scene, 0, 50., Vec3::Y);
    game.sync(&mut scene, None);
    assert_eq!(scene.objects()[&id].transform.position, HIDDEN);
    game.teardown(&mut scene);
    assert!(game
        .blob_shadows
        .iter()
        .all(|id| scene.objects()[id].transform.position == HIDDEN));
    assert!(game
        .enemies
        .iter()
        .all(|e| scene.objects()[&e.spec.id].render_options.casts_shadow));
    assert!(game
        .blob_shadows
        .iter()
        .all(|id| scene.objects()[id].render_options.casts_shadow));
    // The generic dynamic shadow path still invalidates independently.
    let revisions = (scene.shadow_revision(true), scene.shadow_revision(false));
    scene.set_object_position(dynamic, Vec3::new(-4., 0., 5.));
    assert_eq!(scene.shadow_revision(true), revisions.0);
    assert!(scene.shadow_revision(false) > revisions.1);
}

#[test]
fn stress_count_is_bounded_preserves_combat_reset_and_clears_on_teardown() {
    let (mut game, mut scene) = fixture();
    assert_eq!(game.state()["stressModels"], 0);
    game.input(
        Input {
            stress_count: Some(128),
            ..Default::default()
        },
        &mut scene,
    )
    .unwrap();
    assert_eq!(game.stress_count(), 128);
    assert!(game
        .input(
            Input {
                stress_count: Some(MAX_STRESS_MODELS + 1),
                reset: true,
                ..Default::default()
            },
            &mut scene
        )
        .is_err());
    assert_eq!(game.stress_count(), 128);
    start(&mut game, &mut scene);
    game.input(
        Input {
            reset: true,
            ..Default::default()
        },
        &mut scene,
    )
    .unwrap();
    assert_eq!(game.stress_count(), 128);
    game.teardown(&mut scene);
    assert_eq!(game.stress_count(), 0);
}
