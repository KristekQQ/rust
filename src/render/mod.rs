//! Shared wgpu renderer with platform-specific presentation adapters.
mod platform;

mod cut_feedback_gpu;
#[cfg(target_arch = "wasm32")]
pub mod cutting_compute;
#[cfg(target_arch = "wasm32")]
pub mod cutting_resident;
pub mod data;
pub mod depth;
mod depth_resolve;
mod font_glow;
mod half_glow;
pub(crate) mod neon_glow;
pub mod pipeline;
#[cfg(target_arch = "wasm32")]
pub mod state;

pub mod gltf_gpu;
pub mod graph_resources;
#[cfg(target_arch = "wasm32")]
pub mod output;
pub mod profiler;
pub mod renderer;
pub mod target;

pub mod effects;
pub mod textures;

mod hud;
pub mod postprocess;

pub mod environment;

#[cfg(target_arch = "wasm32")]
pub mod xr_output;
pub mod xr_present;

#[cfg(target_arch = "wasm32")]
pub mod graph_probe;
mod view_uniforms;

#[cfg(target_arch = "wasm32")]
mod xr_foveation;
#[cfg(target_arch = "wasm32")]
pub mod xr_gl_output;
