use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use glam::Mat4;

use crate::input::camera::Camera;
use crate::render::renderer::Renderer;

pub fn run(window: web_sys::Window, camera: Rc<RefCell<Camera>>, renderer: Rc<RefCell<Renderer>>) {
    let performance = window.performance().unwrap();
    let start_time = performance.now();
    let prev_time = Rc::new(RefCell::new(start_time));
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    let win_c = window.clone();
    let perf_c = performance.clone();
    let cam_c = camera.clone();
    let rend_c = renderer.clone();
    let prev_c = prev_time.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let now = perf_c.now();
        let dt = (now - *prev_c.borrow()) as f32 / 1000.0;
        *prev_c.borrow_mut() = now;
        let elapsed = (now - start_time) as f32 / 1000.0;
        let angle = elapsed / 5.0 * (2.0 * std::f32::consts::PI);
        {
            let mut cam = cam_c.borrow_mut();
            cam.update(dt);
            let mvp = cam.matrix() * Mat4::from_rotation_z(angle);
            rend_c.borrow_mut().update(mvp);
            let _ = rend_c.borrow_mut().render();
        }
        win_c
            .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
            .unwrap();
    }) as Box<dyn FnMut()>));

    window
        .request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref())
        .unwrap();
}
