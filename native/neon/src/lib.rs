#![recursion_limit = "256"]
#[cfg(target_os = "android")]
mod android;
pub mod app;
mod audio;
pub mod benchmark_replay;
pub mod content;
pub mod game;
mod observer;
mod scene_observer;
mod scores;
pub mod settings;
#[cfg(test)]
mod tests;

pub mod simulation;

#[cfg(test)]
#[test]
fn full_scene_skips_fruit_then_resumes_spawning() {
    use crate::{
        content::Content,
        game::{Game, Recipe},
    };
    use webgpu_wasm::scene::{SceneManager, SCENE_TRIANGLE_BUDGET_EXCEEDED};

    let mut scene = SceneManager::default();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let content = Content::load(&mut scene, &|name| {
        std::fs::read(root.join(name)).map_err(|e| e.to_string())
    })
    .unwrap();
    let mut game = Game::new(&mut scene, &content, None).unwrap();
    let fruit = content.manifest["runtime"]["catalog"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fruit| fruit["name"] == "watermelon")
        .unwrap();
    let handle = fruit["asset"].as_u64().unwrap() as u32;
    let recipe = Recipe {
        fruit: "watermelon".into(),
        scale: 0.126,
        position: [0., 1., -1.],
        velocity: [0.; 3],
        gravity: 0.,
        lifetime: 5.,
    };
    let mut fillers = Vec::new();
    loop {
        match scene.add_cuttable_mesh_variant(handle, 0, glam::Vec3::ONE) {
            Ok(id) => fillers.push(id),
            Err(error) => {
                assert_eq!(error, SCENE_TRIANGLE_BUDGET_EXCEEDED);
                break;
            }
        }
    }
    assert_eq!(
        game.spawn(&mut scene, std::slice::from_ref(&recipe), true)
            .unwrap(),
        0
    );
    let display = scene.add_mesh_variant(handle, 0).unwrap();
    assert_eq!(
        scene
            .set_cuttable(display, true, glam::Vec3::ONE)
            .unwrap_err(),
        SCENE_TRIANGLE_BUDGET_EXCEEDED
    );
    assert!(!scene.is_cuttable(display));
    for id in fillers {
        scene.remove_object(id);
    }
    scene.set_cuttable(display, true, glam::Vec3::ONE).unwrap();
    assert_eq!(game.spawn(&mut scene, &[recipe], true).unwrap(), 1);
}
