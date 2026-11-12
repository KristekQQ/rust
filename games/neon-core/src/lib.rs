//! Shared Neon scene loading, complete gameplay and scores for native and web hosts.
//! The engine owns physics and scene state; hosts provide input and asset bytes.
pub mod content;
pub mod game;
pub mod scores;

pub use content::Content;
pub use game::{Game, Recipe};
pub use scores::Scores;
pub use webgpu_wasm::frame_input::FrameInput;
