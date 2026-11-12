//! Deterministic native renderer/tracking replay; no headset or OpenXR emulation claim.
use glam::{Mat4, Vec3};
use kristek_neon_native::app::{
    App, NEON_GLOW_RADIUS, NEON_GLOW_STRENGTH, NEON_SSR_SCALE, NEON_SSR_STRENGTH,
};
use kristek_rim_native::{
    app::projection,
    xr::{FrameInput, HostApp},
};
use std::{
    error::Error,
    path::PathBuf,
    time::{Duration, Instant},
};
use webgpu_wasm::render::profiler;
use webgpu_wasm::render::target::{RenderTarget, RenderView, Viewport};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    let benchmark_frames = cli_value::<u32>(&args, "--benchmark-frames")?.unwrap_or(0);
    let benchmark_warmup = cli_value::<u32>(&args, "--benchmark-warmup")?.unwrap_or(60);
    let benchmark_period_ms = cli_value::<f64>(&args, "--benchmark-period-ms")?.unwrap_or(16.667);
    let size = cli_value::<u32>(&args, "--size")?;
    if benchmark_frames > 3600 {
        return Err("--benchmark-frames must be in 0..=3600".into());
    }
    if !benchmark_period_ms.is_finite() || benchmark_period_ms < 0. {
        return Err("--benchmark-period-ms must be finite and nonnegative".into());
    }
    let benchmark_period = Duration::try_from_secs_f64(benchmark_period_ms / 1000.)?;
    let total_frames = benchmark_warmup
        .checked_add(benchmark_frames)
        .ok_or("benchmark warmup + frames overflow")?;
    if size.is_some_and(|size| !(128..=2048).contains(&size) || size % 64 != 0) {
        return Err("--size must be a multiple of 64 in 128..=2048".into());
    }
    let glow_off = args.iter().any(|a| a == "--glow-off");
    let glow_resolution = cli_value::<f32>(&args, "--glow-resolution")?.unwrap_or(1.0);
    if glow_resolution != 1.0 && glow_resolution != 0.5 {
        return Err("--glow-resolution must be 1 or 0.5".into());
    }
    let font_glow_atlas = !args.iter().any(|a| a == "--no-font-glow-atlas");
    let inspect_digits = args.iter().any(|a| {
        matches!(
            a.as_str(),
            "--look-at-digits" | "--look-at-digits-oblique" | "--look-at-digits-side"
        )
    });
    let assets = std::env::var_os("NEON_ASSETS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"));
    let read = |name: &str| std::fs::read(assets.join(name)).map_err(|e| format!("{name}: {e}"));
    if args.iter().any(|a| a == "--xr") {
        return kristek_rim_native::xr::run_with(
            "Kristek Neon Native",
            &read,
            &mut || true,
            &mut |d, q, f, read| Ok(Box::new(App::new(d, q, f, read, None)?) as Box<dyn HostApp>),
        );
    }
    let out = args
        .iter()
        .position(|a| a == "--output")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("output/native-neon"));
    std::fs::create_dir_all(&out)?;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))?;
    let info = adapter.get_info();
    eprintln!("Native GPU: {} / {:?}", info.name, info.backend);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("Neon native replay"),
        required_features: if benchmark_frames > 0 {
            adapter.features()
                & (wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES)
        } else {
            wgpu::Features::empty()
        },
        ..Default::default()
    }))?;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut app = App::new(device.clone(), queue.clone(), format, &read, None)?;
    app.renderer.configure_font_glow_atlas(font_glow_atlas);
    app.renderer.configure_neon_glow_resolution(glow_resolution);
    app.set_xr(true);
    let ssr_enabled = !args.iter().any(|a| a == "--ssr-off");
    app.replay_ssr(ssr_enabled);
    if glow_off {
        app.renderer.configure_neon_glow(0., NEON_GLOW_RADIUS);
    }
    let inspect_swords = args.iter().any(|a| a == "--look-at-swords");
    let inspect_cut = args.iter().any(|a| a == "--look-at-cut");
    let width = size.unwrap_or(if inspect_swords {
        1280
    } else if args.iter().any(|a| a == "--look-at-info") {
        1792
    } else {
        640
    });
    let height = width;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Replay eye"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let color = target.create_view(&Default::default());
    let (_depth, depth) = webgpu_wasm::render::depth::create(
        &device,
        width,
        height,
        wgpu::TextureFormat::Depth32Float,
    );
    let head = if args.iter().any(|a| a == "--look-at-digits-side") {
        glam::camera::rh::view::look_at_mat4(
            Vec3::new(1.8, 2.25, -2.05),
            Vec3::new(0., 2.15, -2.15),
            Vec3::Y,
        )
        .inverse()
    } else if args.iter().any(|a| a == "--look-at-digits-oblique") {
        glam::camera::rh::view::look_at_mat4(
            Vec3::new(1.5, 2.25, -0.8),
            Vec3::new(0., 2.15, -2.15),
            Vec3::Y,
        )
        .inverse()
    } else if args.iter().any(|a| a == "--look-at-digits") {
        glam::camera::rh::view::look_at_mat4(
            Vec3::new(0., 2.25, -0.8),
            Vec3::new(0., 2.15, -2.15),
            Vec3::Y,
        )
        .inverse()
    } else if inspect_cut {
        glam::camera::rh::view::look_at_mat4(
            Vec3::new(1.95, 1.55, -1.1),
            Vec3::new(0.45, 1.35, -1.1),
            Vec3::Y,
        )
        .inverse()
    } else if args.iter().any(|a| a == "--look-at-floor") {
        glam::camera::rh::view::look_at_mat4(
            Vec3::new(0., 1.7, 0.8),
            Vec3::new(0., 0., -1.5),
            Vec3::Y,
        )
        .inverse()
    } else if args.iter().any(|a| a == "--look-at-info") {
        glam::camera::rh::view::look_at_mat4(
            Vec3::new(0., 1.7, 0.),
            kristek_rim_native::info::OFFSET,
            Vec3::Y,
        )
        .inverse()
    } else {
        Mat4::from_translation(Vec3::new(0., 1.7, 0.))
    };
    let fov = openxr::Fovf {
        angle_left: -0.7,
        angle_right: 0.7,
        angle_up: 0.7,
        angle_down: -0.7,
    };
    // Broadside inspection poses expose the actual equipped meshes in both eyes.
    let sword_poses = [-1., 1.].map(|side| {
        Some(
            Mat4::from_translation(Vec3::new(side * 0.24, 1.25, -0.95))
                * Mat4::from_rotation_z(side * -0.10)
                * Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2)
                * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2),
        )
    });
    let mut tracking_recovery = false;
    for frame in 0..102 {
        let hands = if frame == 3 {
            [None; 2]
        } else if inspect_swords {
            sword_poses
        } else {
            [
                Some(Mat4::from_translation(Vec3::new(-0.25, 1.25, -0.4))),
                Some(Mat4::from_translation(Vec3::new(0.25, 1.25, -0.4))),
            ]
        };
        app.tick_replay(FrameInput {
            dt: 1. / 120.,
            head: Some(head),
            hands,
            rays: hands,
            ..Default::default()
        })?;
        if frame == 4 {
            tracking_recovery = app.player.hand(0).is_some() && app.player.hand(1).is_some();
        }
    }
    if args.iter().any(|a| a == "--look-at-info") {
        // Readability capture: tracked hands already passed recovery checks;
        // hide equipment now so a sword cannot occlude the diagnostics glyphs.
        app.tick_replay(FrameInput {
            dt: 1. / 120.,
            head: Some(head),
            ..Default::default()
        })?;
    }
    let before_round = app.simulation_state["round"]["remaining"].as_f64().unwrap();
    let first = app.simulation_state["flying"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|v| v.as_u64())
        .map(|id| id as u32);
    let before_position = first.map(|id| app.renderer.scene.objects()[&id].transform.position);
    app.tick_replay(FrameInput {
        dt: 0.25,
        head: None,
        ..Default::default()
    })?;
    let tracking_pause_preserved = app.simulation_state["round"]["remaining"].as_f64().unwrap()
        == before_round
        && first.map(|id| app.renderer.scene.objects()[&id].transform.position) == before_position;
    if !tracking_pause_preserved {
        return Err("Tracking loss advanced native round or projectile".into());
    }
    app.tick_replay(FrameInput {
        dt: 1. / 120.,
        head: Some(head),
        hands: if inspect_swords {
            sword_poses
        } else {
            [None; 2]
        },
        ..Default::default()
    })?;
    if inspect_cut {
        // Explicit offscreen fixture: the real authored fruit, tracked katana,
        // grip velocity, cap-derived plane, pooled texture and live animation.
        let number = |name: &str, default: f32| {
            args.iter()
                .position(|a| a == name)
                .and_then(|i| args.get(i + 1))
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(default)
        };
        let speed = number("--cut-speed", 3.0).clamp(0.5, 10.);
        let age = number("--cut-age", 0.08).clamp(0., 0.64);
        // Use the SAME simulation worker -> publication -> render mirror path
        // as OpenXR. A local Game on renderer.scene would bypass semantic tags.
        let mut scene = webgpu_wasm::scene::SceneManager::default();
        let content = kristek_neon_native::content::Content::load(&mut scene, &read)?;
        let mut game = kristek_neon_native::game::Game::new(&mut scene, &content, None)?;
        game.start(&mut scene)?;
        game.cue_index = 24;
        game.spawn(
            &mut scene,
            &[kristek_neon_native::game::Recipe {
                fruit: "watermelon".into(),
                scale: 0.126,
                position: [0.45, 1.35, -1.1],
                velocity: [0.; 3],
                gravity: 0.,
                lifetime: 5.,
            }],
            false,
        )?;
        app.simulation = kristek_neon_native::simulation::Simulation::new(scene, game)?;
        // This fixture starts a new sequence space; the old worker's publication
        // counter must not make tick_replay skip waiting for the new worker.
        app.applied_sequence = 0;
        for step in 0..25 {
            let dt = 0.025 / speed;
            let mut f = FrameInput {
                dt,
                head: Some(Mat4::from_translation(Vec3::new(0., 1.55, 0.))),
                ..Default::default()
            };
            f.hands[1] = Some(Mat4::from_translation(Vec3::new(
                0.45,
                1. + step as f32 * 0.025,
                -0.45,
            )));
            app.tick_replay(f)?;
            if app.simulation_state["cuts"].as_u64().unwrap_or(0) > 0 {
                break;
            }
        }
        if app.simulation_state["cuts"].as_u64().unwrap_or(0) == 0 {
            return Err("Tracked inspection katana missed authored fruit".into());
        }
        let mut remaining = age;
        while remaining > 0. {
            let dt = remaining.min(0.05);
            app.tick_replay(FrameInput {
                dt,
                head: Some(Mat4::from_translation(Vec3::new(0., 1.55, 0.))),
                ..Default::default()
            })?;
            remaining -= dt;
        }
        eprintln!(
            "Worker cut fixture: speed={speed} age={age} sequence={} state={}",
            app.applied_sequence, app.simulation_state
        );
        let models: Vec<_> = app.renderer.scene.models().keys().copied().collect();
        for model in models {
            app.renderer.register_gltf_model(model)?;
        }
        app.renderer.finish_scene_updates();
    }
    if benchmark_frames > 0 {
        // Runtime worker/age numbers vary between processes. Keep a visible,
        // fixed panel in this frozen GPU fixture instead of masking its pixels.
        let rows: Vec<_> = [
            "RENDER",
            "FPS APP",
            "DISPLAY HZ",
            "FRAME MS",
            "FRAME P95",
            "APP+WAIT MS",
            "UPDATE MS",
            "SIM MS",
            "UPLOAD B",
            "VISIBLE",
            "OBJECTS",
            "DRAWS",
            "SIM AGE MS",
        ]
        .into_iter()
        .map(|key| {
            (
                key,
                if key == "RENDER" {
                    "RUST VULKAN".into()
                } else {
                    "N/A".into()
                },
            )
        })
        .collect();
        app.renderer.scene.update_info_panel(app.info.panel, &rows);
        app.renderer.finish_scene_updates();
        // Freeze the replay exactly where the ordinary image QA starts. No tick,
        // scene finalization or frame_submitted call belongs in this loop.
        let views = [0, 1].map(|eye| {
            let world = head
                * Mat4::from_translation(Vec3::new(if eye == 0 { -0.032 } else { 0.032 }, 0., 0.));
            RenderView {
                view_projection: projection(fov, 0.08, 350.) * world.inverse(),
                camera_position: world.w_axis.truncate(),
            }
        });
        let render_target = RenderTarget {
            color: &color,
            depth: &depth,
            material_mask: false,
            auxiliary: None,
            color_format: format,
            depth_format: wgpu::TextureFormat::Depth32Float,
            width,
            height,
            viewport: Viewport::full(width, height),
            color_load: wgpu::LoadOp::Load,
            depth_load: wgpu::LoadOp::Clear(1.),
            depth_store: wgpu::StoreOp::Discard,
        };
        let mut samples = Vec::with_capacity((benchmark_frames / 30 + 2) as usize);
        let mut observations = Vec::with_capacity((total_frames / 10 + 2) as usize);
        let mut last_sample = 0;
        let mut observe = |phase: &str| {
            let profile = profiler::stats();
            let gpu = &profile["gpu"];
            if let Some(frame) = gpu["sampleFrame"].as_u64() {
                if gpu["available"].as_bool() == Some(true)
                    && frame > benchmark_warmup as u64
                    && frame > last_sample
                {
                    samples.push(serde_json::json!({
                        "observedFrame":profile["frame"], "phase":phase, "gpu":gpu
                    }));
                    last_sample = frame;
                }
            }
            observations.push(serde_json::json!({
                "observedFrame":profile["frame"], "phase":phase, "gpu":gpu
            }));
        };
        profiler::initialize(&device);
        let started = Instant::now();
        for frame in 1..=total_frames {
            let frame_started = Instant::now();
            profiler::begin_frame();
            app.renderer.draw_calls = 0;
            app.renderer.main_draw_calls = 0;
            for (eye, view) in views.iter().enumerate() {
                app.render(*view, &render_target, eye as u32 + 1)?;
            }
            profiler::end_frame(&device, &queue);
            device.poll(wgpu::PollType::Poll)?;
            if frame % 10 == 0 || frame == total_frames {
                observe("paced-loop");
            }
            if let Some(remaining) = benchmark_period.checked_sub(frame_started.elapsed()) {
                std::thread::sleep(remaining);
            }
        }
        let paced_loop_seconds = started.elapsed().as_secs_f64();
        // GPU completion and the final timestamp harvest are outside measurement.
        device.poll(wgpu::PollType::wait_indefinitely())?;
        profiler::begin_frame();
        observe("after-drain");
        drop(observe);
        let final_profile = profiler::stats();
        let benchmark = serde_json::json!({
            "host":"native offscreen frozen stereo replay",
            "executionOS":std::env::consts::OS,
            "deviceLabel":std::env::var("NEON_PHYSICAL_DEVICE").ok(),
            "physicalQuest":cfg!(target_os = "android") && std::env::var("NEON_PHYSICAL_DEVICE").as_deref() == Ok("Quest_3"),
            "openxrRuntime":false, "fpsMeasurement":false,
            "timing":"sampled GPU whole-pass durations; nested sections are separate and must not be summed with passes; pacing is diagnostic only",
            "adapter":{"name":info.name,"backend":format!("{:?}",info.backend),
                "vendor":info.vendor,"device":info.device,
                "driver":info.driver,"driverInfo":info.driver_info,
                "availableFeatures":format!("{:?}",adapter.features()),
                "requestedFeatures":format!("{:?}",device.features())},
            "config":{"frames":benchmark_frames,"warmupFrames":benchmark_warmup,
                "totalRenderedFrames":total_frames,"periodMs":benchmark_period_ms,
                "width":width,"height":height,"eyesPerFrame":2,"ssrEnabled":ssr_enabled,
                "glowOff":glow_off,"fontGlowAtlas":font_glow_atlas,"glowResolution":glow_resolution,
                "view":if inspect_cut {"cut"} else {"player"},"format":format!("{:?}",format),
                "effectParams":{"ssrStrength":NEON_SSR_STRENGTH,"ssrScale":NEON_SSR_SCALE,
                    "glowStrength":if glow_off {0.} else {NEON_GLOW_STRENGTH},"glowRadius":NEON_GLOW_RADIUS},
                "depthStore":"Discard","frozenSimulation":true,"assets":assets,
                "arguments":args,"renderer":app.renderer.debug_values()},
            "pacedLoopSeconds":paced_loop_seconds,
            "samples":samples,"observations":observations,"finalGpu":final_profile["gpu"]
        });
        std::fs::write(
            out.join("benchmark.json"),
            serde_json::to_vec_pretty(&benchmark)?,
        )?;
        profiler::reset();
        app.renderer.draw_calls = 0;
        app.renderer.main_draw_calls = 0;
    }
    let mut pixels = Vec::new();
    for eye in 0..2 {
        let world = Mat4::IDENTITY
            * head
            * Mat4::from_translation(Vec3::new(if eye == 0 { -0.032 } else { 0.032 }, 0., 0.));
        let view = RenderView {
            view_projection: projection(fov, 0.08, 350.) * world.inverse(),
            camera_position: world.w_axis.truncate(),
        };
        app.render(
            view,
            &RenderTarget {
                color: &color,
                depth: &depth,
                material_mask: false,
                auxiliary: None,
                color_format: format,
                depth_format: wgpu::TextureFormat::Depth32Float,
                width,
                height,
                viewport: Viewport::full(width, height),
                color_load: wgpu::LoadOp::Load,
                depth_load: wgpu::LoadOp::Clear(1.),
                depth_store: wgpu::StoreOp::Discard,
            },
            eye + 1,
        )?;
        let bytes = (width * height * 4) as u64;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Replay readback"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let (send, recv) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = send.send(result);
            });
        device.poll(wgpu::PollType::wait_indefinitely())?;
        recv.recv()??;
        let data = buffer.slice(..).get_mapped_range()?.to_vec();
        buffer.unmap();
        let file =
            std::fs::File::create(out.join(if eye == 0 { "left.png" } else { "right.png" }))?;
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&data)?;
        pixels.push(data);
    }
    let different = pixels[0]
        .chunks_exact(4)
        .zip(pixels[1].chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    let unique = pixels[0]
        .chunks_exact(4)
        .map(|p| [p[0], p[1], p[2]])
        .collect::<std::collections::HashSet<_>>()
        .len();
    if different < 10000 || unique < 1000 || !tracking_recovery {
        return Err("Native replay did not produce valid stereo/tracking evidence".into());
    }
    let reflection_work = app.reflection_work();
    // Close glyph inspections may legitimately exclude the reflective floor.
    if ssr_enabled && !inspect_digits && reflection_work[0] <= 0.0 {
        return Err("SSR enabled but no reflection pixels were traced".into());
    }
    let report = serde_json::json!({"status":"PASS","host":"native offscreen replay","backend":format!("{:?}",info.backend),"adapter":info.name,"stereoDifferentPixels":different,"uniqueColors":unique,"ssrEnabled":ssr_enabled,"ssrWork":reflection_work,"sceneModelRoots":app.renderer.scene.models().len(),"renderer":app.renderer.debug_values(),"trackingLossRecovery":tracking_recovery,"trackingPausePreserved":tracking_pause_preserved,"game":app.simulation_state,"executionOS":std::env::consts::OS,"physicalQuest":cfg!(target_os = "android") && std::env::var("NEON_PHYSICAL_DEVICE").as_deref() == Ok("Quest_3"),"openxrRuntime":false,"fpsMeasurement":false,"assets":assets,"background":[0.,0.,0.]});
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    eprintln!(
        "Native replay PASS: {unique} colors, {different} stereo pixels; {}",
        out.display()
    );
    Ok(())
}

/// Optional numeric flags reject missing/invalid values before GPU setup.
fn cli_value<T: std::str::FromStr>(args: &[String], flag: &str) -> Result<Option<T>, Box<dyn Error>>
where
    T::Err: std::fmt::Display,
{
    args.iter()
        .position(|arg| arg == flag)
        .map(|index| {
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("{flag} requires a value"))?;
            value
                .parse::<T>()
                .map_err(|error| format!("{flag}: {error}").into())
        })
        .transpose()
}
