#![cfg(target_arch = "wasm32")]

use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{closure::Closure, JsCast};

use glam::Mat4;

use crate::input::active_camera::{ActiveCamera, CameraType};
use crate::input::{keyboard, mouse};
use crate::render::scene::SceneManager;
use crate::render::data;
use crate::render::state::State;

thread_local! {
    static STATE: RefCell<Option<Rc<RefCell<State>>>> = RefCell::new(None);
    static CAMERA: RefCell<Option<Rc<RefCell<ActiveCamera>>>> = RefCell::new(None);
    static SCENE_MANAGER: RefCell<Option<SceneManager>> = RefCell::new(None);
    static GRID_ID: RefCell<Option<usize>> = RefCell::new(None);
}

#[wasm_bindgen]
pub fn set_grid_visible(show: bool) {
    SCENE_MANAGER.with(|sc| {
        if let Some(mgr) = &mut *sc.borrow_mut() {
            GRID_ID.with(|id_cell| {
                if show {
                    if id_cell.borrow().is_none() {
                        let id = mgr.add_grid(10);
                        *id_cell.borrow_mut() = Some(id);
                    }
                } else if let Some(id) = id_cell.borrow_mut().take() {
                    mgr.remove(id);
                }
            });
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
pub fn add_cube(position: &[f32;3], size: f32) -> usize {
    SCENE_MANAGER.with(|s| {
        if let Some(scene) = &mut *s.borrow_mut() {
            let mut verts = crate::render::data::VERTICES.to_vec();
            for v in verts.iter_mut() {
                v.position[0] = v.position[0] * size + position[0];
                v.position[1] = v.position[1] * size + position[1];
                v.position[2] = v.position[2] * size + position[2];
            }
            scene.add_mesh(&verts, crate::render::data::INDICES, Mat4::IDENTITY)
        }
        0
    })
}

#[wasm_bindgen]
pub fn add_light(position: &[f32;3], color: &[f32;3]) -> usize {
    SCENE_MANAGER.with(|s| {
        if let Some(scene) = &mut *s.borrow_mut() {
            return scene.add_light(*position, *color);
        }
        0
    })
}

#[wasm_bindgen(start)]
pub async fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document
        .get_element_by_id("gpu-canvas")
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let state = Rc::new(RefCell::new(State::new(&canvas).await?));
    let mut manager = SceneManager::new(
        state.borrow().device().clone(),
        state.borrow().queue().clone(),
        state.borrow().format(),
    );
    manager.add_grid(10);
    manager.add_mesh(data::VERTICES, data::INDICES, Mat4::IDENTITY);
    STATE.with(|s| *s.borrow_mut() = Some(state.clone()));
    SCENE_MANAGER.with(|sc| *sc.borrow_mut() = Some(manager));
    let performance = window.performance().unwrap();
    let aspect = state.borrow().aspect;
    let camera = Rc::new(RefCell::new(ActiveCamera::new(aspect)));
    CAMERA.with(|c| *c.borrow_mut() = Some(camera.clone()));

    keyboard::attach(&window, camera.clone());
    mouse::attach(&window, camera.clone());

    let start_time = performance.now();
    let prev_time = Rc::new(RefCell::new(start_time));
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
        let mut cam = camera_c.borrow_mut();
        cam.update(dt);
        let cam_pos = cam.position();
        let cam_matrix = cam.matrix();
        SCENE_MANAGER.with(|sc| {
            if let Some(mgr) = &mut *sc.borrow_mut() {
                mgr.update(dt);
                mgr.set_camera(cam_matrix, cam_pos);
                STATE.with(|st| {
                    if let Some(state) = &mut *st.borrow_mut() {
                        if state.render(mgr).is_err() {
                            return;
                        }
                    }
                });
            }
        });
        window_c
            .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
            .unwrap();
    }) as Box<dyn FnMut()>));

    window.request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref())?;
    Ok(())
}
