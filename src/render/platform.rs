//! Small host services; resource preparation and GPU commands stay shared.
#[cfg(target_arch = "wasm32")]
pub fn performance() -> Option<web_sys::Performance> {
    web_sys::window().and_then(|w| w.performance())
}
#[cfg(not(target_arch = "wasm32"))]
pub struct Clock;
#[cfg(not(target_arch = "wasm32"))]
impl Clock {
    pub fn now(&self) -> f64 {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_secs_f64()
            * 1000.0
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub fn performance() -> Option<Clock> {
    Some(Clock)
}

pub fn report_error(error: &str) {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(error));
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{error}");
}
pub fn modern_gpu(device: &wgpu::Device) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        device.as_webgpu().is_some()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = device;
        true
    }
}
