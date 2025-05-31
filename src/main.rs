fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    webgpu_wasm::start().expect("application start failed");
}
