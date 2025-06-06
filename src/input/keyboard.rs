#![cfg(target_arch = "wasm32")]

use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::Window;

use crate::input::camera::CameraController;

pub fn attach<T>(window: &Window, cam: Rc<RefCell<T>>)
where
    T: CameraController + 'static,
{
    let cam_down = cam.clone();
    let key_down = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
        cam_down.borrow_mut().key_down(e.code());
    }) as Box<dyn FnMut(_)>);
    window
        .add_event_listener_with_callback("keydown", key_down.as_ref().unchecked_ref())
        .unwrap();
    key_down.forget();

    let cam_up = cam.clone();
    let key_up = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
        cam_up.borrow_mut().key_up(e.code());
    }) as Box<dyn FnMut(_)>);
    window
        .add_event_listener_with_callback("keyup", key_up.as_ref().unchecked_ref())
        .unwrap();
    key_up.forget();
}
