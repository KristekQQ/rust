use crate::{
    content::Content,
    game::{Game, Recipe},
};
use glam::{Mat4, Vec3};
use kristek_rim_native::xr::FrameInput;
use webgpu_wasm::scene::SceneManager;
fn load() -> (SceneManager, Content) {
    let mut scene = SceneManager::default();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let content = Content::load(&mut scene, &|name| {
        std::fs::read(root.join(name)).map_err(|e| e.to_string())
    })
    .unwrap();
    (scene, content)
}
fn frame() -> FrameInput {
    FrameInput {
        dt: 1. / 120.,
        head: Some(Mat4::from_translation(Vec3::new(0., 1.55, 0.))),
        ..Default::default()
    }
}
#[test]
fn complete_neon_content_loads_native_models_fonts_and_variant_catalog() {
    let (scene, content) = load();
    assert_eq!(content.manifest["revision"], "cynatar-arena-51");
    for name in ["arenaCabinet", "stealthCat"] {
        let id = content.refs[name].as_u64().unwrap() as u32;
        assert!(scene.models().contains_key(&id), "missing model for {name}");
    }
    let birds = content.refs["birdFlights"].as_array().unwrap();
    assert!(!birds.is_empty());
    for bird in birds {
        assert!(scene
            .models()
            .contains_key(&(bird.as_u64().unwrap() as u32)));
    }
    assert_eq!(
        content.manifest["runtime"]["catalog"]
            .as_array()
            .unwrap()
            .len(),
        10
    );
    for fruit in content.manifest["runtime"]["catalog"].as_array().unwrap() {
        assert!(scene.mesh_exists(fruit["asset"].as_u64().unwrap() as u32));
        assert_eq!(fruit["variants"].as_array().unwrap().len(), 15);
    }
    assert_eq!(content.indicator_bindings.len(), 60);
    assert_eq!(
        content.manifest["runtime"]["minutePlan"]["cues"]
            .as_array()
            .unwrap()
            .len(),
        51
    );
    for key in [
        "sword",
        "leftSword",
        "infoPanel",
        "timerPanel",
        "scorePanel",
        "restartButton",
    ] {
        assert!(scene.object_exists(content.refs[key].as_u64().unwrap() as u32));
    }
}
#[test]
fn native_minute_starts_authored_wave_and_pauses_on_tracking_loss() {
    let (mut scene, content) = load();
    let mut game = Game::new(&mut scene, &content, None).unwrap();
    for icon in content.refs["galleryIcons"].as_array().unwrap() {
        let object = &scene.objects()[&(icon.as_u64().unwrap() as u32)];
        assert_eq!(object.opacity, 0.06);
        assert_eq!(object.color_cutout, 0.);
    }
    game.tick(&mut scene, frame()).unwrap();
    assert_eq!(game.cue_index, 1);
    assert_eq!(game.shots, 6);
    assert_eq!(game.flying.len(), 6);
    assert_eq!(game.score.targets.len(), 6);
    let remaining = game.round.unwrap().remaining;
    game.tick(
        &mut scene,
        FrameInput {
            dt: 2.,
            head: None,
            ..frame()
        },
    )
    .unwrap();
    assert_eq!(game.round.unwrap().remaining, remaining);
    assert!(game.player.head().is_none());
    game.tick(&mut scene, frame()).unwrap();
    assert!(game.round.unwrap().remaining < remaining);
}
#[test]
fn real_tracked_native_katana_cuts_fruit_and_banks_its_scored_stock() {
    let (mut scene, content) = load();
    let mut game = Game::new(&mut scene, &content, None).unwrap();
    game.start(&mut scene).unwrap();
    game.cue_index = usize::MAX;
    game.spawn(
        &mut scene,
        &[Recipe {
            fruit: "watermelon".into(),
            scale: 0.126,
            position: [0.45, 1.35, -1.1],
            velocity: [0.; 3],
            gravity: 0.,
            lifetime: 5.,
        }],
        false,
    )
    .unwrap();
    let before_cut: std::collections::BTreeSet<_> = scene.objects().keys().copied().collect();
    for step in 0..25 {
        let y = 1.0 + step as f32 * 0.025;
        let mut f = frame();
        f.hands[1] = Some(Mat4::from_translation(Vec3::new(0.45, y, -0.45)));
        scene.update(f.dt);
        game.tick(&mut scene, f).unwrap();
    }
    assert_eq!(game.cuts, 1, "{}", game.state());
    assert_eq!(game.score.score, 10);
    assert_eq!(game.score.chains[0].as_ref().unwrap().count, 1);
    assert!(game.state()["fragments"].as_u64().unwrap() >= 2);
    let fragments: Vec<_> = scene
        .objects()
        .iter()
        .filter_map(|(&id, object)| {
            (!before_cut.contains(&id) && (object.emission - 1.1).abs() < 1e-6).then_some(id)
        })
        .collect();
    for _ in 0..100 {
        scene.update(frame().dt);
        game.tick(&mut scene, frame()).unwrap();
    }
    assert!(fragments.len() >= 2);
    for id in fragments {
        let piece = scene
            .objects()
            .get(&id)
            .expect("halves survive the former .75s dissolve");
        assert_eq!(piece.visual_fade, 1.0, "original web halves do not fade");
        assert!(
            !piece.visual_dissolve,
            "original web halves do not dissolve"
        );
    }
    let mut f = frame();
    f.triggers[0] = true;
    game.tick(&mut scene, f).unwrap();
    assert!(game.score.chains[0].is_none());
    assert_eq!(
        game.score.score, 10,
        "bank does not double-award earned points"
    );
}
#[test]
fn native_round_initials_confirm_and_restart_preserves_saved_scores() {
    let (mut scene, content) = load();
    let dir = std::env::temp_dir().join(format!("neon-native-{}", std::process::id()));
    let path = dir.join("scores.json");
    let _ = std::fs::remove_file(&path);
    let mut game = Game::new(&mut scene, &content, Some(path.clone())).unwrap();
    game.start(&mut scene).unwrap();
    game.score.score = 1234;
    game.round.as_mut().unwrap().remaining = 0.001;
    game.tick(&mut scene, frame()).unwrap();
    assert!(game.round.unwrap().finished());
    assert_eq!(game.state()["scoreEntryPending"], true);
    assert!(!path.exists());
    let pointer = content.refs["pointers"][1].as_u64().unwrap() as u32;
    let origin = Vec3::new(0.22, 1.18, -0.31);
    let goal = Vec3::new(-0.45, 1.55, -1.18);
    let mut aim = frame();
    aim.rays[1] = Some(glam::camera::rh::view::look_at_mat4(origin, goal, Vec3::Y).inverse());
    game.tick(&mut scene, aim).unwrap();
    let beam = scene.objects()[&pointer].transform.model();
    assert!(beam.transform_point3(Vec3::Z).distance(origin) < 0.001);
    assert!(beam.transform_point3(Vec3::NEG_Z).distance(goal) < 0.001);
    game.tick(&mut scene, frame()).unwrap();
    assert!(scene.objects()[&pointer].transform.position.y < -100.);
    let mut choose = |index: usize| {
        let mut select = frame();
        select.rays[1] = Some(Mat4::from_translation(Vec3::new(
            (index % 7) as f32 * 0.15 - 0.45,
            1.55 - (index / 7) as f32 * 0.112,
            0.,
        )));
        select.triggers[1] = true;
        game.tick(&mut scene, select).unwrap();
        game.tick(&mut scene, select).unwrap(); // Holding cannot repeat a letter.
        game.tick(&mut scene, frame()).unwrap();
    };
    for index in [27, 0, 1, 2, 3, 26, 25, 27] {
        choose(index);
    }
    assert_eq!(game.state()["scoreEntryPending"], false);
    assert!(scene.objects()[&pointer].transform.position.y < -100.);
    let scores: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(scores, serde_json::json!([{"score":1234,"initials":"ABZ"}]));
    let button = game.ref_id("restartButton").unwrap();
    assert!(
        (scene.objects()[&button].transform.position - Vec3::new(100., -10000., 100.)).length()
            < 0.001
    );
    for _ in 0..610 {
        game.tick(&mut scene, frame()).unwrap();
    }
    assert!(game.round.unwrap().finished(), "wait for explicit restart");
    game.start(&mut scene).unwrap();
    assert_eq!(game.score.score, 0);
    assert_eq!(game.state()["top5"], serde_json::json!([1234]));
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn blade_motion_cannot_restart_round_but_controller_can() {
    let (mut scene, content) = load();
    let mut game = Game::new(&mut scene, &content, None).unwrap();
    let button = game.ref_id("restartButton").unwrap();
    scene.set_object_rotation(button, Vec3::new(0., 1.5, 0.));
    game.start(&mut scene).unwrap();
    assert_eq!(scene.objects()[&button].transform.rotation, Vec3::ZERO);
    game.score.score = 987;
    let mut f = frame();
    f.hands[1] = Some(Mat4::from_translation(Vec3::new(0., 0.2, -0.8)));
    for _ in 0..12 {
        game.tick(&mut scene, f).unwrap();
    }
    assert_eq!(
        game.score.score, 987,
        "held contact must leave before reactivation"
    );
    let remaining = game.round.unwrap().remaining;
    for step in 1..=8 {
        f.hands[1] = Some(Mat4::from_translation(Vec3::new(
            step as f32 * 0.0625,
            0.2,
            -0.8,
        )));
        game.tick(&mut scene, f).unwrap();
    }
    for step in (0..8).rev() {
        f.hands[1] = Some(Mat4::from_translation(Vec3::new(
            step as f32 * 0.0625,
            0.2,
            -0.8,
        )));
        game.tick(&mut scene, f).unwrap();
    }
    assert_eq!(
        game.score.score, 987,
        "blade movement cannot restart the round"
    );
    assert!(game.round.unwrap().remaining < remaining);
    f.primary = true;
    game.tick(&mut scene, f).unwrap();
    assert_eq!(game.score.score, 0);
    assert!(game.round.unwrap().remaining > remaining);
}

#[test]
fn full_native_minute_plays_all_authored_cues_and_149_authored_targets() {
    let (mut scene, content) = load();
    let mut game = Game::new(&mut scene, &content, None).unwrap();
    for _ in 0..7205 {
        let f = frame();
        scene.update(f.dt);
        game.tick(&mut scene, f).unwrap();
        assert!(game.flying.len() <= 10);
    }
    assert_eq!(game.cue_index, 51);
    assert_eq!(game.shots, 149);
    assert!(game.round.unwrap().finished());
}

#[test]
fn cat_xp_drink_consumes_upright_at_face_without_trigger_then_boosts_future_points() {
    cat_xp_drink_gameplay_with_pickup_orientation(glam::Quat::IDENTITY);
}

#[test]
fn cat_xp_drink_quarter_turned_pickup_keeps_sword_mapping_then_tip_boosts_score() {
    cat_xp_drink_gameplay_with_pickup_orientation(glam::Quat::from_rotation_z(
        std::f32::consts::FRAC_PI_2,
    ));
}

fn cat_xp_drink_gameplay_with_pickup_orientation(pickup_rotation: glam::Quat) {
    let (mut scene, content) = load();
    let mut game = Game::new(&mut scene, &content, None).unwrap();
    game.start(&mut scene).unwrap();
    game.cue_index = usize::MAX;
    let cat = game.ref_id("stealthCat").unwrap();
    let drink = game
        .ref_id("xpDrink")
        .expect("current native content exports XP drink");
    assert!(scene.trigger_sprite(cat));
    scene.update(3.);
    let mut input = frame();
    input.dt = 1. / 90.;
    game.tick(&mut scene, input).unwrap();
    assert_eq!(
        game.state()["xpDrinkActive"],
        false,
        "new round and ordinary approach start empty"
    );
    assert_eq!(game.score.cat_wild_remaining(), 0.);
    assert!(scene
        .pet_drink_carrier(input.head.map(|head| head.w_axis.truncate()))
        .is_none());
    scene.update(4.);
    game.tick(&mut scene, input).unwrap();
    let origin = scene.objects()[&cat].transform.position;
    let yaw = scene.objects()[&cat].transform.rotation.y;
    let heading = Vec3::new(yaw.sin(), 0., yaw.cos());
    let pitch = 65_f32.to_radians();
    let pet_head = origin + (Vec3::Y * pitch.cos() - heading * pitch.sin()) * 0.55;
    input.hands[0] = Some(Mat4::from_translation(pet_head));
    let viewer = input.head.unwrap().w_axis.truncate();
    for _ in 0..90 {
        scene.update(input.dt);
        game.tick(&mut scene, input).unwrap();
    }
    assert_eq!(
        game.state()["xpDrinkActive"],
        false,
        "one live contact second cannot earn drink"
    );
    for _ in 0..65 {
        scene.update(input.dt);
        game.tick(&mut scene, input).unwrap();
    }
    assert_eq!(
        game.state()["xpDrinkActive"],
        false,
        "earning sends cat to fetch; no instant can"
    );
    input.hands = [None; 2];
    let mut carried_on_approach = false;
    let mut delivered = false;
    for _ in 0..1500 {
        scene.update(input.dt);
        game.tick(&mut scene, input).unwrap();
        if let Some(carrier) = scene.pet_drink_carrier(Some(viewer)) {
            let lip = scene.objects()[&drink]
                .transform
                .model()
                .transform_point3(Vec3::Y * 0.219);
            assert!(
                lip.distance(carrier.lip) < 0.001,
                "same authoritative can follows fetch return"
            );
            if !carrier.available {
                carried_on_approach = true;
            } else {
                delivered = true;
                break;
            }
        }
    }
    assert!(
        carried_on_approach,
        "cat picks can up at cabinet and carries it on approach"
    );
    assert!(
        delivered,
        "earned fetch returns promptly without ambient wait"
    );
    assert!(scene.objects()[&cat].transform.position.distance(origin) < 0.001);
    assert!(
        game.flying.is_empty(),
        "drink never spawns as cuttable fruit"
    );
    let before_pickup = scene.objects()[&drink].transform.model();
    let pickup_center = before_pickup.transform_point3(Vec3::Y * 0.1);
    input.hands[1] = Some(Mat4::from_rotation_translation(
        pickup_rotation,
        pickup_center,
    ));
    game.tick(&mut scene, input).unwrap();
    assert_eq!(
        game.state()["drinkingHands"],
        serde_json::json!([false, true])
    );
    let held = scene.objects()[&drink].transform.model();
    assert!(
        held.transform_point3(Vec3::Y * 0.1).distance(pickup_center) < 0.001,
        "can center coincides with grip origin"
    );
    let axis = held.transform_vector3(Vec3::Y);
    let blade = input.hands[1].unwrap().transform_vector3(Vec3::NEG_Z);
    assert!(
        axis.distance(blade) < 0.001,
        "can top follows sword direction even with quarter-turned grip"
    );
    assert!(
        scene.objects()[&game.ref_id("sword").unwrap()]
            .transform
            .position
            .y
            < -9999.
    );
    assert_eq!(
        game.score.cat_wild_remaining(),
        0.,
        "pickup alone awards no boost"
    );
    input.hands[0] = Some(Mat4::from_translation(pet_head));
    for _ in 0..135 {
        scene.update(input.dt);
        game.tick(&mut scene, input).unwrap();
    }
    assert!(
        scene.objects()[&cat].transform.position.distance(origin) < 0.001,
        "pickup starts a fresh interaction instead of the old farewell timer"
    );
    assert!(
        scene.pet_sprite(cat, pet_head, viewer),
        "cat remains pettable before the second 1.6 contact seconds complete"
    );
    input.hands[0] = None;
    let mouth = input
        .head
        .unwrap()
        .transform_point3(Vec3::new(0., -0.12, -0.16));
    // The fixed model-to-grip rotation makes can +Y follow controller -Z.
    let can_to_grip = glam::Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    input.hands[1] = Some(Mat4::from_rotation_translation(
        can_to_grip,
        mouth - Vec3::new(0., 0.416, 0.015),
    ));
    for _ in 0..65 {
        scene.update(input.dt);
        game.tick(&mut scene, input).unwrap();
    }
    assert_eq!(
        game.score.cat_wild_remaining(),
        0.,
        "can below the mouth cannot activate boost"
    );
    let can_rotation = pickup_rotation;
    assert_eq!(input.triggers, [false; 2]);
    let grip_position = mouth - can_rotation * Vec3::new(0., 0.116, 0.015);
    input.hands[1] = Some(Mat4::from_rotation_translation(
        can_rotation * can_to_grip,
        grip_position,
    ));
    for step in 0..75 {
        scene.update(input.dt);
        game.tick(&mut scene, input).unwrap();
        if step < 15 {
            let expected = Mat4::from_rotation_translation(
                can_rotation,
                grip_position - can_rotation * Vec3::Y * 0.1,
            );
            assert!(
                scene.objects()[&drink]
                    .transform
                    .model()
                    .abs_diff_eq(expected, 0.001),
                "sip must track wrist exactly without automatic tilt animation"
            );
        }
    }
    assert_eq!(game.state()["xpDrinkActive"], false);
    assert_eq!(
        game.state()["drinkingHands"],
        serde_json::json!([false, false])
    );
    assert!(scene.objects()[&drink].transform.position.y < -9999.);
    assert!((19.0..=20.0).contains(&game.score.cat_wild_remaining()));
    let before = game
        .score
        .chains
        .clone()
        .map(|c| c.map_or(0, |chain| chain.total_points()));
    game.spawn(
        &mut scene,
        &[Recipe {
            fruit: "watermelon".into(),
            scale: 0.126,
            position: [0.45, 1.35, -1.1],
            velocity: [0.; 3],
            gravity: 0.,
            lifetime: 5.,
        }],
        false,
    )
    .unwrap();
    for step in 0..25 {
        input.hands[1] = Some(Mat4::from_translation(Vec3::new(
            0.45,
            1.0 + step as f32 * 0.025,
            -0.45,
        )));
        scene.update(input.dt);
        game.tick(&mut scene, input).unwrap();
    }
    let after = game
        .score
        .chains
        .clone()
        .map(|c| c.map_or(0, |chain| chain.total_points()));
    assert_eq!([after[0] - before[0], after[1] - before[1]], [260, 260]);
    let bird_before = game.score.score;
    game.score.award_round_bonus(game.round, 7);
    assert_eq!(
        game.score.score - bird_before,
        14,
        "boosted birds pay x2 and each fruit stock also pays x2"
    );
    input.dt = 20.;
    game.tick(&mut scene, input).unwrap();
    assert_eq!(game.score.cat_wild_remaining(), 0.);
    game.start(&mut scene).unwrap();
    assert_eq!(game.state()["xpDrinkActive"], false);
    assert_eq!(
        game.state()["drinkingHands"],
        serde_json::json!([false, false])
    );
}

#[test]
fn native_grip_pet_contact_has_bounded_cuddle_and_restart_preserves_visit() {
    let (mut scene, mut content) = load();
    // With no optional drink, the normal cuddle runs to completion without a fetch.
    content.refs.as_object_mut().unwrap().remove("xpDrink");
    let mut game = Game::new(&mut scene, &content, None).unwrap();
    game.start(&mut scene).unwrap();
    let cat = game.ref_id("stealthCat").unwrap();
    assert!(scene.trigger_sprite(cat));
    scene.update(7.1);
    let object = &scene.objects()[&cat];
    let origin = object.transform.position;
    let waiting_frame = object.sprite.unwrap().frame;
    let viewer = Vec3::new(0., 1.55, 0.);
    let yaw = object.transform.rotation.y;
    let heading = Vec3::new(yaw.sin(), 0., yaw.cos());
    let pitch = 65_f32.to_radians();
    let contact = origin + (Vec3::Y * pitch.cos() - heading * pitch.sin()) * 0.55;
    let mut input = frame();
    input.dt = 0.1;
    input.head = Some(Mat4::from_translation(viewer));
    input.hands[0] = Some(Mat4::from_translation(contact));
    for step in 0..90 {
        if step == 50 {
            let before = scene.objects()[&cat].transform;
            let visual = scene.objects()[&cat].sprite;
            game.start(&mut scene).unwrap();
            assert_eq!(scene.objects()[&cat].transform.position, before.position);
            assert_eq!(
                scene.objects()[&cat].sprite,
                visual,
                "restart must not change cat presentation"
            );
        }
        game.tick(&mut scene, input).unwrap();
        scene.update(input.dt);
        if step == 0 {
            assert_ne!(
                scene.objects()[&cat].sprite.unwrap().frame,
                waiting_frame,
                "tracked grip contact starts the cuddle"
            );
        }
    }
    let object = &scene.objects()[&cat];
    assert!(
        object.sprite.unwrap().visible,
        "cat stays visible during bounded cuddle"
    );
    assert!(
        (object.transform.position - origin).length() < 0.001,
        "petting must not rewind movement"
    );
    assert!(
        object.sprite.unwrap().frame >= 8,
        "contact selects cuddle frames"
    );
    // Contact remains present: restarting the round at five seconds must not
    // restart the cuddle clock. At 10.5 seconds the farewell still holds position.
    for _ in 0..15 {
        game.tick(&mut scene, input).unwrap();
        scene.update(input.dt);
    }
    assert!(
        (scene.objects()[&cat].transform.position - origin).length() < 0.001,
        "farewell must hold the endpoint for a full second"
    );
    for _ in 0..10 {
        game.tick(&mut scene, input).unwrap();
        scene.update(input.dt);
    }
    assert!(
        (scene.objects()[&cat].transform.position - origin).length() > 0.001,
        "ten-second cuddle plus farewell must end despite continued contact and round restart"
    );
    input.hands = [None; 2];
    for _ in 0..55 {
        game.tick(&mut scene, input).unwrap();
        scene.update(input.dt);
    }
    assert!(
        !scene.objects()[&cat].sprite.unwrap().visible,
        "bounded visit returns home and disappears"
    );
}

#[test]
fn a_flock_can_hit_a_stationary_players_visor_during_a_round() {
    let (mut scene, content) = load();
    let mut game = Game::new(&mut scene, &content, None).unwrap();
    let input = frame();
    let mut hits = 0;
    let mut first_hit = None;
    for step in 0..7200 {
        let before = scene.visor_splat_remaining();
        scene.update(input.dt);
        game.tick(&mut scene, input).unwrap();
        if scene.visor_splat_remaining() > before {
            hits += 1;
            first_hit.get_or_insert(step as f32 * input.dt);
        }
    }
    assert!(
        first_hit.is_some_and(|t| t < 17.),
        "first flyover missed: {first_hit:?}"
    );
    assert!(
        (1..=3).contains(&hits),
        "actual authored round produced {hits} visor hits"
    );
}

#[test]
fn a_flock_can_hit_keeps_flight_height_when_player_crouches() {
    let (mut scene, content) = load();
    let mut game = Game::new(&mut scene, &content, None).unwrap();
    let mut input = frame();
    let standing_y = 1.85;
    input.head = Some(Mat4::from_translation(Vec3::Y * standing_y));
    game.tick(&mut scene, input).unwrap();
    input.head = Some(Mat4::from_translation(Vec3::Y * 0.95));
    let mut overhead = std::collections::BTreeSet::new();
    let mut left_wing = false;
    let mut right_wing = false;
    for _ in 0..2100 {
        game.tick(&mut scene, input).unwrap();
        scene.update(input.dt);
        for bird in scene.objects().values().filter(|o| {
            o.sprite
                .is_some_and(|s| s.visible && (2..20).contains(&s.key))
                && o.transform.position.z.abs() < 0.35
        }) {
            assert!(bird.transform.position.y > standing_y + 0.18);
            let x = bird.transform.position.x;
            if x.abs() < 0.18 {
                overhead.insert(bird.sprite.unwrap().key);
            }
            assert!(x.abs() < 1.9, "wing outside narrowed formation: {x}");
            left_wing |= x < -1.5;
            right_wing |= x > 1.5;
        }
    }
    assert_eq!(overhead.len(), 3, "one overhead bird per row");
    assert!(
        left_wing && right_wing,
        "both wings cross beside the player"
    );
}

#[test]
fn music_bars_follow_round_time_and_restart_with_the_opening_wave() {
    let (mut scene, content) = load();
    let mut game = Game::new(&mut scene, &content, None).unwrap();
    let mut input = frame();
    let mut latest = 0;
    let mut bars = Vec::new();
    for step in 0..2100 {
        input.dt = if step % 2 == 0 { 1. / 24. } else { 1. / 48. };
        scene.update(input.dt);
        game.tick(&mut scene, input).unwrap();
        let audio = game.audio.snapshot();
        for event in audio
            .events
            .iter()
            .filter(|e| e.id > latest && e.clip.starts_with("neon_bar_"))
        {
            bars.push((event.at, event.clip.clone()));
        }
        latest = audio.latest;
        if game.round.unwrap().finished() {
            break;
        }
    }
    assert_eq!(bars.len(), 30);
    for (i, (at, clip)) in bars.iter().enumerate() {
        assert!((at - 2. * i as f64).abs() < 0.06);
        let section = ["neon_bar_intro", "neon_bar_drive", "neon_bar_final"][i / 10];
        assert_eq!(
            clip,
            if [9, 19, 28].contains(&i) {
                "neon_bar_break"
            } else {
                section
            }
        );
    }
    game.start(&mut scene).unwrap();
    game.tick(&mut scene, input).unwrap();
    assert_eq!(game.cue_index, 1);
    assert_eq!(
        game.audio
            .snapshot()
            .events
            .iter()
            .filter(|e| e.clip == "neon_bar_intro")
            .count(),
        1
    );
}
