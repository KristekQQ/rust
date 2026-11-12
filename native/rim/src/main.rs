//! Deterministic native renderer/tracking replay; no headset or OpenXR emulation claim.
use glam::{Mat4, Vec3};
use kristek_rim_native::{
    app::{projection, App},
    content,
};
use std::{error::Error, path::PathBuf};
use webgpu_wasm::render::target::{RenderTarget, RenderView, Viewport};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    let assets = std::env::var_os("RIM_ASSETS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/models/rim")
        });
    let read = |name: &str| std::fs::read(assets.join(name)).map_err(|e| format!("{name}: {e}"));
    if args.iter().any(|a| a == "--xr") {
        return kristek_rim_native::xr::run(&read, &mut || true);
    }
    let out = args
        .iter()
        .position(|a| a == "--output")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("output/native-rim"));
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
        label: Some("Rim native replay"),
        ..Default::default()
    }))?;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut app = App::new(device.clone(), queue.clone(), format, &read)?;
    app.content.game.set_xr(true);
    let width = if args.iter().any(|a| a == "--look-at-info") {
        1792
    } else {
        640
    };
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
    let head = if args.iter().any(|a| a == "--look-at-info") {
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
    let mut tracking_recovery = false;
    for frame in 0..8 {
        let hands = if frame == 3 {
            [None; 2]
        } else {
            [
                Some(Mat4::from_translation(Vec3::new(-0.25, 1.25, -0.4))),
                Some(Mat4::from_translation(Vec3::new(0.25, 1.25, -0.4))),
            ]
        };
        app.tick_xr(1. / 72., Some(head), hands, hands[1], [0.; 3], false, false)?;
        if frame == 4 {
            tracking_recovery =
                app.content.player.hand(0).is_some() && app.content.player.hand(1).is_some();
        }
    }
    if args.iter().any(|a| a == "--look-at-info") {
        // Readability capture: tracked hands already passed recovery checks;
        // hide equipment now so a sword cannot occlude the diagnostics glyphs.
        app.tick_xr(1. / 72., Some(head), [None; 2], None, [0.; 3], false, false)?;
    }
    let mut pixels = Vec::new();
    for eye in 0..2 {
        let world = app.content.game.xr_origin
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
    let report = serde_json::json!({"status":"PASS","host":"native offscreen replay","backend":format!("{:?}",info.backend),"adapter":info.name,"stereoDifferentPixels":different,"uniqueColors":unique,"sceneModelRoots":app.renderer.scene.models().len(),"renderer":app.renderer.debug_values(),"trackingLossRecovery":tracking_recovery,"game":app.content.game.state(),"physicalQuest":false,"openxrRuntime":false,"assets":assets,"background":content::background()});
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    eprintln!(
        "Native replay PASS: {unique} colors, {different} stereo pixels; {}",
        out.display()
    );
    Ok(())
}
