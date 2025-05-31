use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;

use crate::input::{camera::Camera, keyboard, mouse, r#loop};
use crate::render::renderer::Renderer;

#[wasm_bindgen(start)]
pub async fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document
        .get_element_by_id("gpu-canvas")
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let renderer = Rc::new(RefCell::new(Renderer::new(&canvas).await?));
    let camera = Rc::new(RefCell::new(Camera::new(renderer.borrow().aspect())));

    keyboard::init(&window, camera.clone());
    mouse::init(&window, camera.clone());

    r#loop::run(window, camera, renderer);
    Ok(())
}
