#![cfg(target_arch = "wasm32")]

use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{closure::Closure, JsCast};

use crate::input::active_camera::{ActiveCamera, CameraType};
use crate::input::camera::CameraController;
use crate::input::{keyboard, mouse};
use crate::render::state::State;

thread_local! {
    static STATE: RefCell<Option<Rc<RefCell<State>>>> = RefCell::new(None);
    static CAMERA: RefCell<Option<Rc<RefCell<ActiveCamera>>>> = RefCell::new(None);
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
pub fn load_culling_demo() {
    STATE.with(|state| {
        if let Some(state) = state.borrow().as_ref() {
            state.borrow_mut().scene.load_culling_demo();
        }
    });
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
pub fn load_example_scene() {
    STATE.with(|state| {
        if let Some(state) = state.borrow().as_ref() {
            state.borrow_mut().scene.load_example();
        }
    });
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
pub fn set_grid_visible(show: bool) {
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            st.borrow_mut().set_grid_visible(show);
        }
    });
}

#[wasm_bindgen]
pub fn set_camera_mode(mode: &str) {
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

#[wasm_bindgen]
pub fn resize(width: u32, height: u32) {
    if width == 0 || height == 0 {
        return;
    }
    STATE.with(|s| {
        if let Some(st) = &*s.borrow() {
            st.borrow_mut().resize(width, height);
        }
    });
    CAMERA.with(|c| {
        if let Some(cam) = &*c.borrow() {
            cam.borrow_mut().set_aspect(width as f32 / height as f32);
        }
    });
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
pub async fn start() -> Result<(), JsValue> {
    if STATE.with(|state| state.borrow().is_some()) {
        return Ok(());
    }
    console_error_panic_hook::set_once();
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document
        .get_element_by_id("gpu-canvas")
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let state = Rc::new(RefCell::new(State::new(&canvas).await?));
    STATE.with(|s| *s.borrow_mut() = Some(state.clone()));
    let performance = window.performance().unwrap();
    let aspect = state.borrow().aspect;
    let camera = Rc::new(RefCell::new(ActiveCamera::new(aspect)));
    CAMERA.with(|c| *c.borrow_mut() = Some(camera.clone()));

    keyboard::attach(&window, camera.clone());
    mouse::attach(&window, &canvas, camera.clone());

    let prev_time = Rc::new(RefCell::new(performance.now()));
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    let window_c = window.clone();
    let perf_c = performance.clone();
    let camera_c = camera.clone();
    let state_c = state.clone();
    let prev_time_c = prev_time.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let now = perf_c.now();
        let dt = (now - *prev_time_c.borrow()) as f32 / 1000.0;
        *prev_time_c.borrow_mut() = now;
        {
            let mut cam = camera_c.borrow_mut();
            cam.update(dt);
            let cam_pos = cam.position();
            let cam_matrix = cam.matrix();
            let mut st = state_c.borrow_mut();
            let prepare_start = perf_c.now();
            st.update(dt, cam_matrix, cam_pos);
            st.prepare_ms = perf_c.now() - prepare_start;
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
        }
        window_c
            .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
            .unwrap();
    }) as Box<dyn FnMut()>));

    window.request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref())?;
    Ok(())
}
