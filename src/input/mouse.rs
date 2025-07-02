#![cfg(target_arch = "wasm32")]

use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{Window, HtmlCanvasElement};

use crate::input::camera::CameraController;

pub fn attach<T>(window: &Window, canvas: &HtmlCanvasElement, cam: Rc<RefCell<T>>)
where
    T: CameraController + 'static,
{
    let dragging = Rc::new(RefCell::new(false));

    // Start dragging only when the left button is pressed on the canvas
    {
        let dragging = dragging.clone();
        let canvas = canvas.clone();
        let on_down = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
            if e.button() == 0 {
                if let Some(target) = e.target() {
                    if target == canvas.clone().dyn_into::<web_sys::EventTarget>().unwrap() {
                        *dragging.borrow_mut() = true;
                    }
                }
            }
        }) as Box<dyn FnMut(_)>);
        canvas
            .add_event_listener_with_callback("mousedown", on_down.as_ref().unchecked_ref())
            .unwrap();
        on_down.forget();
    }

    // Stop dragging on mouseup anywhere in the window
    {
        let dragging = dragging.clone();
        let on_up = Closure::wrap(Box::new(move |_e: web_sys::MouseEvent| {
            *dragging.borrow_mut() = false;
        }) as Box<dyn FnMut(_)>);
        window
            .add_event_listener_with_callback("mouseup", on_up.as_ref().unchecked_ref())
            .unwrap();
        on_up.forget();
    }

    // Apply mouse movement only while dragging
    {
        let dragging = dragging.clone();
        let cam_mouse = cam.clone();
        let mouse_move = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
            if *dragging.borrow() && e.buttons() & 1 == 1 {
                cam_mouse
                    .borrow_mut()
                    .mouse_move(e.movement_x() as f32, e.movement_y() as f32);
            }
        }) as Box<dyn FnMut(_)>);
        window
            .add_event_listener_with_callback("mousemove", mouse_move.as_ref().unchecked_ref())
            .unwrap();
        mouse_move.forget();
    }
}
