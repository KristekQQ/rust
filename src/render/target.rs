//! Render inputs independent of any browser canvas or surface.
use glam::{Mat4, Vec3};

#[derive(Clone, Copy)]
pub struct RenderView {
    pub view_projection: Mat4,
    pub camera_position: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
impl Viewport {
    pub fn full(width: u32, height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }
    pub fn validate(self, width: u32, height: u32) -> Result<(), String> {
        if self.width == 0 || self.height == 0 || width == 0 || height == 0 {
            return Err("Render target and viewport must have nonzero dimensions".into());
        }
        if self
            .x
            .checked_add(self.width)
            .is_none_or(|right| right > width)
            || self
                .y
                .checked_add(self.height)
                .is_none_or(|bottom| bottom > height)
        {
            return Err("Viewport must fit inside the render target".into());
        }
        Ok(())
    }
}

/// Single-sample color and depth attachments. Their formats and dimensions
/// must match this descriptor; wgpu validates the underlying GPU resources.
pub struct RenderTarget<'a> {
    pub color: &'a wgpu::TextureView,
    pub depth: &'a wgpu::TextureView,
    pub color_format: wgpu::TextureFormat,
    pub depth_format: wgpu::TextureFormat,
    pub width: u32,
    pub height: u32,
    pub viewport: Viewport,
    /// Use Load for additional views sharing an attachment; Clear clears the whole attachment.
    pub color_load: wgpu::LoadOp<wgpu::Color>,
    pub depth_load: wgpu::LoadOp<f32>,
}
impl RenderTarget<'_> {
    pub fn validate(&self) -> Result<(), String> {
        self.viewport.validate(self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn viewport_accepts_full_target_and_subrect() {
        assert!(Viewport::full(800, 600).validate(800, 600).is_ok());
        assert!(Viewport {
            x: 400,
            y: 100,
            width: 400,
            height: 500
        }
        .validate(800, 600)
        .is_ok());
    }
    #[test]
    fn viewport_rejects_empty_outside_and_overflow() {
        assert!(Viewport::full(0, 600).validate(800, 600).is_err());
        assert!(Viewport::full(800, 600).validate(0, 600).is_err());
        assert!(Viewport {
            x: 401,
            y: 0,
            width: 400,
            height: 600
        }
        .validate(800, 600)
        .is_err());
        assert!(Viewport {
            x: u32::MAX,
            y: 0,
            width: 1,
            height: 1
        }
        .validate(u32::MAX, 600)
        .is_err());
    }
}

/// A reusable depth-only pass target, e.g. a shadow map or depth prepass.
pub struct DepthRenderTarget<'a> {
    pub depth: &'a wgpu::TextureView,
    pub format: wgpu::TextureFormat,
    pub width: u32,
    pub height: u32,
    pub viewport: Viewport,
    pub load: wgpu::LoadOp<f32>,
}
impl DepthRenderTarget<'_> {
    pub fn validate(&self) -> Result<(), String> {
        self.viewport.validate(self.width, self.height)?;
        if !self.format.has_depth_aspect() {
            return Err("Depth target requires a depth format".into());
        }
        Ok(())
    }
}
