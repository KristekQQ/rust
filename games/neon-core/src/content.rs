//! Replay offline-authored rev50/rev51 content. No JavaScript executes at runtime.
use glam::Vec3;
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};
use webgpu_wasm::{
    blade_physics::BladePhysics,
    cutting::CutMesh,
    gltf_loader,
    gltf_model::ModelAsset,
    scene::{RenderOptions, SceneManager},
};

pub struct Content {
    /// Authored scene handles recursively remapped to native scene IDs.
    pub refs: Value,
    /// Full manifest; `runtime` contains catalog, constants and launch recipes.
    pub manifest: Value,
    /// RenderState owns combo indicators; register these after loading the scene.
    pub indicator_bindings: Vec<(u32, String, u32)>,
}
impl Content {
    /// `read` resolves paths relative to native/neon/assets (or APK neon root).
    pub fn load(
        scene: &mut SceneManager,
        read: &dyn Fn(&str) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let mut manifest: Value =
            serde_json::from_slice(&read("scene.json")?).map_err(|e| e.to_string())?;
        if manifest["version"] != 1
            || !matches!(
                manifest["revision"].as_str(),
                Some("cynatar-arena-50" | "cynatar-arena-51")
            )
        {
            return Err("Unsupported Neon content revision".into());
        }
        let mut ids = BTreeMap::new();
        let mut indicator_bindings = Vec::new();
        let operations = manifest["operations"]
            .as_array()
            .ok_or("Missing content operations")?;
        for (index, operation) in operations.iter().enumerate() {
            replay(scene, read, operation, &mut ids, &mut indicator_bindings)
                .map_err(|e| format!("Neon operation {index} {}: {e}", operation["op"]))?;
        }
        remap(&mut manifest, &ids)?;
        Ok(Self {
            refs: manifest["refs"].clone(),
            manifest,
            indicator_bindings,
        })
    }
}
fn replay(
    scene: &mut SceneManager,
    read: &dyn Fn(&str) -> Result<Vec<u8>, String>,
    operation: &Value,
    ids: &mut BTreeMap<String, u32>,
    indicators: &mut Vec<(u32, String, u32)>,
) -> Result<(), String> {
    let op = operation["op"].as_str().ok_or("Missing operation name")?;
    let mut args = operation["args"].clone();
    remap(&mut args, ids)?;
    let a = args.as_array().ok_or("Expected operation arguments")?;
    let p = |n: usize| a.get(n).unwrap_or(&Value::Null);
    let mut created = None;
    // Create calls use the unbound authored ID; mutations resolve an existing ID.
    let id = if op.starts_with("create") || op.starts_with("load") || op == "clear" {
        0
    } else {
        resolve(&operation["id"], ids)?
    };
    let accepted = match op {
        "clear" => {
            scene.clear();
            true
        }
        "createMesh" => {
            created = Some(scene.add_triangle_mesh(mesh(p(0))?)?);
            true
        }
        "createMeshVariants" => {
            let g = p(0);
            let positions: Vec<f32> = decode(&g["positions"])?;
            let indices: Vec<u32> = decode(&g["indices"])?;
            let variants: Vec<Vec<f32>> = decode(&g["variants"])?;
            created = Some(scene.register_mesh_variants(
                &positions,
                &indices,
                &variants.concat(),
                g["smooth"].as_bool().unwrap_or(false),
            )?);
            true
        }
        "createVariantInstance" => {
            created = Some(scene.add_mesh_variant(integer(p(0))?, integer(p(1))?)?);
            true
        }
        "createTextPanel" => {
            let g = p(0);
            let lines: Vec<String> = decode(&g["lines"])?;
            let object = scene.add_text_panel(
                string(&g["title"])?,
                &lines,
                number(&g["width"])?,
                number(&g["height"])?,
            )?;
            if !scene.set_object_position(object, vec3(&g["position"])?) {
                return Err("Panel placement rejected".into());
            }
            created = Some(object);
            true
        }
        "createInfoPanel" => {
            let g = p(1);
            let object =
                scene.add_info_panel(string(p(0))?, number(&g["width"])?, number(&g["height"])?)?;
            if !scene.set_object_position(object, vec3(&g["position"])?) {
                return Err("Panel placement rejected".into());
            }
            created = Some(object);
            true
        }
        "createLight" => {
            let g = p(0);
            created = Some(
                scene
                    .add_light(vec3(&g["position"])?, vec3(&g["color"])?)
                    .ok_or("Light budget exceeded")?,
            );
            true
        }
        "loadFont3D" => {
            let bytes = read(string(p(0))?)?;
            created = Some(
                scene.register_font3d(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?)?,
            );
            true
        }
        "loadModel" => {
            created = Some(scene.add_model(import(string(p(0))?, read)?)?);
            true
        }
        "setPosition" => scene.set_object_position(id, vec3(p(0))?),
        "setRotation" => scene.set_object_rotation(id, vec3(p(0))?),
        "setScale" => scene.set_object_scale(id, vec3(p(0))?),
        "setEmission" => scene.set_emission(id, number(p(0))?),
        "setColorCutout" => scene.set_color_cutout(id, number(p(0))?),
        "setOpacity" => scene.set_opacity(
            id,
            number(p(0))?,
            optional_number(p(1), "opaqueAbove", 8.),
            optional_number(p(1), "variation", 0.),
            optional_number(p(1), "coverage", -1.),
        ),
        "setRenderOptions" => {
            let g = p(0);
            scene.set_render_options(
                id,
                RenderOptions {
                    is_static: g["isStatic"].as_bool().unwrap_or(false),
                    casts_shadow: g["castsShadow"].as_bool().unwrap_or(true),
                    receives_shadow: g["receivesShadow"].as_bool().unwrap_or(true),
                    reflectivity: optional_number(g, "reflectivity", 0.),
                    opaque_reorderable: g["opaqueReorderable"].as_bool().unwrap_or(false),
                },
            )
        }
        "spin" => scene.set_object_spin(id, vec3(p(0))?),
        "setCutter" => {
            scene.set_cutter(
                id,
                if p(0).is_null() {
                    None
                } else {
                    Some([vec3(&p(0)["base"])?, vec3(&p(0)["tip"])?])
                },
            )?;
            true
        }
        "setBladePhysics" => {
            let g = p(0);
            scene.set_blade_physics(
                id,
                if g.is_null() {
                    None
                } else {
                    Some(BladePhysics {
                        edge: vec3(&g["edge"])?,
                        half_width: optional_number(g, "halfWidth", 0.0225),
                        half_thickness: optional_number(g, "halfThickness", 0.004),
                    })
                },
            )?;
            true
        }
        "setCutAssist" => {
            scene.set_cut_assist(id, number(p(0))?)?;
            true
        }
        "setBladeContactResponse" => {
            scene.set_blade_contact_response(id, boolean(p(0))?)?;
            true
        }
        "setBladeTrail" => {
            scene.set_blade_trail(id, boolean(p(0))?)?;
            true
        }
        "setFont3D" => {
            scene.bind_info_panel_font3d(
                id,
                if p(0).is_null() {
                    u32::MAX
                } else {
                    integer(p(0))?
                },
                &p(1).to_string(),
            )?;
            true
        }
        "bindComboIndicator" => {
            indicators.push((id, string(p(0))?.into(), integer(p(1))?));
            true
        }
        "remove" => scene.remove_object(id),
        _ => return Err(format!("Unsupported authored operation {op}")),
    };
    if !accepted {
        return Err(format!("Scene rejected {op} for object {id}"));
    }
    if let Some(id) = created {
        if id == u32::MAX {
            return Err("Scene ID budget exhausted".into());
        }
        ids.insert(tag(&operation["id"]).ok_or("Missing created handle")?, id);
    }
    Ok(())
}
fn mesh(g: &Value) -> Result<CutMesh, String> {
    let positions: Vec<f32> = decode(&g["positions"])?;
    let indices: Vec<u32> = decode(&g["indices"])?;
    if !positions.len().is_multiple_of(3) {
        return Err("Invalid positions".into());
    }
    let color = if g["color"].is_null() {
        Vec3::new(0.18, 0.65, 0.85)
    } else {
        vec3(&g["color"])?
    };
    let mut mesh = CutMesh::new(
        positions.chunks_exact(3).map(Vec3::from_slice).collect(),
        &indices,
        color,
    )?;
    if !g["triangleColors"].is_null() {
        let colors: Vec<f32> = decode(&g["triangleColors"])?;
        if colors.len() != indices.len() {
            return Err("Invalid triangle colors".into());
        }
        for (triangle, color) in mesh.triangles.iter_mut().zip(colors.chunks_exact(3)) {
            triangle.color = Vec3::from_slice(color);
        }
    }
    if g["smooth"].as_bool().unwrap_or(false) {
        mesh.smooth_normals();
    }
    Ok(mesh)
}
fn decode<T: serde::de::DeserializeOwned>(v: &Value) -> Result<T, String> {
    serde_json::from_value(v.clone()).map_err(|e| e.to_string())
}
fn number(v: &Value) -> Result<f32, String> {
    let n = v.as_f64().ok_or("Expected number")? as f32;
    if n.is_finite() {
        Ok(n)
    } else {
        Err("Non-finite number".into())
    }
}
fn optional_number(v: &Value, key: &str, default: f32) -> f32 {
    v[key].as_f64().map_or(default, |n| n as f32)
}
fn integer(v: &Value) -> Result<u32, String> {
    v.as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| "Expected ID/index".into())
}
fn string(v: &Value) -> Result<&str, String> {
    v.as_str().ok_or_else(|| "Expected string".into())
}
fn boolean(v: &Value) -> Result<bool, String> {
    v.as_bool().ok_or_else(|| "Expected boolean".into())
}
fn vec3(v: &Value) -> Result<Vec3, String> {
    let a: [f32; 3] = decode(v)?;
    let p = Vec3::from_array(a);
    if p.is_finite() {
        Ok(p)
    } else {
        Err("Non-finite vector".into())
    }
}
fn tag(v: &Value) -> Option<String> {
    let o = v.as_object()?;
    if o.len() != 1 {
        return None;
    }
    for kind in ["$object", "$asset", "$font"] {
        if let Some(id) = o.get(kind).and_then(Value::as_u64) {
            return Some(format!("{kind}:{id}"));
        }
    }
    None
}
fn resolve(v: &Value, ids: &BTreeMap<String, u32>) -> Result<u32, String> {
    let key = tag(v).ok_or("Expected authored handle")?;
    ids.get(&key)
        .copied()
        .ok_or_else(|| format!("Unresolved handle {key}"))
}
fn remap(v: &mut Value, ids: &BTreeMap<String, u32>) -> Result<(), String> {
    if tag(v).is_some() {
        *v = Value::from(resolve(v, ids)?);
        return Ok(());
    }
    match v {
        Value::Array(a) => {
            for item in a {
                remap(item, ids)?
            }
        }
        Value::Object(o) => {
            for item in o.values_mut() {
                remap(item, ids)?
            }
        }
        _ => {}
    }
    Ok(())
}
fn import(
    file: &str,
    read: &dyn Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<Arc<ModelAsset>, String> {
    let bytes = read(file)?;
    let resources = gltf_loader::resource_uris(&bytes)?;
    let directory = file.rsplit_once('/').map_or("", |(d, _)| d);
    let resolve = |uri: &String| -> Result<Vec<u8>, String> {
        if uri.is_empty() {
            Ok(Vec::new())
        } else {
            read(&format!("{directory}/{uri}"))
        }
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
    gltf_loader::parse_model(&bytes, &buffers, &images).map(Arc::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_revision51_loads_models_fonts_and_catalog() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../native/neon/assets");
        let mut scene = SceneManager::default();
        let content = Content::load(&mut scene, &|name| {
            std::fs::read(root.join(name)).map_err(|e| e.to_string())
        })
        .unwrap();
        assert_eq!(content.manifest["revision"], "cynatar-arena-51");
        assert_eq!(scene.models().len(), 1);
        assert_eq!(
            content.manifest["runtime"]["catalog"]
                .as_array()
                .unwrap()
                .len(),
            10
        );
        assert_eq!(content.indicator_bindings.len(), 60);
        assert!(scene.object_exists(content.refs["sword"].as_u64().unwrap() as u32));
    }

    #[test]
    fn supported_revisions_require_version1() {
        for revision in ["cynatar-arena-50", "cynatar-arena-51"] {
            let manifest = serde_json::to_vec(&serde_json::json!({
                "version": 1, "revision": revision, "operations": [], "refs": {}
            }))
            .unwrap();
            assert!(Content::load(&mut SceneManager::default(), &|_| Ok(manifest.clone())).is_ok());
        }
    }

    #[test]
    fn unsupported_revision_or_version_is_rejected_before_replay() {
        for (version, revision) in [
            (1, "cynatar-arena-49"),
            (1, "cynatar-arena-52"),
            (0, "cynatar-arena-50"),
            (2, "cynatar-arena-51"),
        ] {
            let manifest = serde_json::to_vec(&serde_json::json!({
                "version": version, "revision": revision,
                "operations": [{"op": "unsupported"}], "refs": {}
            }))
            .unwrap();
            let error = Content::load(&mut SceneManager::default(), &|_| Ok(manifest.clone()))
                .err()
                .expect("unsupported manifest must fail");
            assert_eq!(error, "Unsupported Neon content revision");
        }
    }
}
