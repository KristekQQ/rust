#![cfg(target_arch = "wasm32")]

use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{EventTarget, Window};

use crate::input::camera::CameraController;

pub struct Listener {
    target: EventTarget,
    name: &'static str,
    callback: Closure<dyn FnMut(web_sys::Event)>,
}
impl Listener {
    pub fn new(
        target: &EventTarget,
        name: &'static str,
        callback: impl FnMut(web_sys::Event) + 'static,
    ) -> Result<Self, wasm_bindgen::JsValue> {
        let callback = Closure::wrap(Box::new(callback) as Box<dyn FnMut(web_sys::Event)>);
        target.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
        Ok(Self {
            target: target.clone(),
            name,
            callback,
        })
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self
            .target
            .remove_event_listener_with_callback(self.name, self.callback.as_ref().unchecked_ref());
    }
}

pub fn attach<T>(
    window: &Window,
    cam: Rc<RefCell<T>>,
) -> Result<Vec<Listener>, wasm_bindgen::JsValue>
where
    T: CameraController + 'static,
{
    let down = cam.clone();
    let up = cam.clone();
    Ok(vec![
        Listener::new(window.as_ref(), "keydown", move |e| {
            let event = e.unchecked_into::<web_sys::KeyboardEvent>();
            // Jump is a fresh key press, never an auto-repeat after pause/blur.
            if event.code() != "Space" || !event.repeat() {
                down.borrow_mut().key_down(event.code());
            }
        })?,
        Listener::new(window.as_ref(), "keyup", move |e| {
            up.borrow_mut()
                .key_up(e.unchecked_into::<web_sys::KeyboardEvent>().code())
        })?,
        Listener::new(window.as_ref(), "blur", move |_| {
            cam.borrow_mut().clear_input()
        })?,
    ])
}
