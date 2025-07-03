#![cfg(target_arch = "wasm32")]

use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{Window, HtmlCanvasElement, PointerEvent};

use crate::input::camera::CameraController;

pub fn attach<T>(window: &Window, canvas: &HtmlCanvasElement, cam: Rc<RefCell<T>>)
where
    T: CameraController + 'static,
{
    let dragging = Rc::new(RefCell::new(false));

    // Start dragging only when the left button is pressed on the canvas
    {
        let dragging = dragging.clone();
        let canvas_clone = canvas.clone();
        let on_down = Closure::wrap(Box::new(move |e: PointerEvent| {
            if e.buttons() & 1 == 1 {
                if let Some(target) = e.target() {
                    if target == canvas_clone
                        .clone()
                        .dyn_into::<web_sys::EventTarget>()
                        .unwrap()
                    {
                        *dragging.borrow_mut() = true;
                    }
                }
            }
        }) as Box<dyn FnMut(_)>);
        canvas
            .add_event_listener_with_callback("pointerdown", on_down.as_ref().unchecked_ref())
            .unwrap();
        on_down.forget();
    }

    // Stop dragging on mouseup anywhere in the window
    {
        let dragging = dragging.clone();
        let on_up = Closure::wrap(Box::new(move |_e: PointerEvent| {
            *dragging.borrow_mut() = false;
        }) as Box<dyn FnMut(_)>);
        window
            .add_event_listener_with_callback("pointerup", on_up.as_ref().unchecked_ref())
            .unwrap();
        on_up.forget();
    }

    // Apply mouse movement only while dragging
    {
        let dragging = dragging.clone();
        let cam_mouse = cam.clone();
        let mouse_move = Closure::wrap(Box::new(move |e: PointerEvent| {
            if *dragging.borrow() && e.buttons() & 1 == 1 {
                cam_mouse
                    .borrow_mut()
                    .mouse_move(e.movement_x() as f32, e.movement_y() as f32);
            }
        }) as Box<dyn FnMut(_)>);
        window
            .add_event_listener_with_callback("pointermove", mouse_move.as_ref().unchecked_ref())
            .unwrap();
        mouse_move.forget();
    }
}
