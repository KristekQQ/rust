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
#[derive(Clone, Copy, PartialEq)]
pub struct Light {
    pub position: [f32; 3],
    pub _pad_p: f32,
    pub color: [f32; 3],
    pub _pad_c: f32,
}

pub use crate::scene::MAX_LIGHTS;
pub const LIGHT_VERTICES_PER_LIGHT: usize = 8 + 3 * 16 * 2;
pub const EMPTY_LIGHT: Light = Light {
    position: [0.0, 0.0, 0.0],
    _pad_p: 0.0,
    color: [0.0, 0.0, 0.0],
    _pad_c: 0.0,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SceneUniforms {
    pub view_projection: [[f32; 4]; 4],
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
        let intensity = l.color.into_iter().fold(0.0_f32, f32::max).max(0.0001);
        let color = l.color.map(|channel| channel / intensity);
        // Three wire rings make the source position visible from every direction.
        for axis in 0..3 {
            for segment in 0..16 {
                for endpoint in [segment, segment + 1] {
                    let angle = endpoint as f32 * std::f32::consts::TAU / 16.0;
                    let mut position = p;
                    position[(axis + 1) % 3] += 0.16 * angle.cos();
                    position[(axis + 2) % 3] += 0.16 * angle.sin();
                    verts.push(Vertex {
                        position,
                        color,
                        normal,
                    });
                }
            }
        }
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
        0, 2, 1, 0, 3, 2, // top: winding agrees with +Y normal
        4, 5, 6, 4, 6, 7, // bottom: winding agrees with -Y normal
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
            indices.extend_from_slice(&[i0, i1, i2, i1, i3, i2]);
        }
    }
    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn assert_winding_matches_normals(vertices: &[Vertex], indices: &[u16]) {
        let mut checked = 0;
        for triangle in indices.chunks_exact(3) {
            let a = vertices[triangle[0] as usize];
            let b = vertices[triangle[1] as usize];
            let c = vertices[triangle[2] as usize];
            let face = (Vec3::from(b.position) - Vec3::from(a.position))
                .cross(Vec3::from(c.position) - Vec3::from(a.position));
            // UV-sphere pole triangles intentionally have zero area.
            if face.length_squared() < 1e-12 {
                continue;
            }
            let normal = Vec3::from(a.normal) + Vec3::from(b.normal) + Vec3::from(c.normal);
            assert!(
                face.dot(normal) > 0.0,
                "triangle {triangle:?} faces against its normals"
            );
            checked += 1;
        }
        assert!(checked > 0);
    }

    #[test]
    fn all_meshes_have_outward_winding_consistent_with_lighting() {
        assert_winding_matches_normals(VERTICES, INDICES);
        let (vertices, indices) = plane_mesh();
        assert_winding_matches_normals(&vertices, &indices);
        let (vertices, indices) = sphere_mesh(32, 16);
        assert_winding_matches_normals(&vertices, &indices);
    }

    #[test]
    fn light_helpers_fit_the_gpu_buffer_and_mark_each_source() {
        let lights = [Light {
            position: [2.0, 3.0, 4.0],
            color: [0.2, 0.5, 1.0],
            _pad_p: 0.0,
            _pad_c: 0.0,
        }; MAX_LIGHTS];
        let vertices = light_rays(&lights);
        assert_eq!(vertices.len(), MAX_LIGHTS * LIGHT_VERTICES_PER_LIGHT);
        for helper in vertices.chunks_exact(LIGHT_VERTICES_PER_LIGHT) {
            assert_eq!(
                helper[LIGHT_VERTICES_PER_LIGHT - 2].position,
                lights[0].position
            );
            assert!(helper
                .iter()
                .all(|vertex| vertex.position.iter().all(|value| value.is_finite())));
        }
    }
    #[test]
    fn effects_uniform_layout_matches_wgsl() {
        assert_eq!(std::mem::size_of::<EffectsUniforms>(), 352);
        assert_eq!(
            std::mem::offset_of!(EffectsUniforms, reflection_matrix),
            256
        );
        assert_eq!(std::mem::offset_of!(EffectsUniforms, clip_plane), 320);
        assert_eq!(std::mem::offset_of!(EffectsUniforms, params), 336);
    }
    #[test]
    fn instance_material_layout_matches_shader() {
        use crate::visibility::InstanceData;
        assert_eq!(std::mem::size_of::<InstanceData>(), 144);
        assert_eq!(std::mem::offset_of!(InstanceData, material), 128);
        assert_eq!(
            InstanceData::layout()
                .attributes
                .last()
                .unwrap()
                .shader_location,
            11
        );
        assert_eq!(
            InstanceData::layout().attributes.last().unwrap().offset,
            128
        );
    }
    #[test]
    fn uniform_offsets_match_wgsl_alignment() {
        assert_eq!(std::mem::offset_of!(SceneUniforms, camera_pos), 64);
        assert_eq!(std::mem::offset_of!(SceneUniforms, lights), 80);
        assert_eq!(std::mem::size_of::<SceneUniforms>(), 208);
    }
}

impl crate::visibility::InstanceData {
    pub fn layout<'a>() -> VertexBufferLayout<'a> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![
            3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4,
            7 => Float32x4, 8 => Float32x4, 9 => Float32x4, 10 => Float32x4, 11 => Float32x4
        ];
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct EffectsUniforms {
    pub shadow_matrices: [[[f32; 4]; 4]; MAX_LIGHTS],
    pub reflection_matrix: [[f32; 4]; 4],
    pub clip_plane: [f32; 4],
    pub params: [f32; 4],
}
impl Default for EffectsUniforms {
    fn default() -> Self {
        Self {
            shadow_matrices: [glam::Mat4::IDENTITY.to_cols_array_2d(); MAX_LIGHTS],
            reflection_matrix: glam::Mat4::IDENTITY.to_cols_array_2d(),
            clip_plane: [0.0; 4],
            params: [0.0, 0.0, 0.0, 0.00005],
        }
    }
}
