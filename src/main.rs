#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
use webgpu_wasm::start;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    start().unwrap();
}
