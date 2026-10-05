pub mod input;
#[cfg(target_arch = "wasm32")]
pub mod render;
pub mod scene;
pub mod visibility;
#[cfg(target_arch = "wasm32")]
pub mod web;

// Exercise the actual GPU mesh data and uniform layout on the native test host.
#[cfg(test)]
#[allow(dead_code)]
#[path = "render/data.rs"]
mod render_data_tests;
