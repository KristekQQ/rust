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

pub use crate::scene::MAX_LIGHTS;
pub const LIGHT_VERTICES_PER_LIGHT: usize = 8;
pub const EMPTY_LIGHT: Light = Light {
    position: [0.0, 0.0, 0.0],
    _pad_p: 0.0,
    color: [0.0, 0.0, 0.0],
    _pad_c: 0.0,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SceneUniforms {
    pub mvp: [[f32; 4]; 4],
    pub model: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub _pad0: f32,
    pub lights: [Light; MAX_LIGHTS],
}

pub fn grid_vertices(size: i32) -> Vec<Vertex> {
    let mut verts = Vec::new();
    let color = [0.3, 0.3, 0.3];
    let normal = [0.0, 1.0, 0.0];
    for i in -size..=size {
        let f = i as f32;
        verts.push(Vertex {
            position: [-size as f32, 0.0, f],
            color,
            normal,
        });
        verts.push(Vertex {
            position: [size as f32, 0.0, f],
            color,
            normal,
        });
        verts.push(Vertex {
            position: [f, 0.0, -size as f32],
            color,
            normal,
        });
        verts.push(Vertex {
            position: [f, 0.0, size as f32],
            color,
            normal,
        });
    }
    verts
}

pub fn light_rays(lights: &[Light]) -> Vec<Vertex> {
    let mut verts = Vec::new();
    let normal = [0.0_f32, 1.0, 0.0];
    let cross = 0.2_f32;
    for l in lights {
        let p = l.position;
        let color = l.color;
        // small cross marking the light position
        verts.push(Vertex {
            position: [p[0] - cross, p[1], p[2]],
            color,
            normal,
        });
        verts.push(Vertex {
            position: [p[0] + cross, p[1], p[2]],
            color,
            normal,
        });
        verts.push(Vertex {
            position: [p[0], p[1] - cross, p[2]],
            color,
            normal,
        });
        verts.push(Vertex {
            position: [p[0], p[1] + cross, p[2]],
            color,
            normal,
        });
        verts.push(Vertex {
            position: [p[0], p[1], p[2] - cross],
            color,
            normal,
        });
        verts.push(Vertex {
            position: [p[0], p[1], p[2] + cross],
            color,
            normal,
        });
        // line from light to origin
        verts.push(Vertex {
            position: p,
            color,
            normal,
        });
        verts.push(Vertex {
            position: [0.0, 0.0, 0.0],
            color,
            normal,
        });
    }
    verts
}

pub fn plane_mesh() -> (Vec<Vertex>, Vec<u16>) {
    let color = [0.25, 0.25, 0.28];
    let normal_up = [0.0, 1.0, 0.0];
    let normal_down = [0.0, -1.0, 0.0];
    let s = 0.5;
    let positions = [[-s, 0.0, -s], [s, 0.0, -s], [s, 0.0, s], [-s, 0.0, s]];
    let mut vertices = Vec::with_capacity(8);
    for p in positions {
        vertices.push(Vertex {
            position: p,
            color,
            normal: normal_up,
        });
    }
    for p in positions {
        vertices.push(Vertex {
            position: p,
            color,
            normal: normal_down,
        });
    }
    let indices: Vec<u16> = vec![
        0, 1, 2, 0, 2, 3, // top
        4, 6, 5, 4, 7, 6, // bottom
    ];
    (vertices, indices)
}

pub fn sphere_mesh(segments_u: u32, segments_v: u32) -> (Vec<Vertex>, Vec<u16>) {
    let color = [0.7, 0.7, 0.8];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let u_steps = segments_u.max(3);
    let v_steps = segments_v.max(2);
    for v in 0..=v_steps {
        let v_frac = v as f32 / v_steps as f32;
        let phi = v_frac * std::f32::consts::PI;
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        for u in 0..=u_steps {
            let u_frac = u as f32 / u_steps as f32;
            let theta = u_frac * std::f32::consts::TAU;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();
            let x = sin_phi * cos_theta;
            let y = cos_phi;
            let z = sin_phi * sin_theta;
            vertices.push(Vertex {
                position: [x, y, z],
                color,
                normal: [x, y, z],
            });
        }
    }
    let ring = u_steps + 1;
    for v in 0..v_steps {
        for u in 0..u_steps {
            let i0 = (v * ring + u) as u16;
            let i1 = (v * ring + u + 1) as u16;
            let i2 = ((v + 1) * ring + u) as u16;
            let i3 = ((v + 1) * ring + u + 1) as u16;
            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }
    (vertices, indices)
}
