#![cfg(target_arch = "wasm32")]

use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, MouseEvent, PointerEvent, Window};

use crate::input::camera::CameraController;

pub fn attach<T>(
    window: &Window,
    canvas: &HtmlCanvasElement,
    cam: Rc<RefCell<T>>,
) -> Result<Vec<super::keyboard::Listener>, wasm_bindgen::JsValue>
where
    T: CameraController + 'static,
{
    use super::keyboard::Listener;
    let dragging = Rc::new(RefCell::new(false));
    let down = dragging.clone();
    let up = dragging.clone();
    let target: web_sys::EventTarget = canvas.clone().into();
    let movement_canvas = canvas.clone();
    let document = window
        .document()
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("Browser document unavailable"))?;
    Ok(vec![
        Listener::new(canvas.as_ref(), "pointerdown", move |e| {
            let e = e.unchecked_into::<PointerEvent>();
            if e.buttons() & 1 == 1 && e.target().as_ref() == Some(&target) {
                *down.borrow_mut() = true;
            }
        })?,
        Listener::new(window.as_ref(), "pointerup", move |_| {
            *up.borrow_mut() = false
        })?,
        Listener::new(window.as_ref(), "mousemove", move |e| {
            let e = e.unchecked_into::<MouseEvent>();
            let locked = document
                .pointer_lock_element()
                .as_ref()
                .is_some_and(|element| {
                    element == movement_canvas.unchecked_ref::<web_sys::Element>()
                });
            if locked || (*dragging.borrow() && e.buttons() & 1 == 1) {
                cam.borrow_mut()
                    .mouse_move(e.movement_x() as f32, e.movement_y() as f32);
            }
        })?,
    ])
}
