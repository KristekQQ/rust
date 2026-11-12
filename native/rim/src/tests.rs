use crate::{
    app::{pose, projection},
    content,
};
use glam::{Mat4, Vec3};
#[test]
fn native_prefers_120hz_and_retains_supported_fallback() {
    assert_eq!(
        crate::xr::preferred_refresh_rate(&[72., 80., 90., 120.]),
        Some(120.)
    );
    assert_eq!(crate::xr::preferred_refresh_rate(&[72., 90.]), Some(90.));
    assert_eq!(crate::xr::preferred_refresh_rate(&[72., 80.]), None);
    assert_eq!(crate::xr::preferred_refresh_rate(&[]), None);
}

#[test]
fn native_xr_dash_moves_five_metres_and_holding_does_not_repeat() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/models/rim");
    let mut scene = webgpu_wasm::scene::SceneManager::default();
    let mut content = content::load(&mut scene, &|name| {
        std::fs::read(root.join(name)).map_err(|e| e.to_string())
    })
    .unwrap();
    let game = &mut content.game;
    game.input(
        webgpu_wasm::rim_game::Input {
            start: true,
            ..Default::default()
        },
        &mut scene,
    )
    .unwrap();
    game.set_xr(true);
    let head = Some(Mat4::from_translation(Vec3::new(0., 1.7, 0.)));
    // Re-entry must first see a released button, just like WebXR.
    game.xr_buttons(false, false);
    game.xr_move(head, head, [0.; 3], false, 1. / 120.);
    let before = game.player.position;
    for _ in 0..120 {
        game.xr_buttons(false, true);
        game.xr_move(head, head, [0.; 3], false, 1. / 120.);
    }
    assert_eq!(game.player.dash_count, 1);
    assert!((game.player.position.distance(before) - 5.).abs() < 0.05);
    assert_eq!(game.player.stamina, 3.);
}

#[test]
fn native_cut_worker_executes_real_game_snapshots_and_filters_old_encounters() {
    use crate::cut_worker::CutWorker;
    use std::time::{Duration, Instant};
    use webgpu_wasm::rim_game::{CutWork, Input, PreparedCut};

    fn job() -> CutWork {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/models/rim");
        let mut scene = webgpu_wasm::scene::SceneManager::default();
        let mut content = content::load(&mut scene, &|name| {
            std::fs::read(root.join(name)).map_err(|e| e.to_string())
        })
        .unwrap();
        let game = &mut content.game;
        game.input(
            Input {
                start: true,
                ..Default::default()
            },
            &mut scene,
        )
        .unwrap();
        // Re-approach before each distinct slash: real AI retreats between swings.
        for _ in 0..2 {
            let state = game.state();
            let position = state["enemies"][0]["position"].as_array().unwrap();
            game.player.position = Vec3::new(
                position[0].as_f64().unwrap() as f32,
                0.13,
                position[2].as_f64().unwrap() as f32 + 1.1,
            );
            game.player.yaw = 0.;
            // Refresh the blade sample after the reposition, outside the swing.
            game.tick(&mut scene, 0.01, None, [None; 2]);
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
        }
        assert!(
            game.state()["kills"].as_u64().unwrap() >= 1,
            "{}",
            game.state()
        );
        game.take_cut_work()
            .unwrap()
            .expect("real slash must queue cut work")
    }
    fn wait(worker: &mut CutWorker) -> PreparedCut {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(result) = worker.poll().unwrap() {
                return result;
            }
            assert!(Instant::now() < deadline, "worker reply timed out");
            std::thread::yield_now();
        }
    }

    let mut worker = CutWorker::new().unwrap();
    worker.submit(job()).unwrap();
    worker.submit(job()).unwrap();
    assert!(!worker.available());
    assert!(worker.submit(job()).is_err());
    // All queued snapshots belong to the previous encounter, even if their
    // result has already reached the output channel before invalidation.
    worker.invalidate();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !worker.available() {
        assert!(
            worker.poll().unwrap().is_none(),
            "old encounter must not publish"
        );
        assert!(Instant::now() < deadline, "stale replies timed out");
        std::thread::yield_now();
    }
    worker.submit(job()).unwrap();
    let _prepared = wait(&mut worker);
    assert!(worker.available());
    assert!(worker.poll().unwrap().is_none());
    // Closing input with two outstanding jobs must also let the bounded output
    // channel finish without a receiver drain or a teardown deadlock.
    worker.submit(job()).unwrap();
    worker.submit(job()).unwrap();
    drop(worker);
}
#[test]
fn asymmetric_openxr_projection_preserves_frustum_and_depth() {
    let fov = openxr::Fovf {
        angle_left: -0.8,
        angle_right: 0.6,
        angle_down: -0.55,
        angle_up: 0.7,
    };
    let p = projection(fov, 0.08, 350.);
    for (x, y, want_x, want_y) in [
        (fov.angle_left.tan(), fov.angle_down.tan(), -1., -1.),
        (fov.angle_right.tan(), fov.angle_up.tan(), 1., 1.),
    ] {
        let v = p.project_point3(Vec3::new(x * 0.08, y * 0.08, -0.08));
        assert!((v - Vec3::new(want_x, want_y, 0.)).length() < 1e-5);
    }
    assert!((p.project_point3(Vec3::new(0., 0., -350.)).z - 1.).abs() < 1e-5);
    assert_eq!(pose(openxr::Posef::IDENTITY), Mat4::IDENTITY);
}
#[test]
fn original_rim_assets_and_gameplay_load_without_browser() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/models/rim");
    let mut scene = webgpu_wasm::scene::SceneManager::default();
    let mut content = content::load(&mut scene, &|name| {
        std::fs::read(root.join(name)).map_err(|e| e.to_string())
    })
    .unwrap();
    assert_eq!(scene.models().len(), 19);
    let assets = scene
        .models()
        .values()
        .map(|m| std::sync::Arc::as_ptr(&m.asset) as usize)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        assets.len(),
        7,
        "original instances share immutable GLB assets"
    );
    assert!(!content.game.playing());
    content
        .game
        .input(
            webgpu_wasm::rim_game::Input {
                start: true,
                ..Default::default()
            },
            &mut scene,
        )
        .unwrap();
    assert!(content.game.playing());
    content.game.set_xr(true);
    let head = Some(Mat4::from_translation(Vec3::new(0., 1.7, 0.)));
    content.game.xr_move(head, None, [0.; 3], false, 1. / 72.);
    let origin = content.game.xr_origin;
    content
        .player
        .update_head(&mut scene, head.map(|p| origin * p))
        .unwrap();
    for i in 0..2 {
        let hand = origin
            * Mat4::from_translation(Vec3::new(if i == 0 { -0.25 } else { 0.25 }, 1.25, -0.4));
        content
            .player
            .update_hand(&mut scene, i, Some(hand), true, 1. / 72.)
            .unwrap();
        assert!(content.player.hand(i).is_some());
    }
    content.player.reset_tracking(&mut scene);
    assert!(
        content.player.head().is_none()
            && content.player.hand(0).is_none()
            && content.player.hand(1).is_none()
    );
}

#[test]
fn info_cadence_does_not_confuse_display_hz_with_submitted_fps() {
    let mut scene = webgpu_wasm::scene::SceneManager::default();
    let mut player = webgpu_wasm::xr_player::XrPlayer::default();
    let mut info = crate::info::Info::new(&mut scene, &mut player).unwrap();
    info.refresh_hz = Some(90.);
    for _ in 0..90 {
        info.record(1. / 45., 15.);
    }
    assert!((info.timing.fps - 45.).abs() < 0.01);
    assert_eq!(info.refresh_hz, Some(90.));
    info.reset();
    for _ in 0..90 {
        info.record(1. / 90., 5.);
    }
    assert!((info.timing.fps - 90.).abs() < 0.01);
}
#[test]
fn info_panel_follows_feet_reference_not_gaze_and_hides_on_tracking_loss() {
    use glam::Quat;
    let mut scene = webgpu_wasm::scene::SceneManager::default();
    let mut player = webgpu_wasm::xr_player::XrPlayer::default();
    let mut info = crate::info::Info::new(&mut scene, &mut player).unwrap();
    let reference =
        Mat4::from_rotation_translation(Quat::from_rotation_y(0.7), Vec3::new(4., 0.2, 8.));
    player.set_reference(reference);
    let head = reference * Mat4::from_translation(Vec3::new(0., 1.7, 0.));
    player.update_head(&mut scene, Some(head)).unwrap();
    let expected = reference.transform_point3(crate::info::OFFSET);
    let before = scene.objects()[&info.panel].transform.position;
    assert!(before.distance(expected) < 1e-5);
    player
        .update_head(&mut scene, Some(head * Mat4::from_rotation_y(1.3)))
        .unwrap();
    assert!(
        scene.objects()[&info.panel]
            .transform
            .position
            .distance(before)
            < 1e-5
    );
    player.update_head(&mut scene, None).unwrap();
    assert!(scene.objects()[&info.panel].transform.position.y < -1000.);
    player.update_head(&mut scene, Some(head)).unwrap();
    assert!(
        scene.objects()[&info.panel]
            .transform
            .position
            .distance(expected)
            < 1e-5
    );
    info.toggle(&mut scene, &mut player).unwrap();
    assert!(!info.enabled);
    assert!(scene.objects()[&info.panel].transform.position.y < -1000.);
    info.toggle(&mut scene, &mut player).unwrap();
    player.update_head(&mut scene, Some(head)).unwrap();
    assert!(
        scene.objects()[&info.panel]
            .transform
            .position
            .distance(expected)
            < 1e-5
    );
}

#[test]
fn lethal_encounter_restarts_full_arena_without_recreating_models_or_equipment() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/models/rim");
    let mut scene = webgpu_wasm::scene::SceneManager::default();
    let mut content = content::load(&mut scene, &|name| {
        std::fs::read(root.join(name)).map_err(|e| e.to_string())
    })
    .unwrap();
    let identities: Vec<_> = scene
        .models()
        .iter()
        .map(|(id, m)| (*id, std::sync::Arc::as_ptr(&m.asset) as usize))
        .collect();
    content
        .game
        .input(
            webgpu_wasm::rim_game::Input {
                start: true,
                ..Default::default()
            },
            &mut scene,
        )
        .unwrap();
    content.game.set_xr(true);
    let physical = Mat4::from_translation(Vec3::new(0.6, 1.7, -0.4));
    assert!(!content.restart_if_dead(&mut scene, Some(physical)).unwrap());
    for _ in 0..2400 {
        content
            .game
            .xr_move(Some(physical), None, [0.; 3], false, 1. / 60.);
        let head = content.game.xr_origin * physical;
        content
            .game
            .tick(&mut scene, 1. / 60., Some(head), [None; 2]);
        if content.game.is_dead() {
            break;
        }
    }
    assert!(
        content.game.is_dead(),
        "fixture's original enemy attacks must reach lethal damage"
    );
    content.game.player.position = Vec3::new(40., 0.13, 40.);
    content.game.player.yaw = 0.6;
    assert!(content.restart_if_dead(&mut scene, Some(physical)).unwrap());
    let state = content.game.state();
    assert_eq!(state["health"], 100.);
    assert_eq!(state["wave"], 1);
    assert_eq!(state["cuts"]["pending"], 0);
    assert_eq!(state["projectiles"]["active"], 0);
    assert!(content.game.playing());
    let position = content.game.player.position;
    assert!((position.x - 0.).abs() < 1e-5 && (position.z - 42.).abs() < 1e-5);
    let world_head = (content.game.xr_origin * physical).w_axis.truncate();
    assert!((world_head.x - position.x).abs() < 1e-5 && (world_head.z - position.z).abs() < 1e-5);
    assert!((content.game.player.yaw - 0.6).abs() < 1e-5);
    let after: Vec<_> = scene
        .models()
        .iter()
        .map(|(id, m)| (*id, std::sync::Arc::as_ptr(&m.asset) as usize))
        .collect();
    assert_eq!(identities, after);
    assert!(content.player.head().is_none());
}

#[test]
fn native_floor_reference_prefers_recenterable_floor_without_guessing_height() {
    use openxr::ReferenceSpaceType as S;
    assert_eq!(
        crate::xr::floor_reference(&[S::STAGE, S::LOCAL_FLOOR_EXT], true),
        Some(S::LOCAL_FLOOR_EXT)
    );
    assert_eq!(
        crate::xr::floor_reference(&[S::STAGE, S::LOCAL_FLOOR_EXT], false),
        Some(S::STAGE)
    );
    assert_eq!(crate::xr::floor_reference(&[S::LOCAL], true), None);
}
#[test]
fn native_recenter_resets_at_effective_display_time_and_retains_future_changes() {
    let mut changes = crate::xr::ReferenceChanges::default();
    changes.schedule(200);
    changes.schedule(100);
    changes.schedule(100);
    assert!(!changes.take_due(99));
    assert!(changes.take_due(100));
    assert!(!changes.take_due(101));
    assert!(!changes.take_due(199));
    assert!(changes.take_due(201));
    assert!(!changes.take_due(202));
}
