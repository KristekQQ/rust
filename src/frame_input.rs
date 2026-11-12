//! Platform-neutral frame input shared by browser and native game hosts.
#[derive(Clone, Copy, Default)]
pub struct FrameInput {
    pub dt: f32,
    pub head: Option<glam::Mat4>,
    pub hands: [Option<glam::Mat4>; 2],
    pub rays: [Option<glam::Mat4>; 2],
    pub axes: [f32; 3],
    pub triggers: [bool; 2],
    pub primary: bool,
}
