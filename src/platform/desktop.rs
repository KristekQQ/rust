#[cfg(not(target_arch = "wasm32"))]
pub fn start() {
    // desktop startup not implemented
}

#[cfg(target_arch = "wasm32")]
pub fn start() {}
