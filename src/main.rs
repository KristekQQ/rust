#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use webgpu_wasm::start as lib_start;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
#[cfg(target_arch = "wasm32")]
pub async fn start() -> Result<(), JsValue> {
    lib_start().await.map_err(Into::into)
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}

