#![cfg(target_arch = "wasm32")]

use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{closure::Closure, JsCast};

use crate::scene::{Light, Node};
use glam::{Mat4, Vec3};

use crate::input::active_camera::{ActiveCamera, CameraType};
use crate::input::camera::CameraController;
use crate::input::{keyboard, mouse};
use crate::render::state::State;

thread_local! {
    static STATE: RefCell<Option<Rc<RefCell<State>>>> = RefCell::new(None);
    static CAMERA: RefCell<Option<Rc<RefCell<ActiveCamera>>>> = RefCell::new(None);
    static COMMANDS: RefCell<Vec<Command>> = RefCell::new(Vec::new());
}

enum Command {
    AddNode { model: Mat4, parent: i32 },
    AddLight { pos: [f32; 3], color: [f32; 3] },
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
pub fn add_cube(x: f32, y: f32, z: f32, parent: i32) {
    COMMANDS.with(|q| {
        q.borrow_mut().push(Command::AddNode {
            model: Mat4::from_translation(Vec3::new(x, y, z)),
            parent,
        });
    });
}

#[wasm_bindgen]
pub fn add_light(x: f32, y: f32, z: f32, r: f32, g: f32, b: f32) {
    COMMANDS.with(|q| {
        q.borrow_mut().push(Command::AddLight {
            pos: [x, y, z],
            color: [r, g, b],
        });
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
    STATE.with(|s| *s.borrow_mut() = Some(state.clone()));
    let performance = window.performance().unwrap();
    let aspect = state.borrow().aspect;
    let camera = Rc::new(RefCell::new(ActiveCamera::new(aspect)));
    CAMERA.with(|c| *c.borrow_mut() = Some(camera.clone()));

    keyboard::attach(&window, camera.clone());
    mouse::attach(&window, &canvas, camera.clone());

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
        let elapsed = (now - start_time) as f32 / 1000.0;
        let angle = elapsed / 5.0 * (2.0 * std::f32::consts::PI);
        COMMANDS.with(|q| {
            let mut q = q.borrow_mut();
            if !q.is_empty() {
                let mut st = state_c.borrow_mut();
                for cmd in q.drain(..) {
                    match cmd {
                        Command::AddNode { model, parent } => {
                            st.scene.nodes.push(Node {
                                model: model.to_cols_array_2d(),
                                parent,
                                _pad: [0; 3],
                            });
                        }
                        Command::AddLight { pos, color } => {
                            st.scene.lights.push(Light {
                                position: pos,
                                _pad_p: 0.0,
                                color,
                                _pad_c: 0.0,
                            });
                        }
                    }
                }
                st.ensure_capacity();
            }
        });
        {
            let mut cam = camera_c.borrow_mut();
            cam.update(dt);
            let cam_pos = cam.position();
            let cam_matrix = cam.matrix();
            let model = Mat4::from_rotation_z(angle);
            let mut st = state_c.borrow_mut();
            st.update(cam_matrix, model, cam_pos);
            if st.render().is_err() {
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
