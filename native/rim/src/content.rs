//! Native authoring of the original Aurelia scene. Simulation stays in rim_game.
use glam::{Mat4, Vec3};
use serde::Deserialize;
use std::sync::Arc;
use webgpu_wasm::{
    cutting::CutMesh,
    gltf_loader,
    gltf_model::ModelAsset,
    render_appearance,
    rim_game::{Config, EnemySpec, Game},
    rim_movement::Shape,
    scene::{RenderOptions, SceneManager},
    xr_player::XrPlayer,
};

const HIDDEN: Vec3 = Vec3::new(0., -10000., 0.);

pub struct RimContent {
    pub game: Game,
    pub player: XrPlayer,
    restart_config: Config,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Layout {
    city_file: String,
    decorations_file: String,
    models: Vec<ModelSpec>,
}
#[derive(Deserialize)]
struct ModelSpec {
    slug: String,
    file: String,
    height: f32,
    holder: Placement,
    root: Root,
}
#[derive(Deserialize)]
struct Placement {
    position: [f32; 3],
}
#[derive(Deserialize)]
struct Root {
    position: [f32; 3],
    scale: [f32; 3],
}
#[derive(Deserialize)]
struct Collisions {
    shapes: Vec<Shape>,
    obstacles: Vec<Obstacle>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Obstacle {
    min_x: f32,
    max_x: f32,
    min_z: f32,
    max_z: f32,
}

/// `read` resolves paths relative to assets/models/rim. Loads full glTF hierarchy,
/// PBR materials, skins and animations, sharing immutable assets across instances.
/// Like examples/rim/scene.js this replaces the caller's previous scene.
pub fn load(
    scene: &mut SceneManager,
    read: &dyn Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<RimContent, String> {
    let layout: Layout = serde_json::from_slice(&read("layout.json")?)
        .map_err(|e| format!("Aurelia layout: {e}"))?;
    let collision: Collisions = serde_json::from_slice(&read("collisions.json")?)
        .map_err(|e| format!("Aurelia collisions: {e}"))?;
    scene.clear();
    let city = scene.add_model(import(&layout.city_file, read)?)?;
    scene.set_render_options(
        city,
        RenderOptions {
            is_static: true,
            ..Default::default()
        },
    );
    let decorations = scene.add_model(import(&layout.decorations_file, read)?)?;
    scene.set_render_options(
        decorations,
        RenderOptions {
            is_static: true,
            ..unlit()
        },
    );
    scene.set_object_position(decorations, HIDDEN);

    let mut characters = Vec::new();
    for spec in layout.models {
        let asset = import(&spec.file, read)?;
        let id = scene.add_model(asset.clone())?;
        scene.set_object_position(
            id,
            Vec3::from_array(spec.holder.position) + Vec3::from_array(spec.root.position),
        );
        scene.set_object_scale(id, Vec3::from_array(spec.root.scale));
        let idle = asset
            .animations
            .iter()
            .position(|clip| clip.name.as_deref() == Some("Idle"))
            .ok_or_else(|| format!("{} has no Idle animation", spec.file))?;
        scene.play_model_animation(id, idle, true, 1.)?;
        scene.set_render_options(id, enemy_options());
        characters.push((spec, id, asset));
    }
    scene
        .add_light(Vec3::new(-45., 75., 35.), linear([255., 224., 171.]) * 3.1)
        .ok_or("Cannot allocate Aurelia sun")?;

    let mut enemies = Vec::new();
    for (kind, count) in [("drone-orbiter", 8), ("core-tank", 3), ("blade-wraith", 3)] {
        let (spec, source, asset) = characters
            .iter()
            .find(|(s, _, _)| s.slug == kind)
            .ok_or_else(|| format!("Missing Aurelia enemy {kind}"))?;
        for i in 0..count {
            // JS createInstance starts with an independent default pose as well.
            let id = if i == 0 {
                *source
            } else {
                scene.add_model(asset.clone())?
            };
            scene.set_render_options(id, enemy_options());
            enemies.push(EnemySpec {
                id,
                kind: kind.into(),
                height: spec.height,
                offset: spec.root.position,
                scale: spec.root.scale,
            });
        }
    }
    let gladius = import("gladius.glb", read)?;
    let swords = [scene.add_model(gladius.clone())?, scene.add_model(gladius)?];
    for sword in swords {
        scene.set_cutter(
            sword,
            Some([Vec3::new(0., 0., -0.14), Vec3::new(0., 0., -1.02)]),
        )?;
        scene.set_blade_trail(sword, true)?;
        scene.set_render_options(sword, unlit());
    }
    let bolts = (0..32)
        .map(|_| {
            let id = sphere(scene, 1.5);
            scene.set_object_scale(id, Vec3::splat(0.055));
            id
        })
        .collect();
    let beams = (0..14).map(|_| cube(scene, 3.)).collect();
    let charges = (0..14).map(|_| sphere(scene, 2.)).collect();
    let mut health_bars = Vec::new();
    for _ in &enemies {
        health_bars.push([
            bar(scene, Vec3::new(0.025, 0.035, 0.05))?,
            bar(scene, Vec3::new(0.12, 0.9, 0.3))?,
        ]);
    }
    let mut health_drops = Vec::new();
    for _ in 0..4 {
        let green = Vec3::new(0.12, 1.0, 0.3);
        health_drops.push([bar(scene, green)?, bar(scene, green)?, bar(scene, green)?]);
    }
    let stage_panel = scene.add_info_panel("debug-text", 0.34, 0.025)?;
    scene.set_object_position(stage_panel, HIDDEN);
    scene.set_render_options(stage_panel, unlit());
    let blob_shadows = blob_shadows(scene, enemies.len())?;
    let hud = (0..5).map(|_| cube(scene, 1.)).collect();
    let marker = sphere(scene, 1.);
    scene.set_object_scale(marker, Vec3::new(0.3, 0.025, 0.3));
    let restart_config = Config {
        enemies,
        swords,
        bolts,
        beams,
        charges,
        health_bars,
        health_drops,
        stage_panel: Some(stage_panel),
        blob_shadows,
        hud,
        marker,
        shapes: collision.shapes,
        obstacles: collision
            .obstacles
            .into_iter()
            .map(|r| [r.min_x, r.max_x, r.min_z, r.max_z])
            .collect(),
    };
    let game = Game::new(copy_config(&restart_config), scene)?;
    let mut player = XrPlayer::default();
    for (hand, sword) in swords.into_iter().enumerate() {
        // Matches equip(..., {requireTrigger:false}); rim_game owns combat cuts.
        player.bind_hand(scene, hand, Some(sword), false)?;
    }
    Ok(RimContent {
        game,
        player,
        restart_config,
    })
}

impl RimContent {
    /// A fresh encounter reuses all existing GLB roots/GPU assets and equipment.
    /// Teleport the rig to the original safe spawn, compensating room-scale XZ,
    /// while retaining the player's reference yaw rather than rotating the head.
    pub fn restart_if_dead(
        &mut self,
        scene: &mut SceneManager,
        physical_head: Option<Mat4>,
    ) -> Result<bool, String> {
        if !self.game.is_dead() {
            return Ok(false);
        }
        let yaw = self.game.player.yaw;
        self.game.teardown(scene);
        self.player.reset_tracking(scene);
        let mut game = Game::new(copy_config(&self.restart_config), scene)?;
        game.player.yaw = yaw;
        game.set_xr(true);
        if let Some(head) = physical_head {
            let position = head.w_axis.truncate();
            let shift = game
                .xr_origin
                .transform_vector3(Vec3::new(position.x, 0., position.z));
            game.xr_origin.w_axis -= shift.extend(0.);
        }
        game.input(
            webgpu_wasm::rim_game::Input {
                start: true,
                ..Default::default()
            },
            scene,
        )?;
        self.game = game;
        Ok(true)
    }
}
// Config intentionally remains engine-owned; only the native scene recipe keeps
// a restart template. No asset parsing or GPU recreation occurs on death.
fn copy_config(c: &Config) -> Config {
    Config {
        enemies: c
            .enemies
            .iter()
            .map(|e| EnemySpec {
                id: e.id,
                kind: e.kind.clone(),
                height: e.height,
                offset: e.offset,
                scale: e.scale,
            })
            .collect(),
        swords: c.swords,
        bolts: c.bolts.clone(),
        beams: c.beams.clone(),
        charges: c.charges.clone(),
        health_bars: c.health_bars.clone(),
        health_drops: c.health_drops.clone(),
        stage_panel: c.stage_panel,
        blob_shadows: c.blob_shadows.clone(),
        hud: c.hud.clone(),
        marker: c.marker,
        shapes: c.shapes.clone(),
        obstacles: c.obstacles.clone(),
    }
}

fn import(
    file: &str,
    read: &dyn Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<Arc<ModelAsset>, String> {
    let bytes = read(file).map_err(|e| format!("{file}: {e}"))?;
    let resources = gltf_loader::resource_uris(&bytes)?;
    let directory = file.rsplit_once('/').map_or("", |(directory, _)| directory);
    let resolve = |uri: &String| -> Result<Vec<u8>, String> {
        if uri.is_empty() {
            return Ok(Vec::new());
        }
        read(&if directory.is_empty() {
            uri.clone()
        } else {
            format!("{directory}/{uri}")
        })
    };
    let buffers = resources
        .buffers
        .iter()
        .map(resolve)
        .collect::<Result<Vec<_>, _>>()?;
    let images = resources
        .images
        .iter()
        .map(resolve)
        .collect::<Result<Vec<_>, _>>()?;
    gltf_loader::parse_model(&bytes, &buffers, &images)
        .map(Arc::new)
        .map_err(|e| format!("{file}: {e}"))
}
fn enemy_options() -> RenderOptions {
    RenderOptions {
        casts_shadow: false,
        receives_shadow: true,
        opaque_reorderable: true,
        ..Default::default()
    }
}
fn unlit() -> RenderOptions {
    RenderOptions {
        casts_shadow: false,
        receives_shadow: false,
        ..Default::default()
    }
}
fn cube(scene: &mut SceneManager, emission: f32) -> u32 {
    let id = scene.add_cube(HIDDEN);
    scene.set_emission(id, emission);
    scene.set_render_options(id, unlit());
    id
}
fn sphere(scene: &mut SceneManager, emission: f32) -> u32 {
    let id = scene.add_sphere(HIDDEN);
    scene.set_emission(id, emission);
    scene.set_render_options(id, unlit());
    id
}
fn bar(scene: &mut SceneManager, color: Vec3) -> Result<u32, String> {
    let positions = [
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [0.5, 0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
    ]
    .into_iter()
    .map(Vec3::from_array)
    .collect();
    let indices = [
        0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2, 0, 4, 7, 0, 7, 3,
        1, 2, 6, 1, 6, 5,
    ];
    let id = scene.add_triangle_mesh(CutMesh::new(positions, &indices, color)?)?;
    scene.set_object_position(id, HIDDEN);
    scene.set_emission(id, 1.);
    scene.set_render_options(id, unlit());
    Ok(id)
}
fn blob_shadows(scene: &mut SceneManager, count: usize) -> Result<Vec<u32>, String> {
    // Original shared closed 32-segment thin disk, including its lower alpha face.
    let segments = 32u32;
    let bottom = segments + 1;
    let mut positions = vec![0., 0., 0.];
    for i in 0..segments {
        let a = i as f32 * std::f32::consts::TAU / segments as f32;
        positions.extend([a.cos(), 0., a.sin()]);
    }
    positions.extend([0., -0.001, 0.]);
    for i in 0..segments {
        let a = i as f32 * std::f32::consts::TAU / segments as f32;
        positions.extend([a.cos(), -0.001, a.sin()]);
    }
    let mut indices = Vec::new();
    for i in 0..segments {
        let t = i + 1;
        let n = (i + 1) % segments + 1;
        let b = bottom + t;
        let bn = bottom + n;
        indices.extend([0, n, t, bottom, b, bn, t, n, b, n, bn, b]);
    }
    let handle =
        scene.register_mesh_variants(&positions, &indices, &vec![0.; indices.len()], false)?;
    let result = (0..count)
        .map(|_| {
            let id = scene.add_mesh_variant(handle, 0)?;
            scene.set_object_position(id, HIDDEN);
            scene.set_opacity(id, 0.08, 8., 0., -1.);
            scene.set_render_options(id, unlit());
            Ok(id)
        })
        .collect();
    scene.remove_mesh_variants(handle);
    result
}
fn linear(rgb: [f32; 3]) -> Vec3 {
    Vec3::from_array(rgb.map(|c| {
        let v = c / 255.;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }))
}

pub fn background() -> [f32; 3] {
    linear([184., 208., 202.]).to_array()
}

/// Original examples/rim/appearance.js settings. Host also enables shadows/FXAA,
/// disables grid/light helpers and disables bloom to match the browser example.
pub fn appearance() -> render_appearance::Config {
    render_appearance::Config {
        sky: (linear([229., 244., 255.]) * 2.5).to_array(),
        ground: (linear([183., 152., 104.]) * 2.5).to_array(),
        sun_direction: [-45., 75., 50.],
        exposure: 1.25,
        fog_color: background(),
        fog_near: 90.,
        fog_far: 245.,
        shadow_extent: 95.,
        shadow_target: [0., 0., -15.],
        shadow_near: 1.,
        shadow_far: 220.,
        shadow_normal_bias: 0.12,
        specular_aa: true,
    }
}
