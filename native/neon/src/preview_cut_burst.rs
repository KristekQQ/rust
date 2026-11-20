//! Opt-in repeated cut replay through the real game, simulation worker and render mirror.
use glam::{Mat4, Vec3};
use kristek_neon_native::{app::App, content::Content, game::Recipe};
use kristek_rim_native::xr::{FrameInput, HostApp};
use serde_json::{json, Value};
use std::{error::Error, time::Instant};
use webgpu_wasm::render::profiler;

// The configured count fits in the ordinary one-minute round even at minimum cut speed.
const FRUIT_INTERVAL_S: f32 = 2.45;
const FRUIT_POSITION: [f32; 3] = [0.45, 1.35, -1.1];
pub const MAX_CUTS: u32 = 24;

pub fn recipe() -> Recipe {
    Recipe {
        fruit: "watermelon".into(),
        scale: 0.126,
        position: FRUIT_POSITION,
        velocity: [0.; 3],
        gravity: 0.,
        lifetime: 5.,
    }
}

pub fn schedule(content: &mut Content, count: u32) {
    let fruit = recipe();
    content.manifest["runtime"]["minutePlan"]["cues"] = Value::Array(
        (1..count)
            .map(|index| {
                json!({
                    "at": index as f32 * FRUIT_INTERVAL_S,
                    "label": "",
                    "recipes": [{
                        "fruit": fruit.fruit, "scale": fruit.scale, "position": fruit.position,
                        "velocity": fruit.velocity, "gravity": fruit.gravity, "lifetime": fruit.lifetime,
                    }],
                })
            })
            .collect(),
    );
}

pub fn idle(dt: f32) -> FrameInput {
    FrameInput {
        dt,
        head: Some(Mat4::from_translation(Vec3::new(0., 1.55, 0.))),
        ..Default::default()
    }
}

pub fn swing(step: u32, speed: f32) -> FrameInput {
    let mut frame = idle(0.025 / speed);
    frame.hands[1] = Some(Mat4::from_translation(Vec3::new(
        FRUIT_POSITION[0],
        1. + step as f32 * 0.025,
        -0.45,
    )));
    frame
}

pub fn run(
    app: &mut App,
    count: u32,
    speed: f32,
    worker_upload: bool,
    pool: bool,
    mut draw: impl FnMut(&mut App, Option<u32>) -> Result<(), Box<dyn Error>>,
) -> Result<Value, Box<dyn Error>> {
    let mut hits = Vec::with_capacity(count as usize);
    let device = app.device.clone();
    let queue = app.queue.clone();
    let mut draw_frames = 0;
    let mut retired_sequence = None;
    for hit in 1..=count {
        // No blade during this interval. Render on fragment retirement and once
        // the next fruit exists, so old GPU references can actually be recycled.
        let mut ready = false;
        for _ in 0..36 {
            let had_fragments = number(app, "fragments") > 0;
            app.tick_replay(idle(0.1))?;
            let fragments = number(app, "fragments");
            let retired = had_fragments && fragments == 0;
            if retired {
                retired_sequence = Some(app.applied_sequence);
            }
            ready = fragments == 0 && number(app, "shots") == u64::from(hit);
            if retired || ready {
                draw(app, None)?;
                draw_frames += 1;
            }
            if ready {
                break;
            }
        }
        if !ready || number(app, "cuts") != u64::from(hit - 1) {
            return Err(format!(
                "Cut burst {hit} did not reach its ready state: {}",
                app.simulation_state
            )
            .into());
        }
        let adopted_before = app.renderer.worker_cut_meshes_adopted;
        profiler::initialize(&device);
        let mut hit_sample = None;
        for step in 0..25 {
            profiler::begin_frame();
            let started = Instant::now();
            app.tick_replay(swing(step, speed))?;
            let replay_ms = started.elapsed().as_secs_f64() * 1000.;
            profiler::end_frame(&device, &queue);
            if number(app, "cuts") == u64::from(hit - 1) {
                continue;
            }
            let adopted = app.renderer.worker_cut_meshes_adopted - adopted_before;
            if number(app, "cuts") != u64::from(hit)
                || number(app, "fragments") != 2
                || adopted != if worker_upload { 2 } else { 0 }
            {
                return Err(format!(
                    "Cut burst {hit} did not install exactly two halves (adopted={adopted}): {}",
                    app.simulation_state
                )
                .into());
            }
            let stats = profiler::stats();
            let mesh_install_ms = stats["cpu"]["mesh.install"]["lastMs"]
                .as_f64()
                .ok_or("Cut frame did not record mesh.install")?;
            eprintln!(
                "Cut burst {hit}/{count} replay_ms={replay_ms:.3} mesh_install_ms={mesh_install_ms:.3} adopted={adopted} seq={}",
                app.applied_sequence
            );
            hit_sample = Some(json!({
                "hit": hit, "sequence": app.applied_sequence, "strokeStep": step,
                "previousFragmentsRetiredAtSequence": retired_sequence,
                "replayMs": replay_ms, "meshInstallMs": mesh_install_ms,
                "workerCutMeshesAdopted": adopted,
            }));
            break;
        }
        profiler::reset();
        hits.push(hit_sample.ok_or_else(|| format!("Tracked katana missed burst fruit {hit}"))?);
        draw(app, Some(hit))?;
        draw_frames += 1;
    }
    let pool_stats = app.measurement_metrics().cut_pool;
    if pool && !pool_stats.is_some_and(|stats| stats.reuses > 0) {
        return Err("Burst never reused any GPU buffers".into());
    }
    Ok(json!({
        "cutPoolStats": pool_stats, "status": "PASS", "workerUpload": worker_upload, "cutBufferPool": pool,
        "cutSpeed": speed, "fruitIntervalSeconds": FRUIT_INTERVAL_S,
        "drawnFrames": draw_frames, "hits": hits,
    }))
}

fn number(app: &App, field: &str) -> u64 {
    app.simulation_state[field].as_u64().unwrap_or(0)
}
