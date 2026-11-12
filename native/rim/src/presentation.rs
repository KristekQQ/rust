//! Shared per-eye native scene/postprocessing/overlay composition.
use glam::Vec3;
use webgpu_wasm::render::{
    postprocess::PostProcessor,
    renderer::SceneRenderer,
    target::{RenderTarget, RenderView},
};
pub fn render_scene(
    renderer: &mut SceneRenderer,
    post: &mut PostProcessor,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    view: RenderView,
    target: &RenderTarget<'_>,
    slot: u32,
    bg: [f32; 3],
) -> Result<(), String> {
    let target = RenderTarget {
        color_load: wgpu::LoadOp::Clear(wgpu::Color {
            r: bg[0] as f64,
            g: bg[1] as f64,
            b: bg[2] as f64,
            a: 1.,
        }),
        ..*target
    };
    if !post.needs_render(view, target.width, target.height) {
        // With effects disabled there are no intermediate postprocess targets.
        // The shared renderer composes geometry and spatial overlays directly.
        queue.submit([renderer.render_view_slot(view, &target, slot)?]);
        return Ok(());
    }
    let mut commands = Vec::with_capacity(3);
    post.ensure_size(device, target.width, target.height)?;
    let mut scene_target = post.target();
    scene_target.color_load = target.color_load;
    commands.push(renderer.render_view_slot(view, &scene_target, slot)?);
    let (s, d) = renderer.shadow_maps();
    post.set_shadow_maps(device, s, d);
    let (position, color) = renderer
        .scene
        .lights()
        .values()
        .next()
        .map(|l| (l.position, l.color))
        .unwrap_or((Vec3::ZERO, Vec3::ZERO));
    let depth = post.target().depth.clone();
    let (passes, command, final_draw) = post.render(
        device,
        queue,
        view,
        position,
        color,
        &target,
        renderer.shadows_enabled(),
        slot != 0,
        slot,
    );
    renderer.draw_calls += passes;
    if let Some(command) = command {
        commands.push(command);
    }
    if let Some(draw) = final_draw {
        let overlays = RenderTarget {
            depth: &depth,
            auxiliary: None,
            material_mask: false,
            ..target
        };
        commands.push(renderer.draw_postprocess_overlays(&overlays, &draw)?);
    }
    // Submit before preparing the next eye: shared uniforms are queue-owned.
    queue.submit(commands);
    post.record_submission();
    Ok(())
}
