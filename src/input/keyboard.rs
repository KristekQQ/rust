use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

use crate::input::camera::Camera;

pub fn init(window: &web_sys::Window, camera: Rc<RefCell<Camera>>) {
    let cam = camera.clone();
    let key_down = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
        cam.borrow_mut().key_down(e.code());
    }) as Box<dyn FnMut(_)>);
    window
        .add_event_listener_with_callback("keydown", key_down.as_ref().unchecked_ref())
        .unwrap();
    key_down.forget();

    let cam = camera.clone();
    let key_up = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
        cam.borrow_mut().key_up(e.code());
    }) as Box<dyn FnMut(_)>);
    window
        .add_event_listener_with_callback("keyup", key_up.as_ref().unchecked_ref())
        .unwrap();
    key_up.forget();
}
