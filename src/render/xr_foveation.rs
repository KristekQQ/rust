//! Experimental scene-texture FFR. Never changes XR framebuffer attachments.
use glow::HasContext;
use wasm_bindgen::{JsCast, JsValue};
struct Entry {
    key: glow::Texture,
    // A JS wrapper does not prevent wgpu's explicit deleteTexture. Check isTexture
    // before cleanup; never dereference an evicted generational glow key.
    texture: web_sys::WebGlTexture,
}
pub struct GlFoveation {
    binding: JsValue,
    method: js_sys::Function,
    level: f32,
    textures: Vec<Entry>,
    calls: u32,
    failures: u32,
    error: Option<String>,
    rollback_error: Option<String>,
}
fn message(error: &JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}
impl GlFoveation {
    pub fn new(binding: JsValue, level: f32) -> Result<Self, JsValue> {
        if !level.is_finite() || !(0.0..=1.0).contains(&level) {
            return Err(JsValue::from_str("Invalid scene foveation level"));
        }
        let method = js_sys::Reflect::get(&binding, &"foveateBoundTexture".into())?
            .dyn_into::<js_sys::Function>()
            .map_err(|_| JsValue::from_str("Scene texture foveation unavailable"))?;
        Ok(Self {
            binding,
            method,
            level,
            textures: Vec::with_capacity(8),
            calls: 0,
            failures: 0,
            error: None,
            rollback_error: None,
        })
    }
    pub fn stats(&self) -> serde_json::Value {
        serde_json::json!({"status":if self.rollback_error.is_some(){"rollback-failed"}else if self.error.is_some(){"failed"}else{"configured"},
            "level":self.level,"calls":self.calls,"failures":self.failures,
            "trackedTextures":self.textures.len(),"error":self.error,"rollbackError":self.rollback_error,
            "hardwareEffect":"unverified"})
    }
    pub fn reset(&mut self, device: &wgpu::Device) -> Result<(), JsValue> {
        let mut index = 0;
        let mut error = None;
        while index < self.textures.len() {
            let result =
                with_browser_texture(device, &self.textures[index].texture, || self.call(0.0));
            if let Err(problem) = result {
                error = Some(problem);
                index += 1;
            } else {
                self.textures.remove(index);
            }
        }
        self.rollback_error = error.as_ref().map(message);
        error.map_or(Ok(()), Err)
    }
    fn call(&self, level: f32) -> Result<(), JsValue> {
        self.method
            .call2(
                &self.binding,
                &JsValue::from(web_sys::WebGl2RenderingContext::TEXTURE_2D),
                &JsValue::from(level),
            )
            .map(|_| ())
    }
    pub fn apply(
        &mut self,
        device: &wgpu::Device,
        textures: &[&wgpu::Texture],
    ) -> Result<(), JsValue> {
        if self.rollback_error.is_some() {
            return Err(JsValue::from_str("Scene FFR rollback failed; XR must end"));
        }
        if self.error.is_some() || textures.len() > 2 {
            return Ok(());
        }
        let mut keys = [None; 2];
        for (index, texture) in textures.iter().enumerate() {
            // SAFETY: read identity only; do not destroy/mutate HAL resources.
            let Some(raw) = (unsafe { texture.as_hal::<wgpu::hal::api::Gles>() }) else {
                return Ok(());
            };
            match raw.inner {
                wgpu::hal::gles::TextureInner::Texture { raw, target }
                    if target == web_sys::WebGl2RenderingContext::TEXTURE_2D =>
                {
                    keys[index] = Some(raw)
                }
                _ => return Ok(()),
            }
        }
        if keys
            .iter()
            .flatten()
            .all(|key| self.textures.iter().any(|entry| &entry.key == key))
        {
            return Ok(());
        }
        let result = (|| {
            for &key in keys.iter().flatten() {
                if self.textures.iter().any(|entry| entry.key == key) {
                    continue;
                }
                if self.textures.len() == 8 {
                    // Reset before forgetting a live resource, never silently
                    // leave a foveated texture outside the bounded cleanup set.
                    let index = self
                        .textures
                        .iter()
                        .position(|entry| !keys.contains(&Some(entry.key)))
                        .expect("at most two current targets");
                    with_browser_texture(device, &self.textures[index].texture, || self.call(0.0))?;
                    self.textures.remove(index);
                }
                with_hal_texture(device, key, |texture| {
                    // Track before the native call: even a partially failing
                    // implementation belongs to the rollback set.
                    self.textures.push(Entry {
                        key,
                        texture: texture.clone(),
                    });
                    self.calls += 1;
                    self.call(self.level)
                })?;
            }
            Ok::<_, JsValue>(())
        })();
        if let Err(error) = result {
            self.failures += 1;
            self.error = Some(message(&error));
            // Reset ALL earlier eyes/MRT groups, not just the failing group.
            // If rollback itself fails, do not run XR with mixed preferences.
            self.reset(device).map_err(|error| {
                JsValue::from_str(&format!(
                    "Scene FFR rollback failed; XR must end: {}",
                    message(&error)
                ))
            })?;
        }
        Ok(())
    }
}
fn gl_context(device: &wgpu::Device) -> Result<web_sys::WebGl2RenderingContext, JsValue> {
    super::xr_gl_output::context(device)
}
fn with_hal_texture(
    device: &wgpu::Device,
    key: glow::Texture,
    call: impl FnOnce(&web_sys::WebGlTexture) -> Result<(), JsValue>,
) -> Result<(), JsValue> {
    let gl = gl_context(device)?;
    let active = gl
        .get_parameter(web_sys::WebGl2RenderingContext::ACTIVE_TEXTURE)?
        .as_f64()
        .ok_or_else(|| JsValue::from_str("Invalid GL active texture"))? as u32;
    let previous = gl.get_parameter(web_sys::WebGl2RenderingContext::TEXTURE_BINDING_2D)?;
    // SAFETY: borrowed same-device context. Bind through glow's handle table;
    // release HAL guard before JS. Queue resets state before each command buffer.
    {
        let hal = unsafe { device.as_hal::<wgpu::hal::api::Gles>() }
            .ok_or_else(|| JsValue::from_str("Scene FFR requires GLES"))?;
        unsafe {
            hal.context()
                .lock()
                .bind_texture(web_sys::WebGl2RenderingContext::TEXTURE_2D, Some(key));
        }
    }
    let result = (|| {
        let texture = gl
            .get_parameter(web_sys::WebGl2RenderingContext::TEXTURE_BINDING_2D)?
            .dyn_into::<web_sys::WebGlTexture>()
            .map_err(|_| JsValue::from_str("Scene FFR texture unavailable"))?;
        call(&texture)
    })();
    gl.active_texture(active);
    gl.bind_texture(
        web_sys::WebGl2RenderingContext::TEXTURE_2D,
        previous.dyn_ref(),
    );
    result
}
fn with_browser_texture(
    device: &wgpu::Device,
    texture: &web_sys::WebGlTexture,
    call: impl FnOnce() -> Result<(), JsValue>,
) -> Result<(), JsValue> {
    let gl = gl_context(device)?;
    if !gl.is_texture(Some(texture)) {
        return Ok(());
    }
    let active = gl
        .get_parameter(web_sys::WebGl2RenderingContext::ACTIVE_TEXTURE)?
        .as_f64()
        .ok_or_else(|| JsValue::from_str("Invalid GL active texture"))? as u32;
    let previous = gl.get_parameter(web_sys::WebGl2RenderingContext::TEXTURE_BINDING_2D)?;
    gl.bind_texture(web_sys::WebGl2RenderingContext::TEXTURE_2D, Some(texture));
    let result = call();
    gl.active_texture(active);
    gl.bind_texture(
        web_sys::WebGl2RenderingContext::TEXTURE_2D,
        previous.dyn_ref(),
    );
    result
}
