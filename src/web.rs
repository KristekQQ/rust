#![cfg(target_arch = "wasm32")]

use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{closure::Closure, JsCast};

use crate::input::active_camera::{ActiveCamera, CameraType};
use crate::input::camera::CameraController;
use crate::input::{keyboard, mouse};
use crate::render::state::State;

type ImportedResources = (Vec<Vec<u8>>, Vec<Vec<u8>>);
type AnimationFrameCallback = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;

thread_local! {
    static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
    static STATE: RefCell<Option<Rc<RefCell<State>>>> = const { RefCell::new(None) };
    static STARTING: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
    static GENERATION: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static RIM: RefCell<Option<crate::rim_game::Game>> = const { RefCell::new(None) };
    static RIM_REAL_DT: std::cell::Cell<f32> = const { std::cell::Cell::new(0.) };
    static XR_PLAYER: RefCell<crate::xr_player::XrPlayer> = RefCell::new(Default::default());
    static RESET_FRAME_CLOCK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static XR_ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static CAMERA: RefCell<Option<Rc<RefCell<ActiveCamera>>>> = const { RefCell::new(None) };
}

struct Runtime {
    window: web_sys::Window,
    frame: Rc<std::cell::Cell<Option<i32>>>,
    _callback: AnimationFrameCallback,
    _listeners: Vec<keyboard::Listener>,
}
impl Drop for Runtime {
    fn drop(&mut self) {
        if let Some(id) = self.frame.take() {
            let _ = self.window.cancel_animation_frame(id);
        }
        self._callback.borrow_mut().take();
    }
}

/// Release the frame loop, input listeners, camera and GPU scene resources.
#[wasm_bindgen]
pub fn dispose() {
    RIM.with(|slot| slot.borrow_mut().take());
    crate::render::profiler::reset();
    XR_ACTIVE.with(|active| active.set(false));
    XR_PLAYER.with(|player| *player.borrow_mut() = Default::default());
    GENERATION.with(|generation| generation.set(generation.get().wrapping_add(1)));
    STARTING.with(|starting| starting.set(None));
    RUNTIME.with(|runtime| runtime.borrow_mut().take());
    CAMERA.with(|camera| camera.borrow_mut().take());
    STATE.with(|state| state.borrow_mut().take());
}

#[wasm_bindgen]
pub fn gltf_buffer_uris(bytes: &[u8]) -> Result<js_sys::Array, JsValue> {
    let uris = crate::gltf_loader::buffer_uris(bytes).map_err(|e| JsValue::from_str(&e))?;
    Ok(uris.iter().map(|uri| JsValue::from_str(uri)).collect())
}
#[wasm_bindgen]
pub fn load_gltf(bytes: &[u8], buffers: js_sys::Array) -> Result<u32, JsValue> {
    let mut external = Vec::new();
    let mut total_bytes = bytes.len();
    if total_bytes > 128 * 1024 * 1024 || buffers.length() > 1024 {
        return Err(JsValue::from_str(
            "glTF překračuje limit velikosti nebo počtu bufferů",
        ));
    }
    for value in buffers.iter() {
        if !value.is_instance_of::<js_sys::Uint8Array>() {
            return Err(JsValue::from_str("glTF buffers musí být Uint8Array"));
        }
        let buffer = value.unchecked_into::<js_sys::Uint8Array>();
        total_bytes = total_bytes.saturating_add(buffer.length() as usize);
        if total_bytes > 128 * 1024 * 1024 {
            return Err(JsValue::from_str("glTF překračuje limit 128 MiB"));
        }
        external.push(buffer.to_vec());
    }
    let model = crate::gltf_loader::parse(bytes, &external).map_err(|e| JsValue::from_str(&e))?;
    for warning in &model.warnings {
        web_sys::console::warn_1(&JsValue::from_str(warning));
    }
    STATE.with(|state| {
        let state = state.borrow();
        let engine = state
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine není inicializován"))?;
        let result = engine
            .borrow_mut()
            .import_model(&model)
            .map_err(|e| JsValue::from_str(&e));
        result
    })
}

#[wasm_bindgen]
pub fn gltf_resource_uris(bytes: &[u8]) -> Result<String, JsValue> {
    let resources = crate::gltf_loader::resource_uris(bytes).map_err(|e| JsValue::from_str(&e))?;
    Ok(serde_json::json!({"buffers":resources.buffers,"images":resources.images}).to_string())
}

fn model_resources(
    bytes: &[u8],
    buffers: js_sys::Array,
    images: js_sys::Array,
) -> Result<ImportedResources, JsValue> {
    let mut total = bytes.len();
    if total > 128 * 1024 * 1024 || buffers.length() + images.length() > 1024 {
        return Err(JsValue::from_str("glTF překračuje limity zdrojů"));
    }
    let mut convert = |array: js_sys::Array| -> Result<Vec<Vec<u8>>, JsValue> {
        array
            .iter()
            .map(|value| {
                if !value.is_instance_of::<js_sys::Uint8Array>() {
                    return Err(JsValue::from_str("glTF zdroje musí být Uint8Array"));
                }
                let value = value.unchecked_into::<js_sys::Uint8Array>();
                total = total.saturating_add(value.length() as usize);
                if total > 128 * 1024 * 1024 {
                    return Err(JsValue::from_str("glTF překračuje 128 MiB"));
                }
                Ok(value.to_vec())
            })
            .collect()
    };
    Ok((convert(buffers)?, convert(images)?))
}
#[wasm_bindgen]
pub fn load_gltf_model(
    bytes: &[u8],
    buffers: js_sys::Array,
    images: js_sys::Array,
) -> Result<u32, JsValue> {
    let (buffers, images) = model_resources(bytes, buffers, images)?;
    let asset = crate::gltf_loader::parse_model(bytes, &buffers, &images)
        .map_err(|e| JsValue::from_str(&e))?;
    for warning in &asset.warnings {
        web_sys::console::warn_1(&JsValue::from_str(warning));
    }
    STATE.with(|s| {
        let s = s.borrow();
        let engine = s
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine není inicializován"))?;
        let result = engine
            .borrow_mut()
            .import_gltf_model(std::sync::Arc::new(asset))
            .map_err(|e| JsValue::from_str(&e));
        result
    })
}
#[wasm_bindgen]
pub fn model_info(id: u32) -> Result<String, JsValue> {
    STATE.with(|s| {
        let state=s.borrow();
        let engine=state.as_ref().ok_or_else(|| JsValue::from_str("Engine není inicializován"))?;
        let engine=engine.borrow();
        let model=engine.scene.model(id).ok_or_else(|| JsValue::from_str("Model neexistuje"))?;
        let asset=&model.asset;
        let animations:Vec<_>=asset.animations.iter().enumerate().map(|(i,a)| serde_json::json!({"name":a.name.clone().unwrap_or_else(||format!("Animation {i}")),"duration":a.duration})).collect();
        let nodes:Vec<_>=asset.nodes.iter().enumerate().map(|(i,n)| serde_json::json!({"name":n.name.clone().unwrap_or_else(||format!("Node {i}")),"mesh":n.mesh,"camera":n.camera,"morphTargets":n.mesh.and_then(|m|asset.meshes[m].primitives.first()).map(|p|asset.primitives[*p].morph_targets.len()).unwrap_or(0)})).collect();
        let scenes:Vec<_>=asset.scenes.iter().enumerate().map(|(i,s)|serde_json::json!({"name":s.name.clone().unwrap_or_else(||format!("Scene {i}"))})).collect();
        let playback=model.playback.as_ref().map(|p|serde_json::json!({"clip":p.clip,"time":p.time,"speed":p.speed,"looping":p.looping,"playing":p.playing}));
        Ok(serde_json::json!({"animations":animations,"nodes":nodes,"scenes":scenes,"scene":model.scene,"playback":playback,"primitives":asset.primitives.len(),"materials":asset.materials.len(),"textures":asset.textures.len(),"skins":asset.skins.len(),"lastError":model.last_error}).to_string())
    })
}
#[wasm_bindgen]
pub fn play_model_animation(id: u32, clip: usize, looping: bool, speed: f32) -> bool {
    STATE.with(|s| {
        s.borrow().as_ref().is_some_and(|s| {
            s.borrow_mut()
                .scene
                .play_model_animation(id, clip, looping, speed)
                .is_ok()
        })
    })
}
#[wasm_bindgen]
pub fn pause_model_animation(id: u32, paused: bool) -> bool {
    STATE.with(|s| {
        s.borrow().as_ref().is_some_and(|s| {
            s.borrow_mut()
                .scene
                .pause_model_animation(id, paused)
                .is_ok()
        })
    })
}
#[wasm_bindgen]
pub fn seek_model_animation(id: u32, time: f32) -> bool {
    STATE.with(|s| {
        s.borrow()
            .as_ref()
            .is_some_and(|s| s.borrow_mut().scene.seek_model_animation(id, time).is_ok())
    })
}
#[wasm_bindgen]
pub fn stop_model_animation(id: u32) -> bool {
    STATE.with(|s| {
        s.borrow()
            .as_ref()
            .is_some_and(|s| s.borrow_mut().scene.stop_model_animation(id).is_ok())
    })
}
#[wasm_bindgen]
pub fn select_model_scene(id: u32, scene: usize) -> bool {
    STATE.with(|s| {
        s.borrow()
            .as_ref()
            .is_some_and(|s| s.borrow_mut().scene.select_model_scene(id, scene).is_ok())
    })
}
#[wasm_bindgen]
pub fn set_model_morph_weights(id: u32, node: usize, weights: &[f32]) -> bool {
    STATE.with(|s| {
        s.borrow().as_ref().is_some_and(|s| {
            s.borrow_mut()
                .scene
                .set_model_morph_weights(id, node, weights)
                .is_ok()
        })
    })
}
#[wasm_bindgen]
pub fn use_model_camera(id: u32, node: usize) -> bool {
    STATE.with(|s| {
        s.borrow()
            .as_ref()
            .is_some_and(|s| s.borrow_mut().use_model_camera(id, node))
    })
}

#[wasm_bindgen]
pub fn configure_god_rays(density: f32, exposure: f32) -> bool {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .is_some_and(|s| s.borrow_mut().configure_god_rays(density, exposure))
    })
}

#[wasm_bindgen]
pub fn set_postprocess(name: &str, enabled: bool, strength: f32) -> bool {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .is_some_and(|s| s.borrow_mut().set_postprocess(name, enabled, strength))
    })
}
#[wasm_bindgen]
pub fn set_reflection_mask(
    width: u32,
    height: u32,
    pixels: &[u8],
    x: f32,
    z: f32,
    size_x: f32,
    size_z: f32,
) -> bool {
    STATE.with(|state| {
        state.borrow().as_ref().is_some_and(|s| {
            s.borrow_mut().set_reflection_mask(
                width,
                height,
                pixels,
                glam::Vec2::new(x, z),
                glam::Vec2::new(size_x, size_z),
            )
        })
    })
}
#[wasm_bindgen]
pub fn postprocess_stats() -> Vec<f64> {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|s| s.borrow().postprocess_stats().to_vec())
            .unwrap_or_default()
    })
}

/// Maintained GPU acceptance probe: execute a compiled storage-buffer graph.
#[wasm_bindgen]
pub async fn verify_frame_graph_resources() -> Result<String, JsValue> {
    let (device, queue) = STATE.with(|slot| {
        let state = slot.borrow();
        let state = state
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine unavailable"))?
            .borrow();
        Ok::<_, JsValue>(state.graph_probe_handles())
    })?;
    crate::render::graph_probe::run(device, queue)
        .await
        .map(|report| report.to_string())
        .map_err(|e| JsValue::from_str(&e))
}

/// Read-only native frame profiling; GPU data is asynchronous and may be absent.
#[wasm_bindgen]
pub fn frame_profile_stats() -> String {
    crate::render::profiler::stats().to_string()
}
#[wasm_bindgen]
pub fn postprocess_graph() -> String {
    STATE.with(|slot| {
        slot.borrow().as_ref().map_or_else(
            || "null".into(),
            |s| s.borrow().postprocess_graph().to_string(),
        )
    })
}

#[wasm_bindgen]
pub fn configure_ssr(resolution_scale: f32) -> bool {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .is_some_and(|s| s.borrow_mut().configure_ssr(resolution_scale))
    })
}

#[wasm_bindgen]
pub fn ssr_stats() -> Vec<f64> {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|s| s.borrow().ssr_stats().to_vec())
            .unwrap_or_default()
    })
}

#[wasm_bindgen]
pub fn configure_ssr_region(bounds: &[f32]) -> bool {
    if !bounds.is_empty() && bounds.len() != 6 {
        return false;
    }
    let (min, max, enabled) = if bounds.len() == 6 {
        (
            glam::Vec3::from_slice(&bounds[..3]),
            glam::Vec3::from_slice(&bounds[3..]),
            true,
        )
    } else {
        (glam::Vec3::ZERO, glam::Vec3::ZERO, false)
    };
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .is_some_and(|s| s.borrow_mut().configure_ssr_region(min, max, enabled))
    })
}

#[wasm_bindgen]
pub fn ssr_work_stats() -> Vec<f64> {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|s| s.borrow().ssr_work_stats().to_vec())
            .unwrap_or_default()
    })
}

#[wasm_bindgen]
pub fn set_render_effects(shadows: bool, reflections: bool) {
    STATE.with(|state| {
        if let Some(state) = state.borrow().as_ref() {
            state.borrow_mut().set_effects(shadows, reflections);
        }
    });
}
#[wasm_bindgen]
pub fn set_simulation_paused(paused: bool) {
    STATE.with(|state| {
        if let Some(state) = state.borrow().as_ref() {
            state.borrow_mut().scene.paused = paused;
        }
    });
}
#[wasm_bindgen]
pub fn effects_stats() -> js_sys::Float64Array {
    let values = STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|state| state.borrow().effects_stats())
            .unwrap_or([0.0; 19])
    });
    js_sys::Float64Array::from(values.as_slice())
}
#[wasm_bindgen]
pub fn set_object_emission(id: u32, intensity: f32) -> bool {
    STATE.with(|s| {
        s.borrow()
            .as_ref()
            .is_some_and(|s| s.borrow_mut().scene.set_emission(id, intensity))
    })
}
#[wasm_bindgen]
pub fn set_object_opacity(
    id: u32,
    opacity: f32,
    opaque_above: f32,
    variation: f32,
    coverage: f32,
) -> bool {
    STATE.with(|s| {
        s.borrow().as_ref().is_some_and(|s| {
            s.borrow_mut()
                .scene
                .set_opacity(id, opacity, opaque_above, variation, coverage)
        })
    })
}
#[wasm_bindgen]
pub fn set_object_color_cutout(id: u32, threshold: f32) -> bool {
    STATE.with(|s| {
        s.borrow()
            .as_ref()
            .is_some_and(|s| s.borrow_mut().scene.set_color_cutout(id, threshold))
    })
}
/// Optional additive geometry glow; zero disables and releases its GPU cache.
#[wasm_bindgen]
pub fn set_neon_glow(strength: f32, radius: f32) -> bool {
    STATE.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|state| state.borrow_mut().configure_neon_glow(strength, radius))
    })
}
#[wasm_bindgen]
pub fn set_scene_appearance(input: &str) -> Result<(), JsValue> {
    let config: Option<crate::render_appearance::Config> =
        serde_json::from_str(input).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let appearance = config
        .map(crate::render_appearance::Appearance::from_config)
        .transpose()
        .map_err(|e| JsValue::from_str(&e))?
        .unwrap_or_default();
    STATE.with(|slot| {
        let state = slot.borrow();
        let state = state
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine not initialized"))?;
        state.borrow_mut().set_appearance(appearance);
        Ok(())
    })
}
#[wasm_bindgen]
pub fn set_background_color(r: f32, g: f32, b: f32) -> bool {
    STATE.with(|s| {
        s.borrow()
            .as_ref()
            .is_some_and(|s| s.borrow_mut().set_background(glam::Vec3::new(r, g, b)))
    })
}
#[wasm_bindgen]
pub fn set_environment_cube_map(
    size: u32,
    color: &[u8],
    emission: &[u8],
    intensity: f32,
    projection: &[f32],
) -> bool {
    STATE.with(|s| {
        s.borrow().as_ref().is_some_and(|s| {
            s.borrow_mut()
                .set_environment_cube_map(size, color, emission, intensity, projection)
        })
    })
}
#[wasm_bindgen]
pub fn clear_environment_cube_map() -> bool {
    STATE.with(|s| {
        s.borrow().as_ref().is_some_and(|s| {
            s.borrow_mut().clear_environment_cube_map();
            true
        })
    })
}
#[wasm_bindgen]
pub fn set_environment_intensity(intensity: f32) -> bool {
    STATE.with(|s| {
        s.borrow()
            .as_ref()
            .is_some_and(|s| s.borrow_mut().set_environment_intensity(intensity))
    })
}
#[wasm_bindgen]
pub fn configure_environment(
    enabled: bool,
    x: f32,
    y: f32,
    z: f32,
    hx: f32,
    hy: f32,
    hz: f32,
) -> bool {
    STATE.with(|s| {
        s.borrow().as_ref().is_some_and(|s| {
            s.borrow_mut().configure_environment(
                enabled,
                glam::Vec3::new(x, y, z),
                glam::Vec3::new(hx, hy, hz),
            )
        })
    })
}
#[wasm_bindgen]
pub fn environment_captures() -> u32 {
    STATE.with(|s| {
        s.borrow()
            .as_ref()
            .map_or(0, |s| s.borrow().environment_captures())
    })
}
#[wasm_bindgen]
pub fn set_object_render_options(
    id: u32,
    is_static: bool,
    casts_shadow: bool,
    receives_shadow: bool,
    reflectivity: f32,
) -> bool {
    STATE.with(|state| {
        state.borrow().as_ref().is_some_and(|state| {
            state.borrow_mut().scene.set_render_options(
                id,
                crate::scene::RenderOptions {
                    is_static,
                    casts_shadow,
                    receives_shadow,
                    reflectivity,
                },
            )
        })
    })
}
#[wasm_bindgen]
pub fn configure_planar_mirror(id: u32) -> bool {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .is_some_and(|state| state.borrow_mut().scene.configure_mirror(id))
    })
}
#[wasm_bindgen]
pub fn set_object_spin(id: u32, x: f32, y: f32, z: f32) -> bool {
    STATE.with(|state| {
        state.borrow().as_ref().is_some_and(|state| {
            state
                .borrow_mut()
                .scene
                .set_object_spin(id, glam::Vec3::new(x, y, z))
        })
    })
}
#[wasm_bindgen]
pub fn set_projectile(id: u32, vx: f32, vy: f32, vz: f32, gravity: f32, bounce: f32) -> bool {
    STATE.with(|state| {
        state.borrow().as_ref().is_some_and(|state| {
            state.borrow_mut().scene.set_projectile(
                id,
                glam::Vec3::new(vx, vy, vz),
                gravity,
                bounce,
            )
        })
    })
}

#[wasm_bindgen]
pub fn projectile_contacts() -> u32 {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map_or(0, |state| state.borrow().scene.projectile_contacts)
    })
}

/// Select browser presentation; scene rendering itself accepts GPU attachments.
#[wasm_bindgen]
pub fn set_output_mode(mode: &str) -> bool {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .is_some_and(|state| state.borrow_mut().set_output_mode(mode))
    })
}

#[wasm_bindgen]
pub fn set_frustum_culling(enabled: bool) {
    STATE.with(|state| {
        if let Some(state) = state.borrow().as_ref() {
            state.borrow_mut().frustum_culling = enabled;
        }
    });
}

#[wasm_bindgen]
pub fn renderer_stats() -> js_sys::Float64Array {
    let values = STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|state| state.borrow().render_stats())
            .unwrap_or([0.0; 8])
    });
    js_sys::Float64Array::from(values.as_slice())
}

#[wasm_bindgen]
pub fn set_debug_overlay(enabled: bool) {
    STATE.with(|state| {
        if let Some(state) = state.borrow().as_ref() {
            state.borrow_mut().set_debug_overlay(enabled);
        }
    });
}

#[wasm_bindgen]
pub fn debug_stats() -> String {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|state| state.borrow().debug_stats())
            .unwrap_or_else(|| "{}".into())
    })
}

#[wasm_bindgen]
pub fn renderer_backend() -> String {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|state| state.borrow().backend.clone())
            .unwrap_or_default()
    })
}

#[wasm_bindgen]
pub fn clear_all() {
    STATE.with(|state| {
        if let Some(state) = state.borrow().as_ref() {
            state.borrow_mut().scene.clear();
        }
    });
}

#[wasm_bindgen]
pub fn scene_object_count() -> u32 {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|state| state.borrow().scene.objects().len() as u32)
            .unwrap_or(0)
    })
}

#[wasm_bindgen]
pub fn scene_light_count() -> u32 {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .map(|state| state.borrow().scene.lights().len() as u32)
            .unwrap_or(0)
    })
}

#[wasm_bindgen]
pub fn object_exists(id: u32) -> bool {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .is_some_and(|state| state.borrow().scene.object_exists(id))
    })
}

#[wasm_bindgen]
pub fn light_exists(id: u32) -> bool {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .is_some_and(|state| state.borrow().scene.light_exists(id))
    })
}

#[wasm_bindgen]
pub fn set_object_position(id: u32, x: f32, y: f32, z: f32) -> bool {
    STATE.with(|state| {
        state.borrow().as_ref().is_some_and(|state| {
            state
                .borrow_mut()
                .scene
                .set_object_position(id, glam::Vec3::new(x, y, z))
        })
    })
}

#[wasm_bindgen]
pub fn set_object_rotation(id: u32, x: f32, y: f32, z: f32) -> bool {
    STATE.with(|state| {
        state.borrow().as_ref().is_some_and(|state| {
            state
                .borrow_mut()
                .scene
                .set_object_rotation(id, glam::Vec3::new(x, y, z))
        })
    })
}

#[wasm_bindgen]
pub fn set_object_scale(id: u32, x: f32, y: f32, z: f32) -> bool {
    STATE.with(|state| {
        state.borrow().as_ref().is_some_and(|state| {
            state
                .borrow_mut()
                .scene
                .set_object_scale(id, glam::Vec3::new(x, y, z))
        })
    })
}

#[wasm_bindgen]
pub fn set_light_helpers_visible(show: bool) {
    STATE.with(|state| {
        if let Some(state) = state.borrow().as_ref() {
            state.borrow_mut().set_light_helpers_visible(show);
        }
    });
}
#[wasm_bindgen]
pub fn set_grid_visible(show: bool) {
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            st.borrow_mut().set_grid_visible(show);
        }
    });
}

#[wasm_bindgen]
pub fn set_camera_view(px: f32, py: f32, pz: f32, tx: f32, ty: f32, tz: f32) -> bool {
    let accepted = CAMERA.with(|camera| {
        camera.borrow().as_ref().is_some_and(|c| {
            c.borrow_mut()
                .set_view(glam::Vec3::new(px, py, pz), glam::Vec3::new(tx, ty, tz))
        })
    });
    if accepted {
        STATE.with(|s| {
            if let Some(s) = s.borrow().as_ref() {
                s.borrow_mut().clear_model_camera();
            }
        });
    }
    accepted
}

#[wasm_bindgen]
pub fn screen_to_plane(u: f32, v: f32, height: f32) -> Result<Vec<f32>, JsValue> {
    STATE.with(|state| {
        let state = state.borrow();
        let engine = state
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine is not initialized"))?;
        let result = engine
            .borrow()
            .screen_to_plane(u, v, height)
            .map(|p| p.to_array().to_vec())
            .ok_or_else(|| JsValue::from_str("Pointer ray does not intersect plane"));
        result
    })
}

#[wasm_bindgen]
pub fn set_orbit_view(px: f32, py: f32, pz: f32, tx: f32, ty: f32, tz: f32) -> bool {
    let ok = CAMERA.with(|camera| {
        camera.borrow().as_ref().is_some_and(|c| {
            c.borrow_mut()
                .set_orbit_view(glam::Vec3::new(px, py, pz), glam::Vec3::new(tx, ty, tz))
        })
    });
    if ok {
        STATE.with(|state| {
            if let Some(state) = state.borrow().as_ref() {
                state.borrow_mut().clear_model_camera();
            }
        });
    }
    ok
}
#[wasm_bindgen]
pub fn world_to_screen(x: f32, y: f32, z: f32) -> Result<Vec<f32>, JsValue> {
    STATE.with(|state| {
        let state = state.borrow();
        let engine = state
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine unavailable"))?;
        let result = engine
            .borrow()
            .world_to_screen(glam::Vec3::new(x, y, z))
            .map(|p| p.to_vec())
            .unwrap_or_default();
        Ok(result)
    })
}
#[wasm_bindgen]
pub fn adjust_orbit(dx: f32, dy: f32, zoom: f32) -> bool {
    CAMERA.with(|camera| {
        camera
            .borrow()
            .as_ref()
            .is_some_and(|c| c.borrow_mut().adjust_orbit(dx, dy, zoom))
    })
}
#[wasm_bindgen]
pub fn camera_view_state() -> Result<String, JsValue> {
    CAMERA.with(|camera| {
        let camera=camera.borrow();let camera=camera.as_ref().ok_or_else(||JsValue::from_str("Camera unavailable"))?.borrow();
        Ok(serde_json::json!({"position":camera.position().to_array(),"orbitTarget":camera.orbit_target().map(|p|p.to_array())}).to_string())
    })
}

#[wasm_bindgen]
pub fn set_camera_mode(mode: &str) {
    if mode == "free" || mode == "orbit" {
        STATE.with(|s| {
            if let Some(s) = s.borrow().as_ref() {
                s.borrow_mut().clear_model_camera();
            }
        });
    }
    CAMERA.with(|c| {
        if let Some(cam) = &*c.borrow() {
            let mut cam = cam.borrow_mut();
            match mode {
                "free" => cam.set_type(CameraType::Free),
                "orbit" => cam.set_type(CameraType::Orbit),
                _ => {}
            }
        }
    });
}

/// Attach input and a chase camera to an existing glTF root, never spawn a scene.
#[wasm_bindgen]
pub fn control_character(
    id: u32,
    speed: f32,
    run_speed: f32,
    model_yaw: f32,
    idle: i32,
    walk: i32,
    run: i32,
) -> bool {
    STATE.with(|state| {
        let state = state.borrow();
        let Some(state) = state.as_ref() else {
            return false;
        };
        let mut state = state.borrow_mut();
        let Some(model) = state.scene.model(id) else {
            return false;
        };
        let indices = [idle, walk, run];
        if indices.iter().any(|index| {
            *index < -1 || (*index >= 0 && *index as usize >= model.asset.animations.len())
        }) {
            return false;
        }
        let Some(object) = state.scene.objects().get(&id) else {
            return false;
        };
        let position = object.transform.position;
        let clips = indices.map(|index| (index >= 0).then_some(index as usize));
        let accepted = CAMERA.with(|camera| {
            camera.borrow().as_ref().is_some_and(|camera| {
                camera
                    .borrow_mut()
                    .follow_object(id, position, speed, run_speed, model_yaw, clips)
            })
        });
        if accepted {
            state.clear_model_camera();
        }
        accepted
    })
}

#[wasm_bindgen]
pub fn character_state() -> String {
    CAMERA.with(|camera| {
        camera
            .borrow()
            .as_ref()
            .and_then(|camera| {
                let camera = camera.borrow();
                camera.character_state().map(|state| (state, camera.character_physics_state()))
            })
            .map(|((id, position, moving, running), physics)| {
                let (grounded, vertical_velocity, candidates) = physics.unwrap_or((false, 0.0, 0));
                serde_json::json!({
                    "id": id, "position": position.to_array(), "moving": moving, "running": running,
                    "grounded": grounded, "verticalVelocity": vertical_velocity, "collisionCandidates": candidates
                })
                .to_string()
            })
            .unwrap_or_else(|| "null".into())
    })
}

#[wasm_bindgen]
pub fn set_box_collider(id: u32, hx: f32, hy: f32, hz: f32, ox: f32, oy: f32, oz: f32) -> bool {
    STATE.with(|state| {
        state.borrow().as_ref().is_some_and(|state| {
            state.borrow_mut().scene.set_box_collider(
                id,
                glam::Vec3::new(hx, hy, hz),
                glam::Vec3::new(ox, oy, oz),
            )
        })
    })
}

#[wasm_bindgen]
pub fn remove_collider(id: u32) -> bool {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .is_some_and(|state| state.borrow_mut().scene.remove_collider(id))
    })
}

#[wasm_bindgen]
pub fn configure_character_physics(
    hx: f32,
    hy: f32,
    hz: f32,
    gravity: f32,
    jump_speed: f32,
) -> bool {
    let physics = crate::input::character::CharacterPhysics {
        half_extents: glam::Vec3::new(hx, hy, hz),
        gravity,
        jump_speed,
    };
    CAMERA.with(|camera| {
        camera
            .borrow()
            .as_ref()
            .is_some_and(|camera| camera.borrow_mut().set_character_physics(physics))
    })
}

#[wasm_bindgen]
pub fn resize(width: u32, height: u32) {
    if width == 0 || height == 0 {
        return;
    }
    let aspect = STATE.with(|s| {
        s.borrow().as_ref().map(|st| {
            let mut state = st.borrow_mut();
            state.resize(width, height);
            state.aspect
        })
    });
    if let Some(aspect) = aspect {
        CAMERA.with(|c| {
            if let Some(cam) = &*c.borrow() {
                cam.borrow_mut().set_aspect(aspect);
            }
        });
    }
}

#[wasm_bindgen]
pub fn clear_scene() {
    RIM.with(|slot| slot.borrow_mut().take());
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            let mut state = st.borrow_mut();
            state.clear_combo_display();
            state.scene.clear_scene();
            state.combo_score = Default::default();
            state.clear_combo_indicators();
        }
    });
}

/// Create opt-in text geometry in local XY; use ordinary object transforms.
#[wasm_bindgen]
pub fn add_info_panel(kind: &str, width: f32, height: f32) -> Result<u32, JsValue> {
    STATE.with(|slot| {
        let slot = slot.borrow();
        let state = slot
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine is not initialized"))?;
        let result = state.borrow_mut().add_info_panel(kind, width, height);
        result
    })
}

/// Bind an existing native single-value panel; INVALID_ID restores its legacy font.
#[wasm_bindgen]
pub fn bind_info_panel_font3d(panel: u32, font: u32, options_json: &str) -> Result<(), JsValue> {
    cutting_command(|s| s.bind_info_panel_font3d(panel, font, options_json))
}

/// Register a validated closed-mesh glyph library.
#[wasm_bindgen]
pub fn register_font3d(json: &str) -> Result<u32, JsValue> {
    cutting_command(|s| s.register_font3d(json))
}
#[wasm_bindgen]
pub fn builtin_font3d(name: &str) -> Result<u32, JsValue> {
    cutting_command(|s| s.builtin_font3d(name))
}
#[wasm_bindgen]
pub fn font3d_info(id: u32) -> Result<String, JsValue> {
    cutting_command(|s| s.font3d_info(id))
}
#[wasm_bindgen]
pub fn remove_font3d(id: u32) -> bool {
    cutting_command(|s| Ok(s.remove_font3d(id))).unwrap_or(false)
}
#[wasm_bindgen]
pub fn add_font3d_text(font: u32, text: &str, options_json: &str) -> Result<u32, JsValue> {
    cutting_command(|s| s.add_font3d_text(font, text, options_json))
}
#[wasm_bindgen]
pub fn update_font3d_text(id: u32, text: &str, options_json: &str) -> Result<(), JsValue> {
    cutting_command(|s| s.update_font3d_text(id, text, options_json))
}
#[wasm_bindgen]
pub fn font3d_text_info(id: u32) -> Result<String, JsValue> {
    cutting_command(|s| s.font3d_text_info(id))
}

/// Static numeric tube inscription, sharing cached digit meshes with native HUDs.
#[wasm_bindgen]
pub fn add_numeric_text(value: &str, width: f32, height: f32) -> Result<u32, JsValue> {
    cutting_command(|scene| scene.add_numeric_text(value, width, height))
}

/// Static bitmap inscription with no backing rectangle.
#[wasm_bindgen]
pub fn add_text_panel(
    title: &str,
    lines_json: &str,
    width: f32,
    height: f32,
) -> Result<u32, JsValue> {
    if lines_json.len() > 8192 || title.len() > 192 {
        return Err(JsValue::from_str("Text panel input exceeds limits"));
    }
    let lines: Vec<String> =
        serde_json::from_str(lines_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    STATE.with(|slot| {
        let slot = slot.borrow();
        let state = slot
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine is not initialized"))?;
        let result = state
            .borrow_mut()
            .scene
            .add_text_panel(title, &lines, width, height)
            .map_err(|e| JsValue::from_str(&e));
        result
    })
}

#[wasm_bindgen]
pub fn add_cube(x: f32, y: f32, z: f32) -> u32 {
    let mut id = crate::scene::INVALID_ID;
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            id = st.borrow_mut().scene.add_cube(glam::Vec3::new(x, y, z));
        }
    });
    id
}

fn cutting_command<T>(
    command: impl FnOnce(&mut crate::scene::SceneManager) -> Result<T, String>,
) -> Result<T, JsValue> {
    STATE.with(|state| {
        let state = state.borrow();
        let engine = state
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine není inicializován"))?;
        let result = command(&mut engine.borrow_mut().scene).map_err(|e| JsValue::from_str(&e));
        result
    })
}

#[wasm_bindgen]
pub fn add_triangle_mesh(
    positions: &[f32],
    indices: &[u32],
    r: f32,
    g: f32,
    b: f32,
    smooth: bool,
) -> Result<u32, JsValue> {
    if !positions.len().is_multiple_of(3)
        || positions.len() > crate::cutting::MAX_TRIANGLES * 9
        || indices.len() > crate::cutting::MAX_TRIANGLES * 3
    {
        return Err(JsValue::from_str("Invalid triangle mesh buffers"));
    }
    let positions = positions
        .as_chunks::<3>()
        .0
        .iter()
        .map(|&p| glam::Vec3::from_array(p))
        .collect();
    let mut mesh = crate::cutting::CutMesh::new(positions, indices, glam::Vec3::new(r, g, b))
        .map_err(|e| JsValue::from_str(&e))?;
    if smooth {
        mesh.smooth_normals();
    }
    cutting_command(|scene| scene.add_triangle_mesh(mesh))
}

#[wasm_bindgen]
pub fn register_mesh_variants(
    positions: &[f32],
    indices: &[u32],
    colors: &[f32],
    smooth: bool,
) -> u32 {
    cutting_command(|scene| scene.register_mesh_variants(positions, indices, colors, smooth))
        .unwrap_or_else(|error| {
            web_sys::console::error_1(&error);
            crate::scene::INVALID_ID
        })
}
#[wasm_bindgen]
pub fn add_mesh_variant(handle: u32, variant: u32) -> u32 {
    cutting_command(|scene| scene.add_mesh_variant(handle, variant)).unwrap_or_else(|error| {
        web_sys::console::error_1(&error);
        crate::scene::INVALID_ID
    })
}
#[wasm_bindgen]
pub fn remove_mesh_variants(handle: u32) -> bool {
    cutting_command(|scene| Ok(scene.remove_mesh_variants(handle))).unwrap_or(false)
}

#[wasm_bindgen]
pub fn set_cuttable(id: u32, enabled: bool, r: f32, g: f32, b: f32) -> Result<(), JsValue> {
    cutting_command(|scene| scene.set_cuttable(id, enabled, glam::Vec3::new(r, g, b)))
}

#[wasm_bindgen]
pub fn add_colored_triangle_mesh(
    positions: &[f32],
    indices: &[u32],
    colors: &[f32],
    smooth: bool,
) -> Result<u32, JsValue> {
    if positions.len() > crate::cutting::MAX_TRIANGLES * 9
        || !positions.len().is_multiple_of(3)
        || indices.len() > crate::cutting::MAX_TRIANGLES * 3
        || colors.len() != indices.len()
        || colors
            .iter()
            .any(|c| !c.is_finite() || !(0.0..=1.0).contains(c))
    {
        return Err(JsValue::from_str("Invalid triangle colors or mesh budget"));
    }
    let positions = positions
        .as_chunks::<3>()
        .0
        .iter()
        .map(|&p| glam::Vec3::from_array(p))
        .collect();
    let mut mesh = crate::cutting::CutMesh::new(positions, indices, glam::Vec3::ONE)
        .map_err(|e| JsValue::from_str(&e))?;
    for (triangle, &color) in mesh.triangles.iter_mut().zip(colors.as_chunks::<3>().0) {
        triangle.color = glam::Vec3::from_array(color);
    }
    if smooth {
        mesh.smooth_normals();
    }
    cutting_command(|scene| scene.add_triangle_mesh(mesh))
}

#[wasm_bindgen]
pub fn aim_cutter(
    id: u32,
    guides: &[u32],
    center: &[f32],
    settings: &[f32],
) -> Result<Vec<f32>, JsValue> {
    if center.len() != 3 || settings.len() != 4 {
        return Err(JsValue::from_str("Invalid cutting aim arguments"));
    }
    cutting_command(|scene| {
        scene
            .aim_cutter(
                id,
                guides,
                crate::cutting_tool::ToolAim {
                    center: glam::Vec3::from_slice(center),
                    yaw: settings[0],
                    tilt: settings[1],
                    height: settings[2],
                    floor: settings[3],
                },
            )
            .map(|p| p.to_array().to_vec())
    })
}

#[wasm_bindgen]
pub fn set_cutter(id: u32, enabled: bool, endpoints: &[f32]) -> Result<(), JsValue> {
    let blade = if enabled {
        if endpoints.len() != 6 {
            return Err(JsValue::from_str("Cutter requires two local endpoints"));
        }
        Some([
            glam::Vec3::from_slice(&endpoints[..3]),
            glam::Vec3::from_slice(&endpoints[3..]),
        ])
    } else {
        None
    };
    cutting_command(|scene| scene.set_cutter(id, blade))
}

#[wasm_bindgen]
pub fn set_blade_contact_response(id: u32, enabled: bool) -> Result<(), JsValue> {
    cutting_command(|scene| scene.set_blade_contact_response(id, enabled))
}

#[wasm_bindgen]
pub fn set_cut_assist(id: u32, coverage: f32) -> Result<(), JsValue> {
    cutting_command(|scene| scene.set_cut_assist(id, coverage))
}

#[wasm_bindgen]
pub fn cut_with(id: u32, x: f32, y: f32, z: f32, separation: f32) -> Result<String, JsValue> {
    cutting_command(|scene| {
        let report = scene.cut_with(id, glam::Vec3::new(x, y, z), separation)?;
        Ok(serde_json::json!({"removed":report.removed,"pieces":report.pieces,"objectCandidates":report.object_candidates,"bvhNodes":report.bvh_nodes,"triangleTests":report.triangle_tests}).to_string())
    })
}

#[wasm_bindgen]
pub fn play_cut_feedback(pieces: &[u32]) -> Result<String, JsValue> {
    cutting_command(|scene| {
        let objects = scene.play_cut_feedback(pieces)?;
        Ok(serde_json::json!({"objects":objects,"duration":0.65}).to_string())
    })
}
#[wasm_bindgen]
pub fn play_cut_feedback_facing(
    pieces: &[u32],
    mirror_text: bool,
    size: Option<f32>,
    score_label: Option<String>,
) -> Result<String, JsValue> {
    let rewards = if matches!(score_label.as_deref(), Some("hit-points" | "stock-total")) {
        Some(
            STATE
                .with(|slot| {
                    slot.borrow().as_ref().map(|state| {
                        state
                            .borrow()
                            .xr_cut_rewards()
                            .iter()
                            .map(|reward| {
                                (
                                    reward.original,
                                    if score_label.as_deref() == Some("stock-total") {
                                        reward.total_points
                                    } else {
                                        reward.points
                                    },
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .ok_or_else(|| JsValue::from_str("Engine neni inicializovan"))?,
        )
    } else {
        None
    };
    let label = match score_label.as_deref() {
        None => None,
        Some("pending-bonus") => Some(
            STATE
                .with(|slot| {
                    slot.borrow()
                        .as_ref()
                        .map(|state| format!("BONUS {}", state.borrow().xr_pending_bonus()))
                })
                .ok_or_else(|| JsValue::from_str("Engine neni inicializovan"))?,
        ),
        Some("hit-points" | "stock-total") => None,
        Some(_) => return Err(JsValue::from_str("Unknown cut feedback score label")),
    };
    cutting_command(|scene| {
        let objects = if let Some(rewards) = &rewards {
            scene.play_cut_feedback_scored(pieces, mirror_text, size.unwrap_or(1.), rewards)?
        } else {
            scene.play_cut_feedback_labeled(
                pieces,
                mirror_text,
                size.unwrap_or(1.),
                label.as_deref(),
            )?
        };
        Ok(serde_json::json!({"objects":objects,"duration":0.65}).to_string())
    })
}
#[wasm_bindgen]
pub fn clear_cut_feedback() -> Result<(), JsValue> {
    cutting_command(|scene| {
        scene.clear_cut_feedback();
        Ok(())
    })
}
#[wasm_bindgen]
pub fn set_blade_trail(id: u32, enabled: bool) -> Result<(), JsValue> {
    cutting_command(|scene| scene.set_blade_trail(id, enabled))
}

#[wasm_bindgen]
pub fn cut_with_scatter(
    id: u32,
    motion: &[f32],
    separation: f32,
    floor_y: f32,
    distance: f32,
    duration: f32,
) -> Result<String, JsValue> {
    if motion.len() != 3 {
        return Err(JsValue::from_str("Invalid cutting motion"));
    }
    cutting_command(|scene| {
        let report = scene.cut_with_scatter(
            id,
            glam::Vec3::from_slice(motion),
            separation,
            floor_y,
            distance,
            duration,
        )?;
        Ok(serde_json::json!({"removed":report.removed,"pieces":report.pieces,"objectCandidates":report.object_candidates,"bvhNodes":report.bvh_nodes,"triangleTests":report.triangle_tests}).to_string())
    })
}

#[wasm_bindgen]
pub fn triangle_mesh_info(id: u32) -> Result<String, JsValue> {
    cutting_command(|scene| {
        if let Some(panel) = scene.info_panels.get(&id) {
            let transform = scene.objects()[&id].transform;
            let bottom = panel
                .mesh
                .vertices
                .iter()
                .map(|v| {
                    transform
                        .model()
                        .transform_point3(glam::Vec3::from_array(v.position))
                        .y
                })
                .fold(f32::INFINITY, f32::min);
            return Ok(serde_json::json!({"triangles":panel.mesh.indices.len()/3,"vertices":panel.mesh.vertices.len(),"volume":0.,"cuttable":false,"closed":false,"resident":false,"cpuMaterialized":true,"position":transform.position.to_array(),"rotation":transform.rotation.to_array(),"scale":transform.scale.to_array(),"emission":scene.objects()[&id].emission,"bottom":bottom,"cutProgress":0.,"jammedBy":null}).to_string());
        }

        let object = scene
            .triangle_meshes()
            .get(&id)
            .ok_or("Triangle mesh no longer exists")?;
        let transform = scene.objects()[&id].transform;
        let bottom = object.mesh.bottom(transform)?;
        let (cut_progress, jammed_by) = scene.blade_cut_state(id);
        Ok(serde_json::json!({"triangles":object.mesh.triangle_count(),"vertices":object.mesh.vertex_count(),"volume":object.mesh.volume(),"cuttable":object.cuttable,"closed":true,"resident":object.mesh.is_resident(),"cpuMaterialized":object.mesh.ready_cpu().is_some(),"position":transform.position.to_array(),"rotation":transform.rotation.to_array(),"scale":transform.scale.to_array(),"emission":scene.objects()[&id].emission,"bottom":bottom,"cutProgress":cut_progress,"jammedBy":jammed_by}).to_string())
    })
}
/// Explicit diagnostic snapshot, never fetched by the render loop.
#[wasm_bindgen]
pub fn triangle_mesh_snapshot(id: u32) -> Result<String, JsValue> {
    cutting_command(|scene| {
        if let Some(panel) = scene.info_panels.get(&id) {
            let mesh = &panel.mesh;
            return Ok(serde_json::json!({"positions":mesh.vertices.iter().map(|v| v.position).collect::<Vec<_>>(),
                "triangles":mesh.indices.as_chunks::<3>().0,
                "normals":mesh.vertices.iter().map(|v| v.normal).collect::<Vec<_>>(),
                "colors":mesh.vertices.iter().map(|v| v.color).collect::<Vec<_>>()}).to_string());
        }

        let mesh = &scene
            .triangle_meshes()
            .get(&id)
            .ok_or("Triangle mesh no longer exists")?
            .mesh;
        let mesh = mesh.cpu()?;
        let rendered = mesh.render_mesh();
        Ok(serde_json::json!({"positions":mesh.positions.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
            "triangles":mesh.triangles.iter().map(|t| t.indices).collect::<Vec<_>>(),
            "normals":rendered.vertices.iter().map(|v| v.normal).collect::<Vec<_>>(),
            "colors":rendered.vertices.iter().map(|v| v.color).collect::<Vec<_>>()}).to_string())
    })
}

/// Reads actual GPU vertices for explicit verification, never during a normal cut.
#[wasm_bindgen]
pub async fn triangle_mesh_gpu_snapshot(id: u32) -> Result<String, JsValue> {
    let state = STATE
        .with(|s| s.borrow().clone())
        .ok_or_else(|| JsValue::from_str("Engine disposed"))?;
    let (source, device, queue) = {
        let st = state.borrow();
        let (device, queue) = st.resident_devices();
        (
            st.resident_source(id)
                .ok_or_else(|| JsValue::from_str("Mesh is not GPU resident"))?,
            device,
            queue,
        )
    };
    let vertices = source
        .read_vertices(&device, &queue)
        .await
        .map_err(|e| JsValue::from_str(&e))?;
    Ok(serde_json::json!({"positions":vertices.iter().map(|v|&v.position[..3]).collect::<Vec<_>>(),
        "normals":vertices.iter().map(|v|&v.normal[..3]).collect::<Vec<_>>(),
        "colors":vertices.iter().map(|v|&v.color[..3]).collect::<Vec<_>>()}).to_string())
}

async fn cut_resident_async(
    state: Rc<RefCell<State>>,
    generation: u64,
    id: u32,
    motion: glam::Vec3,
    separation: f32,
    scatter: Vec<f32>,
    timing: (web_sys::Performance, f64),
) -> Result<String, JsValue> {
    let (clock, start) = timing;
    use crate::{
        cutting_mesh::{MeshData, MeshStats, SplitRecipe, SupportCurve},
        render::cutting_resident::{ResidentOutcome, RESIDENT_METADATA_BYTES},
    };
    let fail = |e: String| JsValue::from_str(&e);
    let scatter = if scatter.is_empty() {
        None
    } else {
        Some([scatter[0], scatter[1], scatter[2]])
    };
    let (snapshot, candidates) = {
        let st = state.borrow();
        (
            st.scene.cut_snapshot(id).map_err(fail)?,
            st.scene
                .resident_candidates(id, motion, separation, scatter)
                .map_err(fail)?,
        )
    };
    let count = candidates.len();
    let mut staged = Vec::new();
    let mut gpu_buffers = Vec::new();
    let mut fallback = None;
    let mut readback_bytes = 0;
    let gpu_start = clock.now();
    let (device, queue) = state.borrow().resident_devices();
    for candidate in candidates {
        let pending = match state.borrow_mut().dispatch_resident(&candidate) {
            Ok(p) => p,
            Err(e) => {
                fallback = Some(e);
                break;
            }
        };
        let outcome = pending.read(&device, &queue).await.map_err(fail)?;
        readback_bytes += RESIDENT_METADATA_BYTES;
        match outcome {
            ResidentOutcome::Miss => {}
            ResidentOutcome::Unsupported(reason) => {
                fallback = Some(reason);
                break;
            }
            ResidentOutcome::Pieces(pieces) => {
                let pieces = *pieces;
                let recipe = SplitRecipe::new(
                    candidate.mesh.clone(),
                    candidate.a,
                    candidate.b,
                    candidate.motion,
                    candidate.color,
                );
                let data = std::array::from_fn(|side| {
                    let p = &pieces[side];
                    let stats = MeshStats {
                        min: p.bounds_min,
                        max: p.bounds_max,
                        triangles: p.vertex_count as usize / 3,
                        vertices: p.vertex_count as usize,
                        volume: p.volume as f64,
                    };
                    let radius = (p.bounds_min.abs().max(p.bounds_max.abs())
                        * candidate.transform.scale.abs())
                    .length();
                    let support = SupportCurve {
                        rotation: candidate.transform.rotation,
                        angular: candidate.angular[side],
                        scale: candidate.transform.scale,
                        samples: p.support,
                        second_bound: radius * candidate.angular[side].abs().element_sum().powi(2),
                    };
                    MeshData::resident(stats, recipe.clone(), side, support)
                });
                staged.push((candidate, data));
                gpu_buffers.extend(pieces);
            }
        }
    }
    let compute_ms = clock.now() - gpu_start;
    if GENERATION.with(|g| g.get()) != generation {
        return Err(fail("Cut canceled by engine disposal".into()));
    }
    let mut st = state.borrow_mut();
    if st.scene.cut_snapshot(id).ok().as_ref() != Some(&snapshot) {
        return Err(fail(
            "Cut canceled because the scene changed during compute".into(),
        ));
    }
    let gpu_meshes = staged.len();
    let report = if fallback.is_some() {
        if let Some([floor, distance, duration]) = scatter {
            st.scene
                .cut_with_scatter(id, motion, separation, floor, distance, duration)
        } else {
            st.scene.cut_with(id, motion, separation)
        }
    } else {
        st.scene
            .commit_resident(id, motion, separation, scatter, staged, count)
    }
    .map_err(fail)?;
    if fallback.is_none() {
        for (&piece_id, piece) in report.pieces.iter().zip(gpu_buffers) {
            st.install_resident(piece_id, piece);
        }
    }
    let total_ms = clock.now() - start;
    Ok(serde_json::json!({"removed":report.removed,"pieces":report.pieces,"objectCandidates":report.object_candidates,"bvhNodes":report.bvh_nodes,"triangleTests":report.triangle_tests,
        "execution":if fallback.is_none(){"webgpu-resident"}else{"cpu"},"gpuMeshes":if fallback.is_none(){gpu_meshes}else{0},"fallbackReason":fallback,
        "metadataReadbackBytes":readback_bytes,"geometryReadbackBytes":0,"totalMs":total_ms,"computeMs":compute_ms,"cpuMs":(total_ms-compute_ms).max(0.)}).to_string())
}

/// No RefCell borrow survives the asynchronous GPU readback. A scene revision
/// and engine generation guard prevent late results from replacing newer data.
#[wasm_bindgen]
pub async fn cut_with_async(
    id: u32,
    motion: Vec<f32>,
    separation: f32,
    scatter: Vec<f32>,
    execution: String,
) -> Result<String, JsValue> {
    let fail = |message: &str| JsValue::from_str(message);
    if motion.len() != 3
        || (!scatter.is_empty() && scatter.len() != 3)
        || !["auto", "cpu", "gpu", "gpu-resident"].contains(&execution.as_str())
        || !separation.is_finite()
        || !(0.0..=2.0).contains(&separation)
    {
        return Err(fail("Invalid asynchronous cutting options"));
    }
    let state = STATE
        .with(|s| s.borrow().clone())
        .ok_or_else(|| fail("Engine disposed"))?;
    let generation = GENERATION.with(|g| g.get());
    let busy = state.borrow().cutting_busy.clone();
    if busy.replace(true) {
        return Err(fail("A cutting request is already pending"));
    }
    struct Busy(std::rc::Rc<std::cell::Cell<bool>>);
    impl Drop for Busy {
        fn drop(&mut self) {
            self.0.set(false);
        }
    }
    let _busy = Busy(busy);
    let clock = web_sys::window()
        .and_then(|w| w.performance())
        .ok_or_else(|| fail("Performance clock unavailable"))?;
    let start = clock.now();
    let motion = glam::Vec3::from_slice(&motion);
    let gpu_supported = state.borrow().backend == "BrowserWebGpu";
    if (execution == "gpu" || execution == "gpu-resident") && !gpu_supported {
        return Err(fail("Compute cutting requires WebGPU"));
    }
    if execution == "gpu-resident" {
        return cut_resident_async(
            state,
            generation,
            id,
            motion,
            separation,
            scatter,
            (clock, start),
        )
        .await;
    }
    // Measured hybrid readback is slower for current meshes. Auto deliberately
    // stays on the optimized CPU path; forced GPU is explicit and testable.
    let use_gpu = gpu_supported && execution == "gpu";
    let (snapshot, inputs) = if use_gpu {
        let st = state.borrow();
        (
            Some(st.scene.cut_snapshot(id).map_err(|e| fail(&e))?),
            st.scene
                .cut_geometry_inputs(id, motion)
                .map_err(|e| fail(&e))?,
        )
    } else {
        (None, Vec::new())
    };
    let mut geometries = std::collections::BTreeMap::new();
    let mut compute_ms = 0.;
    let mut fallback_reason = None;
    if use_gpu && !inputs.is_empty() {
        let gpu_start = clock.now();
        let pending = state.borrow_mut().dispatch_cut_geometry(&inputs);
        match pending.read().await {
            Ok(rows) => {
                if rows.len() != inputs.len() {
                    return Err(fail("Invalid compute result count"));
                }
                for (input, rows) in inputs.iter().zip(&rows) {
                    match input.decode(rows).map_err(|e| fail(&e))? {
                        Some(geometry) => {
                            geometries.insert(input.id, geometry);
                        }
                        None => {
                            fallback_reason = Some("near-plane precision");
                        }
                    }
                }
            }
            Err(error) => {
                if execution == "gpu" {
                    return Err(fail(&error));
                }
                fallback_reason = Some("compute readback failed");
            }
        }
        compute_ms = clock.now() - gpu_start;
    }
    if GENERATION.with(|g| g.get()) != generation {
        return Err(fail("Cut canceled by engine disposal"));
    }
    let mut st = state.borrow_mut();
    if snapshot.is_some_and(|snapshot| st.scene.cut_snapshot(id).ok().as_ref() != Some(&snapshot)) {
        return Err(fail(
            "Cut canceled because the scene changed during compute",
        ));
    }
    let report = if scatter.is_empty() {
        st.scene
            .cut_with_geometry(id, motion, separation, Some(&geometries))
    } else {
        st.scene.cut_with_scatter_geometry(
            id,
            motion,
            separation,
            [scatter[0], scatter[1], scatter[2]],
            Some(&geometries),
        )
    }
    .map_err(|e| fail(&e))?;
    let total_ms = clock.now() - start;
    Ok(serde_json::json!({"removed":report.removed,"pieces":report.pieces,
        "objectCandidates":report.object_candidates,"bvhNodes":report.bvh_nodes,"triangleTests":report.triangle_tests,
        "execution":if geometries.is_empty() {"cpu"} else {"webgpu-compute"},
        "gpuMeshes":geometries.len(),"fallbackReason":fallback_reason,
        "totalMs":total_ms,"computeMs":compute_ms,"cpuMs":(total_ms-compute_ms).max(0.)}).to_string())
}

#[wasm_bindgen]
pub fn add_plane(x: f32, y: f32, z: f32) -> u32 {
    let mut id = crate::scene::INVALID_ID;
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            id = st.borrow_mut().scene.add_plane(glam::Vec3::new(x, y, z));
        }
    });
    id
}

#[wasm_bindgen]
pub fn add_sphere(x: f32, y: f32, z: f32) -> u32 {
    let mut id = crate::scene::INVALID_ID;
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            id = st.borrow_mut().scene.add_sphere(glam::Vec3::new(x, y, z));
        }
    });
    id
}

#[wasm_bindgen]
pub fn remove_object(id: u32) -> bool {
    let mut ok = false;
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            ok = st.borrow_mut().scene.remove_object(id);
        }
    });
    ok
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)] // Explicit GPU/FFI parameter layout.
pub fn set_object_transform(
    id: u32,
    x: f32,
    y: f32,
    z: f32,
    rx: f32,
    ry: f32,
    rz: f32,
    sx: f32,
    sy: f32,
    sz: f32,
) -> bool {
    let mut ok = false;
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            ok = st.borrow_mut().scene.set_object_transform(
                id,
                glam::Vec3::new(x, y, z),
                glam::Vec3::new(rx, ry, rz),
                glam::Vec3::new(sx, sy, sz),
            );
        }
    });
    ok
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)] // Explicit GPU/FFI parameter layout.
pub fn set_cube_transform(
    id: u32,
    x: f32,
    y: f32,
    z: f32,
    rx: f32,
    ry: f32,
    rz: f32,
    sx: f32,
    sy: f32,
    sz: f32,
) -> bool {
    set_object_transform(id, x, y, z, rx, ry, rz, sx, sy, sz)
}

#[wasm_bindgen]
pub fn clear_lights() {
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            st.borrow_mut().scene.clear_lights();
        }
    });
}

#[wasm_bindgen]
pub fn add_light(x: f32, y: f32, z: f32, r: f32, g: f32, b: f32) -> i32 {
    let mut id = -1;
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            if let Some(light_id) = st
                .borrow_mut()
                .scene
                .add_light(glam::Vec3::new(x, y, z), glam::Vec3::new(r, g, b))
            {
                id = light_id as i32;
            }
        }
    });
    id
}

#[wasm_bindgen]
pub fn remove_light(id: u32) -> bool {
    let mut ok = false;
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            ok = st.borrow_mut().scene.remove_light(id);
        }
    });
    ok
}

#[wasm_bindgen]
pub fn schedule_rotate(
    id: u32,
    rx_deg: f32,
    ry_deg: f32,
    rz_deg: f32,
    delay: f32,
    duration: f32,
) -> bool {
    let mut ok = false;
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            ok = st
                .borrow_mut()
                .scene
                .schedule_rotate(id, rx_deg, ry_deg, rz_deg, delay, duration);
        }
    });
    ok
}

#[wasm_bindgen]
pub fn schedule_remove_object(id: u32, delay: f32) -> bool {
    let mut ok = false;
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            ok = st.borrow_mut().scene.schedule_remove_object(id, delay);
        }
    });
    ok
}

#[wasm_bindgen]
pub fn schedule_remove_light(id: u32, delay: f32) -> bool {
    let mut ok = false;
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            ok = st.borrow_mut().scene.schedule_remove_light(id, delay);
        }
    });
    ok
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)] // Explicit GPU/FFI parameter layout.
pub fn set_light_orbit(
    id: u32,
    radius: f32,
    height: f32,
    speed: f32,
    phase: f32,
    pulse: f32,
    r: f32,
    g: f32,
    b: f32,
) -> bool {
    let mut ok = false;
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            ok = st.borrow_mut().scene.set_light_orbit(
                id,
                radius,
                height,
                speed,
                phase,
                pulse,
                glam::Vec3::new(r, g, b),
            );
        }
    });
    ok
}

#[wasm_bindgen]
pub fn clear_light_orbit(id: u32) -> bool {
    let mut ok = false;
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            ok = st.borrow_mut().scene.clear_light_orbit(id);
        }
    });
    ok
}

#[wasm_bindgen]
pub fn set_light(id: u32, x: f32, y: f32, z: f32, r: f32, g: f32, b: f32) -> bool {
    let mut ok = false;
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            ok = st.borrow_mut().scene.set_light(
                id,
                glam::Vec3::new(x, y, z),
                glam::Vec3::new(r, g, b),
            );
        }
    });
    ok
}

// Apply the same rig origin to platform eyes, head, hands and interaction rays.
fn xr_world(matrix: glam::Mat4) -> glam::Mat4 {
    RIM.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(matrix, |game| game.xr_origin * matrix)
    })
}
fn xr_world_array(matrix: &[f32]) -> Result<[f32; 16], JsValue> {
    let values: &[f32; 16] = matrix
        .try_into()
        .map_err(|_| JsValue::from_str("Invalid XR matrix"))?;
    let pose = glam::Mat4::from_cols_array(values);
    if !crate::xr_input::valid_pose(pose) {
        return Err(JsValue::from_str("Invalid XR pose"));
    }
    Ok(xr_world(pose).to_cols_array())
}
/// Raw platform input; Aurelia owns movement and simulation time.
#[wasm_bindgen]
pub fn rim_update_xr(
    head: &[f32],
    ray: &[f32],
    axes: &[f32],
    cancel: bool,
    dt: f32,
) -> Result<(), JsValue> {
    if !XR_ACTIVE.with(|active| active.get()) || !dt.is_finite() || dt < 0.0 {
        return Err(JsValue::from_str("Invalid XR frame"));
    }
    let pose = |matrix: &[f32]| {
        <&[f32; 16]>::try_from(matrix)
            .ok()
            .map(glam::Mat4::from_cols_array)
    };
    let axes: [f32; 3] = axes
        .try_into()
        .map_err(|_| JsValue::from_str("Three movement axes required"))?;
    RIM.with(|slot| {
        if let Some(game) = slot.borrow_mut().as_mut() {
            game.xr_move(pose(head), pose(ray), axes, cancel, dt);
        }
    });
    Ok(())
}

fn xr_pose_state(
    pose: Option<crate::xr_player::TrackedPoseState>,
    object: Option<u32>,
) -> serde_json::Value {
    if let Some(pose) = pose {
        let orientation = glam::Quat::from_mat4(&pose.matrix).normalize();
        serde_json::json!({"tracked":true,"matrix":pose.matrix.to_cols_array(),"position":pose.matrix.w_axis.truncate().to_array(),"orientation":orientation.to_array(),"speed":pose.speed,"object":object})
    } else {
        serde_json::json!({"tracked":false,"matrix":null,"position":null,"orientation":null,"speed":0.,"object":object})
    }
}
#[wasm_bindgen]
pub fn xr_player_state() -> Result<String, JsValue> {
    cutting_command(|scene| {
        XR_PLAYER.with(|player| {
        let player = player.borrow();
        let live = |id: u32| scene.object_exists(id);
        Ok(serde_json::json!({
            "head":xr_pose_state(player.head(),player.head_object().filter(|id|live(*id))),
            "leftHand":xr_pose_state(player.hand(0),player.controllers[0].object_id().filter(|id|live(*id))),
            "rightHand":xr_pose_state(player.hand(1),player.controllers[1].object_id().filter(|id|live(*id))),
            "inventory":player.inventory().into_iter().filter(|id|live(*id)).collect::<Vec<_>>()
        }).to_string())
    })
    })
}
#[wasm_bindgen]
pub fn xr_bind_head(id: u32) -> Result<(), JsValue> {
    cutting_command(|scene| {
        XR_PLAYER.with(|player| {
            player
                .borrow_mut()
                .bind_head(scene, (id != u32::MAX).then_some(id))
        })
    })
}
#[wasm_bindgen]
pub fn xr_add_item(id: u32) -> Result<(), JsValue> {
    cutting_command(|scene| XR_PLAYER.with(|player| player.borrow_mut().add_item(scene, id)))
}
#[wasm_bindgen]
pub fn xr_remove_item(id: u32) -> Result<(), JsValue> {
    cutting_command(|scene| XR_PLAYER.with(|player| player.borrow_mut().remove_item(scene, id)))
}
#[wasm_bindgen]
pub fn xr_update_head(matrix: &[f32]) -> Result<(), JsValue> {
    if !XR_ACTIVE.with(|active| active.get()) {
        return Err(JsValue::from_str("XR is not presenting"));
    }
    let pose = if matrix.is_empty() {
        None
    } else {
        let matrix: &[f32; 16] = matrix
            .try_into()
            .map_err(|_| JsValue::from_str("Invalid head matrix"))?;
        Some(xr_world(glam::Mat4::from_cols_array(matrix)))
    };
    cutting_command(|scene| XR_PLAYER.with(|player| player.borrow_mut().update_head(scene, pose)))
}

/// Bind an authored object to a grip pose. UINT32_MAX unbinds it.
#[wasm_bindgen]
pub fn xr_bind_controller(hand: usize, id: u32, cutting: bool) -> Result<(), JsValue> {
    if hand > 1 {
        return Err(JsValue::from_str("Invalid controller hand"));
    }
    cutting_command(|scene| {
        XR_PLAYER.with(|player| {
            player
                .borrow_mut()
                .bind_hand(scene, hand, (id != u32::MAX).then_some(id), cutting)
        })
    })
}

/// Opt-in finite blade contact profile; the edge direction is object-local.
#[wasm_bindgen]
pub fn set_blade_physics(
    id: u32,
    edge: &[f32],
    half_width: f32,
    half_thickness: f32,
) -> Result<(), JsValue> {
    let config = if edge.is_empty() {
        None
    } else {
        let edge: &[f32; 3] = edge
            .try_into()
            .map_err(|_| JsValue::from_str("Invalid blade edge"))?;
        Some(crate::blade_physics::BladePhysics {
            edge: glam::Vec3::from_array(*edge),
            half_width,
            half_thickness,
        })
    };
    cutting_command(|scene| scene.set_blade_physics(id, config))
}

/// A platform target ray against an actual transformed scene panel.
#[wasm_bindgen]
pub fn xr_panel_hit(id: u32, matrix: &[f32]) -> Vec<f32> {
    let Ok(matrix) = <&[f32; 16]>::try_from(matrix) else {
        return vec![];
    };
    STATE.with(|slot| {
        let state = slot.borrow();
        let Some(state) = state.as_ref() else {
            return vec![];
        };
        let state = state.borrow();
        let Some(panel) = state.scene.info_panels.get(&id) else {
            return vec![];
        };
        let Some(object) = state.scene.objects().get(&id) else {
            return vec![];
        };
        crate::xr_buttons::ray_panel_intersection(
            xr_world(glam::Mat4::from_cols_array(matrix)),
            object.transform.model(),
            panel.width,
            panel.height,
            5.,
        )
        .map_or_else(Vec::new, |point| point.to_array().to_vec())
    })
}

/// Raw grip poses against the registered finite blade and actual panel volume.
#[wasm_bindgen]
pub fn xr_panel_blade_hit(panel_id: u32, cutter_id: u32, from: &[f32], to: &[f32]) -> Vec<u8> {
    STATE.with(|slot| {
        let state = slot.borrow();
        let Some(state) = state.as_ref() else {
            return vec![];
        };
        let state = state.borrow();
        let Some(panel) = state.scene.info_panels.get(&panel_id) else {
            return vec![];
        };
        let Some(panel_object) = state.scene.objects().get(&panel_id) else {
            return vec![];
        };
        let Some(cutter) = state.scene.objects().get(&cutter_id) else {
            return vec![];
        };
        let Some(blade) = state.scene.tracked_blade(cutter_id) else {
            return vec![];
        };
        let Ok(to) = <&[f32; 16]>::try_from(to) else {
            return vec![0, 0];
        };
        let from = <&[f32; 16]>::try_from(from)
            .ok()
            .map(|matrix| xr_world(glam::Mat4::from_cols_array(matrix)));
        crate::xr_buttons::blade_panel_contact(
            from,
            xr_world(glam::Mat4::from_cols_array(to)),
            blade,
            cutter.transform.scale,
            panel_object.transform.model(),
            [panel.width, panel.height],
            state.frame_dt(),
        )
        .map(u8::from)
        .to_vec()
    })
}

/// Raw platform grip matrix; all pose/sweep/geometry calculations remain in Rust.
#[wasm_bindgen]
pub fn xr_update_controller(hand: usize, matrix: &[f32], pressed: bool) -> Result<String, JsValue> {
    let _scope = crate::render::profiler::cpu_scope("input.contacts.cutting");
    if hand > 1 || !XR_ACTIVE.with(|active| active.get()) {
        return Err(JsValue::from_str("Invalid XR controller frame"));
    }
    let pose = if matrix.is_empty() {
        None
    } else {
        let matrix: &[f32; 16] = matrix
            .try_into()
            .map_err(|_| JsValue::from_str("Invalid grip matrix"))?;
        Some(xr_world(glam::Mat4::from_cols_array(matrix)))
    };
    let (dt, round_finished) = STATE.with(|slot| {
        slot.borrow().as_ref().map_or((0., false), |state| {
            let state = state.borrow();
            (state.frame_dt(), state.xr_round_finished())
        })
    });
    let (report, blade_contacts) = cutting_command(|scene| {
        let before = scene.projectile_contacts;
        let report = XR_PLAYER.with(|player| {
            player
                .borrow_mut()
                .update_hand(scene, hand, pose, pressed && !round_finished, dt)
        })?;
        Ok((report, scene.projectile_contacts.saturating_sub(before)))
    })?;
    // cutting_command's mutable scene borrow must end before borrowing State again.
    let score_awards = STATE.with(|slot| {
        if let Some(state) = slot.borrow().as_ref() {
            let mut state = state.borrow_mut();
            state.record_xr_cuts(&report.removed);
            state.xr_cut_rewards().iter().map(|reward| serde_json::json!({"original":reward.original,"points":reward.points,"totalPoints":reward.total_points})).collect::<Vec<_>>()
        } else { Vec::new() }
    });
    Ok(serde_json::json!({"removed":report.removed,"pieces":report.pieces,"objectCandidates":report.object_candidates,"bvhNodes":report.bvh_nodes,"triangleTests":report.triangle_tests,"bladeContacts":blade_contacts,"scoreAwards":score_awards}).to_string())
}

#[wasm_bindgen]
pub fn configure_combo_display(input: &str) -> bool {
    STATE.with(|s| {
        s.borrow()
            .as_ref()
            .is_some_and(|s| s.borrow_mut().configure_combo_display(input))
    })
}
#[wasm_bindgen]
pub fn clear_combo_display() {
    STATE.with(|s| {
        if let Some(s) = s.borrow().as_ref() {
            s.borrow_mut().clear_combo_display();
        }
    });
}
#[wasm_bindgen]
pub fn xr_bank_chain(index: usize) -> bool {
    if !XR_ACTIVE.with(|active| active.get()) {
        return false;
    }
    STATE.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|state| state.borrow_mut().bank_xr_chain(index))
    })
}
#[wasm_bindgen]
pub fn combo_display_state() -> String {
    STATE.with(|s| {
        s.borrow()
            .as_ref()
            .map_or_else(|| "null".into(), |s| s.borrow().combo_display_state())
    })
}

/// Bind a world object to one of three slots in the currently hit combo row.
#[wasm_bindgen]
pub fn bind_combo_indicator(id: u32, kind: &str, slot: u32) -> bool {
    STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .is_some_and(|state| state.borrow_mut().bind_combo_indicator(id, kind, slot))
    })
}

/// Explicit original-target scoring metadata; fragments do not inherit it.
#[wasm_bindgen]
pub fn set_combo_target(id: u32, kind: &str, points: u32, wild: bool) -> bool {
    STATE.with(|slot| {
        let slot = slot.borrow();
        let Some(state) = slot.as_ref() else {
            return false;
        };
        let mut state = state.borrow_mut();
        if !state.scene.object_exists(id) {
            return false;
        }
        // Keep metadata bounded by live targets, including expired projectiles.
        let live: std::collections::BTreeSet<_> = state.scene.objects().keys().copied().collect();
        state.combo_score.targets.retain(|id, _| live.contains(id));
        state.combo_score.register(id, kind, points, wild)
    })
}

/// Opt-in round duration; the application chooses its game length.
#[wasm_bindgen]
pub fn xr_start_round(seconds: f32) -> Result<(), JsValue> {
    if !XR_ACTIVE.with(|active| active.get()) {
        return Err(JsValue::from_str("XR session is not active"));
    }
    STATE.with(|slot| {
        let slot = slot.borrow();
        let state = slot
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine unavailable"))?;
        let mut state = state.borrow_mut();
        state.start_xr_round(seconds)?;
        XR_PLAYER.with(|player| {
            for controller in player.borrow_mut().controllers.iter_mut() {
                controller.reset(&mut state.scene);
            }
        });
        Ok(())
    })
}

#[wasm_bindgen]
pub fn xr_round_state() -> String {
    STATE.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or_else(|| "null".into(), |state| state.borrow().xr_round_state())
    })
}

/// WebXR host owns session scheduling; Rust retains scene and view calculations.
#[wasm_bindgen]
pub fn xr_set_presenting(active: bool) -> Result<(), JsValue> {
    STATE.with(|slot| {
        let state = slot.borrow();
        let state = state
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine unavailable"))?;
        if !active {
            let mut state = state.borrow_mut();
            XR_PLAYER.with(|player| player.borrow_mut().reset_tracking(&mut state.scene));
        }
        RIM.with(|slot| {
            if let Some(game) = slot.borrow_mut().as_mut() {
                game.set_xr(active);
            }
        });
        XR_ACTIVE.with(|flag| flag.set(active));
        RESET_FRAME_CLOCK.with(|reset| reset.set(true));
        Ok(())
    })
}

#[wasm_bindgen]
pub fn xr_gpu_supported() -> bool {
    STATE.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|state| state.borrow().xr_gpu_supported())
    })
}
/// Same device used by the Rust renderer; required for XRGPUBinding.
#[wasm_bindgen]
pub fn xr_gpu_device() -> Result<JsValue, JsValue> {
    STATE.with(|slot| {
        slot.borrow()
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine unavailable"))?
            .borrow()
            .xr_device()
    })
}
#[wasm_bindgen]
pub fn xr_gpu_color_format() -> Result<String, JsValue> {
    STATE.with(|slot| {
        Ok(slot
            .borrow()
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine unavailable"))?
            .borrow()
            .xr_color_format())
    })
}
/// Render into the browser-owned projection texture. No canvas transfer/readback.
#[wasm_bindgen]
pub fn xr_render_gpu_eye(
    projection: &[f32],
    world: &[f32],
    sub_image: JsValue,
) -> Result<(), JsValue> {
    if !XR_ACTIVE.with(|active| active.get()) {
        return Err(JsValue::from_str("Invalid XR frame"));
    }
    let (matrix, position) = crate::render_math::xr_gpu_view(projection, &xr_world_array(world)?)
        .ok_or_else(|| JsValue::from_str("Invalid XR matrices"))?;
    STATE.with(|slot| {
        let state = slot.borrow();
        let mut state = state
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine unavailable"))?
            .borrow_mut();
        state.render_xr_gpu_image(matrix, position, &sub_image)
    })
}

/// Present completed stereo effects with one final compositor-facing submission.
#[wasm_bindgen]
pub fn xr_present_gpu_frame(images: js_sys::Array) -> Result<(), JsValue> {
    if !XR_ACTIVE.with(|active| active.get()) {
        return Err(JsValue::from_str("Invalid XR frame"));
    }
    STATE.with(|slot| {
        let state = slot.borrow();
        let mut state = state
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine unavailable"))?
            .borrow_mut();
        state.present_xr_gpu_frame(&images)
    })
}

/// Close CPU/GPU profiling after both eyes, including the final XR presentation.
#[wasm_bindgen]
pub fn xr_finish_frame() {
    STATE.with(|slot| {
        if let Some(state) = slot.borrow().as_ref() {
            state.borrow().finish_xr_frame();
        }
    });
}

/// Advance once per XR frame, never once per eye.
#[wasm_bindgen]
pub fn xr_begin_frame(dt: f32) -> Result<(), JsValue> {
    if !XR_ACTIVE.with(|active| active.get()) || !dt.is_finite() || dt < 0.0 {
        return Err(JsValue::from_str("Invalid XR frame"));
    }
    STATE.with(|slot| {
        let state = slot.borrow();
        let state = state
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine unavailable"))?;
        let scale = RIM.with(|slot| {
            slot.borrow()
                .as_ref()
                .map_or(1.0, |game| if dt > 0.0 { game.sim_dt / dt } else { 1.0 })
        });
        RIM_REAL_DT.with(|value| value.set(dt));
        state.borrow_mut().advance_xr(dt, scale);
        Ok(())
    })
}

/// Matrices come unchanged from XRView; clip conversion and eye position are Rust.
#[wasm_bindgen]
pub fn xr_render_eye(
    projection: &[f32],
    world: &[f32],
    width: u32,
    height: u32,
) -> Result<(), JsValue> {
    if !XR_ACTIVE.with(|active| active.get()) || width == 0 || height == 0 {
        return Err(JsValue::from_str("Invalid XR eye"));
    }
    let (matrix, position) = crate::render_math::xr_view(projection, &xr_world_array(world)?)
        .ok_or_else(|| JsValue::from_str("Invalid XR matrices"))?;
    STATE.with(|slot| {
        let state = slot.borrow();
        let state = state
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Engine unavailable"))?;
        let mut state = state.borrow_mut();
        state.resize(width, height);
        state.render_xr_eye(matrix, position)
    })
}

#[wasm_bindgen]
pub fn start() -> js_sys::Promise {
    if STATE.with(|state| state.borrow().is_some()) {
        return js_sys::Promise::resolve(&JsValue::UNDEFINED);
    }
    let generation = GENERATION.with(|generation| generation.get());
    if STARTING.with(|flag| {
        if flag.get().is_some() {
            true
        } else {
            flag.set(Some(generation));
            false
        }
    }) {
        return js_sys::Promise::reject(&JsValue::from_str(
            "Engine initialization already in progress",
        ));
    }
    // Reserve ownership before the first asynchronous poll, so synchronous dispose
    // also invalidates a just-returned start promise.
    wasm_bindgen_futures::future_to_promise(async move {
        start_runtime(generation).await?;
        Ok(JsValue::UNDEFINED)
    })
}

fn check_start_generation(generation: u64) -> Result<(), JsValue> {
    if GENERATION.with(|current| current.get()) != generation {
        Err(JsValue::from_str(
            "Engine initialization canceled by dispose",
        ))
    } else {
        Ok(())
    }
}

async fn start_runtime(generation: u64) -> Result<(), JsValue> {
    struct StartingGuard(u64);
    impl Drop for StartingGuard {
        fn drop(&mut self) {
            STARTING.with(|flag| {
                if flag.get() == Some(self.0) {
                    flag.set(None);
                }
            });
        }
    }
    let _guard = StartingGuard(generation);
    check_start_generation(generation)?;
    console_error_panic_hook::set_once();
    let window =
        web_sys::window().ok_or_else(|| JsValue::from_str("Browser window unavailable"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("Browser document unavailable"))?;
    let performance = window
        .performance()
        .ok_or_else(|| JsValue::from_str("Browser performance unavailable"))?;
    let canvas = document
        .get_element_by_id("gpu-canvas")
        .ok_or_else(|| JsValue::from_str("Canvas gpu-canvas not found"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let state = Rc::new(RefCell::new(
        State::new(&canvas, || check_start_generation(generation)).await?,
    ));
    check_start_generation(generation)?;
    let aspect = state.borrow().aspect;
    let camera = Rc::new(RefCell::new(ActiveCamera::new(aspect)));

    let mut listeners = keyboard::attach(&window, camera.clone())?;
    listeners.extend(mouse::attach(&window, &canvas, camera.clone())?);

    let prev_time = Rc::new(RefCell::new(performance.now()));
    let f: AnimationFrameCallback = Rc::new(RefCell::new(None));
    let g = f.clone();
    let weak = Rc::downgrade(&f);
    let frame = Rc::new(std::cell::Cell::new(None));
    let frame_c = frame.clone();
    let window_c = window.clone();
    let perf_c = performance.clone();
    let camera_c = camera.clone();
    let state_c = state.clone();
    let prev_time_c = prev_time.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        frame_c.set(None);
        let now = perf_c.now();
        let dt = if RESET_FRAME_CLOCK.with(|reset| reset.replace(false)) {
            0.0
        } else {
            (now - *prev_time_c.borrow()) as f32 / 1000.0
        };
        *prev_time_c.borrow_mut() = now;
        if !XR_ACTIVE.with(|active| active.get()) {
            let mut cam = camera_c.borrow_mut();
            let mut st = state_c.borrow_mut();
            let prepare_start = perf_c.now();
            cam.update(dt);
            cam.update_scene(&mut st.scene, dt);
            let cam_pos = cam.position();
            let cam_matrix = cam.matrix();
            let game_view = RIM.with(|slot| {
                let mut slot = slot.borrow_mut();
                let game = slot.as_mut()?;
                let sim = game.tick(&mut st.scene, dt, None, [None; 2]);
                game.playing().then(|| {
                    let (matrix, pos) = game.camera(st.aspect);
                    (sim, matrix, pos)
                })
            });
            if let Some((sim, matrix, pos)) = game_view {
                st.clear_model_camera();
                st.update(sim, matrix, pos);
            } else {
                st.update(dt, cam_matrix, cam_pos);
            }

            if let Err(error) = st.render() {
                web_sys::console::error_1(&error);
                if let Some(status) = web_sys::window()
                    .and_then(|window| window.document())
                    .and_then(|document| document.get_element_by_id("status"))
                {
                    status.set_text_content(Some("Vykreslování se zastavilo. Obnovte stránku."));
                }
                return;
            }
            st.prepare_ms = perf_c.now() - prepare_start;
        }
        if let Some(callback) = weak.upgrade() {
            if let Ok(id) = window_c.request_animation_frame(
                callback.borrow().as_ref().unwrap().as_ref().unchecked_ref(),
            ) {
                frame_c.set(Some(id));
            }
        }
    }) as Box<dyn FnMut()>));

    check_start_generation(generation)?;
    frame.set(Some(window.request_animation_frame(
        g.borrow().as_ref().unwrap().as_ref().unchecked_ref(),
    )?));
    STATE.with(|s| *s.borrow_mut() = Some(state));
    CAMERA.with(|c| *c.borrow_mut() = Some(camera));
    RUNTIME.with(|runtime| {
        *runtime.borrow_mut() = Some(Runtime {
            window,
            frame,
            _callback: g,
            _listeners: listeners,
        })
    });
    Ok(())
}

// Explicit Aurelia scene configuration and input; never spawns a runtime scene.
#[wasm_bindgen]
pub fn configure_rim(input: &str) -> Result<(), JsValue> {
    let config: Option<crate::rim_game::Config> =
        serde_json::from_str(input).map_err(|e| JsValue::from_str(&e.to_string()))?;
    cutting_command(|scene| {
        let game = config
            .map(|c| crate::rim_game::Game::new(c, scene))
            .transpose()?;
        RIM.with(|slot| {
            let mut slot = slot.borrow_mut();
            if let Some(old) = slot.as_mut() {
                old.teardown(scene);
            }
            *slot = game;
        });
        Ok(())
    })
}
#[wasm_bindgen]
pub fn rim_input(input: &str) -> Result<(), JsValue> {
    let input = serde_json::from_str(input).map_err(|e| JsValue::from_str(&e.to_string()))?;
    cutting_command(|scene| {
        RIM.with(|slot| {
            slot.borrow_mut()
                .as_mut()
                .ok_or("Aurelia is not configured")?
                .input(input, scene)
        })
    })
}
#[wasm_bindgen]
pub fn rim_state() -> String {
    RIM.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or("null".into(), |g| g.state().to_string())
    })
}
#[wasm_bindgen]
pub fn rim_take_cut_job() -> Result<Vec<u8>, JsValue> {
    RIM.with(|slot| {
        slot.borrow_mut()
            .as_mut()
            .map_or(Ok(Vec::new()), |g| g.take_cut_job())
            .map_err(|e| JsValue::from_str(&e))
    })
}
#[wasm_bindgen]
pub fn rim_accept_cut(bytes: &[u8]) -> Result<(), JsValue> {
    cutting_command(|scene| {
        RIM.with(|slot| {
            slot.borrow_mut()
                .as_mut()
                .map_or(Ok(()), |g| g.accept_cut(scene, bytes))
        })
    })
}
#[wasm_bindgen]
pub fn rim_cut_failed() {
    RIM.with(|slot| {
        if let Some(g) = slot.borrow_mut().as_mut() {
            g.cut_failed();
        }
    })
}
#[wasm_bindgen]
pub fn rim_split_geometry(bytes: &[u8]) -> Result<Vec<u8>, JsValue> {
    crate::rim_cut::split(bytes).map_err(|e| JsValue::from_str(&e))
}
#[wasm_bindgen]
pub fn rim_xr_buttons(aim: bool, dash: bool) {
    RIM.with(|slot| {
        if let Some(g) = slot.borrow_mut().as_mut() {
            g.xr_buttons(aim, dash);
        }
    })
}
/// Called after current platform hand/head poses and before either eye is drawn.
#[wasm_bindgen]
pub fn xr_prepare_gameplay() {
    STATE.with(|slot| {
        if let Some(state) = slot.borrow().as_ref() {
            let mut state = state.borrow_mut();
            RIM.with(|slot| {
                if let Some(g) = slot.borrow_mut().as_mut() {
                    XR_PLAYER.with(|player| {
                        let p = player.borrow();
                        g.tick(
                            &mut state.scene,
                            RIM_REAL_DT.with(|d| d.get()),
                            p.head().map(|p| p.matrix),
                            [p.hand(0).map(|p| p.matrix), p.hand(1).map(|p| p.matrix)],
                        );
                    });
                }
            });
        }
    })
}
