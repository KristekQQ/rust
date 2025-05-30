#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};

use webgpu_wasm::graphics;
use webgpu_wasm::input;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
#[cfg(target_arch = "wasm32")]
pub async fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document
        .get_element_by_id("gpu-canvas")
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let state = graphics::pipeline::State::new(&canvas).await?;
    let cam = Rc::new(RefCell::new(input::camera::Camera::new(state.aspect())));
    input::controls::hook_events(&window, cam.clone());
    input::controls::run_event_loop(window, cam, state)
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}

