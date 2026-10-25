pub mod blade_physics;
pub mod collision;
pub mod combo_display;
mod contact_shape;
pub mod cutting;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod cutting_bench;
pub mod cutting_mesh;
pub mod cutting_tool;
pub mod debug_overlay;
pub mod frame_graph;
pub mod frame_metrics;
pub mod gltf_animation;
mod gltf_bounds;
pub mod gltf_loader;
pub mod gltf_model;
pub mod info_panel;
pub mod input;
pub mod melee_encounter;
pub mod postprocess_plan;
mod projectile;
#[cfg(target_arch = "wasm32")]
pub mod render;
pub mod render_appearance;
pub mod render_math;
pub mod rim_cut;
pub mod rim_game;
pub mod rim_movement;
pub mod scene;
mod spatial;
pub mod table_tennis;
pub mod table_tennis_contact;
#[cfg(any(test, target_arch = "wasm32"))]
mod translucency;
pub mod visibility;
#[cfg(target_arch = "wasm32")]
pub mod web;
pub mod xr_buttons;
pub mod xr_input;
pub mod xr_locomotion;
pub mod xr_player;
pub mod xr_round;

// Exercise the actual GPU mesh data and uniform layout on the native test host.
#[cfg(test)]
#[allow(dead_code)]
#[path = "render/data.rs"]
mod render_data_tests;

#[cfg(test)]
#[allow(dead_code)]
#[path = "render/target.rs"]
mod render_target_tests;

#[cfg(test)]
mod test_scenes;
