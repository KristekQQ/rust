#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{closure::Closure, JsCast};
#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};

#[cfg(target_arch = "wasm32")]
use super::camera::Camera;

#[cfg(target_arch = "wasm32")]
pub fn attach(window: &web_sys::Window, camera: Rc<RefCell<Camera>>) -> Result<(), wasm_bindgen::JsValue> {
    let cam_key = camera.clone();
    let key_down = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
        cam_key.borrow_mut().key_down(e.code());
    }) as Box<dyn FnMut(_)>);
    window.add_event_listener_with_callback("keydown", key_down.as_ref().unchecked_ref())?;
    key_down.forget();

    let cam_key_up = camera.clone();
    let key_up = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
        cam_key_up.borrow_mut().key_up(e.code());
    }) as Box<dyn FnMut(_)>);
    window.add_event_listener_with_callback("keyup", key_up.as_ref().unchecked_ref())?;
    key_up.forget();

    let cam_mouse = camera;
    let mouse_move = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
        cam_mouse.borrow_mut().mouse_move(e.movement_x() as f32, e.movement_y() as f32);
    }) as Box<dyn FnMut(_)>);
    window.add_event_listener_with_callback("mousemove", mouse_move.as_ref().unchecked_ref())?;
    mouse_move.forget();

    Ok(())
}
