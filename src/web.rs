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
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            st.borrow_mut().scene.clear_scene();
        }
    });
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
        let dt = (now - *prev_time_c.borrow()) as f32 / 1000.0;
        *prev_time_c.borrow_mut() = now;
        {
            let mut cam = camera_c.borrow_mut();
            let mut st = state_c.borrow_mut();
            let prepare_start = perf_c.now();
            cam.update(dt);
            cam.update_scene(&mut st.scene, dt);
            let cam_pos = cam.position();
            let cam_matrix = cam.matrix();
            st.update(dt, cam_matrix, cam_pos);

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
