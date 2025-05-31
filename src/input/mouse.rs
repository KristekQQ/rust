#![cfg(target_arch = "wasm32")]

use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::Window;

use crate::input::camera::Camera;

pub fn attach(window: &Window, cam: Rc<RefCell<Camera>>) {
    let cam_mouse = cam.clone();
    let mouse_move = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
        cam_mouse
            .borrow_mut()
            .mouse_move(e.movement_x() as f32, e.movement_y() as f32);
    }) as Box<dyn FnMut(_)>);
    window
        .add_event_listener_with_callback("mousemove", mouse_move.as_ref().unchecked_ref())
        .unwrap();
    mouse_move.forget();
}
