//! Deterministic real Rust cut/serialization fixture for observer acceptance.
use glam::Vec3;
use webgpu_wasm::scene::{wire::WirePublisher, SceneManager};
fn main() -> Result<(), String> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let mut scene = SceneManager::default();
    let p = [0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0., 1.];
    let i = [0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3];
    let handle = scene.register_mesh_variants(
        &p,
        &i,
        &[0.9, 0.2, 0.1, 0.2, 0.8, 0.1, 0.2, 0.2, 0.9, 0.9, 0.7, 0.1],
        true,
    )?;
    let target = scene.add_mesh_variant(handle, 0)?;
    scene.set_cuttable(target, true, Vec3::ONE)?;
    let cutter = scene.add_cube(Vec3::ZERO);
    scene.set_cutter(
        cutter,
        Some([Vec3::new(0.2, -1., -1.), Vec3::new(0.2, 2., -1.)]),
    )?;
    let mut publisher = WirePublisher::default();
    std::fs::write(out.join("initial.nwr"), publisher.capture(&mut scene)?)
        .map_err(|e| e.to_string())?;
    // Separate sequence for display interpolation without changing the cut stream.
    let mut motion = WirePublisher::default();
    let _ = motion.capture(&mut scene)?;
    assert!(scene.set_object_position(target, Vec3::new(1.2, 0., 0.)));
    std::fs::write(out.join("motion.nwr"), motion.capture(&mut scene)?)
        .map_err(|e| e.to_string())?;
    assert!(scene.set_object_position(target, Vec3::ZERO));
    let result = scene.cut_with(cutter, Vec3::new(0., 0., 3.), 0.)?;
    if result.pieces.len() != 2 {
        return Err("expected two physical cut halves".into());
    }
    for (index, id) in result.pieces.iter().enumerate() {
        assert!(scene.set_object_position(*id, Vec3::new(index as f32 * 1.2, 0., 0.)));
    }
    std::fs::write(out.join("cut.nwr"), publisher.capture(&mut scene)?)
        .map_err(|e| e.to_string())?;
    for id in &result.pieces {
        assert!(scene.remove_object(*id));
    }
    std::fs::write(out.join("removed.nwr"), publisher.capture(&mut scene)?)
        .map_err(|e| e.to_string())?;
    std::fs::write(
        out.join("ids.json"),
        serde_json::json!({"source":target,"model":handle,"halves":result.pieces}).to_string(),
    )
    .map_err(|e| e.to_string())?;
    println!(
        "Rust scene fixture: source {} -> {:?} -> removed",
        target, result.pieces
    );
    Ok(())
}
