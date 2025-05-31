#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

pub mod graphics;
pub mod input;
mod app;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
#[cfg(target_arch = "wasm32")]
pub async fn start() -> Result<(), JsValue> {
    app::run().await
}

#[cfg(not(target_arch = "wasm32"))]
pub fn start() -> Result<(), ()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::graphics::vertex::{VERTICES, INDICES};

    #[test]
    fn cube_vertex_count() {
        assert_eq!(VERTICES.len(), 24);
    }

    #[test]
    fn cube_index_count() {
        assert_eq!(INDICES.len(), 36);
    }
}
