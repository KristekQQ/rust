use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

use crate::input::camera::Camera;

pub fn init(window: &web_sys::Window, camera: Rc<RefCell<Camera>>) {
    let cam = camera.clone();
    let mouse_move = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
        cam.borrow_mut().mouse_move(e.movement_x() as f32, e.movement_y() as f32);
    }) as Box<dyn FnMut(_)>);
    window
        .add_event_listener_with_callback("mousemove", mouse_move.as_ref().unchecked_ref())
        .unwrap();
    mouse_move.forget();
}
