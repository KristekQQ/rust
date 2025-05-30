use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use web_sys::{Window, KeyboardEvent, MouseEvent};

use crate::graphics::pipeline::State;
use crate::input::camera::Camera;

#[cfg(target_arch = "wasm32")]
pub fn hook_events(window: &Window, cam: Rc<RefCell<Camera>>) {
    let cam_key = cam.clone();
    let key_down = Closure::wrap(Box::new(move |e: KeyboardEvent| {
        cam_key.borrow_mut().key_down(e.code());
    }) as Box<dyn FnMut(_)>);
    window
        .add_event_listener_with_callback("keydown", key_down.as_ref().unchecked_ref())
        .unwrap();
    key_down.forget();

    let cam_key_up = cam.clone();
    let key_up = Closure::wrap(Box::new(move |e: KeyboardEvent| {
        cam_key_up.borrow_mut().key_up(e.code());
    }) as Box<dyn FnMut(_)>);
    window
        .add_event_listener_with_callback("keyup", key_up.as_ref().unchecked_ref())
        .unwrap();
    key_up.forget();

    let cam_mouse = cam.clone();
    let mouse_move = Closure::wrap(Box::new(move |e: MouseEvent| {
        cam_mouse
            .borrow_mut()
            .mouse_move(e.movement_x() as f32, e.movement_y() as f32);
    }) as Box<dyn FnMut(_)>);
    window
        .add_event_listener_with_callback("mousemove", mouse_move.as_ref().unchecked_ref())
        .unwrap();
    mouse_move.forget();
}

#[cfg(target_arch = "wasm32")]
pub fn run_event_loop(
    window: Window,
    cam: Rc<RefCell<Camera>>,
    mut state: State,
) -> Result<(), JsValue> {
    let performance = window.performance().unwrap();
    let start_time = performance.now();
    let prev_time = Rc::new(RefCell::new(start_time));
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    let window_c = window.clone();
    let perf_c = performance.clone();
    let camera_c = cam.clone();
    let prev_time_c = prev_time.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let now = perf_c.now();
        let dt = (now - *prev_time_c.borrow()) as f32 / 1000.0;
        *prev_time_c.borrow_mut() = now;
        let elapsed = (now - start_time) as f32 / 1000.0;
        let angle = elapsed / 5.0 * (2.0 * std::f32::consts::PI);
        {
            let mut cam = camera_c.borrow_mut();
            cam.update(dt);
            let cam_matrix = cam.matrix();
            state.update(angle, cam_matrix);
            if state.render().is_err() {
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

