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
    assert_eq!(scene.models().len(), 1);
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
        24
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
    game.cue_index = 24;
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
fn native_round_finish_persists_top5_and_restart_preserves_saved_scores() {
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
    let scores: Vec<u64> = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(scores, [1234]);
    game.start(&mut scene).unwrap();
    assert_eq!(game.score.score, 0);
    assert_eq!(game.state()["top5"], serde_json::json!([1234]));
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn blade_held_in_restart_panel_does_not_repeatedly_reset_round() {
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
        game.score.score, 0,
        "leaving and re-entering activates one fresh round"
    );
    assert!(game.round.unwrap().remaining > remaining);
}

#[test]
fn full_native_minute_plays_all_24_cues_and_140_authored_targets() {
    let (mut scene, content) = load();
    let mut game = Game::new(&mut scene, &content, None).unwrap();
    for _ in 0..7205 {
        let f = frame();
        scene.update(f.dt);
        game.tick(&mut scene, f).unwrap();
        assert!(game.flying.len() <= 10);
    }
    assert_eq!(game.cue_index, 24);
    assert_eq!(game.shots, 140);
    assert!(game.round.unwrap().finished());
}
