#![cfg(target_arch = "wasm32")]

use wgpu::VertexBufferLayout;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub normal: [f32; 3],
}

impl Vertex {
    pub fn layout<'a>() -> VertexBufferLayout<'a> {
        use std::mem;
        VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: (mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

pub const VERTICES: &[Vertex] = &[
    // front - red
    Vertex {
        position: [-0.5, -0.5, 0.5],
        color: [1.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    },
    Vertex {
        position: [0.5, -0.5, 0.5],
        color: [1.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    },
    Vertex {
        position: [0.5, 0.5, 0.5],
        color: [1.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    },
    Vertex {
        position: [-0.5, 0.5, 0.5],
        color: [1.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    },
    // back - green
    Vertex {
        position: [0.5, -0.5, -0.5],
        color: [0.0, 1.0, 0.0],
        normal: [0.0, 0.0, -1.0],
    },
    Vertex {
        position: [-0.5, -0.5, -0.5],
        color: [0.0, 1.0, 0.0],
        normal: [0.0, 0.0, -1.0],
    },
    Vertex {
        position: [-0.5, 0.5, -0.5],
        color: [0.0, 1.0, 0.0],
        normal: [0.0, 0.0, -1.0],
    },
    Vertex {
        position: [0.5, 0.5, -0.5],
        color: [0.0, 1.0, 0.0],
        normal: [0.0, 0.0, -1.0],
    },
    // left - blue
    Vertex {
        position: [-0.5, -0.5, -0.5],
        color: [0.0, 0.0, 1.0],
        normal: [-1.0, 0.0, 0.0],
    },
    Vertex {
        position: [-0.5, -0.5, 0.5],
        color: [0.0, 0.0, 1.0],
        normal: [-1.0, 0.0, 0.0],
    },
    Vertex {
        position: [-0.5, 0.5, 0.5],
        color: [0.0, 0.0, 1.0],
        normal: [-1.0, 0.0, 0.0],
    },
    Vertex {
        position: [-0.5, 0.5, -0.5],
        color: [0.0, 0.0, 1.0],
        normal: [-1.0, 0.0, 0.0],
    },
    // right - yellow
    Vertex {
        position: [0.5, -0.5, 0.5],
        color: [1.0, 1.0, 0.0],
        normal: [1.0, 0.0, 0.0],
    },
    Vertex {
        position: [0.5, -0.5, -0.5],
        color: [1.0, 1.0, 0.0],
        normal: [1.0, 0.0, 0.0],
    },
    Vertex {
        position: [0.5, 0.5, -0.5],
        color: [1.0, 1.0, 0.0],
        normal: [1.0, 0.0, 0.0],
    },
    Vertex {
        position: [0.5, 0.5, 0.5],
        color: [1.0, 1.0, 0.0],
        normal: [1.0, 0.0, 0.0],
    },
    // top - cyan
    Vertex {
        position: [-0.5, 0.5, 0.5],
        color: [0.0, 1.0, 1.0],
        normal: [0.0, 1.0, 0.0],
    },
    Vertex {
        position: [0.5, 0.5, 0.5],
        color: [0.0, 1.0, 1.0],
        normal: [0.0, 1.0, 0.0],
    },
    Vertex {
        position: [0.5, 0.5, -0.5],
        color: [0.0, 1.0, 1.0],
        normal: [0.0, 1.0, 0.0],
    },
    Vertex {
        position: [-0.5, 0.5, -0.5],
        color: [0.0, 1.0, 1.0],
        normal: [0.0, 1.0, 0.0],
    },
    // bottom - magenta
    Vertex {
        position: [-0.5, -0.5, -0.5],
        color: [1.0, 0.0, 1.0],
        normal: [0.0, -1.0, 0.0],
    },
    Vertex {
        position: [0.5, -0.5, -0.5],
        color: [1.0, 0.0, 1.0],
        normal: [0.0, -1.0, 0.0],
    },
    Vertex {
        position: [0.5, -0.5, 0.5],
        color: [1.0, 0.0, 1.0],
        normal: [0.0, -1.0, 0.0],
    },
    Vertex {
        position: [-0.5, -0.5, 0.5],
        color: [1.0, 0.0, 1.0],
        normal: [0.0, -1.0, 0.0],
    },
];

pub const INDICES: &[u16] = &[
    0, 1, 2, 0, 2, 3, // front
    4, 5, 6, 4, 6, 7, // back
    8, 9, 10, 8, 10, 11, // left
    12, 13, 14, 12, 14, 15, // right
    16, 17, 18, 16, 18, 19, // top
    20, 21, 22, 20, 22, 23, // bottom
];

pub fn as_bytes<T: Copy>(data: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            data.as_ptr() as *const u8,
            data.len() * std::mem::size_of::<T>(),
        )
    }
}

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
pub struct SceneUniforms {
    pub mvp: [[f32; 4]; 4],
    pub model: [[f32; 4]; 4],
    /// Normal matrix padded to vec4 rows so that the layout matches WGSL
    pub normal_matrix: [[f32; 4]; 3],
    pub camera_pos: [f32; 3],
    pub _pad0: f32,
    pub lights: [Light; 2],
}
