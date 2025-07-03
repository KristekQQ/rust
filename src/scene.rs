use glam::Mat4;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Light {
    pub position: [f32; 3],
    pub _pad_p: f32,
    pub color: [f32; 3],
    pub _pad_c: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Node {
    pub model: [[f32; 4]; 4],
    pub parent: i32,    // -1 = no parent
    pub _pad: [i32; 3], // 16 B alignment
}

pub struct Scene {
    pub nodes: Vec<Node>,
    pub lights: Vec<Light>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            lights: Vec::new(),
        }
    }
}
